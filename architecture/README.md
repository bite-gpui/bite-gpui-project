# Architecture

How the engine is put together: the layers, the boundaries between them, and the data
that crosses each boundary.

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

## The layer map already exists — in the wrong repository

`.tools/docs/architecture.md` answers two of the three things this directory is for.
Its §1 is the layer map: measured per-crate sizes and a dependency graph, including
the two edges that are deliberate (`gpui_platform` → `gpui_engine`, and `gpui_parley`
→ the `gpui` facade). Its §3 is the design rulings that let a new question be answered
by rule rather than by taste — "decorate traits, do not fork them"; "a leaky
abstraction gets an escape hatch, not a new method".

It lives in the tools repository, which by this repository's own boundary is the wrong
home: `.tools/` is about syncing source-of-truth changes, and it is the repository a
contributor working on the *engine* has no reason to open. It is also why the first
draft of this file claimed the layer map was written down nowhere. It was, one
repository over.

**To settle:** move it, split it, or link it. Its §1 and §3 are GPUI architecture. Its
§2 (commit grammar), §4 (release lines) and §5 (where new work goes) are stack
mechanics, and belong beside `.tools/docs/workflows.md` and `rebase-handoff.md`, which
they already cross-reference. A split along that line looks right — but it is a change
in a second repository, it breaks `.tools/docs/README.md`'s *live* index, and it should
be done deliberately rather than as a side effect. Until then this file points at it.

## Still owed here

1. **The bootstrap order, and what is sealed.** Which parts of an application are fixed
   at `Application` construction and which can still vary per window.
   `with_frame_pipeline` takes a factory because a pipeline is per window;
   `with_text_system` takes a shared `Arc` because it is not; `with_platform` is not
   chained onto the builder at all. Nothing states this in one place, and the governor
   investigation needed it repeatedly.
2. **The frame's data flow, with its interception points.** Where a rate, a filter or a
   meter can be inserted — and, more usefully, which of those the current seams can and
   cannot reach. `spi/` answers the second half; this is the map it hangs on.
