# The surface seam: what the engine owns, and what a producer resolves

- **Status:** settled — the rule the Linux surface arm already follows, written down so it is not
  re-litigated. It names a boundary the surface work keeps rediscovering; it is not a new mechanism.
- **Companion:** [`surfaces.md`](surfaces.md), the design of record for external pixels;
  [`foreign-texture.md`](foreign-texture.md) §4, the colour-space invariant on the RGBA arm;
  [`frame-formats.md`](frame-formats.md), the vocabulary this rule admits.

## 1. Two axes, felt as one

Video reaches the surface path as an apparently endless list — container, framing, codec, profile,
pixel format, layout, colour matrix, range, chroma siting, bit depth. That list is two axes wearing
one coat:

| axis | crosses the seam? | bounded? |
| --- | --- | --- |
| **Describing pixels the GPU must sample** — pixel format, plane layout, tiling, fence, colour transform | yes | **bounded** — the vocabulary of a DRM / CoreVideo / DXGI buffer, a finite set |
| **Video** — container, elementary-stream framing, codec, profile, parsing, seeking, timestamps, pacing, buffering | no | unbounded — and it must never cross |

Colour space looked like the first axis reaching into new territory; it is not. The renderer *does*
the sampling, so which YCbCr it is sampling is the renderer's business. The parser, the decoder, the
container — none of that has touched a line of `gpui`, and none of it should.

## 2. The rule

**The engine's surface vocabulary is closed and total. A producer resolves every video-domain unknown
before the buffer crosses.**

"Total" is the load-bearing word: no engine type carries an *unknown* variant. `YuvRange` is
`Full | Limited`, not `… | Unspecified`; `YuvMatrix` is `Bt601 | Bt709 | Bt2020`, not `… | Unspecified`.
The producer decides `unspecified → limited` and `unspecified → 601-or-709-by-height`, because those
are video conventions, and hands the engine a *concrete* description. The engine never guesses, and
never has an unhandled case it answers by falling back to a default the producer cannot see.

This is the whole of it:

- **Engine** — a description of a buffer a GPU can sample. Plain data: format, layout, tiling, fence,
  colour transform. It performs no interpretation and makes no video-shaped decision.
- **Producer** — everything that turned a stream into that description, including every default the
  standards leave open.
- **Renderer** — maps the description to its own sampling path. It knows a colour matrix is a 4×4; it
  does not know what "BT.709" means in the abstract, and does not need to.

## 3. The smell test

**If a name mentions a codec or a container, it is on the wrong side of the seam.** Nothing in
`gpui_engine` may say `H264`, `AV1`, `AnnexB`, `mp4`, or `keyframe`; nothing a decoder learns about a
stream may appear in a `DmaBufHandle` beyond what the GPU needs to sample one frame. A type named
`SurfaceFormat` is right; a type named `DecodedVideoFrame` is the seam leaking.

## 4. Worked examples from this branch

- **Colour space.** The engine took a `YuvColorSpace { matrix, range }` — total, no `unspecified` — and
  the renderer turned it into a `4×4`. All the standards-churn lives behind the seam: `gpui_va` reads
  `colorspace`/`color_range` off the frame and resolves a stream that names neither by height. Adding
  BT.2020 was a variant on an engine enum and one row in a table; it needed no new engine concept.
- **Chroma reconstruction.** The engine carries `ChromaReconstruction` — a *hint* (`Bilinear |
  LumaGuided`) — and the producer sets the policy. The engine never decides what looks better.
- **(c), the container, when it lands.** `libavformat` will demux `mp4`/`mkv`/`ts` inside `gpui_va`
  and change **nothing** in `gpui`. That it is a zero-engine-change step is the rule working.

## 5. What stays in the engine, and it is a short list

Stuff the *renderer* must act on, and only that:

- the pixel format and its layout (see [`frame-formats.md`](frame-formats.md) for the full vocabulary);
- the DRM modifier (tiling/compression);
- the colour transform — matrix and range now, primaries and transfer function if HDR is ever a
  target, chroma siting if co-sited sampling is ever wrong for a real producer;
- the fences — the acquire fence a consumer waits on, and the release descriptor it signals back; both
  are on the handle now, and together they are the handshake a recycling producer needs.

When that list is exhausted the interface is **done**, because DRM/VA-API's format space is finite.
Everything that remains open-ended after that is the producer's, forever.

## 6. Cross-platform: one vocabulary, three handles

The *concepts* are identical on every platform — an N-plane YCbCr buffer with a subsampling, a bit
depth, a range and a matrix; a tiled layout; a fence. The *handles* and the *native format names* are
not, and should not be forced into one:

- **Linux** — a dma-buf: fourcc + modifier + per-plane stride/offset; VA-API fourccs.
- **macOS** — an `IOSurface` / `CVPixelBuffer`, with a `CVPixelFormatType` (`420v`, `x420`, …).
- **Windows** — a DXGI texture, with a `DXGI_FORMAT` (`NV12`, `P010`, `YUY2`, …).

So the vocabulary lives in the engine as platform-neutral *concepts*, and each backend carries its
platform's handle and maps the concepts onto that platform's native names. A producer on a given
platform meets that platform's handle; the colour-space and layout *enums* are shared, because the
question "which matrix, which range, how many planes" is the same question everywhere. See
[`frame-formats.md`](frame-formats.md) §6 for the mapping table.

## 7. Why this is the thin surface, and what it costs

The engine stays small because it only ever grows by *dimensions a GPU samples*, not by *formats the
world invents*. The cost is borne where it belongs: a producer must resolve every default, and a
producer that refuses to (that hands over "unspecified" and expects the renderer to guess) is not
served. That is the correct pressure — the producer is the only party that knows the stream.
