# P4: what a video surface actually is

- **Status:** proposed — **not run.** Shapes an **additive** format pass; it does **not** gate W2 or
  W3. It answers the format question [`surfaces.md`](surfaces.md) §7 leaves open.
- **Question:** what do real hardware decoders emit on each platform — and is it ever RGBA, or always
  `NV12`/YCbCr, sometimes 10-bit?
- **Gates:** an additive format pass, after the RGBA arms. **Does not gate W2/W3.**
- **Companion:** [`probe-p3-dmabuf-import.md`](probe-p3-dmabuf-import.md), whose format map is where
  this lands on Linux.

## 1. Why this probe exists

The unified surface fragments are **RGBA**: the straight-through rule
([`foreign-texture.md`](foreign-texture.md) §4) assumes an RGBA `_UNORM` view, and W2 ships as an
RGBA/BGRA pass. But `surface()` is the interface for *video* too — macOS's existing `draw_surfaces` is
a **YCbCr** path (Metal's surface fragment multiplies the sample by a `ycbcrToRGB` matrix, wgpu's
`fs_surface` samples two planes), and a VA-API / Media Foundation / NVDEC decoder emits `NV12`, not
RGBA. So the question is whether the Windows and Linux arms need a YCbCr fragment of their own — an
**additive** pass — or whether the producers they serve are RGBA engines, for which the arms already
suffice.

## 2. What is already known, so the probe does not re-measure it

- **RGBA is enough for an engine.** A render target, a `wgpu` texture, a 3D viewport — the producers
  W5 serves — are RGBA, which is why the RGBA arms are the milestone.
- **A *decoder* is a different producer.** Media Foundation / DXVA emits `DXGI_FORMAT_NV12`; VA-API
  emits `NV12`; VideoToolbox emits `420v`/`420f` (bi-planar YCbCr).
- **macOS is the precedent, not the model.** `PaintSurface` and Metal's `surface_fragment` are a
  macOS-only, video-specific, single-renderer path — not the unified surface path this set adds.
- **10-bit is a second axis.** `P010` and HDR formats exist beside 8-bit `NV12`.

## 3. What it must measure, and where

This is less a GPU probe than an **inventory**: capture what a hardware decoder outputs on each
platform (or read the platform's documented formats), and where such a frame would enter GPUI. The
result is a short per-platform table.

## 4. The probes

1. **Windows.** What Media Foundation / DXVA outputs — is it `NV12`? Is `BGRA` available without a
   shader copy, and at what cost?
2. **Linux.** What VA-API / NVDEC / V4L2 output — `NV12`, and which fourcc? Is RGBA available?
3. **macOS.** Confirm the existing YCbCr path as the precedent, and record its shape (two-plane,
   `ycbcrToRGB`).
4. **The entry point.** Does a decoder's frame arrive through `surface()` at all, or through the
   platform's own video path (macOS's `CVImageBuffer` today)?
5. **10-bit.** Does any real target need `P010`/10-bit, or is 8-bit `NV12` the whole of it?

## 5. What each outcome closes

- **Decoders emit only YCbCr** → the Windows/Linux arms need a YCbCr fragment (two-plane sampling) as
  an additive pass; **RGBA ships first and stays**, and P4 never delays it.
- **The producers we care about are RGBA engines** → no format pass; the RGBA arms are the whole of it.
- **The entry-point answer** → whether a decoder's surface is a `SurfaceSource` variant or the
  platform's existing video path, which decides where the format pass attaches.

## 6. Hazards it must not mistake for an answer

- **The producer kind.** An engine emits RGBA; a decoder emits `NV12`. A probe that samples the wrong
  kind answers nothing about the other.
- **10-bit is easy to overlook** and is a different fragment, not a flag.
- **macOS's YCbCr path is a precedent, not a model.** It is video-specific and drawn by one renderer;
  copying its shape into the unified path would be the wrong generalisation.

## 7. What it produces

A per-platform format table and a decision — a second fragment, a format arm, or out of scope —
recorded beside [`surfaces.md`](surfaces.md) §7. It never blocks W2 or W3; it shapes what comes after
them.
