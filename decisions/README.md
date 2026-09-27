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
| [`0002-render-extension-device-model.md`](0002-render-extension-device-model.md) | decided | producer and consumer must be the same device: Path A and Path B are window-owner capabilities, Windows requires `WgpuRenderer`, and bridging a resource across devices is out of scope |
| [`tracy-swap-scope.md`](tracy-swap-scope.md) | evidence for 0001 | what a telemetry swap would have been, and why it stops at the C++ dependency |
| [`windows-path-a-probe.md`](windows-path-a-probe.md) | evidence for 0002 | what the Windows probe answered — a wgpu texture cannot reach GPUI's Direct3D 11 renderer — and what that closes |
