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

## Owed

Documents this section needs, in the order they were missed:

1. **The layer map.** The crate *set* is public (`targets.toml`) but the reason for
   each boundary is written down nowhere. Reading six `Cargo.toml` files to find out
   why `gpui_engine` exists separately from `gpui_authoring` is a tax on every new
   contributor.
2. **The bootstrap order, and what is sealed.** Which parts of an application are
   fixed at construction and which can still vary per window. The governor
   investigation needed this repeatedly: `with_frame_pipeline` takes a factory because
   a pipeline is per window, `with_text_system` takes a shared `Arc` because it is
   not, and nothing said so in one place.
3. **The frame's data flow, with its interception points.** Where a rate, a filter or
   a meter can be inserted — and, more usefully, which of those the current seams can
   and cannot reach. `spi/` answers the second half; this is the map it hangs on.
