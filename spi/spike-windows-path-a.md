# Spike: can a wgpu texture reach GPUI's renderer on Windows?

- **Status:** spike, **answered**. Probe 2 ran green on `windows-latest`; probe 3 was
  answered by reading; probe 1's surface is confirmed and its run is still pending. The
  result is **Outcome B** below.
- **Answers one question:** on Windows, can Path A work when the application renders with
  `wgpu` and the window is drawn by GPUI's default `DirectXRenderer`?
- **The other three platforms are not in scope** — macOS has Metal on both sides, and
  Linux has wgpu on both sides. What this settled for the design is
  [`foreign-texture.md`](foreign-texture.md) §2; what it settled for CI is
  [`verification.md`](verification.md) §3.

## 1. The prior, and why it was already close to settled

The design assumed Windows was "a HAL lookup like the others". Reading wgpu 29's DX12
backend said otherwise, in three steps:

- **A wgpu texture is not shareable, and cannot be made so.** wgpu-hal's DX12 backend
  creates every resource with `D3D12_HEAP_FLAG_NONE` or `…_CREATE_NOT_ZEROED`
  (`wgpu-hal-29.0.4/src/dx12/device.rs:104`,
  `wgpu-hal-29.0.4/src/dx12/suballocation.rs:438`) — never `D3D12_HEAP_FLAG_SHARED`.
  Sharing must be set at creation, so there is no later `CreateSharedHandle` that can
  work.
- **wgpu-hal has no sharing API at all.** There is no `CreateSharedHandle`,
  `OpenSharedResource` or `HEAP_FLAG_SHARED` anywhere in `wgpu-hal-29.0.4/src/dx12/`, and
  nothing in wgpu 29's own source either. The only thing the app can do with the
  underlying resource is *read* it: `raw_resource() -> &ID3D12Resource`
  (`wgpu-hal-29.0.4/src/dx12/mod.rs:990`) — an unshared resource.
- **The reverse direction exists but is not app-reachable.** `create_texture_from_hal`
  would let an app wrap a raw, shareable D3D12 texture in wgpu, except that it takes a
  `hal::dx12::Texture` whose fields are private (`wgpu-hal-29.0.4/src/dx12/mod.rs:979`).
  It is a wgpu-internal escape hatch, not a public one.

And the receiving end, in this tree:

- GPUI's Windows renderer is **Direct3D 11** — `ID3D11Device`, `ID3D11DeviceContext`,
  `ID3D11Texture2D`, `ID3D11ShaderResourceView`
  (`crates/gpui_windows/src/directx_renderer.rs:70`, `:71`, `:79`, `:80`) — over DXGI,
  with `RENDER_TARGET_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM`
  (`crates/gpui_windows/src/directx_renderer.rs:32`).
- Its `draw_surfaces` **is a no-op**: it returns `Ok(())` without drawing when the list
  is non-empty (`crates/gpui_windows/src/directx_renderer.rs:830`). `PaintSurface` carries
  no Windows payload (`crates/gpui_engine/src/scene.rs:749`), so Windows has no
  foreign-surface path at all today.

So the prior was Outcome B — and it was not a setback, because the renderer factory is
exactly the mechanism that makes it a supported configuration rather than a fork.

## 2. The two outcomes

- **Outcome A — interop.** Some route turns the application's offscreen pixels into an
  `ID3D11ShaderResourceView` on GPUI's existing D3D11 device, on the same adapter,
  without a CPU copy.
- **Outcome B — no interop.** Path A on Windows is supported only when the window is
  rendered by `gpui_wgpu::WgpuRenderer` (installed through the factory), where producer
  and consumer share one device and the payload is simply the `TextureView` — the arm
  Linux already uses.

**Criterion:** a probe either produces a sampled, correct composite through the D3D11
renderer (A), or it cannot, and a wgpu-rendered window does (B).

## 3. The probes

1. **Confirm the wgpu resource is unshareable.** Create a `wgpu::Texture`, take
   `as_hal::<hal::api::Dx12>()` (`wgpu-29.0.4/src/api/texture_view.rs:73`), call
   `raw_resource()`, then `ID3D12Device::CreateSharedHandle` on it. Expected: failure
   (`E_INVALIDARG`, not shared). **Status: surface confirmed**
   (`wgpu-hal-29.0.4/src/lib.rs:269`), **run still pending** — it would record the
   HRESULT this document rests on.
2. **Confirm interop is reachable at all, from raw D3D.** Create a D3D12 texture with
   `D3D12_HEAP_FLAG_SHARED | D3D12_HEAP_FLAG_ALLOW_SIMULTANEOUS_ACCESS`, share it, open
   it with `ID3D11Device1::OpenSharedResource1`, build an SRV, and sample it into a
   GPUI-sized target. **Status: `interop OK` on `windows-latest`.** The probe is on the
   branch `bite_v1.22.0-pre-path-a-probe`, at `probes/windows-path-a` (PR #1). It printed,
   in order: a D3D12 device; a shared-heap texture; `CreateSharedHandle -> HANDLE(0x284)`;
   `OpenSharedResource1 -> texture`; an SRV; `PROBE 2: interop OK`. So a D3D12 texture on
   a shared heap **can** be opened on a D3D11 device and sampled. The run is on WARP — a
   hosted runner has no GPU — so what is established is that interop is mechanically
   possible on one adapter, which is also what D3D's documentation promises; hardware is
   expected to behave the same but was not what was measured.
3. **Check the app-facing wrap.** Whether an application can construct a
   `hal::dx12::Texture` (or otherwise reach `create_texture_from_hal`) without forking
   wgpu. **Status: no, by reading** — §1. If it were unexpectedly yes, Outcome A would
   widen.
4. **Confirm Outcome B end to end.** On a Windows window, install a factory returning
   `gpui_wgpu::WgpuRenderer`, create a texture on the renderer's own device, push the
   primitive, and assert the composite contains it. **Status: not run**, and it cannot be
   until [`renderer-seam.md`](renderer-seam.md) and
   [`foreign-texture.md`](foreign-texture.md) exist. This is the recommendation, so it
   should be proven either way.

**The success is narrower than it sounds.** Probe 2 shows the *raw* D3D path works — for
a producer that creates its own D3D12 texture. It does not make a wgpu-produced texture
shareable: probe 1 and probe 3 still close that direction. So Outcome A is reachable for
an application that does not use wgpu's texture API on Windows, and Outcome B remains the
recommendation for Path A as designed.

## 4. Three hazards, for whoever runs probe 4 or revisits Outcome A

- **Adapter identity.** GPUI's D3D11 device comes from DXGI adapter selection and wgpu's
  D3D12 device from its own. Sharing requires the same *physical* adapter (a LUID match).
- **Synchronisation.** Even where sharing works, D3D11 reading while D3D12 writes tears a
  frame unless there is a keyed mutex or a fence handshake. Nothing in the current design
  carries one; Outcome A would add a synchronisation requirement to the
  `ImportedTextureHandle` contract.
- **Colour space, not yet pinned.** GPUI's Windows target is `…_UNORM`, not `…_SRGB`
  (`crates/gpui_windows/src/directx_renderer.rs:32`), so whether a foreign texture should
  be `Bgra8UnormSrgb` or plain `Bgra8Unorm` depends on what GPUI's sampler and shader do
  with it, and that has not been read. Settle it by sampling a known fixture, not by
  assumption.

## 5. What Outcome B changes in the design

- **Drop the Windows arm of `ImportedTextureHandle`.** The drafts' `DirectX(*const c_void)`
  variant rests on the false premise that a wgpu producer can feed GPUI's D3D11 renderer.
  With the payload erased, the enum simply does not have a Windows variant.
- **Make the D3D11 renderer say so.** `draw_surfaces`
  (`crates/gpui_windows/src/directx_renderer.rs:830`) currently succeeds silently for a
  non-empty list; it should return an explicit unsupported error, so an application that
  registers a texture without a wgpu renderer learns why.
- **Document the configuration.** "On Windows, install `gpui_wgpu::WgpuRenderer` to use a
  foreign texture or an inline pass" is a line in the guide, and it is the first real use
  of the factory that is not a third-party renderer.
- **`gpui_wgpu` gains a Windows surface path.** `WgpuRenderer::new` already takes a
  `HasWindowHandle` (`crates/gpui_wgpu/src/wgpu_renderer.rs:268`) and web, macOS and Linux
  call it; Windows would call it with the typed target's `window_handle`.

## 6. What this does not decide

- **macOS and Linux**: unchanged, and unaffected — same API on macOS, same device on
  Linux.
- **Whether `DirectXRenderer` should eventually be reimplemented over wgpu**, retiring the
  D3D11 path entirely. That is the same question as "should wgpu be the default on
  Windows", which [`renderer-seam.md`](renderer-seam.md) §10 leaves open and this spike
  does not foreclose.
