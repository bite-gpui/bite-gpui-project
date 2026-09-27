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

## Proposals

| document | status |
| --- | --- |
| [`input-policy-seam.md`](input-policy-seam.md) | parked — specified but not proposed upstream; no third swap ships |
| [`scene-renderer-seam.md`](scene-renderer-seam.md) | proposed — giving `SceneRenderer` a bootstrap so a renderer is installable through `Application`. Its factory design (§2–§5) is revised by the entry two rows below; its §3, the native hooks, still stands |
| [`dual-path-render-extension.md`](dual-path-render-extension.md) | proposed — the zero-copy texture import and inline command injection RFC, transcribed as received. Its factory-free framing is superseded by the revision below |
| [`dual-path-ioc-architecture.md`](dual-path-ioc-architecture.md) | proposed — the fork's revision of the render extension, merging the two above: the IoC factory, the typed target, and the primitive, with the corrections each needs at this ref. Its first finding is that Path A already half-exists as `PaintSurface` |
| [`dual-path-implementation-spec.md`](dual-path-implementation-spec.md) | proposed — the fine-grained companion to the row above: the initialisation sequence, the colour-space and pipeline-state-isolation invariants, the recovery protocol, and a verification matrix. It inherits that row's corrections rather than repeating them |
| [`wgpu-target-adaptors.md`](wgpu-target-adaptors.md) | proposed — the renderer author's surface: what the target carries (typed handles, not `dyn Any`), the HAL texture extractor, and Path B's coordinate bridge. It finds that erasing the target cycles the dependency graph |
| [`gpu-canvas-dx.md`](gpu-canvas-dx.md) | proposed — the authoring surface the five above exist to serve: a chainable `GpuCanvas` component, built on `div` and `canvas`, so an application renders a GPU viewport in a `div()` without implementing `Element` |

## Spikes

A question a document cannot settle, and the probe that settles it.

| document | question |
| --- | --- |
| [`spike-windows-path-a.md`](spike-windows-path-a.md) | whether a texture rendered by `wgpu` can reach GPUI's Direct3D 11 Windows renderer, or whether Path A and Path B on Windows require installing `WgpuRenderer` |
