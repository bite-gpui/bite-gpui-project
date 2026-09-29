# Evidence: the Windows Path A probe

- **Evidence for** [decision 0002](0002-render-extension-device-model.md) — the measurement
  it rests on. Kept as evidence, not as a proposal: what the design does with the answer is
  [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §2, and what it did to CI is
  [`../spi/rendering/verification.md`](../spi/rendering/verification.md) §3.
- **The question:** on Windows, can Path A work when the application renders with `wgpu`
  and the window is drawn by GPUI's default `DirectXRenderer`? **Answered: no, for the route
  the probe could reach — Outcome B.** Path A and Path B on Windows require installing
  `gpui_wgpu::WgpuRenderer`.
- **Revised 2026-09-28.** §1's third reading — that an application cannot wrap a native
  shareable resource in wgpu — is wrong. A wgpu producer can allocate a shareable D3D12
  resource itself and adopt it, which reaches the default renderer too, so the conclusion is
  qualified rather than closed: the route exists, and [0002](0002-render-extension-device-model.md)
  keeps the same-device route for this milestone and defers it. See
  [`shared-surface.md`](shared-surface.md). The printout in §4 is unchanged.
- **The other three platforms are out of scope** — macOS has Metal on both sides, Linux has
  wgpu on both.
- **The probe** is the crate at `probes/windows-path-a`, on `bite_v1.22.0-pre` (landed with
  PR #2). Its job answered and was removed, so the printout in §4 is the durable record and
  the crate is the reproduction.

## 1. The prior, and the three readings that settled it

The design assumed Windows was "a HAL lookup like the others". Reading wgpu 29's DX12
backend said otherwise:

- **A wgpu texture is not shareable, and cannot be made so.** wgpu-hal's DX12 backend
  creates every resource with `D3D12_HEAP_FLAG_NONE` or `…_CREATE_NOT_ZEROED`
  (`wgpu-hal-29.0.4/src/dx12/device.rs:104`,
  `wgpu-hal-29.0.4/src/dx12/suballocation.rs:438`) — never `D3D12_HEAP_FLAG_SHARED`.
  Sharing must be set at creation, so no later `CreateSharedHandle` can work.
- **wgpu-hal has no sharing API at all.** No `CreateSharedHandle`, no
  `OpenSharedResource`, no `HEAP_FLAG_SHARED` anywhere in `wgpu-hal-29.0.4/src/dx12/`, and
  nothing in wgpu 29's own source either. An application can *read* the underlying resource
  through `raw_resource() -> &ID3D12Resource` (`wgpu-hal-29.0.4/src/dx12/mod.rs:990`), and
  through the raw device it can allocate and adopt (the third reading), but wgpu itself
  neither shares nor imports.
- **The reverse direction exists, and is app-reachable.** `texture_from_raw` builds a `hal`
  texture from a raw `ID3D12Resource` the caller allocated, and `create_texture_from_hal`
  wraps it (`wgpu-hal-29.0.4/src/dx12/device.rs:448`,
  `wgpu-29.0.4/src/api/device.rs:325`) — both public, with public parameter types. An
  application never constructs a `hal::dx12::Texture`; it passes the resource it made. This
  is the reading this document originally got wrong; the correction is
  [`shared-surface.md`](shared-surface.md) and the revision in
  [0002](0002-render-extension-device-model.md).

## 2. The receiving end, in this tree

- GPUI's Windows renderer is **Direct3D 11** — `ID3D11Device`, `ID3D11DeviceContext`,
  `ID3D11Texture2D`, `ID3D11ShaderResourceView`
  (`crates/gpui_windows/src/directx_renderer.rs:75`, `:73`, `:81`, `:82`) — over DXGI, with
  `RENDER_TARGET_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM`
  (`crates/gpui_windows/src/directx_renderer.rs:34`).
- Its `draw_surfaces` was **a no-op**: it returned `Ok(())` without drawing when the list was
  non-empty, and the seam made it return an explicit unsupported error instead
  (`crates/gpui_windows/src/directx_renderer.rs:852`). `PaintSurface` carries no Windows
  payload (`crates/gpui_engine/src/scene.rs:784`), so Windows has no foreign-surface path at
  all.

So the prior was Outcome B — and it was not a setback, because the renderer factory is
exactly the mechanism that makes it a supported configuration rather than a fork.

## 3. The two outcomes

- **Outcome A — interop.** Some route turns the application's offscreen pixels into an
  `ID3D11ShaderResourceView` on GPUI's existing D3D11 device, on the same adapter, without
  a CPU copy.
- **Outcome B — no interop.** Path A on Windows is supported only when the window is
  rendered by `gpui_wgpu::WgpuRenderer` (installed through the factory), where producer and
  consumer share one device and the payload is simply the `TextureView` — the arm Linux
  already uses. (Revised 2026-09-28: this is the route the probe reached, not the only one;
  Outcome A is reachable through adoption — §1's third reading.)

**Criterion:** a probe either produces a sampled, correct composite through the D3D11
renderer (A), or it cannot, and a wgpu-rendered window does (B).

## 4. The probes

1. **Confirm the wgpu resource is unshareable.** Create a `wgpu::Texture`, take
   `as_hal::<hal::api::Dx12>()` (`wgpu-29.0.4/src/api/texture_view.rs:73`), call
   `raw_resource()`, then `ID3D12Device::CreateSharedHandle` on it. **Status: dropped as
   unnecessary.** §1 shows the flag is never set at creation and D3D requires it there, so
   the call cannot succeed — a run would confirm, on one driver, a failure the source
   determines for every driver.
2. **Confirm interop is reachable at all, from raw D3D.** Create a D3D12 texture with
   `D3D12_HEAP_FLAG_SHARED | D3D12_HEAP_FLAG_ALLOW_SIMULTANEOUS_ACCESS`, share it, open it
   with `ID3D11Device1::OpenSharedResource1`, build an SRV, and sample it into a
   GPUI-sized target. **Status: `interop OK` on `windows-latest`.** It printed, in order: a
   D3D12 device; a shared-heap texture; `CreateSharedHandle -> HANDLE(0x284)`;
   `OpenSharedResource1 -> texture`; an SRV; `PROBE 2: interop OK`. So a D3D12 texture on a
   shared heap **can** be opened on a D3D11 device and sampled. The run is on WARP — a
   hosted runner has no GPU — so what is established is that interop is mechanically
   possible on one adapter, which is also what D3D's documentation promises; hardware is
   expected to behave the same but was not what was measured.
3. **Check the app-facing wrap.** Whether an application can reach `create_texture_from_hal`
   without forking wgpu. **Status: yes, by reading — corrected 2026-09-28.** It does not
   construct a `hal::dx12::Texture`; it calls `texture_from_raw` with its own
   `ID3D12Resource` (§1's third reading). Outcome A is therefore reachable — deferred by
   [0002](0002-render-extension-device-model.md) to a later milestone — so a wgpu producer
   that allocates a shareable resource can reach the default renderer. What proves it is a
   native run of the whole loop, in [`shared-surface.md`](shared-surface.md).
4. **Confirm Outcome B end to end.** On a Windows window, install a factory returning
   `gpui_wgpu::WgpuRenderer`, create a texture on the renderer's own device, push the
   primitive, and assert the composite contains it. **Status: not run.** It is the
   acceptance test for the Windows configuration rather than a question about it, so it is
   [`../spi/rendering/verification.md`](../spi/rendering/verification.md) §1's row for whoever builds Path A.

**The success is narrower than it sounds — but not as narrow as this document first read
it.** Probe 2 shows the *raw* D3D path works, for a producer that creates its own D3D12
texture. It does not make a texture wgpu *allocated* shareable: §1's first reading still
closes that, because wgpu always creates with `D3D12_HEAP_FLAG_NONE`. But §1's third reading
was wrong, so the producer need not avoid wgpu's texture API: it allocates the shareable
resource itself, adopts it, and renders through wgpu. That reaches the default renderer too,
which is what [`shared-surface.md`](shared-surface.md) measures.

## 5. Two hazards, for whoever revisits Outcome A

- **Adapter identity.** GPUI's D3D11 device comes from DXGI adapter selection and wgpu's
  D3D12 device from its own. Sharing requires the same *physical* adapter, a LUID match —
  and [`../spi/rendering/verification.md`](../spi/rendering/verification.md) §4 records why that cannot be
  asserted from inside either device.
- **Synchronisation.** Even where sharing works, D3D11 reading while D3D12 writes tears a
  frame unless there is a keyed mutex or a fence handshake. Nothing in the design carries
  one, because one device and one queue need none; Outcome A would add a synchronisation
  requirement to the `ImportedTextureHandle` contract.

A third thing the probe turned up is not a hazard of the rejected path but an unsettled
rule for the chosen one, and it lives with that rule:
[`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §4, on whether GPUI's Windows
sampler wants an sRGB or a plain `UNORM` foreign texture.
