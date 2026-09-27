# The render extension, in detail — state machines, patches, verification

- **Status:** proposed. The fine-grained companion to
  [`dual-path-ioc-architecture.md`](dual-path-ioc-architecture.md), which holds the
  architecture and the numbered corrections (1–10) that this document *assumes and does
  not repeat*. [`gpu-canvas-dx.md`](gpu-canvas-dx.md) holds the authoring surface's
  corrections. Where code below disagrees with either list, the list wins.
- **Target crates:** `gpui_engine`, `gpui_platform`, `gpui_authoring`, `gpui_linux`,
  `gpui_macos`, `gpui_windows`, `gpui_wgpu`.

## 0. What this grain adds, and what it newly gets wrong

The detailed draft restates the API-surface errors already catalogued —
`WindowContext`, `CornerRadii`, `Style` for `StyleRefinement`, `cx.`-level painting,
`TextureId`, the duplicated `set_viewport_size`, `as_scene_renderer`, `wgpu` inside
`gpui_engine`, `gpui_animotion`, and a `bite_gpui` crate that is really the `gpui`
facade. None of those are re-listed here.

What is genuinely new is worth keeping: the eager initialisation sequence (§1), the
colour-space invariant (§2), the pipeline-state isolation matrix (§3), the recovery
protocol (§4), and the verification matrix (§8). Four things it newly gets wrong are
called out in place, and one — the erasure boundary — is a correction to *this*
document's own premise rather than a detail.

## 1. Window initialisation, and why the factory cannot be earlier

The draft's ordering claim is correct and is the reason the factory lives where it
does: **the native surface must exist before the factory runs.** A renderer binds to a
`wl_surface`, an `HWND` or an `NSView`, so the sequence is strictly:

```
WindowOptions.with_renderer_factory(..)      # per window, gpui_platform (correction 1)
    → WindowHost::new                        # destructures options (crates/gpui_authoring/src/window.rs:1441)
    → Platform::open_window(.., WindowParams) # crosses the trait boundary (crates/gpui_platform/src/platform.rs:95)
        → allocate the native window          # wl_surface / HWND / NSWindow
        → pack the backend target             # BackendRendererTarget
        → RendererTarget::new(&backend)       # erase
        → factory.create(target)? | default   # the decision
    → Box<dyn PlatformRenderer> → stored in the window
```

Two consequences the draft states and one it does not:

- The factory is **invoked once per window, before the first frame**, and never again
  — so `recover` (§4) must be self-sufficient, because there is no second chance to
  re-resolve a target.
- `max_texture_size()` and `set_subpixel_layout(is_bgr)` are queried *after*
  construction (x11 at `crates/gpui_linux/src/linux/x11/window.rs:772`), which is why
  they are on `PlatformRenderer` rather than constructor arguments.
- **Newly wrong, and it is a naming gift:** the draft calls this field
  `renderer_factory` as if new. The headless path already has exactly that field and
  that name — `headless_renderer_factory`
  (`crates/gpui_authoring/src/platform/test/platform.rs:54`, threaded from
  `crates/gpui_authoring/src/app/headless_app_context.rs:68`). The window-level field
  should mirror it rather than invent a parallel vocabulary, and the existing headless
  factory is the working precedent for the whole design.

## 2. Path A: the colour-space invariant, and the erasure this document misses

The draft's §1.2 invariant is real and belongs in the spec, because getting it wrong is
invisible until the composite:

- An offscreen texture handed over as Path A **must** be sampled as sRGB
  (`Rgba8UnormSrgb` / `Bgra8UnormSrgb`). A linear texture composites with a ≈2.2 gamma
  error and reads as washed out. The engine's own scene types are colour-managed the
  same way.
- The sampler is a linear, `ClampToEdge` sampler: the primitive is clipped against UI
  rounded corners, and `ClampToEdge` is what stops the clip's edge from bleeding.

**The correction this document needs that the architecture document does not yet
state.** The draft erases the *handle* only where it is convenient: §2 of the
architecture document erases `RendererTarget`, but this spec leaves
`CustomRenderPrimitive` naming concrete types — `ForeignTextureHandle::Wgpu(wgpu::TextureView)`
and, in §3.3, `DrawContext::wgpu_pass: &mut wgpu::RenderPass<'static>`. Both put `wgpu`
in `gpui_engine` (correction 3). The insight the draft is one step away from: **the
erasure is legitimate precisely because the application also chose the renderer.** The
app that registers a foreign texture and the renderer that consumes it are
counterparties, so the concrete type agreement is *their* contract, not the engine's —
and the engine only needs an opaque token plus a payload it never inspects. That is why
`PaintSurface` gets away with a `CVPixelBuffer` (correction 3's cfg pattern) and why
`DrawContext` must be erased the same way as the handle: the callback receives
`&mut dyn Any`, and the renderer the app installed downcasts it.

## 3. Path B: the state-isolation matrix

Keep this table — it is the most useful artifact in the draft, and it survives the API
corrections unchanged, because it describes GPU state rather than Rust types:

| subsystem | what an injected shader changes | what the renderer must restore |
| --- | --- | --- |
| scissor | clamped to `primitive.bounds` | full target rect |
| viewport | possibly local bounds | `(0, 0, device_w, device_h)` |
| pipeline | custom VS/FS bound | the quad/text pipeline |
| depth/stencil | custom tests/masks | disabled |
| blend | custom or additive | `SrcAlpha, OneMinusSrcAlpha` |
| vertex buffers | slots `[0..N]` overwritten | the GPUI instance buffer at slot 0 |
| samplers | sampler registers changed | the atlas sampler |

Two corrections:

- **The primitive carries `order` and `content_mask`**, like every other scene
  primitive (`crates/gpui_engine/src/scene.rs:749`). Without them the injected command
  cannot be placed correctly in the batch list, and the "snapshot/restore" story is
  about the *wrong* boundary — the restore has to rewind to the batch cursor as well as
  the pipeline state.
- **"Flush the active batch" is renderer-specific and the draft's three sketches are
  wrong in the same direction.** Metal and DirectX do carry a pause/restore shape, but
  wgpu's batcher is not a command encoder a caller can pause mid-pass — the wgpu arm is
  the one that has to be designed against `crates/gpui_wgpu/src/wgpu_renderer.rs`, not
  sketched by analogy. That arm is also the only one that has to be written for Path A
  (`PrimitiveBatch::Surfaces(_surfaces) => {}`,
  `crates/gpui_wgpu/src/wgpu_renderer.rs:1546`).

## 4. Device loss and recovery

The protocol is correct in outline and matches the tree. Verified against the ref:

- Recovery is a method on the concrete renderer,
  `WgpuRenderer::recover<W>(&mut self, window: &W) -> anyhow::Result<()>`
  (`crates/gpui_wgpu/src/wgpu_renderer.rs:2131`), called from `present` on x11
  (`crates/gpui_linux/src/linux/x11/window.rs:1765`) and wayland
  (`crates/gpui_linux/src/linux/wayland/window.rs:1960`) guarded by `device_lost()`.
  The draft's `RendererTarget`-taking `recover` is the erased form of the same call.
- The draft's `new_rejecting_software` is real:
  `crates/gpui_wgpu/src/wgpu_context.rs:76`, already used inside recovery
  (`crates/gpui_wgpu/src/wgpu_renderer.rs:2163`).
- The shared-context subtlety the draft names is the important one: the `GpuContext` is
  process-wide (`Rc<RefCell<Option<WgpuContext>>>`, `crates/gpui_wgpu/src/wgpu_renderer.rs:168`),
  so a recovering window must **adopt a context another window already rebuilt** rather
  than construct a second device. The draft has this; it is the part most likely to be
  got wrong in implementation.
- One thing to add: recovery is not only Linux's. macOS and Windows resume from sleep
  and lose devices too; the protocol should be stated once and implemented three times,
  not described as a Wayland/X11 loop.

## 5. The patch set

The corrected list, extending the architecture document's §8. Line-exact code is not
worth reproducing here — the boundary is a moving target — but the *shape* is:

| patch | change | note |
| --- | --- | --- |
| 01 | new `gpui_platform/src/platform_renderer.rs`: `PlatformRenderer`, `RendererFactory`, `DynRendererFactory`, `RendererTarget` | as drafted |
| 02 | `renderer_factory` on **both** `WindowParams` and `WindowOptions` | the draft adds it to `WindowParams` only, then patch 03 reads it off `WindowOptions` — the two patches contradict each other |
| 03 | forward it through `WindowHost::new`'s destructure into `open_window` | as drafted |
| 04–05 | wayland, x11: consult the factory, else the default | as drafted |
| 06–07 | macOS (`crates/gpui_macos/src/window.rs:1100`), Windows (`crates/gpui_windows/src/window.rs:145`) | **absent from the draft**; without them the field is silently ignored on two backends |

The draft's §2.5 shows a real smell the seam should fix rather than copy: the same
surface-config and `RawWindow` construction appears twice, once for the factory arm and
once for the default. Build the target once and pass it to whichever constructor wins,
so the two arms cannot drift.

## 6. The downstream crate

Three corrections, all from the two companion documents:

- **There is no `bite-gpui` crate to add.** The facade crate is `gpui` (package
  `bite-gpui`), and its authoring surface is `gpui_authoring`. `GpuCanvas` belongs in
  `gpui_authoring` beside `canvas`, re-exported as `gpui::GpuCanvas` — see
  [`gpu-canvas-dx.md`](gpu-canvas-dx.md).
- **The HAL helpers belong with a renderer crate, not the facade.** `ForeignTextureExt`
  and the `as_hal` extraction are `wgpu`-specific and belong in `gpui_wgpu` (or a
  sibling), where `wgpu` is already a dependency — not in the crate that defines
  `GpuCanvas`.
- **`GpuCanvas` must not implement `Element` from scratch.** The draft's §3.1 is the
  same shape `gpu-canvas-dx.md` corrects: it would lose the hitbox that `Div`'s prepaint
  registers, so drag and scroll would never fire. Compose a `div()` and delegate.

**`as_hal` is not anywhere in the tree.** `git grep as_hal` returns nothing, on any
platform. The HAL extraction is therefore not "the existing mechanism, wrapped" — it is
new code that has to be written and tested against `wgpu` 29 (`wgpu = "29.0.4"` at the
workspace manifest) on each backend, with the features that expose `hal::api::Metal`
and `hal::api::Dx11`. Treat it as the riskiest unbuilt piece in the design, and note
that on Linux — where wgpu *is* the renderer — Path A needs no extraction at all, only
the `TextureView` the app already owns.

## 7. Headless export

The draft's §4 is a sound **design for a video exporter** and an unsound **description
of `gpui_animotion`**: the crate in this tree is a declarative property-animation
engine (correction 8, and the RFC preamble). The ring-buffer sizing, the "the virtual
clock pauses instead of dropping frames" backpressure rule, and the reused pinned
staging pool are all worth keeping as the design of whatever crate does the export —
they are just not that crate's description.

The parity claim that *does* belong to this design is the one the seam document makes:
the primitives must render offscreen, because that is what lets CI read a frame back
and assert on it.

## 8. Verification matrix

Keep it, as the test plan. Restated after the corrections, each row is an assertion a
test can actually make:

| scenario | the failing behaviour to prevent | the assertion |
| --- | --- | --- |
| resize / surface churn | stale swapchain geometry | after `set_viewport_size`, the next frame's scissor and viewport match the new size |
| DPI change | half- or double-scale viewport | `bounds * scale_factor` rounds to the same integers the platform target reports |
| premultiplied alpha | dark fringes at rounded corners | a known RGBA fixture composites to a known pixel (the offscreen readback the seam document enables) |
| device loss | `SurfaceLost` panic | `device_lost()` then `recover()` then a frame draws, on the headless path |
| thread affinity | `GpuContext` sent across threads | the factory is `!Send` by construction (`Rc`), so the type system, not a test, is the guard |
| Path A colour space | washed-out composite | an sRGB fixture round-trips without a gamma shift |

The last column is the point: the seam document's whole argument is that this feature
is CI-gated *because* the renderer is installable and the frames are readable
in-process. A verification matrix whose rows cannot be asserted headlessly would
contradict the reason the feature was ordered this way.
