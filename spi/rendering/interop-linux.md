# The Linux hatch: an exported dma-buf, ordered by a dma-fence

- **Status:** built — the `gpui_interop` repository's `linux` module (`crates/gpui_interop/src/linux.rs`) exports a shareable `BGRA8`/`RGBA8` or two-plane
  `NV12` image, linear or on a driver-tiled modifier the renderer can import, and a `Fence` that hands
  over the acquire `sync_file`; Linux unit tests allocate each on a real device. §6's adapter step
  landed too: `Adapter::gpu_device` makes a second logical device on the renderer's own
  `vk::PhysicalDevice`, so an export lands on the GPU the window draws on rather than whichever node
  `GpuDevice::open` finds. `crates/gpui_interop/examples/interop_linux.rs` composes the whole path into
  a window, and `crates/gpui_interop/tests/linux_dmabuf_readback.rs` (behind the `readback-test`
  feature) composites a **linear** and a **tiled** RGB buffer and a **linear NV12** one through the
  renderer, reading the RGB pair back byte for byte and the `NV12` one through the shader's BT.601
  matrix — §9 is what that turned up. `SharedBuffer::new` also pre-checks the format against the
  device, so an unsupported request is a named error rather than a bare
  `VK_ERROR_FORMAT_NOT_SUPPORTED`. The adapter step grew the other half a producer that owns its
  device needs: `Adapter::gpu_id` reads the renderer's GPU *identity* — the DRM render node a VA
  decoder or another process opens — and remembers it for a producer off the paint, which
  `gpui_video`'s element reports and `gpui_va` consumes in place of a hardcoded node (§6, §10). P3
  ([`probe-p3-dmabuf-import.md`](probe-p3-dmabuf-import.md)) is the consumer-side import it pairs with.
  The hatch also grew a **render-target** mode — `SharedBuffer::render_target` adds `COLOR_ATTACHMENT`,
  so a producer's own pipeline draws the frame rather than only `clear`/`fill`ing it — and
  `crates/gpui_interop/examples/interop_cube.rs` is that producer: a cube rendered by a Vulkan pipeline
  the example builds on the `GpuDevice` and hands over as a dma-buf, recycled on the renderer's
  `release` (§11).
- **Companion:** [`producer-seam.md`](producer-seam.md) §5, the two ways a producer gets a dma-buf;
  [`surface-seam.md`](surface-seam.md), the total buffer description and the fences on the handle;
  [`player.md`](player.md) §2, the release invariant a recycling producer keeps.

## 1. What it is, and what it is not

On Windows the hatch is the producer's half of a Direct3D 12 → 11 shared texture: allocate shareable,
`CreateSharedHandle`, signal the fence. The renderer owns the other half — `OpenSharedResource1`, the
SRV, the GPU-side wait ([`interop-crate.md`](interop-crate.md) §5).

The Linux module is the same three-step interchange in this platform's clothes — **match the adapter,
move a handle, order the queues** — with two substitutions:

- **the handle is a dma-buf**, not an NT handle: an fd plus a fourcc, a DRM format modifier and a
  per-plane offset/stride;
- **the consumer half is core.** `gpui_wgpu` already imports a dma-buf under its modifier and samples
  it through the NV12 shader ([`player.md`](player.md)'s diagram); the module exports, it does not
  import on the consumer's behalf.

## 2. The payload it must produce

The engine's `DmaBufHandle` is the whole description, and two of its fields decide how the hatch works:

| field | what the hatch owes |
| --- | --- |
| `width`, `height`, `format` (`SurfaceFormatKind`) | the image's geometry and the plane count/subsampling |
| `modifier` | the DRM format modifier the image was created with — never `LINEAR` unless the driver is |
| `planes[]` (`fd`, `offset`, `stride`) | one entry per plane, from the image's layout |
| `acquire_fence` | a **`sync_file`** the renderer waits on before sampling |
| `release` | an **`eventfd`** the renderer writes once the sampling submission completes |

The asymmetry in the last two rows is the engine's, not this module's: the acquire fence is a real
`sync_file`, but the release is an `eventfd` because `wgpu` owns its queue and offers no way to export
a `sync_file` ([`surface-seam.md`](surface-seam.md) §5). The hatch makes one of each;
it does not get to choose the shapes.

## 3. The two ways a producer gets a dma-buf

[`producer-seam.md`](producer-seam.md) §5 draws the line, and it is the same one here:

- **VA-API exports directly.** `vaExportSurfaceHandle` hands back fd, modifier and planes; `gpui_va`
  builds the `DmaBufHandle` and needs none of this module.
- **A general producer must export its own.** A `wgpu` or 3D producer, or another process, has no
  driver-level export. It allocates a shareable image and exports the fd itself — `VK_KHR_external_memory_fd`
  or `EGL_LINUX_DMA_BUF_EXT` — which is **this module**.

## 4. The shape, mirroring `windows.rs`

```rust
/// A shareable image on the producer's device — the counterpart of `SharedSurface`.
pub struct SharedBuffer { .. }

impl SharedBuffer {
    /// Allocate a shareable image of `size` in `format`, in a modifier the renderer can import.
    pub fn new(device: &Device, size: Size<DevicePixels>, format: Format) -> Result<Self>;
    /// `new` with `COLOR_ATTACHMENT` — a shareable image a pipeline can render into (§11).
    pub fn render_target(device: &Device, size: Size<DevicePixels>, format: Format, tiling: Tiling) -> Result<Self>;
    /// The engine's description, ready for `surface()`.
    pub fn handle(&self) -> &DmaBufHandle;
    /// The producer's own render target.
    pub fn image(&self) -> &vk::Image;
    /// Fill it from host plane bytes — the render `clear` cannot do for a multi-plane format.
    pub fn fill(&self, device: &Device, planes: &[&[u8]]) -> Result<()>;
}

/// A fence signalled on the producer's queue — the counterpart of `Fence`.
pub struct Fence { .. }

impl Fence {
    pub fn new(device: &Device) -> Result<Self>;
    /// Signal and hand back a `sync_file` for the consumer's acquire wait.
    pub fn signal(&mut self, queue: &vk::Queue) -> Result<OwnedFd>;
}
```

As on Windows, this is **producer half only**: the acquire `sync_file` goes on the handle, and the
release `eventfd` is the engine's convention that the renderer signals back.

## 5. The Vulkan specifics, which are the actual work

`wgpu` cannot create a shareable image ([`interop-crate.md`](interop-crate.md) §7 — that is what would
reopen this), so the hatch is raw Vulkan through `ash`, a per-platform dependency the way `windows` is
on Windows. The list is short and unforgiving:

- the device must have `VK_KHR_external_memory_fd` and `VK_EXT_external_memory_dma_buf`;
- the format is checked against the device **before** `vkCreateImage` — the feature bits the chosen
  tiling carries and the extent the device caps — so an unsupported format, tiling or size is a named
  error rather than a bare `VK_ERROR_FORMAT_NOT_SUPPORTED`;
- the image is created with `VkExternalMemoryImageCreateInfo` (`DMA_BUF_BIT_EXT`) and, for a tiled
  format, a `VkImageDrmFormatModifierExplicitCreateInfoEXT` naming the modifier;
- memory is allocated with `VkExportMemoryAllocateInfo` and the fd taken with `vkGetMemoryFdKHR`;
- the modifier and plane layout come back from `vkGetImageDrmFormatModifierPropertiesEXT` (or the
  explicit layout that was asked for), and must agree with the modifier and strides on the handle;
- the acquire fence is exported with `VK_EXTERNAL_FENCE_HANDLE_TYPE_SYNC_FD_BIT` (or the semaphore
  equivalent) — a `sync_file` is what the engine's `acquire_fence` takes.

By default the image is created for **transfers and sampling** only: `SharedBuffer::new` writes it with
transfers (`clear`, `fill`) and the consumer samples it, so no usage asks for `COLOR_ATTACHMENT` —
which linear tiling does not report on these devices anyway. `SharedBuffer::render_target` is the other
mode: it adds `COLOR_ATTACHMENT`, for a producer whose own pipeline draws the frame ([§11](#11-the-render-target-mode-and-the-foreign-3d-viewport)).
Because a linear image cannot carry that bit, a rendered buffer is always `Tiling::Tiled` — and on a
device that is not the renderer's, where no modifier is sound, the request is refused rather than
falling back to a linear image that cannot be drawn into. The capability check above asks for exactly
the bits each mode uses, so it cannot reject a linear buffer the driver would have made.

## 6. Matching the adapter, which is not a LUID

[`interop-crate.md`](interop-crate.md) §3 is explicit that `attach` must not become LUID-shaped: on
Windows a match is a LUID; on macOS there is nothing to match; **on Linux it is the renderer's own
physical device**. So the hatch must be handed a device on the renderer's node — or told which node to
open — rather than guessing, and the `Adapter` module owes Linux a `None`-or-device answer, not a failed
match.

**Built, and the honest answer turned out to be the renderer's `VkPhysicalDevice`.** The window's
renderer is `gpui_wgpu::WgpuRenderer`, whose `GpuRenderer::Device` is a `(wgpu::Device, wgpu::Queue)`.
`Adapter::gpu_device` reads that device's HAL Vulkan instance and physical device
(`as_hal::<wgpu::hal::vulkan::Api>()` → `shared_instance().raw_instance()` + `raw_physical_device()`),
and `GpuDevice::on(instance, physical)` makes a **second logical device** on them. That is the Linux
counterpart of the Windows LUID match, and it comes with the same separation the Direct3D arm gets from
D3D12-versus-D3D11 for free: a second device means the producer's queue never races the renderer's. The
instance is the renderer's, so `GpuDevice` tracks whether it owns the instance and destroys only its
own; `GpuDevice::open` stays the fallback for a renderer that lends no device, or is not wgpu.

The example, `crates/gpui_interop/examples/interop_linux.rs`, is the four lines end to end —
`attach(gpu.try_device::<WgpuRenderer>())` → `.adapter().gpu_device()` → `SharedBuffer::new` +
`clear` → `gpu.paint_surface(buffer.handle())` — with the fallback spelled out, so the header can report
which of the two devices the tiles were built on.

**The identity, for a producer that owns its device.** `Adapter::gpu_device` lends the renderer's
device to a producer that renders *on* it — the `SharedBuffer` path. A producer that owns its device
instead — a VA decoder, a 3D engine, another process — cannot render on someone else's device, but
still has to land on the renderer's GPU or its dma-buf will not import, so what it needs is the GPU's
**identity**, not its device. `Adapter::gpu_id` reads it as a [`GpuId`] — the device name Vulkan
reports, and the DRM render node (`VK_EXT_physical_device_drm`: `/dev/dri/renderD<minor>`) a non-Vulkan
producer opens — and **remembers** it process-wide, so a producer that runs off the paint reads it
later with `gpui_interop::gpu_id`. Two consumers landed on it, both in §10: `gpui_va` opens the node it
names instead of a node picked by hand, and `gpui_video`'s element is what reports it.

## 7. What it does not do

- **No consumer half.** Import, the modifier-aware sampling and the wait are core, as on Windows.
- **No readback.** The CPU case is `read_pixels` ([`interop-crate.md`](interop-crate.md) §5).
- **No VA-API export.** That is the producer's (`vaExportSurfaceHandle`), §3.
- **No `SurfaceProducer` over the foreign path.** The crate exports and hands over a `DmaBufHandle`;
  a producer *type* that owns its ring and decoder and paints itself is a `gpui_video` sample, and the
  examples here deliberately do not depend on `gpui_video` (§11).

## 8. What would reopen it

- **`wgpu` gaining shareable creation.** Then §5 collapses from a handshake to a descriptor, and the
  module shrinks the way [`interop-crate.md`](interop-crate.md) §7 says the Windows one would.
- **A cross-vendor modifier that is not importable.** The hatch falls back to a linear buffer and warns,
  which costs the colour it was tiling for — the same warning `gpui_va` gives on the linear case.
- **P3 failing.** Then there is no Linux arm to give this to, and the module stays a stub.

## 9. The read-back test, and the three things it caught

The read-back test (`crates/gpui_interop/tests/linux_dmabuf_readback.rs`) composites a linear and a
tiled buffer through the headless `wgpu` renderer and reads each pixel back. Getting both arms green
caught two faults, neither in the renderer; adding the `NV12` arm later caught a third, in the
capability check.

**The export needed a dedicated allocation.** `SharedBuffer::new` allocated with
`VkExportMemoryAllocateInfo` alone. On NVIDIA a *linear* dma-buf exported that way never finished being
sampled: the consumer's submission timed out — `Failed to wait for headless rendering: The requested
Wait timed out before the submission was completed.` — while the same buffer exported from Intel was
fine, which is why it went unnoticed. `VkMemoryDedicatedAllocateInfo` fixes it, and it is what the two
other dma-buf paths here already allocate with: `gpui_wgpu`'s importer and `gpui_va`'s tiled fill.

**A modifier is portable only within a vendor.** The test's first form built its buffers with
`GpuDevice::open`, which picks its own GPU, while the headless renderer picks its own as well — and
with no compositor to match, its sort falls through to "discrete first". On the two-GPU machine this
ran on (Intel HD 520 + NVIDIA 930M) that exported an Intel `Y_TILED` (`0x100000000000001`) buffer for
an NVIDIA renderer, which cannot import it. The modifier list is the *device's* — NVIDIA offers only
its own `0x0300000004fe01x` block, Intel only `0x10000000000000x`, llvmpipe only `LINEAR` — so a buffer
built from one device's list and consumed on another's has no modifier in common. Routing both ends
through `Adapter::gpu_device` (§6) removes it: the producer's device is on the renderer's
`vk::PhysicalDevice`, so the modifier is picked from the list of the GPU that will sample it.

**The capability check asked for more than the hatch uses.** Its first form required
`COLOR_ATTACHMENT` for an RGB image, and a linear `R8G8B8A8_UNORM` image does not report that bit —
linear tiling cannot be rendered to, so the check rejected a linear buffer the driver makes happily
(and that both arms had been clearing and sampling all along). The fix is to request exactly the usages
the hatch performs — transfers to write, sampling to read — which is what `usage_for` now builds.

All three arms now pass on both devices, with no environment pinning: `LINEAR` everywhere, `Y_TILED`
on Intel and `0x0300000004fe015` on NVIDIA.

The corollary is worth stating plainly, because it is the sharp edge of §6: **`Tiling::Tiled` is only
sound on a device the consumer shares.** `Adapter::gpu_device` gives that by construction;
`GpuDevice::open` — the no-renderer fallback — does not, and a producer that opens its own device and
asks for `Tiled` can export a modifier the renderer will refuse. Handing it the renderer's device is
the whole point of the adapter step, so the hatch enforces it rather than documenting it: `Tiled` is
honoured only on a device `Adapter::gpu_device` lent, and a producer-opened one — or one that offers no
importable modifier — exports linear with a warning. Wedging the renderer is not a failure a library
should leave to a doc comment, and a silently mistiled buffer is worse than a slow one.

## 10. The GPU match in the wild, and the node that was hardcoded

The identity (§6) earned its place by fixing a real producer. `gpui_va` decoded on
`/dev/dri/renderD128`, the Intel render node *the compositor and the window renderer are steered to* —
on a laptop with a discrete GPU, correct only while the renderer follows the compositor, and wrong the
moment it does not. The headless renderer, with no compositor to match, sorts to "discrete first": on
the two-GPU machine this ran on it draws through the NVIDIA 930M (`renderD129`), so a VA surface
exported on Intel was one the renderer could not import.

**`gpui_va` now reads the node, and refuses to guess.** `render_node()` is
`gpui_interop::gpu_id().and_then(|gpu| gpu.render_node)`, and both sites that hardcoded the path —
ffmpeg's `av_hwdevice_ctx_create` and `vaGetDisplayDRM` — go through it. There is deliberately **no
fallback**: a dma-buf is importable only on the renderer's own GPU, so a node chosen by hand is one the
renderer cannot sample, and a producer with no GPU to name does nothing rather than export a surface
the consumer will refuse. Failure beats a wrong node, which is the wedge §9 is about.

**`gpui_video`'s element is what names the GPU.** A producer off the paint has no window to ask, and
the renderer's device is reachable only from a `gpu_canvas`, so a Linux video paints one **reporting**
canvas before it opens its decoder: `report_renderer_gpu` runs `attach(gpu.try_device::<WgpuRenderer>())
→ adapter().gpu_id()` → `remember`, and the element defers producing until `gpui_interop::gpu_id()`
answers (asking for one more animation frame to get there). The frame's own `surface()` with its
`object_fit` is untouched; only the pre-report frame uses the canvas. Off Linux the element is
unchanged.

**Verified on a real window, and seeded in tests.** Running `gpui_video`'s `player` example with
tracing shows the sequence — `reported=false` → `report_renderer_gpu: device=true` → `GpuId { name:
"Intel(R) HD Graphics 520", render_node: Some("/dev/dri/renderD128") }` → `reported=true` →
`serve open_backend=true` → `opened, period 40ms` → `frame at 0.00s …` — the element reports the
*window renderer's* GPU (the compositor's Intel node), `gpui_va` opens the decoder on it, and frames
play. `gpui_va`'s unit tests have no window, so they seed the identity explicitly (`remember(GpuDevice::open().gpu_id())`); a real caller never does.

## 11. The render-target mode, and the foreign 3D viewport

`SharedBuffer::render_target` is `new` with `COLOR_ATTACHMENT` added. That one bit changes what the
crate is for: `new` produces a buffer the hatch *fills* (`clear`, `fill`), `render_target` produces a
frame a producer's **own pipeline draws** — the foreign viewport, where the pixels come from a 3D engine
or another API that is not GPUI's renderer at all.

**Why it forces the renderer's device.** A linear image does not report `COLOR_ATTACHMENT` (§5), so a
rendered buffer must be `Tiling::Tiled` — and `Tiled` is honoured only on the device
`Adapter::gpu_device` lends (§6, §9). `GpuDevice::open` is therefore not a fallback here as it is for
`clear`/`fill`: it would ask for a tiled render target on a device whose modifier the renderer may not
know, so the request is refused, and the example reports that rather than showing an empty canvas.

**The cube.** `crates/gpui_interop/examples/interop_cube.rs` is the mode end to end. It builds a small
Vulkan pipeline on the `GpuDevice` — a render pass with the shareable colour image and a depth
attachment, a `mat4x4` uniform, 24 vertices — and compiles its WGSL to SPIR-V at run time with `naga`,
the frontend and backend `wgpu` itself uses, so no offline shader compiler (`glslc`,
`glslangValidator`) is needed. `naga`'s default writer options flip the clip-space Y for Vulkan, so the
projection is the ordinary y-up, zero-to-one-depth one a `wgpu` shader is written against, and the cube
comes up the right way round.

**The ring.** One buffer cannot be sampled by the compositor and drawn into by the producer at once, so
the example keeps three and cycles: each painted frame takes a free slot, draws into it, and hands it
over with an acquire `Fence::sync_file` and a fresh `release` `eventfd`. A slot is reused only once the
compositor has signalled that release, or a `RELEASE_TIMEOUT` reclaims it so a release that never comes
does not freeze the cube. This is `gpui_va`'s ring in miniature ([`player.md`](player.md) §2 is the same
invariant), and the depth image and framebuffer are per-slot for the same reason: two frames' passes
must not share a depth attachment.

**What was checked, and how.** The window's output cannot be screenshotted in this environment, so the
cube was verified by reading a rendered slot back to host memory and printing it as ASCII. At a rotated
pose the three visible faces come out as distinct regions — top, front and side — the top face is the
brightest and sits **above** the others, which is the one thing the projection and `naga`'s flip have to
agree on, and the silhouette is centred and inside the frame. Sixty frames produced **zero dropped
slots**: a renderer that never sampled the buffers would leave every slot to time out on
`RELEASE_TIMEOUT` and the dropped count would climb, so the count is a direct check that the window's
renderer is compositing the dma-bufs and signalling them back.

**Why it is not in `gpui_video`.** The crate is the seam; a demo is not. The foreign path could carry a
`SurfaceProducer` much as `gpui_va` implements one, but that would put a `gpui_video` dependency in an
example, and the sample-repo split keeps `gpui-interop`'s examples free of it. The cube is therefore
self-contained, and a production foreign producer belongs beside the other samples.
