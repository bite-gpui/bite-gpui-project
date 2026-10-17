- **Opened:** 2026-10-16
- **Status:** open
- **Touches:** `bite-gpui/crates/gpui_wgpu/src/wgpu_context.rs`, `bite-gpui/crates/gpui_wgpu/src/wgpu_renderer.rs`, `bite-gpui/crates/gpui_linux/src/linux/platform.rs`, the renderer factory in `bite-gpui/crates/gpui_engine/src/renderer.rs`, `spi/rendering/`

# No GPU enumeration, capability negotiation, or renderer switch before a window exists

## What is wrong

The chosen GPU — and anything about what it can do — is observable only *after* a window exists:

- `window.gpu_specs()` (`crates/gpui_wgpu/src/wgpu_renderer.rs:1236`) — a name, a device type and
  driver strings, after the
  window, and nothing about what the device can *do*;
- `Adapter::gpu_id` (`gpui_interop`'s `adapter.rs`) — inside a `gpu_canvas`,
  which is to say during a frame, with the device already built.

There is no accessor for the adapter list, no way to ask what a device will import or export, and no way
to change the answer once given. An application therefore cannot decide whether it can do zero-copy
interop until it has paid for a window and a device, and cannot then change its mind except by quitting
and setting an environment variable (0010). A preferences panel that lists adapters, and a config that
remembers one, has nothing to read from.

Read against 0010: that issue asks for a way to *state* a preference; this one asks for a way to *know
what to prefer*, and for what happens when the answer must change.

## Two shapes, in order of preference

1. **Enumerate and negotiate before the renderer is created.** `cx.enumerate_gpus() -> Vec<GpuAdapterInfo>`
   — name, device type, PCI/DRM identity, `wgpu::Features`, and the dma-buf formats and modifiers the
   device would import and export — plus `cx.active_gpu()`. This is what makes `GpuPreference::Custom`
   (0010) expressible as "choose the adapter that satisfies my requirement" rather than a guess, and what
   a preferences panel populates from.
2. **Change the renderer's GPU on the fly.** This requires destroying the `VkDevice` and closing
   `/dev/dri/*`: a multi-second, screen-blanking operation. Device-lost recovery is the nearest existing
   machinery, but it *recreates* a device rather than tearing one down, so it is not the same mechanism.

## The scope of the device — the design question underneath

The GPU device is **process-wide, not per window** (`WgpuContext`), while a window is per window. So
"which GPU" is decided once, for the process, at the first window — which is why a per-viewport choice
needs a different seam from a per-window one.

The shape this round suggests: `WaylandClient`/`X11Client` own a single `WgpuContext` (instance, adapter,
device, queue) and hand a clone into each window's `WgpuRenderer`, so nothing is duplicated; the
*preference* is then configuration on `Application`/`PlatformOptions`, not a per-window argument.
Adapter creation is already lazy — `select_adapter_and_device`
(`crates/gpui_wgpu/src/wgpu_context.rs:604`) verifies a
compatible surface before choosing — which is why the preference must reach that point rather than the
renderer's construction.

## Who picks the GPU

1. **The OS/DE** — the compositor hint (`Compositor GPU hint`,
   `crates/gpui_linux/src/linux/platform.rs:1313`), tier 2 of `sort_adapters`
   (`crates/gpui_wgpu/src/wgpu_context.rs:690`) today.
2. **The application, at startup** — a preference, possibly derived from capabilities (shape 1).
3. **At runtime** — the expensive one, and per the scope above not a per-window operation.

## The practical shape this argues for

Render on the **integrated** GPU and use a discrete GPU only as a **producer**. On a hybrid laptop the
display is wired to the iGPU, so a renderer on the dGPU costs twice: holding the `VkDevice` open keeps
the dGPU powered for as long as the window exists, and every UI frame it renders crosses PCIe back to
the iGPU to be scanned out. A producer that renders on its own device and hands over a dma-buf pays
neither, and can be dropped — releasing the device, letting the dGPU sleep — when the viewport that
needed it closes.

That argues for `GpuPreference::LowPower` as the default, and for reaching a dGPU as a *producer* through
`gpui_interop` rather than as the window renderer. It is also why 0011 is a prerequisite for that path:
the linear cross-device buffer is exactly the one that mis-renders today. The three paths — what stays
tiled, what must be linear, and which of them fails today — are measured in
[`../decisions/optimus-dmabuf-topologies.md`](../decisions/optimus-dmabuf-topologies.md).

## What would close it

The enumerate and active accessors and the factory plumbing they need; a decision on runtime switching,
or its explicit refusal recorded; and the process-wide-versus-per-window device ruling written down.
