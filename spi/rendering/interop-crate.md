# The interop crate: `gpui-interop`

- **Status:** proposed — a plan for the downstream crate [`surfaces.md`](surfaces.md) §5 names. A
  provisional crate is being built **in the fork** (`crates/gpui_interop`), so it can iterate
  quickly; it will move to its own home once its shape is settled. That home is issue
  [`0006`](../../issues/0006-surface-interop.md), and the workstreams and probes it rests on are
  [`surface-plan.md`](surface-plan.md) W5–W6 and P1–P6.
- **Assumes:** the surface element is in the facade (`surface()`), and a window lends its renderer's
  device. **Both are on the upstream path** (§1), and that is what decides this crate's shape.
- **Not a seam:** it replaces no `SceneRenderer` and decorates no `FramePipeline` — it is the
  "crate that is neither" ([`../../architecture/extension-tiers.md`](../../architecture/extension-tiers.md)).
- **A working name.** `gpui-interop` is a placeholder; the crate's real name is part of question 1 of
  issue `0006`.

## 1. The constraint that shapes it: `PaintSurface` is going upstream

The core half of the surface work — the `SurfaceSource` variants, the `surface()` element,
`draw_surfaces` on each renderer, and the `PixelBuffer`/offscreen contract — is being contributed to
**upstream `gpui`**, because it is what upstream asked for. The fork carries it only until it lands.
That fact fixes this crate's posture, and it is the reason the crate is *downstream* rather than a
layer of ours:

- **It depends on the upstreamed API, not on a fork layer.** Its inputs are `surface()`
  (`crates/gpui_authoring/src/elements/surface.rs:35`), the payload enum
  (`crates/gpui_engine/src/scene.rs`), and the answer to
  `device_any` (`crates/gpui_authoring/src/window.rs:3031`) — the shapes that go upstream. It must
  **not** touch the fork-only `CustomRenderPrimitive` route
  (`crates/gpui_engine/src/custom_render.rs:41`), which the unification supersedes: a dependency on it
  would strand the crate the moment that layer is replayed away.
- **The device accessor is part of the same upstream push.** A crate that must allocate an
  `ID3D11ShaderResourceView` on GPUI's device has to *reach* that device, so the typed spelling of
  `device_any` — `DirectXWindowExt::d3d11_device()`, per
  [`0004`](../../decisions/0004-producer-device-rendezvous.md) — is a core addition, not the crate's.
  If only the element is upstreamed and the accessor is not, the crate cannot be built against
  upstream at all; the two travel together, and the plan should open them together.
- **The payload types are upstream surface.** The crate names `ID3D11ShaderResourceView`, a
  `wgpu::TextureView`, a raw `id<MTLTexture>` and a dma-buf fd; the *element* must accept them. That
  is the commitment `0004` records under "what would reopen this", and it is why the crate cannot
  invent its own transport type.

The crate's lifetime is therefore: **against the fork's facade now, against upstream `gpui` once PR 1
and PR 2 land, with the same source either way** — `gpui` is the facade in both, so the crate selects
nothing.

## 2. What it is for

One job: a producer whose device is **not** GPUI's renderer's — another API, another adapter, another
process — turns a frame it rendered into a `SurfaceSource` the UI samples, with no CPU copy. The
same-device case needs none of it: the application hands the SRV/TextureView straight to `surface()`.

The cases that need it: a `wgpu`/Direct3D 12 engine against GPUI's Direct3D 11 renderer; a hardware
decoder on its own context; a webview in a sandboxed process; and, on Linux, any dma-buf producer.
Each is the "other device" row of [`surfaces.md`](surfaces.md) §2, and each is the same three-step
interchange in a platform's clothes: match the adapter, move a handle, order the queues.

## 3. Modules (proposed)

| module | job | platform |
| --- | --- | --- |
| `adapter` | match a foreign device to the window's renderer before anything is shared | all |
| `windows` | Direct3D 12 → Direct3D 11 shared NT handle — allocate shareable, `CreateSharedHandle`, `OpenSharedResource1`, SRV — and the `ID3D12Fence`↔`ID3D11Fence` bridge | windows |
| `macos` | build an `MTLTexture` over an `IOSurface` with `objc2-metal`, adopt it into wgpu, order with `MTLSharedEvent` | macos |
| `linux` | dma-buf alloc/import (fd + fourcc + modifier) via `VK_KHR_external_memory_fd` or `EGL_LINUX_DMA_BUF_EXT`, ordered by a dma-fence | linux |
| `guest` | headless GPUI on a worker thread, handing a frame to a foreign loop | all |

Each platform module is written only if its probe passes: `windows` on **P5/P6/P9** for the Host Mode
direction (D3D12/`wgpu` → D3D11) and on **P1** for its reverse half (D3D11 → `wgpu`, which gates W6 and
not W5); `macos` on **P2**; `linux` on **P3**.

`adapter` is the one module on every platform, but only its *interface* is universal — the mechanism is
Windows-only. There a foreign device is matched to the renderer by LUID; on macOS there is nothing to
match, because wgpu's adapter *is* the `MetalRenderer`'s `MTLDevice`
([`producer-reach.md`](producer-reach.md) §7); on Linux it is device-node selection. So `attach` must
not become LUID-shaped: the honest off-Windows answer is a caller-supplied device or a `None`, not a
failed match. That is part of why P6 runs before §4's shape is frozen.

## 4. Public surface (a sketch)

```rust
pub fn attach(window: &gpui::Window) -> Result<Interop, Unavailable>;

pub struct Interop { .. }
pub struct Adapter { .. }
pub struct SurfacePool { .. }
pub struct Frame { .. }

impl Interop {
    /// The producer-side device, matched to the window's renderer (the P6 question).
    pub fn adapter(&self) -> Adapter;
}

impl Adapter {
    /// A wgpu device/queue the producer can render on, if one matches.
    pub fn wgpu(&self) -> Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)>;
    /// A ring of shareable targets the producer renders into.
    pub fn pool(&self, size: Size<DevicePixels>, format: Format) -> Result<SurfacePool>;
}

impl SurfacePool {
    /// A target to render into this frame.
    pub fn acquire(&mut self) -> Frame;
    /// The same frame as a value the `surface()` element takes.
    pub fn surface(&mut self, frame: &Frame) -> gpui::SurfaceSource;
    /// Signal the fence and release the target back to the ring.
    pub fn submit(&mut self);
}
```

Three shapes are the point, and each follows from a decision already taken:

- **`surface()` returns the type the element takes**, an upstream type — so the application never names
  the transport, exactly as an in-process producer does not.
- **`attach` is fallible, and `Adapter::wgpu` is `Option`.** The honest answer when no adapter matches
  is `None`, not a panic; P6 decides whether a match can be found at all or is the application's.
- **Acquire/submit is a ring, not a fence per frame.** Ordering rides on a signal the producer
  submits and GPUI waits on, or on one queue when the device is shared — the synchronisation
  [`surfaces.md`](surfaces.md) §2 says the same-device route does not need and this one does.
- **`attach` should be able to take a device the caller already has.** P6 decides whether a matching
  adapter can be found at all; on a two-GPU laptop, or a driver that hides the LUID, it may not be, and
  the API's escape is the application handing over the device or adapter it owns. That is why P6 runs
  *before* this shape is frozen ([`surface-plan.md`](surface-plan.md) §2). **Answered:** a LUID match is
  available on the probe machine, so the default resolves one and this stays an escape rather than the
  primary path ([evidence](../../decisions/windows-adapter-luid-probe.md)); the two-GPU choice is the
  case still open.

**Device loss is part of the contract, not an edge.** A Windows driver resets on sleep, a monitor
unplug, a DPI change or a GPU timeout (TDR), and GPUI recreates its device; every handle, fence and
view the pool holds is then a dangling reference. The pool must observe the renderer's `device_lost` →
`recover` and re-negotiate — the acceptance case [`surface-plan.md`](surface-plan.md) §2 P9 names —
rather than leave the application holding a surface the window can no longer sample. P9 ran and
passed ([evidence](../../decisions/windows-device-loss-probe.md)); it adds that the re-negotiation
must **re-enumerate** — a device loss recreates the adapter instance and the LUID changes with it, so
the pool may cache neither the adapter nor a LUID across the loss.

## 5. What it does not do

- **No renderer arm.** It produces a `SurfaceSource`; `draw_surfaces` samples it, and that is
  upstream core, not this crate.
- **No seam, no pipeline.** It is reached by the application, not installed by it.
- **No same-device helper.** That case needs no handle; a helper would only hide that it does not.
- **No readback.** The CPU case is `read_pixels`, upstream.

## 6. Staging, and the open questions

- **Before upstream merges**, the crate builds against the fork's facade — the same `gpui` name, a
  pinned version. **After**, against upstream. The dependency range should say so, and the crate
  should never grow a `gpui_engine` dependency to bridge the two.
- **Question 1** (issue `0006`): where the crate lives — a new repository, which then gains a line in
  [`../../repositories.md`](../../repositories.md) and [`../../online-resources.md`](../../online-resources.md),
  or a crate in an existing one.
- **Question 2**: which crate publishes the payload types, and at which version. The concrete
scaffold — layout, dependencies, and the two tests — is [`interop-scaffold.md`](interop-scaffold.md).

## 7. What would reopen it

- **Upstream taking the element but not the accessor.** Then the crate cannot reach the device and
  would need a fork shim — which breaks the posture in §1 and should send the accessor upstream
  instead, not patch around it.
- **wgpu gaining shareable creation.** The `windows` and `macos` modules collapse from a handshake to
  a descriptor, and most of W5 with them.
