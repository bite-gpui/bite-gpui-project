# The render extension, in detail — state machines, patches, verification

- **Status:** proposed. **v2.0.0** of the implementation companion to
  [`dual-path-ioc-architecture.md`](dual-path-ioc-architecture.md). It supersedes the
  v1.0.0 draft recorded earlier in this file's history.
- **Inherits:** the architecture document's corrections 1–10 and
  [`gpu-canvas-dx.md`](gpu-canvas-dx.md)'s; they are not restated. Where code below
  disagrees, those win.
- **Target crates:** `gpui_engine`, `gpui_platform`, `gpui_authoring`, `gpui_linux`,
  `gpui_macos`, `gpui_windows`, `gpui_wgpu`.

## What v2.0.0 adopted, and what it still gets wrong

Adopted, and correctly so: the facade identity (no separate `bite-gpui` crate), the
**double erasure boundary** (neither `gpui_engine` nor `gpui_platform` names `wgpu`, a
window toolkit or Cocoa), the `renderer_factory` name mirroring
`headless_renderer_factory`, dropping the `as_scene_renderer` shims, HAL code living in
`gpui_wgpu`, `order` and `content_mask` on the primitive, recovery on all four
platforms, and no longer naming `gpui_animotion`. Those are the right calls.

Still wrong, and each is a compile error or an unmade decision rather than a style
point. Corrections 11–20, continuing the architecture document's numbering:

11. **The erased target cannot hold borrows.** `RendererTarget<'a> { raw: &'a dyn Any }`
    requires the referent to be `'static`, because `Any: 'static`. The draft's
    `LinuxRendererTarget<'a> { raw_window: &'a RawWindow, .. }` is therefore not
    `'static` and cannot be coerced to `&dyn Any`. The target must **own** its handles.
    That is easy and already required: `RawWindow` is `Copy` and `'static`
    (`crates/gpui_linux/src/linux/wayland/window.rs:63`), and
    `WgpuRenderer::new`'s bound is `W: … + Clone + 'static`
    (`crates/gpui_wgpu/src/wgpu_renderer.rs:268`). Change every `&'a RawWindow` to
    `RawWindow`.
12. **`as_hal` has a different signature in wgpu 29.** It is
    `unsafe fn as_hal<A: hal::Api>(&self) -> Option<impl Deref<Target = A::TextureView>>`
    (`wgpu-29.0.4/src/api/texture_view.rs:73`) — one generic parameter, no closure,
    `unsafe`, and it returns `Option`. The draft's
    `as_hal::<api::Metal, _>(|hal_view| hal_view.expect(..).raw_handle())` is the
    pre-29 closure form and will not compile. `wgpu::hal` itself is public and safe to
    name (`pub extern crate wgpu_hal as hal`, `wgpu-29.0.4/src/lib.rs:104`).
13. **There is no `hal::api::Dx11`.** wgpu-hal 29.0.4's backends are `dx12`, `gles`,
    `metal` and `vulkan` (`wgpu-hal-29.0.4/src/lib.rs:252`) — there is no DX11 HAL. So
    the Windows arm can only be `Dx12`, and that exposes the real problem below
    (§6): **GPUI's Windows renderer is Direct3D 11**
    (`crates/gpui_windows/src/directx_renderer.rs:2086`) while wgpu is Direct3D 12, so
    the two are not interchangeable without shared-handle interop. Windows is the one
    platform where Path A is not a HAL lookup.
14. **`order` is the scene's to assign, so `current_paint_order()` is neither an API nor
    needed.** `Scene::insert_primitive` computes the order itself
    (`crates/gpui_engine/src/scene.rs:85`, `:95`) and overwrites each primitive's field
    (`:102`–`:131`). The element supplies `bounds` and `content_mask` — the latter from
    `window.content_mask()` (`crates/gpui_authoring/src/window.rs:4586`) — and the scene
    orders it against its siblings. `cx.current_paint_order()` does not exist.
15. **`register_foreign_texture` cannot return `AtlasTextureId`.** `AtlasTextureId`
    (`crates/gpui_engine/src/atlas.rs:254`) indexes the atlas textures the renderer
    *owns*; a foreign texture is not one of them. It needs its own id type, as the
    architecture document's correction 4 says — v2.0.0 dropped that correction rather
    than absorbing it.
16. **The erasure loses the type on macOS and Windows.** `Arc::new(raw_ptr as usize)`
    erases a pointer to a bare `usize`, which will collide with any other `usize`
    payload and cannot be checked. A newtype per backend (`struct MetalTexture(usize)`)
    costs nothing and keeps the downcast honest.
17. **`PlatformRenderer` and `SceneRenderer` now both declare `set_viewport_size`.**
    `SceneRenderer::set_viewport_size` already exists under
    `#[cfg(any(test, feature = "test-support", feature = "bench-support"))]`
    (`crates/gpui_engine/src/renderer.rs:32`); the new required method on the subtrait
    collides with it in test builds. Correction 5 is still open: lift the gate and put
    it on the real contract, or drop it from `PlatformRenderer`.
18. **Patch 02 and patch 03 still contradict each other.** §0 says the field is on both
    `WindowOptions` and `WindowParams`, but patch 02's diff adds it to `WindowParams`
    only (`crates/gpui_platform/src/window.rs:438`) while patch 03 reads it off
    `options`, the `WindowOptions` (`crates/gpui_authoring/src/window.rs:1756`).
    `WindowOptions` is at `:357` in the same file and derives `Debug` at `:356`, so it
    needs the field and the builder method. No patch defines
    `WindowOptions::with_renderer_factory`.
19. **Removing the upcast shims makes trait upcasting load-bearing.** With
    `as_scene_renderer` gone, `PlatformWindow::with_renderer`/`present`
    (`crates/gpui_platform/src/platform_window.rs:151`, `:157`), which take
    `&mut dyn SceneRenderer`, only reach a `Box<dyn PlatformRenderer>` by upcasting.
    That is stable (`rust-toolchain.toml` is 1.95) but should be stated, because it is
    now the thing the whole seam depends on.
20. **`GpuCanvas` still will not compile, and one change is a regression.** §7.

## 1. Initialisation, and why the factory is once per window

The ordering claim is right and is the reason the factory lives where it does: the
native surface must exist before the factory runs, so it is invoked exactly once per
window, after the window is allocated, and never again.

```
WindowOptions.renderer_factory            # gpui_platform (correction 1)
  → WindowHost::new                        # destructures (crates/gpui_authoring/src/window.rs:1441)
  → Platform::open_window(.., WindowParams) # (crates/gpui_platform/src/platform.rs:95)
      → allocate the native window          # wl_surface / HWND / NSWindow
      → pack the backend target once        # owned, correction 11
      → RendererTarget::new(&backend)       # erase
      → factory.create(target)? | default
  → Box<dyn PlatformRenderer> in the window
```

Two consequences that follow from "once": recovery must be self-sufficient on the
returned renderer (there is no second target resolution), and post-construction queries
like `max_texture_size()` and `set_subpixel_layout(is_bgr)` — x11 at
`crates/gpui_linux/src/linux/x11/window.rs:772` — belong on `PlatformRenderer`, not in
the factory's input.

## 2. Path A

The colour-space invariant the draft adds is real and belongs here: the offscreen
texture must be sampled as sRGB (`Rgba8UnormSrgb`/`Bgra8UnormSrgb`), or it composites
with a ≈2.2 gamma error; and the sampler is linear with `ClampToEdge`, so a clip against
rounded corners cannot bleed. This is the generalisation of `PaintSurface`
(`crates/gpui_engine/src/scene.rs:749`), whose macOS payload is a `CVPixelBuffer`.

The erasure is right in principle and, as v2.0.0 states, legitimate because the
registering application and the consuming renderer are counterparties — the engine only
transports a token. Three fixes make it compile: the target owns its handles
(correction 11), the handle payload is a typed newtype rather than `usize`
(correction 16), and the id is not an `AtlasTextureId` (correction 15).

## 3. Path B

The state-isolation matrix is kept verbatim; it describes GPU state and is unaffected
by the API corrections:

| subsystem | changed by an injected shader | restored by the renderer |
| --- | --- | --- |
| scissor | clamped to `primitive.bounds` | the full target rect |
| viewport | possibly local bounds | `(0, 0, device_w, device_h)` |
| pipeline | custom VS/FS | the quad and text pipelines |
| depth/stencil | custom tests/masks | disabled |
| blend | custom or additive | `SrcAlpha, OneMinusSrcAlpha` |
| vertex buffers | slots `[0..N]` overwritten | the GPUI instance buffer at slot 0 |
| samplers | sampler registers changed | the atlas sampler |

One simplification and one warning. The simplification: with correction 14 the element
does not pass an order, so the "rewind the batch cursor" step is about `content_mask`
and the scene's own ordering, not about an order the element computed. The warning:
the draft's "Wgpu: conclude the active `PrimitiveBatch::Surfaces` or flush the quad
draw call" is not a thing wgpu's batcher exposes; the wgpu arm is the one that has to be
designed against `crates/gpui_wgpu/src/wgpu_renderer.rs`, not sketched by analogy from
Metal and DirectX — and it is also the arm Path A still needs
(`PrimitiveBatch::Surfaces(_surfaces) => {}`,
`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`).

## 4. Device loss and recovery

Correct in outline and verified: `WgpuRenderer::recover<W>(&mut self, window: &W)`
(`crates/gpui_wgpu/src/wgpu_renderer.rs:2131`), called from `present` guarded by
`device_lost()` on x11 (`crates/gpui_linux/src/linux/x11/window.rs:1765`) and wayland
(`crates/gpui_linux/src/linux/wayland/window.rs:1960`); `new_rejecting_software` exists
(`crates/gpui_wgpu/src/wgpu_context.rs:76`). The shared-context adoption rule — the
first recovering window rebuilds, the rest adopt (`Rc<RefCell<Option<WgpuContext>>>`,
`crates/gpui_wgpu/src/wgpu_renderer.rs:168`) — is the subtle part and is right.
v2.0.0 correctly drops the "Wayland/X11 only" framing; macOS and Windows lose devices
too, so the protocol is stated once and implemented three times.

## 5. The patch set

Unchanged in shape from v1, with the corrections above applied:

| patch | change |
| --- | --- |
| 01 | new `gpui_platform/src/platform_renderer.rs`: `PlatformRenderer`, `RendererFactory`, `DynRendererFactory`, `RendererTarget` |
| 02 | `renderer_factory` on **both** `WindowOptions` (`crates/gpui_platform/src/window.rs:357`) and `WindowParams` (`:438`), plus the `with_renderer_factory` builder — correction 18 |
| 03 | forward it through `WindowHost::new` into `open_window` |
| 04–05 | wayland (`crates/gpui_linux/src/linux/wayland/window.rs:582`), x11 (`crates/gpui_linux/src/linux/x11/window.rs:769`) |
| 06–07 | macOS (`crates/gpui_macos/src/window.rs:1100`), Windows (`crates/gpui_windows/src/window.rs:145`) |

v2.0.0 keeps the improvement worth keeping from the last round: the backend target is
built **once** and handed to whichever arm wins, so the factory and default paths cannot
drift. It never names patches 06–07, so on two of four backends the field would still be
silently ignored.

## 6. HAL extraction, and the Windows problem

The extraction belongs in `gpui_wgpu` — adopted. Correcting the mechanics:

- `as_hal` is `unsafe`, takes one generic parameter, and returns
  `Option<impl Deref<Target = A::TextureView>>` (correction 12). Wrap it in the
  `unsafe` block the draft omits and handle the `Option`.
- **macOS** works: wgpu's Metal backend and GPUI's Metal renderer are the same API
  (`wgpu-hal-29.0.4/src/lib.rs:258`).
- **Linux** needs no extraction at all — wgpu *is* the renderer, so the app already
  holds the `TextureView`.
- **Windows does not work as drafted, and this is the design's hardest open piece.**
  wgpu-hal has no DX11 backend (correction 13); GPUI's Windows renderer is D3D11. A
  wgpu (D3D12) texture cannot be sampled by a D3D11 device without a shared handle
  (`ID3D12Resource` → `ID3D11Texture2D` via `OpenSharedHandle`), which is a real piece
  of interop to design and test, not a `raw_view()` call. Either Path A is
  D3D12-renderer-only on Windows, or the producer must be a D3D11 device, or the
  shared-handle bridge has to be built. Decide this before writing the patch.

## 7. `GpuCanvas`, again

v2.0.0 adopts "delegate to `div()`" as a tenet and then, in §3.1, does not do it. Four
problems, the last a regression:

- **Still `WindowContext`.** Every callback and lifecycle method must be
  `(…, window: &mut Window, cx: &mut App)`; `RenderOnce::render` is
  `fn render(self, window: &mut Window, cx: &mut App)`
  (`crates/gpui_authoring/src/element.rs:180`). `cx.request_layout`,
  `cx.register_foreign_texture` and `cx.push_custom_primitive` are all `window.`-level.
- **Still `CornerRadii`.** It is `Corners<Pixels>`
  (`crates/gpui_types/src/geometry.rs:2235`).
- **The private child element reintroduces the sizing and hitbox problems.** Its
  `request_layout` uses `Style::default()`, so a child with no intrinsic size collapses;
  and because `GpuCanvas` is a `RenderOnce` and not an `Element`, `.id()` yields
  `Stateful<GpuCanvas>`, which is only an element when `GpuCanvas: Element`
  (`crates/gpui_authoring/src/elements/div.rs:4078`) — so `on_click`/`on_hover` are
  unavailable. [`gpu-canvas-dx.md`](gpu-canvas-dx.md) already resolves both: implement
  `Element` on `GpuCanvas` by delegating to the inner `Div`, push the primitive after
  `Div::paint`, and use no child.
- **The texture callback lost its `bounds` argument** — `Fn(&mut WindowContext) ->
  ForeignTextureHandle`. Path A renders *offscreen into a texture of the element's
  size*, so the callback must receive `bounds`; v1.0.0 had it and v2.0.0 dropped it.
  Restore `Fn(Bounds<Pixels>, &mut Window, &mut App) -> ForeignTextureHandle`.

## 8. Headless export

v2.0.0 stops naming `gpui_animotion` and frames the pipeline as "video recording and
automated CI visual testing" — the correction, accepted. The bounded ring
(`sync_channel(4)`), the "the virtual clock pauses, frames are never dropped"
backpressure rule, and the reused pinned staging pool are a coherent design for a
video exporter; they are just a design for a crate that does not exist yet.

The parity claim that belongs to *this* design is narrower and testable, and it is the
seam document's: the primitives render offscreen, so CI can read a frame back and
assert on it.

## 9. Verification matrix

Kept, as the test plan. Each row is an assertion the headless harness can actually
make:

| scenario | to prevent | assertion |
| --- | --- | --- |
| resize / surface churn | stale swapchain geometry | after `set_viewport_size`, the next frame's scissor and viewport match the new size |
| DPI change | half/double-scale viewport | `bounds * scale_factor` rounds to the coordinates the platform target reports |
| premultiplied alpha | fringes at rounded corners | a known RGBA fixture composites to a known pixel on readback |
| device loss | `SurfaceLost` panic | `device_lost()` → `recover()` → a frame draws, headless |
| thread affinity | `GpuContext` crossing threads | the factory is `!Send` by construction (`Rc`), so the type system is the guard |
| Path A colour space | washed-out composite | an sRGB fixture round-trips without a ≈2.2 gamma shift |
| foreign-texture id | an atlas id reused for a foreign texture | the registry rejects an id it did not hand out (correction 15) |

## Open before implementing

- **Correction 13 / §6** — the Windows cross-API question. This is the only item that
  could invalidate a whole path on a platform, and it has no answer yet.
- **Correction 17** — `set_viewport_size`'s test gate.
- **Correction 11** — confirm every backend target is `'static` (owns its handles).
- **The native hooks** — still owned by
  [`scene-renderer-seam.md`](scene-renderer-seam.md) §3, and still needed: v2.0.0's
  `PlatformRenderer` has no place for the macOS layer
  (`crates/gpui_macos/src/window.rs:3087`) or the Windows appearance
  (`crates/gpui_windows/src/window.rs:1039`).
