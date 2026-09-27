# 0002 — The render extension's device model

- **Decided:** 2026-09-27
- **Status:** decided
- **Evidence:** [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §2 and §5,
  [`windows-path-a-probe.md`](windows-path-a-probe.md),
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
   renderer factory. The default `DirectXRenderer` supports neither, and it must say so
   rather than succeed silently — today it returns `Ok(())` without drawing
   (`crates/gpui_windows/src/directx_renderer.rs:830`).
3. **Sharing a resource across devices is out of scope, not pending.** No `IOSurface`
   variant, no DXGI shared-handle variant, no dma-buf. The erased texture payload keeps
   no Windows arm.
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

On Windows with the default `DirectXRenderer`, tiers 1 and 2 are both closed and only
tier 3 remains until `WgpuRenderer` is installed.

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

## What was rejected

A cross-device draft proposed a tiered model — share the device (Tier 1), bridge with OS
handles (Tier 2), fail loudly (Tier 3). Tiers 1 and 3 are this decision; **Tier 2 is
rejected**, and the reasons are worth keeping because they are what would otherwise be
re-litigated.

Tier 2's primitives, per platform, and why each is unavailable:

| platform | the bridge would be | why it does not work here |
| --- | --- | --- |
| macOS | an `IOSurface`-backed `MTLTexture`, handed over as a raw handle, with an `MTLSharedEvent` for ordering | the backing has to be chosen at creation, and `wgpu::TextureDescriptor` has no field for it; wgpu 29 exposes no external-memory API on any backend |
| Windows | a DXGI shared NT handle (`D3D12_HEAP_FLAG_SHARED` on the producer, `D3D11_RESOURCE_MISC_SHARED_NTHANDLE` on a D3D11 one, `OpenSharedResource1` on the consumer) | same wall: sharing is a creation-time property, and wgpu-hal's DX12 backend always passes `D3D12_HEAP_FLAG_NONE` |
| Linux | a dma-buf export | same wall, and unnecessary: on Linux both sides are wgpu, so Tier 1 applies |

The wrap direction is closed too, which is what makes this a wall rather than an
inconvenience: an application cannot hand its own shareable native texture to wgpu,
because `create_texture_from_hal` takes a `hal::*::Texture` whose fields are private
(`wgpu-hal-29.0.4/src/dx12/mod.rs:979`). So Tier 2 only ever works for a producer that
owns the native API **and is not wgpu** — which is not what this feature is for.

**None of that makes the bridges exotic.** Every compositor consumes its clients' buffers
through exactly these mechanisms — a dma-buf on Wayland, a DXGI surface for DWM, a
`CAMetalDrawable`'s storage for Core Animation — which is how a window reaches the screen at
all. What is rejected is the *counterparty*, not the mechanism: the producer at this seam's
other end is a texture wgpu made, and wgpu offers no way to make one shareable. And it is the
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
- **The payoff is zero.** Path A exists to remove a CPU copy. Tier 2 removes the copy and
  adds a per-platform bridge, a synchronisation protocol and a same-adapter constraint —
  and still cannot be driven by a wgpu producer.

## What was tried, and how the answer was arrived at

Probe 2 on `windows-latest` established that the *raw* D3D path works: a D3D12 texture on
a shared heap can be opened on a D3D11 device and sampled. That is the part that looked
like it might rescue Tier 2. It does not, because it requires the producer to create its
own D3D12 texture — the direction probes 1 and 3 close for wgpu. The full record, and the
one probe still pending, are in [`windows-path-a-probe.md`](windows-path-a-probe.md).

Item 2's Windows clause was measured after it was written and holds, with two riders that
[`windows-presentation-probe.md`](windows-presentation-probe.md) records. `WgpuRenderer` can
present on a Windows window, but only on Direct3D 12, which `gpui_wgpu` does not enable today
(`crates/gpui_wgpu/src/wgpu_context.rs:292`) — so item 2 costs a backend change and not only
plumbing. And the DX12 surface offers `Opaque` alpha alone, so a window it renders cannot be
transparent the way the default renderer's can.

## What would reopen it

- **wgpu gaining an external-memory or shareable-resource API.** Then Tier 2 becomes
  reachable for a wgpu producer and this decision's item 3 is the thing to revisit.
- **A consumer that must run in another process or on a second device** — a hardware
  video decoder, an engine that already owns a device. The bridge then belongs to that
  consumer, and the seam's obligation stays what it is: accept a texture only from the
  renderer's device, and say so when it is not.
- **A decision about Windows' default renderer.** If `DirectXRenderer` were reimplemented
  over wgpu — the same question as "should wgpu be the default?" —
  [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §10 leaves open, item 2 would
  become "install nothing" rather than "install `WgpuRenderer`".
