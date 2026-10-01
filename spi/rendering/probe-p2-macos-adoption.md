# P2: macOS adoption of an `IOSurface`

- **Status:** proposed — **not run.** The probe that gates the interop crate's `macos` module
  ([`surface-plan.md`](surface-plan.md) W5, [`interop-crate.md`](interop-crate.md) §3). It lives with
  the work it gates and becomes a `decisions/` evidence record when it runs
  ([`../../decisions/README.md`](../../decisions/README.md)).
- **Question:** on macOS, can an application build an `MTLTexture` over its *own* `IOSurface` and hand
  it to wgpu, so a wgpu producer and a Metal consumer share one surface with no CPU copy?
- **Gates:** W5's `macos` module. **Companion:** [`probe-p1-reverse-bridge.md`](probe-p1-reverse-bridge.md),
  the same question on Windows.

## 1. Why this probe exists

[`../../decisions/shared-surface.md`](../../decisions/shared-surface.md) measured the pool, the token
and the fence on macOS — a `CVPixelBuffer` over an `IOSurface`, the surface id as the token, a clear
*through* the `CVMetalTextureCache` `MTLTexture`, `io_surface::lookup` to reopen the storage, and an
`MTLSharedEvent` ordering two buffers. Its §4 names the one thing left: **adoption** — nothing builds
an `MTLTexture` over an `IOSurface` with `objc2-metal` and adopts it into wgpu, so that half of the
finding rests on the source, not on a run. [`../../decisions/README.md`](../../decisions/README.md)
says it in one line: *"Only macOS adoption is unmeasured."*

## 2. What is already known, so the probe does not re-measure it

- **wgpu adopts, and cannot create.** `texture_from_raw` + `create_texture_from_hal`, and wgpu-hal's
  Metal backend names no `IOSurface` — so the application builds the texture and wgpu wraps it
  ([`surfaces.md`](surfaces.md) §2).
- **A same-device wgpu producer needs no handover.** wgpu's adapter on macOS *is* the
  `MetalRenderer`'s `MTLDevice`, pointer for pointer, so a producer that renders with wgpu on the same
  GPU is already a Metal producer
  ([`../../decisions/macos-wgpu-producer-probe.md`](../../decisions/macos-wgpu-producer-probe.md)).
  P2 is about the *other* case: a surface the application built, which wgpu must adopt.
- **A `CVPixelBuffer` is `IOSurface`-backed only when its attributes ask for it**, through
  `kCVPixelBufferIOSurfacePropertiesKey` — the null-attribute failure `shared-surface.md` §3 records.

## 3. What it must measure, and where

**Hardware.** A Mac; `macos-14` is enough, since the existing macOS probes and CI rows run there on a
real `Apple Paravirtual device`. **Harness.** A scratch crate in the shape of
`probes/macos-wgpu-producer`, the printout the durable record.

## 4. The probes

1. **Allocate and wrap.** Create a 512×512 BGRA `IOSurface`, and build an `MTLTexture` over it with
   **`objc2-metal`** — the type wgpu-hal's Metal backend speaks, *not* the `metal` crate
   `gpui_apple` uses. *Prints the surface id.*
2. **Adopt.** `texture_from_raw` over that `MTLTexture`, then `create_texture_from_hal` into a
   `wgpu::Texture`; print the adapter. *This is the step nothing has run.*
3. **wgpu writes, CPU reads.** Render into the wgpu texture (a clear, or a `write_texture`), then read
   the bytes back through a fresh `CVPixelBuffer` over the same surface. *Pass: the known colour.*
4. **Metal writes, wgpu reads.** The reverse: write through the `CVMetalTextureCache` `MTLTexture`,
   read on the wgpu side. *Pass: the same colour.*
5. **Fence.** Order a Metal writer against a wgpu reader on the same surface with an `MTLSharedEvent`.
   *Pass: no tear, no stall.*

## 5. What each outcome closes

- **Adoption works, both ways** → the `macos` module is real work, and the macOS bridge is a surface
  handshake rather than a same-device-only shortcut.
- **Adoption is refused** → the macOS bridge is same-device only — which
  [`macos-wgpu-producer-probe.md`](../../decisions/macos-wgpu-producer-probe.md) already covers — and
  the `macos` module is dropped from W5.
- **A result on `Apple Paravirtual device`** is *mechanically possible on one adapter*, the caveat
  every probe in this set carries.

## 6. Hazards it must not mistake for an answer

- **The `metal` vs `objc2-metal` split.** `gpui_apple` uses the `metal` crate; wgpu-hal's Metal
  backend speaks `objc2-metal`. A type mismatch is a build problem, not a verdict — and it is the
  reason the application, not GPUI, builds the texture.
- **Adopt-for-reading vs render-target.** `shared-surface.md` §2 probe 6 reached a wgpu texture's
  `MTLTexture` *for reading*; adopting one as a render target is the stronger claim this probe makes.
- **A two-GPU Mac** is where the power-preference lever is the only one
  ([`producer-reach.md`](producer-reach.md) §7) — the corner `P8` names, not a win here.

## 7. What it produces

A printout, filed as a `decisions/` evidence record beside
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md), and the
[`surface-plan.md`](surface-plan.md) §2 entry for P2 replaced by a link to it.
