# Dual-path GPU render extension and the IoC renderer factory

- **Status:** proposed. The fork's revision of the feature, merging the two documents
  that split it:
  [`scene-renderer-seam.md`](scene-renderer-seam.md) (making a renderer installable)
  and [`dual-path-render-extension.md`](dual-path-render-extension.md) (the primitives).
  It supersedes the *factory* design in the seam document's §2–§5 and the sketches in
  the RFC's Chapters 4–6; both stay for provenance, and one part of the seam document
  is still load-bearing — see "What the seam document still owns" below.
- **Target crates:** `gpui_engine`, `gpui_platform`, `gpui_authoring`, `gpui_linux`,
  `gpui_macos`, `gpui_windows`, `gpui_wgpu`.
- **Ecosystem:** the `gpui` facade and `gpui_authoring`. **Not** `gpui_animotion` — see
  correction 8.

## What the draft misses: the engine already has this primitive

The most important correction, because it changes the shape of Path A rather than a
line of it.

`crates/gpui_engine/src/scene.rs:749` defines `PaintSurface`, a scene primitive for
exactly "content produced outside GPUI, composited into the window":

```rust
#[derive(Clone, Debug)]
pub struct PaintSurface {
    pub order: DrawOrder,
    pub bounds: Bounds<ScaledPixels>,
    pub content_mask: ContentMask<ScaledPixels>,
    #[cfg(target_os = "macos")]
    pub image_buffer: core_video::pixel_buffer::CVPixelBuffer,
}
```

It is a variant of the scene's primitive enum (`Primitive::Surface`,
`crates/gpui_engine/src/scene.rs:228`), it has its own batch
(`crates/gpui_engine/src/scene.rs:475`), an accumulation list
(`crates/gpui_engine/src/scene.rs:50`, pushed at `:132`), and it is **already
implemented in two backends**: Metal (`crates/gpui_apple/src/metal_renderer.rs:1133`)
and DirectX (`crates/gpui_windows/src/directx_renderer.rs:830`). In wgpu it is a no-op
— `PrimitiveBatch::Surfaces(_surfaces) => {}`
(`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`).

Three things follow, and they are the reason this document does not start from
"nothing exists":

1. **Path A is not a new primitive; it is completing an existing one.** The honest
   proposal is *generalise `PaintSurface` into the foreign-texture primitive* — the
   same variant, the same batch, the same three renderer arms, one of which (wgpu) has
   to be written and two of which (Metal, DX) already exist for the CV path.
2. **The engine's precedent for a backend payload is a `#[cfg]` field, not a foreign
   type.** macOS's `CVPixelBuffer` is gated by `#[cfg(target_os = "macos")]` and the
   dependency by `[target.'cfg(target_os = "macos")'.dependencies] core-video` in
   `crates/gpui_engine/Cargo.toml`. It does *not* put a renderer's type in the engine.
3. **`PaintSurface` is a precedent for Path B's problem too.** It carries `order` and
   `content_mask` — the two fields an interleaved primitive needs to sit correctly in
   the batch list, which is what the draft's `InlineCommand` omits.

## Corrections to the draft

Each is a difference from this fork at `bite_v1.22.0-pre`, not a matter of taste.
Corrections 1–3 are the ones that change code.

1. **`WindowOptions` is a `gpui_platform` type, not an `authoring` one.**
   `crates/gpui_platform/src/window.rs:357`. So `WindowOptions::with_renderer_factory`
   is a builder on the platform crate's own type, and §3's diagram puts it a layer too
   high. `WindowParams` lives in the same file at `:438`.
2. **Both types derive `Debug`**, `WindowOptions` at
   `crates/gpui_platform/src/window.rs:356` and `WindowParams` at `:429`, so the
   `DynRendererFactory` newtype §4.1 introduces is needed on *both* fields, not only
   `WindowParams`. §4.1's fix is right; its scope is one field short.
3. **`foreign_texture_handle` must not put `wgpu` in `gpui_engine`.** The draft's
   `ForeignTextureHandle::Wgpu(wgpu::TextureView)` and
   `DrawContext::wgpu_pass: &mut wgpu::RenderPass` would add `wgpu` to the engine,
   whose dependencies are `gpui_types` and small support crates only
   (`crates/gpui_engine/Cargo.toml`) and whose layer order
   [`../architecture/layer-stack.md`](../architecture/layer-stack.md) draws. Two ways
   out, and `PaintSurface` is the precedent for the first:
   - **Mirror `PaintSurface`**: cfg-gated per-platform variants with a cfg-gated
     optional dependency, so Linux gets `wgpu` in the engine the way macOS gets
     `core-video`. Uniform with the tree; the cost is a heavyweight dependency on the
     scene layer.
   - **Type-erase it**: `ForeignTextureHandle(Box<dyn Any>)` (or a
     `Arc<dyn Any + Send + Sync>`), downcast by the renderer that owns the type. Needs
     nothing new in the engine and matches §4.2's `RendererTarget` trick — which is the
     draft's own best idea, applied to the other end of the pipe.
   The second is the recommendation: the draft already argues for erasure at the
   target, and the same argument holds at the texture.
4. **`TextureId` is not a type in this tree.** The engine's identifiers are
   `AtlasTextureId` (`crates/gpui_engine/src/atlas.rs:254`) and `AtlasKey`; a
   foreign-texture registry needs a new id type, and it is a *per-window* registry
   (see correction 9), not an engine one.
5. **`set_viewport_size` already exists on `SceneRenderer`**
   (`crates/gpui_engine/src/renderer.rs:32`), behind
   `#[cfg(any(test, feature = "test-support", feature = "bench-support"))]`. §4.3's
   `PlatformRenderer` redeclares it and would shadow a trait method. A ruling is
   needed: either the test gate is lifted and the method joins the real contract, or
   `PlatformRenderer` omits it.
6. **`as_scene_renderer`/`into_scene_renderer` are probably unnecessary.** They exist
   because `PlatformWindow::with_renderer` and `present` hand out
   `&mut dyn SceneRenderer` (`crates/gpui_platform/src/platform_window.rs:151`, `:157`),
   so a `Box<dyn PlatformRenderer>` must be seen as a `Box<dyn SceneRenderer>`. The
   toolchain is 1.95 (`rust-toolchain.toml`) and trait upcasting has been stable since
   1.86, so `&mut *boxed as &mut dyn SceneRenderer` covers the first and `Box<dyn
   PlatformRenderer> as Box<dyn SceneRenderer>` the second. Keep the explicit methods
   only if upcasting is deliberately avoided.
7. **The patch set covers Linux only.** `gpui_macos` and `gpui_windows` also construct
   renderers (`crates/gpui_macos/src/window.rs:1100`,
   `crates/gpui_windows/src/window.rs:145`), so with §8 as written the factory compiles
   everywhere and is **silently ignored** on two of four backends. Two more patches
   are implied, and until they exist the feature is Linux-only — which should be said
   in §1 rather than discovered.
8. **`gpui_animotion` is not this tree's.** The RFC preamble already records it: the
   `gpui_animotion` in `.uses` is a third-party declarative *property-animation*
   engine, and a video exporter would be a different crate. §7's pipeline is therefore
   a wish about a crate that does not do that, and should name no crate or an
   unimplemented one.
9. **The registry and the push are per window, not per `cx`.** `paint_quad`
   (`crates/gpui_authoring/src/window.rs:4972`) inserts into the frame's scene through
   `Window`; a foreign-texture registry a `paint` callback needs is the same kind of
   per-frame, per-window state. `cx.register_foreign_texture` / `cx.paint_with_callback`
   are `window.`-level.
10. **`WindowHost::new` is the right boundary and the draft names it correctly**
    (`crates/gpui_authoring/src/window.rs:1441`, called at
    `crates/gpui_authoring/src/app.rs:1270`, destructuring into the `open_window` call
    at `crates/gpui_authoring/src/window.rs:1756`). The one addition: `WindowOptions`
    is *not* destructured exhaustively today in the way §3 shows for the new field —
    it is consumed field by field — so the "silently dropped state" hazard is real and
    worth the `#[deny]`-style test the draft implies.

## What the seam document still owns

The revision's `PlatformRenderer` (§4.3) is a factory- and lifecycle trait, and it has
no place for the methods a **native** window calls on its renderer that do not
generalise: the macOS layer (`crates/gpui_macos/src/window.rs:3087`, `:3237`,
`:3166`) and the Windows background appearance
(`crates/gpui_windows/src/window.rs:1039`). Those are exactly what
[`scene-renderer-seam.md`](scene-renderer-seam.md) §3 handles with a cfg-selected
`PlatformRenderer` alias plus per-platform extension traits. The two documents
compose: **take this revision's §4 for the factory, the target and the trait; keep the
seam document's §3 for the native hooks.** Widening this `PlatformRenderer` with
macOS-only methods instead would break the layer-stack ruling against widening a
shared trait for one platform
([`../architecture/layer-stack.md`](../architecture/layer-stack.md)).

## 1. Executive summary and tenets

GPUI has no escape hatch for an external GPU context. A viewport that renders hundreds
of thousands of vertices, a 3D view, a video decoder or a camera feed must either pay a
CPU readback or fork the engine. The revision answers with two mechanisms that belong
together:

- **an IoC renderer factory**, so a window's `SceneRenderer` is something the
  application supplies rather than something the backend hard-codes; and
- **a dual-path primitive**, so a supplied renderer can either import a texture
  (Path A) or draw into the window's own pass (Path B).

The tenets, restated after the corrections above:

1. **No system-RAM copies.** Frames stay in VRAM; the readback path is the thing being
   removed.
2. **A small, non-breaking upstream footprint.** Corrections 1–3 keep every change
   additive: a new field on two `Debug` structs, a new trait, and per-backend fallback
   arms. The exact budget is in §8, and it is the *factory* budget — the renderer work
   is separate.
3. **Coherence and object safety respected.** `Rc<RefCell<…>>` GPU runtimes are
   single-threaded, so the factory is `Rc`-based and the target is borrowed, not sent.
4. **Declarative DX, RAII-gated authoring.** `GpuCanvas` for applications
   ([`gpu-canvas-dx.md`](gpu-canvas-dx.md)); typed, downcast access for renderer
   authors.
5. **Headless parity.** The same primitives must work offscreen, without a display —
   which is what makes them testable at all, per the seam document.

## 2. The trade-off, in one table

The bandwidth argument is the RFC's and is not restated here
([`dual-path-render-extension.md`](dual-path-render-extension.md) Chapter 2); the
summary is that an RGBA8 round trip costs `W × H × 4 × fps × 2` bytes per second, ≈1.0
GB/s at 1080p60 and ≈8.0 GB/s at 4K120, on the bus between GPU and CPU. The four
candidate paradigms and where they land:

| | primitive translation | offscreen blit | Path B inline | Path A texture |
| --- | --- | --- | --- | --- |
| GPU passes | 1 | 2 | **1** | 2 (zero-copy) |
| CPU readback | none | full | **none** | **none** |
| stacking with UI | native | limited | native | **native** |
| scale ceiling | fails >500k verts | high | **max** | high |
| shader freedom | SDFs only | full | **full** | full |

Path A is the one that reuses what the engine already has (§"What the draft misses");
Path B is the one that needs a new primitive with `order` and `content_mask`.

## 3. The IoC factory and its boundary

```
WindowOptions (gpui_platform)          with_renderer_factory(..)
        │  WindowHost::new destructures
        ▼
WindowParams (gpui_platform)           renderer_factory: Option<DynRendererFactory>
        │  cx.platform.open_window(..)
        ▼
backends: gpui_linux / gpui_macos / gpui_windows
        │  factory.create(target)?   else  the backend's own renderer
        ▼
Box<dyn PlatformRenderer>              held by the window, handed out as dyn SceneRenderer
```

The boundary is `WindowHost::new` (`crates/gpui_authoring/src/window.rs:1441`), which
already builds the `WindowParams` for `Platform::open_window`
(`crates/gpui_platform/src/platform.rs:95`) — the one place a window and its raw
handles come into being, and therefore the only place a surface-bound renderer can be
built. The draft's per-window choice (on `WindowOptions`) is a deliberate change from
[`scene-renderer-seam.md`](scene-renderer-seam.md) §4, which put one factory on
`Application`; per-window is more flexible and both can coexist later (an application
default, a per-window override). Only the per-window field is in this revision.

## 4. Engine contracts

### 4.1 The factory and its `Debug` problem

`WindowParams` and `WindowOptions` both derive `Debug`, so the factory cannot be a
bare closure. The newtype is the fix, and it is needed on both fields (correction 2):

```rust
// crates/gpui_platform/src/platform_renderer.rs
pub trait RendererFactory: 'static {
    fn create(&self, target: RendererTarget<'_>) -> anyhow::Result<Box<dyn PlatformRenderer>>;
}

#[derive(Clone)]
pub struct DynRendererFactory(Rc<dyn RendererFactory>);

impl std::fmt::Debug for DynRendererFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DynRendererFactory(..)")
    }
}

/// The prelude's spelling: a closure is a factory.
pub struct FnRendererFactory<F>(pub F);

impl<F> RendererFactory for FnRendererFactory<F>
where
    F: Fn(RendererTarget<'_>) -> anyhow::Result<Box<dyn PlatformRenderer>> + 'static,
{
    fn create(&self, target: RendererTarget<'_>) -> anyhow::Result<Box<dyn PlatformRenderer>> {
        (self.0)(target)
    }
}
```

The newtype is what keeps coherence honest (E0119): `impl RendererFactory for F where
F: Fn(..)` would collide with any other blanket impl the moment one appears.

### 4.2 The target, type-erased

The draft's best idea, and the reason `gpui_platform` needs no `raw-window-handle`
dependency and no knowledge of `wayland_client` or `windows::Win32`:

```rust
// crates/gpui_platform/src/platform_renderer.rs
pub struct RendererTarget<'a> {
    raw: &'a dyn std::any::Any,
}

impl<'a> RendererTarget<'a> {
    pub fn new(raw: &'a dyn std::any::Any) -> Self { Self { raw } }
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> { self.raw.downcast_ref::<T>() }
}
```

Each backend keeps its concrete target private and passes `&concrete as &dyn Any`:

```rust
// crates/gpui_linux/src/linux/platform_renderer.rs
pub enum LinuxRendererTarget<'a> {
    Wayland {
        raw_window: &'a RawWindow,
        gpu_context: gpui_wgpu::GpuContext,
        config: gpui_wgpu::WgpuSurfaceConfig,
        compositor_gpu: Option<gpui_wgpu::CompositorGpuHint>,
    },
    #[cfg(feature = "x11")]
    X11 { /* as above */ },
    Recovery(&'a RawWindow),
}
```

with `MacRendererTarget` (`renderer::Context`, native view, size) and
`WindowsRendererTarget` (`HWND`, devices, composition flag) the same way.

### 4.3 `PlatformRenderer`

The renderer contract, extending `SceneRenderer` with the lifecycle the window needs.
`set_viewport_size` is omitted pending correction 5; the recovery target is the erased
one.

```rust
// crates/gpui_platform/src/platform_renderer.rs
pub trait PlatformRenderer: SceneRenderer {
    fn max_texture_size(&self) -> u32;
    fn set_subpixel_layout(&mut self, _is_bgr: bool) {}
    fn update_transparency(&mut self, _transparent: bool) {}
    fn destroy(&mut self) {}
    fn device_lost(&self) -> bool { false }
    fn needs_redraw(&mut self) -> bool { false }

    #[cfg(not(target_family = "wasm"))]
    fn recover(&mut self, _target: RendererTarget<'_>) -> anyhow::Result<()> {
        anyhow::bail!("renderer does not support recovery")
    }
}
```

and the native hooks stay off it, per "What the seam document still owns".

## 5. The dual-path primitive

Read `PaintSurface` (`crates/gpui_engine/src/scene.rs:749`) as the template. Path A
generalises it; Path B is its sibling with a callback instead of a payload:

```rust
// crates/gpui_engine/src/scene.rs — beside Primitive::Surface
pub enum CustomRenderPrimitive {
    /// Path A: a texture produced outside GPUI, sampled in the composite pass.
    Texture {
        id: ForeignTextureId,          // NOT TextureId; per-window, see correction 4
        handle: ForeignTextureHandle,  // type-erased, see correction 3
        bounds: Bounds<ScaledPixels>,
        content_mask: ContentMask<ScaledPixels>,  // PaintSurface has one; the draft omits it
        radii: Corners<Pixels>,        // not `CornerRadii`
        opacity: f32,
        flip_v: bool,
    },
    /// Path B: commands executed into the window's active pass.
    Inline {
        order: DrawOrder,              // PaintSurface carries one; interleaving needs it
        bounds: Bounds<ScaledPixels>,
        content_mask: ContentMask<ScaledPixels>,
        callback: Box<dyn Fn(&mut DrawContext) + Send + Sync>,
    },
}
```

Bounds are `ScaledPixels` in this engine (`crates/gpui_engine/src/scene.rs:220`), not
`Pixels` — the element converts at paint time.

The two execution lifecycles are the RFC's, with one correction each: for Path A, the
renderer binds the imported view in the *same* pass that composites the quad batch
(the generalisation of `draw_surfaces`), and for Path B, the pause/restore must also
restore the batch cursor, because the primitive sits *inside* the batch list rather
than after it.

## 6. The DX

Unchanged from [`gpu-canvas-dx.md`](gpu-canvas-dx.md), which is the authoring half of
this document: a `GpuCanvas` composed from `div` and `canvas`, delegating `Styled`,
`InteractiveElement` and `Element`, with `on_render_texture` / `on_render_inline`. Read
that document for the shape; the only difference this revision makes is that the
factory is per window rather than process-wide.

## 7. Headless export

The parity claim is the seam document's testability argument, not a video pipeline: the
same primitive must render offscreen, which is what lets a test read it back. Corrections
7 and 8 apply — the pipeline names no crate in this tree.

## 8. The upstream patch set

The factory seam only. Every line is additive.

| patch | file | change | size |
| --- | --- | --- | --- |
| 01 | `crates/gpui_platform/src/platform_renderer.rs` | `PlatformRenderer`, `RendererFactory`, `DynRendererFactory`, `RendererTarget`, `FnRendererFactory` | new file, ~90 LOC |
| 02 | `crates/gpui_platform/src/window.rs` | `renderer_factory` field on `WindowParams` and on `WindowOptions` (both `Debug`, hence the newtype) | ~4 LOC |
| 03 | `crates/gpui_authoring/src/window.rs` | forward the field through `WindowHost::new`'s destructure into the `open_window` call | ~6 LOC |
| 04 | `crates/gpui_linux/src/linux/wayland/window.rs` | consult the factory, else `WgpuRenderer::new` | ~18 LOC |
| 05 | `crates/gpui_linux/src/linux/x11/window.rs` | as 04, plus the post-init `set_subpixel_layout` | ~22 LOC |
| 06 | `crates/gpui_macos/src/window.rs` | as 04, else `renderer::new_renderer` (`:1100`) — **missing from the draft** | ~20 LOC |
| 07 | `crates/gpui_windows/src/window.rs` | as 04, else `DirectXRenderer::new` (`:145`) — **missing from the draft** | ~20 LOC |

**Factory footprint: ~180 LOC, four backends.** Patches 06 and 07 are correction 7.
The dual-path primitives are *not* in this table: Path A adds a real arm to three
renderers (wgpu's `Surfaces` is empty today,
`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`) and Path B adds pause/restore to each, and
neither is a patch to a `window.rs`. Keeping the two budgets separate is what the
draft's single "<150 LOC" claim obscures.

## What this settles, and what it leaves open

**Settled by this revision:** the factory is per window and lives on
`WindowOptions`/`WindowParams`; the target is type-erased; the closure needs a
`Debug` newtype; Path A generalises an existing primitive rather than inventing one.

**Open, and worth a decision before implementing:**

- **Correction 3** — cfg'd `wgpu` in the engine, following `core-video`, or a
  type-erased handle. The recommendation is erasure.
- **Correction 5** — lift `set_viewport_size`'s test gate and put it on the real
  contract, or drop it from `PlatformRenderer`.
- **Correction 6** — upcast or keep the explicit conversions.
- **The native hooks** — take the seam document's §3, and check it still fits once
  `PlatformRenderer` exists.
