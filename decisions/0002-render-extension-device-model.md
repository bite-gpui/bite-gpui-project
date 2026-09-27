# 0002 — The render extension's device model

- **Decided:** 2026-09-27
- **Status:** decided
- **Evidence:** [`../spi/foreign-texture.md`](../spi/foreign-texture.md) §2 and §5,
  [`../spi/spike-windows-path-a.md`](../spi/spike-windows-path-a.md),
  [`../spi/renderer-seam.md`](../spi/renderer-seam.md) §5.5,
  [`../spi/verification.md`](../spi/verification.md) §4

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
one probe still pending, are in [`../spi/spike-windows-path-a.md`](../spi/spike-windows-path-a.md).

## What would reopen it

- **wgpu gaining an external-memory or shareable-resource API.** Then Tier 2 becomes
  reachable for a wgpu producer and this decision's item 3 is the thing to revisit.
- **A consumer that must run in another process or on a second device** — a hardware
  video decoder, an engine that already owns a device. The bridge then belongs to that
  consumer, and the seam's obligation stays what it is: accept a texture only from the
  renderer's device, and say so when it is not.
- **A decision about Windows' default renderer.** If `DirectXRenderer` were reimplemented
  over wgpu — the same question as "should wgpu be the default?" —
  [`../spi/renderer-seam.md`](../spi/renderer-seam.md) §10 leaves open, item 2 would
  become "install nothing" rather than "install `WgpuRenderer`".
