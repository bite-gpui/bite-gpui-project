# Evidence: the macOS wgpu-producer probe

- **Evidence for** [`0004-producer-device-rendezvous.md`](0004-producer-device-rendezvous.md) — the
  producer's reach — and for
  [`../spi/rendering/foreign-texture.md`](../spi/rendering/foreign-texture.md) §2, whose macOS row
  was the one producer route with no measurement behind it.
- **The question:** can a wgpu producer serve GPUI's `MetalRenderer` on macOS, where the renderer
  owns the `MTLDevice` it created
  (`crates/gpui_apple/src/metal_renderer.rs:195`) and `wgpu-hal`'s Metal backend offers no public
  constructor that takes one? **Answered: yes — and there is nothing to hand over, because wgpu's
  adapter *is* the renderer's device.**
- **The probe** is the crate at `probes/macos-wgpu-producer`, on `bite_v1.22.0-pre-path-a`. Its job
  answered and was removed, so the printout in §2 is the durable record and the crate is the
  reproduction: `cargo run --manifest-path probes/macos-wgpu-producer/Cargo.toml` on a Mac.

## 1. What it answered

One `macos-14` job, green in 1m55s, on a runner whose only adapter is `Apple Paravirtual device`
(`IntegratedGpu`, `Metal`) — the same runner, and the same device, the Metal Path A rows and the
presentation probe use.

Three findings, and the first is the one that decides the route:

- **There is no adoption to do, because there is no second device.** wgpu's adapter on macOS is the
  `MTLDevice` the renderer picked, **pointer for pointer** — `0x149810a00` for the renderer's device
  and for wgpu's, under every power preference the probe asked for. macOS hands the same `MTLDevice`
  object to every creator, so the renderer's choice and wgpu's enumeration agree rather than being
  reconciled.
- **A texture wgpu made is one the renderer samples, byte for byte.** The probe made a
  `Bgra8UnormSrgb` texture on that device, filled it both ways a producer would — a `write_texture`
  copy and a render-pass clear whose colour the sRGB encode turns back into the same byte — handed
  the raw `id<MTLTexture>` over through `MetalTextureExt`, and the composite read `[200, 100, 50,
  255]` against the fixture both times. The token builder accepted the texture as it stands:
  `BGRA8Unorm_sRGB`, with `ShaderRead` because the texture was created with `TEXTURE_BINDING`.
- **The public API still cannot *address* a device, which is what makes the first finding
  load-bearing rather than convenient.** `AdapterShared::expose` is private, so the only door in is
  `wgpu::Instance::create_adapter_from_hal` over an adapter from enumeration — an adapter for a
  device macOS chose, not one the caller named. On a machine with two GPUs the power preference is
  therefore the only lever, which is why the probe asked under all three.

## 2. The printout

```
gpui:    device "Apple Paravirtual device" at 0x149810a00; metal sees 1 device(s)
wgpu:    1 adapter(s)
wgpu:    enumerated "Apple Paravirtual device" (IntegratedGpu, Metal)
wgpu:    HighPerformance -> "Apple Paravirtual device" (IntegratedGpu) at 0x149810a00, on gpui's device: true
wgpu:    LowPower -> "Apple Paravirtual device" (IntegratedGpu) at 0x149810a00, on gpui's device: true
wgpu:    None -> "Apple Paravirtual device" (IntegratedGpu) at 0x149810a00, on gpui's device: true
probe:   using the "Apple Paravirtual device" device, which is GPUI's own
probe:   wgpu texture is a BGRA8Unorm_sRGB 1x1
probe:   readback [200, 100, 50, 255] against [200, 100, 50, 255] at 8x8: byte for byte
probe:   wgpu texture is a BGRA8Unorm_sRGB 1x1
probe:   readback [200, 100, 50, 255] against [200, 100, 50, 255] at 8x8: byte for byte
VERDICT: wgpu landed on GPUI's own device and a texture it made was sampled by its Metal renderer
through MetalTextureExt. A wgpu producer on macOS therefore needs no device handover — only a power
preference that matches the one GPUI picked.
```

## 3. What it does not measure

- **A two-GPU Mac.** The runner has one device, so the identity above is measured where it could not
  fail in the interesting way. What the probe does establish for that case is which lever exists: a
  power preference, with `LowPower` matching `MetalRenderer::create_device`'s preference for a
  non-removable, low-power device.
- **Hardware.** `Apple Paravirtual device` is a real Metal device, but it is one behind a hypervisor,
  which is the caveat the Windows rows carry about WARP rather than a different one.
- **The other direction.** A producer on its *own* device still needs the shared-handle bridge
  [`0002`](0002-render-extension-device-model.md) defers; nothing here measures that, and on macOS it
  is the half [`shared-surface.md`](shared-surface.md) left unmeasured too.
