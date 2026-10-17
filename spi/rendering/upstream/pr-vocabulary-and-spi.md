# The vocabulary-and-SPI body, ready to open

- **Target:** `zed-industries/zed`, `main`. **Opened:** not yet. **Depends on:** none — it is the first
  PR, and every arm builds on it.
- **The plan:** [`README.md`](README.md) §2, PR 1. **Welcome upstream:** the Discussion
  [`#64849`](https://github.com/zed-industries/zed/discussions/64849) welcomes `paint_surface` support
  and prefers surface sharing; this PR is the seam and the vocabulary that welcome rests on, proposing
  no platform arm of its own.
- **Built in the fork:** the commit group in [`README.md`](README.md) §7 — `cff2fcddfa` + `9cb292ec77`
  (the seam and its test), `81e8b9376d` (the token — **split**, see "The one commit that splits"),
  `409ae7ddb5` (the vocabulary), `fdfdc9d2e0` (the device door), `2acda37306` + `830386d69b`
  (`GpuCanvas`), `342b9c1be0` (the facade tokens). By path: `crates/gpui_platform/src/platform_renderer.rs`
  (the trait, the target and the factory), `crates/gpui_engine/src/scene.rs`, `surface_format.rs` and
  `dmabuf.rs` (the vocabulary), `crates/gpui_engine/src/renderer.rs` (the device door), and
  `crates/gpui_authoring/src/elements/gpu_canvas.rs` (the element and its context).

## Title

`gpui: a renderer seam, a foreign-surface vocabulary, and the GpuCanvas element`

## Body

### Motivation

An application that already has pixels on the GPU — a video decoder, a webview, another engine — has
no supported way to get them into a GPUI scene, and no way to install the renderer that would draw
them. A window builds the renderer it builds, and the only escape is the per-platform `*WindowExt`
hooks, which are ad-hoc and per-platform. This PR adds the seam a window installs a renderer through,
and the vocabulary an application hands external pixels in with, so the platform arms that follow are
a backend each rather than a new surface each.

### What this does

- **The renderer seam.** `PlatformRenderer` (`crates/gpui_platform/src/platform_renderer.rs:73`) is the
  trait a window holds; `RendererTarget` (`:116`) carries the typed handles a renderer is built
  against, erasing only the backend's own extra; and `RendererFactory` (`:138`) installs one through
  `WindowOptions::renderer_factory` (`crates/gpui_platform/src/window.rs:433`,
  `with_renderer_factory` at `:568`). A window that installs nothing is unchanged — the field defaults
  to `None` and the backends build the renderer they build today.
- **The surface vocabulary.** `SurfaceSource` (`crates/gpui_engine/src/scene.rs:851`) and the arms it
  carries — `DirectXSource` (`:802`; a texture the renderer makes a view of, a view the producer made,
  or a texture shared across devices) and `DmaBufHandle` (`crates/gpui_engine/src/dmabuf.rs:154`) —
  `SurfaceFormat` (`crates/gpui_engine/src/surface_format.rs:114`), and the `PaintSurface` primitive
  (`crates/gpui_engine/src/scene.rs:908`) a surface becomes.
- **The device door.** `GpuRenderer` (`crates/gpui_engine/src/renderer.rs:36`) is the trait a renderer
  that owns a device implements, and `SceneRenderer::as_renderer` (`:16`) finds it — so a producer
  reaches the device the window draws on without the window lending its guest's internals.
- **`GpuCanvas`** (`crates/gpui_authoring/src/elements/gpu_canvas.rs:135`), built by `gpu_canvas()`
  (`:24`): a `Div` in every respect whose callback receives a `GpuCanvasContext` (`:37`) —
  `paint_surface` (`:46`) composites a source produced outside GPUI, `paint_texture` (`:64`) a texture
  made on the window renderer's own device, and `device::<R>()` (`:87`) / `try_device::<R>()` (`:102`)
  hand the device itself to a producer that must make one.
- **The per-platform tokens**, re-exported where an application meets its platform:
  `crates/gpui/src/platform_entry.rs:39` (the Windows pair), `:46` (macOS) and `:53` (Linux/wgpu).
- **The per-platform `*WindowExt` hooks go**, replaced by the context.

**Must not carry:** no platform renderer arm (PR 2–4), no `wgpu`, no Direct3D 12, no YCbCr — and not
the `CustomRenderPrimitive` second-primitive route (next).

### The one commit that splits

`81e8b9376d` bundles the token with the fork's **superseded** `CustomRenderPrimitive` route
(`crates/gpui_engine/src/custom_render.rs:41`; `ImportedTextureHandle` at `:14`, `to_quad_record` at
`:101`). That route is not an ask: `0005` unifies external pixels under the existing `PaintSurface` /
`surface()` rather than a second primitive. Only the parts this PR's vocabulary needs travel — the
quad-record encoding the surface fragment reads (`to_quad_record`) and the `ObservingRenderer` test
support
(`crates/gpui_engine/src/renderer.rs:178`) — and the `CustomRenderPrimitive` framing stays behind. This
is why step 2's cherry-pick splits the commit rather than replaying it whole.

### Testing

The seam is asserted without a GPU: a test-platform window
(`crates/gpui_authoring/src/platform/test/platform.rs:499`) installs a custom renderer through the
factory and asserts its draw was called, and that a window installing nothing still draws
(`window_draws_through_the_renderer_factory_it_is_given`, `:843`). The rows that need a GPU belong to
PR 2–4, not here.

### Notes

Split out of a larger interop effort. This PR is independent of the platform arms (PR 2–4) and of the
offscreen/headless work (PR 6), and every one of them builds on it.

## The fork's organization is not the ask

The fork splits the code into layer crates, so the paths above are the fork's, not upstream's; the
*capability* — the seam, the vocabulary, the element and the device door — is what this proposes
([`README.md`](README.md) §6).
