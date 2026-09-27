# The renderer seam: giving the fifth SPI a bootstrap

- **Status:** proposed. Nothing of it is implemented.
- **Supersedes:** the drafts' factory and target designs — the seam draft's §2–§5 and the
  IoC draft's §3–§4 — which were removed once this chapter carried them
  ([`README.md`](README.md) has the reconciliation). One of §3's rulings also reverses the
  seam draft's §1, which widened `SceneRenderer`; this does not, for the reason recorded
  there.
- **Prerequisite for:** [`foreign-texture.md`](foreign-texture.md) and
  [`inline-commands.md`](inline-commands.md). Those extend what a renderer can be handed;
  this makes the renderer itself installable. Neither replaces the other.
- **Target crates:** `gpui_engine`, `gpui_platform`, `gpui_authoring`, `gpui_runtime`,
  the backend crates (`gpui_macos`, `gpui_apple`, `gpui_windows`, `gpui_linux`,
  `gpui_web`) and the renderer crates (`gpui_wgpu`, `gpui_apple`).

## 1. The gap, in one sentence

Four of the five seams are entered through a single `Application::with_*` call.
`SceneRenderer`, the fifth, has none: every backend window constructs its renderer by
concrete type and calls inherent methods on it, so there is no point at which an
application — or a crate outside the tree — can say *which* renderer to use.

[`../architecture/frame-flow.md`](../../architecture/frame-flow.md) states the same from the
frame side: `PlatformWindow::present` hands the renderer a closure and takes back
*whether the frame was presented*; that is where the fifth seam is entered, and it is
below the pipeline.

## 2. Where the implementations already are

The separation the extraction was meant to achieve is largely **already done** at this
ref. `SceneRenderer` is defined at `crates/gpui_engine/src/renderer.rs:16`, and there are
six implementations:

| implementation | crate | `impl` at | window field |
| --- | --- | --- | --- |
| `MetalRenderer` | `gpui_apple` | `crates/gpui_apple/src/metal_renderer.rs:1608` | `crates/gpui_macos/src/window.rs:667` |
| `MetalHeadlessRenderer` | `gpui_apple` | `crates/gpui_apple/src/metal_renderer.rs:1652` | `crates/gpui_apple/src/metal_renderer.rs:1635` |
| `WgpuRenderer` | `gpui_wgpu` | `crates/gpui_wgpu/src/wgpu_renderer.rs:2433` | `crates/gpui_linux/src/linux/x11/window.rs:271` |
| `DirectXRenderer` | `gpui_windows` | `crates/gpui_windows/src/directx_renderer.rs:2086` | `crates/gpui_windows/src/window.rs:67` |
| `HeadlessRenderer` | `gpui_linux` | `crates/gpui_linux/src/linux/headless/window.rs:258` | `crates/gpui_linux/src/linux/headless/window.rs:57` |
| `TestRenderer` | `gpui_authoring` | `crates/gpui_authoring/src/platform/test/window.rs:587` | `crates/gpui_authoring/src/platform/test/window.rs:27` |

Two of the platforms do not own their renderer at all any more. `gpui_macos` reaches
Metal through a re-export (`crates/gpui_macos/src/gpui_macos.rs:18`), and both Linux
display backends already render through `gpui_wgpu` — x11 constructs it at
`crates/gpui_linux/src/linux/x11/window.rs:769`, wayland at
`crates/gpui_linux/src/linux/wayland/window.rs:582`, and web at
`crates/gpui_web/src/window.rs:172`. `DirectXRenderer` is the one renderer still living
*inside* its platform crate, and even there it is its own file.

**So the extraction is not the work.** A renderer can already live in a crate of its
own. What is missing is that the *type* is still concrete at every use site: a window
holds `WgpuRenderer`, `renderer::Renderer` (an alias for `MetalRenderer`,
`crates/gpui_apple/src/metal_renderer.rs:47`) or `RefCell<DirectXRenderer>`, never a
`dyn SceneRenderer`, and calls backend-specific methods that are not on the trait.

## 3. What each backend asks of its renderer

Read the left column as the rule and the right as the exception: almost everything a
window wants from a renderer is a property any *onscreen* renderer has.

| method | asked by |
| --- | --- |
| `draw`, `sprite_atlas` | all (the trait) |
| `update_drawable_size` | x11 `crates/gpui_linux/src/linux/x11/window.rs:1308`, wayland `crates/gpui_linux/src/linux/wayland/window.rs:1487`, web `crates/gpui_web/src/window.rs:988`, macos `crates/gpui_macos/src/window.rs:3096` |
| `update_transparency` | x11 `crates/gpui_linux/src/linux/x11/window.rs:1365`, macos `crates/gpui_macos/src/window.rs:1861` |
| `set_subpixel_layout` | x11 `crates/gpui_linux/src/linux/x11/window.rs:772`, wayland `crates/gpui_linux/src/linux/wayland/window.rs:653` |
| `max_texture_size` | x11 `crates/gpui_linux/src/linux/x11/window.rs:776`, wayland `crates/gpui_linux/src/linux/wayland/window.rs:596` |
| `destroy` | x11 `crates/gpui_linux/src/linux/x11/window.rs:881`, wayland `crates/gpui_linux/src/linux/wayland/window.rs:771`, macos `crates/gpui_macos/src/window.rs:1386` |
| `device_lost` / `recover` / `needs_redraw` | x11 `crates/gpui_linux/src/linux/x11/window.rs:1756`, wayland `crates/gpui_linux/src/linux/wayland/window.rs:1949` |
| `gpu_specs` | x11 `crates/gpui_linux/src/linux/x11/window.rs:1969`, wayland `crates/gpui_linux/src/linux/wayland/window.rs:2141`, web `crates/gpui_web/src/window.rs:1007`, windows `crates/gpui_windows/src/window.rs:1048` |
| `layer`, `layer_ptr`, `set_presents_with_transaction` | macos `crates/gpui_macos/src/window.rs:3087` (defined `crates/gpui_apple/src/metal_renderer.rs:362`) |
| `set_background_appearance` | windows `crates/gpui_windows/src/window.rs:1039` |

Everything above the macOS and Windows rows generalises *to an onscreen renderer*. Those
two rows are genuinely native: the macOS window hosts a `CAMetalLayer` and hands it out
(`crates/gpui_macos/src/window.rs:3237`), and the Windows window drives a
DirectComposition visual tree.

**The ruling this implies.** The first draft read this table as "widen `SceneRenderer`
with the generalising rows, defaulted". That is not needed, and it costs something: the
window is going to hold a `Box<dyn PlatformRenderer>` after this change, so every method
in the table is already reachable without touching a published trait. Widening
`SceneRenderer` would freeze a trait that a second implementation outside the tree will
one day implement, for methods no engine consumer calls — `SceneRenderer`'s own doc
comment says an onscreen renderer needs only `draw` and `sprite_atlas`. So
`SceneRenderer` is **unchanged**, `GpuSpecs` **stays where it is**
(`crates/gpui_platform/src/gpu.rs:5`, no move to the engine), and the lifecycle lands on
`PlatformRenderer` — which is also what removes the need for a decision about
`set_viewport_size`, below.

## 4. The one place injection already works

`TestPlatform` accepts a renderer factory
(`crates/gpui_authoring/src/platform/test/platform.rs:54`), `TestWindow` takes the result
as `Option<Box<dyn SceneRenderer>>` and falls back to `TestRenderer::new()`
(`crates/gpui_authoring/src/platform/test/window.rs:86`), and `HeadlessAppContext::new`
threads the same factory through
(`crates/gpui_authoring/src/app/headless_app_context.rs:68`). That is the whole design,
working, on one platform: **a factory returning a boxed trait object, with the backend's
own renderer as the fallback.** It is already `Box<dyn SceneRenderer>` there because
nothing in the test window needs a native method. This proposal is that pattern,
generalised to the real backends — and the name `renderer_factory` mirrors
`TestPlatform`'s existing `headless_renderer_factory` deliberately.

## 5. The contracts

### 5.1 `PlatformRenderer`

The window-facing lifecycle. Every method keeps the name the backends already call it by,
so a port is a move rather than a rename:

```rust
// crates/gpui_platform/src/platform_renderer.rs
pub trait PlatformRenderer: SceneRenderer {
    fn update_drawable_size(&mut self, size: Size<DevicePixels>);
    fn max_texture_size(&self) -> u32;
    fn gpu_specs(&self) -> Option<GpuSpecs>;

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

`update_drawable_size` and `max_texture_size` are the two without defaults, because a
renderer that cannot answer them cannot be a window's. `recover` takes a
`RendererTarget<'_>` and not the raw window, because a lost surface is rebuilt from the
handles the renderer was built with, and the shared trait must not name `metal::*` or
`ID3D11Device*`.

**What `set_viewport_size`'s collision turned into.** Both drafts treated the test-gated
`SceneRenderer::set_viewport_size` (`crates/gpui_engine/src/renderer.rs:32`) as a problem
to resolve by lifting the gate or dropping a duplicate. Neither is needed once the
lifecycle is on `PlatformRenderer`: the test-gated method keeps its one caller
(`crates/gpui_authoring/src/platform/test/window.rs:531`) and its one override
(`crates/gpui_apple/src/metal_renderer.rs:1660`), and the production path gets the name
it already has.

### 5.2 The native hooks stay on the platform

The macOS layer methods and the Windows background-appearance method do not generalise,
and the layer-stack ruling is explicit about this shape
([`../architecture/layer-stack.md`](../../architecture/layer-stack.md), "A leaky
abstraction gets an escape hatch, not a new method"). So they do not join
`PlatformRenderer`. Instead the trait *requires* them on the platforms that have them, so
a backend can reach them without a downcast and a renderer author has one trait to
implement per platform:

```rust
// crates/gpui_platform/src/platform_renderer.rs
#[cfg(target_os = "macos")]
pub trait MacSceneRenderer: SceneRenderer {
    fn layer_ptr(&self) -> *mut std::ffi::c_void;
    fn set_presents_with_transaction(&mut self, value: bool);
}

#[cfg(target_os = "windows")]
pub trait WinSceneRenderer: SceneRenderer {
    fn set_background_appearance(&mut self, appearance: WindowBackgroundAppearance);
}

// The native hook as a supertrait, without a second copy of the lifecycle: a trait's
// supertraits cannot be cfg-selected, so this is the trait that can be, and the blanket
// implementation is why no renderer writes this one by hand.
#[cfg(target_os = "macos")]
pub trait NativeSceneHooks: MacSceneRenderer {}
#[cfg(target_os = "windows")]
pub trait NativeSceneHooks: WinSceneRenderer {}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub trait NativeSceneHooks: SceneRenderer {}

#[cfg(target_os = "macos")]
impl<T: MacSceneRenderer + ?Sized> NativeSceneHooks for T {}
#[cfg(target_os = "windows")]
impl<T: WinSceneRenderer + ?Sized> NativeSceneHooks for T {}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl<T: SceneRenderer + ?Sized> NativeSceneHooks for T {}

// `&dyn PlatformRenderer` upcasts to `MacSceneRenderer` through this, so no backend
// downcasts to reach its own — the upcast being transitive is what makes one definition
// enough.
pub trait PlatformRenderer: NativeSceneHooks { /* … as 5.1 … */ }
```

A `WgpuRenderer` on macOS implements `MacSceneRenderer`, and the probe that asked found it does
not have to return a `CAMetalLayer` that matters: wgpu downcasts the view's root layer and
inserts its own when the downcast fails, so what `layer_ptr` returns decides only what the
window's `-[NSView makeBackingLayer]` has to return — the one corner that probe leaves open
([`../../decisions/macos-presentation-probe.md`](../../decisions/macos-presentation-probe.md)). This is
the one part of the seam draft's §3 that is still load-bearing; it put the alias
`PlatformRenderer = dyn MacSceneRenderer` in place of the trait, and the supertrait is the
one change that makes it compose with the lifecycle.

### 5.3 The target: typed handles, one erased extra

A renderer binds to a surface, and the surface comes from the window, so the factory is
handed a target the platform builds:

```rust
// gpui_platform, beside PlatformWindow
pub struct RendererTarget<'a> {
    pub window_handle: Option<RawWindowHandle>,
    pub display_handle: Option<RawDisplayHandle>,
    pub size: Size<Pixels>,
    pub scale_factor: f32,
    pub transparent: bool,
    /// Backend-specific extras, for that backend's own renderer only. `Any` is
    /// `'static`, so this must point at owned data; a backend with nothing to add
    /// passes `None`.
    pub backend: Option<&'a dyn Any>,
}
```

The handles are typed rather than erased because erasing them makes any renderer that is
not the backend's own downcast to the backend's target type, which means depending on the
backend crate — and `gpui_linux` already depends on `gpui_wgpu`, so that is a cycle.
`gpui_platform` already depends on `raw-window-handle` (workspace `0.6`), so naming the
handles costs nothing and `wgpu` still does not enter the platform layer.

The payoff is larger than the argument. With the handles typed, the per-platform downcast
cascade the adaptors draft specified — `surface_info`, `window_handle`,
`display_handle`, each with three or four `#[cfg]` arms and a
`downcast_ref::<LinuxRendererTarget>()` — collapses to field reads, and `gpui_wgpu` stops
naming a backend crate altogether. Surface creation is a plain
`create_surface_unsafe` over the two handles.

The three `WindowsRendererTarget`/`MacRendererTarget`/`LinuxRendererTarget` types the
drafts invented for those downcasts do not exist and are not needed: `RawWindowHandle`
already carries the `NSView` (`AppKitWindowHandle`) and the `HWND`
(`Win32WindowHandle`), and a backend's own `WgpuSurfaceConfig`-shaped extras travel in
`backend` — which is what `backend` is for.

### 5.4 The factory, and its `Debug` problem

`WindowOptions` and `WindowParams` both derive `Debug`
(`crates/gpui_platform/src/window.rs:356`, `:429`), so the factory cannot be a bare
closure and a trait object cannot derive it:

```rust
pub trait RendererFactory: 'static {
    fn create(&self, target: RendererTarget<'_>) -> anyhow::Result<Box<dyn PlatformRenderer>>;
}

#[derive(Clone)]
pub struct DynRendererFactory(pub Rc<dyn RendererFactory>);

impl std::fmt::Debug for DynRendererFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DynRendererFactory(..)")
    }
}

/// The spelling for the common case: a closure is a factory.
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

The newtype is also what keeps coherence honest: `impl RendererFactory for F where F:
Fn(..)` would collide (E0119) with any other blanket impl the moment one appears.

### 5.5 Where it lives, and how it reaches the window

**On the window**, as `renderer_factory: Option<DynRendererFactory>` on `WindowOptions`
(`crates/gpui_platform/src/window.rs:357`) and `WindowParams` (`:438`), with a
`with_renderer_factory` builder. A process-wide default on `App` — which is what the
seam draft specified, beside the other two factories
(`crates/gpui_authoring/src/app.rs:629`, `:633`) — can coexist later as an application
default with a per-window override; only the per-window field is specified here, because
only it can be honest about the surface.

The renderer, unlike the layout engine and the pipeline, cannot be built on the
`gpui_authoring` side: it needs the window's surface, which only the platform window
has. So the field must reach `Platform::open_window`
(`crates/gpui_platform/src/platform.rs:95`), and `gpui_authoring` already builds the
`WindowParams` for that call (`crates/gpui_authoring/src/window.rs:1756`). The boundary
is `WindowHost::new` (`crates/gpui_authoring/src/window.rs:1441`, called from
`crates/gpui_authoring/src/app.rs:1270`), and each backend does:

```rust
let renderer: Box<dyn PlatformRenderer> = match params.renderer_factory {
    Some(factory) => factory.0.create(target)?,
    None => Box::new(WgpuRenderer::new(gpu_context, &raw_window, config, hint)?),
};
```

Two alternatives were weighed and are recorded rather than chosen: a new argument on
`Platform::open_window` (explicit, but changes a shared SPI signature and every backend),
and a `PlatformWindow::install_renderer` called right after creation (touches no SPI, but
every backend must then defer building its default renderer until first use, or build one
and throw it away).

`WindowOptions` is not currently destructured exhaustively at this site — it is consumed
field by field — so a field added to it can be silently dropped. The forwarding patch
should be covered by a test that would fail if it were, and the destructure should be
made exhaustive.

### 5.6 Invoked once, so recovery must be self-sufficient

The factory runs exactly once per window: the native surface must exist before a renderer
can bind to it, so there is no second target resolution and no re-invocation on a
resize or a device loss. Two consequences:

- **Post-construction queries belong on the renderer, not on the factory's input.**
  `max_texture_size` is asked immediately after construction to set the OS toplevel's
  size hints, and `set_subpixel_layout` right after it on x11
  (`crates/gpui_linux/src/linux/x11/window.rs:772`).
- **`recover` has to rebuild from what the renderer already holds.** Its shape at this
  ref is the model: `WgpuRenderer::recover<W>(&mut self, window: &W)`
  (`crates/gpui_wgpu/src/wgpu_renderer.rs:2131`), called from `present` guarded by
  `device_lost()` on x11 (`crates/gpui_linux/src/linux/x11/window.rs:1756`, `:1765`) and
  wayland (`crates/gpui_linux/src/linux/wayland/window.rs:1949`, `:1960`). The subtle
  part is that windows share one GPU context
  (`GpuContext = Rc<RefCell<Option<WgpuContext>>>`,
  `crates/gpui_wgpu/src/wgpu_renderer.rs:168`): the first window to notice rebuilds it
  with `WgpuContext::new_rejecting_software`
  (`crates/gpui_wgpu/src/wgpu_context.rs:76`) and the rest adopt what it left. The same
  protocol is needed on all four platforms — macOS and Windows lose devices too — and it
  is one protocol implemented three times, not a Linux-only concern.

On the recovery path the renderer is handed a `RendererTarget` built from the surface it
already has, not a new one from the factory.

### 5.7 The default path is unchanged

The correctness gate for the whole change is that an application which never calls
`with_renderer_factory` builds the renderer it builds today, by the same construction
call, with the same resize, transparency and device-loss behaviour. Concretely: with
`None`, each backend runs the same constructor expression it runs now, and the only
difference is that the result is held as `Box<dyn PlatformRenderer>` instead of the
concrete type.

## 6. The upstream patch set

The factory seam only. Every line is additive.

| patch | file | change | size |
| --- | --- | --- | --- |
| 01 | `crates/gpui_platform/src/platform_renderer.rs` | `PlatformRenderer`, the cfg-selected native traits, `RendererFactory`, `DynRendererFactory`, `FnRendererFactory`, `RendererTarget` | new file, ~90 LOC |
| 02 | `crates/gpui_platform/src/window.rs` | `renderer_factory` on `WindowParams` and on `WindowOptions`, plus the builder (both `Debug`, hence the newtype) | ~6 LOC |
| 03 | `crates/gpui_authoring/src/window.rs` | forward the field through `WindowHost::new` into the `open_window` call | ~6 LOC |
| 04 | `crates/gpui_linux/src/linux/wayland/window.rs` | consult the factory, else `WgpuRenderer::new` | ~18 LOC |
| 05 | `crates/gpui_linux/src/linux/x11/window.rs` | as 04 | ~18 LOC |
| 06 | `crates/gpui_macos/src/window.rs` | as 04, else `renderer::new_renderer` (`:1100`) | ~20 LOC |
| 07 | `crates/gpui_windows/src/window.rs` | as 04, else `DirectXRenderer::new` (`:145`) | ~20 LOC |

**Factory footprint: ~180 LOC across four backends.** The dual-path primitives are *not*
in this budget: Path A adds a fragment path and a pipeline to two renderers — wgpu's batch arm
is empty today (`crates/gpui_wgpu/src/wgpu_renderer.rs:1546`) and the only surface fragment
that samples a non-atlas texture is a YCbCr one
(`crates/gpui_wgpu/src/shaders.wgsl:1350`) — and Path B adds pause/restore to each. Neither is
a patch to a `window.rs`, and keeping the two budgets separate is what the drafts' single
"<150 LOC" claim obscured.

Two things patch 07 needs that the factory does not cover, both measured after this section
was written. `gpui_wgpu`'s instance is `Backends::VULKAN | Backends::GL`
(`crates/gpui_wgpu/src/wgpu_context.rs:292`), which finds no adapter on Windows, so installing
`WgpuRenderer` there also means enabling `Backends::DX12`; and the DX12 surface offers only
`Opaque` alpha, so a window it renders cannot be transparent the way the default renderer's
can. Both are in
[`../../decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md).

## 7. Migration, in order

1. Add `PlatformRenderer` (with the cfg-selected native traits), `RendererTarget`, the
   factory types and the `Debug` newtype to `gpui_platform`.
2. Change each backend window's renderer field to `Box<dyn PlatformRenderer>`, move the
   inherent methods of §3 onto the trait, and port the call sites.
3. Add `renderer_factory` to `WindowOptions`/`WindowParams` and the builder; forward it
   through `WindowHost::new`; each backend consults it before its default.
4. Add a test that installs a custom renderer through the factory, asserts its `draw` was
   called, and asserts that omitting the call still draws.

Step 4's pattern is already proven on one platform by `TestPlatform` (§4), so it is the
first thing to port rather than the last.

Two optional items, neither of which injection needs:

- **Move `DirectXRenderer` to a `gpui_directx` crate**, the way Metal moved to
  `gpui_apple` and wgpu already has its own; `gpui_windows` re-exports the few names it
  keeps, the same shim `gpui_macos` uses (`crates/gpui_macos/src/gpui_macos.rs:18`). This
  is symmetry, and it needs a Windows host to verify.
- **A process-wide default on `App`** (§5.5), once there is a reason for one.

## 8. What can be verified here, and what cannot

Steps 1–4 are platform-agnostic in shape and land on the Linux and headless paths, which
compile and run on this host; `TestPlatform` is the working proof the pattern holds. The
macOS and Windows halves of step 2 need a macOS or Windows host — and so does the *only*
thing that would catch a mis-wired native hook.

The tools repository records that as a standing gap: every Linux gate can be green while
a platform backend does not compile. It is closed by
[`verification.md`](verification.md) §"The gate", which now runs each backend on its own
platform in CI.

## 9. What this does not change

- **`Platform`'s own seam is untouched.** The factory travels through `WindowParams`
  rather than adding a method to `Platform`.
- **`SceneRenderer` is untouched** (§3) — no widened trait, no moved `GpuSpecs`, no
  decision owed about its test-gated `set_viewport_size`.
- **Presentation timing stays below the pipeline.** `SceneRenderer::draw` still returns
  whether it presented, and `PlatformWindow::present`
  (`crates/gpui_platform/src/platform_window.rs:157`) still feeds that back to the
  backend's frame loop. Nothing here gives a `FramePipeline` a vblank.
- **No custom GPU primitives yet.** `CustomRenderPrimitive`, texture import and inline
  injection are [`foreign-texture.md`](foreign-texture.md) and
  [`inline-commands.md`](inline-commands.md); this document only makes the renderer that
  would implement them installable.

## 10. What would reopen it

- **The macOS layer ownership was the hard part, and it is measured.** A non-Metal renderer on
  macOS can be handed the window's layer and does not even need one: wgpu downcasts the view's
  root layer and inserts its own `CAMetalLayer` when the downcast fails, so `MacSceneRenderer`
  keeps its shape and the layer stays where it is. The one corner left is a `makeBackingLayer`
  that returns nil — [macos-presentation-probe.md](../../decisions/macos-presentation-probe.md).
- **If the target is not enough to build a surface.** `RendererTarget` carries the raw
  handles, the geometry and one erased extra. If a backend needs more of the window at
  construction, the factory's signature is the thing to revisit, not the trait.
- **If the native hooks need more than a supertrait.** Trait upcasting is what lets
  `&dyn PlatformRenderer` reach `layer_ptr`; it has been stable since 1.86 and
  `rust-toolchain.toml` is 1.95, so this is only a risk if the toolchain moves back.
