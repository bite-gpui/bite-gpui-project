# The producer seam: `SurfaceProducer`, and where the video shapes stay

- **Status:** settled — written down against `bite_v1.23.1-pre-interop`, where the video stack
  became three crates (`gpui_va`, `gpui_video_platform`, `gpui_video`, now in the `gpui_video`
  repository) and the player left `crates/gpui/examples/player.rs`. It names a boundary the crate
  split keeps re-meeting; it is not a new mechanism.
- **Companion:** [`surface-seam.md`](surface-seam.md), the vocabulary rule and its two axes;
  [`interop-crate.md`](interop-crate.md), the downstream transport crate, which is **not** a seam;
  [`player.md`](player.md), the producing end this seam pulls from.

## 1. Three producer-side things, and which is which

| thing | what it is | where it lives |
| --- | --- | --- |
| the payload — `SurfaceSource`, `SurfaceFormat`, `DmaBufHandle`, the fences | the *closed and total* description a renderer samples | `gpui_engine`, going upstream |
| **`SurfaceProducer`** | the internal **seam**: one method, `surface()` | `gpui_video_platform` |
| **`gpui-interop`** | the **transport** for a foreign device — match the adapter, move a handle, order the queues | a downstream crate |

The three are felt as one because they are all "producer side", and one of them even carries the
word *interop*. They are different axes: the first is *what crosses* (the vocabulary,
[`surface-seam.md`](surface-seam.md)), the second is *what a producer is* (the pull an element
makes), the third is *how* a surface crosses a device or a process boundary.

## 2. The rule

A producer is one method, and its smallness is the point:

```rust
/// Something outside GPUI that produces surfaces for GPUI to composite.
pub trait SurfaceProducer {
    /// The surface to composite now, or `None` when the producer has nothing ready.
    fn surface(&mut self) -> Option<SurfaceSource>;
}
```

It is deliberately **the surface seam and not a video one** — the crate's own words. A video
producer adds a *cadence* (a frame every period) and a *pool* (recycle a surface only once the
renderer released it); a 3D producer adds neither. What they share, and all this carries, is a
surface. `None` is not a failure: a decoder waiting on its input and a producer between frames both
have nothing this frame, and a caller that already has a surface keeps showing it.

The *video* shapes — cadence, pool, seek, the stream clock, buffering — are the producer's and stay
below the seam, never in the payload ([`surface-seam.md`](surface-seam.md) §2). `gpui_va`'s
`Playback` is where they live: a bounded ring of live surfaces, a release descriptor per hand-off,
and the invariant [`player.md`](player.md) §2 spells out — *a frame taken and never composited is a
frame never released.*

## 3. Why it is not in `gpui-interop`

- **That crate is not a seam.** [`interop-crate.md`](interop-crate.md) records it twice — "it
  replaces no `SceneRenderer` and decorates no `FramePipeline`", and §5 "No seam, no pipeline. It is
  reached by the application, not installed by it."
- **It is heavier.** `gpui-interop` names the facade (`gpui`) and, with its `wgpu` feature, `wgpu`,
  because `attach` takes the device a `gpu_canvas` callback lends. The seam is engine-only; putting
  it here would drag the facade and a graphics API into every producer.
- **Most producers are not interop at all.** A same-device producer — a texture on the window's own
  device — is a valid `SurfaceProducer` and needs no transport. The seam must not imply one.

## 4. Why it is not an SPI

[`../README.md`](../README.md) defines an SPI as a trait a layer implements *in order to be
replaced*, usually installed through a single bootstrap method on `Application`, and warns that
publishing a seam is a permanent commitment. `SurfaceProducer` is neither installed on `Application`
nor a swap: it is an internal trait a producer implements so an element can pull a surface, and an
application never names it. It is not one of the five seams.

## 5. How a producer gets a dma-buf, and why the two cases differ

On Linux the payload is a dma-buf, and there are two ways to have one — which is why the export
helper is not `gpui_va`'s alone:

- **A VA-API producer exports directly.** `gpui_va` calls `vaExportSurfaceHandle`, and the VA driver
  hands back the fd, the modifier (the decoded frame's `I915_FORMAT_MOD_Y_TILED`) and the plane
  strides — the same object its own graphics driver holds — so it builds a `DmaBufHandle` and hands
  it straight to `surface()`. No `VK_KHR_external_memory_fd` appears on that path; the driver did
  that work internally. The one place `gpui_va` reaches for external memory is *import*: filling a
  tiled surface for its `nv12` stand-in and its tests, via `DMA_BUF_EXT`.
- **A general producer must export its own.** A `wgpu` or 3D producer, or another process, has no
  driver-level export. It allocates a shareable image and exports the fd itself — via
  `VK_KHR_external_memory_fd` or `EGL_LINUX_DMA_BUF_EXT` — and hands over the same `DmaBufHandle`,
  ordered by a dma-fence. That allocation-and-export, and the fence, is `gpui-interop`'s `linux`
  module ([`interop-crate.md`](interop-crate.md) §3).

The rule that decides the home is the surface rule again: the *producer* resolves what only it
knows. VA-API knows its own export; a generic producer does not, so the export helper is the
interop crate's, and `gpui_va` keeps its `vaExportSurfaceHandle`.

## 6. The generic-or-video question, and the test

The crate that holds this seam is named `gpui_video_platform`, but the seam it holds is generic.
Two honest ways out, and the smell test settles which name fits:

- **Rename it `gpui_surface`.** The trait *is* the generic surface seam; the name should say so.
- **Keep the name and earn it.** Put the *video* producer shape here — cadence, pool, seek, clock —
  above the generic seam, so `gpui_video_platform` means "the video producer's seam". Then a
  `gpui_va` implements both, and a bare 3D producer implements only the generic one.

[`surface-seam.md`](surface-seam.md) §3 is the test either way: **if a name mentions a codec or a
container, it is on the wrong side of the seam.** A trait named `SurfaceProducer` is generic; a
trait named `VideoProducer`, or a type named `DecodedVideoFrame`, is the seam leaking. So the
*generic* trait stays generic wherever it lives, and only a deliberately video-named trait may carry
the video shapes.

## 7. What would reopen it

- **A same-device producer that needs a cadence.** Today cadence is the video producer's alone. If a
  second kind of producer needs one, the seam grows a second method rather than the element growing
  a producer-shaped case.
- **A published crate implementing `SurfaceProducer`.** Then it is a seam by the SPI definition, and
  §4's reading has to be revisited with the publishing commitment [`../README.md`](../README.md)
  describes.
- **`gpui-interop` gaining a same-device helper.** [`interop-crate.md`](interop-crate.md) §5 rules
  it out — the same-device case needs no handle. If that changes, the boundary in §3 moves with it.
