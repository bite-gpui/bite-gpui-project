# Milestones

- **Status:** the handoff. What is built, where it is, what is next in what order, and the gates a
  resumer runs. Written 2026-09-29 against `bite_v1.23.1-pre` (`e867ece9f9`), re-pointed onto the
  canonical `bite_v1.23.x` on 2026-10-17 and re-derived again when the branch advanced to
  `a3b0e8f46c` (the last tag, `bite_1.23.302`, is `5c0bc06887`); a citation's line numbers move
  whenever that ref does, and `script/check-citations` prints what each one now points at.
- **Read with:** the chapter order in [`README.md`](README.md) for the design — this document is the
  state — and [`producer-reach.md`](producer-reach.md), which is the one gap large enough to have a
  chapter rather than a line here.

## 1. Where the work is

| what | where |
| --- | --- |
| the canonical ref, and what every citation resolves against | `bite_v1.23.x`, tip `a3b0e8f46c` (the last tag, `bite_1.23.302`, is `5c0bc06887`) |
| what the canonical ref already carries | the layer stack; the surface re-cut (`SurfaceSource`/`PaintSurface`, `surface()`, `draw_surfaces` on all three renderers, `GpuCanvas`); the renderer-owned device door (`0006`); the Linux dma-buf importer with its native-layout guard (`0007`); and the GPU preference and enumeration (`0008`) |
| the design of record for external pixels | [`surfaces.md`](surfaces.md) — the surface unification [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) decides — and its plan and probes in [`surface-plan.md`](surface-plan.md) |
| the design of record for external pixels | [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) — the surface unification; its source, the *External Surfaces, Platform Interop, and Headless Rendering in GPUI* design record, is filed with the work |
| Path A, whole | on the canonical ref, sixteen commits, PR [#6](https://github.com/bite-gpui/bite-gpui/pull/6) |
| the downstream crate | [`gpui_interop`](https://github.com/bite-gpui/gpui_interop) — its own repository, the bridge and the input gateway; issue [`0006`](../../issues/0006-surface-interop.md) |
| the checkout the work is in | the `bite-gpui` clone on `bite_v1.23.x`, beside the `gpui_interop` repository |
| the other repositories, and what each is for | [`../../references.md`](../../references.md) |

`#6` merged as `fcde78d01a`, and it was the whole path: the Direct3D arm was stacked on it
(`bite_v1.22.0-pre-path-a-directx`, PR [#7](https://github.com/bite-gpui/bite-gpui/pull/7)) and the
producer's reach on top of that (`bite_v1.22.0-pre-device-rendezvous`, PR
[#8](https://github.com/bite-gpui/bite-gpui/pull/8)), and both were replayed into `#6` rather than
merged anywhere else. So no pull request is open, the two head branches are obsolete, and the
canonical ref carries Path A.

## 2. What is built, and what is measured

| item | chapter | where it is | evidence |
| --- | --- | --- | --- |
| the renderer seam: `PlatformRenderer`, the typed `RendererTarget`, the factory, the native hooks | [`renderer-seam.md`](renderer-seam.md) | the canonical ref | PR #4 |
| the offscreen contract: the three methods ungated, `render_scene` split from `read_pixels`, `PixelBuffer` in place of `image::RgbaImage`, and all three renderers implementing it in the same shape | [`README.md`](README.md) | the canonical ref | `gpui_engine`'s 7 tests, ubuntu CI. Direct3D's "offscreen" is its window's swap chain — it has no headless constructor — so its `render_scene` is "rendered and not presented", which is what its rows already build a hidden window for |
| the surface vocabulary: `SurfaceSource` (`crates/gpui_engine/src/scene.rs:851`), `DirectXSource` (`:802`) and `PaintSurface` (`:908`) | [`surfaces.md`](surfaces.md) | the canonical ref | the three arms below |
| `draw_surfaces` on all three renderers (`crates/gpui_wgpu/src/wgpu_renderer.rs:2086`, `crates/gpui_apple/src/metal_renderer.rs:1156`, `crates/gpui_windows/src/directx_renderer.rs:919`) and the `surface()` element (`crates/gpui_authoring/src/elements/surface.rs:61`) | [`surfaces.md`](surfaces.md) | the canonical ref | [`surface-plan.md`](surface-plan.md) W2/W3; [`verification.md`](verification.md) §2 |
| `GpuCanvas` (`crates/gpui_authoring/src/elements/gpu_canvas.rs:135`) and its context (`:37`): `paint_surface` (`:46`), `paint_texture` (`:64`), `device::<R>()` (`:87`) | [`../authoring/gpu-canvas.md`](../authoring/gpu-canvas.md) | the canonical ref | the hand-rolled demos became the element |
| the Linux dma-buf importer: `DmaBufHandle` (`crates/gpui_engine/src/dmabuf.rs:175`), `DmaBufTextureCache` (`crates/gpui_wgpu/src/dmabuf.rs:785`) and the native-layout guard (`crates/gpui_wgpu/src/dmabuf.rs:256`) | [`zero-copy-import.md`](zero-copy-import.md) | the canonical ref | [`optimus-dmabuf-topologies.md`](../../decisions/optimus-dmabuf-topologies.md); the `surface_dmabuf` suites on both GPUs |
| the GPU preference and enumeration: `GpuPreference` (`crates/gpui_platform/src/gpu.rs:219`), `with_gpu_preference` (`crates/gpui_runtime/src/application.rs:94`), the shared `rank` (`crates/gpui_platform/src/gpu.rs:397`), and `enumerate_gpus` / `active_gpu` (`crates/gpui_authoring/src/app.rs:2116`, `:2124`) over `GpuAdapterInfo` (`crates/gpui_platform/src/gpu.rs:314`) | [`zero-copy-import.md`](zero-copy-import.md) and [`upstream/README.md`](upstream/README.md) PR 5 | the canonical ref | [`0008`](../../decisions/0008-gpu-preference-and-enumeration.md); issue [`0010`](../../issues/0010-application-gpu-preference.md) and [`0012`](../../issues/0012-gpu-enumeration-and-negotiation.md) closed; `App::dmabuf_capabilities` is the capability tier |
| the superseded route: `CustomRenderPrimitive::Texture`, `ImportedTextureHandle` and `paint_imported_texture` | [`foreign-texture.md`](foreign-texture.md) | the canonical ref, superseded by `0005`; the surface path is the public one | `crates/gpui_engine/src/custom_render.rs:41`, `:14`; Path B's `Inline` is unbuilt |
| the wgpu arm, the offscreen target and the readback | [`foreign-texture.md`](foreign-texture.md) §7 | the canonical ref | 3 rows — the sRGB round trip, the ordering, the device identity — skipped with a logged warning where the machine has no adapter |
| the Metal arm | [`foreign-texture.md`](foreign-texture.md) §7 | the canonical ref | 2 rows on `macos-14` — the colour round trip and the boundary check — plus the shader compile the `macos` job already did |
| the Direct3D arm and its CI job | [`foreign-texture.md`](foreign-texture.md) §7 | the canonical ref | 3 rows on `windows-latest`, WARP |
| the producer's reach: `GpuRenderer::device` and the canvas door `GpuCanvasContext::device::<R>`, and a token builder on each of the three arms (`ImportedTextureExt`, `DirectXTextureExt`, `MetalTextureExt`) | [`producer-reach.md`](producer-reach.md) | the canonical ref | the Direct3D and Metal rows take their device through the door — the route an application has — instead of the renderer's own field |
| the macOS producer route, measured rather than built: wgpu's adapter *is* the `MetalRenderer`'s own `MTLDevice`, so a wgpu producer there needs a power preference and no handover | [`producer-reach.md`](producer-reach.md) §7 | the canonical ref | the probe's printout, [`../decisions/macos-wgpu-producer-probe.md`](../../decisions/macos-wgpu-producer-probe.md) |
| the runnable demos: `cargo run -p gpui --example surface`, a producer per platform, and `gpui_interop`'s `interop_cube_drag` | [`foreign-texture.md`](foreign-texture.md) §3, [`zero-copy-import.md`](zero-copy-import.md) | the canonical ref (which carries it as `path_a.rs`); the interop repository | compile for the host, `aarch64-apple-darwin` and `x86_64-pc-windows-msvc`; running needs a display |

## 3. The gaps beside the reach

Small and independent. The transfer function is the one with a milestone of its own (M9); the rest do
not block anything in §4.

- **Metal cannot enforce the same-device rule.** Each arm checks what its sampler needs at the
  boundary, but only Direct3D refuses a texture from *another* device by name — wgpu panics inside
  its own storage, which is loud rather than named, and Metal has no API for it at all, because a
  resource does not expose the device that made it. So on macOS that half of the rule rests on the
  application; it is not closable by writing code
  ([`producer-reach.md`](producer-reach.md) §3).
- **The surface path has no transfer function.** `YuvColorSpace` models a matrix and a range but not
  the transfer function, so an sRGB-encoded producer's bytes are sampled as linear; range and
  transfer are independent axes, so `YuvRange` must not absorb it. Issue
  [`0013`](../../issues/0013-transfer-function-and-srgb-sampling.md).
- **The pull-request test job's filter is narrow.** `bite-ci.yml`'s `tests` job runs
  `cargo test -p gpui_authoring -p gpui_engine --lib` and nothing else, so every other crate's tests
  are local-only — `gpui_wgpu`'s need a GPU adapter, and its older tests fail rather than skip
  without one, which is a decision about what a GPU-less runner is for rather than a wiring change.
  Issue [`0003`](../../issues/0003-scheduled-builds.md) holds the wider version of the question.
- **The canonical ref's citations move when it advances.** Anything that lands on
  `bite_v1.23.x` re-derives every citation in this repository.

## 4. What is next, in order

Reshaped by [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md), which moves
external pixels onto the existing `PaintSurface` / `surface()`. M1, M2, M3, M5, M8 and M9 are done;
M4 is unstarted; M6 is built on Windows and Linux but not published; M7 is partial.

**M1 — the producer's reach. Done.** [`producer-reach.md`](producer-reach.md) and
[`0006`](../../decisions/0006-renderer-owned-device.md) (which supersedes 0004). `GpuRenderer::device`
is built on each renderer that owns a device, and the canvas lends it as
`GpuCanvasContext::device::<R>`, so the decision that was M1's first step is taken and the door exists; what
each platform still lacks is [`producer-reach.md`](producer-reach.md) §7, and none of it blocks
anything below.

**M2 — the gaps in §3. Done for Path A.** The Metal token builder, the exact encoder, the arm's two
rows, the Direct3D renderer's offscreen shape, the wgpu token's reach through the facade, and the
macOS producer route — measured rather than handed over — all landed, and a runnable demo ties them
together. What is left in §3 is the test filter, the transfer function and the citation churn.

**M3 — `GpuCanvas`, retargeted. Done.** [`../authoring/gpu-canvas.md`](../authoring/gpu-canvas.md). The
surface an application meets; 0005 changes only its Path A arm — it produces a `SurfaceSource` and
pushes it through `surface()` rather than a second primitive, and its Path B arm is unchanged. With
M1 built, the payload its callback returns is one an application can construct, which is what makes
the canvas a capability rather than a demonstration. Built in #9 (macOS/Windows), with the same-device
arm (`GpuCanvasContext::paint_texture` → the crate-internal `paint_imported_texture`) added for
Linux/macOS after it.

**M4 — Path B.** [`inline-commands.md`](inline-commands.md). Unstarted. It draws into the window's
own pass, not a buffer, so 0005 leaves it alone; it needs no device export and is not behind M1.

**M5 — the unified surface arm. Done.** `SurfaceSource` / `PaintSurface` on Windows and Linux, with
`draw_surfaces` on all three renderers and the `surface()` element. The Windows half is upstream PR 3
and is built (#9); the Linux half is the dma-buf variant, P3 cleared, with its importer guard
([`0007`](../../decisions/0007-dmabuf-importer-native-layout-guard.md)). This is what makes `surface()`
a capability on every desktop rather than macOS alone.

**M6 — the cross-API bridge, `gpui_interop`. Host Mode built on Windows and Linux; shipping remains.**
The crate is its own repository (issue [`0006`](../../issues/0006-surface-interop.md)); Windows
(`SharedSurface` + `Fence`) and Linux (`SharedBuffer`/`PlaneLayout` + `Fence`, including a
producer-pushed layout) are built and tested, and the crate's README marks macOS **not yet
implemented** (P2). It is not published: it takes the core from the sibling `bite-gpui` working tree,
and `bite-distribution` has no interop target. The reverse direction — a Direct3D 11 producer feeding a
`wgpu` consumer — is still unmeasured ([`producer-reach.md`](producer-reach.md) §6) and is Guest
Mode's, so it keeps a probe of its own ([`interop-crate.md`](interop-crate.md)).

**M7 — guest / headless. Partial.** The offscreen contract is built (M2) and `gpui_interop` has a
`guest` module, but the worker-thread runner that hosts GPUI inside a foreign loop is not measured
(P1, P7).

**M8 — GPU preference and enumeration. Done.** Decision
[`0008`](../../decisions/0008-gpu-preference-and-enumeration.md) and issue
[`0010`](../../issues/0010-application-gpu-preference.md) are implemented: `GpuPreference` on
`Application`, `ZED_DEVICE_ID` read only in the Linux platform (`gpu_preference_from_env`), `LowPower`
by default, and `App::enumerate_gpus` / `active_gpu` with `Custom` naming one adapter. Issue
[`0012`](../../issues/0012-gpu-enumeration-and-negotiation.md) added the capability tier and is
closed: `App::dmabuf_capabilities` reports, per format, the DRM modifiers the renderer's device will
import. Runtime switching stays refused, as recorded.

**M9 — the surface's transfer function. Done.** Issue
[`0013`](../../issues/0013-transfer-function-and-srgb-sampling.md): the traced answer is that GPUI's
targets are non-sRGB `_UNORM` holding display-encoded values, so an sRGB source was already correct
and a **linear** one was the gap. `DmaBufHandle::with_transfer` declares the curve; the shader encodes
only the linear case.

## 5. The gates

**`.meta`.** `script/check-citations` — 450 citations, 0 unresolved (2026-10-17). It
resolves against the canonical ref, so a document may not cite `path:line` for a file that exists
only on a branch: name it, as `producer-reach.md` does for `crates/gpui_engine/src/custom_render.rs`
and the two `imported_texture.rs` files. `.meta` is its own repository; never `git add` the zed
clone into it, and never push from either without checking for work in flight, because it is
committed to concurrently.

**The fork's CI** is `bite-ci.yml`, carried on each `bite_*` branch of `bite-gpui/bite-gpui` — a
pull request resolves its workflows from the merge commit, so a file a branch does not carry does
not run for it. It is the only CI that runs for these pull requests: upstream's `run_tests.yml`
triggers and every job is skipped, because they are gated on the repository owner being
`zed-industries`. Its jobs, and what each is the only place for:

| job | answers | why nothing else can |
| --- | --- | --- |
| `checks` | the target table and the naming rule | it lives in `bite-gpui/distribution` |
| `macos` | the Metal backend compiles | the HLSL equivalent: `xcrun metal` runs in a build script, and a build script runs for the **host** |
| `windows` | the Direct3D backend compiles in release | release is the profile whose build script compiles the HLSL with `fxc`, and whose Rust includes the resulting byte arrays — so the release-only arms are compiled here and nowhere else |
| `windows-path-a` | the Direct3D arm's three rows | they need a device and a swap chain; a hosted runner has WARP, which is why a pass says "mechanically possible on one adapter" |
| `windows-interop` | the cross-device arm — a Direct3D 12 producer's shared texture, composited through the Direct3D 11 renderer | it needs a device and a shared handle; the same WARP caveat as `windows-path-a`, one device boundary further out. Debug is enough here, because `gpui_interop`'s test adds no HLSL of its own |
| `macos-path-a` | the Metal arm's two rows | they need a Metal device, and a hosted runner's is real rather than emulated — the presentation probe records `Apple Paravirtual device`. Debug is enough here, because `gpui_apple`'s build script compiles the shaders in every profile |
| `tests` | the engine's and authoring's rows | they need no display and no GPU |

**Locally.** The Windows backend cannot be built natively, but it type-checks against the target:

```sh
cargo check  -p gpui_windows --target x86_64-pc-windows-msvc --tests --features test-support
cargo clippy -p gpui_windows --target x86_64-pc-windows-msvc --tests --features test-support -- --deny warnings
cargo fmt -p gpui_windows -- --check
```

Three traps that cost time if they are not known:

- `cargo check -p gpui --target x86_64-pc-windows-msvc` fails in `crates/gpui/build.rs` with
  `NotAttempted("llvm-rc")`, because embedding Windows resources needs a resource compiler Linux
  does not have. Check `-p gpui_windows` instead; the facade is CI's job.
- In debug, the HLSL is compiled at run time by `D3DCompileFromFile`
  (`crates/gpui_windows/src/directx_renderer.rs:2369` is the release branch that replaces it) and
  the byte arrays come from `crates/gpui_windows/build.rs`, whose shader compilation is
  `#[cfg(all(target_os = "windows", not(debug_assertions)))]`. So a shader error and a `from_bytes`
  arm are both invisible to a local debug check.
- `cargo fmt -p gpui_windows -- --check` reports two spots — in `events.rs` and `window.rs` — that
  predate the branch. They are not in the way, but a formatting gate would have to deal with them.

**Measuring anything that needs a device** is a probe, not a test, and the convention is
[`../../decisions/README.md`](../../decisions/README.md) — see how
[`../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) is filed as
evidence beside the decision it produced. A probe still running lives with the work it gates.

## 6. The decisions these milestones rest on

- **The rendezvous decision** (M1): [`0004`](../../decisions/0004-producer-device-rendezvous.md),
  superseded by [`0006`](../../decisions/0006-renderer-owned-device.md) — the renderer owns the device
  and the canvas lends it.
- **The surface decision** (M3/M5): [`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md),
  decided and built. The design is [`surfaces.md`](surfaces.md); the plan and probes are
  [`surface-plan.md`](surface-plan.md).
- **The layout decision** (M5's Linux half):
  [`0007`](../../decisions/0007-dmabuf-importer-native-layout-guard.md), decided and implemented. It
  is why a linear dma-buf is refused unless the driver produces its layout.
- **The GPU decision** (M8): [`0008`](../../decisions/0008-gpu-preference-and-enumeration.md), decided
  and implemented for the preference, enumeration and the dma-buf import capability report.
- **The bridge decision** (M6): 0002 deferred it; 0005 schedules it in a downstream crate, and
  [`surface-plan.md`](surface-plan.md) P1/P5/P6 are the probes that decide whether the direction
  worth having works at all.
