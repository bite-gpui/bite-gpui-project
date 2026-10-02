# External surfaces: the unified path

- **Status:** proposed — the design of record for pixels produced outside GPUI. It is the October
  2026 *External Surfaces, Platform Interop, and Headless Rendering in GPUI* design record turned
  into a chapter and reconciled with what is built, per
  [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md). The arms it adds
  (Windows, Linux) are not implemented; the machinery it re-homes is.
- **Assumes:** [`renderer-seam.md`](renderer-seam.md) — a renderer is installable at all — and
  [`producer-reach.md`](producer-reach.md) — how a producer gets the device.
- **Supersedes by name:** [`foreign-texture.md`](foreign-texture.md)'s *second primitive* and its
  `paint_imported_texture` call. The three arms, the colour-space invariant and the rendezvous
  survive the retarget; only the entry point and the enum change.
- **Target crates:** `gpui_engine` (the primitive and its payload enum), `gpui_authoring` (the
  element), and every renderer: `gpui_windows`, `gpui_apple`, `gpui_wgpu`/`gpui_linux`; the
  cross-API bridge is a downstream crate, not one of these.
- **The plan and the probes it gates on:** [`surface-plan.md`](surface-plan.md).

## 1. One primitive, two distances

`PaintSurface` already exists — a variant of the scene's primitive enum
(`crates/gpui_engine/src/scene.rs:238`, struct at `:784`), a `Surface` element and its `surface()`
constructor (`crates/gpui_authoring/src/elements/surface.rs:27`, `:35`), the window call it goes
through on macOS (`crates/gpui_authoring/src/window/mac.rs:17`) and a `draw_surfaces` on every
renderer (`crates/gpui_windows/src/directx_renderer.rs:852`,
`crates/gpui_apple/src/metal_renderer.rs:1156`, and wgpu's empty arm at
`crates/gpui_wgpu/src/wgpu_renderer.rs:1698`). It is macOS-only today because `SurfaceSource` has
one variant (`crates/gpui_authoring/src/elements/surface.rs:13`) and `draw_surfaces` bails off macOS.

**External pixels therefore extend this, rather than adding a second primitive.** The reason is not
taste: a same-device texture is the *zero-copy, single-device special case* of surface interchange.
On Windows both reach the renderer as an `ID3D11ShaderResourceView` and the renderer cannot tell how
it was made, so a separate `Texture` variant buys nothing; on every platform the element, the
ordering, the content mask and the batch are already written and reachable by a name an application
knows — `surface()`. [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md)
records the route we built instead (`CustomRenderPrimitive::Texture`,
`crates/gpui_engine/src/custom_render.rs:41`) and why it is superseded.

The payload becomes a **typed, cfg-gated enum**, the shape `PaintSurface` already has — its
`#[cfg]` `image_buffer` field (`crates/gpui_engine/src/scene.rs:784`) is the precedent, not a compromise:

```rust
pub enum SurfaceSource {
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    CoreVideo(CVPixelBuffer),          // IOSurface-backed
    #[cfg(target_os = "windows")]
    DirectX(ID3D11ShaderResourceView), // same-device texture, or an opened NT handle
    #[cfg(target_os = "linux")]
    DmaBuf(DmaBufHandle),              // fd + fourcc + modifier + stride
}
```

`PaintSurface` also gains `corner_radii` — the element already stubs it
(`crates/gpui_authoring/src/elements/surface.rs:99`) — and the `opacity` and `flip_v` our primitive carried. **Its `bounds`
stay `Bounds<ScaledPixels>`** (`crates/gpui_engine/src/scene.rs:786`): a renderer has no scale factor, so the element
converts at paint time, as `paint_quad` does.

**Name the collision.** "Surface" is overloaded. In wgpu a `Surface` is the *window presentation
target*; a wgpu `Texture` is the buffer, shared or in-process. Here, as in the OS compositors GPUI
follows, a surface is the **external buffer composited into the scene** — which is why the same type
can carry an in-process texture and a cross-process handle.

## 2. The OS surface trinity

Zero-copy exchange across processes, APIs or devices is a kernel-level handshake. The three platforms
have one each, and they are the same concept three times:

| platform | transport | imported as | synchronisation |
| --- | --- | --- | --- |
| macOS | `IOSurface` (via `CVPixelBuffer`) | an `MTLTexture` over the surface | `MTLSharedEvent` |
| Windows | DXGI shared NT handle (`IDXGIResource1`) | `OpenSharedResource1` → `ID3D11Texture2D` → SRV | keyed mutex, or `ID3D12Fence`↔`ID3D11Fence` |
| Linux | `dma-buf` (fd + DRM fourcc/modifier) | `EGLImage` / `VkImage` (external memory fd) | dma-fence / sync fd |

Two rules run under all three, and both are measured rather than argued
([`../../decisions/shared-surface.md`](../../decisions/shared-surface.md)):

1. **The application allocates; GPUI adopts.** wgpu can never *create* a shareable texture — its
   DX12 backend passes `D3D12_HEAP_FLAG_NONE` on everything, and its Metal backend names no
   `IOSurface`. It can *adopt* one: `texture_from_raw` + `create_texture_from_hal` are public on all
   four desktop backends. So the division of labour is fixed — the producer makes the shareable
   resource and GPUI opens it — and it is why the surface payload is a handle the application built.
2. **Same device first; a bridge only across devices.** A same-device texture needs no handle and no
   fence: it is two views of one resource, ordered by submission. A second device needs the handle
   *and* the synchronisation, plus a same-*physical*-adapter guarantee (an LUID match that cannot be
   asserted from inside either device). That is the whole reason the same-device route is the
   milestone and the bridge is downstream.

`dma-buf` belongs to this table, not to the texture row: it is Linux's transport, the sibling of
`IOSurface` and the NT handle, and it is unnecessary only for the same-device case (where, on Linux,
both sides are wgpu). A Linux *cross-process* producer — a VA-API/NVDEC decoder, a Wayland client —
is exactly the case it is for. This corrects `0002`'s "Linux: no bridge needed", which was a claim
about the same-device case alone.

The Linux arm carries two invariants the P3 probe established
([`../../decisions/linux-dmabuf-probe.md`](../../decisions/linux-dmabuf-probe.md)). First, the buffer
must be **self-contained and uncompressed under its declared modifier**: ANV enables implicit (CCS)
compression for a sampled tiled image, and that state is not carried by a single-plane dma-buf, so a
compressed producer imports as raw compressed bytes. Second, an `NV12` buffer is consumed as **two
plane textures** (`R8` + `R8G8`) with the colour matrix in the shader, because the native
`VK_KHR_sampler_ycbcr_conversion` needs an immutable sampler `wgpu` cannot bind. Third, a buffer that
crosses devices must be a **dedicated allocation** (`VkMemoryDedicatedAllocateInfo`) — the importing
GPU refuses it otherwise on every memory type, even while advertising the format as importable.
`DmaBufHandle` is the consumer of that contract, not a place to negotiate it.

## 3. Host mode: composing external frames in

GPUI owns the window, the loop and the swapchain; an external workload streams frames into a viewport.

```
External workload (engine, decoder, webview)         GPUI device rendezvous
  renders an offscreen frame                 ──▶       surface(handle)  ──▶  the scene
        │                                                      │
        └─ same device: an SRV/TextureView/CVImageBuffer ──────┤
        └─ other device: an OS handle (0002's Tier 2) ─────────┘
```

The renderer's obligation is one method, `draw_surfaces`:

- **Bind the surface and draw a quad through it.** The geometry, the content-mask clip and the
  corner SDF are the quad path's, not a second implementation — which is what
  `CustomRenderPrimitive::to_quad_record` (`crates/gpui_engine/src/custom_render.rs:101`) already
  encodes, and what the Direct3D arm already reads (`crates/gpui_windows/src/directx_renderer.rs:862`). The retarget keeps
  it and moves the call.
- **Straight-through, bit-exact sampling.** On Direct3D the surface's non-sRGB view is sampled
  straight to the `*_UNORM` backbuffer, with no transfer function to cancel — the finding
  `foreign-texture.md` §4 already states, and the reason the Windows arm has no re-encode.
- **Unbind immediately after the draw.** `PSSetShaderResources(0, None)` clears the slot so an
  external producer writing the resource next frame cannot race the bind.
- **Fault softly.** A surface with zero dimensions or an invalid state is dropped for that frame,
  not a panic: the rest of the UI frame completes.
- **Do not synchronise.** GPUI manages no fence and no keyed mutex; ordering an external writer
  against GPUI's queue is the producer's obligation (or the interop crate's, §5).

## 4. Guest mode: GPUI inside a foreign loop

There is no desktop `run_embedded`, and there will not be: the platform layers fuse the message
loop, presentation pacing, IME and the executors into one runtime, a host loop would fight the
`CVDisplayLink`/waitable-object pacing, and an alternate runtime Zed never exercises bit-rots. The
supported shape is **headless rendering on a worker thread**:

- the host owns the window and its cadence; the GPUI thread runs a headless platform context with no
  native surface, ticking its executors;
- the frame is produced through the offscreen contract that already exists —
  `render_scene` split from `read_pixels` (`crates/gpui_engine/src/renderer.rs:80`, `:102`, `:110`)
  with `PixelBuffer` (`:19`) as the CPU type — so a GPU consumer takes the surface and a CPU consumer
  takes the bytes, and neither is a special case of the other;
- the two hops a host needs (translated input in, sampled frame out) are ordinary platform work, not
  a new seam.

The offscreen contract is built and its rows are asserted ([`verification.md`](verification.md) §2);
what is unbuilt is the runner that ties the loop together, and it belongs with the interop crate.

## 5. The boundary: what core does *not* do

Core GPUI stays free of `wgpu` and Direct3D 12. Everything that needs them — matching a `wgpu`
adapter to GPUI's Direct3D 11 adapter by LUID, exporting and opening a shared NT handle, and
bridging an `ID3D12Fence` to an `ID3D11Fence` — lives in a **downstream companion crate**
(`gpui-interop`, a working name). Core publishes only the two things the crate needs: the device
(`Window::device_any`, `crates/gpui_authoring/src/window.rs:3031`, erased on the traits at
`crates/gpui_platform/src/platform_window.rs:159` and `:97`) and the surface element. This is
[`0002`](../../decisions/0002-render-extension-device-model.md)'s deferred Tier 2, now *scheduled*
rather than deferred — but the same-device route is the milestone, and the bridge is a second step
whose reverse direction is still unmeasured ([`producer-reach.md`](producer-reach.md) §6). The bridge
the crate ships is the **Host Mode** direction — a `wgpu`/Direct3D 12 producer into GPUI's Direct3D 11
renderer, already measured; the *reverse* (a Direct3D 11 producer into a `wgpu` host) is a Guest Mode
concern scoped to W6, so a P1 failure costs the guest path and not the crate. The crate itself is
planned in [`interop-crate.md`](interop-crate.md).

## 6. What this changes here, and what it keeps

- **Kept verbatim:** `PixelBuffer` and the `render_scene`/`read_pixels` split
  (`crates/gpui_engine/src/renderer.rs:19`, `:102`, `:110`) — the record's upstream PR 1, already built; the quad-record
  encoding (`crates/gpui_engine/src/custom_render.rs:101`); the Direct3D arm's sampler and SRV path
  (`crates/gpui_windows/src/directx_renderer.rs:862`, `create_imported_texture_view` at `:1700`); and `device_any`.
- **Retargeted:** `CustomRenderPrimitive::Texture` → `PaintSurface`; `paint_imported_texture`
  (`window.rs:5000`) → `surface()`; the token builders (`ImportedTextureExt`, `DirectXTextureExt`,
  `MetalTextureExt`) → `From<…> for SurfaceSource`; `draw_custom` → `draw_surfaces`.
- **Superseded:** the second primitive, the erased `ImportedTextureHandle`
  (`crates/gpui_engine/src/custom_render.rs:14`), and `Primitive::Custom`'s `Texture` arm; `CustomRenderPrimitive` keeps
  only its `Inline` (Path B) arm, which is untouched — it draws into GPUI's own pass, not a buffer.
- **Unchanged:** Path B ([`inline-commands.md`](inline-commands.md)), and the seam
  ([`renderer-seam.md`](renderer-seam.md)) — this chapter adds no method to `SceneRenderer` beyond
  the `draw_surfaces` that already exists. Path B is **fork-carried**: upstream has said the callback
primitive is not one it wants ([zed #64849](https://github.com/zed-industries/zed/discussions/64849)),
so it is W7 and never a PR ([`upstream-prs.md`](upstream-prs.md) §5).

## 7. Open, and what would reopen this

- **A same-device texture on macOS and Linux has no natural `SurfaceSource` variant.** `CoreVideo`
  needs an `IOSurface` and `DmaBuf` a dma-buf; neither is a `wgpu::TextureView` or a raw
  `id<MTLTexture>`. On Windows the SRV covers both distances; here it does not. Either the enum
  grows a same-backend arm per platform — leaning back toward the erased payload — or the
  same-device case keeps a path of its own, which is where `GpuCanvas` still earns its place
  ([`../authoring/gpu-canvas.md`](../authoring/gpu-canvas.md)).
- **The `DirectXWindowExt` spelling.** The design sketches `impl DirectXWindowExt for gpui::Window`
  in `gpui_windows`, which cannot name `gpui::Window` (it depends on `gpui_engine` and
  `gpui_platform` only). The typed accessor stays in the facade over `device_any`, as
  [`0004`](../../decisions/0004-producer-device-rendezvous.md) decided.
- **Non-RGBA surfaces.** YCbCr (video), 10-bit and premultiplied-alpha surfaces would make the
  RGBA-only straight-through fragment wrong. Whether they are a second fragment or a format arm is
  unset; it is the first thing to measure (§[`surface-plan.md`](surface-plan.md) P4).
- **Opaque-only Windows presentation.** A window rendered by `WgpuRenderer` has no transparency
  ([`../../decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md)),
  so the unified path inherits the same-device tier's alpha caveat on Windows.
