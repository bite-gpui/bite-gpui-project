# Path A: importing a texture produced outside GPUI

- **Status:** proposed. Nothing of it is implemented.
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

It is a variant of the scene's primitive enum (`crates/gpui_engine/src/scene.rs:228`),
with its own batch (`crates/gpui_engine/src/scene.rs:475`), its own accumulation list
(`crates/gpui_engine/src/scene.rs:50`, pushed at `:132`), and **two of three renderers
already draw it**: Metal (`crates/gpui_apple/src/metal_renderer.rs:1133`) and DirectX
(`crates/gpui_windows/src/directx_renderer.rs:830`). In wgpu it is a no-op —
`PrimitiveBatch::Surfaces(_surfaces) => {}`
(`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`).

So the proposal is to complete that variant rather than invent one: the same batch, the
same three arms, one of which has to be written. `PaintSurface` also supplies two things
the drafts omitted — the precedent for a backend payload (`image_buffer` is a `#[cfg]`
field with a `#[cfg]` dependency, not a foreign type in the engine), and the `order` and
`content_mask` fields a primitive needs to sit correctly in the batch list.

## 2. The device constraint

This is the part the drafts got most wrong, and it decides what each platform can do.
Producer and consumer must be the **same device**, not merely the same API:

| producer → consumer | payload | constraint |
| --- | --- | --- |
| wgpu → `WgpuRenderer` (Linux, and Windows with it installed) | `wgpu::TextureView` | the same `wgpu::Device`; wgpu validates and rejects a mismatch |
| wgpu (Metal backend) → GPUI's Metal renderer (macOS) | the raw `id<MTLTexture>` | the same GPU; a raw driver handle bypasses wgpu's bookkeeping |
| wgpu (D3D12) → GPUI's `DirectXRenderer` (D3D11, Windows default) | *none* | **impossible** |

The third row is why **Path A and Path B on Windows require installing
`gpui_wgpu::WgpuRenderer`** through the factory: GPUI's Windows renderer is Direct3D 11
(`crates/gpui_windows/src/directx_renderer.rs:2086`) while wgpu is Direct3D 12, and a
D3D12 resource is invisible to a D3D11 device unless it was created shareable —
`D3D12_HEAP_FLAG_SHARED`, a *heap* flag set at creation. wgpu never sets it
(wgpu-hal's DX12 backend passes `D3D12_HEAP_FLAG_NONE`), and wgpu 29 exposes no
external-memory API on any backend, so there is no way to ask. The full probe record is
[`spike-windows-path-a.md`](spike-windows-path-a.md).

Three consequences, each of which the drafts got wrong in the other direction:

1. **The Windows arm of `ForeignTextureHandle` is removed.** Its
   `DirectX(*const c_void)` variant rested on the premise that a wgpu producer can feed
   GPUI's D3D11 renderer. It cannot.
2. **The `PaintSurface` macOS field is the precedent, not the model.** A shareable
   resource — an `IOSurface`-backed `MTLTexture`, a DXGI shared NT handle, a dma-buf —
   must be chosen *at creation*, and wgpu offers no descriptor for any of them. A
   "Tier 2" OS-handle bridge only ever works for a producer that owns the native API and
   is not wgpu, which is not what this path is for. It is out of scope, not pending.
3. **Path A is the window owner's capability.** The device belongs to whoever called
   `with_renderer_factory`, so a widget inside someone else's window cannot be a
   producer. That is a boundary, not an accident: it is what makes the Windows answer a
   configuration ("install `WgpuRenderer`") rather than a fork.

The whole of it — the constraint, the Windows configuration, and why the OS-handle
bridges are out of scope rather than pending — is
[`../decisions/0002-render-extension-device-model.md`](../decisions/0002-render-extension-device-model.md).

## 3. The scene types

```rust
// crates/gpui_engine/src/scene.rs — beside Primitive::Surface
pub enum CustomRenderPrimitive {
    /// A texture produced outside GPUI, sampled in the composite pass.
    Texture {
        id: ForeignTextureId,
        handle: ForeignTextureHandle,
        bounds: Bounds<ScaledPixels>,
        content_mask: ContentMask<ScaledPixels>,
        radii: Corners<Pixels>,
        opacity: f32,
        flip_v: bool,
    },
    /* Inline, in inline-commands.md */
}
```

Four things about that shape, each of which a draft got wrong:

- **`ForeignTextureId`, not `AtlasTextureId` and not `TextureId`.** The atlas ids
  (`crates/gpui_engine/src/atlas.rs:254`) index textures the renderer owns; a foreign
  texture is not one of them. `TextureId` is not a type in this tree.
- **The registry is per window, per frame.** A `paint` callback inserts into the frame's
  scene through `Window`, the way `paint_quad`
  (`crates/gpui_authoring/src/window.rs:4972`) does, so the registry that ids it is the
  same kind of state and lives beside it — `window.register_foreign_texture`, not
  `cx.register_foreign_texture`.
- **Bounds are `ScaledPixels`**, like `PaintSurface`'s
  (`crates/gpui_engine/src/scene.rs:220`), not `Pixels`; the element converts at paint
  time.
- **`radii` is `Corners<Pixels>`**, not a `CornerRadii` struct.

The handle is erased, and this is the point where erasure is right rather than a
compromise:

```rust
// crates/gpui_engine/src/custom_render.rs
pub struct ForeignTextureHandle {
    pub payload: Arc<dyn Any + Send + Sync>,
}
```

A texture handle has exactly one counterparty — the application that registered it and
the renderer the application chose — so the two agree on a concrete type and the engine
only transports the token. That is the opposite of the *target* in
[`renderer-seam.md`](renderer-seam.md) §5.3, which any renderer must read and which is
therefore typed. Erasing the handle is also what keeps `wgpu` out of `gpui_engine`, whose
dependencies are `gpui_types` and small support crates
([`../architecture/layer-stack.md`](../architecture/layer-stack.md)); the alternative,
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

Two rules, and getting either wrong is a visible defect rather than a crash:

- **The offscreen texture must be sampled as sRGB** — `Rgba8UnormSrgb` or
  `Bgra8UnormSrgb`. GPUI's swapchain is a `*_UNORM` format treated as sRGB at
  presentation, so a raw linear texture composites with a ≈2.2 gamma error.
- **The sampler is linear with `AddressMode::ClampToEdge`**, so a clip against a rounded
  corner cannot bleed the opposite edge into the blend.

`to_foreign_handle` should validate both and fail early: the format is a property of the
view, and the usage flags must include `TextureUsages::TEXTURE_BINDING`.

## 5. Extracting the handle

The extraction lives in `gpui_wgpu`, where `wgpu` is already a dependency. It is the only
place in the tree that names a HAL API:

```rust
// crates/gpui_wgpu/src/foreign_texture.rs
pub trait ForeignTextureExt {
    fn to_foreign_handle(&self) -> anyhow::Result<ForeignTextureHandle>;
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

## 6. What each renderer has to do

| renderer | work |
| --- | --- |
| `WgpuRenderer` | **write the arm.** `PrimitiveBatch::Surfaces` is `{}` today (`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`); it has to bind the imported view to a dedicated fragment sampler slot and draw the quad with the SDF clip, radii and opacity |
| `MetalRenderer` | already draws `PaintSurface` (`crates/gpui_apple/src/metal_renderer.rs:1133`); downcast the payload to `MetalTexture` and bind it instead of the `CVPixelBuffer` path |
| `DirectXRenderer` | **stop succeeding silently.** `draw_surfaces` returns `Ok(())` without drawing when the list is non-empty (`crates/gpui_windows/src/directx_renderer.rs:830`); with this design it must return an explicit unsupported error, so an application that registered a texture on the default Windows renderer learns why rather than seeing nothing |

The bind happens in the *same* pass that composites the quad batch — the generalisation
of `draw_surfaces` — so there is no second pass and no intermediate target.

## 7. Open

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
