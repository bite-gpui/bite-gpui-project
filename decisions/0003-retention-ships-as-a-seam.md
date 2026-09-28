# 0003 — Retention ships as a seam; its modes ship out of tree

- **Decided:** 2026-09-29
- **Status:** decided
- **Evidence:** [`retention-seam-probe.md`](retention-seam-probe.md)
- **Touches:** [`0001-no-third-swap.md`](0001-no-third-swap.md) (the swap tier),
  [`../architecture/retention-seams.md`](../architecture/retention-seams.md) (§5's trait and §10's R2),
  [`../issues/0005-view-retention.md`](../issues/0005-view-retention.md) (the tracker)

## Decision

**The retention seam ships in this stack. The retaining modes do not.**

A `ViewRetention` seam — the three axes, the two capabilities, the opaque `ViewRecord`, and an
`Immediate` default that reuses nothing — is a fourth *open boundary* beside `LayoutEngine`,
`TextSystem` and `FramePipeline`, bootstrapped by `Application::with_view_retention`.
`PersistentTree`, `SideTables` and the AB recomposition ship out of tree, in a repository of their own.

**`0001` is unamended.** It declines a third swap and a new version slot, and this adds neither: a
boundary whose default does nothing is what `FramePipeline` already is.

## Why

The probe's first finding decides it. Every `Window` field is `pub(crate)`, so the seam's published
surface is the *entire* API a mode gets, and the seam itself cannot be out of tree — it is authoring
types, `Window` methods and an `App` field. R2 was therefore never choosing between an in-tree and an
out-of-tree *seam*; it was choosing whether a *mode* is in tree. A mode is a published implementation
of a published trait, which in this project is the definition of a swap, and swaps are what 0001
bounds.

The rest follows from the shape the seam already has. `retention-seams.md` §5 keeps the record opaque
and leaves capture and replay in authoring, so a mode never names the frame's internals — the items §2
found unnameable outside the crate. What is left for a mode is a store: a node map, a `consumers`
index, a record map, and the two entry points the probe found missing (§3), which belong to the seam
either way.

## Rejected alternatives

| alternative | why not |
| --- | --- |
| ship a retaining mode in tree | it is a swap by 0001's definition and so needs a version slot; and it is the one part of the thread a consumer can take without the stack, since the seam is the interface and a mode is a policy behind it |
| ship the mode as a *wrap* | a wrap decorates a frame pipeline. Retention does not decorate a pass — it replaces the decision at each view boundary, which is why [`../architecture/extension-tiers.md`](../architecture/extension-tiers.md)'s second tier does not reach it. That is the same conclusion 0005 reaches from the walk's side |
| no seam at all — keep `.cached()` and `slot` and stop | that is 0005's "defer". The probe is the argument against it being *forced*: the seam is additive throughout, widens nothing, and does not touch `Frame`. Deferring is still available; it is no longer the only safe answer |
| ship the seam *and* a mode, and let the version slot follow | the slot is the thing 0001 declined to open without a reason, and a seam with a no-op default gives every future mode — including ones not written yet — the reason to open its own |

## What would reopen this

- **A mode needing what the seam cannot express additively** — a widened existing trait, or a change to
  `Frame`. The probe found the two likely candidates (the notified set, the root list) and both resolve
  as new methods. The first instance of one that does not re-opens this record rather than moving a
  mode in.
- **An out-of-tree mode unable to keep up with the seam** — the same tail risk 0001 already records for
  `bite-gp-pass`: a boundary that moves under a consumer. If it bites, the answer is to stabilise the
  seam, not to adopt the mode.
- **Evidence that `Immediate` is not a sufficient default** — that the stack cannot ship the seam alone
  without also changing behaviour for consumers who never opt in. §6's degradation path says it can:
  an engine whose `retained()` is `None`, and a store that always answers `None`, leave the
  immediate-mode frame exactly as it is.
