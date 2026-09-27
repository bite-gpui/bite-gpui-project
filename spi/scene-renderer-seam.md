# The scene-renderer seam: giving the fifth SPI a bootstrap

- **Target crates:** `gpui_engine`, `gpui_platform`, `gpui_authoring`, `gpui_runtime`,
  the backend crates (`gpui_macos`, `gpui_apple`, `gpui_windows`, `gpui_linux`,
  `gpui_web`) and the renderer crates (`gpui_wgpu`, `gpui_apple`)
- **Status:** proposed. Nothing of it is implemented.
- **Relationship to the other documents:** this is the *prerequisite* for
  [`dual-path-render-extension.md`](dual-path-render-extension.md). That RFC extends
  what a renderer can be handed; this one makes the renderer itself *installable*.
  Neither replaces the other.

## The gap, in one sentence

Four of the five seams are entered through a single `Application::with_*` call.
`SceneRenderer`, the fifth, has none: every backend window constructs its renderer by
concrete type and calls inherent methods on it, so there is no point at which an
application — or a crate outside the tree — can say *which* renderer to use.

[`../architecture/frame-flow.md`](../architecture/frame-flow.md) states the same from
the frame side: `PlatformWindow::present` hands the renderer a closure and takes back
*whether the frame was presented*; that is where the fifth seam is entered, and it is
below the pipeline.

## Where the implementations already are

The separation the extraction was meant to achieve is largely **already done** at this
ref, which is worth knowing before proposing to do it again. `SceneRenderer` is
defined at `crates/gpui_engine/src/renderer.rs:16`, and there are six implementations:

| implementation | crate | `impl` at | window field |
| --- | --- | --- | --- |
| `MetalRenderer` | `gpui_apple` | `crates/gpui_apple/src/metal_renderer.rs:1608` | `crates/gpui_macos/src/window.rs:667` |
| `MetalHeadlessRenderer` | `gpui_apple` | `crates/gpui_apple/src/metal_renderer.rs:1652` | `crates/gpui_apple/src/metal_renderer.rs:1635` |
| `WgpuRenderer` | `gpui_wgpu` | `crates/gpui_wgpu/src/wgpu_renderer.rs:2433` | `crates/gpui_linux/src/linux/x11/window.rs:271` |
| `DirectXRenderer` | `gpui_windows` | `crates/gpui_windows/src/directx_renderer.rs:2086` | `crates/gpui_windows/src/window.rs:67` |
| `HeadlessRenderer` | `gpui_linux` | `crates/gpui_linux/src/linux/headless/window.rs:258` | `crates/gpui_linux/src/linux/headless/window.rs:57` |
| `TestRenderer` | `gpui_authoring` | `crates/gpui_authoring/src/platform/test/window.rs:587` | `crates/gpui_authoring/src/platform/test/window.rs:27` |

Two of the platforms do not own their renderer at all any more. `gpui_macos` reaches
Metal through a re-export (`crates/gpui_macos/src/gpui_macos.rs:18`, `use
gpui_apple::metal_renderer as renderer;`), and both Linux display backends already
render through `gpui_wgpu` — x11 constructs it at
`crates/gpui_linux/src/linux/x11/window.rs:769`, wayland at
`crates/gpui_linux/src/linux/wayland/window.rs:582`, and web at
`crates/gpui_web/src/window.rs:172`. `DirectXRenderer` is the one renderer still
living *inside* its platform crate, and even there it is its own file.

**So the extraction is not the work.** A renderer can already live in a crate of its
own. What is missing is that the *type* is still concrete at every use site: a window
holds `WgpuRenderer`, `renderer::Renderer` (an alias for `MetalRenderer`,
`crates/gpui_apple/src/metal_renderer.rs:47`) or `RefCell<DirectXRenderer>`, never a
`dyn SceneRenderer`, and calls backend-specific methods that are not on the trait.

## What each backend asks of its renderer

Read the left column as the rule and the right as the exception: almost everything a
window wants from a renderer is a property any onscreen renderer has.

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

Everything above the macOS and Windows rows generalises. Those two rows are genuinely
native: the macOS window hosts a `CAMetalLayer` and hands it out
(`crates/gpui_macos/src/window.rs:3237`), and the Windows window drives a
DirectComposition visual tree.

## The one place injection already works

`TestPlatform` accepts a renderer factory
(`crates/gpui_authoring/src/platform/test/platform.rs:54`), `TestWindow` takes the
result as `Option<Box<dyn SceneRenderer>>` and falls back to `TestRenderer::new()`
(`crates/gpui_authoring/src/platform/test/window.rs:86`), and `HeadlessAppContext::new`
threads the same factory through
(`crates/gpui_authoring/src/app/headless_app_context.rs:68`). That is the whole
design, working, on one platform: **a factory returning a boxed trait object, with the
backend's own renderer as the fallback.** It is already `Box<dyn SceneRenderer>` there
because nothing in the test window needs a native method. This proposal is the
generalisation of that pattern to the real backends.

## The goal

An application — or a crate outside the tree — supplies a renderer through one
`Application` call, exactly as it supplies a text system or a layout engine:

```rust
application()
    .with_renderer(MyRendererFactory)   // omitted: the backend's own renderer, as today
    .with_text_system(ParleyTextSystem::new())
    .run(...);
```

Omitted, nothing changes: each backend installs the renderer it ships with. Present,
that renderer draws every window instead — `gpui_wgpu::WgpuRenderer` on a backend that
does not default to it, or a renderer that has no crate yet. This is what makes
[`dual-path-render-extension.md`](dual-path-render-extension.md) reachable: the
custom primitives that RFC adds are only useful to a renderer the application chose.

## The design

### 1. Generalise the trait; do not fork it

The methods in the upper rows generalise, so they belong on `SceneRenderer` itself,
each defaulted so that existing implementations — including `HeadlessRenderer`, whose
whole body is `draw` and `sprite_atlas` — compile unchanged:

```rust
pub trait SceneRenderer: 'static {
    fn draw(&mut self, scene: &Scene) -> bool;
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;

    // Added, all defaulted. A renderer that cannot present gives the honest answer:
    // not lost, no redraw owed, no GPU to name.
    fn resize(&mut self, size: Size<DevicePixels>) {}
    fn set_transparency(&mut self, transparent: bool) {}
    fn set_subpixel_layout(&mut self, is_bgr: bool) {}
    fn max_texture_size(&self) -> u32 { 8192 }
    fn destroy(&mut self) {}
    fn device_lost(&self) -> bool { false }
    fn needs_redraw(&self) -> bool { false }
    fn gpu_specs(&self) -> Option<GpuSpecs> { None }
    fn recover(&mut self, target: &RendererTarget<'_>) -> anyhow::Result<()> { Ok(()) }
}
```

Two things move with it:

- **`gpu_specs`.** Its value type sits in `gpui_platform`
  (`crates/gpui_platform/src/gpu.rs:5`) but `SceneRenderer` is in `gpui_engine`, and
  `gpui_platform` depends on `gpui_engine` and not the reverse. `GpuSpecs` is engine-
  level capability information — nothing in it names an OS window — so it moves down
  to `gpui_engine`, `gpui_platform` re-exports it, and
  `PlatformWindow::gpu_specs` (`crates/gpui_platform/src/platform_window.rs:228`)
  simply forwards.
- **`recover` takes a `RendererTarget`**, not the raw window, because a lost surface
  is recreated from the handles the renderer was built with and the shared trait must
  not name `metal::*` or `ID3D11Device*`.

The names are the trait's own, not the backends': `update_drawable_size` becomes
`resize`, because a `SceneRenderer` has a target, not a drawable.

### 2. The factory, and the target it is handed

A renderer binds to a surface, and the surface comes from the window, so the factory
receives a target the platform builds:

```rust
// gpui_platform, beside PlatformWindow
pub struct RendererTarget<'a> {
    pub window_handle: Option<RawWindowHandle>,
    pub display_handle: Option<RawDisplayHandle>,
    pub bounds: Bounds<Pixels>,
    pub scale_factor: f32,
    pub transparent: bool,
    /// For backends whose surface is a view rather than a handle: the NSView on
    /// macOS, the canvas on wasm.
    pub native_view: Option<&'a mut dyn Any>,
}

pub type RendererFactory =
    Rc<dyn for<'a> Fn(RendererTarget<'a>) -> anyhow::Result<Box<dyn PlatformRenderer>>>;
```

`gpui_platform` gains a `raw-window-handle` dependency to name the handles; the crates
that already take `HasWindowHandle` (`gpui_wgpu::WgpuRenderer::new`,
`crates/gpui_wgpu/src/wgpu_renderer.rs:268`) speak the same vocabulary. `native_view`
is the escape hatch for the genuinely-native case without teaching the shared trait
what a view is.

### 3. Native hooks stay on the platform

The macOS layer methods and the Windows background-appearance method do not
generalise, and the layer-stack ruling is explicit about this shape
([`../architecture/layer-stack.md`](../architecture/layer-stack.md), "A leaky
abstraction gets an escape hatch, not a new method"). So they do not join
`SceneRenderer`. Instead each platform names the trait its window holds:

```rust
// gpui_platform, cfg-selected — one name, three definitions
pub type PlatformRenderer = dyn SceneRenderer;              // Linux, web

#[cfg(target_os = "macos")]
pub trait MacSceneRenderer: SceneRenderer {
    fn layer_ptr(&self) -> *mut std::ffi::c_void;
    fn set_presents_with_transaction(&mut self, value: bool);
}
#[cfg(target_os = "macos")]
pub type PlatformRenderer = dyn MacSceneRenderer;

#[cfg(target_os = "windows")]
pub trait WinSceneRenderer: SceneRenderer {
    fn set_background_appearance(&mut self, appearance: WindowBackgroundAppearance);
}
#[cfg(target_os = "windows")]
pub type PlatformRenderer = dyn WinSceneRenderer;
```

`Box<PlatformRenderer>` is then what a backend window stores, what the factory
returns, and what `with_renderer`/`present` hand out — the same spelling everywhere,
differing only in what the alias demands. A `WgpuRenderer` on macOS would implement
`MacSceneRenderer` by returning the `CAMetalLayer` it renders through, which is what a
wgpu surface on macOS is.

### 4. Where the factory lives

On `App`, with the other two factories (`crates/gpui_authoring/src/app.rs:629`,
`:633`), set by `Application::with_renderer` in `crates/gpui_runtime/src/application.rs`
beside `with_layout_engine` (`:92`) and `with_frame_pipeline` (`:107`). The default is
`None`, meaning *the platform's own renderer* — so omitting the call is not a special
case but the existing path.

### 5. Threading it to the window

The renderer, unlike the layout engine and the pipeline, cannot be built on the
`gpui_authoring` side: it needs the window's surface, which only the platform window
has. The layout engine and pipeline are built in `Window::new`
(`crates/gpui_authoring/src/window.rs:2321`, `:2253`), *after* `Platform::open_window`
returns; a renderer cannot be. So the factory must reach `Platform::open_window`
(`crates/gpui_platform/src/platform.rs:95`), the one place a window and its raw handles
come into being. `gpui_authoring` already builds the `WindowParams` for that call
(`crates/gpui_authoring/src/window.rs:1756`), so the factory rides there, and each
backend does:

```rust
let renderer = match params.renderer_factory.as_ref() {
    Some(factory) => factory(target)?,
    None => Box::new(WgpuRenderer::new(gpu_context, &raw_window, config, hint)?),
};
```

`WindowParams` derives `Debug` (`crates/gpui_platform/src/window.rs:429`), so the
factory field needs a `Debug`-implementing wrapper — a small cost, stated so it is not
a surprise.

Two alternatives were weighed and are recorded rather than chosen: a new argument on
`Platform::open_window` (explicit, but changes a shared SPI signature and every
backend), and a `PlatformWindow::install_renderer` called right after creation
(touches no SPI, but every backend must then defer building its default renderer until
first use, or build one and throw it away).

### 6. Finish the extraction where it is unfinished

Only one renderer is still inside its platform crate: `DirectXRenderer`
(`crates/gpui_windows/src/directx_renderer.rs:2086`). Once a window holds
`Box<PlatformRenderer>`, the file can move to a `gpui_directx` crate the way Metal
moved to `gpui_apple` and wgpu already has its own — `gpui_windows` would re-export
the few names it keeps, the same shim `gpui_macos` already uses
(`crates/gpui_macos/src/gpui_macos.rs:18`). This is symmetry, not a prerequisite:
injection works with the file where it is.

### 7. The default path is unchanged

The correctness gate for the whole change is that an application which never calls
`with_renderer` builds the renderer it builds today, by the same construction call,
with the same resize, transparency and device-loss behaviour. Concretely: with `None`,
each backend runs the same constructor expression it runs now, and the only difference
is that the result is held as `Box<PlatformRenderer>` instead of the concrete type.

## Migration, in order

1. Move `GpuSpecs` from `gpui_platform` to `gpui_engine`; re-export it from
   `gpui_platform`.
2. Widen `SceneRenderer` with the defaulted methods of §1; port the six
   implementations' inherent methods onto them. No behaviour change.
3. Add `PlatformRenderer`, `RendererTarget` and `RendererFactory` to
   `gpui_platform`; change each backend window's renderer field to
   `Box<PlatformRenderer>`; make the macOS and Windows methods of §3 the traits they
   are reached through.
4. Add `App::renderer_factory`, `Application::with_renderer`, and the `WindowParams`
   field; each backend consults it before its default.
5. Move `DirectXRenderer` to `gpui_directx` (needs a Windows host), for symmetry.
6. Add a test that installs a custom renderer through `with_renderer`, asserts its
   `draw` was called, and asserts that omitting the call still draws.

## What can be verified here, and what cannot

Steps 1–4 and 6 are platform-agnostic in shape and land on the Linux and headless
paths, which compile and run on this host; the headless factory
(`crates/gpui_authoring/src/platform/test/platform.rs:54`) is the working proof the
pattern holds. The macOS and Windows halves of step 3, and step 5, need a macOS or
Windows host. The tools repository records this as a standing gap — every Linux gate
can be green while a platform backend does not compile — and names the cross-target
gate (`cargo check --target aarch64-apple-darwin`, `--target wasm32-unknown-unknown`)
that would close it (`.tools/docs/journal/maintenance-report.md` §21).

## What this does not change

- **`Platform`'s own seam is untouched.** The factory travels through `WindowParams`
  rather than adding a method to `Platform`.
- **Presentation timing stays below the pipeline.** `SceneRenderer::draw` still returns
  whether it presented, and `PlatformWindow::present`
  (`crates/gpui_platform/src/platform_window.rs:157`) still feeds that back to the
  backend's frame loop. Nothing here gives a `FramePipeline` a vblank.
- **No custom GPU primitives yet.** `CustomRenderPrimitive`, zero-copy texture import
  and inline command injection remain
  [`dual-path-render-extension.md`](dual-path-render-extension.md)'s subject; this
  document only makes the renderer that would implement them installable.
- **No decision about the default renderer.** Whether `gpui_wgpu` should become the
  default on macOS and Windows, retiring Metal and DirectX to optional, is a separate
  question this seam merely makes askable.

## What would reopen it

- **Widening `SceneRenderer` freezes it.** It has no out-of-tree implementation today,
  so the additions are free now; once a renderer crate outside the tree implements it,
  every future method is a break, the same commitment `spi/README.md` describes for a
  swap. If a renderer ships before the trait settles, the methods still guessing
  should stay off it and be reached by downcast, per the layer-stack ruling.
- **The macOS layer ownership is the hard part.** If a non-Metal renderer on macOS
  cannot be handed the window's layer — if the window must own the layer and the
  renderer only borrow it — then `MacSceneRenderer` is the wrong shape, and the layer
  has to become a platform-side resource the renderer is given, which moves work into
  §5.
- **If the target is not enough to build a surface.** `RendererTarget` carries the raw
  handles and an optional native view; if a backend needs more of the window at
  construction, the factory signature is the thing to revisit, not the trait.
