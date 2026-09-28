# Path B: drawing into the window's own pass

- **Status:** proposed. Nothing of it is implemented.
- **Assumes:** [`renderer-seam.md`](renderer-seam.md) — a renderer is installable at all.
- **Companion:** [`foreign-texture.md`](foreign-texture.md) is the other path; the two
  share one scene primitive.
- **Target crates:** `gpui_engine` (the primitive and the token), `gpui_platform` (the
  trait), and every renderer that draws a window.

## 1. What this is

An application draws its own geometry into the pass GPUI is already rendering the window
with, between the UI quads that come before it and those that come after. There is no
intermediate texture at all — which is why it is the cheapest path of the four
([`README.md`](README.md) §"The trade-off") and the only one that can draw a mesh too
large to rasterise into a fixed-size target.

It is the path with the real hazard. A custom pipeline leaves the GPU in a state the UI
quad batcher does not expect, and the symptom is not a crash but wrong pixels in
everything drawn afterwards.

## 2. The scene type, and why it carries a token

```rust
// crates/gpui_engine/src/scene.rs — the sibling of CustomRenderPrimitive::Texture
Inline {
    order: DrawOrder,               // assigned by Scene::insert_primitive
    token: InlineToken,             // renderer-owned; not an erased callback
    bounds: Bounds<ScaledPixels>,
    content_mask: ContentMask<ScaledPixels>,
},
```

The drafts put a `Box<dyn Fn(&mut DrawContext) + Send + Sync>` here, and that cannot
work: the thing the callback wants is the window's *live* render encoder, which cannot
cross an `Any` boundary because `Any: 'static`, and naming its type would put `wgpu` in
`gpui_engine`. So the scene carries an opaque token and the renderer that minted it
resolves it — the callback stays in the renderer, where the encoder is, and the engine
transports a handle exactly as it does for a texture.

The same device model as [`foreign-texture.md`](foreign-texture.md) §2 applies here, and
more sharply: the injected commands execute in *GPUI's* pass, so every resource they bind
— vertex buffer, pipeline, bind group — must belong to that pass's device. Path B is
therefore a window-owner capability too, and on Windows it too requires installing
`WgpuRenderer`. That is what makes the token more than a convenience: a callback holding
resources from its own device would fail inside the pass, so the callback belongs in the
renderer, where the device already is. The whole of it is
[`../decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md).

Which is where the two paths differ in kind, and in what they share: Path A shares the
device and the *queue*, and orders by submission; Path B shares the *encoder* itself, and
orders by the command's position in the pass ([`foreign-texture.md`](foreign-texture.md) §6).

**`order` is the scene's, not the element's.** `Scene::insert_primitive` assigns it
(`crates/gpui_engine/src/scene.rs:85`, `:95`) and overwrites each primitive's field
(`:102`–`:131`). An earlier draft had the element call `cx.current_paint_order()`, which
does not exist and is not needed. The element supplies `bounds` and the `content_mask`
from `window.content_mask()` (`crates/gpui_authoring/src/window.rs:4588`), and the scene
orders it against its siblings.

`content_mask` is not decoration: without it the injected commands draw outside the
element's box, over the UI around it. `PaintSurface` carries one
(`crates/gpui_engine/src/scene.rs:749`); the drafts' `InlineCommand` did not.

## 3. The pipeline-state isolation matrix

The whole contract, and it is the part that is unaffected by any of the API corrections:

| subsystem | changed by an injected shader | restored by the renderer |
| --- | --- | --- |
| scissor | clamped to `primitive.bounds` | the full target rect |
| viewport | possibly local bounds | `(0, 0, device_w, device_h)` |
| pipeline | custom VS/FS | the quad and text pipelines |
| depth/stencil | custom tests and masks | disabled |
| blend | custom or additive | `SrcAlpha, OneMinusSrcAlpha` |
| vertex buffers | slots `[0..N]` overwritten | GPUI's instance buffer at slot 0 |
| samplers | sampler registers changed | the atlas sampler |

Two refinements. The scissor and viewport must be recomputed **from the primitive's
bounds on every injected command**, not set once, because two canvases in one frame is
the normal case. And with `order` coming from the scene (§2) the "restore the batch
cursor" step is about the primitive sitting *inside* the batch list rather than after it:
the renderer must be able to resume the quad batch it interrupted, not merely reset GPU
state.

RAII is the right shape for the restore — a guard that snapshots and rebinds on drop —
because the one case that has to be correct is the early return.

## 4. The coordinate bridge

The application's shader should not have to repeat GPUI's projection. Two things are
provided:

- **The device scissor**, clamped to integer pixels:
  `x = (bounds.origin.x * scale_factor).floor()`, `w = (bounds.size.width *
  scale_factor).ceil()`, and likewise for y and h, clamped to the target.
- **An orthographic matrix** taking local element coordinates `[0, width] × [0, height]`
  to clip space, with Y flipped for GPUI's top-left origin:

  ```
  [ 2/w   0    0   0 ]
  [  0   -2/h  0   0 ]
  [  0    0    1   0 ]
  [ -1    1    0   1 ]
  ```

The drafts gave the scissor function `self.scale_factor` but then defined the Wayland
target's `scale_factor` as `1.0` ("handled via Wayland viewport"), which would compute the
wrong rectangle on a scaled display. Whatever carries the geometry must carry one
scale factor, and it must be the one the surface is actually at.

The matrix is an aid, not a contract: a renderer is free to expose the viewport and let
the shader build its own, and an application that wants 3D will.

## 5. The wgpu arm has to be designed, not sketched

For [`foreign-texture.md`](foreign-texture.md) the wgpu arm is a no-op to write. Here it
is the one that is genuinely unknown, and the drafts' "conclude the active
`PrimitiveBatch::Surfaces` or flush the quad draw call" is not something wgpu's batcher
exposes. The arm has to be designed against
`crates/gpui_wgpu/src/wgpu_renderer.rs` — its batch enum, where quads are submitted, and
how a pass is held — rather than by analogy from Metal and DirectX, whose encoders work
differently.

Two constraints that shape it:

- **A `wgpu::RenderPass` borrows its encoder**, so it cannot be stored as
  `&'static mut` and handed across an `Any`. The callback therefore cannot be *called
  with* the pass by the engine layer; the renderer has to invoke it from inside the frame
  it owns, which is what the token in §2 is for.
- **`PrimitiveBatch::Surfaces` is empty today**
  (`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`), so both paths land in the same place in
  that file.

## 6. Ordering, and the two cases that are not supported

- **Between siblings** the scene handles it: `order` interleaves the injected command
  with the quads around it, so a canvas inside a `div()` draws between that `div`'s
  background and the widgets after it.
- **Under its own box's border** is *not* what happens:
  [`gpu-canvas.md`](../authoring/gpu-canvas.md) delegates to `Div` and pushes the primitive after the
  box paints, so a border draws under the GPU content. A canvas with UI both under and
  over it inside one box is what would reopen that choice.

## 7. Open

- **The wgpu arm's shape** (§5) is the one piece of this design that is not yet designed.
- **Whether the token registry is per frame or per window.** A token names a command
  block; if the block can outlive a frame (a cached pipeline, say) the registry is the
  window's, and if not it is the frame's, like the foreign-texture registry's ids.
- **How much of §4 a renderer must expose.** The scissor is required; the matrix is
  convenience. Whether they are one type or two follows from what the wgpu arm turns out
  to need.
