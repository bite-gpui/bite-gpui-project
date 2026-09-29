# Path A: importing a texture produced outside GPUI

- **Status:** proposed, and built on a branch. Nothing of it is in `bite_v1.22.0-pre`;
  `bite_v1.22.0-pre-path-a` carries the primitive, the token, the `window.` call, both renderer
  arms, the extractor and the readback. The citations below still resolve against the canonical
  ref, so read this chapter as what to build until the branch lands.
- **Assumes:** [`renderer-seam.md`](renderer-seam.md) — a renderer is installable at all.
- **Companion:** [`inline-commands.md`](inline-commands.md) is the other path; the two
  share one scene primitive.
- **Target crates:** `gpui_engine` (the primitive and the token), `gpui_platform` (the
  trait), `gpui_wgpu` (the extractor, and the arm every Linux window needs),
  `gpui_apple`, `gpui_windows` (their own arms).

## 1. What this is

An application renders into a texture it owns, hands GPUI a token for it, and GPUI
composites that texture into the window in the same pass as the UI quads. No bytes cross
PCIe into system memory, and no second window.

The important finding is that this is **not a new primitive**. `PaintSurface`
(`crates/gpui_engine/src/scene.rs:749`) is already "content produced outside GPUI,
composited into the window":

```rust
#[derive(Clone, Debug)]
pub struct PaintSurface {
    pub order: DrawOrder,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    #[cfg(target_os = "macos")]
    pub image_buffer: core_video::pixel_buffer::CVPixelBuffer,
}
```

It is a variant of the scene's primitive enum (`crates/gpui_engine/src/scene.rs:228`), with
its own batch (`crates/gpui_engine/src/scene.rs:475`) and its own accumulation list
(`crates/gpui_engine/src/scene.rs:50`, pushed at `:132`) — and it is **macOS-only video**.
Its only payload is a `CVPixelBuffer`, behind `#[cfg(target_os = "macos")]`
(`crates/gpui_engine/src/scene.rs:749`); its only producer is
`MacWindowExt::paint_surface` (`crates/gpui_authoring/src/window/mac.rs:21`); it is drawn by
**one** renderer rather than two — DirectX's `draw_surfaces` returns an explicit unsupported
error (`crates/gpui_windows/src/directx_renderer.rs:833`) and wgpu's arm is `{}` under the
comment that surfaces "are macOS-only for video playback and are not implemented by the WGPU
renderer" (`crates/gpui_wgpu/src/wgpu_renderer.rs:1544`) — and that one renders **YCbCr**,
not RGBA: Metal's `surface_fragment` returns `ycbcrToRGBTransform * ycbcr`
(`crates/gpui_apple/src/shaders.metal:884`), and wgpu's `fs_surface` does the same over two
planes, `t_y` and `t_cb_cr` (`crates/gpui_wgpu/src/shaders.wgsl:1350`).

So the proposal **reuses that variant's machinery — its batch, its ordering, its content-mask
handling — and not its drawing.** An imported texture is RGBA, and no fragment path in the
tree samples one: the sprite and path fragments sample the atlas, and `fs_surface` is the only
one that samples a texture the atlas does not own. What has to be written is therefore a
fragment path and a pipeline, in two renderers, which is what
[`renderer-seam.md`](renderer-seam.md) §6's budget means by "a real arm". `PaintSurface` also
supplies two things the drafts omitted — the precedent for a backend payload (`image_buffer`
is a `#[cfg]` field with a `#[cfg]` dependency, not a foreign type in the engine), and the
`order` and `content_mask` fields a primitive needs to sit correctly in the batch list.

## 2. The device constraint

This is the part the drafts got most wrong, and it decides what each platform can do.
Producer and consumer must be the **same device**, not merely the same API:

| producer → consumer | payload | constraint |
| --- | --- | --- |
| wgpu → `WgpuRenderer` (Linux, and Windows with it installed) | `wgpu::TextureView` | the same `wgpu::Device`; wgpu validates and rejects a mismatch |
| wgpu (Metal backend) → GPUI's Metal renderer (macOS) | the raw `id<MTLTexture>` | the same GPU; a raw driver handle bypasses wgpu's bookkeeping |
| wgpu (D3D12) → GPUI's `DirectXRenderer` (D3D11, Windows default) | *none, this milestone* | **out of scope** — a shared handle would bridge it as future work, so the milestone installs `WgpuRenderer` instead |

The third row is the one that moved. It was *impossible*, on the reading that wgpu offers
neither a shareable resource nor a way to adopt one; the second half was wrong, so the row is
reachable rather than closed. GPUI's Windows renderer is Direct3D 11
(`crates/gpui_windows/src/directx_renderer.rs:2092`) while wgpu is Direct3D 12, and a
D3D12 resource is invisible to a D3D11 device unless it was created shareable —
`D3D12_HEAP_FLAG_SHARED`, a *heap* flag set at creation, which wgpu never sets
(`wgpu-hal-29.0.4/src/dx12/device.rs:104`). What wgpu *will* do is adopt a resource the
application allocated: `texture_from_raw` and `create_texture_from_hal` are public on every
desktop backend (`wgpu-hal-29.0.4/src/dx12/device.rs:448`,
`wgpu-29.0.4/src/api/device.rs:325`), and the whole loop is measured to work
([`../decisions/shared-surface.md`](../../decisions/shared-surface.md)). What it costs is the
bridge's synchronisation and a same-adapter requirement, so it is **future work this milestone
unblocks** rather than part of this milestone: the same-device rows are what land.

Three consequences, each of which the drafts got wrong in the other direction:

1. **The Windows arm of `ImportedTextureHandle` is removed.** Its
   `DirectX(*const c_void)` variant rested on a wgpu producer feeding GPUI's D3D11 renderer
   from its own device, which the device rule forbids. The same-device route needs no arm —
   Windows matches Linux, with the `TextureView`. The shared-handle arm the future bridge would
   want is recorded in
   [`../decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md)
   and measured in [`../decisions/shared-surface.md`](../../decisions/shared-surface.md).
2. **The `PaintSurface` macOS field is the precedent, not the model.** A shareable resource —
   an `IOSurface`-backed `MTLTexture`, a DXGI shared NT handle, a dma-buf — must be chosen *at
   creation*, and wgpu offers no descriptor for any of them, so the application must allocate
   and adopt it: on Windows `texture_from_raw` over a shareable `ID3D12Resource`, and on macOS
   a `MTLTexture` built with `objc2-metal`, because wgpu-hal names no `IOSurface`. That is the
   future bridge's shape, not this path's.
3. **Path A is the window owner's capability.** The device belongs to whoever called
   `with_renderer_factory`, so a widget inside someone else's window cannot be a
   producer. That is a boundary, not an accident: it is what makes the Windows answer a
   configuration ("install `WgpuRenderer`") rather than a fork.

The whole of it — the constraint, the Windows configuration, and the future bridge — is
[`../decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md),
with the measurement in [`../decisions/shared-surface.md`](../../decisions/shared-surface.md).

## 3. The scene type, and the API the constraint implies

```rust
// crates/gpui_engine/src/scene.rs — beside Primitive::Surface
pub enum CustomRenderPrimitive {
    /// A texture produced outside GPUI, sampled in the composite pass.
    Texture {
        handle: ImportedTextureHandle,
        bounds: Bounds<ScaledPixels>,
        content_mask: ContentMask<ScaledPixels>,
        radii: Corners<ScaledPixels>,
        opacity: f32,
        flip_v: bool,
    },
    /* Inline, in inline-commands.md */
}
```

**There is no registry and no id, and the device constraint is what decides that.** Every
draft wrote `register_foreign_texture(handle) -> ForeignTextureId` and then a primitive
naming the id. That shape is borrowed from the atlas flow, where it is right because the
*engine owns the texture* and the scene can only name it. Here the engine owns nothing —
the application allocated the texture on the renderer's device and keeps it — so there is
nothing to register it into. The handle rides in the primitive, and one `window.` call
pushes it:

```rust
window.paint_imported_texture(handle, bounds, radii, opacity, flip_v);
```

Three consequences, each a removal:

- **No id type.** `AtlasTextureId` (`crates/gpui_engine/src/atlas.rs:254`) indexes textures
  the renderer owns; a second id type existed only to key the registry. (A draft also wrote
  `TextureId`, which is not a type in this tree.)
- **No per-window registry, and no "the registry rejects an id it did not hand out" test.**
  A handle cannot be wrong — it is the resource.
- **No name to cache across frames.** The application keeps the texture in its own field
  and hands it over each frame. What it must not do is hand over one whose *device* is not
  the renderer's, which fails where the bind group is created — and fails there by panicking
  inside wgpu's own storage rather than by reporting a mismatch this side can name, which is
  what [`verification.md`](verification.md) §1's device row actually measures.

The surface an application author meets does not change: `GpuCanvas` is the user API
([`gpu-canvas.md`](../authoring/gpu-canvas.md)) and the call above is internal to it. What changes is
what a third-party element or renderer author touches, and those names are worth fixing
once, here:

| the drafts | this design | why |
| --- | --- | --- |
| `ForeignTextureHandle` | `ImportedTextureHandle` | "foreign" asserts the one thing the device rule forbids; what is imported is the producer's work, not a foreign device |
| `register_foreign_texture(handle) -> ForeignTextureId` | `paint_imported_texture(handle, …)`, no id | above |
| `ForeignTextureExt::to_foreign_handle` | `ImportedTextureExt::to_imported_handle` | the same adjective; it stays an extension trait in `gpui_wgpu`, because that is the only place a HAL API is named |
| `CustomRenderPrimitive::Texture` | unchanged | it is a texture; the payload is what is imported |

**Bounds are `ScaledPixels`**, like `PaintSurface`'s
(`crates/gpui_engine/src/scene.rs:220`), not `Pixels`; the element converts at paint time. **The
radii are `ScaledPixels` too**, and a `Corners` rather than a `CornerRadii` struct: a renderer is
handed scaled pixels and has no scale factor of its own, so the element converts the radii the way
it converts the bounds, as `paint_quad` does, and typing them `ScaledPixels` is what makes the
unconverted form fail to compile instead of drawing a corner too tight on a HiDPI display.

The handle is erased, and this is the point where erasure is right rather than a
compromise:

```rust
// crates/gpui_engine/src/custom_render.rs
pub struct ImportedTextureHandle {
    pub payload: Arc<dyn Any + Send + Sync>,
}
```

A handle has exactly one counterparty — the application that produced it and
the renderer the application chose — so the two agree on a concrete type and the engine
only transports the token. That is the opposite of the *target* in
[`renderer-seam.md`](renderer-seam.md) §5.3, which any renderer must read and which is
therefore typed. Erasing the handle is also what keeps `wgpu` out of `gpui_engine`, whose
dependencies are `gpui_types` and small support crates
([`../architecture/layer-stack.md`](../../architecture/layer-stack.md)); the alternative,
mirroring `PaintSurface` with a cfg-gated `wgpu`, is recorded as the rejected option in
[`README.md`](README.md).

The payload is a **typed newtype per backend**, not a `usize`:

```rust
#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
pub struct MetalTexture(pub *mut std::ffi::c_void);
// `id<MTLTexture>` is not `Send`, but the handle crosses no thread; the assertion is
// what `Arc<dyn Any + Send + Sync>` requires.
unsafe impl Send for MetalTexture {}
unsafe impl Sync for MetalTexture {}
```

A bare `usize` would collide with any other `usize` payload and could not be checked.

## 4. The colour-space invariant

Three rules, and getting any of them wrong is a visible defect rather than a crash:

- **The offscreen texture is declared sRGB** — `Rgba8UnormSrgb` or `Bgra8UnormSrgb`. The
  declaration is what fixes the *content*, not only the sampling: an sRGB view's stored bytes are
  sRGB-encoded by construction, so a producer that renders into one and a producer that writes
  bytes a decoder handed it agree about what they handed over.
- **The fragment decodes the sample and re-encodes it.** Sampling an sRGB view yields *linear*
  values, and GPUI's shaders do not work in linear: its target is a `*_UNORM` format they write
  sRGB-encoded values into, which is why the atlas is non-sRGB as well
  (`crates/gpui_wgpu/src/wgpu_atlas.rs:367-368`). So the fragment has to put the sample back
  through `linear_to_srgb` before blending. On bytes that are sRGB-encoded the round trip is the
  identity, and dropping either half is the ≈2.2 error the first rule alone does not describe —
  a fixture reads back `[147, 32, 8, 255]` instead of `[200, 100, 50, 255]` when the re-encode is
  missing ([`verification.md`](verification.md) §1).
- **The sampler is linear with `AddressMode::ClampToEdge`**, so a clip against a rounded
  corner cannot bleed the opposite edge into the blend.

`to_imported_handle` should validate the format and the usage flags and fail early: the format is a
property of the view, and the usage flags must include `TextureUsages::TEXTURE_BINDING`.

**Not a Windows special case, after all.** The surface format is the renderer's choice, and the
wgpu renderer picks a non-sRGB one on every platform — `preferred_formats = [Bgra8Unorm,
Rgba8Unorm]` (`crates/gpui_wgpu/src/wgpu_renderer.rs:359`) — so a Windows window running
`WgpuRenderer` shows the same `*_UNORM` surface convention the rule above states, and
`DirectXRenderer`'s `B8G8R8A8_UNORM` target
(`crates/gpui_windows/src/directx_renderer.rs:32`) belongs to the renderer that supports
neither path today. The presentation probe measured the DX12 surface offering `Bgra8UnormSrgb` and
`Bgra8Unorm` alike, so the choice is not one the platform makes
([`../decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md)).
The fixture in [`verification.md`](verification.md) §1 is what verifies the rules themselves: it
reads the format from the consumer, so it needs the fragment path to exist first, and on
`bite_v1.22.0-pre-path-a` it does.

## 5. Extracting the handle

The extraction lives in `gpui_wgpu`, where `wgpu` is already a dependency. It is the only
place in the tree that names a HAL API:

```rust
// crates/gpui_wgpu/src/imported_texture.rs
pub trait ImportedTextureExt {
    fn to_imported_handle(&self) -> anyhow::Result<ImportedTextureHandle>;
}
```

`as_hal` in wgpu 29 is `unsafe`, takes **one** generic parameter, and returns an
`Option` — `unsafe fn as_hal<A: hal::Api>(&self) -> Option<impl Deref<Target =
A::TextureView>>` (`wgpu-29.0.4/src/api/texture_view.rs:73`). Every draft wrote the
pre-29 closure form, which does not compile. On macOS the returned view's `raw_handle()`
is the `id<MTLTexture>`; on Linux there is nothing to extract at all — wgpu *is* the
renderer, so the payload is the cloned `TextureView`.

**Do not confuse `wgpu` the device with the one the renderer uses.** The drafts'
reference implementation created a second device (`wgpu::Instance::default()`,
`request_adapter`, `request_device`) and handed over a view from it. That fails wgpu's
device check. The device to render on is the one the window's renderer was built with,
which the factory has: `GpuContext = Rc<RefCell<Option<WgpuContext>>>`
(`crates/gpui_wgpu/src/wgpu_renderer.rs:168`), whose `WgpuContext` exposes
`pub device: Arc<wgpu::Device>` and `pub queue: Arc<wgpu::Queue>`
(`crates/gpui_wgpu/src/wgpu_context.rs:9`). Two notes for whoever writes the guide: the
slot is `None` until the first renderer initialises it, and `Rc<RefCell<…>>` is `!Send`,
so this is a same-thread affordance and not something a worker thread can use.

## 6. The queue, and why there is no fence

One device has one `queue` (`crates/gpui_wgpu/src/wgpu_context.rs:9`), and that is what
orders the producer against the frame. The application submits the pass that fills its
texture; the renderer's frame is submitted after it; so the GPU executes them in that
order and the texture is complete by the time the composite samples it. No semaphore, no
`MTLSharedEvent`, no keyed mutex — the synchronisation the cross-device drafts specified
is the cost of a second device, and there is no second device
([`../decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md)).
The cross-device case that 0002 defers as future work would need exactly that handshake, and
the shared-handle arm in §2 is where it would be carried; what it costs is measured in
[`../decisions/shared-surface.md`](../../decisions/shared-surface.md).

What that asks of the application is a *when*, not a *what*: **the submission has to
happen before the frame's.** The natural place is the paint callback, which runs while the
scene is built and therefore before `draw`; rendering a frame ahead works too. Submitting
from another thread after the frame does not — the device is thread-safe, so nothing
prevents it, it is just wrong.

Path B is the stronger form of the same idea and is *defined* by it: there the commands go
into GPUI's own encoder, in the same pass, so the ordering is the command's position
rather than the submission's ([`inline-commands.md`](inline-commands.md) §2). That is the
one place the two paths differ in kind — Path A shares the device and the queue, Path B
shares the encoder itself.

## 7. What each renderer has to do

| renderer | work |
| --- | --- |
| `WgpuRenderer` | **write the arm.** `PrimitiveBatch::Surfaces` is `{}` today (`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`); it has to bind the imported view to a dedicated fragment sampler slot and draw the quad with the SDF clip, radii and opacity |
| `MetalRenderer` | already draws `PaintSurface` (`crates/gpui_apple/src/metal_renderer.rs:1137`); downcast the payload to `MetalTexture` and bind it instead of the `CVPixelBuffer` path |
| `DirectXRenderer` | **nothing, under this design.** It already returns an explicit unsupported error rather than succeeding silently (`crates/gpui_windows/src/directx_renderer.rs:833`), and Windows runs Path A under `WgpuRenderer` (§2), so this renderer is not asked to sample an imported texture |

The bind happens in the *same* pass that composites the quad batch — the generalisation
of `draw_surfaces` — so there is no second pass and no intermediate target.

## 8. Open

- **The erasure vs a cfg-gated `wgpu` in the engine** (§3). Recommendation: erasure, and
  it is the same argument the target's typing rests on, applied from the other side.
- **Which sampler slot, and whether the payload should carry it.** The drafts say
  "dedicated slot"; pinning the number is a per-renderer decision that belongs with the
  arm, not in the engine.
- **`flip_v`.** Kept on the primitive because the element never sees a texture, so it can
  only pass the flag down. Whether the *application* or the renderer owns the convention
  is unsettled — the drafts hand it to the application, which means every producer has to
  know GPUI's UV direction.
- **Whether the raw Metal handle needs an `MTLSharedEvent`.** Only if a producer ever
  renders on a second device; on one device, order within the frame is the
  synchronisation. The drafts proposed the event because they had already assumed a
  second device.
