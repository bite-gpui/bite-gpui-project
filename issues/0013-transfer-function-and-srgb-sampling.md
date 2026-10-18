- **Opened:** 2026-10-17
- **Status:** closed
- **Touches:** `crates/gpui_engine/src/dmabuf.rs`, `crates/gpui_engine/src/surface_format.rs`, `crates/gpui_wgpu/src/dmabuf.rs`, the surface shader, the `gpui-video` `surface` example

# The surface path has no transfer function, so an sRGB producer is sampled as linear

## What is wrong

The colour a `DmaBufHandle` describes is two axes — a matrix and a range — and there is no third:

```rust
pub struct YuvColorSpace {          // crates/gpui_engine/src/dmabuf.rs:149
    pub matrix: YuvMatrix,          // :98
    pub range: YuvRange,            // :110 — Full | Limited
}
```

declared through `with_color_space` (`crates/gpui_engine/src/dmabuf.rs:283`) and carried as
`DmaBufHandle::color_space` (`:202`) — which its own doc calls **a hint**, so a renderer may ignore even
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
(`crates/gpui_wgpu/src/dmabuf.rs:100`) and `R8G8B8A8_UNORM` (`:103`) — and the format vocabulary has no
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

> **Measured answer (2026-10-18).** The premise does not hold for this renderer, and the correction is
> recorded in the update below: GPUI's targets and atlas are **non-sRGB** `_UNORM`, and both the UI and
> the imported-texture path write **display-encoded** values into them. So an sRGB dma-buf is already
> correct today; the gap is the other direction — a **linear** producer was passed through as if
> encoded. The imported-texture path (`fs_imported_texture`) already decodes its `_SRGB` view and
> re-encodes, which is why the same-device route round-trips.

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

## Update, 2026-10-18 — the transfer function lands, and the issue closes

Implemented on `bite_v1.23.x` (`386d6a53d9`):

- `DmaBufHandle` carries `TransferFunction` (`Srgb`, `Linear`, `Bt709`), independent of
  `YuvColorSpace::range` and of the storage-layout dimensions. It is a **handle-level declaration**,
  beside the colour space, not a `SurfaceFormat` field: the same bytes can be sRGB-encoded or linear,
  and putting it in the format would have made every existing `SurfaceFormatKind::bgra8()` ambiguous.
  The default follows the format — `Srgb` for RGB, `Bt709` for YCbCr, whose matrix already produces
  non-linear `R'G'B'` — so no existing producer changes behaviour. `with_transfer` declares the
  exception.
- The surface shader applies `linear_to_srgb` only when the handle declares `Linear`, because the
  render targets are non-sRGB `_UNORM` and everything else written into them is display-encoded. This
  is the measured alternative to the plan's "lower to `*_SRGB` versus `*_UNORM` view formats": an
  `_SRGB` view would linearize bytes the compositor already expects encoded. The declaration is
  carried in `SurfaceParams`; the YCbCr matrix and range expansion are unchanged, so neither is
  applied twice.
- Validation: the existing BGRA row still reads back **byte-identical** (sRGB is the default), and a
  new row declares `Linear` on a mid-grey buffer and asserts the composite is the sRGB encoding of it
  (linear 64 → sRGB 137). Both pass on the Intel HD 520 and the NVIDIA 930M; `gpui_engine` is 26/26
  and `surface_dmabuf` is 10/10 on both.

What remains outside this issue: the exact BT.601/BT.709 OETF distinction (the two curves are the same,
so `Bt709` covers both and video passes through unchanged), and the Windows Direct3D shared-texture
and macOS CoreVideo arms, whose formats have their own sRGB/linear choice. The producer-side
declaration landed in `gpui_interop` (`58be5d867e`): `SharedBuffer::with_transfer` carries
`Linear` (or any other curve) onto the handle a renderer samples.
