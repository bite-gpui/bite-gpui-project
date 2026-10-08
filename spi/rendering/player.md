# The player: a real stream, end to end

- **Status:** built and run on `bite_v1.23.1-pre-interop` (unmerged). The demo is
  `crates/gpui/examples/player.rs`; the decoder behind it is `crates/gpui_va`.
- **Ref:** this work is not on the canonical `bite_v1.23.1-pre`, so the citations below name files
  and symbols rather than `path:line`; they re-point when the branch merges. Where a claim is about
  the canonical ref it says so.
- **Reads with:** [`surface-seam.md`](surface-seam.md) (the rule the player keeps re-meeting),
  [`frame-formats.md`](frame-formats.md) (the vocabulary the decoder resolves), and
  [`probe-p3-dmabuf-import.md`](probe-p3-dmabuf-import.md) (the import the player feeds).

## 1. What it is

The player is the consumer end of the surface path: a stream is hardware-decoded by VA-API, each
frame exports its surface as a dma-buf under the driver's modifier, and the window's renderer
imports that buffer and composites it through `surface()` — no CPU copy, no GPU copy.

```
H.264 → libavcodec (h264 + hw_device_ctx) → VA-API decode
      → VASurfaceID (NV12, I915_FORMAT_MOD_Y_TILED) → vaExportSurfaceHandle → dma-buf
      → gpui_wgpu imports under the modifier → NV12 shader → window
```

Run it:

```sh
cargo run -p gpui --example player                       # the bundled clip
cargo run -p gpui --example player -- path/to/movie.mp4  # a container, or a raw .264
```

It plays any stream libavformat opens — a container, or a raw H.264 elementary stream (Annex B) —
and paces itself at the rate the stream declares.

## 2. The release invariant, which is the whole game

A bounded playback hands out a surface and waits for the renderer to **release** it before handing
out another. The handle carries the release descriptor the renderer signals from the completion
callback of the submission that sampled the buffer. The rule that keeps biting:

> **A frame taken and never composited is a frame never released.** The renderer can only release a
> scene it actually receives; a frame the application took but did not draw into a presented scene
> is invisible to it, so its descriptor never signals and the bounded ring waits on it for good.

Two consequences the player had to learn, each costing a wedge before it was understood:

1. **Take the frame this render will show, before building the picture.** A frame taken and left for
   the next render is orphaned if that render never comes — the window losing focus mid-frame is
   exactly that — and one orphaned frame blocks the ring forever, because `recycle` cannot pop the
   oldest entry and the frame cannot be released until it is shown.
2. **Take a frame only while the window is shown.** A frame drawn for a hidden window is never
   presented, so it is never released either; the player stops advancing, and stops driving its own
   frame loop, while `Window::is_visible()` is false, and resumes when the platform next requests a
   frame.

`Playback::live` and `Playback::signalled` (`crates/gpui_va/src/gpui_va.rs`) exist so a consumer can
see the ring: a stuck `4 live` with `signalled < 3` is this bug.

The renderer's half is in `crates/gpui_wgpu/src/wgpu_renderer.rs`: the releases are collected from
the *whole* scene (so a culled surface is still released) and registered on both the sampling
submission and the path a frame that is not presented takes (`release_unpresented`).

## 3. What the decoder had to add

- **Containers.** A container is demuxed by libavformat, not framed by the decoder's own parser:
  `Demuxer` opens the file, finds the video stream, copies its codec parameters (which carry the
  out-of-band `avcC` parameter sets), and hands the decoder one framed packet at a time.
  `ffmpeg::load` loads an optional `libavformat.so.62`; without it, raw elementary streams still
  decode.
- **Plain H.264 Baseline.** Profile 66 with no constraint flag has no exact entry in ffmpeg's VA-API
  profile map, so the hwaccel refuses it and falls back to software, whose frames have no surface to
  export. ffmpeg's documented opt-in `AV_HWACCEL_FLAG_ALLOW_PROFILE_MISMATCH` lets it configure the
  closest profile the driver supports. This is not a bitstream patch: it is the API ffmpeg added for
  exactly this, and the decoder warns once if a frame still comes back in a software format.

## 4. Findings that are upstream bugs, not ours

1. **A Wayland frame demand can be lost while awaiting a compositor callback.**
   `crates/gpui_linux/src/linux/wayland/window.rs` makes the render loop demand-driven, and its
   `schedule_frame` treats every state but `Parked`/`Ticking` as "a tick is already armed". Once a
   frame has presented the loop sits in `AwaitingCallback`, waiting on the compositor's
   `wl_callback`; a compositor that stops repainting a surface (occluded, `suspended`, on no output)
   never sends it, and every later demand — `on_next_frame`, `App::flush_effects`, a focus
   `refresh` — is dropped. The window is stranded with no wake that can revive it.
   **Upstream has the same hole** (`schedule_frame`'s `_ => {}` is unchanged on
   `zed-industries/zed` `main`); the fork arms the retry timer as a fallback when demand arrives in
   `AwaitingCallback`, races the callback against it, and retires the loser with a token.
2. **A scene the renderer does not present can leave a producer waiting.** A dma-buf surface's
   release descriptor was registered only for the surfaces `draw_surfaces` actually sampled — so a
   frame the renderer declined (culled, or not reached past the surface-texture acquisition) left
   the producer waiting for good. The fork collects releases from the whole scene, and releases a
   scene it does not present.

Both are candidates for an upstream PR; neither is fork-specific.

## 5. Cost, measured

Instrumented with an `InstrumentedPipeline` (`PhaseMetrics`) and a timer around the decode, the
per-frame split on the HD 520 with `killer_cuts.mp4` was:

| pass | per frame |
| --- | --- |
| evaluate | 0.02 ms |
| **layout** | **2.8 ms** |
| paint | 0.41 ms |
| advance (decode + dma-buf export) | 0.77 ms |

- **Decode is not the cost.** `advance` is 0.77 ms and runs ~25×/s; the hardware path is doing its
  job. CPU is 16% because the **layout pass runs every frame**.
- **The layout engine clears and rebuilds every frame** (`LayoutEngine::clear()` per frame, no
  identity). Even a one-child tree costs ~2 ms, so drawing at the display's 60 Hz for a 25 fps
  stream pays for ~35 layouts a second the picture did not change.
- **`cached()` helps but cannot fix it.** Putting the picture in a cached child view trimmed layout
  2.83 → 2.16 ms, but the cost is per-frame, not per-subtree — and the cache's one-frame lag is
  fatal under any frame deferral (see §6).
- **The lever is the frame rate.** `StandardImmediatePipeline.max_fps(30)` defers frames that arrive
  too soon, and a deferred frame never reaches `evaluate`/`layout`/`paint`. That halves the layout
  cost. The exact-rate version — render at the stream's own period rather than capping — is the
  honest fix; the real fix is a layout engine that does not rebuild from scratch each frame.

## 6. The trap a frame cap sets for a surface producer

A deferred frame is a frame the application is told to draw and then not drawn. With the picture in
a cached child, `advance()` hands the new handle to the child during the render, so the cached
subtree still shows the previous frame this draw; the draw that would present the new one is the
next, and `max_fps` defers whichever draw lands too soon. The 40 ms stream clock beating against the
33 ms cap let frames fall through the gap — half never presented, `signalled` alternating, the ring
wedged at 4.

The lesson is the same one §2 states: **a frame cap is only safe if every frame the application
takes is drawn into the scene of the render that takes it.** A pipeline that can defer a draw after
demand was delivered is otherwise a source of orphans for any `Playback`-shaped producer.

## 7. Release notes

For the fork's next release (per the `bite-distribution` tag convention):

- Fixed the player stalling after the window is occluded or unfocused — a frame taken but not
  composited leaked a surface, and the bounded playback waited on it for good.
- Play containers (MP4, Matroska) and plain-baseline H.264, which previously fell back to software
  decode.
- Fixed a Wayland window being stranded when the compositor stops sending frame callbacks.
- The player renders at the stream's rate and reports where a frame's time goes; layout, not decode,
  is the cost.
