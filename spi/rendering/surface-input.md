# Input over a surface: the bubbling model, capture, and picking

- **Status:** proposed — written against `bite_v1.23.1-pre-interop`. The shim is unbuilt (`guest` is a
  stub, P7 gates the thread), so this is the model a shim must *keep*, not re-invent: GPUI already
  routes input, and a surface enters that routing as a first-class target rather than growing a
  parallel dispatch path beside it.
- **Companion:** [`producer-seam.md`](producer-seam.md), the seam and why picking is the producer's;
  [`interop-crate.md`](interop-crate.md) §3, the `guest` module whose *input* half this is;
  [`player.md`](player.md) §6, the wedge a mis-honoured demand sets; and
  [`../input/input-policy-seam.md`](../input/input-policy-seam.md), the parked **policy** seam — this
  document is about mechanism.

## 1. The two halves, and the posture each has

`guest.rs` states the shape: "a host loop needs **both halves** — the frame out, and translated input
in." The halves do not share a posture, and that is the first thing to keep straight:

| half | what it is | posture |
| --- | --- | --- |
| **frame out** | the transport — match the adapter, move a handle, order the queues | *not a seam*: reached by the application ([`interop-crate.md`](interop-crate.md) §5) |
| **input in** | a shim that translates host events and injects them | the `Platform` seam: *installed* by the application ([`../README.md`](../README.md)) |

So the input half ships a `Platform` implementation — the crate's one seam-*participating* piece.
Implementing a published seam is not the same as publishing one, but it does mean the shim answers to
the SPI rules, and cannot be folded into `attach`/`Adapter`.

## 2. What the shim does, and only that

- **Translate.** A host window reports a pointer in its own frame: window-relative, at the host's
  device-pixel ratio. GPUI wants a `Point<Pixels>` in *logical*, window-relative coordinates, and its
  renderer wants a target in *physical* pixels. The two handshakes — physical size, and the
  logical→physical scale — are the shim's, and each must be renegotiated on a DPR or geometry change
  (resize the target, update the window metrics). A change the host announces and the guest ignores is
  the demand [`player.md`](player.md) §6 warns wedges a producer.
- **Inject.** Post the translated event into GPUI's dispatcher, and nothing more.

GPUI's *policy* — which element wins, how events coalesce, what a rate limit is — is not here. It is
the engine's, and the seam that would host it is parked
([`../input/input-policy-seam.md`](../input/input-policy-seam.md)).

## 3. Routing is the engine's, and a surface is a first-class target

GPUI already routes input, and that model is the one a surface has to enter:

- The scene is **hit-tested** front to back (`Window::hit_test`, the `HitTest` it returns), so what is
  drawn on top is what is hit first.
- Dispatch runs in two passes, `DispatchPhase::Capture` then `DispatchPhase::Bubble`, down and up the
  element tree.
- A gesture is **captured**: `capture_any_mouse_down`, `capture_any_mouse_up`, `capture_pinch` and
  `on_drag` chain the rest of the gesture to wherever it *started*.

The `surface()` element is an ordinary element in that tree — its hitbox is its bounds, and it takes
part in hit-testing, capture and bubbling exactly as a `div` does. The two cases in the question then
fall out of the model rather than needing a rule of their own:

- **A gesture that starts on a GPUI element stays with it.** The element captures at pointer-down, so
  moves keep arriving at it *even while the pointer crosses a surface drawn under it*. The surface
  never sees a move the element has not released.
- **A gesture that starts on the surface stays with the surface.** The surface captures at
  pointer-down, so its drag keeps arriving while the pointer crosses GPUI elements drawn *on top of*
  it — an overlay does not steal the gesture mid-flight.

Both are the same rule: **a gesture belongs to where it started, not to what is under the pointer
now.** Hit-testing per position instead of capturing is exactly what lets an overlay steal a drag
halfway through.

## 4. Pass-through, and why it needs the producer

A rectangle is coarser than a scene. The engine can hit-test a surface's *bounds*, but an imported
surface is one opaque quad to it — the objects inside are the producer's and invisible to GPUI
([`producer-seam.md`](producer-seam.md) §6). So "does the surface want this event" is finer than the
hitbox:

- **The engine's granularity is the rectangle.** A pointer inside the surface's bounds hits the
  surface, unless a GPUI element in front has captured or consumed it.
- **The producer's granularity is the cell.** Within its bounds the producer may pick exactly — a
  G-buffer, or an id buffer it already rendered — and answer **handle** or **pass**. *Pass* is what
  lets a GPUI element *under* the surface's rectangle still receive the event, and it is the only
  honest way for a control composited inside a 3D view to be interactive without GPUI knowing the 3D
  scene.

The shape is a callback the `surface()` element routes with *surface-local* coordinates — never window
coordinates, and never a second hit-test tree. The producer picks its own scene; the engine relays.

## 5. It is not 3D-specific, and that is the point

Nothing above names a codec, a mesh or a G-buffer. A surface is a surface
([`surface-seam.md`](surface-seam.md)); a video under a transport bar, a webview under an overlay, a
3D viewport under gizmos — all are the same rectangle plus callback, and all need capture to keep a
drag from being stolen by whatever it happens to pass beneath. The producer's *picking* is the only
3D-shaped part, and it stays with the producer.

## 6. What would reopen it

- **A guest that drives the other way.** Guest Mode puts GPUI on a worker thread *inside* a foreign
  host, so events flow host→GPUI as above — but GPUI's own events may then need to flow *out* to the
  host (an element that wants a gesture the host owns). That direction is unbuilt and would need its
  own statement.
- **A producer that declines after capture has begun.** Pass-through today is per-event; a drag that
  leaves the region it should own needs a rule the model does not have yet.
- **The engine growing a first-class "declines" signal on a hit target.** Then pass-through stops
  being a producer callback and becomes an input to hit-testing, and §4 shrinks.
