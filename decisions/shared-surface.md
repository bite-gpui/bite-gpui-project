# Evidence: the shared-surface probe

- **Evidence for** [decision 0002](0002-render-extension-device-model.md)'s revision and
  [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §2 — whether a
  shared OS buffer (`IOSurface`, a DXGI shared NT handle, a dma-buf) can carry zero-copy frames
  between a GPUI renderer and a foreign producer or consumer. The use case is video and 3D: a
  decoder or an engine renders into a shared surface and a renderer samples it, or the renderer
  writes into one and an encoder reads it.
- **The question:** can a native renderer own a pool of OS-shared surfaces, and can `wgpu` sit
  on either side of one?
- **The probes** ran as scratch commits on `bite_v1.22.0-pre-renderer-seam` (PR #4), with their
  crates and CI jobs. They were experiments and were removed when the branch was cleaned for
  review, so the printouts below are the durable record and there is no crate to reproduce them
  from — unlike [`windows-path-a-probe.md`](windows-path-a-probe.md), whose crate stays.
- **Status:** all three of the pool, the token and the fence are measured on both platforms, and
  Windows adoption works end to end. The one thing left unmeasured is macOS *adoption*.

## 1. What the source settles

Three readings, and the first corrects a conclusion this project previously reached the other
way:

- **Adoption is app-reachable on every desktop backend.** `Device::texture_from_raw` builds a
  `hal` texture from a raw native handle the caller allocated, and `create_texture_from_hal`
  wraps it (`wgpu-hal-29.0.4/src/dx12/device.rs:448`,
  `wgpu-hal-29.0.4/src/metal/device.rs:358`,
  `wgpu-hal-29.0.4/src/vulkan/device.rs:406`, `wgpu-hal-29.0.4/src/gles/device.rs:127`;
  `wgpu-29.0.4/src/api/device.rs:325`). `wgpu::hal` and `hal::api::*` are public
  (`wgpu-29.0.4/src/lib.rs:104`, `wgpu-hal-29.0.4/src/lib.rs:267`). An application never
  constructs a `hal::dx12::Texture`; it passes the resource it made. This is what
  [`windows-path-a-probe.md`](windows-path-a-probe.md) §1 called impossible, and what
  [0002](0002-render-extension-device-model.md) is revised for.
- **Creation is still closed.** wgpu makes nothing it creates shareable: its DX12 backend passes
  `D3D12_HEAP_FLAG_NONE` or `…_CREATE_NOT_ZEROED` on every resource
  (`wgpu-hal-29.0.4/src/dx12/device.rs:104`, `wgpu-hal-29.0.4/src/dx12/suballocation.rs:440`),
  and its Metal backend names no `IOSurface` anywhere. So the division of labour is fixed — the
  application allocates the shareable resource, wgpu adopts it — rather than being a wall.
- **One import already exists, for the read direction.** wgpu-hal's Vulkan backend has
  `texture_from_d3d11_shared_handle`, gated on `VULKAN_EXTERNAL_MEMORY_WIN32`: an
  import-to-read path for a foreign D3D11 frame. Adoption generalises that shape to a render
  target, which is what a wgpu *producer* needs and what the import-only crate
  `wgpu-external-frame` does not offer.

## 2. The printout

Both jobs green. Windows probe 5 is the decisive one; the macOS half ran to the end only after
the correction §3 records.

**Windows** (`shared-surface-windows`, 2m0s):

```
windows-shared-surface probe

--- probe 1: a D3D11 pool member other clients can open ---
probe 1: 512x512 BGRA with shared NT handle HANDLE(0x294) (the token)

--- probe 2: render into the shared surface ---
probe 2: cleared; ReleaseSync(1) ok

--- probe 3: another device opens the token ---
probe 3: first pixel through the imported texture = [32, 192, 64, 255] -> MATCH

--- probe 4: wgpu's own DX12 texture, offered to CreateSharedHandle ---
probe 4: adapter = "Microsoft Basic Render Driver" (Dx12)
probe 4: CreateSharedHandle FAILED on a wgpu texture: The parameter is incorrect. (0x80070057) -- wgpu-hal creates DX12 textures with D3D12_HEAP_FLAG_NONE, so a texture wgpu allocated cannot be exported. This says nothing about the other direction: probe 5 measures wgpu adopting a resource the application allocated, which is what a wgpu producer needs.

--- probe 5: wgpu adopting a shareable resource we allocated ---
probe 5: allocated a D3D12 texture with D3D12_HEAP_FLAG_SHARED, handle HANDLE(0x4b8)
probe 5: wgpu adopted the resource as a render target
probe 5: cleared it through wgpu
probe 5: D3D11 reads wgpu's clear back through the same handle as [32, 192, 64, 255] -> MATCH

--- probe 6: wgpu's Vulkan backend importing a D3D11 shared texture ---
probe 6: no Vulkan adapter: Err(NotFound { active_backends: Backends(0x0), requested_backends: Backends(VULKAN), supported_backends: Backends(VULKAN | GL | DX12), no_fallback_backends: Backends(0x0), no_adapter_backends: Backends(0x0), incompatible_surface_backends: Backends(0x0) }). The hal entry point is texture_from_d3d11_shared_handle, gated on VULKAN_EXTERNAL_MEMORY_WIN32; on a machine with a Vulkan driver this probe would report whether it accepts a D3D11 handle.
```

**macOS** (`shared-surface-macos`, 1m1s):

```
macos-shared-surface probe

--- probe 1: allocate a shared surface (CVPixelBuffer over an IOSurface) ---
probe 1: 512x512 BGRA, stride 2048, IOSurface id = 5 (the token)
probe 1: metal device = "Apple Paravirtual device"

--- probe 2: render into the shared surface ---
probe 2: cleared the surface through its Metal texture

--- probe 3: the write landed in the shared storage ---
probe 3: first pixel = [32, 192, 64, 255] (stride 2048), expected [32, 192, 64, 255] -> MATCH

--- probe 4: the token alone opens the same storage ---
probe 4: io_surface::lookup(5) -> id 5
probe 4: first pixel through the looked-up surface = [32, 192, 64, 255] -> MATCH
probe 4: a pixel buffer over the looked-up surface: wrapped

--- probe 5: MTLSharedEvent across two command buffers ---
probe 5: the waiting buffer completed; signaled value = 1

--- probe 6: wgpu on either side ---
probe 6: adapter = "Apple Paravirtual device" (Metal)
probe 6: wgpu's only external-memory flag, VULKAN_EXTERNAL_MEMORY_WIN32, on this adapter: false
probe 6: a wgpu texture's MTLTexture is reachable for reading: 0x138008510
probe 6: wgpu can produce no shareable texture (wgpu-hal passes an unshareable heap flag on every DX12 texture, and its Metal backend has no IOSurface entry point at all). It can adopt one, through hal's Device::texture_from_raw followed by Device::create_texture_from_hal, both of which exist on all four desktop backends. The application has to build the MTLTexture over its own IOSurface first, with objc2-metal; this probe does not do that yet, and probe 5 of the Windows half does measure the equivalent adoption on DX12.
```

## 3. The outcome

**Windows: adoption works.** Probe 5 is the measurement
[0002](0002-render-extension-device-model.md) was reopened for, and it passed end to end: wgpu
adopted an application-allocated `D3D12_HEAP_FLAG_SHARED` resource as a render target, cleared
through it, and a D3D11 device opened the same handle and read the clear back —
`[32, 192, 64, 255] -> MATCH`. So §1's first reading is not only source-backed but measured: a
wgpu producer can allocate a shareable resource, render into it, and hand the NT handle to
GPUI's Direct3D 11 renderer, which is Outcome A in
[`windows-path-a-probe.md`](windows-path-a-probe.md) §3. The neighbouring probes bound it:
probe 4 confirms the *export* direction is closed — `CreateSharedHandle` on a wgpu-allocated
texture fails with `0x80070057` — and probe 3 confirms that the pool a token and a keyed mutex
make up works on D3D11. Probe 6 found no Vulkan adapter on the runner, as anticipated.

**macOS: the pool, the token and the fence work.** Probe 1 allocates a `CVPixelBuffer` over an
`IOSurface` and prints its id — `5`, the token. Probe 2 is the half that was unproven: the
surface is cleared *through* the `MTLTexture` the `CVMetalTextureCache` hands back, the same
call `MetalRenderer` already makes for decoded frames, so a renderer can be the producer of a
pool and not only its consumer. Probe 3 reads the clear back on the CPU, probe 4 shows the id
alone opens the same storage — `io_surface::lookup(5)` reads the pixel and a fresh
`CVPixelBuffer` wraps the surface — and probe 5 orders two command buffers with an
`MTLSharedEvent`. The first run is why the probe needed correcting: it created the buffer with a
null attribute dictionary and the runner answered that no `IOSurface` stands behind it, so the
probe stopped before probe 2. A `CVPixelBuffer` is IOSurface-backed only when the attributes
ask for it, through `kCVPixelBufferIOSurfacePropertiesKey`. What macOS
still does not measure is *adoption*: probe 6 reaches a wgpu texture's `MTLTexture` for
reading, but nothing builds one over an `IOSurface` with `objc2-metal` and adopts it, so that
half of §1's first reading rests on the source, not on a run.

## 4. What is not measured

- **macOS adoption**, above: the Metal half of `texture_from_raw` is read, not run.
- **Hardware.** The Windows job runs on WARP (`Microsoft Basic Render Driver`) and the macOS
  job on `Apple Paravirtual device`, so a pass is *mechanically possible on one adapter* rather
  than *works*.
- **Adapter identity and synchronisation** — the two hazards
  [`windows-path-a-probe.md`](windows-path-a-probe.md) §5 names. Windows probe 5 clears, waits
  (a `poll`) and then reads through the other device, but carries no fence across the two
  devices; the macOS fence probe 5 uses orders two buffers on *one* device, which is the pool's
  case and not the cross-device one.
- **The consumer side in GPUI.** No renderer samples an RGBA foreign texture yet; see
  [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §1.
