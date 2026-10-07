# Verification: what can be asserted, and where

- **Status:** proposed, as the test plan for
  [`renderer-seam.md`](renderer-seam.md), [`foreign-texture.md`](foreign-texture.md),
  [`inline-commands.md`](inline-commands.md) and [`gpu-canvas.md`](../authoring/gpu-canvas.md).
  Path A's rows below are written: the encoding and scale rows in `gpui_authoring`, the three that
  need a device in `gpui_wgpu`, the three that run in `gpui_windows`' tests on a Windows runner, and
  the two in `gpui_apple`'s on a macOS one. The producer's reach is implemented too, so the rows
  still *proposed* rather than written are Path B's.
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
| Path A colour space | a washed-out composite | an sRGB fixture round-trips byte for byte; with the fragment's re-encode missing the same fixture shifts by ≈2.2 (`[200, 100, 50]` reads back as `[147, 32, 8]`) |
| Path A ordering | a texture sampled before the pass that fills it | a frame whose producer submits in its paint callback composites the texture; the same frame with that submission removed does not |
| Path A device identity | a texture from a second device | it fails loudly, or cannot be checked at all: wgpu refuses a resource from another device by panicking inside its own storage, Direct3D refuses it by name (`CreateShaderResourceView`), and Metal has no API for it — a resource does not expose the device that made it — so on macOS that half of the rule is the application's to keep |
| the producer's reach | Path A being a demonstration rather than a capability | an application holding only a `Window` — not a renderer — obtains the device its texture is made on: `device_any` returns it, and a token built on it composites. The Direct3D colour row already takes its device this way; what an application-only row would add is the downcast, which needs no device of its own |
| Outcome B end to end | a Windows configuration nothing has ever composited | on `windows-latest`, a window with `WgpuRenderer` installed composites a pushed texture, and the same window with the default renderer reports it unsupported |
| Path B state isolation | UI corruption *after* an injected draw | a quad drawn after an injected command matches the same quad with no injection |
| Path B scissor | drawing outside the element | an injected command cannot paint outside its device scissor rect |
| device loss | a `SurfaceLost` panic | `device_lost()` → `recover()` → a frame draws |
| the bridge after device loss | a dangling shared handle after a driver reset | after `device_lost` → `recover`, the interop pool re-negotiates and a frame composites, rather than panicking ([`surface-plan.md`](surface-plan.md) §2 P9) |
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
  Two harnesses answer it and they are not interchangeable. The `TestPlatform` factory installs a
  renderer in a window, which is how a row about the *window* is asserted — the seam's own test is
  the proof the pattern works. Path A's readback rows are asserted elsewhere instead, on
  `WgpuRenderer::new_offscreen` in `gpui_wgpu`'s own tests, because what they need is a device and
  the renderer that owns it: a producer's texture has to be made *on that device* before any of them
  can run, with no window involved at all. Those tests skip with a logged warning where the machine
  has no adapter, so they are gates only where a GPU or a software rasteriser exists. That shape is
  also why they say nothing about the producer's reach: the test *is* the producer, because a test in
  the renderer's crate can hold the concrete renderer, which is exactly the door an application does
  not have ([`producer-reach.md`](producer-reach.md) §4).
- **On a macOS host.** The Metal arm, and the `MacSceneRenderer` hook — nothing else catches a
  mis-wired native hook. Two rows run there now, on `macos-14`: the colour round trip and the
  boundary check the producer's `MetalTextureExt` answers for. The device-identity row is the one
  row macOS cannot have, for §1's reason.
- **On a Windows host.** The Direct3D arm and `WinSceneRenderer`, and — for
  [`foreign-texture.md`](foreign-texture.md) — the whole Windows configuration. The arm's three rows
  run on `windows-latest` and they pass there, on WARP, which is why they are a result about one
  adapter rather than about hardware. The producer's reach is exercised there too — the colour row
  takes its device through `device_any` — so what Windows still cannot show is the *application's*
  route, because a test is its own producer
  ([`producer-reach.md`](producer-reach.md) §4).

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
| `cargo test --release -p gpui_windows imported_texture` and `cargo test --release -p gpui_windows surface` | windows-latest | Path A's Direct3D rows: they need a device and a swap chain, which no Linux gate has and no cross-compile reaches. The two filters are the external-pixel rows — the imported-texture rows and the surface rows — because the crate's older tests have never run on a runner. Each filter asserts that at least one test ran, so neither can pass vacuously |
| `cargo test -p gpui_interop a_shared_d3d12_surface` | windows-latest | the cross-device arm: a Direct3D 12 producer's shared texture, composited through GPUI's Direct3D 11 renderer and read back byte for byte — the rows above taken one device boundary further out. It needs a device and a shared handle, so the same WARP caveat applies. Debug, not release: `gpui_interop`'s test adds no HLSL of its own, so there is no `fxc` build-script output it needs, and the renderer it drives compiles its shaders at run time in debug. Filtered to the test's own name and guarded the same way — and with the same blind spot: the test skips, by returning `Ok`, where there is no Direct3D 12 device, so a pass proves a test ran, not that the path was exercised |
| `cargo test -p gpui_apple imported_texture` | macos-14 | Path A's Metal rows: they need a Metal device, and a hosted runner's is a real one — `Apple Paravirtual device` — so unlike the Windows rows there is no emulation caveat. Filtered to those two rows and guarded the same way. Debug rather than release, because `gpui_apple`'s build script compiles the shaders in every profile |

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
  (`crates/gpui_windows/src/directx_renderer.rs:2002`). The native job is release, so it
  differs where it matters.
- **macOS:** the same trick does not work. `crates/gpui_apple/src/metal_renderer.rs:38`
  includes `OUT_DIR/shaders.metallib` in every profile, and only a macOS host produces
  it, so a Linux check of the Apple crates type-checks the Rust and not the shaders.

The release gate is a different thing and does not cover this: `verify.yml` verifies a
*target* — its legs are Linux and macOS — and it never compiles the Windows backend at
all. A release receipt therefore says nothing about Direct3D.

## 4. What is not verifiable here, and what a probe is for

- **Real hardware.** The Windows probes run on WARP, which is why
  [`../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) records its
  result as "mechanically possible on one adapter" rather than "works", and why
  [`../decisions/windows-presentation-probe.md`](../../decisions/windows-presentation-probe.md)
  reports its adapter as `Microsoft Basic Render Driver` with `device_type = Cpu`.
- **Adapter identity.** Sharing requires the same physical adapter, and the LUIDs of two
  independently created devices are not comparable from inside either one.
- **The unsolved question.** Whether a wgpu texture can reach GPUI's Direct3D 11 renderer
  needed a native run, and the reading has since moved: wgpu cannot *create* a shareable
  texture, but it can *adopt* one the application allocated. The question is now whether that
  adoption works end to end, which is
  [`../decisions/shared-surface.md`](../../decisions/shared-surface.md).
