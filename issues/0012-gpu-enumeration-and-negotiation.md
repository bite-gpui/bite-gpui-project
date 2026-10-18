- **Opened:** 2026-10-16
- **Status:** closed
- **Touches:** `bite-gpui/crates/gpui_wgpu/src/wgpu_context.rs`, `bite-gpui/crates/gpui_wgpu/src/wgpu_renderer.rs`, `bite-gpui/crates/gpui_linux/src/linux/platform.rs`, the renderer factory in `bite-gpui/crates/gpui_engine/src/renderer.rs`, `spi/rendering/`

# No GPU enumeration, capability negotiation, or renderer switch before a window exists

## What is wrong

The chosen GPU — and anything about what it can do — is observable only *after* a window exists:

- `window.gpu_specs()` (`crates/gpui_wgpu/src/wgpu_renderer.rs:1257`) — a name, a device type and
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
(`crates/gpui_wgpu/src/wgpu_context.rs:708`) verifies a
compatible surface before choosing — which is why the preference must reach that point rather than the
renderer's construction.

## Who picks the GPU

1. **The OS/DE** — the compositor hint (`Compositor GPU hint`,
   `crates/gpui_linux/src/linux/platform.rs:1297`), tier 2 of `sort_adapters`
   (`crates/gpui_wgpu/src/wgpu_context.rs:679`) today.
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

## Update, 2026-10-17 — the rulings are recorded, and enumeration lands

[`../decisions/0008-gpu-preference-and-enumeration.md`](../decisions/0008-gpu-preference-and-enumeration.md)
settles the device scope (process-wide, chosen once), the preference API, the default (`LowPower`) and
the refusal of runtime switching. Step 1 landed with it: `GpuPreference`, `GpuId` and `GpuAdapterInfo`
on `gpui_platform`; `Application::with_gpu_preference`; the `Platform` hook; the Linux plumbing beside
`CompositorGpuHint`; and `ZED_DEVICE_ID` lowered to `Device` in the platform, with the ranking
unit-tested.

Step 2 landed next:

- `App::enumerate_gpus()` returns the adapters the process could render on, and `App::active_gpu()`
  the one the renderer actually chose (both `None`/empty until a platform and, for the active one, a
  window exist). No device is created to answer either.
- `GpuPreference::Custom` now receives the whole list and returns the index of **one** adapter. A
  selector that names nothing, or an index past the list, fails the window naming the shortfall; a
  chosen adapter the display cannot be configured on falls through to `Auto`'s order.
- `GpuAdapterInfo` is mapped from `wgpu::AdapterInfo` in `gpui_wgpu`, so the renderer's type stops at
  its boundary; the Linux clients enumerate from the shared context's instance once it exists.
- `gpui_interop::GpuId::from_adapter` maps the same adapter onto the producer-facing identity (name
  and DRM render node), and `matches_adapter` is the name join a `Custom` selector can use.

What this issue still owed after step 2 was the capability tier `0008` defers: what a chosen device
will import or export, asked on a device the caller already has.

## Update, 2026-10-18 — the capability tier lands, and the issue closes

The import direction is implemented on `bite_v1.23.x` (`fcbab02913`):

- `DmaBufCapabilities` (in `gpui_engine`, beside the surface-format vocabulary) is a per-format list
  of the DRM modifiers the importer will bind. `supports(format, modifier)` is the question a
  producer asks; `modifiers(format)` is the list it hands an allocator.
- `gpui_wgpu::dmabuf_capabilities(device)` answers from the renderer's own device with
  `vkGetPhysicalDeviceFormatProperties`: no device is created and no memory is allocated. The
  formats are the importer's closed set (`Bgra8`, `Rgba8`, `Nv12`, `Nv21`); linear appears when the
  format's linear tiling samples, and driver modifiers only when the device took
  `VK_EXT_image_drm_format_modifier` (otherwise the importer refuses tiled buffers).
- `Platform::dmabuf_capabilities` and `App::dmabuf_capabilities` surface it on the application seam,
  `None` before the first window and on Windows/macOS, whose transports are not dma-bufs.
- Measured on the pair: the Intel HD 520 reports `X_TILED`, `Y_TILED` and `Y_TILED_CCS` for all four
  formats; the NVIDIA 930M reports six block-linear modifiers for the packed RGB pair and only
  `LINEAR` for `NV12`/`NV21`. The suite asserts the closed set and linear support on both, and the
  driver's list is a shape check, not a hardware assertion.

The remaining direction, **export**, is the producer's own device: `GpuDevice::linear_layout`
answers the native linear layout and `SharedBuffer` reports the modifier it exported under, both on
`gpui_interop`. A platform-level export report would duplicate the producer's own query, so it is
not part of this seam.

What stays open is out of this issue's scope: Windows/macOS import capability reports (shared
D3D11 texture / IOSurface, not modifiers), runtime switching (refused by `0008`), and the
producer-facing convenience of asking the same question through `gpui_interop::Adapter` instead of
carrying the report from `App`.
