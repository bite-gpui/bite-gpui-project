# 0005 — External rendering unifies under `PaintSurface`, not a second primitive

- **Decided:** 2026-10-01
- **Status:** decided — supersedes the second-primitive route in
  [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) and
  [`../spi/rendering/producer-reach.md`](../spi/rendering/producer-reach.md); amends
  [`0002`](0002-render-extension-device-model.md) (its deferred Tier 2 bridge is now scheduled in a
  downstream crate) and [`0004`](0004-producer-device-rendezvous.md) (the typed accessor's spelling,
  not its placement)
- **Evidence:** the *External Surfaces, Platform Interop, and Headless Rendering in GPUI* design
  record (October 2026), filed with the work; the code on the canonical ref, cited below; and the
  probes it rests on — [`shared-surface.md`](shared-surface.md),
  [`windows-path-a-probe.md`](windows-path-a-probe.md),
  [`macos-wgpu-producer-probe.md`](macos-wgpu-producer-probe.md); and the upstream Discussion
  [zed-industries/zed#64849](https://github.com/zed-industries/zed/discussions/64849), which agrees
  the model and says the Windows arm is welcome

## Decision

**External pixels reach GPUI as a `PaintSurface`, through the existing `surface()` element. There is
no second scene primitive.** Five consequences:

1. **One primitive, and it already exists.** `Primitive::Surface(PaintSurface)`
   (`crates/gpui_engine/src/scene.rs:238`, struct at `:784`), the `Surface` element and `surface()`
   (`crates/gpui_authoring/src/elements/surface.rs:27`, `:35`), `SurfaceSource`
   (`:13`), `MacWindowExt::paint_surface` (`crates/gpui_authoring/src/window/mac.rs:17`) and each
   renderer's `draw_surfaces` (`crates/gpui_windows/src/directx_renderer.rs:852`) are the seam. They
   are **extended, not replaced**. The element was macOS-only when this was decided, because
   `SurfaceSource` had one variant and `draw_surfaces` was a stub off macOS; it now carries the
   `CoreVideo`/`DirectX`/`DmaBuf` variants and every renderer implements `draw_surfaces`.
2. **The surface payload is a typed, cfg-gated enum.** `SurfaceSource` grows a
   Windows arm and a Linux arm (a dma-buf handle), exactly as `PaintSurface`'s existing `#[cfg]` field
   does today. The Windows arm carries either the texture (the renderer makes the view) or a view the
   producer made (`DirectXSource`) — the ergonomic default and its escape, and the shape that makes
   Windows symmetric with macOS, whose `CoreVideo` arm is likewise a *resource*. `PaintSurface` also
   gains `corner_radii`, which the element already stubs as a `TODO`
   (`crates/gpui_authoring/src/elements/surface.rs:99`), plus the `opacity` and
   `flip_v` our primitive carries.
3. **dma-buf is a Surface transport, not a texture.** It is Linux's sibling of `IOSurface` and the
   DXGI shared NT handle: a kernel-level, cross-process, cross-API buffer handshake, imported into
   an `EGLImage`/`VkImage` at the renderer boundary. It belongs in the same enum as the other two.
4. **Cross-API bridging is downstream.** The Direct3D 12 / `wgpu` → Direct3D 11 shared-handle and
   fence handshake lives in a companion crate (`gpui-interop`), not in core GPUI. Core stays free of
   `wgpu` and D3D12.
5. **Guest mode is the offscreen contract, not a desktop `run_embedded`.** A foreign host embeds
   GPUI as a headless renderer on a worker thread; it is the same
   `render_scene`/`read_pixels`/`render_scene_to_image` contract (`crates/gpui_engine/src/renderer.rs:80`,
   `:102`, `:110`, `:115`), with the GPU path handing back a surface instead of a CPU buffer.

## What each built artifact becomes

The second-primitive route we built is not discarded: **its machinery is the implementation** the
new entry point calls.

| built artifact (this tree) | where | becomes |
| --- | --- | --- |
| `PixelBuffer`, and `render_scene` split from `read_pixels` | `crates/gpui_engine/src/renderer.rs:19`, `:102`, `:110`, `:115` | **kept verbatim** — this is the design's upstream PR 1, already built |
| `CustomRenderPrimitive::to_quad_record` — the geometry, content-mask and corner-SDF encoding into the quad record | `crates/gpui_engine/src/custom_render.rs:101` | **kept** — it is the record the surface fragment reads, so the HLSL/SDF path survives |
| the Direct3D arm: SRV creation, the imported-texture sampler, the per-primitive draw | `crates/gpui_windows/src/directx_renderer.rs:862` | **retargeted** into `draw_surfaces` (`:852`), the stub it replaces |
| `Window::device_any` and the erased `PlatformWindow`/`PlatformRenderer` accessors | `crates/gpui_authoring/src/window.rs:3031`, `crates/gpui_platform/src/platform_window.rs:159`, `crates/gpui_platform/src/platform_renderer.rs:97` | **kept** — the rendezvous 0004 decided; only its typed facade spelling changes (below) |
| `ImportedTextureExt` / `DirectXTextureExt` / `MetalTextureExt` (token builders on a texture) | `crates/gpui_windows/src/imported_texture.rs:43`, re-exported at `crates/gpui/src/platform_entry.rs:37` | **retargeted** into `From<…> for SurfaceSource` converters; the format/usage validation they do becomes the converter's |
| `CustomRenderPrimitive::Texture`, `ImportedTextureHandle`, `Primitive::Custom`, `scene.custom`, `paint_imported_texture`, `painted_imported_textures` | `crates/gpui_engine/src/custom_render.rs:41`, `:14`; `crates/gpui_engine/src/scene.rs:239`; `crates/gpui_authoring/src/window.rs:5000`, `:3037` | **superseded** — the surface path replaces them; `CustomRenderPrimitive` keeps only its `Inline` (Path B) arm |
| the cross-API fence bridge | not built | **not built** — measured in [`shared-surface.md`](shared-surface.md); now parked in `gpui-interop` rather than deferred |

## Why

**The two are the same primitive at different distances.** A same-device texture is the zero-copy,
single-device special case of surface interchange: on Windows both arrive at the renderer as an
`ID3D11ShaderResourceView`, and the renderer cannot tell how the SRV was made — which is why a
distinct `Texture` primitive buys nothing upstream will take. Standardising on `PaintSurface` also
means the element, the ordering, the content mask and the batch are already written and already
reachable by name an application knows (`surface()`).

**It is the shape upstream already agreed to.** Unification under `PaintSurface` plus the Windows
arm is an upstream PR, not a fork: it satisfies an open Windows `paint_surface` request and alters
no existing pass. The second-primitive route would have to be carried on every branch instead.

**The dependency argument does not survive the unification.** `foreign-texture.md` §3 (and
`README.md` §Open) recommended an *erased* handle to keep `wgpu` and the platform graphics APIs out
of `gpui_engine`. But `PaintSurface` already takes a `#[cfg(target_os = "macos")] CVPixelBuffer` —
a cfg-gated dependency is the precedent the element was built on, not a compromise. Unifying with it
is worth more than the erasure, and the device rule is unchanged by the choice.

**The expensive parts are already paid.** The SDF quad clipping, the straight-through `_UNORM`
sampling on Direct3D, the rendezvous and the readback rows are the same engine either way; what the
retarget changes is the *name of the entry point and the enum it sits behind*.

## Rejected alternative

**A second scene primitive, `CustomRenderPrimitive::Texture`.** This is the route we built and merged
as PR #6; it is rejected here on upstream-parity grounds, not on correctness. It is a correct design
— one accumulation list, one batch, an erased token that cannot be wrong, and an arm on all three
renderers — and its arm is not thrown away. What it costs is a second place for the scene to carry
"pixels from outside", a second name an application meets, and a primitive upstream has said it will
not merge. Recorded so the re-litigation is not paid twice.

## What the design does not settle

- **A same-device texture on macOS and Linux has no natural `SurfaceSource` variant.** On Windows the
  SRV covers it, but `CoreVideo` needs an *IOSurface* and `DmaBuf` needs a *dma-buf*, and neither is
  what a `wgpu::TextureView` or a raw `id<MTLTexture>` is. Either the enum grows a same-backend arm
  per platform (which leans back toward the erased payload) or the same-device case keeps a path of
  its own. This is the one gap the design's "same-device is just a special case" does not close, and
  it is where `GpuCanvas` still earns its place.
- **`DirectXWindowExt` as the design sketches it inverts a layer.** Its `impl DirectXWindowExt for
  gpui::Window` lives in `gpui_windows`, but `gpui_windows` depends on `gpui_engine` and
  `gpui_platform` only (`crates/gpui_windows/Cargo.toml`) and cannot name `gpui::Window` — the same
  wall 0004 hit. The typed accessor belongs in the facade over the erased `device_any`, as `0004`
  decided; only the spelling `DirectXWindowExt::d3d11_device()` is new.
- **`GpuCanvas` and Path B.** The design is silent on both. `GpuCanvas` stays as the authoring
  surface and retargets its Path A arm onto `surface()`; Path B (`CustomRenderPrimitive::Inline`) is
  a different capability — commands in GPUI's own pass, not a buffer — and is untouched.
- **The `gpui-interop` boundary.** Adapter-LUID matching, the NT-handle export/import, and the
  `ID3D12Fence`↔`ID3D11Fence` bridge are one crate's worth of surface but not yet specified here;
  the reverse direction (a D3D11 producer feeding a `wgpu` consumer) is still the one
  [`producer-reach.md`](../spi/rendering/producer-reach.md) §6 records as unmeasured.
- **`PaintSurface`'s bounds must stay `ScaledPixels`.** The design writes `Bounds<Pixels>`, but the
  primitive takes `Bounds<ScaledPixels>` (`crates/gpui_engine/src/scene.rs:786`) because a renderer
  has no scale factor of its own; the element converts at paint time, as `paint_quad` does. The
  conversion stays in the element, and the design's `Pixels` is a slip, not a change.

## What would reopen it

- **Upstream rejecting the cfg-gated enum.** If core will not name `ID3D11ShaderResourceView` or a
  dma-buf fd, the erased payload returns and this record's item 2 is wrong.
- **A same-device variant forcing erasure.** If the macOS/Linux same-device gap above is closed with
  a per-backend arm, the enum trends back toward the very erasure it replaced and the split should be
  reconsidered whole.
- **Opaque OS surfaces arriving without a sampler-compatible format** — a YCbCr or 10-bit surface
  would make the straight-through `_UNORM` assumption, and the RGBA-only fragment, wrong.
