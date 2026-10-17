- **Opened:** 2026-10-17
- **Status:** open
- **Touches:** `crates/gpui_engine/src/dmabuf.rs`, `crates/gpui_engine/src/surface_format.rs`, `crates/gpui_wgpu/src/dmabuf.rs`, the surface shader, the `gpui-video` `surface` example

# The surface path has no transfer function, so an sRGB producer is sampled as linear

## What is wrong

The colour a `DmaBufHandle` describes is two axes — a matrix and a range — and there is no third:

```rust
pub struct YuvColorSpace {          // crates/gpui_engine/src/dmabuf.rs:128
    pub matrix: YuvMatrix,          // :98
    pub range: YuvRange,            // :110 — Full | Limited
}
```

declared through `with_color_space` (`crates/gpui_engine/src/dmabuf.rs:251`) and carried as
`DmaBufHandle::color_space` (`:181`) — which its own doc calls **a hint**, so a renderer may ignore even
the two axes that exist.

**Range and transfer are independent, and only range is modelled.** Range says where structural black and
white sit in the integers (`Full` over `0..255`, `Limited` over `16..235`); the transfer function says how
brightness is distributed *between* those two ends. Every combination occurs:

| producer | range | transfer |
| --- | --- | --- |
| YUV video (an `Nv12` frame off a decoder) | limited | BT.601 / BT.709, non-linear |
| screen or game capture | full | non-linear, BT.709 primaries |
| a UI asset or wallpaper (`Bgra8`) | full | sRGB |

Because they are independent, folding a transfer function into `YuvRange` would be wrong: the range axis
is already correct as it stands, and `Full`/`Limited` must not change.

## Where it bites

The Linux importer maps an imported RGB dma-buf to a **linear** Vulkan format — `B8G8R8A8_UNORM`
(`crates/gpui_wgpu/src/dmabuf.rs:89`) and `R8G8B8A8_UNORM` (`:92`) — and the format vocabulary has no
sRGB field either: `SurfaceFormat` (`crates/gpui_engine/src/surface_format.rs:114`) carries model,
subsampling, layout, orders, depth, container and alpha, and `SurfaceFormatKind::bgra8`
(`crates/gpui_engine/src/surface_format.rs:291`) is "8-bit blue/green/red/alpha, straight alpha — the
common desktop format", with nothing to say whether its bytes are sRGB-encoded.

So a producer that emits sRGB-encoded bytes has no way to say so, and the consumer has no way to ask. The
same-device route pulls the other way — the imported-texture handle is built from an sRGB wgpu texture —
so the two routes need not agree about what an 8-bit RGB byte means.

**Asserted, not measured.** Whether the surface shader compensates for the linear sampler has not been
traced. What the types do show is that the question cannot be *asked*: there is no transfer-function field.
Whoever works this should read the shader path and replace this paragraph with the measured answer.

## Why it matters beyond the example

An imported sRGB buffer sampled as linear loses contrast in its shadows and midtones (a gamma mismatch, not
a gamut one: sRGB and BT.709 share primaries). The failure is silent, like `0011`'s, and it lands on the
producers the interop crate exists for — a captured screen, an embedded engine's UI pass, a wallpaper or
asset pipeline — rather than on video, which is already limited-range and non-linear and which the matrix
handles.

## What would close it

- A **transfer-function** dimension, independent of `YuvRange` (`crates/gpui_engine/src/dmabuf.rs:110`) and
  of the storage-layout dimensions `SurfaceFormat` already carries — linear, sRGB, and the BT.709/BT.601
  curves, which share sRGB's primaries.
- A declared sRGB/linear choice lowered to `*_SRGB` versus `*_UNORM` where the importer builds its view
  format, and the same choice carried on the other platform arms rather than left to each.
- The surface shader read, and its decode — the YCbCr matrix and any range expansion — reconciled with
  whatever the sampler now does, so the two are not both applied or neither.
- Tests over the combinations, at the level the existing colour tests work at: `gpui-video`'s `gpui_va`
  suite already has limited-range and full-range cases to extend, and its `surface` example has a tab per
  payload to widen.
