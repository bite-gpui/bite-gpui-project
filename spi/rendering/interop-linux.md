# The Linux hatch: an exported dma-buf, ordered by a dma-fence

- **Status:** built — `gpui_interop`'s `linux` module exports a shareable `BGRA8`/`RGBA8` or two-plane
  `NV12` image, linear or on a driver-tiled modifier the renderer can import, and a `Fence` that hands
  over the acquire `sync_file`; Linux unit tests allocate each on a real device. §6's adapter step
  landed too: `Adapter::gpu_device` makes a second logical device on the renderer's own
  `vk::PhysicalDevice`, so an export lands on the GPU the window draws on rather than whichever node
  `GpuDevice::open` finds. `crates/gpui_interop/examples/interop_linux.rs` composes the whole path into
  a window, and `crates/gpui_interop/tests/linux_dmabuf_readback.rs` (behind the `readback-test`
  feature) composites a **linear** and a **tiled** buffer through the renderer and reads both back byte
  for byte — §9 is what that turned up. P3
  ([`probe-p3-dmabuf-import.md`](probe-p3-dmabuf-import.md)) is the consumer-side import it pairs with.
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
    /// The engine's description, ready for `surface()`.
    pub fn handle(&self) -> &DmaBufHandle;
    /// The producer's own render target.
    pub fn image(&self) -> &vk::Image;
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
- the image is created with `VkExternalMemoryImageCreateInfo` (`DMA_BUF_BIT_EXT`) and, for a tiled
  format, a `VkImageDrmFormatModifierExplicitCreateInfoEXT` naming the modifier;
- memory is allocated with `VkExportMemoryAllocateInfo` and the fd taken with `vkGetMemoryFdKHR`;
- the modifier and plane layout come back from `vkGetImageDrmFormatModifierPropertiesEXT` (or the
  explicit layout that was asked for), and must agree with the modifier and strides on the handle;
- the acquire fence is exported with `VK_EXTERNAL_FENCE_HANDLE_TYPE_SYNC_FD_BIT` (or the semaphore
  equivalent) — a `sync_file` is what the engine's `acquire_fence` takes.

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

## 7. What it does not do

- **No consumer half.** Import, the modifier-aware sampling and the wait are core, as on Windows.
- **No readback.** The CPU case is `read_pixels` ([`interop-crate.md`](interop-crate.md) §5).
- **No VA-API export.** That is the producer's (`vaExportSurfaceHandle`), §3.

## 8. What would reopen it

- **`wgpu` gaining shareable creation.** Then §5 collapses from a handshake to a descriptor, and the
  module shrinks the way [`interop-crate.md`](interop-crate.md) §7 says the Windows one would.
- **A cross-vendor modifier that is not importable.** The hatch must then fall back to a linear buffer,
  which costs the colour it was tiling for — `gpui_va` already warns on the linear case, and the hatch
  needs the same warning.
- **P3 failing.** Then there is no Linux arm to give this to, and the module stays a stub.

## 9. The read-back test, and the two things it caught

The read-back test (`crates/gpui_interop/tests/linux_dmabuf_readback.rs`) composites a linear and a
tiled buffer through the headless `wgpu` renderer and reads each pixel back. Getting both arms green
caught two faults, neither in the renderer.

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

Both arms now pass on both devices, with no environment pinning: `LINEAR` everywhere, `Y_TILED` on
Intel and `0x0300000004fe015` on NVIDIA.

The corollary is worth stating plainly, because it is the sharp edge of §6: **`Tiling::Tiled` is only
sound on a device the consumer shares.** `Adapter::gpu_device` gives that by construction;
`GpuDevice::open` — the no-renderer fallback — does not, and a producer that opens its own device and
asks for `Tiled` can export a modifier the renderer will refuse. Handing it the renderer's device is
the whole point of the adapter step, and the failure it prevents is silent until the frame is sampled.
