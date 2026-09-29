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
| Path A, whole | `bite_v1.22.0-pre-path-a`, fourteen commits, PR [#6](https://github.com/bite-gpui/bite-gpui/pull/6) |
| the checkout the work is in | `.tools/worktrees/wt-seam` in the zed clone |
| the other repositories, and what each is for | [`../../references.md`](../../references.md) |

`#6` is the only pull request left open, and it is the whole path: the Direct3D arm was stacked on
it (`bite_v1.22.0-pre-path-a-directx`, PR [#7](https://github.com/bite-gpui/bite-gpui/pull/7)) and
the producer's reach on top of that (`bite_v1.22.0-pre-device-rendezvous`, PR
[#8](https://github.com/bite-gpui/bite-gpui/pull/8)), and both were replayed into `#6` rather than
merged anywhere else — so the two head branches are obsolete, and `#6`'s base is already the
canonical ref. `#6` is green on the fork's CI; what is left for it is the review, not a stack.

## 2. What is built

| item | chapter | where it is | evidence |
| --- | --- | --- | --- |
| the renderer seam: `PlatformRenderer`, the typed `RendererTarget`, the factory, the native hooks | [`renderer-seam.md`](renderer-seam.md) | the canonical ref | PR #4 |
| the offscreen contract: the three methods ungated, `render_scene` split from `read_pixels`, `PixelBuffer` in place of `image::RgbaImage`, and all three renderers implementing it in the same shape | [`README.md`](README.md) | `bite_v1.22.0-pre-path-a` | `gpui_engine`'s 7 tests, ubuntu CI. Direct3D's "offscreen" is its window's swap chain — it has no headless constructor — so its `render_scene` is "rendered and not presented", which is what its rows already build a hidden window for |
| Path A's scene half: `CustomRenderPrimitive::Texture`, `ImportedTextureHandle`, `to_quad_record`, `ObservingRenderer` | [`foreign-texture.md`](foreign-texture.md) | same branch | `cargo test -p gpui_engine --lib` |
| Path A's authoring half: `Window::paint_imported_texture`, `painted_imported_textures` | [`foreign-texture.md`](foreign-texture.md) §3 | same branch | the radii row in `gpui_authoring`'s 347 tests |
| the wgpu arm, the offscreen target and the readback | [`foreign-texture.md`](foreign-texture.md) §7 | same branch | 3 rows — the sRGB round trip, the ordering, the device identity — skipped with a logged warning where the machine has no adapter |
| the Metal arm | [`foreign-texture.md`](foreign-texture.md) §7 | same branch | 2 rows on `macos-14` — the colour round trip and the boundary check — plus the shader compile the `macos` job already did |
| the Direct3D arm and its CI job | [`foreign-texture.md`](foreign-texture.md) §7 | same branch | 3 rows on `windows-latest`, WARP |
| the producer's reach: `Window::device_any`, and a token builder on each of the three arms (`ImportedTextureExt`, `DirectXTextureExt`, `MetalTextureExt`) | [`producer-reach.md`](producer-reach.md) | same branch | the Direct3D and Metal rows take their device through the accessor — the route an application has — instead of the renderer's own field |

## 3. The gaps beside the reach

Small and independent, and none of them blocks anything in §4.

- **A wgpu producer on macOS is unmeasured.** `WgpuRenderer` cannot be installed in a macOS window
  — its `PlatformRenderer` impl is cfg'd out there, because a macOS window's renderer must answer
  `MacSceneRenderer` and a wgpu renderer has no `layer_ptr` — so a macOS window's consumer is always
  `MetalRenderer` and its token always the raw handle. A wgpu producer would have to render on
  GPUI's own `MTLDevice` to share it, which is `wgpu-hal`'s
  `metal::Device::device_from_raw` and `wgpu::Instance::create_adapter_from_hal`, and nothing has
  run it; see [`producer-reach.md`](producer-reach.md) §7.
- **Metal cannot enforce the same-device rule.** Each arm checks what its sampler needs at the
  boundary, but only Direct3D refuses a texture from *another* device by name — wgpu panics inside
  its own storage, which is loud rather than named, and Metal has no API for it at all, because a
  resource does not expose the device that made it. So on macOS that half of the rule rests on the
  application; it is not closable by writing code
  ([`producer-reach.md`](producer-reach.md) §3).
- **The pull-request test job's filter is narrow.** `bite-ci.yml`'s `tests` job runs
  `cargo test -p gpui_authoring -p gpui_engine --lib` and nothing else, so every other crate's tests
  are local-only — `gpui_wgpu`'s need a GPU adapter, and its older tests fail rather than skip
  without one, which is a decision about what a GPU-less runner is for rather than a wiring change.
  Issue [`0003`](../../issues/0003-scheduled-builds.md) holds the wider version of the question.
- **The canonical ref's citations move when it advances.** Anything that lands on
  `bite_v1.22.0-pre` re-derives every citation in this repository.

## 4. What is next, in order

**M1 — the producer's reach. Done.** [`producer-reach.md`](producer-reach.md) and
[`0004`](../../decisions/0004-producer-device-rendezvous.md). `Window::device_any` is built on all
three renderers, so the decision that was M1's first step is taken and the accessor exists; what
each platform still lacks is [`producer-reach.md`](producer-reach.md) §7, and none of it blocks
anything below.

**M2 — the gaps in §3. Done for Path A.** The Metal token builder, the exact encoder, the arm's two
rows, the Direct3D renderer's offscreen shape, and the wgpu token's reach through the facade all
landed. What is left in §3 is the test filter and the citation churn, neither of which is Path A.

**M3 — `GpuCanvas`.** [`../authoring/gpu-canvas.md`](../authoring/gpu-canvas.md). The surface an
application meets; with M1 built, the token its callback returns is one an application can
construct, which is what makes the canvas a capability rather than a demonstration.

**M4 — Path B.** [`inline-commands.md`](inline-commands.md). Unstarted. It shares the primitive with
Path A and needs no device export by definition — the producer draws into the window's own pass —
so it is the one remaining item that is not behind M1.

**M5 — the cross-device bridge.** 0002's deferred tier, and it needs its own decision plus a probe
before its own code: the direction a D3D11 producer feeding a wgpu consumer needs is the one neither
existing probe measured ([`producer-reach.md`](producer-reach.md) §6). Not on the critical path for
anything above.

## 5. The gates

**`.meta`.** `script/check-citations` — 324 citations, 0 unresolved when this was written. It
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

- **The rendezvous decision** (M1): [`0004`](../../decisions/0004-producer-device-rendezvous.md),
  decided and built. It supplies the gpui→app half of 0002's "the rendezvous is one slot, and it
  works both ways", which the code did not have.
- **The bridge decision** (M5): 0002 defers it, and the second probe answers whether the direction
  worth having works at all.
- **The macOS producer route that needs a probe**: a wgpu producer on GPUI's own `MTLDevice`
  ([`producer-reach.md`](producer-reach.md) §7). Metal's own producer half is closed — the device is
  lent and `MetalTextureExt` builds the token — so what has no home document is the measurement, not
  the code.
