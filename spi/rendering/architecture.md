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
  |    Window::device_any       -> PlatformWindow::gpu_window()              |
  +---------------------------------------------------------------------------+
        |
        v
  +---------------------------------------------------------------------------+
  |  gpui_engine                                                             |
  |    scene::Scene { surfaces: Vec<PaintSurface> }                          |
  |    scene::SurfaceSource { CoreVideo | DirectX | DmaBuf }   (cfg-gated)   |
  |    SceneRenderer::draw_surfaces(&[PaintSurface])                         |
  +---------------------------------------------------------------------------+
        |
        v
  +---------------------------------------------------------------------------+
  |  gpui_platform                                                           |
  |    PlatformWindow   the window surface                                   |
  |    GpuWindow        the GPU decorator over it (device access)            |
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
  |    bounds()  device_any()  cx()                               |
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
  |  DirectX(DirectXSource)       Windows: Texture(..) | View(..)         |
  |  DmaBuf(DmaBufHandle)         Linux: fd + fourcc + modifier + planes  |
  +----------------------------------------------------------------------+
        |
        |  the renderer samples the source while it composites the frame
        v
  SceneRenderer::draw_surfaces   one arm per backend:
  +----------------------------------------------------------------------+
  |  gpui_windows  Direct3D 11: an SRV; the `Texture` arm makes the view  |
  |  gpui_wgpu     wgpu: dma-buf imported by hand (ash) / view           |
  |  gpui_apple    Metal: a CVMetalTextureCache -> MTLTexture per plane   |
  +----------------------------------------------------------------------+
```

The `DirectXSource` pair is the ergonomic/escape shape: the **texture** arm hands the
resource and the renderer makes the view; the **view** arm is for a producer that is the
authority on its own format and mip interpretation.

## 4. The platform seam, and the decorator

`gpui_platform` names no graphics type. A backend window lends its GPU capabilities
through a **decorator**, not by widening the general trait:

```
  +---------------------------------------------+
  |  PlatformWindow        the window            |
  |    ... layout, input, presentation ...       |
  |    gpu_window() -> Option<&dyn GpuWindow>    |   <-- the only GPU door
  +---------------------------------------------+
                 |
                 v
  +---------------------------------------------+
  |  GpuWindow             the decorator         |
  |    device_any() -> Option<Rc<dyn Any>>       |
  +---------------------------------------------+
                 ^  implemented by exactly the backends that have a device
                 |
     gpui_windows   gpui_macos   gpui_linux (wayland, x11)
     (D3D11 device) (MTLDevice)  (wgpu context slot)
```

`Window::device_any` reads it; `GpuCanvasContext::device_any` reads that. A backend that
has no device keeps the defaults and answers nothing, and a *new* GPU operation lands on
`GpuWindow` with a default so only the backends that support it change.

The renderer seam is parallel but separate: `PlatformRenderer::device_any` is what the
renderer's own host holds, and the renderer's `draw_surfaces` is where the payload is
actually consumed.

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
  |    attach(window) -> Interop                                  |
  |    Interop::adapter() -> Adapter                              |
  |    Adapter::wgpu()      a wgpu device on the window's adapter |
  |    SharedSurface / OpenedSurface / Fence   (Direct3D 12 -> 11)|
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
| `SurfaceSource::DirectX(DirectXSource)` (texture **and** view) | `gpui_engine` | the fork is *richer* than PR 2's bare SRV — see [`upstream-prs.md`](upstream-prs.md) §7 |
| `gpu_canvas` + `GpuCanvasContext` | `gpui_authoring` | fork-carried |
| the `GpuWindow` decorator | `gpui_platform` | fork-carried |
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
        |     + gpu_canvas -> GpuCanvasContext + GpuWindow decorator
        |
        v
  (not merged)                       merge is a separate CI-verified step:
                                     the fork's bite_* CI runs the platform rows,
                                     which no Linux gate can reach.
```

Nothing in this slice is on `bite_v1.23.1-pre` yet. Until the branch merges, the docs
that cite the canonical ref describe the pre-slice shape, and the two are reconciled by
the merge and its CI run, not by re-pointing citations ahead of it.
