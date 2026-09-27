# Spike: can `WgpuRenderer` present on a macOS window?

- **Status:** spike, **not run**. The macOS counterpart of
  [`windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md), the gate on
  patch 06 of [`renderer-seam.md`](renderer-seam.md) §6, and the measurement behind that
  document's §10 first reopen trigger.
- **Answers one question:** can `gpui_wgpu::WgpuRenderer` present on a macOS window — and whose
  `CAMetalLayer` is the view's backing layer?
- **Why reading does not settle it.** The Windows probe found that `gpui_wgpu`'s instance asks
  for the wrong backends on the one platform it has never run on. macOS is the other such
  platform, and it has a wrinkle Windows does not: the layer GPUI's view backs itself with is
  the *renderer's*, so a wgpu renderer has to own a layer that wgpu's own surface then reads
  back off the view. Whether that ordering works is not a thing a signature says.
- **Inherits:** corrections 1–20, [decision 0002](../../decisions/0002-render-extension-device-model.md),
  and [`windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md). Windows
  and Linux are out of scope — Windows is measured, and on Linux wgpu *is* the renderer.

## 1. Four readings that close most of it

- **The renderer owns the layer, and the window installs it.** `MetalRenderer::new` creates
  `metal::MetalLayer::new()` (`crates/gpui_apple/src/metal_renderer.rs:158`), and the window's
  `-[NSView makeBackingLayer]` override returns *the renderer's* pointer
  (`crates/gpui_macos/src/window.rs:3237`). Nothing else creates a layer, and the window asks
  the renderer for one rather than making its own.
- **So `MacSceneRenderer`'s proposed shape is today's shape.** `layer_ptr` and
  `set_presents_with_transaction` are inherent methods on the Metal renderer
  (`crates/gpui_apple/src/metal_renderer.rs:366`, `:377`), and the window also reads the layer
  directly to set `contentsScale` (`crates/gpui_macos/src/window.rs:3087`). §10's reopen
  trigger — "if the window must own the layer" — is therefore *not* the current design, and the
  probe is what would move it.
- **wgpu does not create a layer; it requires one that already is a `CAMetalLayer`.** Given
  `RawWindowHandle::AppKit`, wgpu's Metal backend calls
  `raw_window_metal::Layer::from_ns_view(handle.ns_view)`
  (`wgpu-hal-29.0.4/src/metal/mod.rs:161`), which sets `wantsLayer` and then takes `[view
  layer]` (`raw-window-metal-1.1.0/src/lib.rs:402`); `wgpu_hal::metal::Surface::from_layer`
  asserts that layer is a `CAMetalLayer` (`wgpu-hal-29.0.4/src/metal/surface.rs:26`). A view
  whose layer is a plain `CALayer`, or none, fails there — and a view whose layer is the
  renderer's `MetalLayer` satisfies it, because that *is* a `CAMetalLayer` subclass.
- **`gpui_wgpu`'s instance has the Windows problem here too, with a second backend to trip
  on.** Its non-wasm instance is `Backends::VULKAN | Backends::GL`
  (`crates/gpui_wgpu/src/wgpu_context.rs:292`) — no `METAL` — so on macOS a `WgpuRenderer::new`
  selects Metal only if that list changes. And unlike Windows, `GL` *might* answer on macOS,
  which would mean the renderer runs on OpenGL rather than failing outright. Whether it does is
  the part of this that only a run settles.

That leaves the two things a run decides: whether wgpu's Metal path accepts a layer the
renderer created and vended through `makeBackingLayer`, and what `VULKAN | GL` actually finds
on macOS.

## 2. The probes

Run on `macos-14`, the way the Windows probes ran on `windows-latest`, in a new standalone
crate beside `probes/windows-wgpu-present` (its own `[workspace]`, so it costs the main build
nothing). A probe is added while it is answering and taken out when it has.

| # | probe | answers |
| --- | --- | --- |
| 1 | create an `NSView`, set a `CAMetalLayer` on it as the renderer does, build a wgpu surface from `RawWindowHandle::AppKit { ns_view }`, request an adapter, print the surface's formats, present modes and alpha modes, configure, clear, submit, `present()` | whether wgpu's Metal path accepts a view whose layer the renderer installed — the case patch 06 creates |
| 2 | the same view with a plain `CALayer`, and with no layer at all | whether the window could keep its own layer instead of installing the renderer's — §10's reopen trigger, and the failure `raw-window-metal` is read to produce |
| 3 | the same view under `Backends::VULKAN \| GL`, and under `VULKAN` and `GL` separately | what `gpui_wgpu`'s current backend set finds on macOS at all, and whether it is OpenGL — which would be a backend the design has never considered |
| 4 | create the surface *before* any layer exists on the view, then attach one | whether the naive patch's ordering — renderer constructs, surface is built first — fails where an ordering that creates the layer first succeeds |

Probes 1 and 3 are the decisive pair. Probe 4 is the one that decides *how* patch 06 is
written, and it is cheap once 1 has a view.

## 3. The outcomes

- **Yes, and the layer may be the renderer's.** `MacSceneRenderer` keeps its shape, and patch
  06 is plumbing plus `Backends::METAL` on macOS — with the constraint, recorded for whoever
  writes it, that a `WgpuRenderer` must create its layer before its surface and vend it through
  `layer_ptr`.
- **Yes, but the window must own the layer.** §10's trigger fires: `MacSceneRenderer` is the
  wrong shape, the layer becomes a platform-side resource, and the change moves into
  `renderer-seam.md` §5.5's construction site.
- **No.** `WgpuRenderer` cannot be installed on macOS, Metal stays the renderer there, and the
  open question "should wgpu be the default on macOS and Windows?" is answered no for macOS —
  which is worth knowing cheaply.

## 4. Hazards

- **A runner with no Metal device.** A hosted macOS runner is a virtual machine. Apple's
  virtualisation is expected to supply a Metal device, but if it does not, the probe's adapter
  list says so and the result is a fact about the runner rather than about the design — which
  is why probe 1 prints `enumerate_adapters` before it tries anything.
- **A window without a session.** `raw-window-metal` asserts `MainThreadMarker`, so the probe
  has to run on the main thread, and creating an `NSView` needs an AppKit session. If the
  runner's job has none, the failure is environmental and has to be told apart from the
  answer, in the same way WARP is on Windows.
- **A layer that is set but never displayed.** The probes present to a layer that is not on
  screen; whether a presented frame is *visible* is what the end-to-end row in
  [`verification.md`](verification.md) §1 is for, after patch 06 exists.

## 5. What this does not decide

- **Path A on macOS with the Metal renderer.** Unchanged, and not in question: Metal produces
  and Metal consumes, which is why macOS was never the hard platform for Path A.
- **Whether `WgpuRenderer` should become the default on macOS**, beyond the mechanical question
  of whether it *can* be installed. The seam makes that askable; nothing decides it.
- **Windows and Linux.** Measured and unchanged.
