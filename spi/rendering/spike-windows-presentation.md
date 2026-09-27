# Spike: can `WgpuRenderer` present on Windows?

- **Status:** spike, **not run**. The next measurement
  [decision 0002](../../decisions/0002-render-extension-device-model.md) needs, and the gate
  on patch 07 of [`renderer-seam.md`](renderer-seam.md) §6.
- **Answers one question:** can `gpui_wgpu::WgpuRenderer` own a Windows window's
  presentation — and what does the window have to stop doing for it to?
- **Why reading does not settle it.** 0002 item 2 makes Windows Path A and Path B
  conditional on installing `WgpuRenderer`, and every other platform satisfies that by
  construction: Linux *is* wgpu, macOS has Metal on both sides. Windows is the one platform
  where `WgpuRenderer` has never run. `gpui_windows` presents through Direct3D 11 and
  DirectComposition; wgpu would present through its own DX12 swapchain on the same `HWND`.
  Two swapchains and two APIs on one window works or fails for a reason no source states.
- **Inherits:** corrections 1–20,
  [decision 0002](../../decisions/0002-render-extension-device-model.md), and
  [`windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md). macOS and Linux are
  out of scope.

## 1. Four readings that close most of it

- **The renderer does not own the D3D11 device.** `DirectXDevices`
  (`crates/gpui_windows/src/platform.rs:84`) is created once at platform init
  (`crates/gpui_windows/src/platform.rs:167`) and threaded into every window
  (`crates/gpui_windows/src/platform.rs:298`). Patch 07 replaces the renderer, not the
  device; the device stays.
- **The device's other consumer is text, and it hands the renderer bytes, not a texture.**
  `DirectWrite`'s `GPUState` holds the same `ID3D11Device` and immediate context
  (`crates/gpui_windows/src/direct_write.rs:68`, `:184`), and that immediate context is
  documented as shared with `DirectXRenderer` and `DirectXAtlas` on the UI thread
  (`crates/gpui_windows/src/direct_write.rs:888`). Glyph rasterization draws on that device
  and **reads the result back to CPU** (`crates/gpui_windows/src/direct_write.rs:1176`), so
  glyphs reach the atlas as bytes, not as a shared texture. **This is what keeps Outcome B
  alive:** a DX12 renderer does not break text, because text never crosses to the renderer
  as a GPU resource. Had it crossed, the DX11/DX12 split would have closed text as well, and
  Windows would have had no configuration at all.
- **`gpui_wgpu` is already surface-agnostic.** `WgpuRenderer::new` takes any
  `HasWindowHandle + HasDisplayHandle` (`crates/gpui_wgpu/src/wgpu_renderer.rs:268`) and
  builds an untyped `RawHandle` surface from it
  (`crates/gpui_wgpu/src/wgpu_renderer.rs:281`). A shim around the `HWND`, of the same shape
  as the private `RawWindow` the Wayland window already carries
  (`crates/gpui_linux/src/linux/wayland/window.rs:64`), is the whole requirement.
- **The factory already hands over what the shim needs.** `RendererTarget` carries the
  `RawWindowHandle` ([`renderer-seam.md`](renderer-seam.md) §5.3), which on Windows is the
  `Win32WindowHandle` around the `HWND`.

What that leaves is presentation and the machinery around it. `WindowsWindowState::new`
builds a `DirectXRenderer` (`crates/gpui_windows/src/window.rs:145`) that owns an
`IDXGISwapChain1` bound either to the `HWND` or to a DirectComposition visual
(`crates/gpui_windows/src/directx_renderer.rs:895`), presents `Present(0, ..)`
(`crates/gpui_windows/src/directx_renderer.rs:265`), and targets `B8G8R8A8_UNORM`
(`crates/gpui_windows/src/directx_renderer.rs:32`).

## 2. The probes

Run on `windows-latest`, the way
[`windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) probe 2 was, in a new
standalone crate beside `probes/windows-path-a` (its own `[workspace]`, so it costs the main
build nothing). A probe is added while it is answering and taken out when it has.

| # | probe | answers |
| --- | --- | --- |
| 1 | create an `HWND` with `CreateWindowExW`, build a wgpu instance, adapter and device, `create_surface_unsafe` over `RawWindowHandle::Win32`, `configure`, clear, submit, `present()`; print the adapter, the backend and `surface.get_capabilities()` | whether wgpu can present on a Windows `HWND` at all, and with which formats, present modes and alpha modes |
| 2 | bind an `IDCompositionTarget`/`IDCompositionVisual` to the same `HWND` — GPUI's path when DirectComposition is on — and present from wgpu on it | whether the window must *skip* its swapchain and its composition tree for wgpu to be the only presenter, which is the part of `WindowsWindowState::new` patch 07 has to make conditional |
| 3 | present a frame with non-opaque alpha on a window GPUI would make transparent | whether per-pixel alpha survives without DirectComposition, or whether transparency is a documented gap under `WgpuRenderer` |
| 4 | read a fixture back through the wgpu surface and compare it with the same fixture composited through `DirectXRenderer` | the colour-space question [`foreign-texture.md`](foreign-texture.md) §4 leaves open — sRGB or plain `UNORM` — now with a device to answer it |
| 5 | resize (`update_drawable_size` → reconfigure) and drive the recovery protocol ([`renderer-seam.md`](renderer-seam.md) §5.6) on the wgpu surface | that resize and device loss, which the default renderer handles in this backend, hold for a factory-installed one |

Probes 1 and 2 are the decisive pair; 3–5 are consequences patch 07 has to carry, and each is
cheap once 1 has a window.

## 3. The outcomes

- **Yes.** Patch 07 is viable, and the window makes its swapchain and composition tree
  conditional on no renderer factory being installed. The D3D11 device stays, for text, and
  the only new question is whatever probe 3 turns up.
- **Partly.** Presentation works but a consequence does not — transparency is the likely one
  — and it is recorded as a limitation of the Windows configuration rather than a defect.
- **No.** Windows Path A and Path B are unavailable and 0002 item 2 is wrong: the factory
  still installs a renderer, but on Windows it cannot be `WgpuRenderer`, so there is no
  device the producer and the consumer can both be on. That reopens the decision rather than
  changing a patch.

## 4. Hazards

- **WARP.** A hosted runner has no GPU, so probe 2's caveat carries over exactly: what is
  measured is one adapter — the software one — and hardware is expected to behave the same
  without being what was measured.
- **A hidden window.** `Present` on a window that is not shown can return
  `DXGI_STATUS_OCCLUDED`; that is not a failure, and the probe has to either treat it as
  success or show the window.
- **A message pump.** A window with no pump is still valid for surface creation and
  presentation, but nothing posted to it runs; if the probe needs a pump, that is itself a
  finding for patch 07.
- **Two swapchains, one `HWND`.** Probe 2 is the one that can fail for a reason the source
  does not state, which is why it is second rather than last.

## 5. What this does not decide

- **The end-to-end configuration.** That a pushed texture composites through a
  `WgpuRenderer`-backed *GPUI window* is [`verification.md`](verification.md) §1's "Outcome B
  end to end" row. It needs patch 07 first, so it is that patch's acceptance test rather than
  a question this spike can ask.
- **Whether `DirectXRenderer` should be reimplemented over wgpu**, retiring Direct3D 11
  entirely. That is 0002's "should wgpu be the default on Windows" question; this spike only
  tells us whether wgpu *could* be.
- **macOS and Linux.** Unchanged — macOS has Metal on both sides, and on Linux wgpu is
  already the renderer.
