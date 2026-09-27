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

## Seams

One document per seam, plus the chapters of the one that has more than a trait.

| document | status |
| --- | --- |
| [`input-policy-seam.md`](input-policy-seam.md) | parked — specified but not proposed upstream; no third swap ships |
| [`renderer-seam.md`](renderer-seam.md) | proposed — giving `SceneRenderer` a bootstrap, and the three contracts that follow: the widened trait, the typed target a renderer is built against, and the factory a window consults |

## The render extension

One feature, six chapters, because it is the only one that needs a primitive, a
renderer contract and an authoring surface at once. Read them in order; each assumes the
one before it.

| chapter | subject |
| --- | --- |
| [`renderer-seam.md`](renderer-seam.md) | the seam: the widened `SceneRenderer`, `PlatformRenderer`, the typed target, the factory, recovery, and the upstream patch set |
| [`foreign-texture.md`](foreign-texture.md) | Path A: importing a texture produced outside GPUI, the erasure, the colour-space invariant, and what each platform can actually do |
| [`inline-commands.md`](inline-commands.md) | Path B: drawing into the window's own pass, the pipeline-state isolation matrix, and the coordinate bridge |
| [`gpu-canvas.md`](gpu-canvas.md) | the authoring surface, so an application never meets `Element` |
| [`verification.md`](verification.md) | what a test can assert, and which platform each check needs |
| [`spike-windows-path-a.md`](spike-windows-path-a.md) | the one question the documents above could not settle, and what the probe answered |

## How this set was reconciled

2026-09-27. Six drafts of the render extension arrived over two days, from two
directions, and they overlapped: two of them specified a factory, three specified the
same primitive, and four of them carried numbered corrections against each other. The
chapters above are the merge. The drafts are kept in [`drafts/`](drafts/) as *evidence* —
what was proposed, and what each got wrong — rather than as specification, the way
`decisions/` keeps unnumbered evidence beside numbered records.

| draft | became |
| --- | --- |
| [`drafts/dual-path-render-extension.md`](drafts/dual-path-render-extension.md) — the RFC, transcribed as received | the shape of `foreign-texture.md` and `inline-commands.md`; its Chapter 5 sketches are not portable here |
| [`drafts/scene-renderer-seam.md`](drafts/scene-renderer-seam.md) | `renderer-seam.md` — its §1 (widen the trait), §3 (native hooks) and migration order are kept nearly whole; its process-wide factory (§2, §4, §5) is replaced |
| [`drafts/dual-path-ioc-architecture.md`](drafts/dual-path-ioc-architecture.md) | `renderer-seam.md` for the factory, the target and the trait, `foreign-texture.md` for the primitive; its finding that `PaintSurface` already half-exists is the reason Path A is not a new primitive |
| [`drafts/dual-path-implementation-spec.md`](drafts/dual-path-implementation-spec.md) | `renderer-seam.md` (recovery), `foreign-texture.md` (colour space), `inline-commands.md` (state isolation), `verification.md` |
| [`drafts/wgpu-target-adaptors.md`](drafts/wgpu-target-adaptors.md) | `renderer-seam.md` (the typed target), `foreign-texture.md` (HAL extraction), `inline-commands.md` (the coordinate bridge) |
| [`drafts/gpu-canvas-dx.md`](drafts/gpu-canvas-dx.md) | `gpu-canvas.md`, nearly whole |
| cross-device texture sharing (never committed) | `foreign-texture.md` §"The device constraint"; its Tier 2 is rejected there |
| [`spike-windows-path-a.md`](spike-windows-path-a.md) | kept as a spike |

### Contradictions, and which way each was settled

1. **Erased or typed `RendererTarget`?** **Typed.** The erased form
   (`raw: &'a dyn Any`) makes any renderer that is not the backend's own downcast to
   `LinuxRendererTarget`-style types, which requires depending on the backend crate —
   and `gpui_linux` already depends on `gpui_wgpu`, so that is a cycle.
   `gpui_platform` already depends on `raw-window-handle`, so naming the handles costs
   nothing. One consequence is worth more than the argument: with the handles typed,
   the *whole* per-platform downcast cascade in the adaptors draft — `surface_info`,
   `window_handle`, `display_handle`, each with three arms and a downcast — collapses to
   field reads, and `gpui_wgpu` stops needing to name a backend crate at all.
2. **Is `PlatformRenderer` a trait or a type alias?** **A trait, with a cfg-selected
   supertrait.** The seam draft wanted `type PlatformRenderer = dyn SceneRenderer` with
   per-platform extension traits; the revision wanted `trait PlatformRenderer:
   SceneRenderer`. Both are kept: the trait carries the lifecycle, and the native hooks
   are extension traits that the trait requires on the platforms that have them.
   Widening the shared trait with macOS-only methods instead would break the layer-stack
   ruling against doing that.
3. **`set_viewport_size` on `PlatformRenderer`?** **No — and the name is wrong.**
   `SceneRenderer::set_viewport_size` is test-gated
   (`crates/gpui_engine/src/renderer.rs:32`), has exactly one caller (the test window,
   `crates/gpui_authoring/src/platform/test/window.rs:531`) and one override
   (`crates/gpui_apple/src/metal_renderer.rs:1660`). The *production* resize path
   already has a name: `WgpuRenderer::update_drawable_size`
   (`crates/gpui_wgpu/src/wgpu_renderer.rs:1132`). So neither "lift the gate" nor "drop
   it" was needed — the contract gets the production name, which is also what makes a
   factory-installed renderer behave exactly as the backend's own.
4. **Where does the factory live — `Application` or the window?** **The window.**
   `WindowOptions`/`WindowParams`, mirrored by `TestPlatform`'s existing
   `headless_renderer_factory`. A process-wide default and a per-window override can
   coexist later; only the per-window field is specified.
5. **`RendererFactory`: a bare `Rc<dyn Fn>` or a newtype?** **A trait plus a newtype.**
   `WindowOptions` and `WindowParams` both derive `Debug`
   (`crates/gpui_platform/src/window.rs:356`, `:429`), which a closure cannot satisfy and
   a trait object cannot derive. The `FnRendererFactory` adapter keeps the closure
   spelling for the common case.
6. **The Windows arm of `ForeignTextureHandle`.** **Removed.** A wgpu texture cannot be
   read by GPUI's Direct3D 11 renderer, and wgpu offers no shareable resource — see
   `foreign-texture.md` and the spike. The payload is the wgpu `TextureView` on every
   platform wgpu renders, and the raw Metal handle on macOS.
7. **`SharedGraphicsContext`, from the uncommitted draft.** **Rejected.** It duplicates
   `WgpuContext` field for field (`crates/gpui_wgpu/src/wgpu_context.rs:9`) and
   `WgpuRenderer::new` takes the *existing* `GpuContext`, so a parallel type cannot be
   injected. It also creates a device, which is the thing the draft's own §1 warns
   against.
8. **`gpui_animotion`.** **Not this tree's, and not named.** The `gpui_animotion` in
   `.uses` is a third-party declarative *property-animation* engine; the RFC's video
   exporter is a crate that does not exist here.
9. **wgpu 29's builder API.** Corrected in the drafts: `Adapter::request_device` takes
   one argument and returns a `Result`
   (`wgpu-29.0.4/src/api/adapter.rs:58`), and `Instance::request_adapter` returns a
   `Result`, not an `Option` (`wgpu-29.0.4/src/api/instance.rs:167`).
10. **The drafts' authoring names.** `WindowContext` and `ViewContext` do not exist
    here; `CornerRadii` is `Corners<Pixels>`; and painting is a `window.` capability, not
    a `cx.` one. Corrected throughout.

### Settled

Each of these was decided on evidence and is not reopened by re-reading the drafts.

- **Path A completes an existing primitive, it does not add one.** `PaintSurface`
  (`crates/gpui_engine/src/scene.rs:749`) is already "content produced outside GPUI,
  composited into the window", already a scene variant with its own batch, and already
  drawn by two of three renderers. The wgpu arm is a no-op to write.
- **The texture handle is erased; the target is not.** They are not the same kind of
  value: a handle has one counterparty, the application and the renderer it chose, so a
  `dyn Any` payload is a private agreement between them; the target is read by any
  renderer the application installs.
- **Path A is the window owner's capability.** Producer and consumer must be the same
  device, which only the factory that built the renderer has. A widget inside someone
  else's window cannot be a producer.
- **Windows is Path A and Path B only under `gpui_wgpu::WgpuRenderer`.** The default
  `DirectXRenderer` is Direct3D 11 and the producer is Direct3D 12; there is no copy-free
  bridge, and wgpu offers no shareable resource. Decided in
  [`../decisions/0002-render-extension-device-model.md`](../decisions/0002-render-extension-device-model.md).
- **The factory is invoked once, before the first frame.** Recovery is therefore
  self-sufficient on the returned renderer, and post-construction queries belong on the
  renderer's trait rather than on the factory's input.
- **The trait upcast is load-bearing.** `PlatformWindow::with_renderer` and `present`
  hand out `&mut dyn SceneRenderer`
  (`crates/gpui_platform/src/platform_window.rs:151`, `:157`), and the explicit
  `as_scene_renderer` shims are gone, so reaching a `Box<dyn PlatformRenderer>` as a
  `dyn SceneRenderer` is upcasting. Stable since 1.86; `rust-toolchain.toml` is 1.95.

### Open

- **The handle's erasure vs a cfg-gated `wgpu` in `gpui_engine`.** The recommendation is
  erasure, following the target's own argument. `PaintSurface` is the precedent for the
  other way — a cfg'd field with a cfg'd dependency, as macOS's `CVPixelBuffer` has.
- **The native hooks' shape.** Should hold: the extension-trait form in
  `renderer-seam.md`. What would reopen it is in that document.
- **macOS layer ownership.** If a non-Metal renderer on macOS cannot be handed the
  window's layer, `MacSceneRenderer` is the wrong shape and the layer has to become a
  platform-side resource.
- **Whether `WgpuRenderer` should become the default on macOS and Windows**, retiring
  Metal and DirectX to optional. The seam makes the question askable; nothing decides it.

## Spikes

A question a document cannot settle, and the probe that settles it.

| document | question |
| --- | --- |
| [`spike-windows-path-a.md`](spike-windows-path-a.md) | whether a texture rendered by `wgpu` can reach GPUI's Direct3D 11 Windows renderer, or whether Path A and Path B on Windows require installing `WgpuRenderer` |
