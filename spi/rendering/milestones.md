# Milestones

- **Status:** the handoff. What is built, where it is, what is next in what order, and the gates a
  resumer runs. Written 2026-09-29, against `bite_v1.22.0-pre` at `a2884d2de7`; a citation's line
  numbers move whenever that ref does, and `script/check-citations` prints what each one now points
  at.
- **Read with:** the chapter order in [`README.md`](README.md) for the design — this document is the
  state — and [`producer-reach.md`](producer-reach.md), which is the one gap large enough to have a
  chapter rather than a line here.

## 1. Where the work is

| what | where |
| --- | --- |
| the canonical ref, and what every citation resolves against | `bite_v1.22.0-pre`, tip `a2884d2de7` (the merge of #5) |
| what the canonical ref already carries | the renderer seam (#4) and the fork's CI file (#5). **None** of Path A |
| Path A | `bite_v1.22.0-pre-path-a`, five commits, PR [#6](https://github.com/bite-gpui/bite-gpui/pull/6) |
| the Direct3D arm, stacked on it | `bite_v1.22.0-pre-path-a-directx`, two commits, PR [#7](https://github.com/bite-gpui/bite-gpui/pull/7) |
| the checkout the work is in | `.tools/worktrees/wt-seam` in the zed clone |
| the other repositories, and what each is for | [`../../references.md`](../../references.md) |

`#7` is stacked on `#6` rather than on the canonical ref, so its base has to be retargeted to
`bite_v1.22.0-pre` once `#6` merges. Both are green on the fork's CI; neither is merged.

## 2. What is built

| item | chapter | where it is | evidence |
| --- | --- | --- | --- |
| the renderer seam: `PlatformRenderer`, the typed `RendererTarget`, the factory, the native hooks | [`renderer-seam.md`](renderer-seam.md) | the canonical ref | PR #4 |
| the offscreen contract: the three methods ungated, `render_scene` split from `read_pixels`, `PixelBuffer` in place of `image::RgbaImage` | [`renderer-seam.md`](renderer-seam.md) §4 | `bite_v1.22.0-pre-path-a` | `gpui_engine`'s 7 tests, ubuntu CI |
| Path A's scene half: `CustomRenderPrimitive::Texture`, `ImportedTextureHandle`, `to_quad_record`, `ObservingRenderer` | [`foreign-texture.md`](foreign-texture.md) | same branch | `cargo test -p gpui_engine --lib` |
| Path A's authoring half: `Window::paint_imported_texture`, `painted_imported_textures` | [`foreign-texture.md`](foreign-texture.md) §3 | same branch | the radii row in `gpui_authoring`'s 347 tests |
| the wgpu arm, the offscreen target and the readback | [`foreign-texture.md`](foreign-texture.md) §7 | same branch | 3 rows — the sRGB round trip, the ordering, the device identity — skipped with a logged warning where the machine has no adapter |
| the Metal arm | [`foreign-texture.md`](foreign-texture.md) §7 | same branch | **type-checked and shader-compiled only — no test exists** |
| the Direct3D arm and its CI job | [`foreign-texture.md`](foreign-texture.md) §7 | `bite_v1.22.0-pre-path-a-directx` | 3 rows on `windows-latest`, WARP |
| **the producer's reach** | [`producer-reach.md`](producer-reach.md) | **nothing** | **no platform, no configuration, no test** |

## 3. The gaps beside the big one

Small, independent, and worth doing while the rendezvous is being decided:

- **`DirectXRenderer`'s offscreen override is test-gated.** In `crates/gpui_windows/src/directx_renderer.rs`
  it overrides only `render_scene_to_image`, under `#[cfg(any(test, feature = "test-support"))]`, so
  in a normal build the Direct3D renderer reports offscreen rendering unsupported and implements
  neither `render_scene` nor `read_pixels`. `WgpuRenderer` overrides all three ungated; the Direct3D
  renderer should have the same shape.
- **Metal has no test.** Its arm in `crates/gpui_apple/src/metal_renderer.rs` is the only one of the
  three that nothing exercises, and `MetalRenderer::new_headless` exists, so the row is writable in
  `gpui_wgpu`'s image.
- **The pull-request test job's filter is narrow.** `bite-ci.yml`'s `tests` job runs
  `cargo test -p gpui_authoring -p gpui_engine --lib` and nothing else, so every other crate's tests
  are local-only — `gpui_wgpu`'s need a GPU adapter, and its older tests fail rather than skip
  without one, which is a decision about what a GPU-less runner is for rather than a wiring change.
  Issue [`0003`](../../issues/0003-scheduled-builds.md) holds the wider version of the question.
- **The canonical ref's citations move when it advances.** Anything that lands on
  `bite_v1.22.0-pre` re-derives every citation in this repository.

## 4. What is next, in order

**M1 — the producer's reach.** [`producer-reach.md`](producer-reach.md). Blocks M3, and blocks any
claim that Path A is usable. Its first step is a decision, not code: which direction the rendezvous
takes (gpui lends the device, the application lends one, or both), whether it is an erased accessor
on the renderer's trait reached through the window, or a shared slot per platform. The chapter sets
out the two shapes and what each costs.

**M2 — the gaps in §3.** Independent of M1, cheap, and it keeps the Direct3D and Metal arms honest.

**M3 — `GpuCanvas`.** [`../authoring/gpu-canvas.md`](../authoring/gpu-canvas.md). The surface an
application meets, and the reason M1 comes first: a canvas whose callback can only return a token
no application can construct is a demonstration.

**M4 — Path B.** [`inline-commands.md`](inline-commands.md). Unstarted. It shares the primitive with
Path A and needs no device export by definition — the producer draws into the window's own pass —
so it is the one remaining item that is not behind M1.

**M5 — the cross-device bridge.** 0002's deferred tier, and it needs its own decision plus a probe
before its own code: the direction a D3D11 producer feeding a wgpu consumer needs is the one neither
existing probe measured ([`producer-reach.md`](producer-reach.md) §6). Not on the critical path for
anything above.

## 5. The gates

**`.meta`.** `script/check-citations` — 308 citations, 0 unresolved when this was written. It
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
  (`crates/gpui_windows/src/directx_renderer.rs:1889` is the release branch that replaces it) and
  the byte arrays come from `crates/gpui_windows/build.rs`, whose shader compilation is
  `#[cfg(all(target_os = "windows", not(debug_assertions)))]`. So a shader error and a `from_bytes`
  arm are both invisible to a local debug check.
- `cargo fmt -p gpui_windows -- --check` reports two spots — in `events.rs` and `window.rs` — that
  predate the branch. They are not in the way, but a formatting gate would have to deal with them.

**Measuring anything that needs a device** is a probe, not a test, and the convention is
[`../../decisions/README.md`](../../decisions/README.md) — see how
[`../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) is filed as
evidence beside the decision it produced. A probe still running lives with the work it gates.

## 6. What has no home yet

- **The rendezvous decision** (M1): it amends 0002's "the rendezvous is one slot, and it works both
  ways", which the code does not implement.
- **The bridge decision** (M5): 0002 defers it, and the second probe answers whether the direction
  worth having works at all.
- **Metal's producer half**: [`foreign-texture.md`](foreign-texture.md) §8, unresolved since before
  the arms were written.
