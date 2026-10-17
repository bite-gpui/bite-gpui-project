- **Opened:** 2026-10-16
- **Status:** closed
- **Touches:** `bite-gpui/crates/gpui_wgpu/src/dmabuf.rs`, `spi/rendering/surfaces.md` §2, `spi/rendering/frame-formats.md` §7, `gpui-video/crates/gpui_va/examples/surface.rs`, the published `bite-gp-wgpu-1.23.301`

# A linear dma-buf is sampled at the driver's plane layout, not the handle's

## What is wrong

`import_luma_chroma` builds the plane layouts from the handle (`dmabuf.rs:141`) and the
`VkImageDrmFormatModifierExplicitCreateInfoEXT` that carries them (`dmabuf.rs:149`) — then **drops them
when the modifier is linear**:

```rust
let tiled = handle.modifier != DmaBufHandle::LINEAR;                              // dmabuf.rs:137
…
.tiling(if tiled { DRM_FORMAT_MODIFIER_EXT } else { LINEAR })                      // dmabuf.rs:163
let image_info = if tiled { image_info.push_next(&mut modifier_layout) } else { image_info };  // dmabuf.rs:171
```

`import_plane` does the same for a single plane (`dmabuf.rs:377`). So for a **linear** buffer the
`VkImage` carries no explicit layout, the driver supplies its own — `vkGetImageSubresourceLayout` — and
the plane offsets the `DmaBufHandle` declares are ignored. The function's own doc says the opposite
("the buffer is a single object at the two planes' offsets", `dmabuf.rs:120`), which is true of the
*producer's* object and not of the image the driver builds.

(Line numbers are from the published `bite-gp-wgpu-1.23.301`.)

## Measured

A linear `G8_B8R8_2PLANE_420_UNORM` 128×128, `vkGetImageSubresourceLayout` per plane (2026-10-16, one
laptop — an Intel HD 520 and an NVIDIA GeForce 930M):

| driver | plane 0 | plane 1 | image size |
| --- | --- | --- | --- |
| Intel iHD (Mesa 26.0.8) | `offset 0`, row 128 | `offset 16384`, row 128 | 24576 |
| NVIDIA 580.178.04 | `offset 0`, row 128 | `offset 65536`, row 128 | 131072 |

The NVIDIA driver pads luma to 64 KiB. A producer exporting the canonical DRM `NV12` layout — chroma
directly after luma, at 16384 — is then read at 65536, which is padding: chroma is zero, and BT.601 of
zero chroma is a **green** cast. `gpui-video`'s `surface` example reproduced exactly that on the 930M
while rendering correctly on the iGPU, and neither side reported an error — the renderer sampled the
padding and the producer never learned its offset was unused.

The same measurement on the pair, and why linear is the only modifier that crosses vendors, is
[`../decisions/optimus-dmabuf-topologies.md`](../decisions/optimus-dmabuf-topologies.md).

This is not the tiled-buffer corner of 0008: it is the *linear* path, and linear is the only path a
buffer may take to cross devices (0008).

## Why it matters beyond the example

Any producer whose linear plane offsets differ from the consuming driver's default mis-renders
silently. An Intel-exported linear buffer sampled on an NVIDIA renderer is the obvious case; so is a
camera or capture pipeline laying `NV12` out to the DRM convention. `gpui_va` is unaffected today only
because it exports a *tiled* surface, which takes the explicit-layout branch.

## Fix

Pass the explicit plane layouts for `LINEAR` too. `VK_EXT_image_drm_format_modifier` permits
`DRM_FORMAT_MOD_LINEAR` with `VkImageDrmFormatModifierExplicitCreateInfoEXT`, so the code path already
exists — only the `if tiled` guard is wrong. It needs the extension *enabled on the device*, which
`WgpuContext::device_with_surface_import` (`wgpu_context.rs:509`) already arranges where the adapter
supports it (see 0008); where it does not, **refuse the handle** rather than sample it wrong.

## What would close it

Explicit layouts on the linear path, or an explicit refusal when the extension is unavailable; and a
test whose producer's linear offsets differ from the driver's default. The existing tests use canonical
layouts only, which is why this passed.

## Note: the record disagrees with itself about the shape

`spi/rendering/surfaces.md` §2 (`:131`) says the portable shape is "**plane textures** (`R8` + `R8G8`)
with the colour matrix in the shader"; `spi/rendering/frame-formats.md` §7 (`:165`) says what is built
is "one multi-planar image, two plane views". The shipped shape is the one that cannot honour a linear
handle's offsets. One of the two is stale, and reconciling them is part of answering this.

## Update, 2026-10-17 — declaring the layout is necessary but not sufficient

Measured on the NVIDIA 930M: an image created with the declared layout through
`ImageDrmFormatModifierExplicitCreateInfoEXT` is created and bound **without error**, and
`vkGetImageSubresourceLayout` on that image **echoes the declared offsets** (chroma at 4096) — while its
memory requirement stays the driver's own 131072 bytes for a 6144-byte layout. Sampling it **hangs the
command processor**: the renderer's submission never completed and the GPU sat at 100% until the machine
was rebooted. It hung with a buffer sized to the driver's own requirement, so the fault is the *layout*,
not an out-of-bounds read.

So the sound rule is the one this issue's second clause asks for, made concrete: compare the handle's
declared layout against the layout the driver gives a **plain `LINEAR`** image and refuse a mismatch, and
check the image's requirement against the dma-buf's own size. That is
[`../decisions/0007-dmabuf-importer-native-layout-guard.md`](../decisions/0007-dmabuf-importer-native-layout-guard.md),
which also drops the producer report's `honoured` field — a driver echoes an explicit declaration back, so
that query can never disagree with the producer and the field read as verification. The report now publishes
the device's own plane layout (`native`) and required size, so an `export_at` producer declares the padded
layout rather than guesses it; measured on the 930M, a producer that does so composites. The `surfaces.md`
§2 / `frame-formats.md` §7 disagreement above is still open.

## Closed, 2026-10-17 — the importer refuses a layout the driver did not produce

The `LINEAR` path carries the handle's layouts through and guards them
([`../decisions/0007-dmabuf-importer-native-layout-guard.md`](../decisions/0007-dmabuf-importer-native-layout-guard.md)):
the declared plane **offsets** are compared with the ones the driver gives a plain, unbound `LINEAR`
image, and a mismatch refuses the surface; the image's memory requirement is then checked against the
dma-buf's own size. Offsets rather than pitches, because the 930M gives a plain `NV12` a pitch of 64 and
an exportable one 128 at the same offsets. `GpuDevice::linear_layout` publishes that layout (`native`)
and the size to allocate (`required`), so a producer-pushed buffer declares what the device lays out
instead of guessing.

Measured on the pair this issue measured on:

| | native chroma | `required` | canonical declared |
| --- | --- | --- | --- |
| Intel HD 520 | 4096 | 6144 | composites |
| NVIDIA 930M | 65536 | 131072 | **refused, not sampled** |

`gpui_wgpu`'s and `gpui_va`'s `surface_dmabuf` suites pass on both GPUs (8/8 and 14/14), covering the
refusal, the report, and both happy paths — consumer-guided (`SharedBuffer::new` + `fill`) and
producer-pushed (declare `linear_layout`'s `native`, size to `required`). The non-native layout that
hung the command processor is now refused before submission.

The `surfaces.md` §2 / `frame-formats.md` §7 wording disagreement remains a documentation note: the
shipped importer is one multi-planar image with two plane views, which is what `frame-formats.md` §7
describes. Reconciling the §2 wording is docs work, not a blocker on the behaviour this issue tracked.
