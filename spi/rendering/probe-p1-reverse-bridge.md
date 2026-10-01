# P1: does the reverse bridge direction work?

- **Status:** proposed — **not run.** This is a probe *specification*: the measurement that gates the
  *reverse* route of the interop crate's `windows` module ([`surface-plan.md`](surface-plan.md) W6,
  [`interop-crate.md`](interop-crate.md) §3). It lives here, with the work it gates, because a
  probe that has run is filed as *evidence* beside its decision and a probe that has not does not
  ([`../../decisions/README.md`](../../decisions/README.md)). When it runs, its printout becomes a
  `decisions/` record the way [`../../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md)
  and [`../../decisions/shared-surface.md`](../../decisions/shared-surface.md) did.
- **Question:** on Windows, can a **Direct3D 11 producer** hand a frame to a **wgpu consumer**
  (GPUI's `WgpuRenderer`) with no CPU copy — the direction *neither* existing probe measured?
- **Gates:** W6, and only W6 — the *guest* route. **Decides:** whether a Direct3D 11 producer's frame
  can reach a `wgpu` **host** (GPUI hosted inside a foreign `wgpu` loop). **Host Mode never needs it**:
  a D3D11 producer uses the default `DirectXRenderer` and a `wgpu` producer uses `WgpuRenderer`, so W5
  is unaffected either way.

## 1. Why this probe exists

Both existing measurements created the shared resource on the **Direct3D 12** side and read it from
Direct3D 11 — [`../../decisions/shared-surface.md`](../../decisions/shared-surface.md) §2 probe 5
renders through wgpu into an app-allocated shared resource and D3D11 reads it back. That is the
`wgpu`/D3D12 *producer* → D3D11 *consumer* direction.

The design's other case is the reverse: a **D3D11 producer** — a Media Foundation or DXVA decoder, a
D3D11 engine — feeding a `wgpu` consumer. The one first-party entry point in that direction is
Vulkan's `texture_from_d3d11_shared_handle`, gated on `VULKAN_EXTERNAL_MEMORY_WIN32`
([`producer-reach.md`](producer-reach.md) §6; `wgpu-hal-29.0.4/src/vulkan/device.rs:544`), which
`shared-surface.md` §2 probe 6 could not reach — the runner had **no Vulkan adapter**. This probe is
probe 6 run where the question can actually be answered.

## 2. What is already known, so the probe does not re-measure it

- **wgpu adopts, and cannot create.** `texture_from_raw` + `create_texture_from_hal` are public on
  all four desktop backends, and wgpu creates nothing shareable (`D3D12_HEAP_FLAG_NONE` on
  everything). So the producer allocates and wgpu adopts — fixed before this probe runs
  ([`surfaces.md`](surfaces.md) §2).
- **Interop is mechanically possible on one adapter.** A D3D12 shared-heap texture can be opened on a
  D3D11 device and sampled (`windows-path-a-probe.md` §4 probe 2, on WARP).
- **The consumer can be the same device.** A D3D11 producer on GPUI's *default* `DirectXRenderer`
  needs no bridge at all — that is the same-device route
  ([`producer-reach.md`](producer-reach.md) §6). This probe is only about the cross-device case.

## 3. What it must measure, and where

**Hardware.** A Windows machine with a GPU and a **Vulkan driver**. A hosted `windows-latest` runner
is not enough on its own — probe 6 found no Vulkan adapter there — so this is a local or
self-hosted-hardware run, and the first step checks which backends wgpu even enumerates.

**Harness.** A scratch crate in the shape of `probes/windows-path-a`, with an optional CI job; the
printout is the durable record, and the crate is kept or removed the way `windows-path-a` was.

## 4. The probes

1. **Environment first.** Enumerate wgpu adapters and record which backends are present (Vulkan vs
   Direct3D 12) and each adapter's name and device type. *A run with no Vulkan adapter answers
   nothing about route A* — it is the condition probe 6 stalled on, not a result.
2. **The producer.** On a Direct3D 11 device, allocate an `ID3D11Texture2D` created shareable
   (`D3D11_RESOURCE_MISC_SHARED_NTHANDLE`, with a keyed mutex or a fence), clear it to a known
   colour, and export the NT handle with `IDXGIResource1::CreateSharedHandle`. *Prints the handle.*
3. **Route A — wgpu on Vulkan.** Import the handle through wgpu-hal's
   `texture_from_d3d11_shared_handle`, wrap with `create_texture_from_hal`, sample it, read it back.
   *Pass: the known colour, byte for byte.*
4. **Route B — wgpu on Direct3D 12.** If Vulkan is absent or route A fails: open the same NT handle
   with `ID3D12Device::OpenSharedHandle` to get an `ID3D12Resource`, adopt it with `texture_from_raw`,
   sample and read back. *Pass: the same colour.*
5. **Synchronisation.** Order the producer against the consumer — a keyed mutex, or a D3D11 fence
   bridged to the consumer's — and confirm no tear and no stall. *This is the half `shared-surface.md`
   §4 records as unmeasured: its Windows probe 5 clears, polls and reads, but carries no fence.*

## 5. What each outcome closes

- **Route A or B passes** → Guest Mode's reverse route exists; record *which* backend, because it
  decides whether the crate needs Vulkan enabled or rides on DX12.
- **Both fail** → the reverse direction is not first-party reachable; Guest Mode's reverse route is
  out, and **W5 is unchanged**. The design answer is then already in hand: a D3D11 producer uses the
  default
  `DirectXRenderer` (same device), and a `wgpu` producer uses `WgpuRenderer` — the two tiers
  [`producer-reach.md`](producer-reach.md) §6 already names.
- **No Vulkan driver** → route A is untested, not failed; the run is inconclusive for it, and route B
  may still answer the DX12 half.

## 6. Hazards it must not mistake for an answer

- **Adapter identity.** Sharing needs the same *physical* adapter — a LUID match between devices
  created independently ([`producer-reach.md`](producer-reach.md) §6;
  `windows-path-a-probe.md` §5). A pass on a machine where both land on one adapter says nothing about
  a two-GPU machine.
- **WARP and virtual adapters.** A result on `Microsoft Basic Render Driver` is "mechanically possible
  on one adapter", not "works on hardware" — the caveat `shared-surface.md` §4 attaches to every
  Windows probe so far.
- **The `VULKAN_EXTERNAL_MEMORY_WIN32` gate.** The entry point is behind a wgpu-hal feature; a build
  that does not enable it makes route A look impossible when it is only unbuilt.
- **Dedicated allocation.** External-memory imports have allocation requirements (a dedicated
  allocation, matching tiling/format); a failure there is a harness bug, not a verdict.

## 7. What it produces

A printout, filed as a `decisions/` evidence record beside
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) and
[`0002`](../../decisions/0002-render-extension-device-model.md) — and the `surface-plan.md` §2 entry
for P1 replaced by a link to it. If it passes, `interop-crate.md` §3's `windows` module is real work;
if it fails, Guest Mode loses only the reverse route and the plan changes with it
([`surface-plan.md`](surface-plan.md) §5).
