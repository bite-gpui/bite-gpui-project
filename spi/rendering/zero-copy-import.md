# Zero-copy import: the producer, the buffer, and the guard

- **Status:** built and measured on `bite_v1.23.x`. The **transfer-function** axis is open
  ([`0013`](../../issues/0013-transfer-function-and-srgb-sampling.md)); everything else below is
  implemented and exercised on an Intel HD 520 and an NVIDIA 930M.
- **Companions:** [`surfaces.md`](surfaces.md) — the unified path and the OS trinity;
  [`interop-linux.md`](interop-linux.md) — the Linux producer half and its Vulkan specifics;
  [`frame-formats.md`](frame-formats.md) — the semantic vocabulary this chapter assumes;
  [`producer-seam.md`](producer-seam.md) — the internal `SurfaceProducer` trait.
- **Evidence:** [`../../decisions/0007-dmabuf-importer-native-layout-guard.md`](../../decisions/0007-dmabuf-importer-native-layout-guard.md)
  — the guard, its measurements and the rejected alternatives;
  [`../../issues/0011-linear-dmabuf-plane-layouts.md`](../../issues/0011-linear-dmabuf-plane-layouts.md)
  — the bug it closed; [`../../decisions/optimus-dmabuf-topologies.md`](../../decisions/optimus-dmabuf-topologies.md)
  — what the iGPU/dGPU pair does, including the cross-device refusal and the host-visible export
  fault.

## 1. What zero-copy has to guarantee

A frame produced outside the window renderer reaches the scene through `surface()`. Zero-copy means
only a handle and a fence cross: the producer writes into memory the window renderer can sample, and
nothing is downloaded or re-uploaded. [`surfaces.md`](surfaces.md) §1–§2 owns that path. This chapter
owns what has to be true *about the buffer* for the last step — the fragment shader's read — to be
correct and safe.

Three boundaries are load-bearing, and each has failed somewhere in this stack:

1. **Same physical device.** A dma-buf names memory allocated on one GPU. The consumer's driver must
   be able to map and sample it; a producer on another device is refused rather than shared
   ([`optimus-dmabuf-topologies.md`](../../decisions/optimus-dmabuf-topologies.md) §"cross-device
   import fails"). The cross-API bridge is a different mechanism, [`../../decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md).
2. **Semantic and physical are separate.** What a pixel *means* is not where its bytes *live*.
   Conflating them yields a buffer that samples without error and decodes the wrong colour.
3. **A layout the driver did not produce is refused before submission.** `vkCreateImage` with an
   explicit layout is a declaration, not a validation; a driver can accept it, echo it back, and then
   wedge when the texture unit samples it.

## 2. The semantic format and the physical instance

The engine keeps the two apart, and each has exactly one consumer:

| | **semantic format** | **physical instance** |
| --- | --- | --- |
| what it answers | what does a texel mean? | where is a plane's first row, and how long is it? |
| dimensions | colour model, subsampling, plane topology, component order, sample depth and container, alpha, **range**, **matrix**, **transfer** | width, height, DRM modifier, per-plane `{ offset, stride }`, the fd, acquire/release fences |
| carried by | `SurfaceFormat` / `SurfaceFormatKind` ([`frame-formats.md`](frame-formats.md) §5) | `DmaBufHandle` / the interop crate's frame descriptor |
| consumed by | the fragment shader and the pipeline | the driver and the texture mapping unit (TMU) |
| never sees | a byte offset or a row pitch | a colour model or a matrix |

The TMU does the byte arithmetic — offset, pitch, and tile swizzle — in silicon. The shader receives
normalized texels from the plane views and applies the semantic decode. A change in one axis must not
leak into the other: a padded luma plane is a physical fact, a limited-range BT.709 frame is a
semantic one, and neither implies the other.

The producer resolves every semantic unknown **before** the buffer crosses, which is
[`surface-seam.md`](surface-seam.md) §2's rule; the importer never guesses a colour model from a
`VkFormat`.

## 3. The colour pipeline

Three axes, and only two are built. They are orthogonal, so none is folded into another.

### Range — built

Range says which integers are structural black and white:

- **Limited** (broadcast swing): luma `16..=235`, chroma `16..=240` in 8-bit.
- **Full** (PC swing): `0..=255` across every channel.

For an 8-bit limited texture sampled as normalized floats, the shader expands before the matrix:

```text
Y  = (Y_raw  - 16/255)  / (219/255)
Cb = (Cb_raw - 128/255) * (255/224)
Cr = (Cr_raw - 128/255) * (255/224)
```

Sampling limited as full washes blacks grey; sampling full as limited crushes shadows and clips
highlights. The `gpui_va` suite has both cases (`a_limited_range_surface_converts_through_the_shader`,
`a_limited_surface_declared_as_full_range_is_not_the_bars`).

### Matrix — built as a declared axis

`YuvColorSpace { matrix, range }` (`crates/gpui_engine/src/dmabuf.rs:128`, fields at `:98` and `:110`)
is carried on the handle and declared with `with_color_space` (`:251`). The luma weights:

| matrix | `Y'` |
| --- | --- |
| BT.601 (SD) | `0.299 R' + 0.587 G' + 0.114 B'` |
| BT.709 (HD, sRGB primaries) | `0.2126 R' + 0.7152 G' + 0.0722 B'` |
| BT.2020 (UHD / WCG) | `0.2627 R' + 0.6780 G' + 0.0593 B'` |

A producer that omits the metadata applies the broadcast convention: limited range, `BT.601` below
576 lines and `BT.709` at 576 and above. The shader path the `gpui_va` fixture exercises is BT.601 —
its `ycbcr_to_rgb` inverse defines the expected pixel. Selecting BT.709/BT.2020 *per handle* is not
covered by a test here; the type carries the axis, the coverage does not yet.

### Transfer — not built

The transfer function says how brightness is distributed **between** the two ends range fixes. It is
independent of range, and every combination occurs: limited BT.709 video, full BT.709 screen capture,
full sRGB UI assets.

The importer maps an imported RGB dma-buf to a **linear** format — `B8G8R8A8_UNORM` and
`R8G8B8A8_UNORM` (`crates/gpui_wgpu/src/dmabuf.rs:89`, `:92`) — and `SurfaceFormat` has no field that
could say otherwise (`crates/gpui_engine/src/surface_format.rs:114`; `SurfaceFormatKind::bgra8` at
`:291`). So an sRGB producer is sampled as linear, losing contrast in shadows and midtones; sRGB and
BT.709 share primaries, so this is a gamma mismatch, not a gamut one. [`0013`](../../issues/0013-transfer-function-and-srgb-sampling.md)
is the fix: a transfer dimension independent of range, lowered to `*_SRGB` versus `*_UNORM`, with the
shader reconciled so the curve is applied once.

## 4. The three sharing contracts

Every hand-over is one of three shapes. What separates them is **who chooses the plane layout**, and
therefore what the consumer must trust.

| | who chooses the layout | API | typical producer |
| --- | --- | --- | --- |
| **1. driver-chosen** | the driver | `SharedBuffer::new` (+ `clear` / `fill`) | an engine on the renderer's device |
| **2. un-negotiated push** | a third party | `DmaBufHandle::new` | VA-API, a camera, PipeWire |
| **3. negotiated push** | the driver, reported to the producer | `GpuDevice::linear_layout` + `SharedBuffer::export_at` | a CPU-staged or canonical producer |

### Contract 1 — the driver chooses

`SharedBuffer::new` creates a `vk::Image` in device-local memory, asks the driver for each plane's
layout with `vkGetImageSubresourceLayout`, and exports that as the handle. Because both sides are the
same driver on the same physical device, the layout rules agree by construction. `Tiling::Tiled` is
sound only on the renderer's **own** device (`Adapter::gpu_device`); a device the producer opened
itself exports linear with a warning, because its modifier is not known to be importable.

### Contract 2 — a third party pushes

Fixed-function hardware allocates its own pool and exports the fd, modifier and plane table. The
consumer cannot choose the layout and does not get to ask for one; it validates the handle against the
guard (§5) and either imports it or drops the frame. This is the path `vaExportSurfaceHandle` and a
capture pipeline take.

### Contract 3 — the driver answers first

`GpuDevice::linear_layout(format, width, height, proposed_planes)` builds the image the importer would
and reports:

- **`native`** — the plane layout the device gives a plain, unbound `LINEAR` image. This is what the
  producer must **declare**; a driver samples only a layout it produced itself.
- **`required`** — the bytes that layout's image needs. This is what the buffer must **hold**.
- **`declared`** / **`backed()`** — how far the producer's own proposal is from the device's, and
  whether its span alone backs the image.

`SharedBuffer::export_at` then allocates a `vk::Buffer` with the declared offsets and strides and
uploads the content. It always allocates **device-local** memory and stages the bytes through a
host-visible buffer with a `vkCmdCopyBuffer`: on the 930M, exporting a `HOST_VISIBLE` allocation with
`VkExportMemoryAllocateInfo` segfaults inside `libnvidia-glcore` at `vkAllocateMemory`
([`optimus-dmabuf-topologies.md`](../../decisions/optimus-dmabuf-topologies.md) §"the dGPU's driver
faults"), while `DEVICE_LOCAL` exports and samples. A producer never asserts a layout it was not given
in contracts 1 and 3; contract 2 is the only assertion, and it is what the guard exists for.

## 5. The guard

### Why an explicit layout is not a capability check

Passing the handle's plane table through `VkImageDrmFormatModifierExplicitCreateInfoEXT` tells the
driver where the bytes are. Per the specification it is a **declaration**: `vkCreateImage` assumes the
caller got those offsets from a compatible allocator. Calling `vkGetImageSubresourceLayout` on the
resulting image **echoes the declaration back** — it cannot reveal whether the texture unit can sample
it. Measured on the 930M: a canonical 64×64 `NV12` declaration (`chroma = 4096`, `size = 6144`) is
created and bound without error, reports `offset = 4096`, and **hangs the command processor** the
moment the shader samples plane 1. The GPU sat at 100% until the machine was rebooted.

### The two clauses

Before importing a **linear** handle, the importer runs, with no memory allocation and no submission:

1. **Offsets are the driver's own.** It creates a plain, unbound `LINEAR` image of the handle's format
   and extent, reads each plane's layout, and requires **`declared.offset == native.offset`**.
2. **The buffer can back the image.** It requires
   `bind_offset + vkGetImageMemoryRequirements().size <= lseek(fd, SEEK_END)`.

Either clause failing refuses the surface, naming the shortfall, and the frame is dropped — the
"refuse rather than sample wrong" posture. A driver-chosen layout (contract 1) and a negotiated one
(contract 3) pass the first clause by construction; a canonical or padded push passes only when it is
the device's own.

### Pitches are deliberately not compared

The first clause compares **offsets only**. A pitch is not a hazard in the same way: the producer's
memory is laid out at whatever pitch its handle declares, and the driver produces **two** pitches for
one format depending on how the image is made. Measured on the 930M, a plain `LINEAR` `NV12` gets a
pitch of 64 while an *exportable* one gets 128, at the same offsets. Comparing pitches refused
`SharedBuffer`'s own output; comparing offsets accepts both and still refuses the 4096 layout.

### What it costs

Nothing in VRAM. `vkCreateImage` allocates no memory until `vkBindImageMemory`, so the probe is a
create, a query per plane, and a destroy. The capacity clause is `lseek`. No command buffer is
recorded and no queue is touched.

### The measured case (64×64, the fixtures' size)

| | canonical | Intel HD 520 | NVIDIA 930M |
| --- | --- | --- | --- |
| plane 0 | `0`, pitch 64 | `0`, pitch 64 | `0`, pitch 64 |
| plane 1 | `4096`, pitch 64 | `4096`, pitch 64 | **`65536`**, pitch 64 |
| image needs | 6,144 B | 6,144 B | **131,072 B** |
| canonical push | — | accepted | **refused, not sampled** |
| negotiated push | — | accepted | **accepted and composites** |

The 128×128 case in [`optimus-dmabuf-topologies.md`](../../decisions/optimus-dmabuf-topologies.md)
matches: chroma at 16384 on Intel and 65536 on NVIDIA, image sizes 24576 and 131072.

### What the guard does not cover

- **Tiled handles.** A vendor modifier's layout is the modifier's, and there is no plain image to
  compare it with. That path is gated on `VK_EXT_image_drm_format_modifier` instead
  ([`../../issues/0008-dmabuf-tiled-modifiers-wgpu.md`](../../issues/0008-dmabuf-tiled-modifiers-wgpu.md)).
- **A producer that cannot be asked.** A third party emitting only a canonical layout and offering no
  negotiation is refused on a padding device. Making *that* one work needs a copy or convert step,
  the fallback [`0012`](../../issues/0012-gpu-enumeration-and-negotiation.md) argues for.

## 6. Multi-GPU topology

The hand-over happens before the compositor sees anything, so what decides zero-copy is the GPU the
producer allocated on against the GPU the renderer samples on — not the panel. The measurements and
the paths are [`optimus-dmabuf-topologies.md`](../../decisions/optimus-dmabuf-topologies.md); the
rules this chapter depends on:

- **Same GPU, or no import.** An iGPU-exported dma-buf is refused on the dGPU on every memory type.
  Producers obtain the renderer's device through `gpui_interop::attach` inside `gpu_canvas`
  (`Adapter::gpu_device`), so export and import land on one card. Headless fixtures honour
  `ZED_DEVICE_ID` for the producer as well as the renderer, so a pinned run does not accidentally
  test a cross-GPU import.
- **Tiled inside a device, linear across one.** A tiled modifier is opaque to another vendor, so a
  buffer that must cross devices is linear; linear is also the only `NV12` modifier the 930M exposes.
  That it is the *presentation* step, and not the surface, which resolves the window linearly across
  PCIe is **asserted**, not measured.
- **A decoder-less dGPU.** A `GM108`/`GP108` part has no NVDEC block: VA-API initialises but reports
  zero profiles, and `gpui_va` refuses the frame. Decode must run on the compositor's GPU; pinning the
  renderer to such a dGPU leaves no hardware path.

## 7. Invariants

These are the things that go wrong silently if a change forgets them:

- **Never sample a layout the driver did not produce.** "It created the image" is not evidence.
- **Offsets, not pitches.** The pitch is the producer's memory; the offset is the page mapping.
- **The capacity clause is separate.** A correctly-aligned layout bound to a short buffer samples
  past its end.
- **The guard allocates nothing.** Any change that binds the probe image, or submits work to ask, has
  changed its cost and its failure mode.
- **The semantic axes are independent.** Do not fold transfer into range, range into the matrix, or
  either into the DRM modifier.
- **Contract 1 and 3 never assert a layout.** They ask. Only contract 2 asserts, and that is why §5
  exists.

## 8. What is built, and what is open

| part | state |
| --- | --- |
| contracts 1–3 | built; both the consumer-guided and the negotiated path composite on the Intel HD 520 and the NVIDIA 930M |
| the zero-VRAM guard (offsets + capacity) | built; the layout clause's refusal and the capacity clause's refusal are each tested |
| `GpuDevice::linear_layout` (`native`, `required`, `declared`, `backed()`) | built; checked against an independent measurement on both devices |
| device-local + staging for an exported push | built; the host-visible export fault is why |
| range and matrix | built as declared axes; range is tested both ways, per-handle matrix selection is not |
| **transfer function (sRGB vs UNORM)** | **open** — [`0013`](../../issues/0013-transfer-function-and-srgb-sampling.md) |
| GPU enumeration and negotiation before a window exists | **open** — [`0012`](../../issues/0012-gpu-enumeration-and-negotiation.md) |
| application-facing GPU preference | **open** — [`0010`](../../issues/0010-application-gpu-preference.md) |
| conversion fallback for an un-negotiable third party | **open** — `0012` |
