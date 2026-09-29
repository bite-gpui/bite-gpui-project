# The producer's reach

- **Status:** proposed. The gap it names is in the tree rather than in these documents: Path A's
  *consumer* half is built on `bite_v1.22.0-pre-path-a` and `bite_v1.22.0-pre-path-a-directx`,
  and nothing on any platform can hand a renderer a texture yet. Citations resolve against the
  canonical ref, so they point at what is there; a branch name says where the rest lives.
- **Assumes:** [`foreign-texture.md`](foreign-texture.md) — the primitive, the token, and what a
  renderer has to draw — and [`renderer-seam.md`](renderer-seam.md) — the factory, the target,
  and the rule that post-construction queries belong on the renderer.
- **Why it is a chapter:** §7 of `foreign-texture.md` asks what each *renderer* has to do, and
  every row of it can be answered — as of this branch all three are — while nothing can make the
  texture the renderer then samples. The producer's side is a second obligation, and it is the one
  that decides whether Path A is a capability or a demonstration.
- **Target crates:** `gpui_platform` (where a rendezvous would go), `gpui_wgpu`, `gpui_windows`,
  `gpui_apple`.

## 1. The two halves

A foreign texture arrives in two steps, and the device rule in
[0002](../../decisions/0002-render-extension-device-model.md) makes them sequential:

1. the producer makes a texture **on the renderer's device** and wraps it in an
   `ImportedTextureHandle`;
2. the renderer resolves the token and samples it.

Step 2 is what the chapters specify, and it is built. Step 1 needs the device *before* anything
can be made on it, which is a question about who holds the device and how it is handed over — and
that question is not answered by any chapter.

| step | where | state |
| --- | --- | --- |
| the primitive, the token, the encoder both shaders read | `crates/gpui_engine/src/custom_render.rs` | built, `bite_v1.22.0-pre-path-a` |
| the window call | `Window::paint_imported_texture`, `crates/gpui_authoring/src/window.rs` | built, same branch |
| the arm, per renderer | wgpu, Direct3D, Metal | built; wgpu and Direct3D have rows on CI, Metal has none |
| **the producer's reach** | **nothing** | **not built on any platform** |

## 2. One rendezvous, two directions

"It has to be the renderer's device" does not say who creates it. Two configurations satisfy the
rule, and they serve different producers:

| direction | what happens | what exists |
| --- | --- | --- |
| **gpui → app** | gpui creates the device; the application asks for it and renders a texture on it | nothing, on any platform, in a build an application gets |
| **app → gpui** | the application creates the device and installs a renderer that adopts it | wgpu only, and only through a factory |

The second is why `GpuContext` is a shared `Rc` rather than a field:
`GpuContext = Rc<RefCell<Option<WgpuContext>>>` (`crates/gpui_wgpu/src/wgpu_renderer.rs:168`),
whose `WgpuContext` exposes `pub device: Arc<wgpu::Device>` and `pub queue: Arc<wgpu::Queue>`
(`crates/gpui_wgpu/src/wgpu_context.rs:9`). An application that owns the slot and returns
`WgpuRenderer::new(context, …)` from its own factory ends up holding the device the renderer
adopts — so on wgpu there *is* an export, it is just one the application performs on itself. It
requires installing a factory, which is the boundary
[0002](../../decisions/0002-render-extension-device-model.md) states as "Path A is the window
owner's capability".

What is missing is the other direction: a window whose renderer gpui built cannot be handed a
texture by anything.

## 3. What each platform can do

| renderer | token type an application can name | device it can be given | device it hands out | tested |
| --- | --- | --- | --- | --- |
| `WgpuRenderer` | yes — `gpui_wgpu` re-exports the extractor | yes, through a factory it installs | no | 3 rows, ubuntu CI |
| `MetalRenderer` | the type is public (`gpui_engine::MetalTexture`) | no — `MetalRenderer::new` calls `create_device()` itself (`crates/gpui_apple/src/metal_renderer.rs:195`) | no | **none** |
| `DirectXRenderer` | no — `pub(crate)` | no | no | 3 rows, windows CI |

Three facts behind that table:

- **The accessors that exist are test-gated.** `WgpuRenderer::device` and `queue` carry
  `#[cfg(any(test, feature = "test-support", feature = "bench-support"))]`, and so do
  `DirectXTextureExt` and `DirectXRenderer::device` on the Direct3D branch. They are not in the
  build an application gets, so they are not a reach even though they are `pub`.
- **The Direct3D types are crate-private besides.** `crates/gpui_windows/src/directx_renderer.rs`
  and `directx_devices.rs` expose nothing, so an application cannot name the renderer to build one
  either.
- **Metal is closed in both directions.** Its renderer picks its own device at construction, so
  not even the app→gpui configuration is available — which is
  [`foreign-texture.md`](foreign-texture.md) §8's open question, and the only place the gap had
  been noticed.

The factory's input does not close it either. `RendererTarget`'s one erased field
(`crates/gpui_platform/src/platform_renderer.rs:129`) is an **input**, and what each platform puts
in it is a *surface configuration* or the platform's device bundle for its own renderer: X11 sends
`&WgpuSurfaceConfig` (`crates/gpui_linux/src/linux/x11/window.rs:783`), Windows sends
`&DirectXDevices` (`crates/gpui_windows/src/events.rs:1322`). There is no path from a window back
to a device its renderer holds.

## 4. Why the tests do not show it

Every Path A test so far lives in the renderer's own crate and holds the concrete renderer:
`WgpuRenderer::new_offscreen` plus `WgpuRenderer::device`, or a `DirectXRenderer` built on a hidden
window plus `DirectXRenderer::device`. The test therefore plays the producer itself, which is what
makes the rows real — both halves really do run — and is also why they say nothing about the step
an application would take, because that step does not exist to exercise. A resumer should read
"three Path A tests pass on `windows-latest`" as "the arm works", not as "Path A is usable".

## 5. Two shapes for the rendezvous

**(a) An erased accessor on the renderer's trait, reached through the window.** The seam's own rule
already says where a post-construction query belongs — "on the renderer, not on the factory's
input" ([`renderer-seam.md`](renderer-seam.md) §5.6) — and the erasure is the same idiom as the
target's:

```rust
// crates/gpui_platform/src/platform_renderer.rs, beside `RendererTarget::backend`
fn device(&self) -> Option<&dyn Any>;
```

`WgpuRenderer` returns its `WgpuContext`, `DirectXRenderer` its `DirectXDevices`, `MetalRenderer`
the `MTLDevice` §8 is waiting for. The obstacle is the reach, not the accessor:
`PlatformWindow::with_renderer` hands out `&mut dyn SceneRenderer`
(`crates/gpui_platform/src/platform_window.rs:151`), which has no such method, and the seam refuses
to widen `SceneRenderer` for a platform-shaped concern. So this needs either a second accessor on
`PlatformWindow`, or a generic escape on `SceneRenderer` — and the rejected alternative is worth
naming: a downcast from the window to a backend's own type is what the target's typing already
refused, for the reason that it makes `gpui_platform` depend on a backend crate.

**(b) Generalise the slot.** Give each platform's device bundle the shape `GpuContext` already has
— an `Rc<RefCell<Option<…>>>` the application also holds — and no trait changes at all. On Windows
that means the platform's `DirectXDevices` becoming reachable (and, if the app-supplied direction is
wanted, acceptable as an input). Cheap per platform, and it is the mechanism the wgpu path already
uses; but it is one slot type per platform and it still requires the application to install a
renderer.

The two are complementary rather than alternatives: (a) answers "I have a texture, lend me your
device", which is a 3D engine or a map renderer; (b) answers "here is my device, decode on it",
which is Media Foundation, because `IMFDXGIDeviceManager::ResetDevice` wants to be handed a device
rather than to borrow one.

## 6. The tier this leaves on Windows

This is the part worth writing down before anyone proposes a bridge:

- **wgpu has no Direct3D 11 backend.** `wgpu-hal-29.0.4`'s backends are `dx12`, `vulkan`, `metal`,
  `gles` and `noop`. A wgpu producer can therefore never be on a `DirectXRenderer`'s device, so
  under that renderer a wgpu producer is 0002's Tier 2 *by construction* rather than by omission.
- **Tier 1 on Windows is not gone, it is a choice of renderer.** A Direct3D 11 producer on
  `DirectXRenderer` — a Media Foundation or DXVA decoder, D3D11 compute, a D3D11 engine — needs no
  bridge at all, which is the configuration
  `bite_v1.22.0-pre-path-a-directx` makes possible. A wgpu producer gets Tier 1 on a window that
  installed `WgpuRenderer`. The two are mutually exclusive *per window*, because the tier follows
  the renderer: `DirectXRenderer` buys per-pixel transparency and D3D11 Tier 1, `WgpuRenderer` buys
  wgpu Tier 1 and `Opaque` alone
  ([`../decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md)).
- **The direction a bridge would need is the one not measured.** Both existing measurements created
  the shared resource on the **D3D12** side and read it from D3D11
  ([`../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) §4 probe 2,
  [`../decisions/shared-surface.md`](../../decisions/shared-surface.md) §2 probe 5). A D3D11
  producer feeding a wgpu consumer is the reverse, and the one first-party entry point in that
  direction is Vulkan's — `texture_from_d3d11_shared_handle`
  (`wgpu-hal-29.0.4/src/vulkan/device.rs:544`) — which
  [`../decisions/shared-surface.md`](../../decisions/shared-surface.md) §2 probe 6 had no adapter to
  reach. So a second probe comes before that bridge, not after it.

## 7. Open

- **Which direction the rendezvous takes** — lent, adopted, or both — and whether it is a trait
  accessor (§5a) or a slot (§5b). This is the decision the next milestone needs first.
- **Whether the backend payload types become public API.** They have to be nameable for anyone
  outside a backend crate to construct a token or read a device, which makes them a published
  surface and therefore a commitment, not an implementation detail.
- **What Metal's producer half is**, given that its renderer creates its own device: either a
  constructor that accepts one, or an accessor, and §8 of
  [`foreign-texture.md`](foreign-texture.md) records the question.
- **Whether the token should carry its own synchronisation.** It does not need one while producer
  and consumer share a device and a queue ([`foreign-texture.md`](foreign-texture.md) §6); a bridge
  would add it to the contract, which is a reason to decide the direction before the token's shape
  is frozen.
