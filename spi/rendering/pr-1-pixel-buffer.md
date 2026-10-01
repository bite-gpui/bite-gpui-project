# PR 1 — the body, ready to open

- **Target:** `zed-industries/zed`, `main`. **Opened:** not yet.
- **The plan:** [`../upstream-prs.md`](../upstream-prs.md) §2. **Precondition:** none — this is the
  independent one, and it ships first.
- **Built in the fork:** `crates/gpui_engine/src/renderer.rs:19` (`PixelBuffer`), `:102`/`:110`/`:115`
  (the `render_scene` / `read_pixels` / `render_scene_to_image` split), `:80` (the trait). The fork's
  paths differ from upstream's because the fork is split into layer crates; the code is the same.

## Title

`gpui: split offscreen rendering from readback, and drop the image dependency from SceneRenderer`

## Body

### Motivation

`SceneRenderer` couples two things that do not need to be coupled: **rendering** a scene into a target,
and **copying** the result back to the CPU. It does the copy as part of the same call, and it names
`image::RgbaImage` in its signature — so every renderer, including the ones that never read pixels
back, depends on an image codec for a type it does not use.

That coupling is what stands in the way of headless rendering (server-side capture, snapshotting, and
embedding GPUI where there is no window): the GPU usually wants the frame **on the GPU**, and only a
consumer that genuinely needs bytes should pay for the round trip. It is also one fewer dependency on
a core trait.

### What this does

- Adds `PixelBuffer` — RGBA8, tightly packed, no external dependency — and uses it wherever
  `image::RgbaImage` was named.
- Splits `SceneRenderer::render_scene(scene, size)` (encode and submit into an offscreen target, **no
  readback**) from `read_pixels()` (copy that target to a `PixelBuffer`), and keeps
  `render_scene_to_image` as the two together for callers that want both.
- Gives `render_scene` and `read_pixels` default implementations that report themselves unsupported,
  so an onscreen renderer still only *needs* `draw` and `sprite_atlas`.

### Shape note

`PixelBuffer` is deliberately minimal: **RGBA8, unpremultiplied, tightly packed, top-left origin**. A
backend whose target is BGRA converts while it copies, which is where the swizzle already happens. I
did **not** add a `format`/`stride` field — happy to widen the type at the boundary if you would
prefer that, but the minimal shape is what the renderers here need.

### Testing

The offscreen path is exercised by the existing tests: a known scene renders offscreen and reads back
the expected pixels. No behavior change for onscreen rendering — the default implementations report
unsupported exactly as before.

### Notes

Split out of a larger interop effort; this piece is independent and has no bearing on the surface work
that follows it.
