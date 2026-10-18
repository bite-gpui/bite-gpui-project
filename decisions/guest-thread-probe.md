# The guest thread probe: an offscreen frame crosses a thread as a surface

- **Opened:** 2026-10-18
- **Status:** closed — measured on both GPUs; the offscreen contract gained `offscreen_surface_handle`.
- **Evidence for:** [`0005`](0005-external-rendering-unifies-under-surface.md) §4 (guest mode) and
  [`../spi/rendering/probe-p7-guest-thread.md`](../spi/rendering/probe-p7-guest-thread.md) (P7).
- **Touches:** `gpui_engine/src/renderer.rs` (`offscreen_surface_handle`),
  `gpui_wgpu/src/wgpu_renderer.rs` (`WgpuOffscreenSurface`, the `!Send` row).

## The question

Can a headless GPUI frame be produced on a worker thread and sampled by a host loop — across the
`!Send` renderer boundary — as a **surface**, not only as bytes?

## What was measured

A worker thread builds its own `WgpuHeadlessRenderer`. The `!Send` renderer never crosses; it
renders a scene, reads the pixels, and sends
`SceneRenderer::offscreen_surface_handle()` — an `Arc<dyn Any + Send + Sync>` — to the host:

1. **Spawn.** The headless context and its executors are built and used entirely on the worker
   thread. Nothing about the renderer is reached from the host.
2. **Render.** A scene renders offscreen, exactly as a window's frame would.
3. **The crossing, both ways.** The CPU path (`read_pixels`) and the zero-copy path (the handle)
   arrive together. The host asserts the CPU pixel, then downcasts the handle to
   `WgpuOffscreenSurface` and samples its texture.
4. **The boundary.** The host reaches the same device through a second headless renderer — the
   headless context is process-wide, so it is another handle on the device, not a second one — copies
   the texture to a buffer, and asserts the same red pixel the CPU path returned.
5. **Ordering.** The guest's readback waits for its submission before it sends, so the host samples a
   completed frame; the guest keeps the renderer and target alive until the host answers on a second
   channel.

Measured on the Intel HD 520 (`ZED_DEVICE_ID=1916`) and the NVIDIA 930M (`1346`): both pass.

## What the contract lacked, and now has

`SceneRenderer` exposed only bytes (`render_scene`, `read_pixels`). It now carries:

```rust
fn offscreen_surface_handle(&self) -> Option<Arc<dyn Any + Send + Sync>> { None }
```

`WgpuHeadlessRenderer` returns a `WgpuOffscreenSurface` wrapping an `Arc<wgpu::Texture>` and the
frame's size. The engine still names no graphics API: the host that samples is a wgpu client and
downcasts to its own type. `ObservingRenderer` forwards the handle to its inner renderer, so wrapping
a renderer does not hide the surface.

## What it means for W6

The guest's GPU path is buildable as the design assumed: the renderer stays pinned to its thread, the
handle crosses, and the host samples it. What is still W6's work is the runner around it — the
executors' cadence, the command channel, and input translation in — and what is still unmeasured is
the reverse transport (P1). Cross-device sampling is not this probe: the host uses the same device,
which is the guest contract's rule.

## What was built afterwards

The runner is `gpui_interop::Guest` (`src/guest.rs`, feature `guest`). It opens one headless window
on a worker thread, drives `Window::draw` + `Window::present` when the host asks, and wraps the
window's renderer in a tap that reports `offscreen_surface_handle` after every presented frame. The
host's `request_frame` blocks until that frame arrives, and `Guest::update` posts closures back to
the guest thread, in order.

Three seams had to exist for it, and are now part of the core:

- `Window::present` is a public, doc-hidden runtime method beside `Window::draw` — the platform's
  frame callback is not available to a host that owns its own loop.
- The Linux headless window honours `WindowParams::renderer_factory` (it previously ignored it and
  kept its discard renderer), and sizes the renderer's viewport in `present`.
- `WgpuHeadlessRenderer` implements `PlatformRenderer`, answering the texture limit and adapter
description from the device it already owns.

The rows are `tests/guest_runner.rs` (feature `guest-test`): the host asks for a frame and samples a
known red pixel on the same device; a resize changes the frame's size; an update posted before a
request is visible in that frame; and a renderer with no shareable surface is a named refusal rather
than a hang. Both rows pass on the Intel HD 520 and the NVIDIA 930M. What remains is the input
adapter over `Guest::update`, and P1 for the Windows reverse transport.
