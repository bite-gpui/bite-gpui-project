# The renderer-author surface: targets, surfaces, HAL and the state guard

- **Status:** proposed. The renderer-author companion to
  [`dual-path-implementation-spec.md`](dual-path-implementation-spec.md), for the DX an
  *extension* author meets — as opposed to the application author
  [`gpu-canvas-dx.md`](gpu-canvas-dx.md) covers.
- **Inherits:** corrections 1–20 from the architecture and implementation documents.
- **Target crates:** `gpui_wgpu`, `gpui_platform`, `gpui_authoring`.

## 1. The structural finding: the target cannot be the adaptor boundary

§2 is the part to read first, because its premise does not hold. It implements
`WindowTargetExt for RendererTarget`, in `gpui_wgpu`, by *downcasting* to
`gpui_linux::LinuxRendererTarget` / `MacRendererTarget` / `WindowsRendererTarget`. That
requires `gpui_wgpu` to depend on `gpui_linux`, `gpui_macos` and `gpui_windows` — but
the direction is the other way: `gpui_linux` depends on `gpui_wgpu`
(`gpui_wgpu = { workspace = true, optional = true, features = ["font-kit"] }`) as does
`gpui_web`. `gpui_wgpu → gpui_linux → gpui_wgpu` is a cycle Cargo rejects.

The cycle is the symptom. The cause is that **`dyn Any` requires `'static` and the
target is borrowed**: `LinuxRendererTarget` holds `&RawWindow`, an owned-but-borrowed
handle, so it is not `'static` and cannot be coerced to `&dyn Any` at all
(correction 11). And even without lifetimes, a renderer written by a third party cannot
name `LinuxRendererTarget` without depending on `gpui_linux` — which is exactly the
platform-specific coupling §1 says the abstraction exists to remove.

**The payload and the target are not the same kind of erased value**, which is why the
"double erasure" tenet works for one and not the other:

- The **texture handle** has exactly one counterparty: the application that produced it
  and the renderer *that application chose*. Both can name the concrete type; the
  engine only transports a token. Erasure is right.
- The **target** is consumed by *any* renderer the application installs — including one
  whose author has never seen the backend crates. Its cross-platform facts must be
  *typed*, not erased.

The fix is the one [`scene-renderer-seam.md`](scene-renderer-seam.md) §2 already had,
and it is cheap: **`gpui_platform` already depends on `raw-window-handle`**
(`raw-window-handle.workspace = true` in `crates/gpui_platform/Cargo.toml`, workspace
version `0.6`). No new dependency is needed, and `wgpu` still does not enter the
platform layer. (This also corrects that document's §2, which said the dependency would
have to be added — at `bite_v1.22.0-pre` it is already there.)

## 2. What the target should carry

```rust
// crates/gpui_platform/src/platform_renderer.rs
pub struct RendererTarget<'a> {
    /// The cross-platform facts every renderer needs, typed.
    pub window_handle: Option<RawWindowHandle>,
    pub display_handle: Option<RawDisplayHandle>,
    pub size: Size<Pixels>,
    pub scale_factor: f32,
    pub transparent: bool,
    /// Backend-specific extras, for the backend's own default renderer only.
    pub backend: &'a dyn Any,
}
```

Three consequences:

- **`WindowTargetExt` collapses to nothing worth a trait.** Creating a surface is
  `instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle { raw_window_handle,
  raw_display_handle })` from the typed fields — no downcast, no backend dependency, and
  therefore a method on the caller's side or a free function in `gpui_wgpu`, which may
  stay in `gpui_wgpu` only because it no longer names a backend.
- **The scale factor must come from the platform.** §2.2 hardcodes `1.0` on Linux
  ("handled via Wayland viewport") and `2.0` on macOS; both are wrong on mixed-DPI
  setups, and the second is a comment admitting it did not query the backing scale
  factor. Scale belongs beside `size`, supplied by the backend.
- **Present mode is the renderer's choice**, so it must not be a
  `wgpu::PresentMode` field on a `gpui_platform` type. Leave it out.

The backend's extra `&'a dyn Any` still cannot be *borrowed* data (correction 11): a
backend that needs to hand its default renderer something more than the typed fields
must own it or put it behind an `Rc`.

## 3. Path A: the extractor

The extractor belongs in `gpui_wgpu` (adopted). The mechanics need three fixes:

- **`as_hal` is not the closure form.** In wgpu 29 it is
  `pub unsafe fn as_hal<A: hal::Api>(&self) -> Option<impl Deref<Target = A::TextureView>>`
  (`wgpu-29.0.4/src/api/texture_view.rs:73`) — one generic parameter, `unsafe`, and an
  `Option` return. §3.2's `as_hal::<api::Metal, _, Result<usize>>(|hal_view| …)` is the
  pre-29 shape (correction 12).
- **The guards §3.1 names are implementable, but not from the view.** A `TextureView`
  exposes only `texture()`, `as_hal()` and `as_custom()`
  (`wgpu-29.0.4/src/api/texture_view.rs:30`, `:73`, `:80`). The format and usage checks
  go through the texture: `view.texture().format()`
  (`wgpu-29.0.4/src/api/texture.rs:169`) and `view.texture().usage()` (`:176`). Say so,
  because a reader will otherwise look for `TextureView::format`.
- **The payload is a typed newtype, not a `usize`** (correction 16) — a bare `usize`
  collides with any other `usize` payload and cannot be checked.

**Windows is still the open problem** (correction 13): `hal::api::Dx11` does not exist —
wgpu-hal 29's backends are `dx12`, `gles`, `metal`, `vulkan`
(`wgpu-hal-29.0.4/src/lib.rs:252`) — and GPUI's Windows renderer is Direct3D 11
(`crates/gpui_windows/src/directx_renderer.rs:2086`). A wgpu (D3D12) view cannot be
handed to a D3D11 device without shared-handle interop. This document does not resolve
it; it repeats it because §3.2 does.

## 4. Path B: coordinates, and why the context cannot be erased

The coordinate work is right in spirit and should be kept: GPUI is top-left logical
pixels, the hardware is bottom-left physical, and the bridge is
`bounds * scale_factor` clamped to integers, plus a projection matrix so the app's
shader does not repeat it.

Two corrections:

- **Pin the matrix convention.** §4.1 writes `[[f32; 4]; 4]` with the translation in the
  fourth row — a row-vector, row-major convention. WGSL and wgpu are column-major, so
  the same numbers uploaded as a `mat4x4<f32>` transpose. State which convention is
  meant, and make the Y flip agree with the scissor rectangle's origin, or the viewport
  renders mirrored on one axis.
- **The context cannot be `&mut dyn Any`.** §4.2's
  `GpuRenderContext { raw_encoder: &'a mut dyn Any }` cannot hold a live
  `wgpu::RenderPass<'a>`: `Any: 'static` (correction 11, same root cause as §1). Nor
  can the engine store a `Box<dyn Fn(&mut wgpu::RenderPass)>`, because naming the type
  would put `wgpu` in `gpui_engine`.

  The resolution is the one that makes both paths the same shape: **the callback is
  registered with the renderer, and the scene carries an opaque token.** The
  application already chose the renderer, so it can name the pass type it wants:

  ```rust
  // Path A, today's shape — the payload is owned, so erasure works
  let id = window.register_foreign_texture(handle);
  window.paint_custom_primitive(CustomRenderPrimitive::Texture { id, bounds, … });

  // Path B, the same shape — the closure lives in the renderer, not the scene
  let token = app_side_renderer.register_inline(move |pass: &mut wgpu::RenderPass| { … });
  window.paint_custom_primitive(CustomRenderPrimitive::Inline { token, bounds, content_mask });
  ```

  This gives Path B an owner for its state guard too — the snapshot/restore and the
  batch-cursor rewind belong to whoever holds the pass, which is the renderer, not a
  context object the engine erases.

## 5. The reference implementation

§5 is the right thing to aim for — an app embedding a map without platform branches —
but as written it does not compile, and two of the failures are wgpu-29 API drift:

- **`request_adapter` and `request_device` return `Result`, not `Option`, and
  `request_device` takes one argument.** `Instance::request_adapter` returns
  `impl Future<Output = Result<Adapter, RequestAdapterError>>`
  (`wgpu-29.0.4/src/api/instance.rs:167`) and `Adapter::request_device(&self, desc:
  &DeviceDescriptor<'_>) -> … Result<(Device, Queue), RequestDeviceError>`
  (`wgpu-29.0.4/src/api/adapter.rs:58`). §5's `.ok_or_else(…)` and
  `request_device(&desc, None)` are both pre-29.
- **`Render::render` is still `&mut ViewContext<Self>`.** It is
  `fn render(&mut self, window: &mut Window, cx: &mut Context<Self>)`
  (`crates/gpui_authoring/src/element.rs:164`); `ViewContext` does not exist here.
- **`on_render_texture` still has no `bounds`**, so the offscreen texture cannot be
  sized to the element. This is correction 20 recurring; §5 uses it in the very example
  it is meant to demonstrate.

## 6. Verification

The draft's table is a good checklist. Add the two rows the corrections imply:

| scenario | to prevent | assertion |
| --- | --- | --- |
| handle resolution | a surface that does not match the window | the returned surface config equals the platform's physical size (typed target, §2) |
| colour guard | washed-out composite | a non-sRGB view is rejected by `to_foreign_handle` via `view.texture().format()` (§3) |
| scissor integrity | drawing outside the box | injected commands cannot draw outside `device_scissor_rect()` |
| pipeline sanitisation | leaked pipeline state | after an inline command, the quad pipeline and instance buffer are restored |
| dependency direction | the renderer crate reaching a backend | `cargo metadata` shows no `gpui_wgpu → gpui_linux` edge (§1) |
| DPI | a constant scale factor | on a simulated 2× display the scissor and viewport match the physical size |

## Open

- **§1 changes the architecture document.** The "double erasure" tenet holds for the
  payload and not for the target; the target becomes typed. That decision should be
  made in [`dual-path-ioc-architecture.md`](dual-path-ioc-architecture.md) rather than
  here, since three documents depend on it.
- **Windows Path A** (correction 13) — still unanswered, still the only item that can
  invalidate a platform.
- **Path B's registry** — where the renderer-owned token table lives, and how the
  application reaches its own renderer to register. This is the one piece of Path B
  that the seam document does not yet specify.
- **Native hooks** — still
  [`scene-renderer-seam.md`](scene-renderer-seam.md) §3.
