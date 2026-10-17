# Pointer lock: the relative pointer a 3D camera wants

**Status: future work, parked.** Nothing is designed, proposed or built. Recorded so that the finding
is not re-derived: what a 3D viewport's *camera* needs — as opposed to the gesture the input gateway
gives it — is a window-level pointer lock, and that lies below the seam the gateway sits above, so
`gpui_interop` cannot reach it at all.

**Companion:** [`../rendering/surface-input.md`](../rendering/surface-input.md), the gateway this
would sit beside; [`../rendering/surfaces.md`](../rendering/surfaces.md) §4, host and guest modes;
[`input-policy-seam.md`](input-policy-seam.md), the other parked input document.

**Line references** are to the canonical ref `bite_v1.23.x` (the engine sources,
under `crates/gpui_platform`, `crates/gpui_types`, `crates/gpui_authoring`), and to the
`gpui_interop` repository for the crate side.

---

## 1. What is wanted, and why a drag is not it

A surface can already be dragged: [the input gateway](../rendering/surface-input.md) founds the
engine's drag on a pointer-down and keeps delivering surface-local moves wherever the pointer goes, so
a viewport turns while a button is held. What it cannot do is let a producer drive a **camera** — the
first-person case, where the pointer is locked to the window, the cursor stops at the edge, and the
*delta* of the mouse is the camera's. A 3D shooter, a level editor's fly-through, a CAD orbit that
should keep going when the pointer reaches the edge: each expects the pointer to be *taken*, and
released on Escape — the near-universal convention — and mandatorily when the window loses focus.

The difference is not one of degree. A drag is a gesture **with a button**, bounded by that button's
hold; a camera is continuous motion **without** one. Stretching the gateway to cover the second would
make a demand on the OS window that an element cannot make.

## 2. Why it cannot be done where the gateway is

`gpui_interop` reaches the engine as an element: events in, pixels out. Locking the pointer is a
property of the OS window, and GPUI owns the window — a producer has none to lock. The engine's whole
pointer surface is three calls and a query:

- `Platform::set_cursor_style(CursorStyle)` — `crates/gpui_platform/src/platform.rs:261`
- `Platform::hide_cursor_until_mouse_moves()` and `Platform::is_cursor_visible()` — `:265`, `:268`
- `PlatformWindow::mouse_position() -> Point<Pixels>` —
  `crates/gpui_platform/src/platform_window.rs:98`

and nothing that grabs, confines or locks. The raw handle GPUI lends
(`impl HasWindowHandle for Window`, `crates/gpui_authoring/src/window.rs:8142`) is not an answer: the
backend's own event loop would still be the one receiving the motion, and it delivers that motion as
an absolute position (§3), so a producer reaching around GPUI would fight the framework rather than
extend it.

## 3. Two things are missing, and the second reaches further

**A platform call.** A pointer mode — locked, confined, or free — with a per-backend implementation.
Not one of the four backends the engine targets has a line of this today:

| platform | primitive |
| --- | --- |
| macOS | `CGAssociateMouseAndMouseCursorPosition(false)`, cursor hidden; `NSEvent` already carries `deltaX`/`deltaY` |
| Wayland | `zwp_pointer_constraints_v1` (`locked`, or `confined` for the weaker mode) plus `zwp_relative_pointer_v1` for the motion |
| X11 | `XGrabPointer` plus XInput2 raw motion (with a warp to re-centre where the protocol needs one) |
| Windows | raw input — `RegisterRawInputDevices` with `RIDEV_NOLEGACY` and `WM_INPUT`; `ClipCursor` for confinement alone |

**A delta in the event shape.** `MouseMoveEvent` is
`{ position: Point<Pixels>, pressed_button: Option<MouseButton>, modifiers: Modifiers }`
(`crates/gpui_types/src/input.rs:102`), and the position is the only motion it carries. A locked
pointer has no meaningful position to report, so a camera reads a delta the event does not have — and
giving it one is a change to the engine's **event vocabulary**, not the accessor-sized change the
gateway needed (`App::active_drag_value`). That, more than the platform calls, is what puts this in a
different order of work.

## 4. The questions a design has to answer

- **Which trait, and is the method defaulted?** `Platform` is where `set_cursor_style` lives, and it is
  a *published* seam ([`../README.md`](../README.md)), so a new non-defaulted method forces every
  implementor, out-of-tree ones included. Most of `Platform`'s later methods carry defaults; whether a
  pointer mode can is a design question, not a detail.
- **Who owns the lock, and for how long?** It has to release on Escape, on focus loss, on window
  deactivation and on the element's disappearance. A lock a dropped element still holds is a poisoned
  cursor, so ownership is the sharp end — the OS call is the easy half.
- **What does confinement do to hit-testing?** A lock that confines the pointer to a rectangle the
  element does not cover is a shape the engine has never had; hover and `mouse_position` both assume
  the cursor may be anywhere.

## 5. Where it stands upstream

Nowhere. On 2026-10-14 there is **no PR, issue or line of code** for pointer lock, relative mouse mode
or cursor confinement in `zed-industries/zed` — searched for winit's `CursorGrabMode` and for each of
the four platform calls in §3. The nearest merged work is the cursor-*style* pair,
`active_drag_cursor_style` / `set_active_drag_cursor_style`
([#32028](https://github.com/zed-industries/zed/pull/32028), +20 lines in `app.rs`), and the open
[#63790](https://github.com/zed-industries/zed/pull/63790) is a second instance of wanting to change a
cursor mid-drag. The one PR that wanted 3D at all —
[#43299](https://github.com/zed-industries/zed/pull/43299), an RGBA arm on the video path, macOS only,
the same route the surface work takes — was closed as *"not something we're ready to take on right
now, … might reopen when the time is right"*.

So there is no precedent to lean on and no seam to extend quietly: a mode on `Platform`, four
backends, and a delta on the move event.

## 6. When to revisit

When a surface's producer needs a camera rather than a gesture — the first demo that has to keep
turning as the pointer leaves the window, or that wants the cursor out of the way. Not before. The
gesture covers the viewport under the pointer, it costs no core change, and a lock is a window-level
demand whose *lifetime* is far easier to get wrong than its OS call is to write.
