# The retention seam, probed

- **Status:** evidence for [`0003`](0003-retention-ships-as-a-seam.md). Read-only: no code was written
  and nothing was compiled.
- **Method and limits.** Traced by reading, against `bite_v1.22.0-pre`. Every claim is a citation, and
  the checkout this was done in is on `bite_v1.14.x`, so a build would have meant switching branches
  and compiling the stack from scratch. What a build would add is whether the types line up, which the
  sketches in [`../architecture/retention-seams.md`](../architecture/retention-seams.md) §5–§6 carry —
  not whether the surface is *reachable*, which is what this probe asked.

## 1. Every `Window` field is `pub(crate)`

`pub struct Window<'frame>` (`crates/gpui_authoring/src/window.rs:1428`) has `pub(crate) core`
(`:1431`) and `pub(crate) frame_state` (`:1433`); the frame's `dirty_views` is `pub(crate)` (`:1227`);
and `WindowInvalidator`, which holds the notified set, is a `pub(crate)` type whose `take_views`
(`:287`) is `pub` on it.

**Consequence.** A mode living outside the crate reaches the window only through `pub` methods. So the
seam's published surface is the *entire* API a mode gets, and a missing accessor is not an
inconvenience to work around — it is a mode that cannot be written. That reframes the question R2 was
asked: not "how large is the diff" but "is the published surface sufficient".

## 2. The seam is in-stack in every option

`Application` is a thin wrapper — every `with_*` is a `set_*` on `App` through `self.0.borrow_mut()`
(`crates/gpui_runtime/src/application.rs:13`, with the existing setters at
`crates/gpui_authoring/src/app.rs:2931` and `:2937`). So a bootstrap for retention costs a method on
`Application`, a `set_view_retention_factory` beside the two already there, and a field on `App`.

So the **seam cannot be out of tree**: it is authoring types, `Window` methods and an `App` field.
R2 was therefore never choosing between an in-tree and an out-of-tree *seam*; it was choosing whether a
*mode* is in tree.

## 3. The seam as designed is missing two entry points

Both from this probe, both additive, both seam-side:

- **The notified set.** `ViewRetention::begin_frame(&mut self, window, cx)` hands the implementation a
  `&mut Window` and nothing it may read. §5 has the store own the expansion — the `consumers` map and
  the dirty set — but the notified entities live in the `pub(crate)` invalidator. **Fix:** authoring
  reads `take_views()` and passes it, so `begin_frame` takes `notified: &[EntityId]`. One argument, and
  the invalidator stays private.
- **The root list.** A's `ViewTree` owns `roots`/`next_roots` and swaps them in `finish_frame`;
  the frame's roots are also the window's, since events dispatch against them (R7). §5's `finish_frame`
  reconciles roots but defines no interface for them. **Fix:** §5 settles it as the window's — the walk
  collects the roots it attached and hands `finish_frame` that list — which is less state in the seam
  rather than more.

Neither changes `Frame` and neither widens an existing trait, so both ship with the seam in either
option.

## 4. Where a mode stops

What a mode needs from outside the seam: the notified set and the root list (§3), plus
`LayoutEngine::retained()` and the `TextSystem` use-scopes (§6, both additive). Everything else is its
own store — the node map, the `consumers` index, the record map.

And the record stays opaque, which is the finding that makes any of this expressible: `reuse` returns a
`ViewRecord` that *authoring* captured and *authoring* replays, so a mode never names
`PrepaintStateIndex`, `PaintIndex`, `reuse_prepaint` or `reuse_paint` — the items §2 finding 4 records
as unnameable outside the crate.

## 5. The footprint, and the decision it implies

| what the seam adds | where | kind |
| --- | --- | --- |
| `ViewRetention`, `ViewKey`, `ReadSet`, `ViewRecord` | `gpui_authoring` | new |
| `Window::capture_view_record` / `replay_view_record` | `gpui_authoring` | new methods |
| the notified-set and root-list entry points | `gpui_authoring` | new methods (§3) |
| factory field, `set_view_retention_factory` | `App` | new |
| `Application::with_view_retention` | `gpui_runtime` | new |
| `RetainedLayout`, `LayoutEngine::retained()` | `gpui_engine` | new, defaulted |
| text-use scopes on `TextSystem` | `gpui_engine` | new, defaulted |

Additive throughout: no existing trait widened, `Frame` untouched, and the default is no store at all
rather than an implementation that does nothing. A seam with no shipped swap is what `FramePipeline`
already is, and 0001 declines a third *swap*, not a fourth *open boundary*. A mode, by contrast, is a
published implementation of a published trait — a swap by 0001's own definition, and therefore a
version slot, which is the thing 0001 declined.

**What would falsify that:** a mode needing something the seam cannot express without widening an
existing trait or changing `Frame`. This probe found the two likely candidates and both resolved
additively (§3), so the falsifier currently has no instance — and one appearing is a reason to re-open
[`0003`](0003-retention-ships-as-a-seam.md), not to move a mode in.
