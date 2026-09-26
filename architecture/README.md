# Architecture

How the engine is put together: the layers, the boundaries between them, and the data
that crosses each boundary.

## Documents

| document | what it is |
| --- | --- |
| [`layer-stack.md`](layer-stack.md) | the layer map with measured sizes, the dependency direction, the two deliberate edges, and the rulings that decide where a new item goes |

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

1. **The bootstrap order, and what is sealed.** Which parts of an application are fixed
   at `Application` construction and which can still vary per window.
   `with_frame_pipeline` takes a factory because a pipeline is per window;
   `with_text_system` takes a shared `Arc` because it is not; `with_platform` is not
   chained onto the builder at all. Nothing states this in one place, and the governor
   investigation needed it repeatedly.
2. **The frame's data flow, with its interception points.** Where a rate, a filter or a
   meter can be inserted — and, more usefully, which of those the current seams can and
   cannot reach. [`../spi/`](../spi/README.md) answers the second half; this is the map
   it hangs on.
3. **The seam inventory as a diagram.** `spi/` lists the five boundaries as a table,
   which says what each one is but not how a frame moves through them. That is the
   picture a new contributor needs first, and it is the one nobody has drawn.
