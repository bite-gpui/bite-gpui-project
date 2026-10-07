# The upstream PRs

- **Status:** proposed — the pull requests to open against `zed-industries/zed`, and, as important,
  what each must **not** carry. The decision is
  [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md); the design is
  [`surfaces.md`](surfaces.md); the order and the probes are [`surface-plan.md`](surface-plan.md)
  (W1, W2); what stays ours is [`interop-crate.md`](interop-crate.md).
- **Evidence:** the upstream Discussion [zed-industries/zed#64849](https://github.com/zed-industries/zed/discussions/64849),
  which is the signal §1 reads — a maintainer welcomes `paint_surface` on Windows, prefers surface
  sharing, and declines a Direct3D 12 backend, an external-device mode, and the callback primitive.
- **Two PRs are core, and a third is a proposal.** PR 1 is hygiene and independent; PR 2 is the one
  upstream said it would take; PR 3 is the guest-mode ask, which may want its own discussion first.
- **The fork's payload is richer than PR 2's.** Where this document describes upstream's PR — a bare
  `ID3D11ShaderResourceView` Windows variant — the fork builds
  `SurfaceSource::DirectX(DirectXSource)`, whose `DirectXSource` is `Texture` (the renderer makes the
  view) or `View` (the producer made it) — [`surfaces.md`](surfaces.md) §1. The bare SRV is what the
  PR proper proposes.

## 1. What upstream has already said, and what it settles

The Discussion is not a wish — it is a set of decisions we can build against. Each quote, and the
consequence for this plan:

| upstream, in the Discussion | what it settles |
| --- | --- |
| *"we don't support `paint_surface` on windows yet but i would not have any problem with upstreaming support for that since it should be straightforward"* | **PR 2 is welcome.** The Windows surface arm is not a fight. |
| *"i would strongly lean towards surface sharing here because it's quite a bit simpler for everyone involved at the cost of one copy"* | **PR 2 is surface sharing — an `ID3D11ShaderResourceView` the renderer samples** — not 11on12, not a second backbuffer path. |
| *"if you want … external content … that shows up in the GPUI scene and supports all of the GPUI features (clipping etc.) then you want the `paint_surface` support"* | The unified surface path is the right ask, and the feature list (clipping) is what `PaintSurface` already gives. |
| *"i don't think we would take a D3D12 backend"* | **No Direct3D 12 backend, ever, in this plan.** Windows stays Direct3D 11 in core. |
| *"we don't use wgpu everywhere … we like controlling everything in the stack"* | **Nothing `wgpu` in core.** The bridge is downstream ([`interop-crate.md`](interop-crate.md)). |
| on an external device / `run_embedded`: *"we would not be interested in supporting this"* | **Guest Mode is headless**, not a disaggregated window ([`surfaces.md`](surfaces.md) §4). |
| on the paint callback primitive: *"i don't think we'd want this upstream either but it wouldn't be that hard to keep in sync"* | **Path B is fork-carried, not proposed.** It is W7, and it never becomes a PR. |
| on 11on12: *"trivial to add that in your fork"* | 11on12 is a **fork patch**, not upstream. |

Two of these change the plan, and both are now written into it: **Path B is not an upstream PR**
([`surface-plan.md`](surface-plan.md) W7), and **the bridge is downstream** — which the Discussion
confirms rather than merely permits.

## 2. PR 1 — core hygiene: `PixelBuffer`, and rendering split from readback

**Title:** *Decouple offscreen rendering from CPU readback, and drop the `image` dependency from the
renderer trait.*

**Why.** Upstream's `SceneRenderer` couples rendering to a CPU round trip and names `image::RgbaImage`
in its signature, so every renderer carries an image codec it does not need, and a consumer that wants
the frame on the GPU pays for a copy it never reads. This is the change with reach beyond any one
feature: it is what makes headless rendering and snapshotting a contract rather than a test shim.

**Scope.**

- Extract `PixelBuffer` (`crates/gpui_engine/src/renderer.rs:19`) — RGBA8, tightly packed, no external
  dependency — in place of `image::RgbaImage`.
- Split `render_scene` (into an offscreen target, no copy) from `read_pixels` (the CPU copy), with
  `render_scene_to_image` as the two together (`crates/gpui_engine/src/renderer.rs:102`, `:110`,
  `:115`).
- Un-gate the offscreen methods so a renderer without a window can implement them.

**What it must not carry.** No platform types, no `wgpu`, no new seam. It is a signature change and a
type move.

**One shape to settle at review.** The fork's built `PixelBuffer` is RGBA8-tightly-packed with no
`format` or `stride` field; the design sketch proposed a `PixelFormat` enum and a stride. The built
shape is simpler and already tested (`crates/gpui_engine/src/renderer.rs:19`), and is the one to
propose — widen it only if upstream asks, because a format enum is a boundary this PR does not need.

**Tests and risk.** Low. The engine's rows already exercise it ([`verification.md`](verification.md)
§2). Independent of PR 2 — submit it first. The body is [`pr-1-pixel-buffer.md`](pr-1-pixel-buffer.md).

## 3. PR 2 — `paint_surface` on Windows via Direct3D 11 surface sharing

**Title:** *Implement `paint_surface` on Windows via Direct3D 11 surface sharing.* (The maintainer's
own phrasing.)

**Why.** `surface()` is the compositor interface for pixels GPUI did not draw, and it is macOS-only
today. Extending it to Windows is what the Discussion asked for, it is the shape upstream said it would
take, and it removes one of the cheapest reasons to fork — the one this whole record exists to remove.

**Scope — three parts, all core.**

1. **The element's payload becomes cross-platform.** `SurfaceSource`
   (`crates/gpui_authoring/src/elements/surface.rs:13`) grows a Windows variant carrying an
   `ID3D11ShaderResourceView`; `PaintSurface` (`crates/gpui_engine/src/scene.rs:784`) stops being
   `#[cfg(macos)]`-only in a way that leaves the element unusable elsewhere, and gains the
   `corner_radii` the element already stubs (`crates/gpui_authoring/src/elements/surface.rs:99`).
2. **`draw_surfaces` on `DirectXRenderer`** (`crates/gpui_windows/src/directx_renderer.rs:852`) — the
   stub it is today — samples the SRV through the quad pipeline: **straight-through `_UNORM`, no
   transfer function to cancel**, the geometry and content-mask clip the quad path already gives, and
   an **immediate unbind** (`PSSetShaderResources(0, None)`) so a producer writing next frame cannot
   race the bind. **Fault softly**: a bad or mismatched SRV drops the frame, never panics.
3. **The device rendezvous** — `DirectXWindowExt::d3d11_device()`, so a producer can allocate the SRV
   *on the window's device*. This travels with the element: without it the feature is unusable, so it
   is part of this PR and not a follow-up ([`surfaces.md`](surfaces.md) §5,
   [`0004`](../../decisions/0004-producer-device-rendezvous.md)).

**What it must not carry.** No NT-handle export/import, no cross-API fences, no `wgpu`, no Direct3D 12
— all of that is the downstream bridge ([`interop-crate.md`](interop-crate.md)) and the Discussion
declines it in core. **No YCbCr**: ship RGBA/BGRA, and a format arm is a later, additive pass
(`surface-plan.md` P4).

**Tests.** The rows [`verification.md`](verification.md) §2 already describes, on `windows-latest`: an
SRV composites through `surface()`, and the straight-through round trip is bit-exact.

**Risk.** Low, and the Discussion says so: it alters no existing pass and satisfies an open request.
The body is [`pr-2-windows-paint-surface.md`](pr-2-windows-paint-surface.md).

## 4. PR 3 (proposed) — headless window support, for Guest Mode

**Title:** *Open a headless window from `Platform`, so a host loop can drive GPUI offscreen.*

**Why.** Guest Mode ([`surfaces.md`](surfaces.md) §4) runs GPUI headless inside a foreign loop. The
offscreen *rendering* contract is PR 1; what is missing upstream is a way to **open a window with no OS
shell** and dispatch input into it.

**Scope.** A headless-window constructor on `Platform` (`open_headless_window`), and the input path a
host uses to push translated events. It builds on the recent headless work the sketch points at rather
than starting one.

**What it must not carry.** No external-device mode, no "GPUI inside someone else's window" — the
Discussion declines that outright: GPUI owns the window, the loop, the input and the presentation, and
disaggregating that is more than a renderer. Guest Mode is the *headless* answer to that, not a
`run_embedded` for desktops.

**Note.** This is the PR with the weakest prior signal — the Discussion did not speak to headless
directly — so it may deserve its own discussion before a PR. It is listed here so it is not forgotten,
not because it is cleared.

## 5. What stays downstream — and never becomes a PR

| capability | where | why not upstream |
| --- | --- | --- |
| the cross-device bridge, per OS transport: Windows NT-handle export/import (D3D12/`wgpu` → D3D11) and `ID3D12Fence`↔`ID3D11Fence`; macOS `IOSurface` adoption and `MTLSharedEvent`; Linux dma-buf and a dma-fence | `gpui-interop` | it would put `wgpu`, Direct3D 12 and an OS transport in core, which the Discussion declines |
| adapter matching — a LUID match on Windows, a caller-supplied device elsewhere | `gpui-interop` | a producer-side concern; Windows-only in mechanism, and P6 decides whether it is even reliable |
| the Linux `DmaBuf` surface arm (W3) | the fork | the macOS arm is upstream's and PR 2 covers Windows; P3 cleared it |
| **Path B — the inline callback primitive** | the fork (W7) | the Discussion: *"i don't think we'd want this upstream either"* |
| 11on12 backbuffer injection | the fork | *"trivial to add that in your fork"* |
| the guest runner and host input adapters | `gpui-interop` | a foreign loop's job |

## 6. Order and risk

1. **PR 1 first** — independent, low risk, and it is the precondition for anything offscreen.
2. **PR 2 next** — the one upstream has already welcomed; the highest-value, lowest-risk change in the
   set, because it *removes* a fork reason rather than adding a capability.
3. **PR 3 last, maybe after its own discussion.**
4. **Path B never** — it is ours to carry, and now we know upstream does not want it.
