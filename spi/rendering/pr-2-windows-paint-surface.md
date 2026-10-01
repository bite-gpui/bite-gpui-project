# PR 2 — the body, ready to open

- **Target:** `zed-industries/zed`, `main`. **Opened:** not yet. **Depends on:** PR 1
  ([`pr-1-pixel-buffer.md`](pr-1-pixel-buffer.md)) only in spirit — the two touch different paths and
  can land in either order.
- **The plan:** [`../upstream-prs.md`](../upstream-prs.md) §3. **Welcome upstream:** the Discussion
  [`#64849`](https://github.com/zed-industries/zed/discussions/64849) — *"i would not have any problem
  with upstreaming support for that since it should be straightforward"*, and *"i would strongly lean
  towards surface sharing here"*.
- **Built in the fork:** `crates/gpui_authoring/src/elements/surface.rs:13` (`SurfaceSource`), `:99`
  (the `corner_radii` stub); `crates/gpui_engine/src/scene.rs:784` (`PaintSurface`);
  `crates/gpui_windows/src/directx_renderer.rs:852` (the `draw_surfaces` stub) and `:862` (the arm it
  takes over from); `crates/gpui_authoring/src/window.rs:3031` (the device accessor, `device_any`).

## Title

`gpui: implement paint_surface on Windows via Direct3D 11 surface sharing`

## Body

### Motivation

`surface()` is GPUI's compositor interface for pixels GPUI did not draw — a video frame, an external 3D
viewport, a webview — and it is **macOS-only** today: `SurfaceSource` has a single `CVImageBuffer`
variant, and `DirectXRenderer::draw_surfaces` is unimplemented. On Windows there is no way to put
externally-produced content into the scene, which is one of the more common reasons to fork.

Per the [earlier discussion](https://github.com/zed-industries/zed/discussions/64849), this is the
**surface-sharing** route — not 11on12, not a second backbuffer. The producer renders offscreen and
hands the renderer a texture view to sample; the renderer composites it into the scene like any other
primitive, with clipping and the rest.

### What this does

- **The element becomes cross-platform.** `SurfaceSource` gains a `DirectX` variant carrying an
  `ID3D11ShaderResourceView`; `PaintSurface` gains the corner radii the element already stubs; and the
  `surface()` constructor and the element's paint path stop being macOS-gated.
- **`DirectXRenderer::draw_surfaces` implements the Windows arm,** reusing the sprite pipeline: it
  binds the SRV to the pixel-shader slot and draws the quad through the existing pipeline (so bounds,
  content mask, radii and the index buffer are the ones the quads already use), samples **straight
  through `_UNORM`** (no sRGB decode/re-encode — the producer's bytes reach the surface unchanged),
  **unbinds the SRV immediately** after the draw so a producer writing into it next frame cannot race
  the bind, and **drops the primitive rather than panicking** when the SRV is invalid.
- **`DirectXWindowExt::d3d11_device()`** exposes the window's `ID3D11Device`, so a producer can
  allocate or open the resource on the device the renderer draws from. Without this the feature is
  unusable, so it is part of this change rather than a follow-up.

### What this deliberately does not do

- **No cross-device handles or fences.** The producer is responsible for the resource being on the
  window's device and for ordering its submission before GPUI's frame; on one device the queue gives
  that for free. Bridging a resource from *another* device — a shared NT handle, a keyed mutex or a
  fence — is left to the caller (or a downstream crate), so nothing here pulls `wgpu` or Direct3D 12
  into gpui.
- **No YCbCr / NV12.** This ships RGBA/BGRA; a two-plane video format is an additive follow-up, not a
  prerequisite.
- **No Direct3D 12, and no 11on12.**

### Synchronization model

GPUI does not manage external fences or mutexes. The caller ensures the producer's submission happens
before GPUI's frame; on the same device and queue, submission order is the ordering. This is the same
rule the macOS path already relies on, and it is what keeps the arm small.

### Testing

A test that composites a known RGBA SRV on a Windows runner (WARP) and reads it back **byte for byte**,
asserting the straight-through round trip; plus the existing suite on the platform. The row mirrors the
macOS surface test.

### Notes

macOS's existing `draw_surfaces` is a YCbCr video path drawn by one renderer. This is the RGBA
counterpart and does not touch it.
