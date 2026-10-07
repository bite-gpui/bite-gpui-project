# Extension tiers: swaps and wraps

The engine is extended two ways, and the distinction is not stylistic. It decides what
a crate can do, which seam it can plug into, and — for the project's own purposes — what
publishing it commits to.

The five *seams* are [`../spi/README.md`](../spi/README.md)'s subject: the traits, their
bootstraps, and which have a second implementation. This document is about the two tiers
a *crate* can sit in, and why a wrap is not a swap with fewer features.

## Swaps — replace what does the work

A swap implements one of the seam traits and is installed at bootstrap. It is
**exclusive**: one implementation per seam, chosen when the application is built, in
place of the default. It replaces the algorithm — a shaper, a constraint solver, the
frame pipeline itself.

Publishing a swap is a permanent commitment, which `../spi/README.md` states from the
seam's side: an implementation outside the tree has to keep compiling, so the trait
freezes in a way an internal one does not.

## Wraps — decorate the frame pipeline

A wrap implements `FramePipeline` and delegates. It does not replace what does the
layout or the painting; it is handed every pass of every frame it admits, forwards the
ones it does not change, and wraps whatever pipeline is already installed.

That is a narrower position than a swap, and it is a more useful one in two specific
places, because of what the seam actually hands over:

- **The decision.** `should_render` is asked *before* any of the frame's work starts
  (`crates/gpui_authoring/src/window/frame_pipeline.rs:45`), so a wrap can defer a
  frame without any of it running. That is where a rate lives.
- **The passes.** `evaluate_roots`, `layout_roots`, `paint_roots` and the three that
  close the frame run through the pipeline in turn, which is what lets a wrap time each
  one, or inspect and replace the roots between them
  (`crates/gpui_authoring/src/window/frame_pipeline.rs:87`). `draw` is the default
  arrangement of them and can itself be overridden
  (`crates/gpui_authoring/src/window/frame_pipeline.rs:122`).

Those are the only two things a wrap can do, and the boundary is not a limitation to be
worked around — it is the seam (`../spi/README.md` says the same from the other side:
presentation belongs below the pipeline, not in a decorator on it).

That the passes run *through* the wrap is also what makes it the place telemetry belongs.
A scope per pass composes the same way any other decorator does, and because the scopes
are emitted in-process, a test can turn them on and read them back — which is why
telemetry that could not be CI-gated as a swap (see
[`../decisions/tracy-swap-scope.md`](../decisions/tracy-swap-scope.md)) *is* gated as a
wrap in `bite-gp-pass`.

**The trap worth knowing.** Every pass has a default implementation that calls the
`Window` method directly (`crates/gpui_authoring/src/window/frame_pipeline.rs:52`). A
decorator that forgets to forward a pass therefore does not fail — it silently bypasses
the pipeline it wraps for that pass. A wrap is not "a pipeline with extra behaviour"; it
is only as transparent as the passes it explicitly forwards.

`FramePipeline` is the only seam a wrap can plug into, because it is the only one that
hands a crate the frame's stages. A decorator on `TextSystem` or `LayoutEngine` would
have to *be* the implementation, which is a swap.

## What the tiers can and cannot do

| | swap | wrap |
| --- | --- | --- |
| replaces the algorithm a seam runs | yes | no |
| decides whether a frame's work runs | only by being the pipeline | yes — the point |
| times or inspects the frame's passes | only by being the pipeline | yes |
| paces presentation (vblank, swapchain) | `SceneRenderer` owns it | **no** — no swapchain, no vblank, no timestamp |
| composes with another implementation | no — exclusive | yes, in any number, in any order |
| commits the project to a frozen trait | yes, on publication | no — it consumes a trait rather than defining one |

The last row is the one that matters when deciding whether to publish. A wrap on
crates.io constrains the trait no more than any other consumer does; what it commits to
is *its own* API.

## A crate that is neither

The five seams and the two tiers describe crates that plug *into* the engine. The surface work adds a
third shape: a crate that replaces no seam and decorates no pipeline, and simply *consumes* the
published surface, sitting entirely above the facade. `gpui-interop` (a working name) is the first —
it matches a `wgpu` adapter to the window's device, translates a shared OS handle into one GPUI's
renderer can bind, and bridges the fences. None of that is a `SceneRenderer`, a `FramePipeline`, or
any other seam; it builds on two published things and adds nothing to core — `surface()`, the element
an application hands a buffer to, and the renderer's device the canvas lends (`GpuRenderer` /
`GpuCanvasContext::device::<R>()`)
([`../decisions/0006-renderer-owned-device.md`](../decisions/0006-renderer-owned-device.md)).
The design and the probes that gate it are [`../spi/rendering/surfaces.md`](../spi/rendering/surfaces.md)
and [`../spi/rendering/surface-plan.md`](../spi/rendering/surface-plan.md).

What it commits to is smaller than a swap and different from a wrap: it freezes no trait and decorates
no frame, but it does depend on the *published payload types* a token is built from — the commitment
[`0006`](../decisions/0006-renderer-owned-device.md) carries forward under "what would reopen this".
That is the price of a crate that must name
`ID3D11ShaderResourceView` and a `wgpu` adapter in the same function.

## Where the crates sit

| crate | tier | what it does |
| --- | --- | --- |
| `bite-gp-parley` | swap | replaces `TextSystem` |
| `bite-gp-morphorm` | swap | replaces `LayoutEngine` |
| `bite-gp-pass` | wrap | throttles the frame pipeline by a rate policy and keeps a ledger of what its passes cost; behind a `puffin` feature, also emits one profiler scope per pass |
| `ThrottledPipeline`, `InstrumentedPipeline` | wrap | the facade's own two decorators (in-tree, `gpui_runtime`) |
| `gpui-interop` (planned) | neither — a consumer | matches a `wgpu` adapter to the window's device, translates a shared OS handle, and bridges fences; builds on `surface()` and the canvas-lent device, adds nothing to core |

`decisions/0001-no-third-swap.md` is a decision about the **swap** tier only: it says no
third *swap* ships, and it is not a statement about wraps. A wrap is a different
question, with a different commitment, and it is answered separately.
