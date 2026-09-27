# SPI

A service provider interface here is a trait a layer implements in order to be
replaced, usually installed through a single bootstrap method on `Application`.
[`../architecture/frame-flow.md`](../architecture/frame-flow.md) has the one exception,
and the frame the seams are entered in.

The distinction worth keeping in mind: an SPI is a *seam*, so publishing one is a
permanent commitment. Once a swap is on crates.io there is an implementation outside
the tree that has to keep compiling, which constrains the trait in a way an internal
trait is not constrained.

A crate can extend the engine without being a swap, though. A **wrap** decorates the
frame pipeline instead of replacing anything, and the two tiers commit to different
things — which is why only one of the five seams can host a wrap, and why publishing one
is not the same decision as publishing a swap. See
[`../architecture/extension-tiers.md`](../architecture/extension-tiers.md).

## The boundaries

Verified 2026-09-26 against the source in a `bite_*` branch.

| seam | trait | bootstrap | a second implementation |
| --- | --- | --- | --- |
| `TextSystem` | `crates/gpui_engine/src/text_system.rs:33` | `Application::with_text_system(Arc<dyn TextSystem>)` | **published** — `bite-gp-parley` |
| `LayoutEngine` | `crates/gpui_engine/src/layout.rs:53` | `Application::with_layout_engine(impl Fn() -> Box<dyn LayoutEngine>)` | **published** — `bite-gp-morphorm` |
| `FramePipeline` | `crates/gpui_authoring/src/window/frame_pipeline.rs:28` | `Application::with_frame_pipeline(impl Fn(WindowId) -> Box<dyn FramePipeline>)` | **no swap ships** — a *wrap* decorates it instead: `bite-gp-pass`, see [`../architecture/extension-tiers.md`](../architecture/extension-tiers.md). Why no third *swap*: [`../decisions/0001-no-third-swap.md`](../decisions/0001-no-third-swap.md) |
| `Platform` | `crates/gpui_platform/src/platform.rs:59` | `Application::with_platform(Rc<dyn Platform>)` | none out-of-tree; host-selected by `cfg(target_os)` |
| `SceneRenderer` | `crates/gpui_engine/src/renderer.rs:16` | **no `Application` hook.** Reached through `PlatformWindow::with_renderer` and `PlatformWindow::present`, so it is swapped by supplying a `Platform` | none |

The bootstrap signatures are in `crates/gpui_runtime/src/application.rs:42-125`.

## What the table is saying

Two of five seams have a published second implementation, and both replace a real
algorithm — a shaper, a constraint solver. The rest are host-bound (`Platform`), reached
only through another seam (`SceneRenderer`, through `PlatformWindow`), or open for a
reason worth reading before adding anything to them (`FramePipeline`).

The fourth of those is the one a *wrap* answers, and it is worth separating the two
questions: no second **swap** implements `FramePipeline`, and `bite-gp-pass` is a
**wrap** on it rather than one — a decorator that decides and observes without
replacing what lays out or paints. [`../architecture/extension-tiers.md`](../architecture/extension-tiers.md)
has the distinction.

`SceneRenderer` owns presentation timing: its `fn draw(&mut self, scene: &Scene) -> bool`
returns whether it presented, and `PlatformWindow::present` hands that result back to
the backend, which uses it to pace its own frame loop. That is the capability a
`FramePipeline` cannot reach — no swapchain, no vblank, no timestamp, and
`Window::present` is private to `gpui_authoring` — which is why presentation belongs
below the pipeline rather than in a decorator on it. The whole flow is drawn in
[`../architecture/frame-flow.md`](../architecture/frame-flow.md).

One more link in the chain is worth knowing before measuring any of this: the frame
source throttles *before* a pipeline is consulted — an unfocused window to
`WindowOptions::inactive_frame_interval` (30 fps by default), and either to 60 fps under
thermal pressure. A pipeline's answer is not the last word on a window's rate.

## Folders

By concern, because one of the seams is a project rather than a trait.

| folder | what belongs |
| --- | --- |
| [`rendering/`](rendering/README.md) | the render extension — the renderer seam and the two primitives — with [its own index](rendering/README.md) and chapter order |
| [`authoring/`](authoring/gpu-canvas.md) | the authoring surface an application meets. `GpuCanvas` today; anything else that is a *surface* rather than an interface |
| [`input/`](input/input-policy-seam.md) | the input-policy seam, parked |
| [`rendering-project/`](rendering-project/README.md) | all the rendering work at one path: grouped symlinks to every document it needs, wherever it lives |

## Seams

One document per seam, wherever it sits.

| document | status |
| --- | --- |
| [`input/input-policy-seam.md`](input/input-policy-seam.md) | parked — specified but not proposed upstream; no third swap ships |
| [`rendering/renderer-seam.md`](rendering/renderer-seam.md) | proposed — giving `SceneRenderer` a bootstrap, and the three contracts that follow: the trait a window holds, the typed target a renderer is built against, and the factory the window consults |

## Spikes

None open. The last one — whether `gpui_wgpu::WgpuRenderer` can present on a macOS window, and
whose `CAMetalLayer` the view's backing layer is — ran on `macos-14` and is now
[`../decisions/macos-presentation-probe.md`](../decisions/macos-presentation-probe.md).

A question a document cannot settle, and the probe that settles it, is recorded the way
[`../decisions/windows-path-a-probe.md`](../decisions/windows-path-a-probe.md) is: as
*evidence* beside the decision it produced, in [`../decisions/`](../decisions/README.md). A
spike still running lives with the work it gates, and is refiled there once it has an
answer.
