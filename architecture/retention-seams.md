# Retention seams, and what blocks them

- **Status: proposed.** Nothing here is implemented, and the resolutions in §5–§6 are designs, not
  code. This is the *seam* companion to [`view-retention.md`](view-retention.md): that chapter is
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

Five, of which **three are axes** a mode chooses and **two are capabilities** it uses. Full
signatures, and the four-mode matrix they produce, are in the section of [`view-retention.md`](view-retention.md)
§5 that this document grows out of; the axes are:

| axis / capability | trait | layer | choices |
| --- | --- | --- | --- |
| axis | `ViewRetention` | authoring | `Immediate` · `PersistentTree` (A) · `SideTables` (B) |
| axis | `Reactivity` | authoring | `StrictNotify` (A) · `UpdateGenerations` (B) |
| axis | `RetainedLayout` | engine | `–` · `NodeOwned` (A) · `KeyedPathHash` (B) |
| capability | `TextSystem` scopes | engine | shared |
| capability | `Scene` replay + `ViewRecord` | engine + authoring | shared |

| mode | `ViewRetention` | `Reactivity` | `RetainedLayout` |
| --- | --- | --- | --- |
| status quo | `Immediate` | – | – |
| A — Zed view tree | `PersistentTree` | `StrictNotify` | `NodeOwned` |
| B — gpui-fast | `SideTables` | `UpdateGenerations` | `KeyedPathHash` |
| AB — combined | `PersistentTree` | `UpdateGenerations` | `NodeOwned` |

The point of the factoring is the last row: **AB is A's spine with B's sensor**, so it is a
recomposition rather than a third engine.

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

## 7. Blockers still open

Each is real work, and each is already demonstrably needed by one of A or B.

| # | blocker | why it blocks |
| --- | --- | --- |
| R2 | **the ship decision** — `decisions/0001-no-third-swap.md`; the unchanged-upstream-surface aim; a `with_view_retention` bootstrap beside `crates/gpui_runtime/src/application.rs:92`/`:107` | decides *whether* any mode ships, and where |
| R3 | **ambient inputs are not dependencies** — focus, window active, viewport size, mouse position, modality, hover. Today `ViewElementCacheKey` (`view.rs:302`) is bounds, content mask and text style only, and a focus/resize still forces `refresh()` | the largest source of full rebuilds; without it a "clean" view is stale |
| R4 | **per-view element state and occurrence identity** — one `Frame` element-state map, keyed by `GlobalElementId`; repeated mounts of one entity collide | sibling mounts must keep separate state and subscriptions |
| R5 | **frame-arena lifetimes and measure closures** — the per-draw arena means a measure closure may capture arena data and cannot outlive the frame | a record must not hold arena references |
| R6 | **orphan layout roots** — `layout_as_root` inside a node (list items, editor blocks) is reached by nothing the node retains | leaks one Taffy tree per item per frame |
| R7 | **dispatch identity and deferred context** — the dispatch tree's ids are per-frame and replay remaps them; a deferred draw (`window.rs:981`) records the element-id stack, text style, content mask, rem size and offset it was attached in | a replayed subtree must dispatch, and a deferred root must re-render in its captured context |
| R8 | **text-cache ownership** — per-view carry wants the cache single-threaded and node-owned | carrying text per view is not a free addition to the current cache |
| R9 | **input handler / IME on replay** — the platform's handle must resolve to the current frame's focused handler | replaying a view must move its handler correctly |
| R10 | **atlas/tile lifetime** — a replayed sprite must not reference a released tile | Zed shipped two fixes for exactly this |
| R11 | **the notify contract widens to all views** — retaining every view makes a missing `notify` a stale frame everywhere, not just in a `.cached()` view | correctness of the whole application; found with the oracle |
| R12 | **inspector and accessibility full-refresh fallbacks** — `.cached()` disables itself while picking (`window.rs:7493`) | retention must either work under both or keep the fallback |
| R13 | **the tax and memory** — ≈0.55 µs/dirty node + 0.06–0.11 µs/dirty element, paid on all-dirty frames too; ~2.5 KB per node | decides whether a mode is worth installing |

## 8. Where this leaves the thread

The recorder (R0) and the published contracts (R1) are the two that looked fatal and are not: one
resolves by publishing an opaque token instead of the frame slice, the other by adding capabilities
additively. What remains is a queue of correctness work (R3–R12), none of which is expressible at an
existing seam — which is why [`issues/0005-view-retention.md`](../issues/0005-view-retention.md)
still reads as "adopt, defer, or reject", and why the fork-in-step remains the option that needs
none of this.
