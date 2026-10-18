# Issues

The tracker for work that spans repositories — the things with no single branch to
live on.

An issue that concerns one branch belongs on that branch's tracker. An issue that
concerns the *relationship* between branches, or between a branch and a published
crate, has no home there and lives here.

## Convention

One file per issue, `NNNN-short-title.md`, numbered in order of opening. It opens with
a small header block:

```
- **Opened:** 2026-09-26
- **Status:** open | blocked | closed
- **Touches:** `.meta/spi/…`, `.dist/…`, branch `bite_v1.21.x`
```

then what the problem is, what has been tried, and what would close it.

Closed issues are kept. A tracker that deletes its history is a tracker whose
questions get re-argued.

## Index

| issue | status | subject |
| --- | --- | --- |
| [`0001-check-citations.md`](0001-check-citations.md) | closed | 73 citations carry a line number and nothing verified them; `script/check-citations` now resolves every one against `bite_v1.22.0-pre` |
| [`0002-ci-for-fork-prs.md`](0002-ci-for-fork-prs.md) | open | the CI in `bite-gpui/distribution` never runs on `bite-gpui/bite-gpui` pull requests; a thin caller into `distribution`'s `pr-checks` entry point answers it and is on `bite_v1.22.0-pre`, leaving the ruleset gate and the other branches |
| [`0003-scheduled-builds.md`](0003-scheduled-builds.md) | open | the pull-request gate is deliberately minimal; what it leaves out — `--all-features`, wasm32, clippy, the tests, the packaging checks — needs a scheduled build to run on |
| [`0004-inspector-token-upstream.md`](0004-inspector-token-upstream.md) | open | the stack removes `inspector_id` from `Element` while upstream still threads it, so every sync re-pays the divergence on a public trait; ported to upstream's tree, pushed as `gpui-drop-inspector-token` on `vanuan/zed`, and opened as [zed-industries/zed#64854](https://github.com/zed-industries/zed/pull/64854) |
| [`0005-view-retention.md`](0005-view-retention.md) | open | retained (incremental) view rendering is being built outside the stack — Zed's view tree (`#63800`, split for landing, prerequisites already on `main`) and `gpui-fast#2` (merged) — and is not a seam, so it is the first use the layer rulings cannot answer; adopt, defer or reject, and in which shape |
| [`0006-surface-interop.md`](0006-surface-interop.md) | open | the surface implementation spans an upstream repo and a downstream `gpui-interop` crate — the upstream PR sequence, two out-of-tree arms, the bridge, and the probes that gate them; P7 is now measured (a frame crosses a thread as a surface, `guest-thread-probe.md`), the org indexes carry all eleven repositories, and what remains is the upstream PRs, publication, P1/P2, and the guest runner |
| [`0007-gpui-glob-import-recurses-on-test.md`](0007-gpui-glob-import-recurses-on-test.md) | open | `use gpui::*;` glob-imports the `test` attribute macro, which shadows the builtin `#[test]` and makes `#[gpui::test]` recurse forever; import explicitly instead |
| [`0008-dmabuf-tiled-modifiers-wgpu.md`](0008-dmabuf-tiled-modifiers-wgpu.md) | closed | the Linux arm is linear-only (accepted): a vendor-tiled dma-buf needs `VK_EXT_image_drm_format_modifier` on the renderer's device, and `wgpu` does not enable it — the fix is future `wgpu-hal` work, recorded here |
| [`0009-windows-probe-vm.md`](0009-windows-probe-vm.md) | closed | W5's probes (P5/P6/P9) and the same-device sufficiency test need a real Direct3D device CI has not got; the MUX-less discrete GPU proved not passable, but a GVT-g mediated Intel iGPU gives the guest a real D3D11 FL 11_1 / D3D12 FL 12_1 device — `script/interop-vm` boots it and `script/d3dprobe` verifies it |
| [`0010-application-gpu-preference.md`](0010-application-gpu-preference.md) | closed | adapter selection was a fixed ranking inside the renderer, with `ZED_DEVICE_ID` read deep in `wgpu_context` as the only control; [0008](../decisions/0008-gpu-preference-and-enumeration.md) puts a `GpuPreference` (`Auto`/`LowPower`/`HighPerformance`/`Device`) on `Application`, lowers the environment variable onto `Device` in the platform, and makes `LowPower` the default — implemented and unit-tested, `Custom` deferred to `0012` |
| [`0011-linear-dmabuf-plane-layouts.md`](0011-linear-dmabuf-plane-layouts.md) | closed | on the `LINEAR` path `gpui_wgpu` dropped the plane layouts it built and sampled the driver's own, so a canonical DRM `NV12` buffer was read out of its padding (measured: chroma at 16384 on Intel, 65536 on NVIDIA 580) and rendered green; [0007](../decisions/0007-dmabuf-importer-native-layout-guard.md) refuses a layout the driver did not produce (offsets compared, not pitches) and `GpuDevice::linear_layout` publishes the native layout and required size — both suites pass on Intel and the NVIDIA 930M; the `surfaces.md` §2 / `frame-formats.md` §7 disagreement remains a docs note |
| [`0012-gpu-enumeration-and-negotiation.md`](0012-gpu-enumeration-and-negotiation.md) | closed | nothing could ask what a device can import or export, or change the renderer's GPU before a window exists; [0008](../decisions/0008-gpu-preference-and-enumeration.md) records the process-wide device scope and refuses runtime switching, enumeration landed — `App::enumerate_gpus`/`active_gpu`, `GpuAdapterInfo` mapped from `wgpu`, and `Custom` selecting one adapter — and the capability tier now reports the dma-buf formats and modifiers the renderer's device imports (`App::dmabuf_capabilities`, measured on both GPUs) |
| [`0013-transfer-function-and-srgb-sampling.md`](0013-transfer-function-and-srgb-sampling.md) | closed | the surface path modelled a YUV matrix and range but no **transfer function**; measured: GPUI's targets are non-sRGB `_UNORM` and already hold display-encoded values, so the real gap was a **linear** producer passed through as encoded, and `DmaBufHandle::with_transfer` now declares it (sRGB and video curves pass through; linear is encoded in the shader) |
