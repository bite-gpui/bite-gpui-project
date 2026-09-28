# Architecture

How the engine is put together: the layers, the boundaries between them, and the data
that crosses each boundary.

## Documents

| document | what it is |
| --- | --- |
| [`layer-stack.md`](layer-stack.md) | the layer map with measured sizes, the dependency direction, the two deliberate edges, and the rulings that decide where a new item goes |
| [`frame-flow.md`](frame-flow.md) | one frame from the platform asking for it to pixels on the glass, the seam each stage is entered through, and what a pipeline can and cannot reach |
| [`reactive-layer.md`](reactive-layer.md) | how a state change becomes a frame: `notify`, the effect queue that runs observers, the invalidator that decides whether a window is dirty, and the entity-granular cache that decides which subtrees are not rebuilt at all |
| [`extension-tiers.md`](extension-tiers.md) | the two ways a crate extends the engine — a *swap* replacing a seam's implementation, a *wrap* decorating the frame pipeline — and what each can do, cannot do, and commits to |
| [`view-retention.md`](view-retention.md) | **proposed, unimplemented** — retained (incremental) view rendering: the two external efforts in detail, and a combined design |
| [`retention-seams.md`](retention-seams.md) | **proposed, unimplemented** — how retention would attach to this stack: the seams that exist, the traits a seam would need, and every blocker R0–R13 resolved as a design (the ship decision, R2, still open) |

The first four describe the engine as it is. `view-retention.md` and `retention-seams.md` are
the exception: both are **proposed** and nothing in them is implemented here. They are kept
beside the descriptive chapters because the thread is an engine-wide change the layer rulings
have to decide, not a seam a crate can implement — see
[`../issues/0005-view-retention.md`](../issues/0005-view-retention.md).

## Belongs here

- the layer map: which crate owns which concern, and the direction dependencies run
- the bootstrap path: what `Application` wires, in what order, and what is already
  fixed by the time the first window opens
- the frame's data flow — input → reactive state → element tree → layout → scene →
  GPU — and every point at which a stage can be intercepted
- invariants that are load-bearing and easy to break

## Does not belong here

- publishing, staging, naming and versions — `.dist/` (`DESIGN.md`,
  `docs/contract.md`)
- how to *use* a seam — `.uses/`, `.website/`
- per-seam trait contracts — [`../spi/`](../spi/README.md)
- building and releasing the stack — `.tools/docs/architecture.md`, `workflows.md`,
  `rebase-handoff.md`

## Where `layer-stack.md` came from

It is the first half of `.tools/docs/architecture.md`, moved on 2026-09-26. That file
was two documents in one: what GPUI's layers *are*, and how the stack is *built and
released*. Only the first answers anything a reader of this directory is asking, and it
sat in the repository a contributor working on the engine has no reason to open.

The second half — the move/adapt pairing, the release lines, where the work goes next —
stayed there and the file was renumbered around it. One subsection was dropped rather
than moved: its release-version scheme, which `.dist/docs/contract.md` §6 already
specifies in full. A second copy of a fact is a second copy to keep in sync, which is
this repository's own convention.

## Still owed here

1. **The bootstrap order, in the detail the seams need.** What `Application` applies in
   what order, and what is already fixed by the time the first window opens — the
   question that decides whether a seam can still be changed at runtime, and the one the
   frame-pipeline investigation kept running into. [`frame-flow.md`](frame-flow.md) records each
   seam's lifetime but not the order they are applied in.

Everything else this directory was missing now exists: the layer map in
[`layer-stack.md`](layer-stack.md), the frame and its interception points in
[`frame-flow.md`](frame-flow.md), the state-to-frame chain in
[`reactive-layer.md`](reactive-layer.md), and the two extension tiers in
[`extension-tiers.md`](extension-tiers.md).
