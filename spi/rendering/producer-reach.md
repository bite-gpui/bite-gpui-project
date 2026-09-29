# The producer's reach

- **Status:** built. The whole of Path A is on the canonical ref, merged as PR #6, and the accessor a
  producer reaches the device through is built on it; the decision behind that accessor is
  [`0004`](../../decisions/0004-producer-device-rendezvous.md), decided. What each platform's
  producer half still lacks is §7, and it is not the same gap anywhere.
- **Assumes:** [`foreign-texture.md`](foreign-texture.md) — the primitive, the token, and what a
  renderer has to draw — and [`renderer-seam.md`](renderer-seam.md) — the factory, the target, and
  the rule that post-construction queries belong on the renderer.
- **Why it is a chapter:** §7 of `foreign-texture.md` asks what each *renderer* has to do, and every
  row of it could be answered while nothing could make the texture the renderer then samples. The
  producer's side is a second obligation, and it was the one that decided whether Path A is a
  capability or a demonstration — every other step in §1's table was built before it. It stays a
  chapter now that it is built, because the shape was a decision and what is left is per platform.
- **Target crates:** `gpui_platform` (where the accessor lives), `gpui_authoring` (which forwards
  it), `gpui_wgpu`, `gpui_windows`, `gpui_apple`.

## 1. The two halves

A foreign texture arrives in two steps, and the device rule in
[0002](../../decisions/0002-render-extension-device-model.md) makes them sequential:

1. the producer makes a texture **on the renderer's device** and wraps it in an
   `ImportedTextureHandle`;
2. the renderer resolves the token and samples it.

Step 2 is what the chapters specify. Step 1 needs the device *before* anything can be made on it,
which is a question about who holds the device and how it is handed over — and it is not a property
of the primitive, so no chapter about the primitive answers it.

| step | where | state |
| --- | --- | --- |
| the primitive, the token, the encoder both shaders read | `crates/gpui_engine/src/custom_render.rs` | built, the canonical ref |
| the window call | `Window::paint_imported_texture`, `crates/gpui_authoring/src/window.rs` | built, the canonical ref |
| the arm, per renderer | wgpu, Direct3D, Metal | built, all three: Direct3D's rows and Metal's run on CI, wgpu's only where the machine has an adapter |
| the producer's reach | `Window::device_any` and the per-backend token builder | built, all three; what each platform still lacks is §7, and it is not the same gap anywhere |

## 2. One rendezvous, two directions

"It has to be the renderer's device" does not say who creates it. Two configurations satisfy the
rule, and they serve different producers:

| direction | what happens | where it is |
| --- | --- | --- |
| **gpui → app** | gpui creates the device; the application asks for it and renders a texture on it | built — `Window::device_any` hands the renderer's device, the gpui→app half of [0002](../../decisions/0002-render-extension-device-model.md)'s "one slot, and it works both ways" |
| **app → gpui** | the application creates the device and installs a renderer that adopts it | wgpu only, and only through a factory |

`GpuContext` is a shared `Rc` rather than a field for the second reason:
`GpuContext = Rc<RefCell<Option<WgpuContext>>>` (`crates/gpui_wgpu/src/wgpu_renderer.rs:173`),
whose `WgpuContext` exposes `pub device: Arc<wgpu::Device>` and `pub queue: Arc<wgpu::Queue>`
(`crates/gpui_wgpu/src/wgpu_context.rs:9`). An application that owns the slot and returns
`WgpuRenderer::new(context, …)` from its own factory ends up holding the device the renderer
adopts — so on wgpu there *is* an export, it is just one the application performs on itself. It
requires installing a factory, which is the boundary
[0002](../../decisions/0002-render-extension-device-model.md) states as "Path A is the window
owner's capability".

The two directions are one rendezvous rather than two, and that is what the reach turned out to be:
the slot a factory would have been given is the slot a window's `device_any` returns, so a producer
that was handed nothing still reads `device` and `queue` out of the same place.

## 3. What each renderer lends, and what an application can name

| renderer | device it lends | token builder an application can name | tested |
| --- | --- | --- | --- |
| `WgpuRenderer` | the shared `GpuContext` slot — both `device` and `queue` | `gpui::ImportedTextureExt` — `gpui_wgpu`'s trait, re-exported by the platform crate the way the other two are — on a `wgpu::TextureView`, which checks the view samples as sRGB and the texture is a `TEXTURE_BINDING` | 3 rows, run locally only |
| `MetalRenderer` | its `MTLDevice` | `gpui::MetalTextureExt` on a `metal::TextureRef`, which checks the declaration is sRGB and the usage includes `ShaderRead` | 2 rows, `macos-14` |
| `DirectXRenderer` | its `ID3D11Device` | `gpui::DirectXTextureExt` on an `ID3D11Texture2D`, which checks `B8G8R8A8` and `SHADER_RESOURCE` | 3 rows, `windows-latest` |

Two facts behind the table:

- **The device is reached through the window, not the renderer.** `PlatformWindow::with_renderer`
  hands out `&mut dyn SceneRenderer` (`crates/gpui_platform/src/platform_window.rs:150`) and no
  device, and the seam refuses to widen `SceneRenderer` for a platform-shaped concern, so the
  accessor is `device_any` on `PlatformWindow` — which `Window` forwards to — rather than a method
  on the renderer's trait. It answers `None` for a renderer with nothing to lend, which is an answer
  rather than a failure. The anchor is
  [`0004`](../../decisions/0004-producer-device-rendezvous.md)'s decision, and §5 is why.
- **The three token builders agree; the device check does not.** Each checks what its sampler needs
  before a token exists — an sRGB view, `B8G8R8A8` with `SHADER_RESOURCE`, an sRGB declaration with
  `ShaderRead` — so a producer's mistake is named at the boundary rather than composited. What they
  cannot all do is refuse a texture from *another* device: wgpu panics inside its own storage,
  Direct3D names the mismatch (`CreateShaderResourceView` refuses it), and Metal has no API for it,
  because a resource does not expose the device that made it. On macOS the same-device rule rests on
  the application rather than on the renderer.

The factory's input does not close it either. `RendererTarget`'s one erased field
(`crates/gpui_platform/src/platform_renderer.rs:147`) is an **input**, and what each platform puts
in it is a *surface configuration* or the platform's device bundle for its own renderer: X11 sends
`&WgpuSurfaceConfig` (`crates/gpui_linux/src/linux/x11/window.rs:783`), Windows sends
`&DirectXDevices` (`crates/gpui_windows/src/events.rs:1322`). Before the accessor there was no path
from a window back to a device its renderer holds; `device_any` is that path, and it needs no target
field because it is answered after the renderer exists.

## 4. Why the tests still do not show the whole of it

Every Path A row lives in a renderer's own crate and holds the concrete renderer:
`WgpuRenderer::new_offscreen` plus `WgpuRenderer::device`, or a `DirectXRenderer` built on a hidden
window. The Direct3D colour row has moved one step towards an application — it takes its device
through `device_any`, the route an application has, rather than through the renderer's own field —
but the test is still its own producer, because a test in the crate that names `ID3D11Texture2D` can
call `to_imported_handle` on the device it holds. So the rows prove the arm and the reach work; what
they cannot prove is that the two together are *enough* for an application, because a test is not
one. A resumer should read "three Path A tests pass on `windows-latest`" as "the arm and the
accessor work", not as "Path A is usable".

## 5. The shape the reach took

[`0004`](../../decisions/0004-producer-device-rendezvous.md) is decided, and the shape is the third
anchor the drafts did not have: the accessor hangs on the **window**.

- **Erased on the shared traits, typed in the crate that owns the type.**
  `PlatformRenderer::device_any`, `PlatformWindow::device_any` and `Window::device_any` return
  `Option<Rc<dyn Any>>`, because the shared trait must not name `ID3D11Device` or `MTLDevice`; the
  typed form an application writes is a trait in the facade, because that is the lowest crate that
  can name both a `Window` and a backend's types — no backend crate depends on `gpui_authoring`, so
  none of them can implement a trait for `Window`.
- **Owned rather than borrowed.** The only route from a window to a renderer is a closure, and no
  borrow outlives one, so a `&dyn Any` accessor cannot be written at all; `Rc<dyn Any>` costs an
  allocation per call and removes the lifetime.
- **`None` is an answer.** A renderer that draws offscreen, or one a factory installed that is not
  the backend's own, has nothing to lend.

The two shapes this chapter used to weigh are the record's rejected alternatives: an accessor on
`PlatformRenderer` reached through `with_renderer` costs an upcast and a userland downcast, so the
typed part of the API is a convention rather than a signature; and generalising each platform's
device bundle to the `GpuContext` shape needs a slot type per platform and still requires the
application to install a renderer, which is the *other* direction.

## 6. The tier this leaves on Windows

This is the part worth writing down before anyone proposes a bridge:

- **wgpu has no Direct3D 11 backend.** `wgpu-hal-29.0.4`'s backends are `dx12`, `vulkan`, `metal`,
  `gles` and `noop`. A wgpu producer can therefore never be on a `DirectXRenderer`'s device, so
  under that renderer a wgpu producer is 0002's Tier 2 *by construction* rather than by omission.
- **Tier 1 on Windows is not gone, it is a choice of renderer.** A Direct3D 11 producer on
  `DirectXRenderer` — a Media Foundation or DXVA decoder, D3D11 compute, a D3D11 engine — needs no
  bridge at all. A wgpu producer gets Tier 1 on a window that installed `WgpuRenderer`. The two are
  mutually exclusive *per window*, because the tier follows the renderer: `DirectXRenderer` buys
  per-pixel transparency and D3D11 Tier 1, `WgpuRenderer` buys wgpu Tier 1 and `Opaque` alone
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

## 7. What is left, per platform

The reach is built everywhere; the gaps are not the same gap.

- **Linux / `WgpuRenderer` — complete in tree, ungated in CI only.** The three rows pass on a Linux
  host that has an adapter and are excluded from CI, because they need that adapter and
  `gpui_wgpu`'s older tests fail rather than skip without one
  ([`milestones.md`](milestones.md) §3). Nothing else is missing: the accessor hands over the slot,
  `ImportedTextureExt` builds the token from a view, and Linux reaches it as
  `gpui::ImportedTextureExt` — `gpui_linux` re-exports it for the reason `gpui_macos` re-exports
  Metal's.
- **Windows / `DirectXRenderer` — complete for a Direct3D 11 producer**, which is what a Media
  Foundation or DXVA decoder is: `device_any` hands it the device the platform built, so a texture
  made on that device needs no handle and nothing to synchronise. A **wgpu** producer under this
  renderer is not on it and cannot be — wgpu has no Direct3D 11 backend — so that is 0002's Tier 2
  and §6's bridge, a different piece of work rather than a gap in this one.
- **macOS / `MetalRenderer` — complete but for the device check.** `device_any` hands the `MTLDevice`
  the renderer created, `MetalTextureExt` builds the token from a texture on it, and two rows run on
  `macos-14`. What it does not have is Direct3D's enforcement of the same-device rule: a Metal
  resource does not expose the device that made it, so a texture from another device composites
  rather than failing, and that half of the rule rests on the application.
- **Why the builder mattered more on macOS than on the other two.** The drafts' other route was to
  run the window itself on `WgpuRenderer` and use the wgpu token, and that is not available: the
  `platform_renderer` module carrying `WgpuRenderer`'s `PlatformRenderer` impl sits behind
  `#[cfg(not(any(target_family = "wasm", target_os = "macos")))]`
  (`crates/gpui_wgpu/src/gpui_wgpu.rs`), because a macOS window's renderer must answer
  `MacSceneRenderer` and `WgpuRenderer` has no `layer_ptr`. So a macOS window's consumer is always
  `MetalRenderer` and its token is always the raw handle — which is why the builder was the whole of
  the gap rather than one of two ways around it.
- **A wgpu producer on macOS needs no handover — measured, not argued.** A producer that renders with
  wgpu does not need `WgpuRenderer` in the window; it needs its wgpu device to *be* GPUI's `MTLDevice`,
  so the texture it makes is visible to the Metal renderer that samples the raw handle. It is: wgpu's
  adapter on macOS is the `MTLDevice` `MetalRenderer` created, **pointer for pointer**, under every
  power preference, and a `Bgra8UnormSrgb` texture wgpu made — filled both ways a producer would, a
  `write_texture` copy and a render-pass clear — was sampled through `MetalTextureExt` and read back
  byte for byte
  ([`../decisions/macos-wgpu-producer-probe.md`](../../decisions/macos-wgpu-producer-probe.md)). So
  the route needs neither `Device::device_from_raw` (`wgpu-hal-29.0.4/src/metal/device.rs:376`) nor
  `wgpu::Instance::create_adapter_from_hal` (`wgpu-29.0.4/src/api/instance.rs:390`) — a power
  preference is the whole of it, because `AdapterShared::expose` is private and a device can be
  enumerated but not *addressed*. What that leaves unmeasured is a two-GPU Mac, and it is why the
  finding is load-bearing rather than convenient: there the preference is still the only lever.
- **The payload types are a published surface, and the versions come with them.** An application
  that downcasts `device_any`'s answer to `ID3D11Device` or `metal::Device` has to depend on the
  same crate version the backend does, and build a token with a type the backend publishes. All
  three are reachable from the facade as `gpui::ImportedTextureExt`, `gpui::DirectXTextureExt` and
  `gpui::MetalTextureExt` — except on Windows, where a factory-installed `WgpuRenderer` takes its
  token from `gpui_wgpu` directly, because that is the crate the renderer comes from. That is the
  commitment
  [`0004`](../../decisions/0004-producer-device-rendezvous.md) names under "what would reopen this",
  and it is why [`foreign-texture.md`](foreign-texture.md) §8 keeps the payload question open.
