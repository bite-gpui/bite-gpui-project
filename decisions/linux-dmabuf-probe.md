# Evidence: the Linux dma-buf probe

- **Evidence for** [decision 0005](0005-external-rendering-unifies-under-surface.md)'s Linux arm and a
  correction to [0002](0002-render-extension-device-model.md), which dismissed the Linux bridge as
  *"unnecessary: on Linux both sides are wgpu"*. The use case is a cross-process or cross-API producer
  — a VA-API/NVDEC decoder, a Wayland client, an engine on its own device — handing the Linux renderer
  a dma-buf to sample with no CPU copy.
- **The question:** can a producer that is *not* wgpu allocate a dma-buf and hand GPUI's Linux
  (wgpu/Vulkan) renderer a fd it imports and samples?
- **The probe** is a scratch crate, `bite-gpui`/`probes/linux-dmabuf`, kept beside the other probes.
  Run with `cargo run --manifest-path probes/linux-dmabuf/Cargo.toml` on a Linux host.
- **Status:** the environment, the flat-linear `VkImage` import, the wgpu adoption plus shader sample,
  the `B8G8R8A8` (`ARGB8888`) format map, the tiled `Y_TILED` import, the `NV12` two-plane import with
  its shader-side colour conversion, and the `sync_file` fence are measured on real hardware and pass
  — the probe's gate is cleared. Two producer-side conditions fell out and are now W3's contract: a
  tiled buffer must be *self-describing and uncompressed* under its declared modifier (ANV's implicit
  CCS state is not carried by a single-plane dma-buf), and a two-plane `NV12` buffer is sampled as two
  `R8`/`R8G8` textures with the colour matrix done in the shader, because the native ycbcr conversion
  needs an immutable sampler `wgpu` cannot bind. Cross-device import (Intel ↔ NVIDIA, both
  directions) and the cross-device `sync_file` are measured too, and need a dedicated allocation. Every
  case the probe set out to measure now passes; only other *hardware* is untested
  ([`probe-p3-dmabuf-import.md`](../spi/rendering/probe-p3-dmabuf-import.md)).

## 1. What the source settles

- **wgpu's Vulkan device enables the import extensions.** wgpu-hal pushes `VK_KHR_external_memory_fd`
  and `VK_EXT_external_memory_dma_buf` onto the device extension list when the physical device offers
  them (`wgpu-hal-29.0.4/src/vulkan/adapter.rs:1296`), so the import the renderer needs is not refused
  at wgpu's own boundary. (`VK_KHR_bind_memory2` and `VK_KHR_image_format_list` are core in Vulkan
  1.1/1.2, so they need no name on a 1.4 device.)
- **Adoption is a public seam.** `Device::as_hal`, `vulkan::Device::raw_device`/`shared_instance`,
  `texture_from_raw` and `Device::create_texture_from_hal` are all public, the same adoption path
  [`shared-surface.md`](shared-surface.md) §1 reads on every backend; this probe is the first to run
  it on Linux for a dma-buf.

## 2. The run measured

The harness prints rather than asserts, in stages:

1. the adapters wgpu sees, and the raw Vulkan devices behind them, with the four external-memory
   extensions the import needs;
2. the producer — a raw Vulkan device, not wgpu — allocating a dma-buf, binding a linear `VkImage`
   over it, writing a known colour, and exporting the fd;
3. the raw consumer importing that fd as a second `VkImage` and reading the bytes back;
4. the wgpu consumer importing the fd on its own device, adopting the image
   (`texture_from_raw` + `create_texture_from_hal`), and sampling it through a compute shader;
5. the driver's DRM format modifiers, and a tiled `Y_TILED` round trip;
6. the `NV12` two-plane round trip: the producer exports one fd carrying a luma plane and an
   interleaved chroma plane, the raw consumer reads each plane back through its own `VkImage`, and the
   wgpu consumer adopts the two planes as `R8`/`R8G8` textures and converts them with a shader-side
   BT.709 matrix;
7. the `sync_file` fence: the producer signals a `SYNC_FD` semaphore, the consumer imports the fd and
   waits on it before copying;
8. the cross-device round trip: a dma-buf allocated on one GPU, exported, imported and read on the
   *other*, ordered by a `sync_file` across the device boundary — run in both directions.

## 3. The printout

Run 2026-10-02, on a hybrid laptop (Intel HD 520 integrated + NVIDIA 930M discrete), Mesa 26.0.8,
proprietary NVIDIA 580.178.04, Vulkan 1.4:

```text
=== wgpu adapters: 4 ===
  AdapterInfo { name: "Intel(R) HD Graphics 520 (SKL GT2)", vendor: 32902, device_type: IntegratedGpu, driver: "Intel open-source Mesa driver", backend: Vulkan, .. }
  AdapterInfo { name: "NVIDIA GeForce 930M", vendor: 4318, device_type: DiscreteGpu, driver: "NVIDIA", driver_info: "580.178.04", backend: Vulkan, .. }
  AdapterInfo { name: "llvmpipe (LLVM 21.1.8, 256 bits)", device_type: Cpu, backend: Vulkan, .. }
  AdapterInfo { name: "NVIDIA GeForce 930M/PCIe/SSE2", device_type: Other, backend: Gl, .. }

=== vulkan physical devices: 3 ===
  device: name=Intel(R) HD Graphics 520 (SKL GT2) type=INTEGRATED_GPU api=1.4.335 vendor=0x8086 id=0x1916
      VK_KHR_external_memory_fd: present
      VK_EXT_external_memory_dma_buf: present
      VK_KHR_image_format_list: present
      VK_KHR_bind_memory2: present
  device: name=NVIDIA GeForce 930M type=DISCRETE_GPU api=1.4.312 vendor=0x10de id=0x1346
      VK_KHR_external_memory_fd: present
      VK_EXT_external_memory_dma_buf: present
      VK_KHR_image_format_list: present
      VK_KHR_bind_memory2: present
  device: name=llvmpipe (LLVM 21.1.8, 256 bits) type=CPU api=1.4.335 vendor=0x10005 id=0x0000
      VK_KHR_external_memory_fd: present
      VK_EXT_external_memory_dma_buf: present
      VK_KHR_image_format_list: present
      VK_KHR_bind_memory2: present

=== dma-buf round trip on Intel(R) HD Graphics 520 (SKL GT2) ===
producer: exported fd 43, 1024 bytes, fourcc=ABGR8888 (R8G8B8A8_UNORM), modifier=linear
consumer: imported fd 43 as a 16x16 linear R8G8B8A8_UNORM image
round trip: 1024 bytes match through a VkImage, no CPU copy
consumer: adopted fd 43 into a wgpu texture on "Intel(R) HD Graphics 520 (SKL GT2)"
sample: [0.1254902, 0.7529412, 0.2509804, 1.0] — expected [0.1254902, 0.7529412, 0.2509804, 1.0] — MATCH

=== dma-buf round trip on Intel(R) HD Graphics 520 (SKL GT2) ===
producer: exported fd 43, 1024 bytes, fourcc=ARGB8888 (B8G8R8A8_UNORM), modifier=linear
consumer: imported fd 43 as a 16x16 linear B8G8R8A8_UNORM image
round trip: 1024 bytes match through a VkImage, no CPU copy
consumer: adopted fd 43 into a wgpu texture on "Intel(R) HD Graphics 520 (SKL GT2)"
sample: [0.1254902, 0.7529412, 0.2509804, 1.0] — expected [0.1254902, 0.7529412, 0.2509804, 1.0] — MATCH

modifiers for R8G8B8A8_UNORM:
  DRM_FORMAT_MOD_LINEAR (0x0000000000000000), planes 1, sampled true
  I915_FORMAT_MOD_X_TILED (0x0100000000000001), planes 1, sampled true
  I915_FORMAT_MOD_Y_TILED (0x0100000000000002), planes 1, sampled true
  I915_FORMAT_MOD_Y_TILED_CCS (0x0100000000000004), planes 2, sampled true

=== tiled round trip, I915_FORMAT_MOD_Y_TILED (0x0100000000000002) ===
  VK_EXT_image_compression_control: false
  VK_EXT_image_drm_format_modifier: true
  external dma-buf features: EXPORTABLE | IMPORTABLE, compatible handle types: OPAQUE_FD | DMA_BUF_EXT
producer: modifier 0x100000000000002, plane layout offset=0 size=4096 row_pitch=128
producer: exported fd 43, 16x16 tiled
consumer: imported fd, modifier 0x100000000000002, plane layout offset=0 size=4096 row_pitch=128
  producer self-readback: 1024/1024 bytes match
round trip: tiled VkImage 1024/1024 bytes match the producer's gradient — MATCH

=== NV12 two-plane round trip (32x32) ===
  VK_KHR_sampler_ycbcr_conversion: true (unused — dual-plane shader conversion)
producer: exported fd 43, 32x32 NV12, Y@0 U/V@1024, 1536 bytes
consumer: imported fd 43, read Y 1024/1024 and U/V 512/512 bytes back through VkImages
consumer: adopted fd 43 and fd 44 into wgpu textures on "Intel(R) HD Graphics 520 (SKL GT2)"
sample: NV12 -> RGB, 1024 pixels, 1024/1024 match the BT.709 matrix

=== sync_file fence probe ===
producer: cleared, signaled, exported sync_file fd 43
fence: producer's clear read back after a sync_file wait — MATCH, no tear

=== cross-device dma-buf: Intel(R) HD Graphics 520 (SKL GT2) -> NVIDIA GeForce 930M ===
  producer: linear dma-buf EXPORTABLE | IMPORTABLE, SYNC_FD semaphore EXPORTABLE | IMPORTABLE
  consumer: linear dma-buf EXPORTABLE | IMPORTABLE, SYNC_FD semaphore EXPORTABLE | IMPORTABLE
producer: cleared on Intel(R) HD Graphics 520 (SKL GT2), exported image fd 44 and sync fd 45, 4096 bytes linear
  consumer: imported the image into memory type 0
cross-device: NVIDIA GeForce 930M read the Intel(R) HD Graphics 520 (SKL GT2) dma-buf after the sync_file — 1024/1024 bytes match

=== cross-device dma-buf: NVIDIA GeForce 930M -> Intel(R) HD Graphics 520 (SKL GT2) ===
  producer: linear dma-buf EXPORTABLE | IMPORTABLE, SYNC_FD semaphore EXPORTABLE | IMPORTABLE
  consumer: linear dma-buf EXPORTABLE | IMPORTABLE, SYNC_FD semaphore EXPORTABLE | IMPORTABLE
producer: cleared on NVIDIA GeForce 930M, exported image fd 46 and sync fd 47, 4096 bytes linear
  consumer: imported the image into memory type 0
cross-device: Intel(R) HD Graphics 520 (SKL GT2) read the NVIDIA GeForce 930M dma-buf after the sync_file — 1024/1024 bytes match
```

## 4. The outcome

**The gate is cleared, and the adoption runs.** All three Vulkan devices — the Intel integrated GPU,
the NVIDIA discrete GPU on the proprietary driver, and the llvmpipe software rasteriser — expose all
four external-memory extensions, so the import is refused nowhere. On the Intel device, a raw-Vulkan
producer allocated a dma-buf, bound a linear `VkImage` over it, exported its fd; a consumer imported
it byte-for-byte (no CPU copy); and wgpu adopted that imported image and sampled it through a shader,
reading the known colour back exactly. The same round trip holds for both `R8G8B8A8` and `B8G8R8A8`
(the `ABGR8888` and `ARGB8888` fourccs), so the fourcc→`VkFormat` map is right, and a `sync_file`
fence orders the producer against the consumer with no CPU stall. This corrects 0002's one-line
dismissal: the dma-buf transport is mechanically available *and* the renderer's adoption seam works on
Linux, not just on Windows.

**The tiled case passes too, with one producer-side caveat.** With the producer's surface left
uncompressed, a `Y_TILED` dma-buf round-trips byte-for-byte through a *second* `VkImage` created with
`VkImageDrmFormatModifierExplicitCreateInfoEXT` on the producer's own plane layout (offset 0, size
4096, row pitch 128) — the same contract a foreign producer imposes. The caveat is what the first
attempt hit: ANV enables lossless (CCS) compression by default for a sampled `Y_TILED` `R8G8B8A8`
image, and that compression state is **not** carried by the exported single-plane dma-buf, so a
consumer reading the plane sees the compressed payload as raw bytes. The probe isolates this rather
than guessing: after a CPU write of a uniform sentinel into the shared memory, the producer's own
image reads back *non-uniform* (`cc 66 66 72 …`) while the importer reads it uniform — a transform no
tiling permutation could produce, i.e. decompression state the plane does not describe. Requesting
`VK_IMAGE_USAGE_STORAGE_BIT` on the producer image makes ANV allocate it uncompressed, and the round
trip is then byte-exact. `VK_EXT_image_compression_control`, which would disable the compression
directly, is not advertised by this driver.

**`NV12` passes too, as two plane textures and a shader-side matrix.** The producer exported one fd
carrying a 32×32 luma plane and a 16×16 interleaved chroma plane; a raw consumer imported it and read
both planes back through their own `VkImage`s byte-for-byte (Y 1024/1024, U/V 512/512); and the wgpu
consumer adopted the two planes as `R8Unorm` and `Rg8Unorm` textures and converted them in a compute
shader, matching the same BT.709 matrix computed on the CPU for all 1024 pixels. The conversion is
done in the shader, not with `VK_KHR_sampler_ycbcr_conversion` even though the driver exposes it: the
native conversion requires an immutable sampler baked into the descriptor-set and pipeline layout,
which `wgpu` has no abstraction for, so a consumer built on `wgpu` cannot bind it. Two plane textures
and a matrix is the shape Chromium Ozone, mpv and WebCodecs use, and the one that fits.

**Cross-device is measured too, in both directions.** A dma-buf allocated on the Intel GPU was
imported and read on the NVIDIA GPU, and the reverse, each read byte-for-byte (1024/1024) and each
ordered by a `sync_file` the producer exported and the consumer imported and waited on — not a
`queue_wait_idle`, which would have hidden the cross-device order the probe exists to test. Two things
the measurement added: the import needs a **dedicated allocation** (`VkMemoryDedicatedAllocateInfo`),
without which the discrete GPU refuses it on *every* memory type while still advertising the format as
`IMPORTABLE`; and the two directions are not symmetric in general, so both are run rather than assumed.

**The gate is cleared.** Every case probe 4 requires — the fourcc→`VkFormat` map, a vendor-tiled
modifier, and `NV12` — now passes on real hardware with no CPU copy, and the cross-device and fence
cases beside them. W3 is no longer blocked on the probe, and it carries the producer-side invariants
the probe established: a tiled buffer must be uncompressed and self-describing under its declared
modifier; an `NV12` buffer is consumed as two plane textures with the colour conversion in the shader;
and a dma-buf crossing devices must be a dedicated allocation.

## 5. What is not measured

- **Hardware.** One machine; a pass is *works on this adapter pair* rather than *works everywhere*.
  The tiled import is measured only on ANV, where the compression behaviour above is specific to the
  Intel driver, and the cross-device pair is Intel ↔ NVIDIA, both directions.
- **Other vendors.** The cross-vendor import is measured only between Intel and NVIDIA. AMD, and the
  `llvmpipe` adapter (enumerated but not round-tripped), are untested as either end of a dma-buf.
