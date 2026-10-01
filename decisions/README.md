# Decisions

What was decided, when, on what evidence, and what was rejected.

A decision record earns its place when the *reason* is not visible in the code. The
alternative that was turned down is the part that gets re-litigated otherwise, and the
cost of re-litigating it is paid by whoever did not know it had been considered.

## Convention

- Numbered in order of decision: `NNNN-short-title.md`.
- Each carries a status: **decided**, **in force**, **open**, or **superseded**.
- Unnumbered documents are **evidence** for a numbered record, not decisions.
- Publishing decisions live in `.dist/docs/decisions.md` §13. This directory is for
  architecture and interface decisions.

## Index

| record | status | outcome |
| --- | --- | --- |
| [`0001-no-third-swap.md`](0001-no-third-swap.md) | decided | `FramePipeline` stays an open seam; no third swap, no new version slot |
| [`0002-render-extension-device-model.md`](0002-render-extension-device-model.md) | decided | producer and consumer must be the same device: Path A and Path B are window-owner capabilities, and Windows requires `WgpuRenderer`. Revised 2026-09-28 — a wgpu producer *can* adopt a shareable resource it allocated, so the cross-device bridge is deferred as future work rather than rejected as impossible |
| [`0003-retention-ships-as-a-seam.md`](0003-retention-ships-as-a-seam.md) | decided | the retention seam ships in this stack as a fourth open boundary, with no store installed by default; the retaining modes ship out of tree, so `0001` is unamended |
| [`0004-producer-device-rendezvous.md`](0004-producer-device-rendezvous.md) | decided | how a producer reaches the renderer's device — a window lends its renderer's device, erased on the shared traits (`device_any`) and typed in the facade; built on the canonical ref, and it supplies the gpui→app half of 0002's "the rendezvous is one slot, and it works both ways". It does not take 0002's deferred bridge, which needs a probe first |
| [`0005-external-rendering-unifies-under-surface.md`](0005-external-rendering-unifies-under-surface.md) | decided | external pixels reach GPUI as a `PaintSurface` through the existing `surface()`, not a second scene primitive: the payload becomes a cfg-gated `SurfaceHandle` (CoreVideo / DirectX SRV / dma-buf), cross-API bridging moves downstream to a `gpui-interop` crate, and guest mode is the offscreen contract on a worker thread. Supersedes the `CustomRenderPrimitive::Texture` route of PR #6 by name, retargeting its arms into `draw_surfaces`. The design is [`../spi/rendering/surfaces.md`](../spi/rendering/surfaces.md), the plan and probes [`../spi/rendering/surface-plan.md`](../spi/rendering/surface-plan.md) |
| [`0005-external-rendering-unifies-under-surface.md`](0005-external-rendering-unifies-under-surface.md) | decided | external pixels reach GPUI as a `PaintSurface` through the existing `surface()`, not a second scene primitive: the payload becomes a cfg-gated `SurfaceHandle` (CoreVideo / DirectX SRV / dma-buf), cross-API bridging moves downstream to a `gpui-interop` crate, and guest mode is the offscreen contract on a worker thread. Supersedes the `CustomRenderPrimitive::Texture` route of PR #6 by name, retargeting its arms into `draw_surfaces` |
| [`retention-seam-probe.md`](retention-seam-probe.md) | evidence for 0003 | what the R2 probe traced: every `Window` field is `pub(crate)`, so the seam's published surface is the whole API a mode gets; the seam's design was missing the notified set and a root-list interface, and both resolve additively |
| [`tracy-swap-scope.md`](tracy-swap-scope.md) | evidence for 0001 | what a telemetry swap would have been, and why it stops at the C++ dependency |
| [`windows-path-a-probe.md`](windows-path-a-probe.md) | evidence for 0002 | what the Windows probe answered — no route from a wgpu texture to GPUI's Direct3D 11 renderer — and what it closed. §1's third reading is corrected 2026-09-28 |
| [`shared-surface.md`](shared-surface.md) | evidence for 0002 | the shared OS buffer probe: on Windows, wgpu adoption works end to end — probe 5 renders through wgpu into an app-allocated shared resource and D3D11 reads it back; on macOS, the pool, the token and the fence work. Only macOS adoption is unmeasured |
| [`windows-presentation-probe.md`](windows-presentation-probe.md) | evidence for 0002 | what the presentation probe answered — `WgpuRenderer` can present on a Windows window on Direct3D 12, which `gpui_wgpu` does not enable today, and not with transparency |
| [`macos-presentation-probe.md`](macos-presentation-probe.md) | evidence for [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §10 | what the macOS probe answered — `WgpuRenderer` presents on a view, on Metal, which `gpui_wgpu` does not enable either, and the layer question dissolves because wgpu inserts its own |
| [`macos-wgpu-producer-probe.md`](macos-wgpu-producer-probe.md) | evidence for 0004 and for §2 of [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) | what the producer probe answered — wgpu's adapter on macOS *is* the `MTLDevice` the `MetalRenderer` owns, pointer for pointer, so a wgpu producer needs no device handover; a texture it made was sampled through `MetalTextureExt` byte for byte |
