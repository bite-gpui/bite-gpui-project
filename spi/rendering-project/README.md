# The rendering project

**Status: the import milestone is complete.** This project was *getting foreign pixels into GPUI*:
the renderer seam (`SceneRenderer` behind `PlatformRenderer`), the unified `PaintSurface` /
`surface()` path — Path A — the producer's device reach, and the `GpuCanvas` authoring surface. That
set is built and is the top of this folder. What is not built is under [`todo/`](todo/): a few gaps
in the path itself (GPU selection, the transfer function), and the two directions it leaves —
carrying events across a surface, and rendering GPUI *into* a foreign renderer — which are separate
efforts, sketched below.

Everything the render extension needs is at one path, as symlinks. The documents live where their
subject lives — the chapters in [`../rendering/`](../rendering/README.md), the authoring surface in
[`../authoring/`](../authoring/gpu-canvas.md), the decisions and their evidence in
[`decisions/`](../../decisions/README.md), the layer and frame views in
[`architecture/`](../../architecture/README.md) — and this folder shows the whole set at once.

Nothing here is a document. Every entry is a symlink, so opening one opens the original and editing
one edits the original.

The subfolders mirror the sources — `rendering/`, `authoring/`, `decisions/`, `architecture/`,
`issues/` — at the top for the built set, and again under `todo/` for the work not done, so a path
here reads as the path there with a prefix and `ls -R` answers "what do I have to read" without
prose.

## Current architecture (built)

| here | is |
| --- | --- |
| [`rendering/README.md`](../rendering/README.md) | the chapter order, the trade-off, and the reconciliation |
| [`rendering/renderer-seam.md`](../rendering/renderer-seam.md) | the seam: `PlatformRenderer`, the typed target, the factory, recovery, and the implementation order |
| [`rendering/foreign-texture.md`](../rendering/foreign-texture.md) | Path A — importing a texture produced outside GPUI (built) |
| [`rendering/surfaces.md`](../rendering/surfaces.md) | the design of record for external pixels — the unification under `PaintSurface`/`surface()`, and host vs. guest modes |
| [`rendering/surface-plan.md`](../rendering/surface-plan.md) | the implementation plan for `surfaces.md`, and the probes it is gated on |
| [`rendering/surface-seam.md`](../rendering/surface-seam.md) | the settled rule: the engine owns a *total* pixel description, and a producer resolves every video-domain unknown |
| [`rendering/zero-copy-import.md`](../rendering/zero-copy-import.md) | the importer as a whole — the semantic/physical split, the zero-VRAM layout guard, the colour pipeline including the transfer-function axis, and the multi-GPU rules |
| [`rendering/producer-reach.md`](../rendering/producer-reach.md) | the other half of Path A — how a producer gets the device — what each platform lends now, and what is left |
| [`rendering/verification.md`](../rendering/verification.md) | the matrix, and the platform each check needs |
| [`rendering/milestones.md`](../rendering/milestones.md) | the handoff: what is built, what is next in what order, and the gates a resumer runs |
| [`rendering/upstream/README.md`](../rendering/upstream/README.md) | the upstreaming plan — the sequence, the PRs and their order, and the commits behind each |
| [`rendering/upstream/pr-vocabulary-and-spi.md`](../rendering/upstream/pr-vocabulary-and-spi.md) | the body for PR 1 — the renderer seam, the surface vocabulary and `GpuCanvas` |
| [`rendering/upstream/pr-windows-paint-surface.md`](../rendering/upstream/pr-windows-paint-surface.md) | the body for PR 3 — `paint_surface` on Windows |
| [`rendering/upstream/pr-offscreen.md`](../rendering/upstream/pr-offscreen.md) | the body for PR 6 — the offscreen render/readback split |
| [`authoring/gpu-canvas.md`](../authoring/gpu-canvas.md) | the authoring surface an application meets (built) |
| [`decisions/0002-render-extension-device-model.md`](../../decisions/0002-render-extension-device-model.md) | decision 0002 — which devices a producer may use, and which it may not |
| [`decisions/0004-producer-device-rendezvous.md`](../../decisions/0004-producer-device-rendezvous.md) | decision 0004 — how a producer reaches the device: the window lends it (superseded by 0006) |
| [`decisions/0005-external-rendering-unifies-under-surface.md`](../../decisions/0005-external-rendering-unifies-under-surface.md) | decision 0005 — external pixels unify under the existing `PaintSurface`, not a second primitive |
| [`decisions/0006-renderer-owned-device.md`](../../decisions/0006-renderer-owned-device.md) | decision 0006 — the renderer owns the device and the canvas lends it; supersedes 0004 |
| [`decisions/0007-dmabuf-importer-native-layout-guard.md`](../../decisions/0007-dmabuf-importer-native-layout-guard.md) | decision 0007 — a linear dma-buf is imported only at a layout the driver itself produces; closes 0011 |
| [`decisions/0008-gpu-preference-and-enumeration.md`](../../decisions/0008-gpu-preference-and-enumeration.md) | decision 0008 — the GPU is process-wide, chosen once from an application `GpuPreference`; steps 1–3 implemented (the preference, enumeration and the dma-buf import capability) |
| [`decisions/0009-gpu-preference-platform-backends.md`](../../decisions/0009-gpu-preference-platform-backends.md) | decision 0009 (draft) — how the preference and enumeration reach Windows and macOS: a two-part `GpuId`, DXGI/DXCore and `MTLCopyAllDevices`, and a per-backend default; steps 1–3 in force, neither backend run on hardware yet |
| [`decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) | the first measurement 0002 rests on |
| [`decisions/shared-surface.md`](../../decisions/shared-surface.md) | the shared-buffer probe that reopens 0002 — Windows adoption and the macOS pool, token and fence measured; macOS adoption open |
| [`decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md) | the measurement 0002's Windows clause rests on |
| [`decisions/macos-presentation-probe.md`](../../decisions/macos-presentation-probe.md) | the measurement renderer-seam §6's macOS commit and §10 rest on |
| [`decisions/macos-wgpu-producer-probe.md`](../../decisions/macos-wgpu-producer-probe.md) | the measurement that answers Path A's macOS producer route — wgpu's adapter *is* the `MetalRenderer`'s device |
| [`decisions/linux-dmabuf-probe.md`](../../decisions/linux-dmabuf-probe.md) | the measurement the Linux dma-buf arm rests on (P3) |
| [`decisions/optimus-dmabuf-topologies.md`](../../decisions/optimus-dmabuf-topologies.md) | the measurement of an iGPU/dGPU pair — what stays tiled, what must be linear, and which path fails |
| [`decisions/windows-interop-manifest.md`](../../decisions/windows-interop-manifest.md) | the interop row that died before `main` for want of an application manifest |
| [`architecture/frame-flow.md`](../../architecture/frame-flow.md) | where the seam is entered in a frame |
| [`architecture/layer-stack.md`](../../architecture/layer-stack.md) | the ruling the extension has to clear |
| [`issues/0008-dmabuf-tiled-modifiers-wgpu.md`](../../issues/0008-dmabuf-tiled-modifiers-wgpu.md) | closed — the Linux arm is linear-only (accepted); tiled modifiers are future `wgpu-hal` work |
| [`issues/0009-windows-probe-vm.md`](../../issues/0009-windows-probe-vm.md) | closed — the probe VM gives the Windows devices CI has not got |
| [`issues/0011-linear-dmabuf-plane-layouts.md`](../../issues/0011-linear-dmabuf-plane-layouts.md) | closed — a linear dma-buf was sampled at the driver's plane layout, not the handle's |
| [`issues/0012-gpu-enumeration-and-negotiation.md`](../../issues/0012-gpu-enumeration-and-negotiation.md) | closed — the preference and enumeration landed, and the capability tier reports the dma-buf formats and modifiers the renderer's device imports |
| [`issues/0013-transfer-function-and-srgb-sampling.md`](../../issues/0013-transfer-function-and-srgb-sampling.md) | closed — the surface path declares a transfer function: sRGB and the video curves pass through, and linear light is encoded for the non-sRGB target |

The GPU selection work and the transfer function are settled and built (`0008`, `0012`, `0013`),
with `0009` carrying the Windows and macOS halves.

## Follow-on work — in its own repositories

The built path runs one way: a producer's pixels *into* GPUI. The directions it leaves are their own
projects, in their own repositories — **`gpui_interop`** (the bridge and input crate) and
**`gpui_video`** (the video stack and the player) — and their chapters and probes are under
[`todo/`](todo/).

### 1. Events across a surface (export / import)

The gateway that lets a producer meet GPUI's events without depending on `gpui`. Its first slice is
built in the `gpui_interop` repository (`src/input.rs`, `gpui_interop::input`); scroll, key and
pinch, the guest platform, and the pointer lock a 3D camera wants are not.

| here | is |
| --- | --- |
| [`rendering/surface-input.md`](../rendering/surface-input.md) | the gateway, the engine's drag that carries the gesture, and pass-through (built in part) |
| [`rendering/surfaces.md`](../rendering/surfaces.md) §4 | host and guest modes, the context the gateway sits in |
| [`input/pointer-lock.md`](../input/pointer-lock.md) | the relative pointer a 3D *camera* wants — parked, and below the seam the gateway sits above |
| [`input/input-policy-seam.md`](../input/input-policy-seam.md) | the parked ingress-*policy* seam |

### 2. Rendering GPUI into a foreign renderer (guest mode)

The other direction: GPUI as a headless renderer hosted inside a foreign loop, its frames and its
device exported. The offscreen contract is built on the canonical ref; the crate and the video
stack are in the `gpui_interop` and `gpui_video` repositories (`guest.rs`, `adapter.rs`; the
`gpui_va`/`gpui_video` crates).

| here | is |
| --- | --- |
| [`rendering/interop-crate.md`](../rendering/interop-crate.md) | the downstream `gpui-interop` crate — Host Mode built, Guest Mode not |
| [`rendering/interop-scaffold.md`](../rendering/interop-scaffold.md) | the crate's concrete scaffold — layout, API and tests |
| [`rendering/producer-seam.md`](../rendering/producer-seam.md) | the internal `SurfaceProducer` seam, and where the video shapes stay |
| [`rendering/player.md`](../rendering/player.md) | the runnable player end of the surface path — built in the fork, unmerged |
| [`rendering/frame-formats.md`](../rendering/frame-formats.md) | the video frame-format vocabulary |
| [`rendering/interop-linux.md`](../rendering/interop-linux.md) | the Linux hatch: the producer half of a shared dma-buf |
| [`rendering/probe-p1-reverse-bridge.md`](../rendering/probe-p1-reverse-bridge.md) | the probe that gates the reverse route |
| [`rendering/probe-p2-macos-adoption.md`](../rendering/probe-p2-macos-adoption.md) | the probe that gates the macOS module |
| [`rendering/probe-p4-video-formats.md`](../rendering/probe-p4-video-formats.md) | the inventory behind the additive YCbCr pass |
| [`rendering/probe-p7-guest-thread.md`](../rendering/probe-p7-guest-thread.md) | ran — the frame crosses a thread as a shareable surface; the runner itself is not built |

### 3. Path B — drawing into the window's own pass

Independent of both, and unstarted: the application's own commands drawn into the window's pass
rather than composited as a buffer.

| here | is |
| --- | --- |
| [`rendering/inline-commands.md`](../rendering/inline-commands.md) | Path B — drawing into the window's own pass |

### The coordination issue

| here | is |
| --- | --- |
| [`issues/0006-surface-interop.md`](../../issues/0006-surface-interop.md) | the surface work across repositories — the upstream PR sequence, two out-of-tree arms, the crate, and the probes (open) |

## Why symlinks, and why folders

A copy would be a second place for a fact to go stale, and this repository's rule is that a fact
specified once is not restated — so nothing here is copied. Folders rather than a flat list, because
the tables above are the index and `ls -R` should agree with them.

**One caveat, which follows from what a relative link is: it resolves against the path a document
was *opened* by, and every symlink here sits deeper than its source** — one level for the built set,
two under `todo/`. Opened at its real path a document resolves all of its own links; opened *through*
its symlink, only links into the same group survive — the rendering chapters and
`authoring/gpu-canvas.md` still reach each other, because those subtrees move together — while a link
into `decisions/`, `architecture/`, `input/` or `issues/` lands the symlink's own depth off. That is
why the tables link every document where it lives rather than through the entry beside it; the
symlinks are for `ls`.
