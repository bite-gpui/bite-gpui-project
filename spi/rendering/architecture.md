# The rendering slice: how pixels produced outside GPUI reach a window

- **Status:** the architecture as built on the fork branch `bite_v1.23.1-pre-interop`.
  That branch is **ahead of the canonical ref `bite_v1.23.1-pre` and is not merged
  into it** — the merge is a separate, CI-verified step (the fork's `bite-ci.yml` runs
  the per-platform rows; see [`verification.md`](verification.md)).
  Where this document names a file, the shape is what the branch has; a citation that
  resolves against the canonical ref would show the pre-slice shape.
- **Scope:** the *rendering slice* — the crates and seams through which a producer's
  pixels (a video frame, a decoded buffer, a second GPU's render target) are composited
  into a GPUI window. It is the map of this chapter; [`README.md`](README.md) is the
  index and the rest of the directory is the record.

## 1. The layer stack

```
  application code
  +---------------------------------------------------------------------------+
  |  surface(source)                       gpu_canvas(|gpu| ...)              |
  +---------------------------------------------------------------------------+
        |                                          |
        |  element-level                           |  entity/Render-level
        v                                          v
  +---------------------------------------------------------------------------+
  |  gpui            the facade: re-exports the authoring surface             |
  +---------------------------------------------------------------------------+
        |
        v
  +---------------------------------------------------------------------------+
  |  gpui_authoring                                                          |
  |    elements::surface        -> inserts a PaintSurface primitive          |
  |    elements::gpu_canvas     -> GpuCanvasContext (a slice of Window)      |
  |    device::<R>() / try_device::<R>() -> R::Device  (the producer door)  |
  +---------------------------------------------------------------------------+
        |
        v
  +---------------------------------------------------------------------------+
  |  gpui_engine                                                             |
  |    scene::Scene { surfaces: Vec<PaintSurface> }                          |
  |    scene::SurfaceSource { CoreVideo | DirectX | DmaBuf }   (cfg-gated)   |
  |    SceneRenderer::draw_surfaces(&[PaintSurface])                         |
  |    GpuRenderer::device() -> Option<Self::Device>   (owns the device)     |
  +---------------------------------------------------------------------------+
        |
        v
  +---------------------------------------------------------------------------+
  |  gpui_platform                                                           |
  |    PlatformWindow   the window surface   (no GPU accessor)               |
  |    PlatformRenderer the renderer seam                                    |
  +---------------------------------------------------------------------------+
        |
        +-----------+-------------+-------------+
        v           v             v             v
  +-----------+ +-----------+ +-----------+ +-----------+
  |gpui_windows| | gpui_wgpu | |gpui_apple | |gpui_linux |
  | D3D11/DXGI | |wgpu/Vulkan| |   Metal   | | X11/Wayland|
  +-----------+ +-----------+ +-----------+ +-----------+
```

`gpui_interop` sits to the side (§5): it is a *downstream* crate, not a layer, and
depends on `gpui` rather than being depended on by it.

## 2. The two authoring entry points

A producer reaches the scene through one of two elements, both in `gpui_authoring`:

```
  surface(source)              an ordinary element; composites one source
        |                      and lays out / stacks like any other child.
        v
  +---------------------------------------------------------------+
  |  PaintSurface { order, bounds, content_mask, source }         |
  +---------------------------------------------------------------+

  gpu_canvas(|gpu| ...)        a box whose content a paint-time callback supplies
        |
        v
  +---------------------------------------------------------------+
  |  GpuCanvasContext   a slice of Window:                        |
  |    paint_surface(impl Into<SurfaceSource>)   -> PaintSurface   |
  |    paint_texture(handle, radii, opacity, flip_v)              |
  |    bounds()  device::<R>()  try_device::<R>()  cx()              |
  +---------------------------------------------------------------+
        |
        +--- the *only* place raw GPU import is reachable: a bare   |
             `&mut Window` or an `App` cannot composite a foreign   |
             surface; `Window::paint_imported_texture` is crate-    |
             internal, under `GpuCanvasContext::paint_texture`.     |
```

Both funnel into the same scene primitive, so the renderer never learns which element
produced it. `Canvas` (the CPU drawing element, `paint_path`/`paint_quad`) is the
sibling that contributes nothing here.

## 3. The payload, and who views it

```
  SurfaceSource                            one value, three cfg-gated payloads
  +----------------------------------------------------------------------+
  |  CoreVideo(CVPixelBuffer)     macOS: an IOSurface-backed buffer       |
  |  DirectX(DirectXSource)       Windows: Texture | View | Shared         |
  |  DmaBuf(DmaBufHandle)         Linux: fd + fourcc + modifier + planes  |
  +----------------------------------------------------------------------+
        |
        |  the renderer samples the source while it composites the frame
        v
  SceneRenderer::draw_surfaces   one arm per backend:
  +----------------------------------------------------------------------+
  |  gpui_windows  Direct3D 11: an SRV; `Shared` opens the NT handle     |
  |  gpui_wgpu     wgpu: dma-buf imported by hand (ash) / view           |
  |  gpui_apple    Metal: a CVMetalTextureCache -> MTLTexture per plane   |
  +----------------------------------------------------------------------+
```

The `DirectXSource` trio spans the device boundary. `Texture` and `View` are **device-bound**:
the texture arm hands the resource and the renderer makes the view, the view arm is for a producer
that is the authority on its own format and mip interpretation, and both must be made on the window
renderer's device — which is why they cannot be built before paint and belong inside a `gpu_canvas`
callback. `Shared` is the **device-independent** arm: an NT handle (plus an optional shared fence)
the renderer opens, views and waits on at draw, so the producer never needs the window's device. It
joins CoreVideo and dma-buf as **a token the renderer resolves**, and it is what makes `surface()`
work on Windows.

## 4. The platform seam, and the renderer-owned device

`gpui_platform` names no graphics type. The **renderer owns its device**, and a trait beside
the seam — not the window — is the door:

```
  +--------------------------------------------------+
  |  GpuRenderer : SceneRenderer        (gpui_engine)|
  |    type Device                                    |
  |    device() -> Option<Self::Device>               |
  +--------------------------------------------------+
                 ^  implemented by the renderers that have a device
                 |
     gpui_windows     gpui_apple        gpui_wgpu
     (ID3D11Device)   (metal::Device)   (Arc<Device>, Arc<Queue>)
     and the headless renderers beside each

     renderers with no device — the authoring TestRenderer,
     the Linux headless discard HeadlessRenderer — do not
     implement it: absence, not a fake device.
```

`GpuCanvasContext::device::<R>()` reaches it, and its fallible twin `try_device::<R>()` is
for a producer that degrades. Both borrow the window's renderer scoped
(`PlatformWindow::with_renderer`) and clone the device out, so the handle is owned and may
outlive the call. Naming `R` *is* the assertion: `device::<R>()` panics only when the
window's renderer is not `R`, and `try_device::<R>()` folds the wrong-renderer case together
with a renderer that has no device. The window has no GPU accessor at all.

The renderer seam is parallel but separate: the renderer's `draw_surfaces` is where the
payload is actually consumed, and `GpuRenderer` is what the canvas downcasts through to hand
a producer the device.

## 5. The cross-device arm: `gpui_interop`

The two elements above cover a producer on the window renderer's *own* device, or a
payload the renderer can already import. A producer on a **different** device goes
through the downstream crate:

```
   producer (another API / adapter / process)
        |
        |  renders into a shared, committed texture; hands an NT handle
        v
  +--------------------------------------------------------------+
  |  gpui_interop                                                 |
  |    attach(device) -> Interop     (the canvas-lent device)     |
  |    Interop::adapter() -> Adapter                              |
  |    Adapter::wgpu()      a wgpu device on the window's adapter |
  |    SharedSurface / Fence   (Direct3D 12 producer half)        |
  +--------------------------------------------------------------+
        |
        |  the opened view becomes SurfaceSource::DirectX(DirectXSource::View)
        v
     surface(..)  /  gpu_canvas(|gpu| gpu.paint_surface(..))   ->  the same scene
```

`gpui_interop` is provisionally in the fork and never depended on by `gpui`; it is the
"other device" row of the surface trinity — *match the adapter, move a handle, order the
queues*.

## 6. What this slice adds on top of upstream

| piece | where | upstream? |
| --- | --- | --- |
| the renderer seam (`SceneRenderer`, `PlatformRenderer`, factory) | `gpui_platform`, `gpui_engine` | fork-carried; not yet proposed |
| `PaintSurface` + `draw_surfaces` | `gpui_engine`, each backend | proposed as PR 2 (Windows arm) |
| `surface()` element | `gpui_authoring` | proposed as PR 2 |
| `SurfaceSource::DirectX(DirectXSource)` (texture, view **and** shared) | `gpui_engine` | the fork is *richer* than PR 2's bare SRV — see [`upstream-prs.md`](upstream-prs.md) §7 |
| `gpu_canvas` + `GpuCanvasContext` | `gpui_authoring` | fork-carried |
| the renderer-owned device (`GpuRenderer`) and its canvas door (`device::<R>()`) | `gpui_engine`, `gpui_authoring` | fork-carried |
| `gpui_interop` (adapter match, handle/fence transport) | its own crate | downstream, never a PR — [`interop-crate.md`](interop-crate.md) |
| the dma-buf (Linux) and CoreVideo (macOS) arms | backends | the Linux arm is PR-adjacent; see [`milestones.md`](milestones.md) |

The ordering and the split are [`upstream-prs.md`](upstream-prs.md) §6; the read of the
fork's build against that plan is §7 there.

## 7. Merge status

```
  origin/bite_v1.23.1-pre            <- the canonical ref the docs anchor citations to
        |                               still has: on_render_surface/on_render_texture,
        |                               Window::paint_imported_texture (pub), the
        |                               *WindowExt paint traits, gpui_wgpu::GpuContext
        |
        *  ... branch bite_v1.23.1-pre-interop (this slice) ...
        |     + surface payload texture/view   + gpui_interop
        |     + gpu_canvas -> GpuCanvasContext + GpuRenderer (renderer-owned device)
        |
        v
  (not merged)                       merge is a separate CI-verified step:
                                     the fork's bite_* CI runs the platform rows,
                                     which no Linux gate can reach.
```

Nothing in this slice is on `bite_v1.23.1-pre` yet. Until the branch merges, the docs
that cite the canonical ref describe the pre-slice shape, and the two are reconciled by
the merge and its CI run, not by re-pointing citations ahead of it.
