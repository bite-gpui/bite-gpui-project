- **Opened:** 2026-10-01
- **Status:** open — the crate and its home are settled; what remains is shipping (upstreaming,
  publishing, P1 and P2).
- **Touches:** `bite-gpui/bite-gpui` (the upstream PR sequence and the Windows/Linux arms), the `gpui_interop` repository, [`../decisions/0005-external-rendering-unifies-under-surface.md`](../decisions/0005-external-rendering-unifies-under-surface.md), [`../spi/rendering/surfaces.md`](../spi/rendering/surfaces.md), [`../spi/rendering/surface-plan.md`](../spi/rendering/surface-plan.md)

# The surface implementation, across repositories

## The problem

The surface work has no single branch to live on. It is a sequence of upstream pull requests, two
out-of-tree renderer arms, a downstream companion crate that now exists, and the probes that gate it.
No one branch can own it, which is why it is here.

The decision is [`0005`](../decisions/0005-external-rendering-unifies-under-surface.md), the design is
[`surfaces.md`](../spi/rendering/surfaces.md), and the order and the probes are
[`surface-plan.md`](../spi/rendering/surface-plan.md) §1 and §2. This issue is the coordination those
three do not carry: what goes where, what has to be decided before what, and what "done" means. It does
not restate them. The crate is described in
[`../spi/rendering/interop-crate.md`](../spi/rendering/interop-crate.md) and its scaffold in
[`../spi/rendering/interop-scaffold.md`](../spi/rendering/interop-scaffold.md). Its gating probes are
specified in
[`../spi/rendering/probe-p1-reverse-bridge.md`](../spi/rendering/probe-p1-reverse-bridge.md),
[`probe-p2-macos-adoption.md`](../spi/rendering/probe-p2-macos-adoption.md),
[`probe-p3-dmabuf-import.md`](../spi/rendering/probe-p3-dmabuf-import.md),
[`probe-p5-fence-loop.md`](../spi/rendering/probe-p5-fence-loop.md),
[`probe-p6-adapter-luid.md`](../spi/rendering/probe-p6-adapter-luid.md),
[`probe-p9-device-loss.md`](../spi/rendering/probe-p9-device-loss.md),
[`probe-p4-video-formats.md`](../spi/rendering/probe-p4-video-formats.md) and
[`probe-p7-guest-thread.md`](../spi/rendering/probe-p7-guest-thread.md). The pull requests the upstream
half becomes are described in
[`../spi/rendering/upstream/README.md`](../spi/rendering/upstream/README.md).

## What is built, and what is settled

- **The core path.** The offscreen contract — `PixelBuffer`, and `render_scene` split from
  `read_pixels` — is W1; the Windows and Linux surface arms and `GpuCanvas` are W2–W4. All are built on
  the canonical ref, so the work here is the *entry point* and the *bridge* (W5–W6), not the engine.
  The upstream PRs stay additive, and nothing here blocks a release of the current line.
- **The home and name (question 1).** `gpui_interop` is its own repository,
  `bite-gpui/gpui_interop`, listed in [`../repositories.md`](../repositories.md) and
  [`../references.md`](../references.md). It sits above the facade and adds nothing to core
  ([`../architecture/extension-tiers.md`](../architecture/extension-tiers.md)).
- **The crate is built.** `src/{adapter,guest,input,linux,macos,windows}.rs`, the cube and Linux
  examples, and the cross-device, dma-buf-readback and surface-input tests. Windows (`SharedSurface`
  and `Fence`) and Linux (`SharedBuffer`/`PlaneLayout` and `Fence`, including a producer-pushed
  layout) are implemented; the crate's README marks macOS **not yet implemented**, because question
  5's P2 is still unmeasured.
- **P1 gates the guest runner only (question 4).** The reverse bridge (Direct3D 11 → `wgpu`) is W6's,
  not the crate's (W5): Host Mode's direction is measured, and a Direct3D 11 producer uses the default
  `DirectXRenderer`. A P1 failure costs Guest Mode, not the crate.
- **The device-loss contract (question 6) is settled and measured.** P9 ran on the Windows guest: the
  pool tears down and re-negotiates on a fresh device without crashing
  ([`../decisions/windows-device-loss-probe.md`](../decisions/windows-device-loss-probe.md)).
- **The sequence (question 3) is decided.** Branch first, per
  [`../spi/rendering/upstream/README.md`](../spi/rendering/upstream/README.md): rebase
  `bite_v1.24.x` onto `v1.24`, `v1.25-pre` and `main`; carry the rendering commits onto a branch off
  `main`; then open the PRs. Nothing is executed yet.
- **P7 is measured (question 5), and its runner is built.** A worker thread's offscreen frame crosses
  as a shareable surface; the contract gained `SceneRenderer::offscreen_surface_handle`, and the row
  passes on both GPUs ([`../decisions/guest-thread-probe.md`](../decisions/guest-thread-probe.md)).
  `gpui_interop::Guest` is the runner around it: a headless window on a worker thread, host-paced
  frames, and `Guest::update` for work back in. Its test (`tests/guest_runner.rs`, feature
  `guest-test`) passes on the Intel HD 520 and the NVIDIA 930M.

## What remains — and why this issue is open

- **The upstream PRs are not opened.** The plan is written; every body says "opened: not yet", and no
  pull request exists.
- **The crate is not published, and cannot yet be used beside a released `gpui`.** Its manifest is
  `0.1.0` and takes `gpui`, `gpui_engine`, `gpui_wgpu` and `gpui_windows` from the sibling `bite-gpui`
  working tree (`path = "../bite-gpui/crates/…"`); `bite-distribution/targets.toml` has no interop
  target. The repository's own comment says those paths become the published package at release.
- **The published payload types (question 2) wait on that release.** The crate names the `windows` and
  `wgpu` types directly and pins them in its manifest; which crate publishes the core's payloads, and
  at which version, is settled only when the core is released and the paths become versions.
- **The probes are only partly filed (question 5).** P3, P5, P6, P7 and P9 have evidence records
  ([`../decisions/linux-dmabuf-probe.md`](../decisions/linux-dmabuf-probe.md),
  [`../decisions/windows-fence-loop-probe.md`](../decisions/windows-fence-loop-probe.md),
  [`../decisions/windows-adapter-luid-probe.md`](../decisions/windows-adapter-luid-probe.md),
  [`../decisions/guest-thread-probe.md`](../decisions/guest-thread-probe.md),
  [`../decisions/windows-device-loss-probe.md`](../decisions/windows-device-loss-probe.md)); P1 and
  P2 have specifications but no record, and P4 is an inventory.
- **M6 and M7.** The Windows Host Mode bridge is built; the macOS module waits on P2. M7's guest
  runner is built and measured; what it leaves is the input adapter over `Guest::update`, and P1's
  reverse transport for the Windows Guest Mode direction — neither gates the runner's threading or
  its frame hand-off.
- **The org indexes are updated.** `repositories.md`, `references.md` and `online-resources.md` carry
  all eleven repositories, including `gpui_interop` and `gpui_video`, whose GitHub descriptions,
  topics and homepage are now set (2026-10-18).

## What would close it

The upstream PRs merged; `gpui_interop` published and depending on a released `gpui`; P1 and P2
measured and filed as evidence; and M6's macOS module and M7's input adapter built. The home, the name,
the crate itself and P7 no longer gate closure, and neither does the guest runner's frame path.
