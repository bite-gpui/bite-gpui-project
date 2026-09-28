# Retention seams, and what blocks them

- **Status: proposed.** Nothing here is implemented, and the resolutions in §5–§8 are designs,
  not code. This is the *seam* companion to [`view-retention.md`](view-retention.md): that chapter is
  what the two designs do; this one is how retention would attach to *this* stack, and what stops
  each attachment point.
- **Sources.** The same two efforts, read at the same places: Zed `#63800` at
  `refs/pull/63800/head` (`3d0b7f0a`) and gpui-fast's `main` (`fast/`, `docs/`). Claims about this
  stack are cited against `bite_v1.22.0-pre`. `bite-gp-morphorm` and `bite-gp-parley` are named but
  not cited by line: they are published from repositories of their own, not from this checkout.

## 1. The seams that exist

| seam | where | what it carries | a second implementation |
| --- | --- | --- | --- |
| `LayoutEngine` | `crates/gpui_engine/src/layout.rs:53` | `clear()` per frame; `request_layout(style, rem_size, scale_factor, children) -> LayoutId` — **no identity** | `bite-gp-morphorm`; Taffy at `crates/gpui_engine_default/src/layout.rs:375` |
| `FrameSession` | `crates/gpui_engine/src/frame_session.rs:18` | owns the layout engine for a window; the scene stays in the facade's frame | — |
| `FramePipeline` | `crates/gpui_authoring/src/window/frame_pipeline.rs:28` | passes over `PreparedRoots`; `should_render` defers a frame | `bite-gp-pass` |
| `TextSystem` | `crates/gpui_engine/src/text_system.rs:33` | already carries layouts across frames — `layout_index` at `:133`, `reuse_layouts`, `truncate_layouts` | `bite-gp-parley` |
| `.cached()`, and **not a seam** | `view.rs:295` (`ViewElementState`), `view.rs:302` (`ViewElementCacheKey`), `view.rs:426` (`prepaint_cached_view`) | one view's previous prepaint/paint ranges, replayed by `window.rs:4244` / `window.rs:4317` against `window.rs:1021` / `window.rs:1031` | — |

## 2. Why it is not a plugin today

Four findings, each the reason for one:

1. **The reuse unit is the view boundary, inside the walk.** A view is an `Element`
   (`crates/gpui_authoring/src/element.rs:55`) driven by `Drawable` during `layout_roots` /
   `paint_roots`. Retention must decide *per occurrence, mid-walk*; there is no trait at that point.
2. **`LayoutEngine` carries no identity and is told to clear.** `request_layout` receives style and
   child `LayoutId`s; the contract is `clear()` "ready for a fresh frame". A retaining solver cannot
   match a node to an element, and would be cleared anyway.
3. **`FramePipeline` sees roots, not view occurrences.** It can *defer a whole frame*, which is
   pacing, not subtree reuse; one dirty row still rebuilds everything.
4. **The reuse state is `pub(crate)`.** The record/replay is index surgery over the frame's flat
   vectors, not a type anyone outside the crate can name.

## 3. What a re-cut of the crates would and would not buy

The question "can we move parts of `gpui_authoring` into `gpui_engine` / `gpui_runtime`" has a
clear answer, and it is *no*, along the axis that matters:

- The crate says it itself: *"The element DSL is not separable from the runtime: an element is laid
  out and painted with a concrete `Window` and `App`"* (`crates/gpui_authoring/src/gpui_authoring.rs:8`).
- `gpui_runtime` is **above** authoring — it installs `Application::with_layout_engine`
  (`crates/gpui_runtime/src/application.rs:92`) and `with_frame_pipeline` (`:107`) but reaches
  authoring only through its public surface. It can hold a *policy* or a decorator; it cannot own
  the frame state.
- `gpui_engine` is **below** `gpui_platform`, and the frame's channels name `PlatformInputHandler`
  and `PlatformWindow` — so the frame cannot move down without inverting that edge.
- The tier boundary already exists, *inside* the crate: `_authoring.rs:65` "What is not authoring
  API" (frame orchestration, element storage, frame double-buffering and the hit-test tree at `:75`)
  and `:82` "The rest of `Window` is runtime API".

The pivot is `Window`. Any cut that isolates the walk also moves `Window`, which either drags the
platform SPI into the engine or erases the engine/authoring distinction.

## 4. The traits a retention seam needs

Five, of which **three are axes** a mode chooses and **two are capabilities** it uses. The
signatures are in §5 and §6, where [`view-retention.md`](view-retention.md) §5's combined design
is turned into a seam on *this* stack; the axes are:

| axis / capability | trait | layer | choices |
| --- | --- | --- | --- |
| axis | `ViewRetention` | authoring | `Immediate` · `PersistentTree` (A) · `SideTables` (B) |
| axis | `Reactivity` | authoring | `StrictNotify` (A) · `UpdateGenerations` (B) |
| axis | `RetainedLayout` | engine | `–` · `NodeOwned` (A) · `KeyedPathHash` (B) |
| capability | `TextSystem` scopes | engine | shared |
| capability | `Scene` replay + `ViewRecord` | engine + authoring | shared |

The four modes are corners of that space; §8 is the matrix. The point of the factoring is its
last row: **AB is A's spine with B's sensor**, so it is a recomposition rather than a third
engine.

## 5. R0 — the recorder (resolved)

**The blocker.** An out-of-tree `ViewRetention` needs to record and replay a view's slice of the
frame. That slice is not a handle; it is a protocol across eleven channels in
`crates/gpui_authoring/src/window.rs:995` (`Frame`): hitboxes, tooltips (`:976`), deferred draws
(`:981`, holding a frame-arena `AnyElement`), the dispatch tree, accessed element states, mouse
listeners (type-erased closures), input handlers (the platform SPI), cursor styles, tab stops and
the text-cache index — addressed by `PrepaintStateIndex` (`:1021`) and `PaintIndex` (`:1031`) and
replayed by `reuse_prepaint` (`:4244`) / `reuse_paint` (`:4317`), which **move** slices out of the
rendered frame into the next and remap dispatch ids.

**Why publishing it is wrong.** The channels are heterogeneous in kind (plain data, non-cloneable
`dyn` closures, the platform SPI, frame-arena objects); the transfer is move-based and
order-sensitive; a record's lifetime is one frame pair, which nothing can enforce; two fields are
feature-gated; and the whole thing would freeze `Frame` — the crate's most-churned structure — as a
permanent public contract. It is the addition the rulings exist to prevent ("decorate traits; do not
fork them"; a leaky abstraction gets an escape hatch, not a new method).

**Resolution.** Publish the *opaque record*, not the slice. Authoring owns capture, splice and
replay; the trait owns only the store and the decision:

```rust
/// Opaque: authoring captures, splices and replays it.
pub struct ViewRecord(/* the private ranges + the closures/handlers moved out of the frame */);

pub trait ViewRetention: 'static {
    fn begin_frame(&mut self, window: &mut Window<'_>, cx: &mut App);
    /// May this view be replayed? `reads` is what it read when last built; the impl owns the
    /// answer (a dirty set, a consumers map, generations…) and the record it kept, if any.
    fn reuse(&mut self, key: ViewKey, reads: &ReadSet,
             window: &Window<'_>, cx: &App) -> Option<ViewRecord>;
    fn store(&mut self, key: ViewKey, record: ViewRecord);
    fn finish_frame(&mut self, window: &mut Window<'_>, cx: &mut App);
}

impl Window<'_> {
    fn capture_view_record(&mut self, f: impl FnOnce(&mut Window)) -> ViewRecord;
    fn replay_view_record(&mut self, record: &ViewRecord,
                          children: impl Fn(ViewKey) -> Option<ViewRecord>);
}
```

This keeps the eleven channels private, makes the orchestrator *shape* selectable (the impl owns the
map from `ViewKey` to `ViewRecord`, so status quo stores nothing, A a tree, B side tables, AB a
tree), and keeps the dirty-child-in-a-clean-parent splice inside authoring, where B's `splice.rs`
case becomes the `children` callback rather than a public buffer operation.

## 6. R1 — the published contracts (resolved)

**The blocker.** `LayoutEngine` and `TextSystem` are published as the swap seams
(`crates/gpui_engine/src/layout.rs:53`, `crates/gpui_engine/src/text_system.rs:33`) with
implementations outside the tree, so widening them incompatibly breaks published crates — the
opposite of the project's aim.

**Resolution: capability methods, added the additive way.** Retained layout is a concept that
*generalises*, so per `layer-stack.md` it earns a place on the trait — but as a provided method, so
no implementor changes:

```rust
// gpui_engine
pub struct LayoutKey(u64);                 // opaque; supplied by the walk

pub trait RetainedLayout {                 // new; morphorm and parley never name it
    fn request_layout_keyed(&mut self, key: LayoutKey, style: &EngineLayoutStyle,
                            rem_size: Pixels, scale_factor: f32, children: &[LayoutId]) -> LayoutId;
    fn retain(&mut self, root: LayoutId);
    fn retire(&mut self, root: LayoutId);
    fn relayout(&mut self, root: LayoutId, available_space: Size<AvailableSpace>,
                scale_factor: f32, ctx: &mut dyn MeasureContext);
    fn layout_unchanged(&self, root: LayoutId) -> bool;
}

pub trait LayoutEngine {
    // …unchanged…
    /// This engine's retention capability. Default: none — an engine that lacks it still works.
    fn retained(&mut self) -> Option<&mut dyn RetainedLayout> { None }
}
```

`TextSystem` gets the same shape, defaulted *shims over the carry it already has* (`layout_index` at
`crates/gpui_engine/src/text_system.rs:133`):

```rust
fn begin_text_use(&self) {}
fn end_text_use(&self) -> TextUse;                       // whole-frame carry by default
fn seed_text_use(&self, use_: &TextUse) { /* reuse_layouts(use_.index()) */ }
```

**What each crate does.** `bite-gp-morphorm` and `bite-gp-parley` compile unchanged and simply
cannot *host* a retained mode; Taffy (`crates/gpui_engine_default/src/layout.rs:375`) implements
`RetainedLayout`; authoring asks `layout_engine.retained()` once per frame and a retained mode with
`None` **degrades** to "retain render/prepaint/paint, relayout every frame". Placement falls out —
`LayoutKey`/`RetainedLayout`/`TextUse` in `gpui_engine`, which name no `EntityId`; `ViewRetention`
and its vocabulary in `gpui_authoring`, where `EntityId` and `Window` live. **No vocabulary hoist is
needed.** The price to state plainly: `request_layout_keyed` duplicates `request_layout`, because
the compatible way to add a parameter is a new method.

## 7. Blockers resolved (as designed)

The open items divide into three classes: fatal semantic hazards, frame-isolation invariants, and
the deployment trade-offs. Each resolution below is a design, not code — and where one is taken
from A or B rather than invented, it says so. Most of them are A's own remaining work, listed in
its plan, which is why they read as workable rather than hypothetical.

```
                          ┌── Class 1: semantic hazards (R11, R3, R5)
                          │     invalidation correctness, across the application
                          │
   R2–R13 taxonomy ───────┼── Class 2: frame-isolation invariants (R4, R6, R7, R8, R9, R10)
                          │     replay validity, across the Frame's own channels
                          │
                          └── Class 3: trade-offs and bootstrap (R2, R12, R13)
                                the engine tax, the fallback boundaries, the per-window factory
```

### Class 1: the semantic hazards

**R11 — the `notify` contract, and the one divergence to take.** In an immediate runtime a missing
`cx.notify()` is masked: an unrelated sibling, a reflow or a parent refresh rebuilds the subtree
anyway. Retain every view and the same omission leaves that subtree serving its last frame — a
stale region, not a permanent freeze, since any full refresh clears it, but stale until then. A
and B split here, and it is the one place they truly do. A makes it a decision: *"The engine does
not compensate for missing notifications (no revision counters, no 'any `update` is a change'
rule), because that rule cannot tell a read-only `update` from a mutation"* — missing
notifications are bugs, found with the oracle. B takes the opposite reading: an
`entity.update(..)` outside a draw counts as changed *even if nobody notified it*, because
upstream would have re-rendered the reading view anyway. **AB takes B's sensor deliberately.**
The cost is re-rendering on a read-only `update`; what it buys is bug-compatibility with view
code written against upstream. It is a trade, not a free guarantee — B's rule still cannot see a
mutation made through shared interior mutability, or through a channel that is neither `notify`
nor an observed `update`, and B documents that residue: anything outside entities, globals and
list or scroll state *"has to be notified of"*.

**R3 — the ambient inputs.** Focus, window activation, viewport size, mouse position and hover are
read imperatively while an element builds, so they are in no read set, and A measures the stopgap:
*"A focus change, hover change or resize still calls `window.refresh()` today, so those frames
rebuild everything; they cost what every frame cost before the engine."* It is the largest source
of full rebuilds, and the coarse fix is wrong — one window-wide generation would make every focus
or resize a full-window rebuild, a refresh wearing a generation. **Resolution (A's own sketch, and
its list):** an `AmbientInput` read set per node — `Focus`, `WindowActive`, `ViewportSize`,
`MousePosition`, `Hover(HitboxId)` — recorded through a `Cell`, since the readers take `&Window`,
and expanded dirty-then-ancestors exactly as an entity read is. Hover needs two mechanisms
because a mouse move is not the only way the set changes: a **mouse move** diffs the hovered
hitbox set against the previous frame (hitbox ids are stable across a replayed frame) and dirties
only the readers of hitboxes that entered or left it — which is what retires the `refresh()` in
`div`'s hover listeners; a **relayout under a still mouse** moves what is under the cursor, so the
post-prepaint hit test runs the same diff and schedules a frame. A modality flip dirties every
hover reader. Without the second mechanism hover tears exactly where a clean parent shifts a child
under a stationary pointer. A third bucket catches the rest — bounds, content mask, text style,
rem size, scale, opacity, image cache — and is compared before reuse (A's `ViewNodeCacheKey`;
this stack's is `ViewElementCacheKey`, `view.rs:302`).

**R5 — frame-arena handles and carried measurement.** `BoxedMeasureFn` is `'static`
(`crates/gpui_engine/src/layout.rs:45`), so a measure closure cannot hold a borrowed arena
reference and a dangling pointer is not the hazard. The hazard runs the other way: the per-draw
arena is cleared by flipping a validity flag, so a closure kept across a frame and reading
arena-backed data sees invalidated memory, not freed memory. **Resolution — both halves, and they
complement each other.** Carry the *result*, not the closure: B states the rule for a measured node
— *"when last frame's text element at the same place measured the same text, runs and text style,
the new element takes a copy of that measurement and the node is left clean"*, because a measured
node *"given a new closure would be dirtied every frame, with every node above it"*. A adds the
classification for a closure that cannot be detached: a **frame-bound** node (`frame_bound_nodes`)
is excluded from `reuse_layout`, never carried past its frame, and rebuilt next frame — which is
how A first handled deferred draws before it made them roots. A carried measurement is therefore
always a value.

### Class 2: the frame-isolation invariants

Each is a channel of the frame, and each resolution is cited to the mode that has it.

| # | invariant | failure mode | resolution (as designed) |
| --- | --- | --- | --- |
| **R4** | occurrence identity | two mounts of one `Entity<V>` take the same element path — `ViewElement::id()` is `self.entity_id.map(ElementId::View)` (`view.rs:313`) — so they share one `(GlobalElementId, TypeId)` entry in the `Frame`'s single element-state map (`window.rs:998`) and cannot keep separate state or subscriptions | split **mount identity** from **state identity**: `View::element_id()` says where a node mounts and `View::entity()` which entity backs it; the mount is found again by the occurrence `(element path, parent node, nth)`, identity within a node being `(node, Location::caller(), nth)` overridden by an explicit key. A repeated mount of one entity goes to the next occurrence, and element state is per node |
| **R6** | orphan layout roots | a subtree laid out as its own root inside a node (`layout_as_root`, `element.rs:246` — list rows, editor blocks, the measured row of a `uniform_list`) hangs off no child list the node retains, so retiring the node's root never reaches it and it leaks one Taffy tree per item per frame | A found this by review and fixes it at layout time rather than by a list to retire: `Window::compute_layout` marks such orphan roots as **frame layout**, and `TaffyLayoutEngine::finish_frame` removes them as subtrees, stopping short of another node's retained root and never touching a tree a node painted with (`layout_trees_measured_inside_a_node_do_not_accumulate`) |
| **R7** | dispatch identity and deferred context | the dispatch tree's ids are per-frame. Upstream remaps a replayed subtree by an offset delta, which A documents as insufficient once empty nodes are elided and a child owns a nested range. And a `DeferredDraw` (`window.rs:981`) can only render in the ambient context it was attached in | A records a node's pushes and pops as a contiguous range of the live `dispatch_tree.nodes`, snapshots the non-empty nodes after paint, and resolves each parent to a kept node of the scope (`DispatchParent::Recorded`) or the scope's attachment point (`DispatchParent::Attachment`); reuse pushes them back under the active node. A **node-owned dispatch tree with stable ids** removes the snapshot altogether and is A's own follow-up — cleaner, but *"not needed for correctness or the measured performance"*. The deferred root must *capture the ambient context it was attached in* — element-id stack, text-style stack, rem size, content mask, offset and dispatch parent — and re-render there |
| **R8** | text-cache ownership | carrying text per view needs the cache to keep, per node, the `(key, layout)` pairs it looked up, not the whole-frame carry the trait's default shim gives (`crates/gpui_engine/src/text_system.rs:133`) | A makes the node the unit: each phase of a node holds its `TextUse` — including text shaped inside a Taffy measure closure, attributed to the node that requested the measured layout, since measuring runs outside the traversal — and a redraw seeds it back into the frame cache first; the cache stays one previous frame deep. The follow-up is to record the uses on the window's traversal stack as slices of one frame-level vector, so the `Arc` handles are moved rather than cloned and dropped per node |
| **R9** | input handler / IME on replay | `Frame::input_handlers` (`window.rs:1006`) holds the platform's `PlatformInputHandler`s; a replayed view whose subtree owned the focused handler would leave the platform pointing into a frame that is gone | resolve **dynamically**: A has the recordings own their handlers, leased out of their slot for the length of a call, and `PlatformInputHandler` *"resolves the rendered frame's input handler through its context on every call"*, so a replayed view moves nothing — it changes which handler the context resolves to |
| **R10** | atlas / tile lifetime | a replayed sprite can reference a tile the atlas released between frames — the crash behind Zed's atlas work | this one is **R1-shaped, not a pure invariant**: it needs an additive engine-SPI change, and there are two independent halves. Zed merged the renderer half — `#64623`, backends *skip a sprite whose texture was released instead of panicking* — while its `Window`-lifetime companion `#64619` was closed unmerged. A's branch does not ref-count tiles either: it forces a full refresh when an image is evicted, and lists *"audit for the same eviction pattern"* over GPUI's per-frame-use caches as remaining work. AB takes ref-marked tiles as a requirement on `PlatformAtlas` (`crates/gpui_engine/src/atlas.rs:56`) |

### Class 3: the trade-offs and the bootstrap

**R2 — the ship decision, and the shape of the hook.** This is the one still open, and it is the
user's call. A retention mode is not a *swap* in the sense of
[`decisions/0001-no-third-swap.md`](../decisions/0001-no-third-swap.md) — it is not a published
implementation of a published trait — but it is exactly the change `layer-stack.md`'s test says a
new use should not need, so adopting it *in* the stack re-opens 0001 while shipping it *out* of
tree (a `bite-gp-retained` the stack does not name, with the stack carrying only the seam and
`Immediate`) does not. Either way the hook follows the bootstrap seams already there:
`with_layout_engine` (`crates/gpui_runtime/src/application.rs:92`) and `with_frame_pipeline`
(`:107`) are factories, the first taking no window and the second a `WindowId`, and retention is
stateful per window — so it is the second shape, one instance per `WindowId`:

```rust
impl Application {
    pub fn with_view_retention(
        self,
        factory: impl Fn(WindowId) -> Box<dyn ViewRetention> + 'static,
    ) -> Self;
}
```

**R12 — the inspector and accessibility.** The interim is what the mechanisms already do: A's
`.cached()` bypasses itself while the inspector is picking (`is_inspector_picking`,
`window.rs:7493`), and B lists the same four things that are never drawn from the last frame — a
refresh, a drag, the inspector picking, accessibility active. **Resolution:** keep the fallback as
the interim — return `None` and rebuild in full while either is active — and take A's plan as the
target, since A names deleting both: stable per-mount ids with partial accessibility
`TreeUpdate`s, and inspector overrides modelled as reactive entities so an edit dirties exactly
one node.

**R13 — the tax, and where retention is allowed to sit.** A's cost model, measured on A's branch
and not this stack's: an all-dirty frame pays roughly `0.55 µs × dirty nodes + 0.08 µs × dirty
elements` — the per-node cost (occurrence lookup, cache key, three phases of begin/end, dependency
recording, the dispatch snapshot, layout retention) flat in node size, and the per-element cost
0.06–0.11 µs (a hitbox item, a primitive-kind byte, a text line's handle). It is paid on all-dirty
frames too, which is why the tax is invisible where nodes are many and trivial (`Siblings/all
dirty`) and only partly returned elsewhere. **Resolution — a boundary, not a universal:** pin
retention to `Entity<V: Render>`, the view boundary, so the per-node tax is paid once per view and
not per element, and the fine-grained leaves (`div`, text spans, `RenderOnce` closures) stay
transient. Memory follows the boundary: a trivial leaf node weighs about 2.5 KB, ~680 B of it the
`ViewNode` struct, and a 3-pane workspace holds about 20 nodes — tens of KB, not the megabytes a
per-element store would cost.

## 8. The resolution matrix

The four modes are corners of the axis space in §4, with the two Class 2 channels on which they
differ beyond the axes.

| mode | `ViewRetention` | `Reactivity` | `RetainedLayout` | dispatch (R7) | atlas (R10) |
| --- | --- | --- | --- | --- | --- |
| status quo | `Immediate` | – | – | frame-global ids, offset-remapped | unchanged |
| A — Zed view tree | `PersistentTree` | `StrictNotify` | `NodeOwned` | per-node ranges, snapshotted from the live tree | full refresh on eviction |
| B — gpui-fast | `SideTables` | `UpdateGenerations` | `KeyedPathHash` | spliced stretches in the frame's buffer | not addressed |
| AB — combined | `PersistentTree` | `UpdateGenerations` | `NodeOwned` | node-owned tree, stable ids | ref-marked tiles |

Two things the matrix is careful about. **The dispatch column is where AB goes past its sources**:
node-owned dispatch ids are A's own follow-up item, and A says they are *"not needed for
correctness or the measured performance"*; AB takes them because stable identity is what the
spine is for, not because either source needed them. **The atlas column is mostly negative**:
neither A nor B ref-counts tiles today — the merged Zed fix is the renderer skipping a released
texture, which hides the symptom rather than the lifetime — so AB is the first of the four to make
the atlas obey retention.

## 9. Where this leaves the thread

The recorder (R0) and the published contracts (R1) were the two that looked fatal and are not: one
resolves by publishing an opaque token instead of the frame slice, the other by adding
capabilities additively. The items that remained (R2–R13) are now resolved *as designs* rather
than listed as unknown, and the shape of the resolution is the point: every one is either a
per-input dependency (Class 1), an invariant about replaying a channel of the frame (Class 2), or
a deployment choice (Class 3). None is expressible at an existing seam, which is why they are
work to implement rather than a trait to publish.

What is *not* resolved is R2 — the ship decision, 0001's question aimed at a fourth candidate.
Until it is taken, [`issues/0005-view-retention.md`](../issues/0005-view-retention.md) still reads
as "adopt, defer, or reject", and the fork-in-step remains the option that needs none of this:
A's resolutions are work A has begun, and B ships without any of them.
