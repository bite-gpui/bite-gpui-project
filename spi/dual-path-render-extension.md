# RFC: Dual-Path GPU Render Extension SPI

- **Target crates:** `gpui_engine`, `gpui_authoring`, `gpui_platform` (`gpui_macos`,
  `gpui_windows`, `gpui_wgpu`), ecosystem extension crate (`bite_gpui_render_ext`)
- **Status:** proposed. Not implemented, not accepted.
- **Author:** community extension architecture proposal, transcribed as received.

## Where this stands against this fork

Checked 2026-09-26 against the source in a `bite_*` branch. Three things a reader
should know before treating the sketches below as buildable here.

**None of it exists.** `CustomRenderPrimitive`, `ForeignTextureHandle`, `DrawContext`,
`register_foreign_texture` and `paint_with_callback` have no occurrences anywhere in
the tree. The RFC is a proposal in full. The only surfaces it names that are real are
`Scene` and `SceneRenderer`.

**The seam it needs has no bootstrap.** `SceneRenderer`
(`crates/gpui_engine/src/renderer.rs:16`) is one of the five SPIs, and it is the one
with no `Application::with_renderer` — see [`README.md`](README.md). Chapter 8's
"minimal core patches" are the reason: installing a custom renderer is not reachable
through the facade today, so this SPI could not be *installed* even once implemented.
That is the first thing to fix, and it is an upstream-facing change rather than an
out-of-tree one — specified in [`scene-renderer-seam.md`](scene-renderer-seam.md),
which this RFC depends on.

**The authoring sketches use upstream's names.** Chapter 5's element code takes
`cx: &mut WindowContext`. This fork has no `WindowContext`: an element's methods take
`window: &mut Window<'_>` and `cx: &mut App`, and `Context<T>` is the entity context.
The rendering API in Chapters 4 and 6 is unaffected, because `Scene` and
`SceneRenderer` are shared vocabulary — but the examples in Chapter 5 will not compile
here as written.

One claim to check before relying on it: Chapter 7 casts `gpui_animotion` as the
headless video-recording path ("Async FFmpeg Pipe"). The `gpui_animotion` in this tree
is a third-party declarative *property animation* engine — 0.7.0, from
`github.com/astrimid/gpui_animotion`, vendored at
`.uses/scroll-demo/patches/gpui_animotion` — not a video encoder. If a video-export
crate is intended, it is a different crate, and this chapter names one that does
something else.

---

## Chapter 1 — Executive summary

This RFC introduces a **Dual-Path GPU Render Extension SPI** to the GPUI ecosystem. It
provides a cohesive, unified mechanism for embedding high-performance, external GPU
rendering pipelines into standard GPUI element trees without compromising the core
engine's zero-abstraction native performance.

The SPI solves two complementary rendering demands under one cohesive primitive:

1. **Path A — Zero-Copy Texture Import (VRAM Bridge):** bridges an external rendering
   context (such as an offscreen `wgpu::Texture`, video decoder surface, or camera
   feed) directly into GPUI's fragment sampler as a native driver pointer
   (`id<MTLTexture>` on macOS, `ID3D11ShaderResourceView` on Windows).
2. **Path B — Inline Command Injection (Single-Pass Execution):** allows custom GPU
   shaders (such as real-time vector map tessellation or 3D viewports) to execute draw
   commands directly onto the active window swapchain command encoder.

Both execution paths operate with **zero CPU-to-GPU memory copies**, preserve
**bit-for-bit parity with headless video recording**, and allow **full 2D compositing
and layering** under standard UI widgets.

## Chapter 2 — Motivation and problem space

Integrating high-throughput, custom GPU logic into retained 2D UI toolkits typically
faces three distinct failure modes:

- **The system RAM bottleneck (`RenderImage`):** upstream GPUI's standard offscreen
  image API relies on CPU staging buffers. Moving raw RGBA pixels from GPU VRAM → CPU
  system memory (`Vec<u8>`) → GPU texture atlas consumes ~1.0 GB/s at 1080p @ 60 FPS
  and up to ~16.0 GB/s at 4K @ 120 FPS. This saturates the memory bus, invalidates CPU
  caches, and introduces 1–2 frames of input latency.
- **Primitive translation limits:** converting hundreds of thousands of vector map
  vertices or 3D geometry into standard GPUI box/path primitives causes massive CPU
  tessellation overhead and removes the ability to write custom GPU shaders.
- **Upstream maintenance and fork divergence:** hard-forking GPUI to replace platform
  backends with `gpui_wgpu` creates ongoing divergence from upstream Zed. When
  upstream updates its engine, heavy forks break compatibility with ecosystem libraries.

## Chapter 3 — Unified extension architecture

Both paths hang off a single primitive in the scene graph:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        User Land Application                           │
│   • gpui-kit Component (UI)        • gpui_animotion (Timeline Stepper) │
│   • wgpu Map Viewport (Context)    • Custom Shader Nodes               │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                 gpui_authoring Layer / WindowContext                   │
│   • cx.register_foreign_texture(handle)   ──► Path A (VRAM Composite)  │
│   • cx.paint_with_callback(callback)      ──► Path B (Inline Injection)│
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Compiles into
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                              gpui::Scene                               │
│   [Primitive::Quad] ──► [CustomRenderPrimitive] ──► [Primitive::Text]  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │ Evaluated during draw()
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│              gpui_platform (Metal / DirectX / WGPU Backends)           │
│   • Path A: Samples VRAM pointer directly in fragment pass             │
│   • Path B: Pauses batcher, yields active encoder, restores state      │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        Unified Final Framebuffer                       │
│   • Onscreen Presentation (DisplayLink / OS Swapchain)                 │
│   • Headless Video Recording (gpui_animotion -> Async FFmpeg Pipe)     │
└────────────────────────────────────────────────────────────────────────┘
```

## Chapter 4 — Core data types and the engine SPI

### The unified scene primitive (`gpui_engine`)

```rust
// crates/gpui_engine/src/custom_render.rs
use crate::*;

pub type RenderCallback = Box<dyn Fn(&mut DrawContext) + Send + Sync>;

pub enum CustomRenderPrimitive {
    /// Path A: zero-copy texture sampling via a direct VRAM pointer/view
    Texture {
        id: TextureId,
        handle: ForeignTextureHandle,
        bounds: Bounds<Pixels>,
        radii: CornerRadii,
        opacity: f32,
        flip_v: bool,
    },
    /// Path B: direct GPU command execution within the active render pass
    InlineCommand {
        bounds: Bounds<Pixels>,
        callback: RenderCallback,
    },
}

#[derive(Clone, Debug)]
pub enum ForeignTextureHandle {
    #[cfg(target_os = "macos")]
    Metal(*const std::ffi::c_void), // id<MTLTexture>

    #[cfg(target_os = "windows")]
    DirectX(*const std::ffi::c_void), // ID3D11ShaderResourceView* or D3D12 GPU descriptor

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    Wgpu(wgpu::TextureView),
}

pub struct DrawContext<'a> {
    pub bounds: Bounds<Pixels>,

    #[cfg(target_os = "macos")]
    pub metal_encoder: *mut std::ffi::c_void, // id<MTLRenderCommandEncoder>

    #[cfg(target_os = "windows")]
    pub d3d11_context: *mut std::ffi::c_void, // ID3D11DeviceContext*

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    pub wgpu_pass: &'a mut wgpu::RenderPass<'static>,
}
```

## Chapter 5 — Authoring layer and developer experience

### An element with Path A (zero-copy texture)

```rust
use gpui::*;

pub struct TextureMapElement {
    map_state: Entity<WgpuMapState>,
}

impl Element for TextureMapElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(&mut self, _: Option<&GlobalElementId>, cx: &mut WindowContext) -> (LayoutId, Self::RequestLayoutState) {
        (cx.request_layout(Style::default(), []), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Bounds<Pixels>, _: &mut Self::RequestLayoutState, _: &mut WindowContext) {}

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        cx: &mut WindowContext,
    ) {
        let map = self.map_state.read(cx);
        // Render offscreen to a wgpu texture in VRAM
        let wgpu_view = map.render_offscreen(bounds.size);

        // Wrap the HAL pointer without CPU readback
        let handle = ForeignTextureHandle::from_wgpu(&wgpu_view);
        let texture_id = cx.register_foreign_texture(handle);

        cx.paint_image(bounds, CornerRadii::default(), texture_id, 1.0);
    }
}
```

### An element with Path B (inline injection)

```rust
use gpui::*;

pub struct InlineMapElement {
    map_state: Entity<WgpuMapState>,
}

impl Element for InlineMapElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn request_layout(&mut self, _: Option<&GlobalElementId>, cx: &mut WindowContext) -> (LayoutId, Self::RequestLayoutState) {
        (cx.request_layout(Style::default(), []), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Bounds<Pixels>, _: &mut Self::RequestLayoutState, _: &mut WindowContext) {}

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        cx: &mut WindowContext,
    ) {
        let map = self.map_state.read(cx);

        // Inject commands directly into the active swapchain encoder
        cx.paint_with_callback(bounds, move |draw_ctx| {
            map.draw_inline(draw_ctx);
        });
    }
}
```

### Composition with overlays

Both primitives participate in the box tree, so floating controls, menus and modals
stack on top:

```rust
fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement {
    div()
        .size_full()
        .relative()
        // Embedded map viewport (Path A or Path B)
        .child(InlineMapElement::new(self.map_state.clone()))
        // Standard controls floating on top
        .child(
            div()
                .absolute()
                .top_4()
                .left_4()
                .child(Button::new("zoom_in", "+").on_click(cx.listener(|this, _, cx| {
                    this.map_state.update(cx, |map, _| map.zoom_in());
                }))),
        )
}
```

## Chapter 6 — Native platform renderer integration

### macOS (`gpui_macos::MetalRenderer`)

- **Path A:** the renderer casts `ForeignTextureHandle::Metal(*const c_void)` back to
  `id<MTLTexture>` and binds it to the fragment shader texture slot
  (`[encoder setFragmentTexture:metalTexture atIndex:FOREIGN_SLOT]`) during the quad
  pass.
- **Path B:**
  1. `[self pauseCurrentBatch]` flushes pending UI quads.
  2. Set the hardware scissor rectangle to `primitive.bounds`.
  3. Execute the user callback with the active `id<MTLRenderCommandEncoder>`.
  4. `[self restoreRenderPipelineState]` rebinds GPUI's SDF pipeline, depth state and
     sampler arguments before continuing.

### Windows (`gpui_windows::DirectXRenderer`)

- **Path A:** binds `ID3D11ShaderResourceView*` directly via `PSSetShaderResources`.
- **Path B:** flushes vertex buffers, invokes the callback with
  `ID3D11DeviceContext*`, and resets constant buffers and blend states.

## Chapter 7 — Headless export integration

Because both paths operate at the `gpui::Scene` primitive level, offscreen rendering
inherits them without modification:

1. **Virtual clock stepping:** the timeline advances time Δt deterministically.
2. **Offscreen evaluation:** the window renders the frame to an offscreen render
   target instead of the window swapchain.
3. **Async streaming:** the final framebuffer is copied to a staging buffer and sent
   through a non-blocking `SyncSender` ring buffer to an FFmpeg child worker thread
   (`h264_videotoolbox` / `h264_nvenc`) to write the video file.

## Chapter 8 — Upstream decoupling and distribution strategy

To let development proceed without waiting on upstream Zed review:

- **Minimal core patches:** a small patch series on top of upstream GPUI to expose
  `CustomRenderPrimitive` hooks on `Scene` and `SceneRenderer`.
- **Standalone crates.io releases:** concrete helper bindings, HAL pointer extractors
  (`wgpu::Texture::as_hal()`) and view adapters published as independent modular
  crates (`bite_gpui_render_ext`).
- **Zero overhead when unused:** if an application registers no custom primitives,
  GPUI's standard SDF quad batching pipeline stays untouched.
