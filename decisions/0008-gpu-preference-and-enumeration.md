# 0008 — The GPU is process-wide, chosen before the first window from an application preference

- **Decided:** 2026-10-17
- **Status:** decided — steps 1 and 2 implemented on the working tree: the types (`GpuPreference`,
  `GpuId`, `GpuAdapterInfo`), `Application::with_gpu_preference`, the `Platform` hook, the Linux
  plumbing, the `ZED_DEVICE_ID` lowering and a unit-tested ranking; `App::enumerate_gpus` /
  `App::active_gpu` and the `Custom` variant. Capability negotiation (step 3) is not.
- **Evidence:** [`../issues/0010-application-gpu-preference.md`](../issues/0010-application-gpu-preference.md)
  (the preference) and [`../issues/0012-gpu-enumeration-and-negotiation.md`](../issues/0012-gpu-enumeration-and-negotiation.md)
  (enumeration and the device scope); the measured pair in
  [`optimus-dmabuf-topologies.md`](optimus-dmabuf-topologies.md); the device rules in
  [`0006-renderer-owned-device.md`](0006-renderer-owned-device.md) and
  [`0002-render-extension-device-model.md`](0002-render-extension-device-model.md).
- **Touches:** `gpui_platform/src/gpu.rs`, `gpui_platform/src/platform.rs`,
  `gpui_runtime/src/application.rs`, `gpui_linux/src/linux/…` (Wayland and X11),
  `gpui_wgpu/src/wgpu_context.rs`, `gpui_wgpu/src/wgpu_renderer.rs`.

## Decision

1. **One GPU per process, chosen once.** The device is already process-wide (`WgpuContext` is shared by
   every window, [`0006`](0006-renderer-owned-device.md)), so the preference is **application-level**,
   not a per-window argument. It is applied where the adapter is selected —
   `select_adapter_and_device` (`crates/gpui_wgpu/src/wgpu_context.rs:589`) — before the first window makes a surface
   current.
2. **The application states a `GpuPreference`.** `Application::with_gpu_preference(…, GpuPreference)`,
   carried on the platform and read at selection. The variants:
   - `Auto` — today's ranking: compositor hint, then device type, then backend;
   - `LowPower` — the integrated adapter when there is one;
   - `HighPerformance` — the discrete adapter when there is one;
   - `Device(GpuId)` — a named adapter, matched by PCI vendor/device;
   - `Custom(Arc<dyn Fn(&[GpuAdapterInfo]) -> Option<usize> + Send + Sync>)` — the closure receives
     the enumerated list and returns the index of the one adapter it wants. It **chooses** a GPU, it
     does not rank them; `None`, or an index outside the list, fails the window with the shortfall
     named rather than silently picking another. The chosen adapter is tried first and the rest keep
     `Auto`'s order as fallbacks.
3. **`ZED_DEVICE_ID` becomes `Device`, not a separate tier.** The variable is read in the Linux
   (`gpu_preference_from_env`, `crates/gpui_linux/src/linux/platform.rs:158`) and lowered
   onto `GpuPreference::Device`; no crate in the renderer, and nothing in `gpui_platform`, reads the
   environment. One precedence order then lives in one place. The **headless** path has no platform
   instance, so `current_headless_renderer` asks the platform crate for the same value
   (`gpui_linux::gpu_preference_from_env`), while `current_headless_renderer_with` takes an explicit
   preference. The env var names a device id only, so the lowered `GpuId` leaves the vendor
   unconstrained.
4. **The default is `LowPower`.** On a hybrid laptop the panel is wired to the integrated GPU, so a
   renderer on the discrete one keeps it powered for the window's lifetime and crosses PCIe every
   frame to be scanned out. A discrete GPU reached as a *producer* pays neither and can be dropped.
   `Auto` remains available for a caller that wants today's ranking.
5. **Enumeration is a synchronous accessor on the app, and capability is a second tier.**
   - `App::enumerate_gpus() -> Vec<GpuAdapterInfo>` and `App::active_gpu() -> Option<GpuAdapterInfo>`,
     implemented by the platform (a default empty list for a backend that cannot).
   - `GpuAdapterInfo` carries what a choice needs: `name`, `device_type`, `vendor_id`, `device_id`,
     `backend`, and whether the adapter is software. It deliberately does **not** carry
     `wgpu::Features` or a dma-buf format/modifier list yet: both need a device, and a capability probe
     creates one. Proving what a device will **import** is the interop crate's question
     (`GpuDevice::linear_layout`, `SharedBuffer`), and belongs beside it rather than in the adapter
     list.
   - `GpuId` here is the selection identity — PCI `vendor_id` (optional) and `device_id`. The
     producer-facing identity (name + DRM render node) stays `gpui_interop::GpuId`; mapping one to the
     other is part of the enumeration slice.
6. **Runtime switching is refused for now, and the refusal is recorded.** Changing the renderer's GPU
   means destroying the `VkDevice`, closing `/dev/dri/*`, and recreating every render target — a
   multi-second, screen-blanking operation that device-lost recovery does not model (it *recreates* a
   device, it does not tear one down). An application that must change GPU restarts.

## Why

The measured topology makes the choice load-bearing rather than cosmetic
([`optimus-dmabuf-topologies.md`](optimus-dmabuf-topologies.md)): an iGPU-exported dma-buf is refused
on the dGPU on every memory type, so the producer and the renderer must be the same card; the panel is
wired to the iGPU; and a `GM108` discrete part has no decoder at all. Today the only control is a
debugger's environment variable read deep in the renderer, and the chosen GPU is observable only after
a window exists (`crates/gpui_wgpu/src/wgpu_renderer.rs:1255`, and only inside a `gpu_canvas` through
`Adapter::gpu_id`, [`../issues/0012-gpu-enumeration-and-negotiation.md`](../issues/0012-gpu-enumeration-and-negotiation.md)), so an
application cannot decide, cannot ask, and cannot change its mind.

## Rejected

- **A per-window device.** It would duplicate the instance, adapter, device and queue per window, and
  the surface/swapchain would follow the window rather than the process. The seam that owns a device is
  the renderer; the scope that owns the *choice* is the application.
- **Keeping `ZED_DEVICE_ID` as a renderer-internal tier.** Two precedence orders then exist, and a
  debug variable can silently disagree with an application's stated preference.
- **Selecting by adapter name** rather than PCI identity. Names are driver strings, not identities,
  and the producer already matches on vendor/device or the DRM node.
- **A capability list on `GpuAdapterInfo` now.** It forces a device at enumeration time, which is the
  cost enumeration exists to avoid; and "what will this device import" is answered by the interop
  crate on a device it already has.
- **Runtime switching in this round.** No caller has asked for it, and the operation is user-visible
  enough that its contract (blanking, failure recovery, in-flight surfaces) deserves its own record.

## Consequences

- **A behaviour change on hybrid laptops:** the default renderer moves from discrete (today's tie-break)
  to integrated. That is the intended fix, and `Auto`/`HighPerformance` keep the old answer reachable.
- **The platform gains one optional method** (`set_gpu_preference`, default no-op), so macOS and Windows
  compile unchanged; their renderers do not take a `wgpu` preference yet, and say so.
- **The headless path takes a preference, or asks the platform for one.** With no platform instance,
  `current_headless_renderer` calls the platform crate's `gpu_preference_from_env`, so the debug
  override still applies where it did; `current_headless_renderer_with` takes an explicit preference.
  Without either, the default is `Auto`. The context is cached, so the first caller decides.
- **The device remains process-wide**, so `Custom` and `Device` are resolved once; a per-viewport
  producer is a different seam (0012 §"scope of the device").

## Plan

1. **Types and plumbing.** `GpuPreference`, `GpuId` and `GpuAdapterInfo` in `gpui_platform/src/gpu.rs`;
   `Platform::set_gpu_preference` (default no-op); `Application::with_gpu_preference`; thread it through
   the Linux clients beside `CompositorGpuHint` to `WgpuRenderer::new` → `WgpuContext` → the ranking;
   lower `ZED_DEVICE_ID` in the Linux platform; default `LowPower`.
2. **Enumeration and the custom choice.** `App::enumerate_gpus` / `App::active_gpu`;
   `GpuAdapterInfo` mapped out of `wgpu::AdapterInfo` on Linux; `Custom` resolved against the
   enumerated list before ranking; the mapping to `gpui_interop::GpuId`
   (`GpuId::from_adapter`) beside the producer-facing identity. The Windows and macOS arms, which
   need a different identity and a device the platform has already created, are designed in
   [`0009`](0009-gpu-preference-platform-backends.md) (draft).
3. **Capability negotiation.** What a chosen device will import/export, asked on a device the caller
   already has — the interop crate's report, surfaced through the application seam.
