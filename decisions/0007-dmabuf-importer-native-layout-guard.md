# 0007 — The dma-buf importer refuses a linear layout the driver itself does not produce

- **Decided:** 2026-10-17
- **Status:** decided — implemented on the working branch `bite_v1.23.x-dmabuf-linear-plane-layouts`, not
  yet on the canonical ref; exercised on both the Intel HD 520 and the NVIDIA 930M.
- **Evidence:** [`../issues/0011-linear-dmabuf-plane-layouts.md`](../issues/0011-linear-dmabuf-plane-layouts.md)
  — the bug this closes, and the layouts it measured;
  [`optimus-dmabuf-topologies.md`](optimus-dmabuf-topologies.md) — what an iGPU/dGPU pair does with a linear
  buffer; and the wedge measured 2026-10-17 on that pair (below).
- **Touches:** [`../issues/0011-linear-dmabuf-plane-layouts.md`](../issues/0011-linear-dmabuf-plane-layouts.md)
  — whose second "what would close it" clause (an explicit refusal) this is;
  [`0008-dmabuf-tiled-modifiers-wgpu.md`](../issues/0008-dmabuf-tiled-modifiers-wgpu.md) — the same
  `VK_EXT_image_drm_format_modifier` capability gate, on the tiled path;
  [`linux-dmabuf-probe.md`](linux-dmabuf-probe.md) — whose "crosses devices in both directions" this does
  not rely on.

## Decision

**A linear handle is imported only if the plane layout it declares is one the driver itself produces.**

On the `LINEAR` path — where `gpui_wgpu` gates the explicit layout it built on `tiled` rather than on
whether the layout can be given (`crates/gpui_wgpu/src/dmabuf.rs:137`, `:149`, `:349`) — the importer now:

1. **Shapes the guard around the driver's own layout.** It builds a plain, **unbound** `LINEAR` image of the
   handle's format and extent and reads back the **plane offsets** the driver gives it. The handle's
   declared offsets must equal those. A driver can sample only a layout it produces itself, so this is the
   comparison that means anything. The *pitch* is deliberately not compared: the driver produces two for one
   format depending on whether the image is exportable (measured: 64 plain, 128 exportable on the 930M, at
   the same offsets), and the producer's memory is laid out at whatever pitch its handle declares.
2. **Checks the buffer can back the image.** A driver may size its image for more memory than the declared
   layout spans, so the image's requirement is compared against the dma-buf's own size (`lseek(SEEK_END)`),
   not the declared span.

Either clause failing **refuses the surface**, naming the shortfall, and the surface is dropped — the
"refuse rather than sample wrong" posture of the module's own contract. The producer's half mirrors it: a
`SharedBuffer` built on the device declares the layout the driver gave *it* and so passes, and
`GpuDevice::linear_layout` publishes the layout the device gives a plain `LINEAR` image (`native` — offsets
and strides) together with the size its image requires (`required`), so an `export_at` producer declares
the device's own layout rather than guessing at it.

### Why a software guard is affordable

Neither clause needs an allocation. In Vulkan `vkCreateImage` allocates no memory until `vkBindImageMemory`,
so a probe image — and the identical image the importer will build — costs no VRAM, no submission, and no
queue work. The whole guard is a create, a query, and a destroy.

### The aspect rule, which is easy to get wrong

A plane of a **plain `LINEAR`** image is named by `PLANE_0`/`PLANE_1` (or the colour aspect when there is
one plane); a plane of a **`DRM_FORMAT_MODIFIER_EXT`** image is named by `MEMORY_PLANE_0_EXT`/`…_1_EXT`.
Querying a modifier image with the plane aspects, or a linear one with the memory-plane aspects, returns
nonsense rather than an error. The guard queries the plain image, so it uses the plane aspects.

### The pitch rule, which is not the offset rule

The guard compares plane **offsets**, not pitches. An offset is the hazard: the part's texturing hardware
maps planes on fixed page boundaries, so a chroma plane at 4096 where the driver wants 65536 wedges it. A
pitch is not: the producer's memory is laid out at whatever pitch its handle declares, and the driver
produces two of them for one format depending on how the image is made. Measured on the 930M, a plain
`LINEAR` `NV12` gets a pitch of 64 while an *exportable* one — the image a `SharedBuffer` exports — gets
128, both with chroma at 65536. Comparing pitches refused `SharedBuffer`'s own output; comparing offsets
accepts both and still refuses the layout that wedges.

## Why

Measured on the pair `0011` measured on — an Intel HD 520 and an NVIDIA GeForce 930M, one laptop:

| | luma pitch | chroma offset | `vkGetImageMemoryRequirements().size` |
| --- | --- | --- | --- |
| Intel, plain `LINEAR` (planar `NV12`) | 64 | 4096 | 6144 |
| NVIDIA 930M, plain `LINEAR` | 64 | **65536** | **131072** |

An exportable `LINEAR` `NV12` on the 930M reports pitch 128 at the same offsets — the reason the guard
compares offsets, not pitches (above).

The two facts that force the decision, both measured on the 930M:

- **The driver accepts a layout it cannot sample, and reports it back.** An image created with
  `ImageDrmFormatModifierExplicitCreateInfoEXT` declaring chroma at 4096 is created and bound without
  error, and `vkGetImageSubresourceLayout` on *that image* returns 4096 — echoing the declaration. So a
  layout check against the imported image can never disagree, and `honoured` is not evidence.
- **Sampling it hangs the command processor.** The renderer's submission never completed; the GPU sat at
  100% until the machine was rebooted. It hung with a buffer sized to the driver's own 131072-byte
  requirement, so the failure is the *layout*, not an out-of-bounds read — which is why the capacity clause
  alone is not enough.

The asymmetry also explains the one that already worked: `gpui_interop`'s `SharedBuffer` declares the layout
`vkGetImageSubresourceLayout` gave it, so on the 930M its declared layout *is* the driver's, the comparison
passes, and the same import path works.

## Rejected

- **Trusting the declaration — "hand the driver an explicit layout and it will honour it."** The 930M
  accepts, echoes, and then wedges the GPU. Vulkan makes `vkCreateImage` a declaration, not a validation.
- **The capacity check alone.** Necessary — an image bound to a dma-buf smaller than it needs samples past
  the buffer — but not sufficient: the hang survived a correctly-sized buffer.
- **Verifying the layout on the *imported* image** (`vkGetImageSubresourceLayout` after an explicit
  create). It echoes the declaration, so it agrees by construction. The comparison has to be against a
  *plain* image the driver laid out itself.
- **Refusing every linear import on a device that pads.** That would drop the producers that work today —
  a `SharedBuffer` on its own device, a VA-API surface, the interop fixture — all of which declare the
  driver's own layout and which this guard accepts.

## Consequences

- **Intel (Mesa):** a canonical `NV12` layout *is* its plain-linear layout, so a canonical producer keeps
  working exactly as before.
- **NVIDIA 930M (and Maxwell-class parts):** a canonical layout is now **refused**, not hung; a producer
  that declares the padded layout the device reports is accepted, and both `SharedBuffer::new` (the layout
  the driver gave it) and `linear_layout`'s `native` (the padded one) composite there — measured on both.
  Path 2 on that part therefore negotiates, rather than asserting.
- **`LinearLayout::honoured` goes.** A field whose value is echoed from the query is worse than absent: it
  reads as verification. The report keeps `declared`, `required` and `backed()`.
- **Tests, one per behaviour:** the layout clause's refusal (`gpui_wgpu`'s and `gpui_va`'s
  `surface_dmabuf` suites — a handle declaring chroma a page past the driver's own is dropped, window
  empty); the capacity clause's refusal (`gpui_wgpu`'s — a buffer too small for its declared layout is
  dropped, window empty); the report (`gpui_interop`'s unit tests — `native`, `declared` and `required`
  checked against an independent measurement on both devices); and the two happy paths, which composite on
  **both** GPUs — consumer-guided (`SharedBuffer::new` + `fill`) and producer-pushed (ask `linear_layout`,
  declare its `native`, size to its `required`). Both suites' producers honour `ZED_DEVICE_ID`, so a pinned
  run keeps export and import on one GPU instead of testing a cross-GPU import by accident.
- **Still out of scope:** a producer that *cannot* be asked for the device's layout — a third party that
  emits only a canonical buffer and offers no negotiation — is refused on a padding device. Making that one
  work needs a copy or convert step, which is the conversion fallback `0012` argues for, not this record.
