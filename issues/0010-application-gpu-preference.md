- **Opened:** 2026-10-16
- **Status:** closed
- **Touches:** `bite-gpui/crates/gpui_wgpu/src/wgpu_context.rs`, `bite-gpui/crates/gpui_linux/src/linux/platform.rs`, the renderer factory in `bite-gpui/crates/gpui_engine/src/renderer.rs`, `gpui-interop/crates/gpui_interop/src/linux.rs`

# An application cannot choose the GPU; only `ZED_DEVICE_ID` can

## What is wrong

Adapter selection is a fixed ranking inside the renderer — `sort_adapters`
(`crates/gpui_wgpu/src/wgpu_context.rs:679`), whose ordering `adapter_key` documents
(`crates/gpui_wgpu/src/wgpu_context.rs:836`):

1. `ZED_DEVICE_ID` match — explicit override
2. Compositor GPU match — the GPU the display server renders on
3. Device type (discrete > integrated > other > virtual > cpu)
4. Backend

The only application-facing control is tier 1, and it was a **`std::env::var` read** — `device_id_filter`
— deep inside the renderer's context, not an argument. (After `0008` the one read is in the Linux
platform, `gpu_preference_from_env`, `crates/gpui_linux/src/linux/platform.rs:163`.) So:

- an application that knows which GPU it wants can only *be launched with an environment variable*, a
  shell-level accident rather than an application decision, and one the program itself cannot see;
- an application cannot express a *predicate* over adapters at all ("one that can export a dma-buf",
  "the one my producer's device matches") — only a raw PCI id, and only if it already knows it;
- the variable is a debugger's (it is named for an editor's builds) doing duty as a product API.

Nothing on `Application` or `PlatformOptions` reaches this. The chosen GPU is readable only *after* a
window exists — `window.gpu_specs()` (`crates/gpui_wgpu/src/wgpu_renderer.rs:1257`), or inside a
`gpu_canvas` through `Adapter::gpu_id` (`gpui_interop`'s `adapter.rs`) — which is 0012's subject.

## Proposal

A `GpuPreference` carried on `Application`/`PlatformOptions` and consulted where the adapter is actually
chosen (`select_adapter_and_device`, `crates/gpui_wgpu/src/wgpu_context.rs:589`):

```rust
pub enum GpuPreference {
    Auto,             // today's ranking: compositor hint, then device type, then backend
    LowPower,         // integrated
    HighPerformance,  // discrete
    Device(GpuId),
    Custom(Arc<dyn Fn(&AdapterInfo) -> Option<usize> + Send + Sync>),
}

Application::new().with_gpu_preference(GpuPreference::LowPower).run(…);
```

The accessors a preferences panel needs to populate `Device(GpuId)` — `cx.enumerate_gpus()`,
`cx.active_gpu()` — are 0012's, and this variant is unusable without them.

**Reroute `ZED_DEVICE_ID` onto it.** Parse the variable in the platform, beside the compositor hint
(`crates/gpui_linux/src/linux/platform.rs:1297`), and lower it to `GpuPreference::Device`, so the
environment
variable and the API become one mechanism. Two things follow: a debug variable can no longer silently
disagree with an application's stated preference, and precedence is decided in one place instead of by
a tier ordering buried in the renderer.

## What would close it

`GpuPreference` on `Application`/`PlatformOptions`, plumbed to `select_adapter_and_device`;
`ZED_DEVICE_ID` implemented as `GpuPreference::Device` rather than an `env::var` read; and the default
decided — see 0012 for why it should be `LowPower`.

## Closed, 2026-10-17 — the preference is an application setting

[`../decisions/0008-gpu-preference-and-enumeration.md`](../decisions/0008-gpu-preference-and-enumeration.md)
settles the shape. The device is process-wide and chosen once, before the first window;
`GpuPreference` (`Auto`, `LowPower`, `HighPerformance`, `Device`) is stated with
`Application::with_gpu_preference`, carried on the platform and read where the adapter is chosen;
`ZED_DEVICE_ID` is lowered to `Device` in the platform (the headless path asks the platform crate
for the same value, because it has no platform instance); and the default is `LowPower`, with `Auto` preserving the old
ranking. `Custom` waited for enumeration, which is `0012`'s half and has now landed: it receives the
enumerated list and names one adapter, rather than ranking them.

## Caveat

`GpuId` does not exist in tree. `gpui_interop::GpuId` (`gpui_interop`'s `linux.rs`)
is the only one, and it is out of tree, carrying a device name and a DRM render node — the two things a
producer matches on. A `Device(GpuId)` variant either adopts that type or the tree grows its own.
