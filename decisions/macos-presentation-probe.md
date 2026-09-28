# Evidence: the macOS presentation probe

- **Evidence for** [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) —
  §6's macOS commit and §10's first reopen trigger, which it turns out not to fire. Kept as
  evidence, not as a proposal: what the design does with the answer is that patch.
- **The question:** can `gpui_wgpu::WgpuRenderer` present on a macOS window, and whose
  `CAMetalLayer` is the view's backing layer? **Answered: yes, on Metal — and the layer
  question dissolves, because wgpu adapts to whatever layer the view has.**
- **The probe** is the crate at `probes/macos-wgpu-present`, on `bite_v1.22.0-pre` (PR #3). Its
  job answered and was removed, so the printout in §2 is the durable record and the crate is
  the reproduction.

## 1. What it answered

One `macos-14` job, green in 1m7s, on a runner whose only adapter is
`Apple Paravirtual device` (`backend = Metal`, `device_type = IntegratedGpu`) — a real Metal
device, which is stronger than the Windows probes' WARP:

| view | backend set | surface | presented |
| --- | --- | --- | --- |
| the layer the caller installed | `METAL` | created | **yes** |
| the same view | `VULKAN \| GL` — what `gpui_wgpu` asks for today | **not created** | — |
| the same view | `VULKAN` | **not created** | — |
| the same view | `GL` | **not created** | — |
| a plain `CALayer` | `METAL` | created | **yes** |
| no layer of its own | `METAL` | created | **yes** |
| no layer yet, then one installed | `METAL` | created, both times | **yes** |

Two findings, and the second corrects this spike's own prior:

- **`gpui_wgpu`'s backend set did not reach Metal, and on macOS it failed a step earlier than
  on Windows.** `Backends::VULKAN | Backends::GL`
  (`crates/gpui_wgpu/src/wgpu_context.rs:309`) could not even create a surface here —
  *"Failed to create surface for any enabled backend"* — where on Windows it created one and
  then found no adapter. Either way `WgpuRenderer::new` failed before a frame, so the macOS
  commit needed `Backends::METAL` for the same reason the Windows one needed `Backends::DX12`;
  the seam enables both (`crates/gpui_wgpu/src/wgpu_context.rs:314`). `GL` did **not** answer
  on macOS, which the spike listed as the thing to rule out.
- **wgpu does not require the view to have a `CAMetalLayer`, and does not care about the
  order.** The spike read `raw-window-metal`'s `from_ns_view` as taking the view's layer, and
  `wgpu_hal::metal::Surface::from_layer`'s `isKindOfClass` assert as requiring one. That is not
  what happens: `from_retained_layer` **downcasts** the root layer and, when the downcast
  fails, wraps the old layer in an `ObserverLayer` and inserts a **new** `CAMetalLayer` as a
  sublayer (`raw-window-metal-1.1.0/src/lib.rs:344`). The raw-handle path then calls
  `Surface::new(layer)` (`wgpu-hal-29.0.4/src/metal/mod.rs:179`) and never reaches the assert,
  which belongs to `create_surface_from_layer` (`wgpu-hal-29.0.4/src/metal/mod.rs:137`) —
  wgpu-core's separate `create_surface_metal(*mut c_void)` entry point
  (`wgpu-core-29.0.4/src/instance.rs:390`). Every view works, with any layer or none, in any
  order.

## 2. The printout

```
machine adapter: "Apple Paravirtual device", backend = Metal, device_type = IntegratedGpu
--- METAL, layer installed by the caller: Backends(METAL) ---
METAL, layer installed by the caller: surface created from the view
METAL, layer installed by the caller: adapter = "Apple Paravirtual device", backend = Metal, device_type = IntegratedGpu
METAL, layer installed by the caller: formats = [Bgra8UnormSrgb, Bgra8Unorm, Rgba16Float, Rgb10a2Unorm]
METAL, layer installed by the caller: present_modes = [Fifo, Immediate]
METAL, layer installed by the caller: alpha_modes = [Opaque, PostMultiplied]
METAL, layer installed by the caller: configured format = Bgra8UnormSrgb, alpha_mode = Opaque
METAL, layer installed by the caller: PRESENT OK
--- gpui_wgpu's set (VULKAN | GL): Backends(VULKAN | GL) ---
gpui_wgpu's set (VULKAN | GL): surface NOT created: Failed to create surface for any enabled backend: {}
--- VULKAN: Backends(VULKAN) ---
VULKAN: surface NOT created: Failed to create surface for any enabled backend: {}
--- GL: Backends(GL) ---
GL: surface NOT created: Failed to create surface for any enabled backend: {}
--- METAL, plain CALayer: Backends(METAL) ---
METAL, plain CALayer: surface created from the view
METAL, plain CALayer: PRESENT OK
--- METAL, no layer: Backends(METAL) ---
METAL, no layer: surface created from the view
METAL, no layer: PRESENT OK
--- METAL, no layer yet: Backends(METAL) ---
METAL, no layer yet: surface created from the view
METAL, no layer yet: PRESENT OK
--- METAL, after the layer arrived: Backends(METAL) ---
METAL, after the layer arrived: surface created from the view
METAL, after the layer arrived: PRESENT OK
```

## 3. The outcome

**Yes**, and §10's first reopen trigger does not fire:

- **`MacSceneRenderer` keeps its shape.** The window can go on having
  `-[NSView makeBackingLayer]` return the renderer's layer
  (`crates/gpui_macos/src/window.rs:3257`), because wgpu neither needs that layer to be a
  `CAMetalLayer` nor minds when it arrives. The macOS commit is plumbing plus `Backends::METAL`, the
  same shape as the Windows commit.
- **A wgpu renderer need not vend a layer at all**, subject to §4's one unmeasured corner.
- **Transparency is available here, unlike Windows:** `alpha_modes = [Opaque, PostMultiplied]`
  against the DX12 surface's `[Opaque]` only.

## 4. What is not measured

- **A `makeBackingLayer` that returns nil.** The probe's layerless view is a plain `NSView`,
  which AppKit gives a default `CALayer`, so `raw-window-metal`'s `[view layer]` still finds
  one. A GPUI view overrides that method to return the *renderer's* pointer, which for a wgpu
  renderer would be null — so the macOS commit has to either keep `layer_ptr` non-null or stop
  returning it, and the probe narrows that decision without making it.
- **Hardware.** Apple's paravirtual GPU is a real Metal device rather than a software one, but
  it is still one device.
- **Resize and device loss**, which are
  [`../spi/rendering/verification.md`](../spi/rendering/verification.md) §1's rows and patch
  06's acceptance test.

## 5. What this does not decide

- **Whether `WgpuRenderer` should become the default on macOS.** That it *can* be installed is
  what this asked; nothing here decides that it should.
- **Path A on macOS with the Metal renderer**, which is unchanged — Metal produces and Metal
  consumes.
- **Windows and Linux.** Measured and unchanged.
