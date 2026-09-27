# Verification: what can be asserted, and where

- **Status:** proposed, as the test plan for
  [`renderer-seam.md`](renderer-seam.md), [`foreign-texture.md`](foreign-texture.md),
  [`inline-commands.md`](inline-commands.md) and [`gpu-canvas.md`](../authoring/gpu-canvas.md).
- **Why it is a chapter of its own:** most of this feature's failure modes are silent — a
  wrong scale factor, a missed gamma, a leaked pipeline, a device that is not the one the
  texture came from. None is a compile error, and three of the four platforms cannot be
  run from a Linux host.

## 1. The matrix

Each row is an assertion a test can make, not a thing to look at:

| scenario | to prevent | assertion |
| --- | --- | --- |
| resize / surface churn | stale swapchain geometry | after `update_drawable_size`, the next frame's scissor and viewport match the new size |
| DPI / scale factor | a half- or double-scale viewport | `bounds * scale_factor` rounds to the coordinates the platform target reports |
| premultiplied alpha | fringes at rounded corners and antialiased edges | a known RGBA fixture composites to a known pixel on readback |
| Path A colour space | a washed-out composite | an sRGB fixture round-trips without a ≈2.2 gamma shift |
| Path A ordering | a texture sampled before the pass that fills it | a frame whose producer submits in its paint callback composites the texture; the same frame with that submission removed does not |
| Path A device identity | a texture from a second device | the bind fails, and the failure names the mismatch |
| Outcome B end to end | a Windows configuration nothing has ever composited | on `windows-latest`, a window with `WgpuRenderer` installed composites a pushed texture, and the same window with the default renderer reports it unsupported |
| Path B state isolation | UI corruption *after* an injected draw | a quad drawn after an injected command matches the same quad with no injection |
| Path B scissor | drawing outside the element | an injected command cannot paint outside its device scissor rect |
| device loss | a `SurfaceLost` panic | `device_lost()` → `recover()` → a frame draws |
| thread affinity | a GPU context crossing threads | the factory is `!Send` by construction (`Rc`), so the type system is the guard |
| the native hooks | a backend that has not compiled since it was written | each backend compiles on its own platform (§3) |

The readback rows are why headless rendering is load-bearing rather than a convenience:
[`renderer-seam.md`](renderer-seam.md) §4's factory already exists on `TestPlatform`, so
a test can install a renderer, draw a scene, and read the result back without a display.

The two rows that are not assertion-shaped are the last two: thread affinity is a
compile-time property, and "each backend compiles" is CI.

## 2. Where each can be asserted

- **On this host, headless.** Every readback row, the scissor and state-isolation rows,
  and device loss, because the Linux renderer is wgpu and wgpu has no display requirement.
  The `TestPlatform` factory is the existing proof that the pattern works.
- **On a macOS host.** The Metal arm, and the `MacSceneRenderer` hook — nothing else
  catches a mis-wired native hook.
- **On a Windows host.** The Direct3D arm and `WinSceneRenderer`, and — for
  [`foreign-texture.md`](foreign-texture.md) — the whole Windows configuration, since the
  default renderer supports neither path.

## 3. The gate

Every Linux gate can be green while a platform backend does not compile: the backends are
`#![cfg(target_os = "…")]`-gated, so a Linux build compiles them to nothing. The tools
repository records this as a standing gap and names the cross-target gate that would
close it (`.tools/docs/journal/maintenance-report.md` §21). The fork's CI closes it on the
real platforms, because standard GitHub-hosted runners are free for a public repository:

| job | runner | why there, and not cross-compiled |
| --- | --- | --- |
| the distribution checks | ubuntu | the target table and the naming rule, from `bite-gpui/distribution` |
| `cargo check -p gpui` | macos-14 | the only place the Metal shaders are compiled at all: `gpui_apple`'s build script runs `xcrun metal`, and a build script runs for the **host** |
| `cargo check --release -p gpui` | windows-latest | release is the profile whose build script compiles the HLSL with `fxc` and whose Rust includes the result, which is also what makes the job unsatisfiable anywhere but Windows |

**Cross-compiling was tried first and is not enough, though it earned its keep.** A Linux
job checking `--target x86_64-pc-windows-msvc` found
`crates/gpui_windows/src/dialog.rs` importing `gpui::ForegroundExecutor` from a crate
with no `gpui` dependency — invisible to every Linux gate, because nothing built on Linux
compiles that file. But it cannot answer the questions that need a device: the probes
below need Direct3D 11, Direct3D 12 and WARP, none of which exist on Linux, and Wine's
D3D is emulated in a way that does not model shared-handle interop — running the probe
under Wine would answer a question about Wine.

Two asymmetries are worth stating, because they look like the same check and are not:

- **Windows:** a cross-target `cargo check` in debug is nearly equivalent to a native
  one, because in debug the HLSL is compiled at run time by `D3DCompileFromFile` and the
  Rust includes the generated bindings only when debug assertions are off
  (`crates/gpui_windows/src/directx_renderer.rs:1883`). The native job is release, so it
  differs where it matters.
- **macOS:** the same trick does not work. `crates/gpui_apple/src/metal_renderer.rs:36`
  includes `OUT_DIR/shaders.metallib` in every profile, and only a macOS host produces
  it, so a Linux check of the Apple crates type-checks the Rust and not the shaders.

The release gate is a different thing and does not cover this: `verify.yml` verifies a
*target* — its legs are Linux and macOS — and it never compiles the Windows backend at
all. A release receipt therefore says nothing about Direct3D.

## 4. What is not verifiable here, and what a probe is for

- **Real hardware.** The Windows probes run on WARP, which is why
  [`../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) records its
  result as "mechanically
  possible on one adapter" rather than "works".
- **Adapter identity.** Sharing requires the same physical adapter, and the LUIDs of two
  independently created devices are not comparable from inside either one.
- **The unsolved question.** Whether a wgpu texture can reach GPUI's Direct3D 11 renderer
  is the one thing no document could settle; it needed a native run, and the answer is in
  the spike.
