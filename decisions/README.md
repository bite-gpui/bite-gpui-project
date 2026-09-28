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
| [`tracy-swap-scope.md`](tracy-swap-scope.md) | evidence for 0001 | what a telemetry swap would have been, and why it stops at the C++ dependency |
| [`windows-path-a-probe.md`](windows-path-a-probe.md) | evidence for 0002 | what the Windows probe answered — no route from a wgpu texture to GPUI's Direct3D 11 renderer — and what it closed. §1's third reading is corrected 2026-09-28 |
| [`shared-surface.md`](shared-surface.md) | evidence for 0002 | the shared OS buffer probe: on Windows, wgpu adoption works end to end — probe 5 renders through wgpu into an app-allocated shared resource and D3D11 reads it back; on macOS, the pool, the token and the fence work. Only macOS adoption is unmeasured |
| [`windows-presentation-probe.md`](windows-presentation-probe.md) | evidence for 0002 | what the presentation probe answered — `WgpuRenderer` can present on a Windows window on Direct3D 12, which `gpui_wgpu` does not enable today, and not with transparency |
| [`macos-presentation-probe.md`](macos-presentation-probe.md) | evidence for [`../spi/rendering/renderer-seam.md`](../spi/rendering/renderer-seam.md) §10 | what the macOS probe answered — `WgpuRenderer` presents on a view, on Metal, which `gpui_wgpu` does not enable either, and the layer question dissolves because wgpu inserts its own |
