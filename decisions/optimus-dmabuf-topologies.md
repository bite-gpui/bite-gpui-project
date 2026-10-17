# Optimus dma-buf topologies: what stays tiled, what must be linear, and which path fails

Evidence for [`0005`](0005-external-rendering-unifies-under-surface.md) and
[`0002`](0002-render-extension-device-model.md), and for issues
[`0011`](../issues/0011-linear-dmabuf-plane-layouts.md) and
[`0012`](../issues/0012-gpu-enumeration-and-negotiation.md). Companion to
[`linux-dmabuf-probe.md`](linux-dmabuf-probe.md), which carries the same-GPU round-trip; this one is
about the *pair* of GPUs an Optimus laptop has.

Read on one machine, 2026-10-16: Intel HD 520 (iGPU, `i915` + Mesa 26.0.8, `/dev/dri/renderD128`) and
NVIDIA GeForce 930M (dGPU, 580.178.04, `/dev/dri/renderD129`), the panel wired to the iGPU. **Measured**
means read on that pair; **asserted** means general driver or vendor behaviour that was not probed here.

## Why it matters: three paths, and one of them is a trap

The dma-buf hand-over happens *before* the OS compositor sees anything, so what decides whether it is
zero-copy is the GPU the producer allocated on against the GPU the renderer samples on — not the panel.

| | producer → renderer | renderer → panel |
| --- | --- | --- |
| **1. iGPU → iGPU** | tiled, in system memory, zero copy | tiled, straight to the display controller |
| **2. dGPU → dGPU** | tiled, in VRAM, zero copy — **asserted** for single-plane RGB | the driver blits the finished window linear across PCIe to the iGPU — **asserted** |
| **3. iGPU → dGPU** | **fails** — **measured** | — |

Path 3 is the one an application reaches by pinning its renderer to the dGPU while its producer runs on
the iGPU. `gpui-video`'s `surface` example walked into it on 2026-10-16; it is the reason a renderer's
GPU has to be *chosen with the producer*, not after (0012), and the linear half of it is 0011.

## Tiling does not cross vendors — asserted

Each vendor's tiling is hardwired in its memory controller and rasteriser: Intel uses
`I915_FORMAT_MOD_X_TILED` / `Y_TILED`, NVIDIA `DRM_FORMAT_MOD_NVIDIA_BLOCK_LINEAR_2D(...)`, AMD
`AMD_FMT_MOD` swizzle modes. Neither can read the other's tiles, so on an Intel + NVIDIA pair the only
modifier both understand is `DRM_FORMAT_MOD_LINEAR` (`0x0`) — which is why `linux-dmabuf-probe.md` can
call linear "the portable choice" and why a buffer that must cross devices has to be linear.

Same-vendor pairs (an AMD APU with an AMD discrete card, say) may share a tiled modifier, by the same
argument. Not probed here.

## Measured: what the two drivers advertise

The four external-memory extensions (`KHR_external_memory_fd`, `EXT_external_memory_dma_buf`,
`KHR_sampler_ycbcr_conversion`, `EXT_image_drm_format_modifier`) are present on both devices, and on
llvmpipe.

DRM format modifiers offered for two-plane `G8_B8R8_2PLANE_420_UNORM` (`NV12`):

| device | modifiers |
| --- | --- |
| Intel HD 520 | `0x0` (LINEAR), `0x0100000000000001` (`X_TILED`), `0x0100000000000002` (`Y_TILED`) |
| NVIDIA 930M | `0x0` only |
| llvmpipe | `0x0` only |

A `SAMPLED` two-plane `NV12` image is importable *and* exportable as a dma-buf on both — LINEAR on both,
and OPTIMAL on NVIDIA only (Intel refuses OPTIMAL with `ERROR_FORMAT_NOT_SUPPORTED`). A single-plane
`R8G8B8A8_UNORM` and a `TRANSFER_SRC|TRANSFER_DST` buffer are importable and exportable on all three.

So the GPU-independent facts an application can rely on are: a **linear** two-plane buffer crosses, and
a **tiled** one is the vendor's own business.

**Why `NV12` shows no tiled modifier on the 930M — asserted.** Vulkan offers two tiling routes:
`VK_IMAGE_TILING_OPTIMAL` (opaque, internal) and `VK_IMAGE_TILING_DRM_FORMAT_MODIFIER_EXT` (an explicit
Linux modifier). The 930M supports OPTIMAL `NV12`, but exposes no multi-planar block-linear layout
through the modifier extension, so the DRM list for `NV12` is linear-only. Whether the same device
*does* offer `BLOCK_LINEAR_2D` for single-plane RGB was **not** measured — it is the assumption path 2
rests on, and the cheapest thing to probe next.

## Measured: a linear plane layout is not the canonical one

`vkGetImageSubresourceLayout` for a linear `G8_B8R8_2PLANE_420_UNORM` 128×128:

| driver | plane 0 | plane 1 | image size |
| --- | --- | --- | --- |
| Intel iHD | `offset 0`, row 128, size 16384 | `offset 16384`, row 128, size 8192 | 24576 |
| NVIDIA 580.178.04 | `offset 0`, row 128, **size 65536** | `offset 65536`, row 128, size 65536 | 131072 |

The NVIDIA driver pads luma to 64 KiB, so its chroma plane does not sit where the DRM `NV12` layout puts
it. `gpui_wgpu`'s importer drops the handle's plane layouts when the modifier is linear
(`crates/gpui_wgpu/src/dmabuf.rs:171`, `crates/gpui_wgpu/src/dmabuf.rs:377`) and samples the driver's
layout instead, so a canonical producer renders green on the dGPU and correctly on the iGPU. This is 0011.

## Measured: cross-device import fails

A producer on the iGPU handing a buffer to a renderer on the dGPU was refused on **every** memory type:

```
dropping a dma-buf surface: no memory type accepted the dma-buf import (last: Some(ERROR_OUT_OF_DEVICE_MEMORY))
```

The importer does try all types the image allows, each with a
`VkMemoryDedicatedAllocateInfo` (`crates/gpui_wgpu/src/dmabuf.rs:476`), so the failure is not a missed
heap check —
`VkExternalMemoryFeatureFlags::IMPORTABLE` was reported for that buffer and the driver still refused it.
Only the same-GPU case imported.

**This contradicts `linux-dmabuf-probe.md`**, whose index entry says "a dma-buf also crosses devices in
both directions, ordered by a cross-device `sync_file` and needing a dedicated allocation". One of the
two is wrong, and the difference is likely the *exporter*: the probe's producer and the `surface`
example's differ in exactly the dedicated allocation the probe names as the requirement — the example's
exporter allocates a bare exportable allocation (`surface.rs`, `Vulkan::allocate`), whereas the probe's
was built to satisfy the import. Reconciling these two is worth a probe before path 3 is called
impossible or path 2 is called sufficient; 0012's "enumerate and negotiate" would answer it from the
application side.

## Measured: the dGPU's driver faults on a host-visible export

Exporting a dma-buf from a `HOST_VISIBLE` allocation on the 930M **segfaults inside
`libnvidia-glcore`** at `vkAllocateMemory` with `VkExportMemoryAllocateInfo`; Mesa exports one happily.
The same call from a `DEVICE_LOCAL` allocation succeeds, and the imported surface then samples. This is
why a producer for this pair must export device-local memory (stage the bytes and copy on the GPU), and
why "the driver advertises `EXPORTABLE`" is not sufficient to try it.

## Asserted, and worth probing

- `BLOCK_LINEAR_2D` for single-plane RGB on the 930M (path 2's premise).
- Same-vendor cross-device tiling (an AMD pair).
- The `vkQueuePresentKHR` resolve across PCIe — that the driver blits only the finished window, and only
  then, is vendor behaviour taken on faith here.

## What this means for the two open issues

0011's fix is the plane layouts on the linear path — which is *the* path every cross-device buffer must
take, so it is not a corner. 0012's default (`GpuPreference::LowPower`, the iGPU renders, a dGPU is a
*producer*) is path 1, with path 2 as the performance option for a producer that owns a viewport. Path 3
is the one to refuse, and today an application can only discover it by trying.
