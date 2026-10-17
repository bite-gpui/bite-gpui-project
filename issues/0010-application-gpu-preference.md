- **Opened:** 2026-10-16
- **Status:** open
- **Touches:** `bite-gpui/crates/gpui_wgpu/src/wgpu_context.rs`, `bite-gpui/crates/gpui_linux/src/linux/platform.rs`, the renderer factory in `bite-gpui/crates/gpui_engine/src/renderer.rs`, `gpui-interop/crates/gpui_interop/src/linux.rs`

# An application cannot choose the GPU; only `ZED_DEVICE_ID` can

## What is wrong

Adapter selection is a fixed ranking inside the renderer — `sort_adapters`
(`crates/gpui_wgpu/src/wgpu_context.rs:695`), whose tiers are documented at
`crates/gpui_wgpu/src/wgpu_context.rs:687`:

1. `ZED_DEVICE_ID` match — explicit override
2. Compositor GPU match — the GPU the display server renders on
3. Device type (discrete > integrated > other > virtual > cpu)
4. Backend

The only application-facing control is tier 1, and it is a **`std::env::var` read** — `device_id_filter`
(`crates/gpui_wgpu/src/wgpu_context.rs:320`, the `"ZED_DEVICE_ID"` read at `:321`) — deep inside
`gpui_wgpu`'s context, not an argument. So:

- an application that knows which GPU it wants can only *be launched with an environment variable*, a
  shell-level accident rather than an application decision, and one the program itself cannot see;
- an application cannot express a *predicate* over adapters at all ("one that can export a dma-buf",
  "the one my producer's device matches") — only a raw PCI id, and only if it already knows it;
- the variable is a debugger's (it is named for an editor's builds) doing duty as a product API.

Nothing on `Application` or `PlatformOptions` reaches this. The chosen GPU is readable only *after* a
window exists — `window.gpu_specs()` (`crates/gpui_wgpu/src/wgpu_renderer.rs:1236`), or inside a
`gpu_canvas` through `Adapter::gpu_id` (`gpui_interop`'s `adapter.rs`) — which is 0012's subject.

## Proposal

A `GpuPreference` carried on `Application`/`PlatformOptions` and consulted where the adapter is actually
chosen (`select_adapter_and_device`, `crates/gpui_wgpu/src/wgpu_context.rs:604`):

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
(`crates/gpui_linux/src/linux/platform.rs:1313`), and lower it to `GpuPreference::Device`, so the
environment
variable and the API become one mechanism. Two things follow: a debug variable can no longer silently
disagree with an application's stated preference, and precedence is decided in one place instead of by
a tier ordering buried in the renderer.

## What would close it

`GpuPreference` on `Application`/`PlatformOptions`, plumbed to `select_adapter_and_device`;
`ZED_DEVICE_ID` implemented as `GpuPreference::Device` rather than an `env::var` read; and the default
decided — see 0012 for why it should be `LowPower`.

## Caveat

`GpuId` does not exist in tree. `gpui_interop::GpuId` (`gpui_interop`'s `linux.rs`)
is the only one, and it is out of tree, carrying a device name and a DRM render node — the two things a
producer matches on. A `Device(GpuId)` variant either adopts that type or the tree grows its own.
