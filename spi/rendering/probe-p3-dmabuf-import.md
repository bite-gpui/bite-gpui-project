# P3: dma-buf import on Linux

- **Status:** run — the environment, the flat-linear `VkImage` import, the wgpu adoption plus shader
  sample, and the `B8G8R8A8` (`ARGB8888`) format map all pass; the driver enumerates Intel tiled
  modifiers, and a `Y_TILED` producer exports its image, but the consumer's import mismatches on the
  explicit plane layout. `NV12` and fence cases remain. The printout and outcome are
  [`../../decisions/linux-dmabuf-probe.md`](../../decisions/linux-dmabuf-probe.md). It gates the Linux
  surface arm ([`surface-plan.md`](surface-plan.md) W3, [`interop-crate.md`](interop-crate.md) §3).
- **Question:** on Linux, can a producer that is *not* wgpu — a hardware decoder, a Wayland client —
  hand GPUI a **dma-buf** its renderer imports and samples, with no CPU copy?
- **Gates:** W3. **Companion:** [`probe-p2-macos-adoption.md`](probe-p2-macos-adoption.md), the same
  question on macOS.

## 1. Why this probe exists

`0002` dismissed the Linux bridge in one line — *"a dma-buf export … nothing wgpu cannot do for it …
it is unnecessary: on Linux both sides are wgpu, so Tier 1 applies"*
([`../../decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md)
§"What was rejected"). That is true of the *same-device* case, and only that. A cross-process or
cross-API producer — a VA-API/NVDEC decoder, a Wayland client, an engine on its own device — needs the
transport, which is why [`surfaces.md`](surfaces.md) §2 puts dma-buf in the trinity beside `IOSurface`
and the NT handle. **Nothing has been measured on Linux at all**, so this probe starts from zero.

## 2. What is already known, so the probe does not re-measure it

- **The Linux renderer is wgpu, on Vulkan** — so the import target is a `VkImage` over an
  external-memory fd, not an `EGLImage` unless a GL path is chosen.
- **wgpu adopts, and cannot create.** `texture_from_raw` on the Vulkan backend takes a raw `VkImage`
  the application created; wgpu creates nothing shareable.
- **A dma-buf is an fd plus layout** — a DRM fourcc, a modifier, a stride and an offset — not a
  pointer, which is why it is a surface and not a texture
  ([`surfaces.md`](surfaces.md) §2).
- **A flat RGBA buffer is not representative.** Real producers emit `NV12` (multi-planar) with a
  vendor memory *modifier* (tiling), not a linear `ARGB8888`; a probe that only ever imports the flat
  linear case passes while the real path is broken. That is the false positive §4 probe 4 exists to
  close.
- **The Linux rows skip without an adapter**, and so will this: it is a gate only where a GPU or a
  software rasteriser that supports the import exists.

## 3. What it must measure, and where

**Hardware.** A Linux host with a GPU adapter and a DRM node; record whether the adapter is hardware
or software (`llvmpipe`), because a software adapter is not a hardware verdict. For probe 4, a
producer that can emit a tiled buffer or `NV12` — or a synthetic one carrying a vendor modifier.
**Harness.** A scratch crate; the printout is the durable record.

## 4. The probes

1. **Environment first.** Enumerate wgpu adapters; record which is Vulkan, the driver, and whether the
   device enables the external-memory extensions (`VK_KHR_external_memory_fd`,
   `VK_EXT_external_memory_dma_buf`, `VK_KHR_image_format_list`, `VK_KHR_bind_memory2`). *No adapter,
   or none with the extensions, is a condition to record, not a result.*
2. **The producer.** Allocate a dma-buf and export the fd with its fourcc, modifier, stride and
   offset — a linear `ARGB8888`/`XRGB8888` buffer is enough to start. *Prints the fd and layout.*
3. **Import and sample.** Create a `VkImage` from the fd (`VK_KHR_external_memory_fd`), adopt it into
   wgpu with `texture_from_raw` + `create_texture_from_hal`, sample it, read back. *Pass: the known
   bytes.*
4. **The format map, and a real layout.** Repeat for the formats a real producer emits:
   `XRGB8888`/`ARGB8888`, **at least one tiled modifier** (a vendor modifier, not only
   `DRM_FORMAT_MOD_LINEAR`), and `NV12` as the multi-planar case (two planes, two fds or one plus an
   offset). *Pass: each maps to a sampling `VkFormat` and samples correctly.* A run that imports only
   a flat, linear RGBA buffer does **not** mark the Linux path ready.
5. **Synchronisation.** Order the producer against the renderer with a dma-fence (`sync_file`), the
   Linux counterpart of P1's keyed mutex and P2's `MTLSharedEvent`. *Pass: no tear, no stall.*

## 5. What each outcome closes

- **Import works** *including the tiled and `NV12` cases of probe 4* → W3's Linux arm is real work,
  and the correction to `0002` is measured rather than argued. A pass on the flat linear case alone is
  not this outcome.
- **Import is refused, or no adapter has the extensions** → the Linux surface arm is deferred; Linux
  stays same-device (wgpu) only, and [`surfaces.md`](surfaces.md) §2's Linux row is a correction in
  principle but not yet a capability.
- **A software-adapter failure** is inconclusive for hardware — recorded as such, like the WARP caveat
  on Windows.

## 6. Hazards it must not mistake for an answer

- **Modifier and tiling.** A tiled buffer imported as linear (or the reverse) fails or samples
  garbage; the modifier must match, and `DRM_FORMAT_MOD_LINEAR` is the easy case.
- **Multi-planar formats are not one plane.** `NV12` is two; an import that assumes one plane is a
  harness bug.
- **Software adapters may refuse import.** `llvmpipe` is a fine *consumer* but often not an
  external-memory peer.
- **The sandbox.** A runner without a DRM node or an adapter cannot answer this, the way a GPU-less
  runner cannot answer the wgpu rows.

## 7. What it produces

A printout, filed as a `decisions/` evidence record beside
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) and
[`0002`](../../decisions/0002-render-extension-device-model.md), and the
[`surface-plan.md`](surface-plan.md) §2 entry for P3 replaced by a link to it.
