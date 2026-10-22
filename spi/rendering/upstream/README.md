# The upstream PRs

- **Status:** proposed — the pull requests to open against `zed-industries/zed`, and, as important,
  what each must **not** carry. The decision is
  [`0005`](../../../decisions/0005-external-rendering-unifies-under-surface.md); the design is
  [`surfaces.md`](../surfaces.md); the order and the probes are [`surface-plan.md`](../surface-plan.md)
  (W1, W2); what stays ours is [`interop-crate.md`](../interop-crate.md).
- **Evidence:** the upstream Discussion [zed-industries/zed#64849](https://github.com/zed-industries/zed/discussions/64849),
  which is the signal §1 reads — a maintainer welcomes `paint_surface` on Windows, prefers surface
  sharing, and declines a Direct3D 12 backend, an external-device mode, and the callback primitive.
- **The order is provisional.** Six PRs, in §2's order: **1** the vocabulary and the new SPI/API;
  **2–4** the platform arms — Linux, Windows and Mac — of which Windows is the one upstream said it
  would take; **5–6** the capabilities the fork has not built or has only half built (GPU selection,
  offscreen/headless), each **independent of the rest and fileable in parallel**. The sequence to get
  there is below.
- **The fork's payload is richer than PR 3's.** Where this document describes upstream's PR — a bare
  `ID3D11ShaderResourceView` Windows variant — the fork builds
  `SurfaceSource::DirectX(DirectXSource)`, whose `DirectXSource` is `Texture` (the renderer makes the
  view), `View` (the producer made it) or `Shared` (a device-independent NT handle the renderer
  resolves) — [`surfaces.md`](../surfaces.md) §1. The bare SRV is what the PR proper proposes; §6 records
  what of the fork is upstreamable capability and what is its own organization.

## The sequence to get there

**Logged, not executed.** The plan to turn these PRs into reality:

1. **Rebase the line forward** onto upstream's `v1.25` (done — that is `bite_v1.25.x`) and then
   `main` — the stack as it is, on the newer bases, so the rendering work sits on a base a PR can
   target.
2. **Branch an upstreaming branch off upstream `main`** and carry the rendering commits onto it —
   cherry-pick / migrate / transplant, per commit, the way the stack is built.
3. **Run the tests, refine the comments, open the PRs.**

§2 is the PRs; §7 is the commits each would carry.

## 1. What upstream has already said, and what it settles

The Discussion is not a wish — it is a set of decisions we can build against. Each quote, and the
consequence for this plan:

| upstream, in the Discussion | what it settles |
| --- | --- |
| *"we don't support `paint_surface` on windows yet but i would not have any problem with upstreaming support for that since it should be straightforward"* | **PR 3 is welcome.** The Windows surface arm is not a fight. |
| *"i would strongly lean towards surface sharing here because it's quite a bit simpler for everyone involved at the cost of one copy"* | **PR 3 is surface sharing — an `ID3D11ShaderResourceView` the renderer samples** — not 11on12, not a second backbuffer path. |
| *"if you want … external content … that shows up in the GPUI scene and supports all of the GPUI features (clipping etc.) then you want the `paint_surface` support"* | The unified surface path is the right ask, and the feature list (clipping) is what `PaintSurface` already gives. |
| *"i don't think we would take a D3D12 backend"* | **No Direct3D 12 backend, ever, in this plan.** Windows stays Direct3D 11 in core. |
| *"we don't use wgpu everywhere … we like controlling everything in the stack"* | **Nothing `wgpu` in core.** The bridge is downstream ([`interop-crate.md`](../interop-crate.md)). |
| on an external device / `run_embedded`: *"we would not be interested in supporting this"* | **Guest Mode is headless**, not a disaggregated window ([`surfaces.md`](../surfaces.md) §4). |
| on the paint callback primitive: *"i don't think we'd want this upstream either but it wouldn't be that hard to keep in sync"* | **Path B is fork-carried, not proposed.** It is W7, and it never becomes a PR. |
| on 11on12: *"trivial to add that in your fork"* | 11on12 is a **fork patch**, not upstream. |

Two of these change the plan, and both are now written into it: **Path B is not an upstream PR**
([`surface-plan.md`](../surface-plan.md) W7), and **the bridge is downstream** — which the Discussion
confirms rather than merely permits.

## 2. The PRs, and what each carries

Six, in the order §4 gives. Each is a capability plus the commits §7 lists, grounded in `bite_v1.25.x`;
the fork's organization — the crate split, type names, caches — is the reference implementation, not the
ask (§6). Where a commit straddles two of these, step 2's cherry-pick splits it.

### PR 1 — the vocabulary and the new SPI/API

**Adds:** the renderer seam a window holds — `PlatformRenderer`, the typed target and the factory that
installs a renderer — and the surface vocabulary a consumer meets: `SurfaceSource` (CoreVideo /
`DirectXSource` / `DmaBufHandle`), `SurfaceFormat`, the `PaintSurface` primitive, the `GpuCanvas` element
with its `GpuCanvasContext`, and the `GpuRenderer` device door. It replaces the per-platform `*WindowExt`
hooks, so they go.

**Must not carry:** no platform renderer arm (PR 2–4), no `wgpu`, no Direct3D 12, no YCbCr. The fork-only
`CustomRenderPrimitive` route
([`0005`](../../../decisions/0005-external-rendering-unifies-under-surface.md)) is not an ask.

**Commits:** `29bba3424c` + `0a26fe14c5` (the seam and its test), `c916d1f4dc` (the token),
`493592a555` (the vocabulary), `9fee3ce8d8` (the device), `bcaf568da8` + `2d69f7691c` (`GpuCanvas`),
`cafaa0b629` (the facade tokens).

**Body:** [`pr-vocabulary-and-spi.md`](pr-vocabulary-and-spi.md).

### PR 2 — Linux: dma-buf, Vulkan and wgpu

**Adds:** the Linux window through the renderer; the dma-buf import (`DmaBufTextureCache`, the modifier,
the two-plane NV12 sample); the wgpu arm's `draw_surfaces` and its imported-texture fragment; the Linux
facade re-exports and the wgpu headless renderer.

**Commits:** `4133514795`, `9d5ea87403`, `6813500537`, `f2771c04f2`, `dd2727c0dd`, `58cacb9784`,
`30c527f632`, `9429812ca1`, `72b3661059`.

### PR 3 — Windows: D3D11 textures and D3D12 shared buffers

**Adds:** the Windows window through the renderer; `draw_surfaces` on `DirectXRenderer`, sampling an
imported `ID3D11ShaderResourceView` straight through `_UNORM` (no transfer function to cancel, and an
immediate unbind so a producer writing next frame cannot race the bind); the `DirectXTextureExt` token;
and the renderer-owned **consumer** half of a Direct3D 12 / `wgpu` shared NT handle and fence
(`DirectXSource::Shared`), cached per handle — so a producer on a second device hands over a handle and
nothing device-specific. A bad or mismatched view drops the frame, never panics.

**Commits:** `5c29591591`, `c2955d8036`, `a4fefffb6f`, `9609e99bcc`, `4e4415fd60`, `d5d9ba0ee0` (the arm
and the shared-texture consumer), `b23b6d1d48` (the CI row).

**Note — the fork diverges from upstream's producer route.** The ask names
`DirectXWindowExt::d3d11_device()`; the fork replaced it with `GpuRenderer` plus
`GpuCanvasContext::device::<R>()` / `try_device::<R>()` and deleted `DirectXWindowExt`
([`0006`](../../../decisions/0006-renderer-owned-device.md)). The *capability* is unchanged; the spelling
is the fork's.

**Must not carry:** the cross-device **producer** half — allocate, export the NT handle, signal the fence
— which stays downstream ([`interop-crate.md`](../interop-crate.md)); and no YCbCr (RGBA/BGRA ships, a
format arm is additive — `surface-plan.md` P4).

**Body:** [`pr-windows-paint-surface.md`](pr-windows-paint-surface.md) — the body is upstream's
same-device arm; the `Shared` half is the capability §6 describes.

### PR 4 — Mac/Apple: Metal and `IOSurface`

**Adds:** the macOS window through the renderer; the Metal arm taking `SurfaceSource::CoreVideo`; the
`MetalTextureExt` producer half, with the exact sRGB round trip; the macOS backend's device.

**Commits:** `1b647e8a22`, `711698e134`, `f3cc13a9a9`, `56c0c9c392`, `2e9468661e`.

### PR 5 — GPU enumeration and selection (selection and enumeration built, capability not)

**Adds:** an API for an application to *list* the adapters a machine has and *select* one per window or
renderer. **Selection is built:** `GpuPreference` on `Application`, `ZED_DEVICE_ID` read only in the
Linux platform, `LowPower` by default. **Enumeration is built:** `App::enumerate_gpus` / `active_gpu`
and `GpuAdapterInfo`, with `Custom` naming one adapter. **Capability is not:** there is no report of
what a device will import or export, beyond `GpuSpecs` *reporting*
(`crates/gpui_platform/src/gpu.rs`). A separate PR for the API, and one per platform.

**Commits:** `36523f8a52` (the preference), `95204431ac` (enumeration), `b4a88e5fee` (the environment
read moves to the platform).

### PR 6 — offscreen and headless (partial)

**Adds:** offscreen rendering as a renderer capability (`PixelBuffer`, the `render_scene` / `read_pixels`
split) and a headless window a host loop drives — the aim being to *schedule* a workload onto a chosen
GPU, which needs PR 5.

**Partial:** the offscreen contract and its Metal/Direct3D halves are built, and a headless window exists
on some platforms (the Linux headless window, the wgpu and Metal headless renderers, the test platform);
the per-platform scheduling and the selection half are not. A separate PR for the API, and one per
platform.

**Commits:** `f9d6d66583` (the contract), `f97ecc828e` + `503649acd0` (Direct3D), `d1d96a2ab9` (Metal),
`7cd9a0f6a6` (the test renderer's readback); the headless constructors are partial.

**Body:** [`pr-offscreen.md`](pr-offscreen.md).

## 3. What stays downstream — and never becomes a PR

| capability | where | why not upstream |
| --- | --- | --- |
| the cross-device *producer* half, per OS transport: allocate shareable, export the NT handle / `IOSurface` / dma-buf, and signal the fence (Windows `ID3D12Fence`→`ID3D11Fence`) | `gpui-interop` | it would put `wgpu`, Direct3D 12 and the allocation/export in core, which the Discussion declines. Its *consumer* half — the renderer opening a handle it is given — is the upstreamable `DirectXSource::Shared` capability (PR 3, §6) |
| adapter matching — a LUID match on Windows, the renderer's device on Linux, nothing to match on macOS | `gpui-interop` | a producer-side concern; per-platform in mechanism, and P6 decides whether it is even reliable |
| the Linux `DmaBuf` surface arm (W3) | the fork | the macOS arm is upstream's and PR 3 covers Windows; P3 cleared it |
| **Path B — the inline callback primitive** | the fork (W7) | the Discussion: *"i don't think we'd want this upstream either"* |
| 11on12 backbuffer injection | the fork | *"trivial to add that in your fork"* |
| the guest runner and host input adapters | `gpui-interop` | a foreign loop's job |

## 4. Order and risk

1. **PR 1 first** — the vocabulary and the SPI; every arm and the offscreen contract build on it.
2. **PR 2–4 next** — Linux, Windows and Mac; independent of each other, and Windows is the one upstream
   has already welcomed.
3. **PR 5–6 after, or in parallel** — GPU selection and offscreen/headless; each is a PR for the API and
   one per platform, and PR 6's *selection* half needs PR 5.
4. **Path B never** — it is ours to carry, and now we know upstream does not want it.

## 5. What the fork's build says back to this plan

The fork has carried the work further than §2–§3 assume. Three things are worth reading against
them; none overturns the order in §4.

1. **PR 3 can be opened now.** The Windows arm is built — [`surfaces.md`](../surfaces.md) §1's
   `SurfaceSource`, the `Surface` element, `draw_surfaces` on all three renderers, and a runnable
   example — so the body is already the shape it would propose. Opening it while the code is fresh
   is cheaper than replaying it onto a branch that has moved.
2. **The payload is the one place PR 3 and the fork differ, and it is a judgement call.** §2's PR 3
   and [`pr-windows-paint-surface.md`](pr-windows-paint-surface.md) propose a bare
   `ID3D11ShaderResourceView`, and the note above records that the fork builds
   `SurfaceSource::DirectX(DirectXSource)` — a texture (the renderer makes the view) or a view the
   producer made. **Recommendation:** propose the pair rather than the bare SRV. It is small, it is
   what makes Windows symmetric with macOS's resource-based `CoreVideo` arm, and it is what the
   fork's element and examples already take — so a bare-SRV merge would leave the fork replaying a
   second shape on top of the one it built. If upstream prefers the minimal arm, keep the bare SRV
   *knowingly* and record the divergence here rather than discover it at replay.
3. **The downstream crate is real now, and has its own home.** `gpui_interop` lives in its own
   repository, `bite-gpui/gpui_interop` — the Windows/Linux/macOS transports, the input gateway and
   guest mode, with the cross-device, dma-buf-readback and surface-input tests on CI. §3 is
   unchanged — the bridge stays downstream — and the home issue
   [`0006`](../../../issues/0006-surface-interop.md) question 1 is settled by it
   ([`interop-crate.md`](../interop-crate.md)).

## 6. Upstream the capability, not the fork's organization

The maintainer's posture for this pass: **upstream as much of the rendering slice as possible, but not
the exact slice organization.** The fork is a working reference, not the target layout, so a PR body
proposes the *capability* — the payload arm, the renderer method, the device route — and leaves the
fork's crate and type organization out of the ask. What that splits into:

- **The producer's device door is a capability; its fork spelling is not.** The fork reaches the
  device through `GpuRenderer` plus `GpuCanvasContext::device::<R>()` / `try_device::<R>()`, and has
  deleted `DirectXWindowExt` ([`0006`](../../../decisions/0006-renderer-owned-device.md)); PR 3's body
  names `DirectXWindowExt::d3d11_device()` (§2's PR 3 note, kept). The capability is "a producer can
  obtain the renderer's device"; whether that is a window extension or a downcast to the renderer is
  the part upstream may organize differently.
- **The `DirectXSource::Shared` arm is the upstreamable capability for the cross-device Windows
  case.** The renderer owns the consumer half — `OpenSharedResource1`, the SRV, `OpenSharedFence` and
  `ID3D11DeviceContext4::Wait`, cached per handle — so a producer exports an NT handle and does nothing
  device-specific. That is the same shape as macOS's `CoreVideo` and Linux's dma-buf: a
  device-independent token the renderer resolves, and it is what makes `surface()` work for a
  cross-device producer. Propose the arm and the renderer-owned resolution; do not propose
  `gpui_interop`, the `SharedDirectXSurface` field layout, or the cache as upstream surface.
- **Fork-specific framing stays ours.** Path B, the guest runner, adapter matching, 11on12 and the
  interop crate's posture are fork or downstream, not the PR (§3); the superseded
  `CustomRenderPrimitive` route ([`0005`](../../../decisions/0005-external-rendering-unifies-under-surface.md))
  is not an ask either.

In one line: each PR proposes the smallest core change that unblocks the capability, and the fork's
organization — the exact crate split, type names and caches — is the reference implementation, not the
proposal.

## 7. The commits each PR would carry

§2 reads against the fork's actual history — `bite_v1.25.x`, tip `f02141ce91` (no tag on this line yet). Each PR is the fork's commit group replayed as one:

| PR | fork commits (`bite_v1.25.x`) |
| --- | --- |
| **1** vocabulary + SPI/API | `29bba3424c`, `0a26fe14c5`; `c916d1f4dc`; `493592a555`; `9fee3ce8d8`; `bcaf568da8`, `2d69f7691c`; `cafaa0b629` |
| **2** Linux | `4133514795`, `9d5ea87403`, `6813500537`, `f2771c04f2`, `dd2727c0dd`, `58cacb9784`, `30c527f632`, `9429812ca1`, `72b3661059` |
| **3** Windows | `5c29591591`, `c2955d8036`, `a4fefffb6f`, `9609e99bcc`, `4e4415fd60`, `d5d9ba0ee0`, `b23b6d1d48` |
| **4** Mac/Apple | `1b647e8a22`, `711698e134`, `f3cc13a9a9`, `56c0c9c392`, `2e9468661e` |
| **5** GPU selection | `36523f8a52` (the preference), `95204431ac` (enumeration), `b4a88e5fee` (the env read); the capability tier unbuilt |
| **6** offscreen + headless | `f9d6d66583`, `f97ecc828e`, `503649acd0`, `d1d96a2ab9`, `7cd9a0f6a6`; headless partial |

The fork's own grouping, which these are cut from — **core** (the renderer seam and the
imported-texture path), **#2** the surface vocabulary and `GpuCanvas`, **#3** Linux, **#4** Windows,
**#5** macOS, then the example and the upkeep — is recorded in `bite-tools`'s
`docs/journal/surface-recut-2026-10-15.md`.
