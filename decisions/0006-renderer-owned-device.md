# 0006 — The producer reaches the device through the renderer, not the window

- **Decided:** 2026-10-07
- **Status:** decided — supersedes [`0004`](0004-producer-device-rendezvous.md), whose window-lent
  erased `device_any`, the `GpuWindow` decorator and the facade's `DirectXWindowExt` are deleted.
- **Evidence:** [`../spi/rendering/producer-reach.md`](../spi/rendering/producer-reach.md) — how a
  producer gets the device it must make its texture on, per platform; and
  [`../spi/authoring/gpu-canvas.md`](../spi/authoring/gpu-canvas.md) — the canvas door.
- **Touches:** [`0002-render-extension-device-model.md`](0002-render-extension-device-model.md) — the
  device rule this still implements; [`0004`](0004-producer-device-rendezvous.md) — the record this
  replaces, whose typed spelling and erased accessor it inverts;
  [`0005-external-rendering-unifies-under-surface.md`](0005-external-rendering-unifies-under-surface.md)
  — which retargeted the surface but left the accessor's spelling open.

## Decision

**The renderer owns its device, and a typed, downcast-based door on the canvas lends it. The window
has no GPU accessor.** A trait beside the seam carries the device:

```rust
pub trait GpuRenderer: SceneRenderer {
    type Device;
    fn device(&self) -> Option<Self::Device>;
}
```

Each concrete renderer that has a device implements it with its own type, and a renderer with nothing
to lend does **not** implement it — absence, not a fake device:

| renderer | `type Device` |
| --- | --- |
| `DirectXRenderer` | `ID3D11Device` |
| `MetalRenderer`, `MetalHeadlessRenderer` | `metal::Device` |
| `WgpuRenderer`, `WgpuHeadlessRenderer` | `(Arc<wgpu::Device>, Arc<wgpu::Queue>)` |
| `TestRenderer` (authoring), `HeadlessRenderer` (Linux discard) | — no impl |

The producer's door is on the canvas, in `GpuCanvasContext` (`crates/gpui_authoring/src/elements/gpu_canvas.rs`):

- `device::<R: GpuRenderer>() -> R::Device` — panics only when the window's renderer is not `R`;
  naming `R` *is* the assertion.
- `try_device::<R: GpuRenderer>() -> Option<R::Device>` — the fallible form, for a producer that
  degrades.

Both borrow the renderer scoped (`PlatformWindow::with_renderer`) and clone the device out, so the
returned handle is **owned** and may outlive the call.

Deleted with the old model: the `GpuWindow` decorator trait, `PlatformWindow::gpu_window`,
`PlatformRenderer::device_any`, `Window::device_any`, `GpuCanvasContext::device_any`, and the facade's
`DirectXWindowExt`. `DirectXRenderer` is now `pub` in `gpui_windows` and re-exported by the facade as
`gpui::DirectXRenderer`, because a producer names it to ask for its device.

Three consequences the model forces:

1. **The device is reachable only at paint time**, inside a `gpu_canvas` callback. The renderer and its
   device do not exist before then, so a bare `&mut Window` or an `App` cannot reach one.
2. **The handle is epoch-scoped.** A device-lost recovery replaces the renderer's device; a handle or
   texture a producer held across that boundary is stale and must be rebuilt on the new one.
3. **A same-device texture is the exception on Linux, not the rule.** The portable currency there is
   the dma-buf (`SurfaceSource::DmaBuf`), which needs no window device at all; `device::<R>()` is only
   for a same-device texture. On Windows, a same-device payload builds *inside* `gpu_canvas`, because a
   `surface()` element cannot take a same-device source before paint.

**The DirectX payload gained a third, device-independent arm.** `DirectXSource` is now
`Texture(ID3D11Texture2D) | View(ID3D11ShaderResourceView) | Shared(SharedDirectXSurface)`. The first
two are device-bound, which is exactly why they cannot be built before paint; `Shared` is the
renderer's counterpart to CoreVideo and dma-buf — a token the renderer resolves. It carries
`texture: HANDLE` (from `CreateSharedHandle`), an optional `fence: SharedDirectXFence { handle, value }`,
and the `width`/`height` the element needs before the renderer has opened it. `DirectXRenderer::surface_view`
owns the consumer half — `OpenSharedResource1` + `CreateShaderResourceView`, and
`ID3D11Device5::OpenSharedFence` + `ID3D11DeviceContext4::Wait` — cached per handle under the same
`SURFACE_VIEW_CACHE_PATIENCE` / device-loss rule as the existing `surface_views` cache (cleared in
`handle_device_lost_impl`, pruned in `prune_surface_views`), so the open, the view and the wait
happen once per handle, at draw, rather than per frame in the application.

`gpui_interop`, the downstream crate, follows — twice. Its door is now
`attach<D: 'static>(device: Option<D>) -> Result<Interop, Unavailable>`, taking the device the canvas
lends rather than a `&gpui::Window` it no longer has any way to get a device from. And with the
renderer owning the consumer half of `Shared`, the crate is **producer-only** on the Direct3D path:
`SharedSurface::open`, `OpenedSurface`, `Fence::open` and `Fence::wait_gpu` are deleted; it keeps
`SharedSurface::new`/`handle`/`resource`/`width`/`height`/`format` and
`Fence::new`/`handle`/`value`/`signal`, and hands the renderer
`DirectXSource::Shared { texture: surface.handle(), fence: Some(SharedDirectXFence { handle: fence.handle(), value }), .. }`.

## Why

**The erased accessor leaked renderer internals through the window.** `0004` hung `device_any` — an
`Option<Rc<dyn Any>>` — on `PlatformWindow` and `Window`, because the shared trait may not name a
device. But a window is not a GPU object; what a producer actually wants is the concrete device of the
concrete renderer, and it can say so. `0004`'s own rejected-alternatives table already refused the
other shapes for the same reason: the typed part of the API should be a signature, not a convention.

**A typed downcast is the honest route.** `PlatformWindow::with_renderer` already hands out
`&mut dyn SceneRenderer`; the only missing piece is naming the concrete renderer. `GpuRenderer` makes
that a supertrait-beside trait with an associated `Device`, and `device::<R>()` is a downcast whose
type argument is the whole of the assertion. There is no facade trait to spell and no orphan-rule wall
— no backend crate needs to name `gpui::Window` — and the compiler checks the pairing.

**The `Option` folds two cases, so the canvas splits them.** `GpuRenderer::device` answering `Option`
conflates *the window's renderer is not the one you named* with *the renderer is the one you named and
has no device to lend right now*. `device::<R>()` treats the first as a programming error — the
assertion — while `try_device::<R>()` answers `None` for either, for a producer that degrades.

**Owned, because the reach is a closure.** The renderer is reached only through `with_renderer`'s
closure, and no borrow of its device outlives one; cloning the device out is the only shape that
returns it. This is the same conclusion `0004` reached for `Rc<dyn Any>`, kept.

## Rejected alternatives

| alternative | why not |
| --- | --- |
| the window-lent, erased `device_any` (`0004`'s shape) | it puts a device query in the window's contract and hands back `dyn Any` the caller must downcast in userland; the renderer that owns the device is the object to name |
| a typed accessor trait in the facade (`DirectXWindowExt::d3d11_device()`) | it names one backend per trait and needs the facade to re-export each; one generic `device::<R>()` names them all, with no facade spelling |
| widening `SceneRenderer` with the accessor | still refused — the seam must not name a device; `GpuRenderer` is a separate supertrait a renderer opts into |
| a fake device on the renderers that have none | it makes "no device" a value to test rather than an absence the type system states |

## What would reopen this

- **A producer that needs the context or the queue as well, under one signature.** The Direct3D answer
  is a device and the immediate context comes from it, and wgpu's answer is a pair; if a third backend
  could lend neither, the associated `Device` type is the thing to revisit, not the anchor.
- **A renderer that cannot lend, and a producer that needs one anyway.** The shared-handle arm is that
  answer on Windows (`DirectXSource::Shared`, opened by the renderer); elsewhere the cross-device
  bridge (`0002`'s deferred tier, now `0005`'s downstream crate) is, and it is a contract change
  rather than a change to this door.
- **The per-backend payload types becoming a published surface** — the commitment `0004` named, carried
  forward: an application that names `ID3D11Device` or `metal::Device` as `R::Device` pins the crate
  version the backend publishes.
