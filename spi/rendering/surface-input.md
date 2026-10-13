# Input over a surface: the gateway, capture, and pass-through

- **Status:** sketch — unbuilt. Written against `bite_v1.23.1-pre-interop`. It keeps two things
  straight: GPUI *hosts* input, and a producer must be able to meet its surface's events **without
  depending on `gpui`**.
- **Companion:** [`producer-seam.md`](producer-seam.md), the seam and why picking is the producer's;
  [`interop-crate.md`](interop-crate.md) §1/§2, the crate that already names `gpui` and so can adapt
  its events; [`player.md`](player.md) §6, the wedge a mis-honoured demand sets;
  [`../input/input-policy-seam.md`](../input/input-policy-seam.md), the parked **policy** seam — this
  document is about mechanism.

## 1. Who owns the event, and where it crosses

**GPUI is the host.** It owns the window, hit-tests its scene front to back (`Window::hit_test`, the
`HitTest` it returns), dispatches in `DispatchPhase::Capture` then `DispatchPhase::Bubble`, and
captures a gesture to wherever it started. The `surface()` / `gpu_canvas()` element is an ordinary
element in that tree — its hitbox is its bounds.

So the events that reach that element are the producer's, and there are two ways to hand them over:

- **Raw.** The element hands GPUI's own event types (`MouseDownEvent`, …) to the producer. Simple, and
  it makes every producer depend on `gpui`, on GPUI's event vocabulary, and on GPUI's coordinate space.
- **Through a gateway.** `gpui-interop` owns an event gateway: the producer subscribes to it, the
  element routes the events it receives to it, and it *adapts* before delivery. The producer depends on
  `gpui-interop` — never on `gpui`.

The gateway is the better half because `gpui-interop` is already the side that may name `gpui`: it
depends on the facade for `attach` and `GpuCanvasContext::device::<R>()`
([`interop-crate.md`](interop-crate.md) §1). It is the *specific implementation*, so re-exporting and
re-shaping GPUI's events is exactly its place — and it keeps the producer's dependency surface to one
crate, which is the whole reason it exists.

## 2. What the gateway adapts

- **Coordinates.** GPUI's events carry a window-relative logical `Point<Pixels>`. A producer wants its
  own frame: the gateway translates to **surface-local physical pixels** and carries the scale factor,
  so a 3D scene or a webview gets a position it can use without knowing GPUI's tree, the element's
  placement, or the window's DPR. A producer that wants logical surface-local coordinates is the same
  translation with the scale left in.
- **Vocabulary.** A producer-facing event enum — pointer move/down/up, scroll, pinch, key, modifiers —
  mapped from GPUI's, so the producer never names `MouseDownEvent`.
- **The answer.** The producer returns **handled** or **pass**. *Pass* is what lets a GPUI element
  *under* the surface's rectangle still receive the event (§4).
- **Per-producer shape.** Because the gateway is interop's, it can answer what each producer needs — a
  webview's CSS-like logical coordinates, a 3D scene's physical pixels from the surface origin, a
  video's pass-through — without `gpui` learning about any of them.

## 3. Routing and capture stay the engine's

The gateway adapts; it does not route. Two rules the model already has, and the gateway must not
re-implement:

- **A gesture that starts on a GPUI element stays with it.** The element captures at pointer-down, so
  moves keep arriving at it *even while the pointer crosses a surface drawn under it*. The surface
  never sees a move the element has not released.
- **A gesture that starts on the surface stays with the surface.** The surface captures at
  pointer-down, so its drag keeps arriving while the pointer crosses GPUI elements drawn *on top of*
  it — an overlay does not steal the gesture mid-flight.

Both are one rule: **a gesture belongs to where it started, not to what is under the pointer now.**
Hit-testing per position instead of capturing is what lets an overlay steal a drag halfway through —
and that is the engine's to get right, not the gateway's, which only ever sees what GPUI already routed
to the surface.

## 4. Pass-through, and why it needs the producer

A rectangle is coarser than a scene. The engine can hit-test a surface's *bounds*, but an imported
surface is one opaque quad to it — the objects inside are the producer's and invisible to GPUI
([`producer-seam.md`](producer-seam.md) §6). So "does the surface want this event" is finer than the
hitbox:

- **The engine's granularity is the rectangle.** A pointer inside the surface's bounds hits the
  surface, unless a GPUI element in front has captured or consumed it.
- **The producer's granularity is the cell.** Within its bounds the producer may pick exactly — a
  G-buffer, or an id buffer it already rendered — and answer **handle** or **pass** through the
  gateway. *Pass* is what lets a GPUI element *under* the surface's rectangle still receive the event,
  and it is the only honest way for a control composited inside a 3D view to be interactive without
  GPUI knowing the 3D scene.

The gateway carries the answer; it does not make the decision. The producer picks its own scene, and the
engine relays.

## 5. The reverse direction: a guest platform

Guest Mode ([`surfaces.md`](surfaces.md) §4) runs GPUI **headless on a worker thread inside a foreign
host**, so the ownership flips: the host owns the window and the cadence, and its events have to be
translated into GPUI's own input shape and handed to GPUI, which then routes them as in §3.

That is a different direction and a different shape from the gateway — a `Platform` implementation
(`PlatformWindow::on_input` takes `PlatformInput`), not a producer subscription — and it is unbuilt and
gated on P7 ([`probe-p7-guest-thread.md`](probe-p7-guest-thread.md)). The renderer is `!Send` by
construction ([`producer-reach.md`](producer-reach.md) §2), so only a `Send + Sync` handle crosses. It
is recorded here only so the two halves are not confused: **GPUI hosting a producer's surface needs the
gateway; a foreign host hosting GPUI needs a `Platform`.**

## 6. It is not 3D-specific, and that is the point

Nothing above names a codec, a mesh or a G-buffer. A surface is a surface
([`surface-seam.md`](surface-seam.md)); a video under a transport bar, a webview under an overlay, a
3D viewport under gizmos — all are the same gateway plus pass-through, and all need capture to keep a
drag from being stolen by whatever it happens to pass beneath. The producer's *picking* is the only
3D-shaped part, and it stays with the producer.

## 7. What would reopen it

- **GPUI growing a producer-facing event API of its own.** Then the gateway is a re-export rather than
  an adapter, and §2 shrinks to the coordinate translation.
- **A producer that needs GPUI's *raw* event** — the unadapted type, for an element tree it draws
  itself. That is not the gateway's shape, and it is where the "raw" arm of §1 would have to come back.
- **The engine growing a first-class "declines" signal on a hit target.** Then pass-through stops being
  a gateway answer and becomes an input to hit-testing, and §4 shrinks.
