# 0002 — The render extension's device model

- **Decided:** 2026-09-27
- **Revised:** 2026-09-28 — the claim that a wgpu producer cannot wrap a native shareable
  resource was wrong. See "Revision" below.
- **Revised:** 2026-09-29 — item 2's Windows clause was written before the Direct3D arm existed.
  The default `DirectXRenderer` supports Path A too, for a Direct3D 11 producer; the clause holds
  only for a *wgpu* producer. See "Revision, 2026-09-29" below.
- **Status:** decided — the same-device model holds; the shared-handle route is future work,
  corrected and recorded in the revision below
- **Evidence:** [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §2 and §5,
  [`windows-path-a-probe.md`](windows-path-a-probe.md),
  [`shared-surface.md`](shared-surface.md),
  [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §5.5,
  [`../spi/rendering/verification.md`](../spi/rendering/verification.md) §4

## Decision

**Producer and consumer must be the same device, not merely the same API.** Four
consequences follow, and they are the whole of the render extension's relationship with
GPU resources:

1. **Path A and Path B are window-owner capabilities.** The device belongs to whoever
   built the window's renderer, so a widget inside someone else's window cannot be a
   producer. This is a boundary of the design, not an accident of it.
2. **On Windows both paths require `gpui_wgpu::WgpuRenderer`**, installed through the
   renderer factory. The default `DirectXRenderer` supports neither, and it says so rather
   than succeeding silently: `draw_surfaces` returns an explicit unsupported error
   (`crates/gpui_windows/src/directx_renderer.rs:833`). Corrected 2026-09-29 for a Direct3D 11
   producer — see the second revision below.
3. **Sharing a resource across devices is out of scope for this milestone.** No `IOSurface`
   variant, no DXGI shared-handle variant, no dma-buf; the erased texture payload keeps no
   Windows arm. It is deferred, not impossible — the revision below corrects the reason, and
   [`shared-surface.md`](shared-surface.md) measures the bridge a later milestone could take.
4. **The device is reached through the factory**, which already carries it:
   `GpuContext = Rc<RefCell<Option<WgpuContext>>>`
   (`crates/gpui_wgpu/src/wgpu_renderer.rs:168`) with
   `pub device: Arc<wgpu::Device>` and `pub queue: Arc<wgpu::Queue>`
   (`crates/gpui_wgpu/src/wgpu_context.rs:9`).

## What the constraint implies for a producer

The constraint reads as a restriction; in practice it is a decision tree, and where a
producer lands decides which mechanism it uses:

1. **It can record its drawing into our pass → Path B.** No resource crosses anything and
   no intermediate texture exists; the same device is required, but nothing is shared.
   The fit for a producer that draws *commands* — a tessellator, a 3D view.
2. **It can render into a texture on our device → Path A.** The application allocates the
   texture and keeps ownership; the renderer samples it. The fit for a producer that can
   only produce a *texture* — a decoder, a camera, an engine with its own pipeline.
3. **Neither — a separate process, or a device it does not control → the host staging
   copy.** Read the frame back and upload it, which is what a `RenderImage`-shaped path
   already does, at the bandwidth the render extension exists to remove (≈1 GB/s at
   1080p60). A GPU→GPU copy would avoid the host, but only within one device — which is
   tier 2's condition rather than an alternative to it.

On Windows with the default `DirectXRenderer`, tiers 1 and 2 are both closed for a *wgpu*
producer, and only tier 3 remains until `WgpuRenderer` is installed; a Direct3D 11 producer's
tier 1 is that renderer itself, which is the revision below.

## The rendezvous is one slot, and it works both ways

"It has to be our device" does not mean GPUI has to be the one that creates it. The
`GpuContext` slot is filled by whoever gets there first and adopted by everyone after:

```rust
let mut ctx_ref = gpu_context.borrow_mut();
let context = match ctx_ref.as_mut() {
    Some(context) => { context.check_compatible_with_surface(&surface)?; context }
    None => ctx_ref.insert(WgpuContext::new(instance, &surface, compositor_gpu)?),
};
```

(`crates/gpui_wgpu/src/wgpu_renderer.rs:308`-`:317`). So an application that already has a
device, adapter and queue it wants to use can build the `WgpuContext`, put it in the slot
from its own factory, and the renderer adopts it;
`check_compatible_with_surface` is the guard that the adapter still presents to this
window. The other direction needs no protocol at all — the first renderer to be built
fills the slot, and every producer afterwards reads `device` and `queue` from it.

The seam therefore does not need "GPUI provides a canvas/context to the application" and
"the application provides GPUI a resource pool" as separate designs. It needs the one
rendezvous, which exists today for device recovery, and the erased payload above it. What
it cannot do is accept a resource from a *second* device, which is what §"What was
rejected" is about.

The second direction — a producer reading `device` and `queue` out of the slot — is what
[`0004`](0004-producer-device-rendezvous.md) builds: `Window::device_any` is that read, and
it took a route to the slot that the code did not have.

## What was rejected

A cross-device draft proposed a tiered model — share the device (Tier 1), bridge with OS
handles (Tier 2), fail loudly (Tier 3). Tiers 1 and 3 are this decision; **Tier 2 is
deferred**, not rejected as impossible — the revision below corrects that reading, and
[`shared-surface.md`](shared-surface.md) measures it working. The reasons it is not taken now
are worth keeping, because they are what taking it later would have to pay.

Tier 2's primitives, per platform, and what wgpu cannot do for each:

| platform | the bridge would be | what wgpu cannot do for it |
| --- | --- | --- |
| macOS | an `IOSurface`-backed `MTLTexture`, handed over as a raw handle, with an `MTLSharedEvent` for ordering | wgpu-hal's Metal backend names no `IOSurface` anywhere, so the application has to build the `MTLTexture` itself with `objc2-metal`; and wgpu's Metal backend speaks `objc2-metal`, not the `metal` crate `gpui_apple` uses |
| Windows | a DXGI shared NT handle (`D3D12_HEAP_FLAG_SHARED` on the producer, `D3D11_RESOURCE_MISC_SHARED_NTHANDLE` on a D3D11 one, `OpenSharedResource1` on the consumer) | wgpu always *creates* with `D3D12_HEAP_FLAG_NONE`, so a texture wgpu allocated cannot be shared; the application has to allocate the shareable resource itself |
| Linux | a dma-buf export | nothing — it is unnecessary: on Linux both sides are wgpu, so Tier 1 applies |

Only the *creation* half of the wall holds: wgpu makes nothing it creates shareable. The
**wrap direction is open** — an application can adopt a shareable resource it allocated —
which the revision below corrects and the shared-surface probe measures.

**None of that makes the bridges exotic.** Every compositor consumes its clients' buffers
through exactly these mechanisms — a dma-buf on Wayland, a DXGI surface for DWM, a
`CAMetalDrawable`'s storage for Core Animation — which is how a window reaches the screen at
all. What is deferred is the *counterparty*, not the mechanism: wgpu offers no way to make a
texture it created shareable, so the bridge has to be driven by the application rather than
through wgpu's texture API. And it is the
**device**, not the process, that forces a bridge — two devices in one process need one as much
as two processes do, and sharing inside one device needs none at all, only two views of the
same resource. That is why tier 3's condition is "a device it does not control" and "a separate
process" is only one way to arrive at it.

Three costs that Tier 2 would have added, each absent from the chosen design:

- **Synchronisation.** A second device means a second queue, so the consumer has to wait
  on the producer: an `MTLSharedEvent` signalled by one command buffer and waited on by
  the other on macOS, a keyed mutex or a fence handshake on D3D, a dma-buf fence on
  Linux. With one device and one queue, submission order is the ordering and there is
  nothing to signal.
- **Adapter identity.** Cross-device sharing needs the same *physical* adapter, a LUID
  match between two devices that were created independently and do not guarantee one.
  `verification.md` §4 records why that is also why it cannot be asserted from inside
  either device.
- **The payoff does not cover the cost.** Path A exists to remove a CPU copy. Tier 2 removes
  the copy and adds a per-platform bridge, a synchronisation protocol and a same-adapter
  constraint — three new obligations for one saved copy.

## What was tried, and how the answer was arrived at

Probe 2 on `windows-latest` established that the *raw* D3D path works: a D3D12 texture on
a shared heap can be opened on a D3D11 device and sampled. With the revision below that is
not a curiosity but the shape of a possible Tier 2 — a producer that allocates its own
D3D12 texture and adopts it can drive the bridge. Whether it works end to end — allocate
shareable, adopt, render through wgpu, read back through D3D11 — is what the second
shared-surface probe measured. The first record is
[`windows-path-a-probe.md`](windows-path-a-probe.md); the new one is
[`shared-surface.md`](shared-surface.md).

Item 2's Windows clause — that `WgpuRenderer` can present there — was measured after it was
written and holds, with two riders that
[`windows-presentation-probe.md`](windows-presentation-probe.md) records. `WgpuRenderer` can
present on a Windows window, but only on Direct3D 12, and reaching it was a backend change and
not plumbing alone: the seam's Windows commit is what enables `Backends::DX12`
(`crates/gpui_wgpu/src/wgpu_context.rs:311`). And the DX12 surface offers `Opaque` alpha alone,
so a window it renders cannot be transparent the way the default renderer's can.

## Revision, 2026-09-28: adoption is app-reachable

The paragraph that called the wall symmetric — "an application cannot hand its own shareable
native texture to wgpu" — is wrong, and it came from a bad grep: `texture_from_raw` does not
match a search for `fn from_raw`. In `wgpu-hal` 29.0.4 every desktop backend exposes

```rust
pub unsafe fn texture_from_raw(resource: <native handle>, …) -> Texture
```

(`wgpu-hal-29.0.4/src/dx12/device.rs:448`, `wgpu-hal-29.0.4/src/metal/device.rs:358`,
`wgpu-hal-29.0.4/src/vulkan/device.rs:406`, `wgpu-hal-29.0.4/src/gles/device.rs:127`), taking
the raw resource by value with public parameter types; `create_texture_from_hal` then wraps it
(`wgpu-29.0.4/src/api/device.rs:325`), and both `wgpu::hal` and `hal::api::*` are public
(`wgpu-29.0.4/src/lib.rs:104`, `wgpu-hal-29.0.4/src/lib.rs:267`). An application never
constructs a `hal::dx12::Texture` — it passes an `ID3D12Resource` it allocated.

So the two halves of the wall are not the same:

- **Creation is still closed.** wgpu cannot make a shareable texture. Its DX12 backend passes
  `D3D12_HEAP_FLAG_NONE` or `…_CREATE_NOT_ZEROED` on everything it creates
  (`wgpu-hal-29.0.4/src/dx12/device.rs:104`, `wgpu-hal-29.0.4/src/dx12/suballocation.rs:440`),
  and its Metal backend has no `IOSurface` entry point. This stands.
- **Adoption is open.** A producer can allocate the shareable resource itself — the raw D3D12
  device is reachable through `as_hal::<Dx12>().raw_device()` — adopt it with
  `texture_from_raw`, render into it with wgpu, and hand the OS handle to a native consumer.

What this makes *possible* is a later milestone: Path A on Windows need not install
`WgpuRenderer` if the producer allocates a shareable D3D12 resource and the two sides
synchronise. The costs listed above do not go away — synchronisation, and a same-adapter
requirement whose LUIDs cannot be compared from inside either device — so the decision keeps
the same-device model for this milestone and records the bridge as future work.
[`shared-surface.md`](shared-surface.md) measures that it works.

## Revision, 2026-09-29: Windows is a choice of renderer, not one renderer

Item 2 said "on Windows both paths require `gpui_wgpu::WgpuRenderer`" and that the default
`DirectXRenderer` "supports neither". The first half is the *wgpu* reading and still holds — a wgpu
producer has no Direct3D 11 backend, so under the default renderer it is cross-device by
construction. The second half does not: a Direct3D 11 producer — Media Foundation, DXVA, a D3D11
engine — makes its texture on the device the default renderer already draws from, and the arm that
samples it is built ([`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md)
§7). So the Windows answer is a choice of renderer per window rather than a required one:
`DirectXRenderer` for a Direct3D 11 producer, `WgpuRenderer` for a wgpu one — and they cannot be the
same window, because the tier follows the renderer. The accessor both use is
[`0004`](0004-producer-device-rendezvous.md). Path B remains unbuilt, so this revision is about
Path A alone.

## What would reopen it

- **Taking the deferred bridge.** Tier 2 is reachable today (the revision above) and measured
  ([`shared-surface.md`](shared-surface.md)); a milestone that needs a foreign-device or
  another-process producer is what would take it, and a *creation*-time shareable API from
  wgpu would make it cheap.
- **A consumer that must run in another process or on a second device** — a hardware
  video decoder, an engine that already owns a device. The bridge then belongs to that
  consumer, and the seam's obligation stays what it is: accept a texture only from the
  renderer's device, and say so when it is not.
- **A decision about Windows' default renderer.** If `DirectXRenderer` were reimplemented
  over wgpu — the same question as "should wgpu be the default?" —
  [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §10 leaves open, item 2 would
  become "install nothing" rather than "install `WgpuRenderer`".
