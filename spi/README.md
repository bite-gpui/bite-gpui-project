# SPI

A service provider interface here is a trait a layer implements in order to be
replaced, usually installed through a single bootstrap method on `Application`.
[`../architecture/frame-flow.md`](../architecture/frame-flow.md) has the one exception,
and the frame the seams are entered in.

The distinction worth keeping in mind: an SPI is a *seam*, so publishing one is a
permanent commitment. Once a swap is on crates.io there is an implementation outside
the tree that has to keep compiling, which constrains the trait in a way an internal
trait is not constrained.

## The boundaries

Verified 2026-09-26 against the source in a `bite_*` branch.

| seam | trait | bootstrap | a second implementation |
| --- | --- | --- | --- |
| `TextSystem` | `crates/gpui_engine/src/text_system.rs:33` | `Application::with_text_system(Arc<dyn TextSystem>)` | **published** — `bite-gp-parley` |
| `LayoutEngine` | `crates/gpui_engine/src/layout.rs:53` | `Application::with_layout_engine(impl Fn() -> Box<dyn LayoutEngine>)` | **published** — `bite-gp-morphorm` |
| `FramePipeline` | `crates/gpui_authoring/src/window/frame_pipeline.rs` | `Application::with_frame_pipeline(impl Fn(WindowId) -> Box<dyn FramePipeline>)` | none ships — [`../decisions/0001-no-third-swap.md`](../decisions/0001-no-third-swap.md) |
| `Platform` | `crates/gpui_platform/src/platform.rs:59` | `Application::with_platform(Rc<dyn Platform>)` | none out-of-tree; host-selected by `cfg(target_os)` |
| `SceneRenderer` | `crates/gpui_engine/src/renderer.rs:16` | **no `Application` hook.** Reached through `PlatformWindow::with_renderer` and `PlatformWindow::present`, so it is swapped by supplying a `Platform` | none |

The bootstrap signatures are in `crates/gpui_runtime/src/application.rs:42-125`.

## What the table is saying

Two of five seams have a published second implementation, and both replace a real
algorithm — a shaper, a constraint solver. The rest are host-bound (`Platform`), reached
only through another seam (`SceneRenderer`, through `PlatformWindow`), or open for a
reason worth reading before adding anything to them (`FramePipeline`).

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
| [`dual-path-render-extension.md`](dual-path-render-extension.md) | proposed — a `SceneRenderer` extension SPI (zero-copy texture import and inline command injection). Nothing of it is implemented; the seam it extends has no `Application` hook, so reaching it means supplying a `Platform` |
