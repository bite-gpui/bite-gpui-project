# The `gpui-interop` scaffold

- **Status:** proposed — the crate's concrete shape: its home, layout, dependencies, public API, and
  the test that proves the upstreamed `paint_surface` support is **sufficient**. It complements
  [`interop-crate.md`](interop-crate.md) (the design and the posture) with the thing to build.
- **Read with:** [`interop-crate.md`](interop-crate.md) (why it exists, and the upstreaming constraint),
  [`upstream-prs.md`](upstream-prs.md) (the API it consumes), [`surface-plan.md`](surface-plan.md)
  (the probes that gate each module).
- **Home:** unsettled — issue [`0006`](../../issues/0006-surface-interop.md) question 1. A new
  `gpui-interop` repository is the recommendation; a crate in the fork builds against the facade and
  the W2 branch until then.

## 1. The point of the test

The crate is not only a bridge. Its **first test proves the PR is worth merging**: that a foreign
producer's frame reaches the scene through `surface()` with the two things PR 2 adds — the Windows
`SurfaceSource` variant and `DirectXWindowExt::d3d11_device()`. That test needs **no bridge** and no
second device; it is the same-device case, and it is the acceptance test PR 2 should carry
([`verification.md`](verification.md) §2). If it passes on the PR's branch, the upstreamed feature
does the job; if it does not, the PR is not yet the feature upstream agreed to take.

## 2. Home and workspace

```toml
# Cargo.toml
[package]
name = "gpui-interop"
# the gpui it builds against is the facade — upstream's once PR 2 lands,
# the fork's before that; same name, a pinned version.

[features]
default = []
wgpu = ["dep:wgpu"]              # the cross-API bridge

[dependencies]
anyhow.workspace = true
gpui.workspace = true
gpui_util.workspace = true       # Size / DevicePixels helpers, if not re-exported

[target.'cfg(target_os = "windows")'.dependencies]
windows.workspace = true
wgpu = { workspace = true, optional = true }

[target.'cfg(target_os = "macos")'.dependencies]
objc2-metal.workspace = true
wgpu = { workspace = true, optional = true }

[target.'cfg(target_os = "linux")'.dependencies]
wgpu = { workspace = true, optional = true }
```

**The one dependency rule**, from [`interop-crate.md`](interop-crate.md) §1: it depends on `gpui` and
the platform graphics crates, **never on `gpui_engine`** — the fork-only layer that the upstreaming
makes disappear.

## 3. File layout

```
gpui-interop/
  Cargo.toml
  src/
    lib.rs        // Interop, Adapter, SurfacePool, Frame, attach(), Unavailable
    adapter.rs    // match a foreign device to the window's renderer (P6)
    windows.rs    // D3D12 -> D3D11 NT handle, and the fence bridge (P1, P5)
    macos.rs      // an MTLTexture over an IOSurface, adopted into wgpu (P2)
    linux.rs      // dma-buf import (P3)
    guest.rs      // the headless worker-thread runner (P7)
  tests/
    same_device.rs   // the PR-2 sufficiency test — no bridge, runs today
    bridge.rs        // #[cfg(windows)] the cross-device test — P1/P5/P6-gated
  examples/
    viewport.rs      // a foreign frame in a GPUI window, end to end
```

## 4. Public surface (expanded from [`interop-crate.md`](interop-crate.md) §4)

```rust
/// Negotiate with a window's renderer. Fails when there is nothing to bridge *to*.
pub fn attach(window: &gpui::Window) -> Result<Interop, Unavailable>;

#[derive(Debug)]
pub enum Unavailable {
    NoDevice,     // the renderer lends nothing (offscreen, or a foreign renderer)
    NoAdapter,    // no wgpu adapter matches the window's device (P6)
    Unsupported,  // the platform module is not built (a feature/cfg not enabled)
}

pub struct Interop { /* hold the window's device, erased */ }

impl Interop {
    /// The producer-side device, matched to the window's renderer.
    pub fn adapter(&self) -> Adapter;
}

pub struct Adapter { /* backend-specific */ }

impl Adapter {
    /// A wgpu device/queue the producer can render on, if one matches (P6).
    pub fn wgpu(&self) -> Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)>;
    /// A ring of shareable targets the producer renders into.
    pub fn pool(&self, size: Size<DevicePixels>) -> Result<SurfacePool>;
}

pub struct SurfacePool { /* the ring + the fence */ }

impl SurfacePool {
    /// A target to render into this frame.
    pub fn acquire(&mut self) -> Frame;
    /// The same frame as a value the `surface()` element takes.
    pub fn surface(&mut self, frame: &Frame) -> gpui::SurfaceSource;
    /// Signal the fence and release the target back to the ring.
    pub fn submit(&mut self);
}

pub struct Frame { /* backend-specific */ }
```

Three things to hold to, all from the design: **`attach` is fallible and `Adapter::wgpu` is
`Option`** (P6 may say no match exists, and `attach` may then take a caller-supplied device);
**`surface()` returns the type the element takes**, so the application never names the transport; and
**`acquire`/`submit` is a ring**, ordered by a fence the producer submits and GPUI waits on.

## 5. The modules, and what gates each

| module | types | gate |
| --- | --- | --- |
| `adapter` | `Adapter` — the LUID match, and the caller-supplied fallback | **P6** (run first) |
| `windows` | `SharedSurface` (D3D12 allocate → `CreateSharedHandle` → `OpenSharedResource1` → SRV), `Fence` (`ID3D12Fence`↔`ID3D11Fence`) | **P1**, **P5** |
| `macos` | `AdoptedSurface` (an `MTLTexture` over an `IOSurface` built with `objc2-metal`, adopted into wgpu) | **P2** |
| `linux` | `DmaBufSurface` (fd + fourcc + modifier → `VkImage`) | **P3** |
| `guest` | `Guest` (a headless worker thread; a `Frame` out) | **P7** |

Every module is written only if its probe passes; that is the whole reason the probes come first
([`surface-plan.md`](surface-plan.md) §2).

## 6. Building before the PR lands

The crate calls `surface(srv)`, which does not exist until the fork's W2 retarget or upstream's PR 2.
Two ways to keep it building:

- **(a) Build against the branch.** Point the `gpui` dependency at the W2 branch (the fork's, then
  upstream's). This is the faithful one and the one the sufficiency test wants.
- **(b) A `legacy-primitive` shim.** A feature that routes the same-device hand-off through the fork's
  *current* second primitive — `paint_imported_texture` (`crates/gpui_authoring/src/window.rs:5000`)
  and `CustomRenderPrimitive::Texture` (`crates/gpui_engine/src/custom_render.rs:41`) — so the device
  accessor (`device_any`, `crates/gpui_authoring/src/window.rs:3031`) can be smoke-tested *today*,
  before W2. It is deleted the moment (a) is available; it exists so the crate is not blocked on a
  branch that is not merged.

## 7. The two tests

**`tests/same_device.rs` — the PR-2 sufficiency test.** No bridge, no second device.

1. Open a window (the fork's test harness on `windows-latest`, or a real one).
2. `let device = window.d3d11_device().ok_or(Unavailable::NoDevice)?;` — the accessor PR 2 adds.
3. Allocate an `ID3D11Texture2D` (`B8G8R8A8_UNORM`), clear it to a known colour, make an SRV.
4. Paint `surface(srv)` inside a `div()`.
5. Render a frame and read it back; **assert the colour lands in the surface's rect** — the
   straight-through round trip, byte for byte.

*Pass:* PR 2 and the accessor are **sufficient** for the simplest interop case, which is the claim the
record has been making. *This test is also what PR 2 should carry*, so upstream's reviewers see the
feature being used.

**`tests/bridge.rs` — the crate's own test.** `#[cfg(target_os = "windows")]`, gated on **P1** (route)
and **P5** (fence): a second device renders into a shared handle, the bridge opens it, and the same
rect assertion holds. Skipped with a logged warning where the route is unavailable — the pattern the
wgpu rows already use.

## 8. What the scaffold does not include

- **No renderer arm.** `draw_surfaces` is PR 2's; the crate produces a `SurfaceSource` and nothing else.
- **No seam, no pipeline.** It is reached by the application, not installed by it.
- **No same-device helper.** The same-device case needs none — the sufficiency test is the exception,
  and it is a test.
- **No readback.** The CPU path is `read_pixels`, upstream.
