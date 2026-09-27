# Spike: can a wgpu texture reach GPUI's renderer on Windows?

- **Status:** spike, **not yet run**. The question is answerable here only by
  cross-compiling the probe and running it on Windows; see "Cross-compilation" below for
  what that buys and what the disk on this host allows.
- **Answers one question:** on Windows, can Path A work when the application renders
  with `wgpu` and the window is drawn by GPUI's default `DirectXRenderer`?
- **Inherits:** corrections 1–20 and
  [`wgpu-target-adaptors.md`](wgpu-target-adaptors.md). The other three platforms are
  not in scope — macOS has Metal on both sides, Linux has wgpu on both sides.

## The prior, and why it is already close to settled

The design assumed Windows is "a HAL lookup like the others". Reading wgpu 29's DX12
backend says otherwise, and the evidence points hard at one outcome:

- **A wgpu texture is not shareable, and cannot be made so.** wgpu-hal's DX12 backend
  creates every resource with `D3D12_HEAP_FLAG_NONE` or `…_CREATE_NOT_ZEROED`
  (`wgpu-hal-29.0.4/src/dx12/device.rs:104`,
  `wgpu-hal-29.0.4/src/dx12/suballocation.rs:438`) — never `D3D12_HEAP_FLAG_SHARED`.
  Sharing must be set at creation, so there is no later `CreateSharedHandle` that can
  work.
- **wgpu-hal has no sharing API at all.** There is no `CreateSharedHandle`,
  `OpenSharedResource` or `HEAP_FLAG_SHARED` anywhere in
  `wgpu-hal-29.0.4/src/dx12/`. The only thing the app can do with the underlying
  resource is *read* it: `Texture::raw_resource() -> &ID3D12Resource`
  (`wgpu-hal-29.0.4/src/dx12/mod.rs:990`) — an unshared resource.
- **The reverse direction exists but is not app-reachable.**
  `Device::create_texture_from_hal<A>(hal_texture, …)`
  (`wgpu-29.0.4/src/api/device.rs:325`) would let an app wrap a raw, shareable D3D12
  texture in wgpu — except that it takes a `hal::dx12::Texture`, whose fields are
  private, so an application cannot construct one. It is a wgpu-internal escape hatch,
  not a public one.

And the receiving end, in this tree:

- GPUI's Windows renderer is **Direct3D 11**: `ID3D11Device`, `ID3D11DeviceContext`,
  `ID3D11Texture2D`, `ID3D11ShaderResourceView`
  (`crates/gpui_windows/src/directx_renderer.rs:70`, `:71`, `:79`, `:80`), over DXGI,
  with `RENDER_TARGET_FORMAT = DXGI_FORMAT_B8G8R8A8_UNORM`
  (`crates/gpui_windows/src/directx_renderer.rs:32`).
- Its `draw_surfaces` **is a no-op**: it returns `Ok(())` without drawing when the list
  is non-empty (`crates/gpui_windows/src/directx_renderer.rs:830`). `PaintSurface`
  carries no Windows payload (`crates/gpui_engine/src/scene.rs:749`), so Windows has no
  foreign-surface path at all today.

If the prior holds, the answer is **Outcome B**, and it is not a setback — the renderer
factory we specified is exactly the mechanism that makes it a supported configuration
rather than a fork.

## The two outcomes

- **Outcome A — interop.** Some route turns the application's offscreen pixels into an
  `ID3D11ShaderResourceView` on GPUI's existing D3D11 device, on the same adapter,
  without a CPU copy.
- **Outcome B — no interop.** Path A on Windows is supported only when the window is
  rendered by `gpui_wgpu::WgpuRenderer` (installed through `renderer_factory`), where
  producer and consumer share one device and the payload is simply the `TextureView` —
  the arm Linux already uses.

**Criterion:** a probe either produces a sampled, correct composite through the D3D11
renderer (A), or it cannot, and a wgpu-rendered window does (B).

## Cross-compilation: what it buys, and what it cannot

Verified on this host: `rustup target add x86_64-pc-windows-msvc` succeeds (≈200 MB)
and `cargo check --target x86_64-pc-windows-msvc` passes for a zero-dependency crate
(0.2 s, 92 KB of artifacts). The toolchain path works from Linux.

**What it buys** is the gate the tools repository asks for
(`.tools/docs/journal/maintenance-report.md` §21): a compile check of the Windows
crates from a Linux host, which is what catches "every Linux gate can be green while a
platform backend does not compile". The same job covers `aarch64-apple-darwin` and
`wasm32-unknown-unknown`, which the report also names.

**What it cannot do** is answer the question. Cross-compiling produces a Windows
binary; it does not run it. The probes need D3D11, D3D12 and WARP, none of which exist
on Linux — and Wine's D3D is emulated (WineD3D/vkd3d) in a way that does not model
shared-handle interop or WARP, so running the probe under Wine would answer a question
about Wine.

So the verification splits in two:

- a cross-target `cargo check` job on `ubuntu-latest`, gating the Windows, Apple and
  wasm crates;
- the probe built for Windows and run **natively** on `windows-latest`, where WARP
  supplies both a D3D11 and a D3D12 device on the same adapter — which is the
  configuration probe 2 needs.

The binding constraint on this host is **disk, not the toolchain**: 1.2 GB free. The
`windows` crate alone, and `wgpu` for probe 1, exceed that for a fresh target tree, and
`gpui_windows`'s tree needs several GB. Reclaimable without touching source:
`.morphorm/target` (4.4 GB) and `.parley/target` (3.1 GB), both regenerable with
`cargo clean`, and five unused rustup toolchains (≈1.5 GB each; the clone pins 1.95.0
and the out-of-tree crates pin 1.98.1).

### The two CI jobs

```yaml
cross-check:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - run: rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin wasm32-unknown-unknown
    - run: cargo check --target x86_64-pc-windows-msvc -p gpui_windows -p gpui_authoring
    - run: cargo check --target aarch64-apple-darwin -p gpui_apple -p gpui_macos
    - run: cargo check --target wasm32-unknown-unknown -p gpui_web

path-a-probes:
  runs-on: windows-latest
  steps:
    - uses: actions/checkout@v4
    - run: cargo test --test path_a_probes
```

Probe 1 is the one that needs `wgpu`; probes 2 and 3 need only the `windows` crate. It
is worth splitting them into two test binaries so the compile gate stays cheap.

## Probes, in order

Each is small and each is decisive; stop as soon as one settles it.

1. **Confirm the wgpu resource is unshareable.** Create a `wgpu::Texture`, take
   `view.as_hal::<hal::api::Dx12>()` (`wgpu-29.0.4/src/api/texture_view.rs:73`), call
   `raw_resource()`, then `ID3D12Device::CreateSharedHandle` on it. Expected: failure
   (`E_INVALIDARG` / not shared). Record the HRESULT — this is the fact the whole
   document rests on.
2. **Confirm interop is reachable at all, from raw D3D.** Create a D3D12 texture with
   `D3D12_HEAP_FLAG_SHARED | D3D12_HEAP_FLAG_ALLOW_SIMULTANEOUS_ACCESS`, share it, open
   it with `ID3D11Device1::OpenSharedResource1`, build an SRV, and sample it into a
   GPUI-sized target. If this works, Outcome A is *mechanically* possible — but only
   for an application that creates its own D3D12 texture and therefore does not get to
   use wgpu's texture API. That distinction is the point of the probe.
3. **Check the app-facing wrap.** Determine whether an application can construct a
   `hal::dx12::Texture` (or otherwise reach `create_texture_from_hal`) without forking
   wgpu. Expected: no. If unexpectedly yes, Outcome A widens.
4. **Confirm Outcome B end to end.** On a Windows window, install a custom
   `renderer_factory` returning `gpui_wgpu::WgpuRenderer` at
   `crates/gpui_windows/src/window.rs:145`'s branch (patch 07), create a texture on the
   renderer's own device, push the primitive, and assert the composite contains it.
   This is the recommendation, so it should be proven either way.

Three hazards to fold into whichever probe runs first:

- **Adapter identity.** GPUI's D3D11 device comes from DXGI adapter selection; wgpu's
  D3D12 device from its own. Sharing requires the same *physical* adapter (LUID match).
  Log both LUIDs in probe 2.
- **Synchronisation.** Even if sharing works, D3D11 reading while D3D12 writes tears a
  frame unless there is a keyed mutex or a fence handshake. Nothing in the current
  design carries one; Outcome A would add a synchronisation requirement to the
  `ForeignTextureHandle` contract.
- **Colour space, not yet pinned.** GPUI's Windows target is `…_UNORM`, not `…_SRGB`
  (`crates/gpui_windows/src/directx_renderer.rs:32`). Whether a foreign texture should
  be `Bgra8UnormSrgb` (as the wgpu-target document says) or plain `Bgra8Unorm` depends
  on what GPUI's sampler and shader do with it, and that has not been read. Settle it in
  probe 4 by sampling a known fixture, not by assumption.

## Recommendation (pending probe 1, which is expected to hold)

**Outcome B.** Path A and Path B on Windows are supported when the window is rendered by
`gpui_wgpu::WgpuRenderer`; the default `DirectXRenderer` supports neither. The reasons:

- Probe 1 is expected to fail, and it is decisive: without `D3D12_HEAP_FLAG_SHARED` at
  creation there is no shareable resource, and wgpu offers no way to set it.
- Outcome A's only working route (probe 2) requires the application to abandon wgpu's
  texture API on Windows, which contradicts the reason Path A exists.
- Outcome B costs nothing new. Both sides are the same device, so the payload is the
  `TextureView` the Linux arm already carries
  (`crates/gpui_wgpu/src/wgpu_renderer.rs:1546` is the batch arm still to write), and
  the factory that makes it selectable was specified in
  [`dual-path-ioc-architecture.md`](dual-path-ioc-architecture.md).

## What Outcome B changes in the design

- **Drop the Windows arm of `ForeignTextureHandle`.** The earlier drafts'
  `DirectX(*const c_void)` (SRV) variant rests on the false premise that a wgpu
  producer can feed GPUI's D3D11 renderer. The erased-payload design means the enum can
  simply not have a Windows variant; the payload for wgpu-rendered windows is the
  `TextureView`, on every platform wgpu renders.
- **Make the D3D11 renderer say so.** `draw_surfaces`
  (`crates/gpui_windows/src/directx_renderer.rs:830`) currently succeeds silently for a
  non-empty list; with Outcome B it should return an explicit unsupported error, so an
  application that installs Path A without a wgpu renderer learns why.
- **Document the configuration.** "On Windows, install `gpui_wgpu::WgpuRenderer` to use
  a foreign texture or an inline pass" is a line in the guide, and the first real use of
  the factory that is not a third-party renderer.
- **`gpui_wgpu` gains a Windows surface path.** `WgpuRenderer::new` already takes a
  `HasWindowHandle` (`crates/gpui_wgpu/src/wgpu_renderer.rs:268`) and web/macOS/Linux
  call it; Windows would call it with the typed target's `window_handle`.
- **CI.** Probes 1–3 are unit-testable on `windows-latest` against WARP (both D3D11 and
  D3D12 are available there). If WARP cannot share, that is consistent with Outcome B
  and the gate becomes "Path A works on a wgpu-rendered Windows window".

## What this spike does not decide

- macOS and Linux: unchanged.
- Whether `DirectXRenderer` should eventually be reimplemented over wgpu, retiring the
  D3D11 path entirely. That is the same question as "should wgpu be the default on
  Windows", which [`dual-path-ioc-architecture.md`](dual-path-ioc-architecture.md)
  deliberately leaves open.
