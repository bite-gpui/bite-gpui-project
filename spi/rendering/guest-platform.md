# The guest's `Platform`: headless GPUI a host drives

- **Status:** sketch — the `guest` module is a doc-only stub ([`interop-crate.md`](interop-crate.md)
  §3), and P7 gates the thread ([`probe-p7-guest-thread.md`](probe-p7-guest-thread.md)). Guest Mode
  itself is [`surfaces.md`](surfaces.md) §4. This is the shape to build against, not a plan of record.
- **Companion:** [`surface-input.md`](surface-input.md), the input half in its own right;
  [`producer-reach.md`](producer-reach.md) §2 and [`verification.md`](verification.md) §1, the `!Send`
  boundary; [`player.md`](player.md) §4/§6, the demand and the wedge.

## 1. Why the `Platform` seam, and not a new one

Guest Mode is *"headless GPUI on a worker thread"* ([`surfaces.md`](surfaces.md) §4): the host owns the
OS window and the cadence, and the GPUI thread runs a headless context. That is a `Platform` — the
seam whose bootstrap is `Application::with_platform(Rc<dyn Platform>)`
([`../README.md`](../README.md)) — and the seam *already has the slots*:

| a host must | the seam's method |
| --- | --- |
| inject input | `PlatformWindow::on_input(Box<dyn FnMut(`[`PlatformInput`]`) -> DispatchEventResult>)` |
| ask for a frame | `PlatformWindow::on_request_frame(Box<dyn FnMut(RequestFrameOptions)>)`, `schedule_frame` |
| get the frame out | `PlatformWindow::with_renderer(&mut dyn FnMut(&mut dyn SceneRenderer))`, `present(..) -> bool` |
| set geometry and DPR | `PlatformWindow::resize(Size<Pixels>)`, `scale_factor()`, `content_size()` |

So the `guest` module ships a `Platform` implementation, and the application *installs* it. That is a
different posture from the transport — which is *reached*, not installed ([`interop-crate.md`](interop-crate.md)
§5) — and it is why the two halves sit in one crate but not one API. Implementing a published seam is
not publishing one: this adds no SPI.

## 2. The shape

The renderer is `!Send` by construction — the factory holds an `Rc`
([`producer-reach.md`](producer-reach.md) §2) — so the renderer stays on the guest thread and only a
`Send + Sync` handle crosses, which is the shape `ImportedTextureHandle` already takes
(`Arc<dyn Any + Send + Sync>`, [`probe-p7-guest-thread.md`](probe-p7-guest-thread.md) §2). So:

```rust
/// On the worker thread; owns the renderer and the GPUI `Application`. Not `Send`.
pub struct Guest { .. }

impl Guest {
    /// A headless context of `size` at `scale`, with the frames it makes handed to `on_surface`.
    pub fn new(open: impl FnOnce() -> Box<dyn SceneRenderer> + 'static, /* … */) -> Self;
    /// What the host thread holds — `Send + Sync`, and the only thing that crosses.
    pub fn handle(&self) -> GuestHandle;
}

/// The host's end. Any thread.
#[derive(Clone)]
pub struct GuestHandle { .. }

impl GuestHandle {
    /// The host's event, already translated to GPUI's own input shape.
    pub fn dispatch(&self, input: PlatformInput) -> DispatchEventResult;
    /// The host's cadence: a frame is due, and the guest may present one.
    pub fn request_frame(&self);
    /// Geometry or DPR changed: the render target resizes and the window metrics move.
    pub fn set_metrics(&self, size: Size<Pixels>, scale_factor: f32);
}
```

Three things the sketch fixes:

- **Input crosses as `PlatformInput`, not as GPUI's `KeyDown`/`MouseDown`.** The seam's unit is the
  platform's own event enum, and translating `PlatformInput` → events is the *engine's* job, after the
  callback returns. So the shim translates the *host's* event into `PlatformInput` and stops there —
  hit-testing, capture and bubbling are what GPUI then does with it ([`surface-input.md`](surface-input.md) §3).
- **Cadence is a demand the host makes, not a loop the guest runs.** `request_frame` is the host's vblank;
  the guest must honour every demand, because a dropped one strands the host exactly as
  [`player.md`](player.md) §4 describes the Wayland loop stranding a window. `gpui_linux`'s demand-driven
  loop is the model.
- **Metrics are a handshake, not a setter.** `set_metrics` is the DPR and physical-size agreement
  [`surface-input.md`](surface-input.md) §2 names; a change the host announces and the guest drops is
  the wedge [`player.md`](player.md) §6 warns about.

## 3. The frame out is the open bit

The offscreen contract returns **bytes**: `render_scene` renders into a target and `read_pixels` copies
it out ([`probe-p7-guest-thread.md`](probe-p7-guest-thread.md) §1). Guest Mode needs the *surface*
instead — the GPU path [`surfaces.md`](surfaces.md) §4 describes — and whether the contract has a
method for that is the thing P7 measures. The candidate is that the guest renders into a **shareable
target** and hands out its handle:

- the target comes from the transport's own ring (`Adapter::pool(..)`, the `SurfacePool` in
  [`interop-crate.md`](interop-crate.md) §4);
- the handle is `Send + Sync`, so it crosses where the renderer cannot;
- the host opens it the way its own consumer does — a shared NT handle, a dma-buf, an `IOSurface`.

So the frame-out half and the transport are the same interchange seen twice: the guest is a producer
whose consumer is a foreign loop.

## 4. What it does not do

- **No OS window.** The host owns it; the guest's `PlatformWindow` is not one.
- **No policy.** Coalescing, throttling, "which element wins" is the engine's; the parked
  [`../input/input-policy-seam.md`](../input/input-policy-seam.md) is where that would live.
- **No picking.** The producer picks its own scene ([`surface-input.md`](surface-input.md) §4).

## 5. What would reopen it

- **The offscreen contract growing a surface method.** Then §3 stops being the guest's problem and the
  frame-out half becomes a call.
- **The renderer losing `!Send`.** Then the renderer itself could cross and the handle indirection in §2
  would collapse.
- **Events that need to flow the other way.** An element that wants a gesture the host owns — unbuilt,
  and its own statement ([`surface-input.md`](surface-input.md) §6).
