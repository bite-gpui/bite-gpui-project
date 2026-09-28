# 0005 — Retained (incremental) view rendering, and where it lands

- **Opened:** 2026-09-28
- **Status:** open
- **Touches:** `architecture/layer-stack.md` (the rulings, and the test they serve),
  `architecture/reactive-layer.md` (the reuse that already ships),
  `decisions/0001-no-third-swap.md` (the swap tier), and — outside this project —
  the Zed view-tree PRs (`#63800`, and its infrastructure companion `#64425`) and
  `longbridge/gpui-fast#2`

## The problem

**Retained view rendering** — drawing a view again from the last frame while nothing it
depends on changed, skipping `render`, layout, prepaint and paint — is being built outside
this stack, in two places, and neither is a seam. Both change how `Window` evaluates an
element tree: a view's output has to be recorded and replayed, its Taffy subtree kept
across frames, and its text measurements carried, which lands in `gpui_authoring`,
`gpui_engine_default` and the reactive layer at once.

That is precisely the case `architecture/layer-stack.md` §"The test the rulings exist to
serve" is written for: a new use should need only published crates and an extension
trait, and *"if it needs a change to the stack, that is the signal to re-open the rulings
rather than to special-case it."* So the question this issue tracks is not "which crate
does retained mode go in" but **whether the stack carries per-view retention at all, and
in which shape** — a ruling to take, not a module to move.

## What already ships here, and why it is not this

Reuse exists in the stack today, and it is a different mechanism. It is opt-in per
entity, through `.cached()` (`crates/gpui_authoring/src/view.rs:43`,
`crates/gpui_authoring/src/view.rs:242`) and `EntitySlotExt::slot`
(`crates/gpui_authoring/src/elements/slot.rs:16`), both funnelling into
`prepaint_cached_view` (`crates/gpui_authoring/src/view.rs:426`).
[`architecture/reactive-layer.md`](../architecture/reactive-layer.md) §"The other half:
what is *not* rebuilt" describes that mechanism and its contract in full; two things about
it matter here.

- **It reuses one node's own subtree, not a tree.** A cache hit replays that view's
  previous prepaint range and re-registers the entities it read; the parent is not reused
  with it, and a plain `.child(entity)` is rebuilt every frame. Incremental evaluation is
  the other direction: a clean parent replayed *around* a dirty child.
- **Its contract needs the subtree to be given a definite size**, because measuring would
  mean building it, which is the thing being avoided. A retained engine has no such
  constraint — it keeps the last frame's measurement instead.

Two pieces a retained engine would consume are already here. The text system carries line
layouts across a reused frame — `layout_index` / `reuse_layouts` / `truncate_layouts` on
the `TextSystem` trait (`crates/gpui_engine/src/text_system.rs:133-139`), implemented over
the line-layout cache (`crates/gpui_engine_default/src/line_layout.rs:150-209`) — and the
frame retains its hit-test tree across the double buffer
(`crates/gpui_authoring/src/_authoring.rs:75`). What is missing is the evaluation that
consults them at each view boundary.

## What has been tried, and where it stands

Two efforts, and they are **two different answers to the same siting question** — which is
the part worth deciding before any code moves. Neither is finished, and the first is being
landed on upstream `main` in pieces rather than in one PR. Each is written up in detail,
along with a combined design, in
[`../architecture/view-retention.md`](../architecture/view-retention.md) — read from the
branches' own docs (`crates/gpui/docs/view_tree.md` and `view_tree_render_path.md` at
`refs/pull/63800/head`, tip `3d0b7f0a`, and gpui-fast's `docs/retained-mode.md`), not from
their pull-request summaries. The seams it would need, and the blockers to each, are in
[`../architecture/retention-seams.md`](../architecture/retention-seams.md).

| effort | where | shape | what it keeps |
| --- | --- | --- | --- |
| **Zed view tree** — `#63800` *"gpui: Incremental view rendering with a view tree"* (mikayla-maki) | upstream, in-tree | a persistent view tree the element walk consults at every view boundary | per-node Taffy subtree and a record of what it drew, per phase; a frame as an ordered list of roots; the rendered frame as the scene cache |
| **gpui-fast** — `#2` *"Add Retained Mode for views"* and `#3`–`#7`, over a commit series by `sunli829` (huacnlee) | a fork kept in step with upstream | `fast/` modules beside upstream's files, one-line hooks in them, `script/check-upstream` enforcing per-file budgets against a pinned `zed_commit` | every view (not only cached ones) plus layout nodes; the parent around a notified child is patched (`fast/splice.rs`); text measurement carry-over; `GPUI_VIEW_RETENTION=0` to turn it off |

**The Zed view tree is a draft that was split for landing, not an abandoned approach** —
`#64425` (*"scene lanes, input handler fix and bench harness ahead of the view tree"*,
which exists precisely to *"extract the GPUI infrastructure that can land independently
ahead of #63800"*) is the same. The split is visible on upstream `main` as granular
patches the view tree needs and that stand on their own:

- **`#64753`** — `bench_metrics`, multi-metric Criterion benchmarks (Sep 27);
- **`#64843`** — seeded `#[gpui::bench]` benchmarks and heap-allocation metrics (Sep 28);
- **`#64842`** — a randomized element tree for measuring *"what one frame costs under each
  class of change"* (Sep 28), whose own summary states the finding the view tree exists to
  change: *"on `main`, every class of change costs about the same as a full refresh… that
  is the curve retained rendering should bend"*;
- **`#64718`** — Linux headless rendering in `gpui_wgpu` (Sep 25), which a headless render
  benchmark needs.

So the **measurement apparatus for this thread is already on upstream `main`**, and it
measures the very thing the view tree is meant to move. That bears on §"The number not to
carry" below, and on what "defer" would mean: the curve those patches print is upstream's,
not this stack's.

What the two share is the contract a retained engine has to satisfy: reads recorded during
render establish dirtiness (`notify` still establishes change), Taffy subtrees and text
measurements are kept, and a window refresh is the correctness oracle an incremental frame
is asserted equal to. What they differ on is where the change lives — upstream itself, or a
fork upstream can take piece by piece — which is the siting question this project's
rulings answer by rule rather than by taste.

## Why this is not a swap, and what that costs

The nearest seam is `FramePipeline`, whose passes run `evaluate_roots` → `layout_roots` →
`paint_roots` (`crates/gpui_authoring/src/window/frame_pipeline.rs:60,70,75,87`) behind
`draw` (`:122`). A wrap on it can **defer** a frame — `should_render` is asked first
(`crates/gpui_authoring/src/window/frame_pipeline.rs:45`) — but it cannot reuse a subtree:
`evaluate_roots` hands it an element tree that has already been built, so there is no node
to replay by the time a pass runs. Reuse needs the walk itself to consult a node at each
view boundary, and the walk is inside `Window`, not behind a trait — which is why
`gpui-fast`'s diff touches `window.rs`, `view.rs`, `elements/div.rs` and `taffy.rs`.
`architecture/extension-tiers.md` says the same from the tier side: a wrap changes no
algorithm, and is only as transparent as the passes it forwards.

So per-view retention is an engine change by construction, and the rulings it implicates
are `architecture/layer-stack.md`'s aim and test, read together with
[`decisions/0001-no-third-swap.md`](../decisions/0001-no-third-swap.md) — the decision that
the swap tier stops at two. It does not *contradict* 0001 (a retention engine is not a
third swap; it is not a swap at all), but it is the first use raised that the test in
`layer-stack.md` cannot satisfy, which is the test's own trigger to re-open.

## The number not to carry

The frequently quoted "60% → 15% CPU while scrolling" is **`gpui-fast`'s own two-column
figure, on `gpui-fast`** (its README and PR #2), and the view tree's are paired criterion
runs on its now-deleted branch. Neither is a measurement this stack has produced, and the
harness that would produce one now exists upstream (`#64842`) rather than here.
[`../../.uses/README.md`](../../.uses/README.md) §"Measurement policy" already rules on this shape:
a number that appears in public has a committed, two-column benchmark here that produces
it, with the run recorded. If the stack adopts retention, that benchmark has to be written,
not quoted.

## What would close it

**One of three, recorded in [`../decisions/`](../decisions/README.md):**

1. **Adopt.** Choose a shape — a minimal engine change behind the existing
   `.cached()`/slot path, or a `fast/`-style branch kept in step with upstream — and carry
   a two-column benchmark alongside it.
2. **Defer.** Leave it out of the stack and state the evidence that would change that: a
   consumer that needs it, or the view tree completing its landing upstream — whose
   prerequisites are already on `main` (see above).
3. **Reject.** Record that per-view retention is deliberately not the stack's work, and
   why, so it is not re-argued.
