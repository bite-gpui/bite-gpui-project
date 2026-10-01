# The render extension

GPUI has no escape hatch for an external GPU context. A viewport that draws hundreds of
thousands of vertices, a 3D view, a video decoder or a camera feed has three ways in
today, and all three cost something: translate the geometry into GPUI's box and path
primitives, blit an offscreen image through a CPU staging buffer — the shape `RenderImage`
has — or fork the engine. The readback alone moves `W × H × 4 × fps × 2` bytes a second,
≈1.0 GB/s at 1080p60 and ≈8.0 GB/s at 4K120, across the bus between GPU and CPU, which
saturates the memory bus and costs one to two frames of latency.

The extension answers with two mechanisms: **a surface** — a buffer the application produced,
composited into the window's scene through `surface()`, the route called **Path A** while it was
built — and **inline commands** — the application's own commands drawn into the window's pass,**Path B**. [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) reunites the
surface route with the primitive GPUI already had, so `surface()` is the entry point and no second
scene type is added; [`surfaces.md`](surfaces.md) is that design. Where the four candidates land:

| | primitive translation | offscreen blit | Path B inline | Path A texture |
| --- | --- | --- | --- | --- |
| GPU passes | 1 | 2 | **1** | 2 (zero-copy) |
| CPU readback | none | full | **none** | **none** |
| stacking with UI | native | limited | native | **native** |
| scale ceiling | fails past ~500k vertices | high | **max** | high |
| shader freedom | GPUI's SDFs only | full | **full** | full |

Read the chapters in order; each assumes the one before it. The authoring surface lives in
its own folder, because it is a surface an application meets rather than part of the seam.

| chapter | subject |
| --- | --- |
| [`renderer-seam.md`](renderer-seam.md) | the seam: `PlatformRenderer`, the typed target, the factory, recovery, and the implementation order. `SceneRenderer` itself is unchanged |
| [`foreign-texture.md`](foreign-texture.md) | Path A: importing a texture produced outside GPUI, the erasure, the colour-space invariant, and what each platform can actually do |
| [`inline-commands.md`](inline-commands.md) | Path B: drawing into the window's own pass, the pipeline-state isolation matrix, and the coordinate bridge |
| [`verification.md`](verification.md) | what a test can assert, and which platform each check needs |
| [`producer-reach.md`](producer-reach.md) | the other half of Path A: how a producer gets the device it has to make its texture on, what each platform lends now, and what is left |
| [`surfaces.md`](surfaces.md) | **the design of record for external pixels** — the unification under `PaintSurface`/`surface()`, the IOSurface/DXGI/dma-buf trinity, host and guest modes, and the interop boundary |
| [`surface-plan.md`](surface-plan.md) | the implementation plan for `surfaces.md`, and the probes each stage is gated on |
| [`milestones.md`](milestones.md) | the handoff: what is built and where, what is next in what order, and the gates a resumer runs |
| [`../authoring/gpu-canvas.md`](../authoring/gpu-canvas.md) | the authoring surface, so an application never meets `Element` |

Eight documents outside this folder are part of the work rather than of the design:
[`../../decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md)
decides which devices a producer may use,
[`../../decisions/0004-producer-device-rendezvous.md`](../../decisions/0004-producer-device-rendezvous.md)
decides how a producer reaches one, and
[`../../decisions/0005-external-rendering-unifies-under-surface.md`](../../decisions/0005-external-rendering-unifies-under-surface.md)
decides that the two are one primitive, a `PaintSurface`. The measurements they rest on are
[`../../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md), what 0002's
Windows clause rests on,
[`../../decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md),
its second rider,
[`../../decisions/shared-surface.md`](../../decisions/shared-surface.md), the shared-buffer
measurement that reopens it,
[`../../decisions/macos-presentation-probe.md`](../../decisions/macos-presentation-probe.md), what
§6's macOS commit and §10 rest on, and
[`../../decisions/macos-wgpu-producer-probe.md`](../../decisions/macos-wgpu-producer-probe.md), the
producer probe, which answers §2's macOS row.
[`rendering-project/`](../rendering-project/README.md) gathers symlinks to all of them.

## Status

The seam is merged: PR #4 landed on `bite_v1.22.0-pre` as `c9f279d839`, carrying the contracts,
the factory, and each backend's window, as §6 of [`renderer-seam.md`](renderer-seam.md) lists.
`SceneRenderer` is unchanged, as that chapter assumes: its only implementors are the four
backends' renderers and the test and headless windows.

Path A is on the canonical ref, merged as PR #6 — its engine and authoring halves, the offscreen
mode in the renderer contract the path is asserted through, all three arms (wgpu, Metal and
Direct3D), and the producer's reach, so an application can obtain the device its texture is made on
as well as paint one.

The name of that path changed after it merged. [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md)
unifies external pixels under the existing `PaintSurface` / `surface()` rather than a second
primitive, so what PR #6 built is retargeted into `draw_surfaces`: the same arms, colour space and
rendezvous, under the entry point upstream already has. The offscreen half — `PixelBuffer`, and
`render_scene` split from `read_pixels` — is that record's upstream PR 1, already built.

What that adds up to is that the three consumer arms and the three producers are level: each
renderer samples an RGBA foreign texture, lends its device, has a token builder an application names
as `gpui::…` — the wgpu one through the platform crate, the Direct3D one not at all when the window
installed `WgpuRenderer` — and implements the offscreen contract, which Direct3D was the last to do.

A runnable demo ties the two halves together: `cargo run -p gpui --example path_a` asks the window
for its device, refills a texture on it every frame, hands the renderer a token from a paint
callback, and paints an ordinary `div()` over the composite. Its producer differs per platform,
because the renderer does — wgpu on Linux and macOS, Direct3D 11 on Windows.

What that still does not add up to is Path A being *finished*, and
[`producer-reach.md`](producer-reach.md) is where the remainder lives: the Windows bridge —
[0002](../../decisions/0002-render-extension-device-model.md)'s deferred tier, which needs a
measurement before it needs code — and the same-device rule's enforcement on macOS, which no code can
supply at all. The macOS producer route was a third, and it is measured now. `GpuCanvas` is
unwritten too, and [`inline-commands.md`](inline-commands.md) is still nothing but a proposal.

The offscreen mode is the change with reach beyond Path A — `PixelBuffer` is the contract's pixel
type now, and rendering is separate from reading back — so [`verification.md`](verification.md) §2
records where each row runs.

The seam's test runs for the pull requests: `bite-ci.yml`'s `tests` job runs
`cargo test -p gpui_authoring -p gpui_engine --lib` on Linux, where the tests live.

The platform claims rest on the probe printouts the chapters cite.

## Next

The ordered plan is [`milestones.md`](milestones.md): what is built and where, what comes next in
what order, and the gates a resumer runs. It is there rather than here because the list below had
grown past what an index should carry, and two copies of a plan are two copies to keep in step.

In one line, the order is: **`GpuCanvas`**, the surface an application meets — the three arms, the
three producers and the demo are level, so what is left beside them is the canvas itself; then Path
B, which is independent of all of it; then a third-party renderer; then the cross-API bridge — which
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) schedules downstream
rather than deferring, and puts the unified `surface()` arm ahead of it. The producer's reach, the
three arms and the macOS probe are done, and are no longer on the list.

The list this section used to hold is closed. The citations were re-pointed to the merged ref — the
merge moved lines in every file the chapters cite, and §3's Windows rows and §5.2 took real
citations with them. And `bite-ci.yml` has a `tests` job:
`cargo test -p gpui_authoring -p gpui_engine --lib` on Linux, where the tests live, which is the
engine's and authoring's rows running for the pull requests rather than only locally. The job's
filter is narrow, and widening it to the other packages is still open.

Porting the branch to the other targets is the replay in `tools`, and the canonical pass it
wanted is done.

## How this set was reconciled

2026-09-27. Six drafts of the render extension arrived over two days, from two directions,
and they overlapped: two of them specified a factory, three specified the same primitive,
and four of them carried numbered corrections against each other. The chapters above are
the merge. The drafts themselves have been **removed** — every part of them is either
superseded by a chapter, recorded below as a correction, or was wrong enough not to keep,
and a specification that no longer compiles is not evidence. The table is the provenance:
what each proposed, and what became of it.

| draft | became |
| --- | --- |
| `dual-path-render-extension.md` — the RFC, transcribed as received | the shape of `foreign-texture.md` and `inline-commands.md` |
| `scene-renderer-seam.md` | `renderer-seam.md` — its §3 (native hooks) and the order it works in are kept; its §1 (widen the trait) and its process-wide factory (§2, §4, §5) are not |
| `dual-path-ioc-architecture.md` | `renderer-seam.md` for the factory, the target and the trait, `foreign-texture.md` for the primitive; its finding that `PaintSurface` already half-exists is why Path A keeps that variant's shape rather than inventing one — though not its macOS-only YCbCr drawing, which [`foreign-texture.md`](foreign-texture.md) §1 states once |
| `dual-path-implementation-spec.md` | `renderer-seam.md` (recovery), `foreign-texture.md` (colour space), `inline-commands.md` (state isolation), `verification.md` |
| `wgpu-target-adaptors.md` | `renderer-seam.md` (the typed target), `foreign-texture.md` (HAL extraction), `inline-commands.md` (the coordinate bridge) |
| `gpu-canvas-dx.md` | `../authoring/gpu-canvas.md`, nearly whole |
| cross-device texture sharing (never committed) | `foreign-texture.md` §"The device constraint" and [decision 0002](../../decisions/0002-render-extension-device-model.md); its Tier 2 is rejected there |
| `spike-windows-path-a.md` | neither a chapter nor a removal: it is the measurement [decision 0002](../../decisions/0002-render-extension-device-model.md) rests on, so it was refiled as [`../../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) |

Two claims in the drafts are worth naming, because they are the kind that get copied: the
RFC's "≈16.0 GB/s at 4K120" is its own formula miscounted — `W × H × 4 × fps × 2` is ≈8.0
— and its Chapter 7 names `gpui_animotion` as a video encoder, which is a different crate
(item 8 below).

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
   (`crates/gpui_engine/src/renderer.rs:95`), has exactly one caller (the test window,
   `crates/gpui_authoring/src/platform/test/window.rs:529`) and one override
   (`crates/gpui_apple/src/metal_renderer.rs:1813`). The *production* resize path
   already has a name: `WgpuRenderer::update_drawable_size`
   (`crates/gpui_wgpu/src/wgpu_renderer.rs:1297`). So neither "lift the gate" nor "drop
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
6. **The Windows arm of `ImportedTextureHandle`.** **A shared handle, not a raw device
   pointer.** A wgpu texture cannot be read by GPUI's Direct3D 11 renderer as a resource on
   its own device, and wgpu cannot create a shareable one — but it can adopt one the
   application allocated, so a DXGI shared NT handle reaches D3D11 where a raw pointer cannot.
   See [`foreign-texture.md`](foreign-texture.md) §2,
   [`../../decisions/shared-surface.md`](../../decisions/shared-surface.md), and 0002's
   revision. The same-device payload stays the wgpu `TextureView` on every platform wgpu
   renders, and the raw Metal handle on macOS.
7. **`SharedGraphicsContext`, from the uncommitted draft.** **Rejected.** It duplicates
   `WgpuContext` field for field (`crates/gpui_wgpu/src/wgpu_context.rs:14`) and
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
10. **The drafts' authoring names, and their texture API.** `WindowContext` and
    `ViewContext` do not exist here; `CornerRadii` is `Corners<Pixels>`; painting is a
    `window.` capability, not a `cx.` one; and there is no registry to `register` a
    texture into, so it is one `paint` call and not a `register`-then-name pair
    ([`foreign-texture.md`](foreign-texture.md) §3). Corrected throughout.

### Settled

Each of these was decided on evidence and is not reopened by re-reading the drafts.

- **Path A reuses `PaintSurface`'s machinery, and not its drawing.** The variant, its batch,
  its ordering and its content-mask handling are what the primitive is shaped after
  (`crates/gpui_engine/src/scene.rs:784`); the drawing is new, because `PaintSurface` is
  itself macOS-only video with a YCbCr fragment path, drawn by one renderer, and every sprite
  and path fragment samples the atlas instead of a texture of its own.
  [`foreign-texture.md`](foreign-texture.md) §1 states it once.
- **The texture handle is erased; the target is not.** They are not the same kind of
  value: a handle has one counterparty, the application and the renderer it chose, so a
  `dyn Any` payload is a private agreement between them; the target is read by any
  renderer the application installs.
- **Path A is the window owner's capability.** The device is the one the factory that built
  the renderer chose, so a widget inside someone else's window cannot be a producer; a
  producer on another device reaches it only through the shared-handle bridge (see Open).
- **Windows takes the same-device route, on whichever renderer matches the producer.** A wgpu
  producer runs under `gpui_wgpu::WgpuRenderer`, as on Linux, and the payload is the
  `wgpu::TextureView`; a Direct3D 11 producer runs under the default `DirectXRenderer`, whose arm
  is built, and the payload is its own `ID3D11Texture2D`. They are exclusive per window, because
  the tier follows the renderer — `DirectXRenderer` buys per-pixel transparency and `WgpuRenderer`
  buys wgpu. The shared-handle bridge that would reach either from a wgpu producer's foreign device
  is measured and deferred, not part of this milestone.
- **The factory is invoked once, before the first frame.** Recovery is therefore
  self-sufficient on the returned renderer, and post-construction queries belong on the
  renderer's trait rather than on the factory's input.
- **The trait upcast is load-bearing.** `PlatformWindow::with_renderer` and `present`
  hand out `&mut dyn SceneRenderer`
  (`crates/gpui_platform/src/platform_window.rs:150`, `:167`), and the explicit
  `as_scene_renderer` shims are gone, so reaching a `Box<dyn PlatformRenderer>` as a
  `dyn SceneRenderer` is upcasting. Stable since 1.86; `rust-toolchain.toml` is 1.95.

### Open

- **The handle's erasure vs a cfg-gated `wgpu` in `gpui_engine` — closed by
  [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md).** The unified
  `SurfaceHandle` follows `PaintSurface`'s existing shape — a cfg'd variant with a cfg'd dependency,
  as macOS's `CVPixelBuffer` already is — so the erasure this bullet recommended is withdrawn.
- **The native hooks' shape.** Should hold: the extension-trait form in
  [`renderer-seam.md`](renderer-seam.md). What would reopen it is in that document.
- **Whether `WgpuRenderer` should become the default on macOS and Windows**, retiring
  Metal and DirectX to optional. The seam makes the question askable and nothing decides it —
  though both halves are now measured rather than unknown, below.
- **The shared-handle bridge — scheduled downstream by
  [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md).** A producer on a
  foreign device reaches GPUI's renderer through a DXGI shared handle it allocates and GPUI adopts;
  measured to work ([`../../decisions/shared-surface.md`](../../decisions/shared-surface.md)), at the
  cost of a fence and a same-adapter guarantee the same-device route does not need. It lives in a
  downstream crate rather than core, and its reverse direction is still unmeasured
  ([`surface-plan.md`](surface-plan.md) P1).
- **Windows presentation for a non-D3D11 renderer.** Measured: `WgpuRenderer` can present
  on a Windows window, on Direct3D 12 — which `gpui_wgpu` does not enable today, so the Windows commit
  is a backend change as well as plumbing — and the DX12 surface offers only `Opaque` alpha,
  so transparency is the one capability the default renderer has that this one does not.
  [`../../decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md).
- **macOS presentation for a non-Metal renderer, and the layer it needs.** Measured:
  `WgpuRenderer` presents on a macOS view, on Metal, which `gpui_wgpu` does not enable either
  — the same backend change, one step earlier in the call — so `MacSceneRenderer` keeps its
  shape, §10's first trigger does not fire, and transparency is available here where the DX12
  surface refuses it. The corner it leaves open is a `makeBackingLayer` that returns nil.
  [`../../decisions/macos-presentation-probe.md`](../../decisions/macos-presentation-probe.md).
- **Egress, if it is wanted.** Every chapter here is about a producer reaching *into* GPUI. The
  other direction — handing GPUI's own output to a foreign consumer — is unspecified, and the
  device rule already fixes its shape: on our device it is two views of one resource and needs
  no handle at all, on another device it is the bridge 0002 now leaves open, and otherwise it is
  a readback. Video out is this direction, and
  [`../../decisions/shared-surface.md`](../../decisions/shared-surface.md) is its first
  measurement; what is *not* missing is the OS compositor, which is already a consumer of our
  surface through exactly those bridges, and is how a window reaches the screen.
