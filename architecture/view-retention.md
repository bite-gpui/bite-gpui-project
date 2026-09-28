# Retained view rendering: the two efforts, and a combined design

- **Status: proposed.** Nothing described here is implemented in this stack, and the two
  approaches it documents are other projects' — Zed's and `gpui-fast`'s. It is written so
  the choice can be taken on the two designs as they stand rather than re-derived from
  their PR threads. Why the thread exists, why it is not a seam, and what "adopt, defer or
  reject" would mean are in [`../issues/0005-view-retention.md`](../issues/0005-view-retention.md);
  the reuse the stack already ships, and why it is a different mechanism, is in
  [`reactive-layer.md`](reactive-layer.md) §"The other half: what is *not* rebuilt".
- **Sources.** Zed `#63800` (*"gpui: Incremental view rendering with a view tree"*), its
  infrastructure companion `#64425` (*"…ahead of the view tree"*), and
  `longbridge/gpui-fast#2` (*"gpui: Add Retained Mode for views"*, merged 2026-09-28).
  Both are read at their own documentation, not at their pull-request summaries:
  `crates/gpui/docs/view_tree.md` and `crates/gpui/docs/view_tree_render_path.md` on Zed's
  `refs/pull/63800/head` (tip `3d0b7f0a`, base `5a9b9558db`, 91 commits, 48 files,
  +9212/−1256), and `docs/retained-mode.md` in gpui-fast. A closed, unmerged branch's tip
  is still fetchable as `refs/pull/<n>/head`, which is what makes a closed PR checkable
  rather than only quotable. The code claims about *this* stack live in the issue, against
  `bite_v1.22.0-pre`.

  **One terminology point the engine itself makes.** Zed's view tree is *memoisation*: the
  authoring model is unchanged, and "retained" is used only where Taffy subtrees are
  literally kept across frames (`view_tree.md` §Vocabulary). The thread is named here for
  the outcome — a frame renders less — not for the technique, since the two efforts use
  the word differently.

## How to read this

Sections **2** and **3** are the two designs, each written to stand on its own — read
either alone and it is complete. Section **4** compares them line by line, **5** is the
combined design, and **6**–**7** are what adopting any of it commits this project to and
what is still open. A is written from its branch documentation (`view_tree.md`,
`view_tree_render_path.md`); B from its code and `docs/`.

## 1. The contract every retained engine keeps

Three things hold across both efforts, and any combined design inherits them:

1. **Reads establish dirtiness; `notify` establishes change.** A view that read an entity
   is dirtied when that entity notifies; no property equality decides it. State a render
   reads that is not an entity or a global — an `Rc<RefCell<…>>`, the time,
   `window.modifiers()` — still needs an explicit `notify`, exactly as a `.cached()` view
   does today. **The two efforts split on one clause of this, and it is a real
divergence.** A states it as a contract and refuses to compensate: *"no revision
counters, no 'any `update` is a change' rule, because that rule cannot tell a read-only
`update` from a mutation"* — a missing notify is an application bug the oracle finds. B
instead treats an `entity.update(..)` outside drawing as a change, *"since upstream would
have rendered the view reading it again anyway"*. §4 carries it.
2. **A full refresh is the correctness oracle.** An incremental frame is asserted equal to
   the same state drawn from scratch. Both efforts ship this as a test, not as an
   intention.
3. **The expensive things are kept, not recomputed**: the Taffy subtree under a clean
   view, and the shaped/measured text it holds.

Everything below is the two efforts' different answers to *how* those three are delivered.

## 2. Approach A — the view tree (Zed `#63800`)

GPUI stays immediate-mode; the engine memoizes views. `render` still describes the whole
frame and the tree is still walked top-down — `layout`, `prepaint`, `paint` — but at every
**view boundary** the walk asks the view tree *is this view clean? graft what it drew last
frame; else render it.*

### 2.1 The node and the walk

The engine is `crates/gpui/src/view_tree.rs` (`ViewTree`, holding
`nodes: SlotMap<ViewNodeId, ViewNode>`) plus `view_node.rs` (the node and its output).
Each mounted view *occurrence* is a node — the same `Entity<V>` rendered in two places is
two nodes. A node owns:

- its **Taffy subtree**, kept across frames (`layout: Option<LayoutId>`);
- a **record of what it drew** (`NodeOutput`), per phase in `Vec<OutputItem>` — primitives,
  hitboxes, listeners, dispatch nodes, text (`TextUse`) and element state;
- the **cache key** of the ambient inputs it was drawn under (`ViewNodeCacheKey`: bounds,
  content mask, rem size, scale factor, opacity, image cache, and a 64-bit hash of the
  text style), the entities its render read (`DependencySet`, a `SmallVec<[EntityId; 8]>`),
  and the frame it last painted in (`painted_frame`).

Dirty nodes rebuild; their ancestors rebuild with them; everything else is replayed. A
node is a `SlotMap` key, and per-node dirty / frame-bound / mounted state is a flag plus a
frame stamp rather than three hash sets (`722930e`).

### 2.2 Reads establish dirtiness

Every entity read while a node renders is recorded as a dependency. The node engine owns a
**`consumers` map** — the union of every live node's dependencies — and expands a window's
dirty set through it at the start of each frame, rather than `App::notify` fanning out.
`notify` goes back to notifying only the entity it was handed (`da19363`). Parent nodes
depend on their children through the same graph, which replaced a separate parent walk.
Global reads are tracked through the same machinery.

### 2.3 Nodes are the frame

Each node owns what it drew in one `Vec<OutputItem>` per phase, with `Child` splices where
a child's output belongs. The frame's queries — `hit_test`, mouse listeners, cursor style,
tab stops, input handlers — walk the frame in drawing order, resolving through the node
records. `Frame` shrinks to focus, window-active, the dispatch tree, the deferred-draw work
queue and scene linearization.

The scene is the **rendered frame itself**, so no primitive is copied at paint: a node's
record is the *kinds* it painted, one byte each, in runs split by children and layers,
plus the frame's lane cursors where each run begins. Replaying a clean node copies its
primitives out of the rendered frame at those cursors into the next frame. `Scene::finish`
now index-sorts 12-byte `(kind, index)` keys into the public lanes the renderers read — by
sorting keys and gathering, not by sorting primitives — and keeps a painted-to-sorted
position per primitive so records survive the sort (`f7a3622`, `b02eec7`). Renderers are
untouched.

Two consequences worth naming: a record addresses one particular frame, so **a node is
reusable only if it painted or replayed in the frame just drawn** (a node that prepainted
without painting renders again the next frame, and a skipped draw advances the engine's
frame counter so nothing claims the empty frame); and a record's ranges are resolved
against live lane cursors, not re-serialised.

### 2.4 Roots and deferred content

A frame is an **ordered list of roots**: the window's root view, the roots attached by
`defer_draw` in priority order, then the prompt, drag overlay or tooltip. A deferred draw
records `OutputItem::Root` in its owner's output; rendering the owner emits it, replaying
the owner re-attaches it, and the root lives as long as some drawn output attaches it.
Nothing is recorded outside a node. This is how a popover survives its owner being reused.

### 2.5 Two actors: entities and components

- **Entities** (`Render`) you can refer to, so you manage them, and `notify` is their
  contract.
- **Components** (a new `Component` trait, `render(&self, window, cx)`) are managed for
  you and cannot be referred to: inline by default, in an element-id scope of
  `(type, nth)` so sibling `use_state` never collides, and `.cached()` — which requires
  `PartialEq` — mounts one as a node re-rendered only when its inputs change.

`RenderOnce` is unchanged and un-deprecated here; migrating it to `Component` is a
follow-up. `View` is sealed and doc-hidden — the shape `ViewElement` draws, not a way in.
Keyed local state (`use_state`) is a real entity owned by the node and dropped at unmount.

### 2.6 Nodes are storage, not entities

No refcounts, no observers, no handles: a `SlotMap`. That is what keeps the graph acyclic
and the memory bounded, and it is why `ViewNode` could be shrunk from 1488 to 680 bytes by
storing phase-specific data once (`3d0b7f0`).

### 2.7 The oracle

The reference implementation is **the view tree under full refresh**: an incremental frame
must equal `window.refresh()` from the same state.
`VisualTestContext::assert_incremental_matches_full_refresh` asserts it; `view_tree::oracle_tests`
drives a seeded fixture (nested views, `uniform_list`, wrapped text, focus, hover, scroll,
a deferred popover, resize) and `test_workspace_rendering_stress` drives a 3-pane
workspace. It fails within two steps if root re-attachment on graft is broken.

Zed itself needs no changes: no existing test changes outside `gpui`, `gpui_macos` and
`benchmarks`.

### 2.8 Cost, and the two bugs it found

The published cost model is **≈ 0.55 µs per dirty node** (flat in node size) **+ ≈ 0.06–0.11
µs per dirty element**, less what retention saves. Both slopes are linear across a 16–32×
range of N. Every Zed-shaped fixture comes out ahead, including the all-dirty ones
(`Workbench/update/row` −64%, `…/editor` −42%, `…/mixed` −43%, `…/full` −23%,
`editor_render` −2.2%). The two synthetic sweeps are reported as microseconds per node and
per element, not as a frame percentage, because neither shape is how GPUI is used.

Two bugs the review found and fixed, both worth carrying into any design:

- **Orphan layout roots.** A tree laid out with `layout_as_root` inside a node (a list
  item, an editor block, a measured `uniform_list` row) is not reached by retiring the
  node's retained root, so it leaked one Taffy tree per item per frame until the next full
  refresh. The fix marks an orphan root computed inside a node as *frame* layout, and
  `finish_frame` removes frame layout as subtrees, stopping at other nodes' retained roots
  and leaving alone a tree a node painted with (`0eb3003`).
- **Deferred-root dependencies.** An entity read only while a deferred root (a popover)
  paints stopped invalidating the window once the owner was clean and reused. The window's
  tracked entities now come from the view tree's `consumers` map (`da19363`).

### 2.9 What `#64425` extracted ahead of it

The view tree was deliberately preceded by a PR that lands independently: scene primitives
in paint-order lanes with compact `(key, index)` sort and one gather of renderer lanes at
frame completion; platform text input resolved through the current frame so a platform
handle does not retain a stale handler; `Hash` for `TextStyle` with signed-zero
canonicalization; group hitboxes moved from an `App` global to frame-local `Window` state;
global-write counting in profiler builds; process RSS in `BenchReport`. Main's `.cached()`
path keeps its behaviour through a lightweight operation stream. The parts that cannot
stand alone — dispatch snapshot/replay, retained Taffy roots, node output integration,
`ViewTreeStats`, the oracle and benchmark fields — stay with the view tree.

### 2.10 What it adds to the public surface

Measured against the crate's exports: `gpui::Component` (a new public trait; `.cached()`
on it requires `PartialEq`) and `ViewTreeStats` with `Window::view_tree_stats()`.
Everything else is `pub(crate)`: `view_tree` and `view_node` are internal modules, and
`View` stays sealed and doc-hidden, so the upstream *implementor* surface (`Element`,
`RenderOnce`, `IntoElement`) is unchanged. The test-only oracle
`VisualTestContext::assert_incremental_matches_full_refresh` sits behind `test-support`.
That is the whole addition — the cost of A is one new trait, not a rewrite of the ones
that exist.

### 2.11 The render path, step by step

`view_tree_render_path.md` is the companion to this section; the mechanics are these.

A view becomes an element through `ViewElement<V>`. `V: View` is the sealed trait behind
`Entity<T: Render>`, `AnyView`, `Component` and `RenderOnce`; a view with an `element_id()`
mounts as a node, one without renders inline.

**A frame** is `Window::draw`: it turns the invalidator's notified entities into dirty
nodes (`invalidate_entities` → `ViewTree::invalidate_entities`, expanded through
`consumers`), opens the frame (`begin_view_tree_frame` — a full refresh marks every node
dirty, and if *every* node is dirty the whole Taffy tree is dropped, since nothing could be
grafted anyway), draws the roots, closes it (`finish_view_tree_frame`: reconcile the roots,
retire the layout trees the frame finished with) and swaps `rendered_frame`/`next_frame`.

**One node, three phases.** `ViewElement::request_layout`, `prepaint` and `paint` each ask
the tree the same question at their top:

- **layout** — build the cache key without bounds, `begin_node_occurrence` (find the node by
  its occurrence — the running element-path hash, parent and nth — or create it, splice a
  `Child` item into the parent's output and push it on the traversal stack), then
  `reuse_layout`. A graft returns the node's `layout` and runs nothing; otherwise
  `restart_node_render` (reseed the node's text, reset its output) and
  `cx.track_reads(render + request_layout)`, after which `store_layout` returns the previous
  Taffy root and `retire_layout` drops its subtree.
- **prepaint** — the key now has bounds, and a graft additionally requires
  `retained_layout_unchanged` (the node's root `taffy::Layout` equal to the one recorded
  when it last drew), so a box that moved renders after all. `graft_view_node_prepaint`
  replays the node's recorded dispatch nodes and re-attaches the roots it deferred;
  `finish_node_phase` runs `reconcile_children`, unmounting what did not come back.
- **paint** — a graft is `graft_view_node_paint` (`ViewTree::replay_scene`, copying the
  record's primitives out of the rendered frame); a render is `begin_view_node_paint`,
  `element.paint`, `finish_view_node_paint` (store the scene, snapshot the dispatch nodes),
  then `store_node_render` — which records the root layout, diffs the read set into
  `consumers`, and clears the node's dirty flag.

**Queries** — `hit_test`, `mouse_listeners`, `cursor_style`, `focused_input_handler`,
`tab_stops`, `prepaint_tooltip` — are walks over the frame's roots in drawing order,
descending into `Child` splices (`ViewTree::walk` / `walk_rev`). On the 512-node fixture a
walk is 0.15–0.2% of the frame; on a real window (10–30k items) ~10–30 µs, two or three per
mouse event.

## 3. Approach B — retained views and layout nodes (gpui-fast)

**This is not one pull request.** `longbridge/gpui-fast` is an experimental fork built over
six PRs (`#2`–`#7`) and a longer run of direct commits — largely by `sunli829`, the author
of Longbridge's GPUI Kit — that added the mechanism in pieces: `memo` first (`ea60e66`),
then a retained-view and cached-view path that took over memo's layout-node keeping and
hover checks (`e0b6381`), a *balanced* bounds tree (`887a2c1`), a glyph run rendered once
rather than once per glyph (`07f880a`), a native font kept per size instead of per line
(`a9a9533`), element-id paths hashed once when the id is made (`ffbebf2`), inspector ids
built only while the inspector is open (`afbd11e`), and the incremental-vs-scratch oracle
(`bae418d`). `#2` (*"Add Retained Mode for views"*) is the merge that pulled the `fast/`
shape together, and `#7` is the later refinement of the splice. The current `fast/` tree no
longer contains `memo.rs`: memo was folded into the retained machinery and its public API
deleted.

Two rules, and they shape every line: **GPUI's existing API stays unchanged** — the public
API is upstream's, so this changes how GPUI draws, not what it offers — and **GPUI's own
code changes as little as possible**, so a new upstream is a merge rather than a port.

### 3.1 Retain every view, not just cached ones

Every view — any `Entity<V: Render>` or `AnyView` in the element tree, **cached or not** —
is drawn again from the last frame while nothing it depends on changed. It is then neither
rendered, laid out, prepainted nor painted. Its layout comes from the nodes it kept; its
hitboxes, listeners, dispatch nodes and primitives are copied from the last frame. Nothing
about how views are written changes. The model is QuickGUI's dependency-scoped components —
a subtree is drawn again from what it drew while nothing its render read has changed —
carried into GPUI without changing GPUI's API or how views are written.

Nothing is reused in a frame the window is rebuilding anyway: a `Window::refresh` (and
what forces one — a resize, a focus change), an active drag, the inspector picking, or
accessibility being active. Those frames cost what every frame cost before.

### 3.2 What a view depends on: three logged kinds, with generations

`fast/dependencies.rs` records, per retained subtree, a `RenderDependencies`:

```rust
struct RenderDependencies {
    entities: Rc<[EntityId]>,
    globals: Rc<[TypeId]>,
    states: Rc<[(StateVersion, u64)]>, // shared state, at the version it was read at
    generation: u64,                   // the global generation when recorded
    updates: u64,                      // the entity update generation when recorded
}
```

- **Entities** — every `EntityId` accessed while the subtree was built (rendered, laid out,
  prepainted, painted), including models it read without observing them and the views nested
  in it.
- **Globals** — every global `TypeId` read, against a monotonic `global_generation`, so a
  write after the recording counts.
- **Versioned shared state** — a `StateVersion`, an `Rc<Cell<u64>>` a `ScrollHandle` or
  `ListState` bumps when it changes, recorded with the version read. This is what catches
  "the scroll moved" when nothing was notified, and `ListOffset::moves_from` exists so a
  scroll to where it already is changes nothing.

Recordings **nest**, and each returns `all` (everything read while it was open) and `own`
(everything outside the recordings nested in it), so a reused child's reads are attributed
to the child rather than to the parent replaying around it.

Two rules about updates are why this is not simply "what it read":

- an **`entity.update(..)` outside drawing** stamps an update generation even with no
  `notify`; a retained subtree that read it is rebuilt, as upstream would have rebuilt the
  view reading it;
- a **`notify` with no update** (a scroll wheel, a dragged scrollbar, an animation asking to
  be drawn) draws the notified view again but *not* the views that read it, since it holds
  what it held. A notify *during* drawing — a view changing a model it reads as it renders —
  does count as an update, because nothing else can tell that it changed.

### 3.3 A record per retained subtree, copied and shifted

Each frame keeps a record per retained subtree: where its hitboxes, dispatch nodes,
listeners and primitives went, what it read, the hovers it was painted by, and the layout
nodes it holds. Drawing one again copies its record *and the records nested in it*, shifted
to where the copy landed, so a nested subtree stays reusable on its own when what surrounds
it has to be built again — a rebuilt parent no longer rebuilds everything under it.

The scene keeps upstream's operation stream (`fast/scene.rs` is test-only helpers; the
`Scene` and its `PaintOperation` stream are still upstream's), and what is *replayed* is the
ordering: a **balanced** bounds tree (`fast/bounds_tree.rs`) hands last frame's orderings
back while the bounds come in the same. Upstream builds that tree from scratch each frame
and leaves it unbalanced; inserting a reused subtree's primitives into it made most of a
still frame (60 panels × 64 labels: 3.3 ms retained against 0.31 ms with the orderings
replayed). This is B's answer to the problem A solves with lane cursors, and a different
one: the operation stream stays, the bounds tree is what is kept.

### 3.4 The splice

A view that is dirty **only because a view nested in it** was notified — it was not
notified itself, nothing it read changed, it is hovered as it was — is drawn from the last
frame *around* the nested view, and the nested view is built again in the gap, at its own
layout nodes and with what it inherited there (`fast/splice.rs`). If the nested view then
asks for another layout, the view around it is built after all, *taking over the nested
view already built* rather than building it twice. This is the case a per-subtree record
cannot replay wholesale, and the splice is the buffer surgery that patching it requires.

### 3.5 Layout nodes, and the key that finds them

Upstream clears the whole Taffy tree at the end of every frame. B keeps nodes across frames
and writes a node's style, children and measurement only when they differ, so Taffy's own
per-node cache survives. An element finds its node again by a **`u64` key that is a running
hash of its path from the root** (`fast/layout_key.rs`): each element mixes in its
`ElementId` (hashed) when it has one, or its index among its unidentified siblings
otherwise, and the parent's key is always mixed in, so the key encodes the whole ancestor
path and a node can never be matched to an element that moved to a different parent. A
`SplitMix64` finalizer keeps small child indices from colliding in practice.

Two things that path does not cover, and how they are handled:

- **Things laid out during prepaint** — list items, which a list can size only once it knows
  how many fit — arrive with no path at all. A *prepaint-scope* key (salted so it cannot
  collide with the same element's children) hangs them off the element that laid them out,
  and `AnyElement::layout_as_list_item` additionally keys an unidentified list item by its
  index, so scrolling by one row does not hand every item its neighbour's nodes.
- **The identity rule is upstream's element-state bargain**: an identified element keeps its
  node when siblings are inserted or reordered around it; an unidentified one is matched by
  position.

So B *does* derive `u64` layout keys by hashing the path — which is the point A's
node-owned `LayoutId` removes. See §5.1.

### 3.6 Text measurement carry-over

Text measurement is keyed by shaping (with recoloring handled in place), the line layout
cache carries over, and shaping is counted per frame (`fast/text.rs`). A text element whose
text, runs and style did not change takes over last frame's measurement and leaves its
node clean — so a view that *is* built again is still not laid out again, and re-measuring
alone no longer asks for an extra frame.

### 3.7 The switch, and the behaviour changes

`Window::set_view_retention(false)`, or `GPUI_VIEW_RETENTION=0` for every window, draws
every view every frame — for comparison and debugging. The behaviour changes are the price
of "every view depends on what it read":

- an entity updated outside drawing (`entity.update(..)`) counts as changed even without a
  `notify`, as upstream would have rendered the reading view again — and conversely, an
  entity *notified without being updated* draws itself again but does **not** draw the
  views that read it, since nothing they hold changed;
- cached views are *also* rebuilt when an entity or global they read changes;
- `ListState` and `ScrollHandle` bump a version when their state changes;
- `div` carries opacity through prepaint; window-control hitboxes are copied with reused
  paint; `VisualTestContext::draw` draws from scratch (an oracle).

### 3.8 Correctness and what it measured

The oracle drives two windows through the same random history — one incremental, one from
scratch — and requires every frame to match (24 seeds × 60 steps), now covering uncached
sibling, nested and deferred views, a model read without being observed, and a global; it
also asserts views really were reused, and it fails if a dependency check is disabled.
`gpui_perf --verify` compares the quads both modes paint on every frame across 18
scenarios (text sprites and paths are not compared, since nothing public exposes them).

Real-window, against the `gpui-pre 0.3.7` snapshot, scrolling at 32 px/frame: idle −94%,
sidebar −84%, component page −76%, data table −63%, periodic refresh −81%, list −66% CPU.
Headless, 60 panels × 64 labels: still 10.43 → 0.25 ms (−98%), one panel −88%, six −70%,
all sixty −20%. Of 18 headless `gpui_perf` scenarios, none is slower; three gain little,
and it names why — `table-virtual-scroll` (the scrolled rows are not views and move every
frame), `list-live-updates` (every view reads the one model that changes), and
`settings-toggle` (≈4 ms of Taffy layout, a layout-engine follow-up).

### 3.9 The `fast/` shape, and how it stays mergeable

`fast/` holds the addition by topic — `retained.rs`, `dependencies.rs`, `layout.rs`,
`layout_key.rs`, `splice.rs`, `text.rs`, `stats.rs`, `bounds_tree.rs`, `scene.rs` (test
helpers) and `tests/` — and `fast/mod.rs` states the rule the fork is built on: *"Upstream
files only call into this module: a field holding this module's state, a line forwarding a
method to it, a hook at the point something happens."* Nothing is glob-imported or
glob-re-exported from `fast/`; every use names `crate::fast::<topic>::Name`.

`docs/upstream-sync.md` makes that enforceable. `script/check-upstream` compares every file
in the tracked upstream directories against the pinned import and fails on: a file added or
removed under an upstream directory; a hunk adding more than 8 lines, a file adding more
than 40 or removing more than 20; a differing binary; or any glob import from `fast/`. A
justified exception goes in `script/upstream-allowlist` with a reason — typically a hub file
with many one-line hooks, or an upstream body replaced by a `fast/` module that it now
forwards to, e.g. `crates/gpui/src/view.rs removed=210 # ViewElement's cache-by-bounds is
replaced by fast::retained's retained views`. A whole file can be replaced by redirecting
the module (`#[path = "fast/<file>.rs"] mod <name>;`), leaving the upstream file exactly as
upstream has it. Taffy, `Scene` and element state stay upstream's; the *state* the fork adds
lives in a `<topic>` struct held in one upstream field. Upstream is pinned by commit —
`zed_commit = 7960b2a7…`, with `import_commit = 11a44c488…` the commit holding that copy
unchanged — and a sync is a merge whose conflicts should touch hooks only.

Rule 5 is **no new public API**: what tests and `gpui_perf` need to measure or to switch
retention is compiled under `test-support` and exported from `gpui.rs` one item at a time
(`pub use fast::stats::LayoutStats`), and `GPUI_VIEW_RETENTION` is the runtime switch. Later
passes stripped what was not needed — `memo`'s public API, global-id caching, `Keyed` —
which is why the current tree has no `memo.rs`.

### 3.10 The frame, phase by phase

`docs/retained-mode.md` is the companion; the mechanics are these.

The walk is upstream's — **build** (render views, request layout), **prepaint** (compute
layout, place elements), **paint** (turn them into the scene) — with one decision added at
each view boundary. A view placed in the tree (`Entity<V: Render>` or `AnyView`, `.cached()`
or not) is a *retained subtree*: while `RenderDependencies` says nothing it read has changed,
and it is drawn in the same place (bounds, content mask, text style, opacity) with the same
hover state, it is neither rendered, laid out, prepainted nor painted — its record is copied
from the last frame into this one, shifted to where it landed, and the views nested in it
come with it.

The per-frame state lives in `fast/retained.rs`; `fast/stats.rs` counts the decisions for
the tests and `gpui_perf`. A view that *is* built opens a `begin_recording_dependencies`
scope (`fast/dependencies.rs`) and stores what it saw; a view that is *reused* replays its
dependencies into the enclosing recording (`replay_dependencies`), so the parent's
dependency set still covers the subtree the frame did not walk. Layout is keyed by
`fast/layout_key.rs` and gated by `fast/layout.rs` (§3.5); text is carried by
`fast/text.rs` (§3.6); and a view dirty only because a child was notified is patched by
`fast/splice.rs` (§3.4) rather than built around the child. `GPUI_VIEW_RETENTION=0` draws
every view from scratch through the same walk, which is how the two modes are compared
(§3.8).

## 4. Where they agree, and where they diverge

| | A — view tree (Zed) | B — gpui-fast |
| --- | --- | --- |
| structural spine | a persistent **view tree** of nodes | **none**; flat per-subtree records |
| what a view's identity is | a `SlotMap` node key, found by an occurrence of `(running path hash, parent, nth)` | nothing durable; records and layout keys are re-derived each frame |
| an `update` with no `notify` | **not** a change — notify is the contract | a change |
| invalidation | per-node dependency set + a `consumers` map (entities *and* globals) | per-subtree "what it read" + bounds/mask/style/opacity + hovers + list/scroll versions |
| a dirty child in a clean parent | replay the parent, re-run the child at its node | the parent replays, the child is rebuilt *in the gap* (`fast/splice.rs`) |
| layout retention | the node owns its Taffy subtree; orphan roots dropped with the frame | a `u64` hash of the element path finds the node, with a prepaint-scope key for list items; a style fingerprint gates the write |
| text | carried per node; reseeding skipped for a node that redrew last frame | carried per shaping; recoloring in place; reshaping counted |
| scene | the rendered frame is the scene cache; lane cursors + painted-to-sorted positions | upstream's operation stream kept; a **balanced** bounds tree hands back last frame's orderings |
| deferred content | a root list; `OutputItem::Root` re-attached on replay | records copied with the subtree |
| public API | adds `Component` (a trait) and `ViewTreeStats`; rest `pub(crate)`, `View` sealed/doc-hidden | adds **none** |
| switch | (none shipped; the design is default-on with a refresh escape) | `GPUI_VIEW_RETENTION=0` / `set_view_retention(false)` |
| oracle | incremental == `refresh()`, asserted in a test context | incremental == from-scratch, plus a per-frame quad comparison |
| where it lives | in-tree, upstream | a fork in step with upstream: `fast/` + one-line hooks, pinned `zed_commit`, budgets in `script/check-upstream` |

The divergence that matters most is the **spine**: A buys node identity and pays for it in
public API and in a frame model where a record addresses one particular frame; B buys "no
public API, minimal upstream diff" and pays for it in `splice.rs` buffer surgery and in
re-deriving identity every frame.

## 5. A combined design

The synthesis below is A's spine carrying B's receipts. It is a proposal; nothing measures
it here.

### 5.1 The spine: a per-view node, and no new public trait

Take A's persistent node — it is what makes the scene cursors stable and lets a node own its
layout root — but keep **B's constraint that nothing is added to the public surface**. A
already stores nodes as engine storage rather than entities (`SlotMap<ViewNodeId, ViewNode>`);
the public cost in A is the new `Component` trait and `ViewTreeStats`, a question separate
from retention.

**On the layout key, the drafting note is right and an earlier correction of it here was
wrong: B *does* derive `u64` layout keys.** `fast/layout_key.rs` mixes each element's
`ElementId` (or its sibling index) into a running parent-and-child hash, so the key *is* the
path, re-derived every frame, with a collision risk the code itself calls out. A instead
puts `layout: Option<LayoutId>` on the node (`store_layout` / `retire_layout`), so the root
is found by identity rather than recomputed — which is what removes the per-frame path
hashing. The symmetry the drafting missed: A *also* hashes the element path, but only for
*occurrence* identity (`Window::element_path_hash`, to find the node); the layout root is
then owned by the node. The synthesis takes the node-owned root and leaves the path hash
only where identity is genuinely per-frame.

### 5.2 Invalidation: per-node dependencies, plus what a node cannot see

Adopt A's per-node dependency set and its `consumers` map (entity *and* global reads,
expanded at frame start). Add the dependency kinds a node's own reads do not capture, which
B had to add explicitly:

- **where it is drawn** — bounds, content mask, text style, opacity (a moved view is built
  again at its kept layout nodes);
- **hovers** it was painted by, and interactions inside it;
- **state versions** — `ListState` and `ScrollHandle` bump a version when their state
  changes, so a scroll that changes nothing else still invalidates.

That is exactly the bug A hit from the other side: a deferred root's reads did not bubble
into its owner's scope. B's answer — record hovers, bounds and versions as first-class
dependencies — is the more complete one, and the combined design takes it.

### 5.3 Layout and text

Keep A's per-node retained Taffy subtree with the *frame-layout* orphan rule (5.2's second
bug in §2.8). Use B's style fingerprint as the test for "same style, children and
measurement" before re-laying out. Carry text per node, keyed by shaping, and carry the
line-layout cache as both do; a rebuilt view whose text did not change is not laid out
again.

### 5.4 Scene composition without buffer surgery

Adopt A's scene — the rendered frame as the node scene cache, lane `(kind, index)` records,
frame cursors, painted-to-sorted positions — which retires B's `splice.rs`: with node
identity and stable cursors there is no flat-buffer surgery to perform. Keep B's finding
about the **bounds tree**: whichever scheme is used, reusing a view must not re-insert its
primitives into an unbalanced per-frame tree, or a still frame goes quadratic (B measured
3.3 ms → 0.31 ms). Replay the ordering.

### 5.5 Where it would live for this project

B's `fast/` shape is *this project's own method*: a stack of forward commits against
upstream, additions in their own files, one-line hooks in the upstream files, a budget
enforced by a script. So the siting answer for `bite-gpui` is B's, not A's — a branch in
step with upstream rather than an in-tree rewrite — and a public trait is out unless it is
decided separately, which is what [`layer-stack.md`](layer-stack.md) §"Decorate traits; do
not fork them" and the *unchanged public surface* aim already require.

## 6. What adopting it would commit

- **The layer-stack test fails by construction.** This is not a swap and not an extension
  trait; it changes the element walk. [`issues/0005-view-retention.md`](../issues/0005-view-retention.md)
  argues this is the ruling to take, not a special case to add.
- **A decision record**, with the shape (§5.5) and the scope (default-on vs opt-in) fixed,
  and the alternatives recorded.
- **A benchmark here, two-column, from `.uses/`.** Note that the *harness* landed upstream
  as `#64842`; the number this project publishes still has to be one this project's own
  bench prints, or the measurement policy in `.uses/README.md` is being waived rather than
  met.

## 7. Open questions the combined design has not settled

Two of these have since got a seam-level sketch in [`retention-seams.md`](retention-seams.md) §8 —
the ambient inputs (there, R3, from A's own plan) and the ambient context a replayed deferred root
must re-capture (R7) — which moves them from "unsettled" to "designed, unimplemented". The rest
are open.

- **Ambient inputs** — focus, window-active, hover — as tracked dependencies instead of a
  refresh. Today (A's own note) a focus change costs a full-priced frame.
- **Fine-grained caching through clean ancestors**, with deferred roots re-rendering in
  captured context.
- **`RenderOnce` and auto-promotion** of stateful components into nodes.
- **Damage regions**, and per-kind output lanes if a profile shows the frame walk costing.
- **The escape hatch.** B ships a runtime switch; a combined design has to say whether
  retention is default-on (A) with `refresh()` as the way out, or behind `GPUI_VIEW_RETENTION`.
