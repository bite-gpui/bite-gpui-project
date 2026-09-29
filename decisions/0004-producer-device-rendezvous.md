# 0004 — How a producer reaches the renderer's device

- **Opened:** 2026-09-29
- **Status:** open
- **Evidence:** [`../spi/rendering/producer-reach.md`](../spi/rendering/producer-reach.md) — the gap,
  the two directions a rendezvous can take, and what each platform can do today.
- **Touches:** [`0002-render-extension-device-model.md`](0002-render-extension-device-model.md) — the
  device rule this implements, and its "the rendezvous is one slot, and it works both ways", which
  the code does not implement; [`../spi/rendering/milestones.md`](../spi/rendering/milestones.md) —
  this is its M1, and the bridge it enables is its M5;
  [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §5.6 (where a
  post-construction query belongs) and §10 (what would reopen the seam).

## The question

Path A's consumer half is built on two branches and its producer half is built nowhere: a renderer
can sample an imported texture, and nothing can hand it one. Step 1 of every producer needs **the
device the renderer draws on** — that is what
[0002](0002-render-extension-device-model.md) requires — so the question is who holds that device,
how an application asks for it, and whether it can be given one instead.

Nothing in this record takes [0002](0002-render-extension-device-model.md)'s deferred bridge. The
bridge is a tier 2 question with its own probe to run (see "The tier underneath"), and deciding this
one is what makes the bridge worth deciding at all.

## What is already settled

**The app→gpui direction is settled in shape, and reachable on one renderer.** An application that
must own the GPU context supplies it by installing a renderer: `RendererFactory::create` takes a
`RendererTarget` (`crates/gpui_platform/src/platform_renderer.rs:138`) and returns the window's
renderer. On wgpu that is a whole export already, because the slot is a shared `Rc`:
`GpuContext = Rc<RefCell<Option<WgpuContext>>>` (`crates/gpui_wgpu/src/wgpu_renderer.rs:168`) with
`pub device: Arc<wgpu::Device>` and `pub queue: Arc<wgpu::Queue>`
(`crates/gpui_wgpu/src/wgpu_context.rs:9`), so an application that holds the slot, builds
`WgpuRenderer` from it, and returns that from its factory ends up holding the device the renderer
adopts. It is not a general answer: on Windows the Direct3D types are `pub(crate)`, so an
application cannot name the renderer the factory would have to return, and `MetalRenderer::new`
calls `create_device()` itself (`crates/gpui_apple/src/metal_renderer.rs:195`), so there is no
device for an application to supply.

**The device rule itself does not move.** Same device, one queue, no fence
([`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §2, §6).

**The token stays erased.** `ImportedTextureHandle` is `Arc<dyn Any + Send + Sync>`, and nothing
here changes that: whatever the accessor returns, the payload an application puts in the token is
its own type in its own crate
([`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §3).

## The fork: which anchor the accessor hangs on

The direction is not really the fork — both directions serve someone, and app→gpui exists for wgpu.
The fork is where the *typed* accessor is anchored, because that decides what it takes to reach.

An anchor must be a type the application already holds: a `Window`
(`crates/gpui_authoring/src/window.rs:1265` owns `Box<dyn PlatformWindow>`), or a renderer reached
through `PlatformWindow::with_renderer` (`crates/gpui_platform/src/platform_window.rs:151`), which
hands out `&mut dyn SceneRenderer` and not a `PlatformRenderer`.

| anchor | shape | what it costs |
| --- | --- | --- |
| **A. the renderer** | an erased accessor on `PlatformRenderer`, beside `max_texture_size`; userland upcasts to `PlatformRenderer`, then downcasts the payload to a native type | the upcast the seam keeps deliberately narrow, and a downcast in the application, so the typed part of the API is a convention rather than a signature |
| **B. the window** | a per-platform extension trait — `DirectXWindowExt::d3d11_device`, and its Metal and wgpu twins — implemented for `Window` in each backend crate, so its method signature may name a native type | the trait is local to the backend crate, so naming `ID3D11Device` is fine and the orphan rule is satisfied; what is missing is a route from `Window` to whatever owns the device, because the field is `pub(crate)` to `gpui_authoring` |
| **C. the platform window** | an erased accessor beside `with_renderer` on `PlatformWindow`, which `Window` forwards to the way it forwards `content_size` and `platform_window.with_renderer` (`crates/gpui_authoring/src/window.rs:1787`); the typed trait of B at the edge | one new method on a shared trait, whose return type is `&dyn Any` — the same erasure the target already uses for the same reason — and the one downcast moves into the backend crate that owns the type |

C is B with the reach problem solved rather than deferred, and A is the only one of the three that
touches `SceneRenderer`'s neighbourhood. All three keep `gpui_engine` free of hardware types, which
is not negotiable: the seam's rule is that the shared trait "must not name `metal::*` or
`ID3D11Device*`" ([`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §5.1),
and `MacSceneRenderer` and `WinSceneRenderer` exist in their present cfg-selected form for exactly
that reason.

## What the answer has to satisfy

- **A device the application did not create must be the one the renderer draws on.** On Windows the
  platform owns it — `WindowsPlatformState.directx_devices` (`crates/gpui_windows/src/platform.rs:84`)
  — and the renderer is built *from* that bundle (`crates/gpui_windows/src/window.rs:165`), which is
  why an accessor that reads the platform's slot answers for the renderer too. On Linux the client
  owns the same shape (`crates/gpui_linux/src/linux/x11/client.rs:192`). On macOS the renderer owns
  it, and nothing else does.
- **It has to be a build an application gets.** `WgpuRenderer::device` and `queue`, and
  `DirectXTextureExt` and `DirectXRenderer::device`, are `#[cfg(any(test, feature =
  "test-support", feature = "bench-support"))]` today. That is how Path A is tested and precisely why
  it is not usable: the tests walk through a door that does not exist in a release build
  ([`../spi/rendering/producer-reach.md`](../spi/rendering/producer-reach.md) §4).
- **The token types have to be nameable too.** An accessor alone does not let an application push
  anything: the type the payload holds is `pub(crate)` on `DirectXRenderer`'s side and absent on
  Metal's. Whatever this decides, the per-backend payload type becomes part of a published surface.
- **Metal needs one of two things, and they are different changes.** Either its renderer keeps
  creating the device and something reaches into it, or the constructor accepts one the platform
  created — the Windows shape. The second makes the answer uniform across platforms; the first is
  smaller and leaves macOS the odd one out.
- **The accessor must be able to say "not this renderer".** A window whose renderer is not the
  backend's own — a `WgpuRenderer` on Windows, or a factory's third-party renderer — has no
  Direct3D device to lend, and the answer has to be `None` rather than a panic.

## The tier underneath

Two facts fix what is possible, and they are worth stating here because a proposal for the bridge
will otherwise re-derive them:

- **Tier 1 is a choice of renderer, not of API.** A Direct3D 11 producer on `DirectXRenderer` needs
  no handle at all — that is the configuration `bite_v1.22.0-pre-path-a-directx` enables, and it is
  what a Media Foundation or DXVA decoder wants. A wgpu producer gets tier 1 on a window that
  installed `WgpuRenderer`. The two are mutually exclusive per window, because the tier follows the
  renderer: `DirectXRenderer` buys per-pixel transparency and Direct3D 11 tier 1, `WgpuRenderer` buys
  wgpu tier 1 and `Opaque` alone
  ([`windows-presentation-probe.md`](windows-presentation-probe.md)).
- **wgpu has no Direct3D 11 backend.** `wgpu-hal-29.0.4`'s backends are `dx12`, `vulkan`, `metal`,
  `gles` and `noop`, so a wgpu producer in a default Windows window is cross-device *by
  construction* — [0002](0002-render-extension-device-model.md)'s tier 2, not an omission. The
  borrowed Direct3D 11 device then serves as the *allocator* for the shared `ID3D11Texture2D` and
  `ID3D11Fence` that wgpu adopts through NT handles, which is
  [`shared-surface.md`](shared-surface.md) probe 5's loop run in the other direction.

That last clause is the part with no measurement behind it: both existing probes created the shared
resource on the **D3D12** side and read it from Direct3D 11
([`windows-path-a-probe.md`](windows-path-a-probe.md) §4 probe 2,
[`shared-surface.md`](shared-surface.md) §2 probe 5), and the one first-party entry point in the
direction a wgpu consumer needs is Vulkan's — `texture_from_d3d11_shared_handle`
(`wgpu-hal-29.0.4/src/vulkan/device.rs:544`) — which probe 6 had no adapter to reach. So the bridge
takes a second probe before it takes a decision, and the fence handshake it needs
(`ID3D12Fence` ↔ `ID3D11Fence`, waited on by GPUI's own immediate context) is a change to the
token's contract rather than to the primitive: today's Direct3D payload holds the texture and the
renderer builds the view, and a fence-carrying payload would have to be waited on inside
`draw_custom`, before the draw that samples it.

## What would decide it

- **Which producer is served first.** Media Foundation wants to own the device
  (`IMFDXGIDeviceManager::ResetDevice` hands it a device rather than borrowing one), so its answer is
  app→gpui and the `DirectXWindowExt`-shaped accessor is for everyone else: a 3D viewport, a map, a
  canvas tool that would rather not own a swapchain's worth of state.
- **Whether the erased accessor's one downcast is acceptable at the edge.** A puts it in userland;
  C puts it in the backend crate and leaves a type-checked signature for the application. B removes
  it and needs the route.
- **Whether uniformity across the three platforms is worth the macOS change.** Accepting a device in
  `MetalRenderer`'s constructor makes every platform own-and-lend the same way; an accessor into a
  renderer that creates its own device is smaller and leaves macOS answering a different question.
- **Whether the payload types shipping publicly is acceptable.** It is the same commitment as any
  published swap, and 0001 is where this project's position on those lives.
