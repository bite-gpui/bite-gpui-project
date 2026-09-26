# SPI

A service provider interface here is a trait a layer implements in order to be
replaced, installed through a single bootstrap method on `Application`.

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
| `SceneRenderer` | `crates/gpui_engine/src/renderer.rs:16` | **none** — there is no `with_renderer` | none |

The bootstrap signatures are in `crates/gpui_runtime/src/application.rs:42-125`.

## What the table is saying

Two of five seams have a published second implementation, and both replace a real
algorithm — a shaper, a constraint solver. The rest are host-bound (`Platform`),
unreachable (`SceneRenderer` has no bootstrap to install one through), or open for a
reason worth reading before adding anything to them (`FramePipeline`).

`SceneRenderer` is the boundary that already owns presentation timing:
`fn draw(&mut self, scene: &Scene) -> bool` returns whether it presented, and a
backend that paces itself on compositor callbacks returns `false` to retry the frame.
That is the capability a `FramePipeline` cannot reach — it has no swapchain, no vblank
and no timestamp — which is why presentation belongs to a renderer and not to a
pipeline decorator.

## Proposals

| document | status |
| --- | --- |
| [`input-policy-seam.md`](input-policy-seam.md) | parked — specified but not proposed upstream; no third swap ships |
