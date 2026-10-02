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
- **Status:** the environment, the flat-linear `VkImage` import, and the wgpu adoption plus shader
  sample are measured on real hardware and pass. The tiled-modifier, `NV12` and fence cases of
  [`probe-p3-dmabuf-import.md`](../spi/rendering/probe-p3-dmabuf-import.md) §4.4–§4.5 are not yet
  measured, so the Linux arm is *mechanically possible* rather than *ready*.

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

The harness prints rather than asserts, in four stages:

1. the adapters wgpu sees, and the raw Vulkan devices behind them, with the four external-memory
   extensions the import needs;
2. the producer — a raw Vulkan device, not wgpu — allocating a dma-buf, binding a linear `VkImage`
   over it, writing a known colour, and exporting the fd;
3. the raw consumer importing that fd as a second `VkImage` and reading the bytes back;
4. the wgpu consumer importing the fd on its own device, adopting the image
   (`texture_from_raw` + `create_texture_from_hal`), and sampling it through a compute shader.

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
```

## 4. The outcome

**The gate is cleared, and the adoption runs.** All three Vulkan devices — the Intel integrated GPU,
the NVIDIA discrete GPU on the proprietary driver, and the llvmpipe software rasteriser — expose all
four external-memory extensions, so the import is refused nowhere. On the Intel device, a raw-Vulkan
producer allocated a dma-buf, bound a linear `VkImage` over it, exported its fd; a consumer imported
it byte-for-byte (no CPU copy); and wgpu adopted that imported image and sampled it through a shader,
reading the known colour back exactly. This corrects 0002's one-line dismissal: the dma-buf transport
is mechanically available *and* the renderer's adoption seam works on Linux, not just on Windows.

**It is not "ready".** The pass is the flat, `DRM_FORMAT_MOD_LINEAR` `R8G8B8A8_UNORM` case. A real
producer emits `NV12` with a vendor tiling modifier — neither of which this run measures (§5) — so W3
stays gated on those before the Linux arm is built.

## 5. What is not measured

- **The format map and a real layout.** Only `R8G8B8A8_UNORM` linear is measured. `XRGB8888`, a
  tiled (vendor) modifier, and `NV12` two-plane are the cases
  [`probe-p3-dmabuf-import.md`](../spi/rendering/probe-p3-dmabuf-import.md) §4.4 requires before the
  path counts.
- **Synchronisation.** No dma-fence (`sync_file`) orders the producer against the consumer — the
  Linux counterpart of the keyed mutex and the `MTLSharedEvent` (§4.5).
- **Cross-device.** Export and import run on one device (the integrated GPU). Intel producer →
  NVIDIA consumer is the hybrid case dma-buf exists for, and is not yet run.
- **Hardware.** One machine; the NVIDIA and llvmpipe rows are enumerated but not round-tripped, so a
  pass is *works on this adapter* rather than *works everywhere*.
