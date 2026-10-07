# Frame formats: the comprehensive video vocabulary

- **Status:** proposed — the vocabulary, not yet the implementation. What the Linux arm builds today is
  the 8-bit RGBA / `NV12` family plus a declared colour space; the rest is specified so the vocabulary
  is *total* before the code is. The point is that the list below is finite and can be closed.
- **Gates:** nothing. It answers the format question
  [`probe-p4-video-formats.md`](probe-p4-video-formats.md) opens and shapes the additive format pass.
- **Companion:** [`surface-seam.md`](surface-seam.md) §2, the rule this vocabulary must satisfy;
  [`probe-p3-dmabuf-import.md`](probe-p3-dmabuf-import.md), the Linux import measurement it extends.

## 1. Why a vocabulary, and why it is curated

A video frame is not one thing. It is a **shape** — how many planes, how the chroma is subsampled and
ordered — a **sample depth and its container** (8, or 10 bits inside 16), a **colour space** (matrix,
range, and for HDR primaries and transfer), a **chroma siting**, and a **layout** (the DRM modifier).
Those are the *dimensions*; a *format* is one point in the space of them.

The tempting design is to model the dimensions orthogonally and let a producer name any point. It is
wrong here for the reason [`surface-seam.md`](surface-seam.md) gives: not every point is realisable
(there is no semi-planar 4:1:1), and a renderer handed an arbitrary combination would have to
re-validate it and map it to a native format that may not exist. So the engine carries a **curated
enum of the formats that occur**, chosen *from* those dimensions so its completeness can be audited,
and maps 1:1 to a native format per platform. The dimensions are the *design*; the enum is the
*interface*.

## 2. The dimensions

| dimension | values |
| --- | --- |
| colour model | `YCbCr`, `RGB` |
| chroma subsampling | `4:0:0` (mono), `4:1:1`, `4:2:0`, `4:2:2`, `4:4:4` |
| plane layout | `planar` (separate Y/Cb/Cr), `semi-planar` (Y + interleaved CbCr), `packed` (interleaved luma+chroma in one plane) |
| channel order | `CbCr`, `CrCb` (only meaningful for semi-planar and packed) |
| sample depth | `8`, `10`, `12`, `16` |
| container | `U8` (one byte), `U16` (a 16-bit word; a narrower sample in its **high** bits), `U32Packed` |
| range | `Full`, `Limited` |
| matrix | `BT.601`, `BT.709`, `BT.2020-NCL` |
| primaries / transfer | `BT.709`/`sRGB`, `BT.2020`/`PQ`, `BT.2020`/`HLG` — **specified, not implemented**; only HDR needs them |
| chroma siting | `co-sited`, `left`, `top-left` — **specified, not implemented** |
| alpha | `none`, `straight`, `premultiplied` (RGB only) |
| layout / tiling | the DRM modifier (a vendor tiling or compression code) |

The "shape" shorthand used below reads `4:2:0 semi, CbCr, 8` for a half-resolution interleaved chroma
plane at 8 bits, `planar` for separate Cb/Cr planes, and `packed` for one interleaved plane.

## 3. The formats video needs

The DRM column is representative — libdrm's `drm_fourcc.h` is the authority, and it is not installed
where this was written. The VA column comes from `/usr/include/va/va.h`. The Vulkan column is the
`VkFormat` the importer would create a `VkImage` with (multi-planar names are one image; `planar` rows
are sampled as N single-channel views of one image, as this branch already does for `NV12`).
`—` means the platform has no format for it — real, and part of why a curated set is right.

### 8-bit YCbCr — the decode default

| format | shape | DRM | VA | Vulkan | Apple | DXGI |
| --- | --- | --- | --- | --- | --- | --- |
| `Nv12` | 4:2:0 semi, CbCr, 8 | `NV12` | `NV12` | `G8_B8R8_2PLANE_420_UNORM` | `420v`/`420f` | `NV12` |
| `Nv21` | 4:2:0 semi, CrCb, 8 | `NV21` | `NV21` | `G8_B8R8_2PLANE_420_UNORM` (chroma view swapped) | — | — |
| `I420` | 4:2:0 planar, 8 | `YU12` | `I420` | 3× `R8_UNORM` views | `y420` | — |
| `Yv12` | 4:2:0 planar, CrCb, 8 | `YV12` | `YV12` | 3× `R8_UNORM` views | — | — |
| `Nv16` | 4:2:2 semi, 8 | `NV16` | — | `G8_B8R8_2PLANE_422_UNORM` | — | — |
| `Yu16` | 4:2:2 planar, 8 | `YU16` | `422H` | 3× `R8_UNORM` views | — | — |
| `Yuyv` | 4:2:2 packed, 8 | `YUYV` | `YUY2` | one plane, decoded in the shader | `yuvs` | `YUY2` |
| `Uyvy` | 4:2:2 packed, 8 | `UYVY` | `UYVY` | one plane, decoded in the shader | `2vuy` | — |
| `Nv24` | 4:4:4 semi, 8 | `NV24` | — | `G8_B8R8_2PLANE_444_UNORM` | — | — |
| `Yuv444` | 4:4:4 planar, 8 | `YU24` | `444P` | 3× `R8_UNORM` views | — | — |
| `Yuv411` | 4:1:1 packed, 8 | — | `411P` | one plane, decoded in the shader | — | — |
| `Mono` | 4:0:0, 8 | `R8` | `Y800` / `Y8` | `R8_UNORM` | `OneComponent8` | `R8_UNORM` |

### 10 / 12 / 16-bit YCbCr — HDR, Main10, AV1, mezzanine

| format | shape | DRM | VA | Vulkan | Apple | DXGI |
| --- | --- | --- | --- | --- | --- | --- |
| `P010` | 4:2:0 semi, 10-in-16 MSB | `P010` | `P010` | `G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16` | `x420`/`xf20` | `P010` |
| `P012` | 4:2:0 semi, 12-in-16 MSB | `P012` | `P012` | `G12X4_B12X4R12X4_2PLANE_420_UNORM_3PACK16` | — | — |
| `P016` | 4:2:0 semi, 16 | `P016` | `P016` | `G16_B16R16_2PLANE_420_UNORM` | — | `P016` |
| `I010` | 4:2:0 planar, 10-in-16 MSB | — | `I010` | 3× `R16_UNORM` views | — | — |
| `P210` | 4:2:2 semi, 10-in-16 MSB | `P210` | — | `G10X6_B10X6R10X6_2PLANE_422_UNORM_3PACK16` | — | — |
| `P410` | 4:4:4 semi, 10-in-16 MSB | `P410` | `Q416` | `G10X6_B10X6R10X6_2PLANE_444_UNORM_3PACK16` | — | — |
| `Q410` | 4:4:4 planar, 10-in-16 MSB | `Q410` | — | 3× `R16_UNORM` views | — | — |
| `Y210` | 4:2:2 packed, 10-in-16 MSB | — | `Y210` | one plane, decoded in the shader | `v210` | `Y210` |
| `Y216` | 4:2:2 packed, 16 | — | `Y216` | one plane, decoded in the shader | — | `Y216` |
| `Y410` | 4:4:4 packed, 10-in-32 | — | `Y410` | one plane, decoded in the shader | — | `Y410` |
| `Y416` | 4:4:4 packed, 16-in-32 | — | `Y416` | one plane, decoded in the shader | — | `Y416` |
| `Nv15` | 4:2:0 tiled, 10-in-16 MSB (vendor) | `NV15` | — | — | — | — |

### RGB — engines, UI, screenshots, HDR targets

| format | shape | DRM | VA | Vulkan | Apple | DXGI |
| --- | --- | --- | --- | --- | --- | --- |
| `Bgra8` | 8:8:8:8 | `AR24`/`XR24` | `BGRA` | `B8G8R8A8_UNORM` | `32BGRA` | `B8G8R8A8_UNORM` |
| `Rgba8` | 8:8:8:8 | `AB24`/`XB24` | `RGBA` | `R8G8B8A8_UNORM` | — | `R8G8B8A8_UNORM` |
| `Rgb10a2` | 10:10:10:2 | — | `A2R10G10B10` | `A2R10G10B10_UNORM_PACK32` | — | `R10G10B10A2_UNORM` |
| `Rgb565` | 5:6:5 | `RGB16` | `RGB565` | `B5G6R5_UNORM_PACK16` | — | `B5G6R5_UNORM` |
| `Rgba16F` | 16:16:16:16 half float | — | — | `R16G16B16A16_SFLOAT` | — | `R16G16B16A16_FLOAT` |
| `Rgba64` | 16:16:16:16 | — | `ARGB64` | `R16G16B16A16_UNORM` | — | — |
| `RgbPlanar` | planar 8 | — | `RGBP`/`BGRP` | 3× `R8_UNORM` views | — | — |

## 4. The decoder subset — what actually matters

The whole space is above; the subset a *decoder* hands a renderer today is small, and the format pass
should be built in this order:

1. **8-bit 4:2:0** — `NV12` (the universal default) and `I420`/`YV12`. **Built.**
2. **10-bit 4:2:0** — `P010`/`P016`: HEVC Main10, VP9 profile 2, AV1. The first real extension, and the
   one HDR needs.
3. **4:2:2 / 4:4:4 at 10-bit** — `P210`/`P410`, and packed `Y210`/`Y410`: professional and mezzanine
   workflows, not end-user playback.
4. **Mono** — a rare decode output; `R8` is already the sampling shape.

RGB is not a decoder output; it is the *engine* producer, and it is done (`Bgra8`/`Rgba8`). `NV21`,
`NV16`, `NV24`, `YUYV` and the 4:1:1 legacy formats are encode/legacy paths — real, curated above, and
lower priority than any of the four.

## 5. The engine model

The engine holds two types. [`SurfaceFormat`] is the *dimensions* — one private field per dimension in
§2, with public readers — and it is built nowhere but its own module, so a `SurfaceFormat` always
describes a shape that exists. [`SurfaceFormatKind`] is an **enum that chooses among pre-constructed
`SurfaceFormat`s**: each variant names one format of §3 and carries the shape it is, and the named
constructors on it (`nv12()`, `p010()`, …) are the only ways a producer makes one.

```text
SurfaceFormatKind   // the choice: an enum, one variant per format of §3, each carrying a SurfaceFormat
  └ SurfaceFormat   // the dimensions: model · subsampling · layout · order · depth · container · alpha
                      // (private fields and private constructors; public readers, plane_count, is_yuv)
YuvColorSpace       // { matrix, range } — present today; primaries/transfer when HDR lands
ChromaReconstruction // the producer's hint — present today
modifier            // the DRM tiling code — present today
ChromaSiting        // co-sited | left | top-left — specified, not implemented
```

The enum is the choice because this vocabulary is closed: a format it does not contain cannot be named,
and a renderer that matches on it gets the compiler's help. The struct is the dimensions because they
are what a renderer *reads* — a `VkFormat` is `rgb_order` at `depth`, and nothing about it needs a name.

A `planar` format is many views of one image (as `NV12` is two views of one image today); a `packed`
format is one plane decoded in the fragment, because no GPU samples it natively. Neither is a new
engine *concept* — both are already in the importer.

## 6. One vocabulary, three mapping tables

The dimensions in §2 are the same on every platform; only the names differ, and the engine holds the
neutral enum. Each backend owns one mapping:

- **Linux** — dma-buf: §3's DRM fourcc + modifier + per-plane stride/offset; the importer maps to a
  `VkFormat`.
- **macOS** — `IOSurface`/`CVPixelBuffer`: §3's `CVPixelFormatType` (`420v`, `x420`, …); the Metal
  arm maps to an `MTLPixelFormat`.
- **Windows** — DXGI texture: §3's `DXGI_FORMAT` (`NV12`, `P010`, `YUY2`, …); the Direct3D arm maps to
  a view.

A producer on a given platform meets that platform's handle and that platform's names. The *concepts*
— subsampling, depth, range, matrix, tiling, fence — are shared, because they are the questions a GPU
asks of any buffer. So "does every platform need its own implementation?" is **yes for the handle and
the native mapping, no for the vocabulary**: the same enum, three tables.

## 7. What is built, and what is specified

| part | state |
| --- | --- |
| `Bgra8`, `Rgba8` | built (single-plane, sampled straight through) |
| `Nv12` | built (one multi-planar image, two plane views, matrix in the shader) |
| colour space: matrix + range | built (declared by the producer, honoured by the renderer) |
| chroma reconstruction hint | built |
| the rest of the planar/semi-planar YCbCr set above | **curated and named, dropped by the importer** — the vocabulary is total before the code |
| packed YCbCr (`YuYv`, `Y210`) and sub-byte packed RGB (`Rgb565`, `A2R10G10B10`) | **curated in §3 but not modelled** — neither fits the dimensions honestly yet (see below), so neither is a `SurfaceFormatKind` |

## 8. Hazards this must not mistake for detail

- **Bit depth is not a flag.** 10-bit is a different sample width and, for packed formats, a different
  fragment — the same warning [`probe-p4-video-formats.md`](probe-p4-video-formats.md) §6 makes.
- **MSB vs LSB alignment.** `P010` puts its 10 bits in the *top* of a 16-bit sample; reading it as
  `R16` and scaling by `1/65535` instead of `1/1023` is a silent, plausible-looking error.
- **Tiling is not format.** A `P010` buffer with a vendor modifier is neither `P010`+modifier as two
  independent facts nor a different format: the modifier is a *layout* decorator, and the fourcc is
  the *format*. Conflating them is how a vendor-tiled variant (`NV15`) becomes a mystery.
- **Packed formats are decoded, not sampled.** No GPU samples `YUYV` natively; it is one plane and a
  fragment that unpacks it, which is a new fragment — not a new entry in a table. And their component
  *interleave* is a dimension §2 does not have, which is why they are named here but not yet modelled.
- **Legacy is still real.** 4:1:1 (`DV`) and `YV12` exist in the wild; they are curated, and they are
  not free.
- **Siting is usually unrecorded.** Most streams name no chroma siting, so "co-sited" is a default the
  producer applies — one more unknown resolved behind the seam, per
  [`surface-seam.md`](surface-seam.md) §2.
