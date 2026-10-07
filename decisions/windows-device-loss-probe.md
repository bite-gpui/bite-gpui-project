# Evidence: the Windows device-loss probe (P9)

- **Evidence for** the W5 acceptance case [`../spi/rendering/surface-plan.md`](../spi/rendering/surface-plan.md)
  §2 P9 and the contract [`../spi/rendering/interop-crate.md`](../spi/rendering/interop-crate.md) §4
  ("device loss is part of the contract, not an edge"). Kept as evidence: what the design does with
  the answer is the pool's teardown and re-negotiation in `gpui-interop`, not a patch here.
- **The question:** when GPUI's renderer loses and recreates its device, can the interop pool tear
  down and re-negotiate without crashing the host process — and does the application need to be told?
  **Answered: yes, and yes — the pool must re-enumerate; the adapter does not survive the loss
  unchanged.**
- **The probe** is [`../script/interop-probe`](../script/interop-probe), run with `--pnp` on the
  Windows probe guest ([`../issues/0009-windows-probe-vm.md`](../issues/0009-windows-probe-vm.md)):
  a real GVT-g Intel HD 520 adapter (`8086:1916`), not WARP. Its full specification is
  [`../spi/rendering/probe-p9-device-loss.md`](../spi/rendering/probe-p9-device-loss.md).

## 1. What it answered

The five probes, in order:

1. **Attach and composite.** A producer device (D3D12) holds a shared NT texture and a shared
   `ID3D12Fence`; a D3D11 device on the same adapter opens both, orders on the fence, and reads the
   bytes back **byte-exact** (0 of 4096 pixels wrong). The pool is attached and compositing.
2. **Lose the device.** The loss is staged as a **real device removal**: the adapter's PnP device is
   restarted (`pnputil /restart-device "<instance-id>"` — *"Device restarted successfully."*), which
   stops and re-adds the driver. Device A's `GetDeviceRemovedReason()` then returns
   `DXGI_ERROR_DEVICE_REMOVED` (`0x887A0005`).
3. **Who notices?** **Nothing signals it.** At the D3D level there is no callback: the pool learns of
   the loss because its next use fails, so it must *poll* — and, worse, it must not reuse the adapter
   it held (see 4). This is the crate's hook, and the answer is that the crate has to drive the
   teardown itself.
4. **Re-negotiate.** The adapter has to be **re-enumerated**: a PnP restart recreates the device
   instance, so an `IDXGIAdapter` held across the loss is stale and `D3D12CreateDevice` on it fails
   with `DXGI_ERROR_UNSUPPORTED` indefinitely. After re-enumerating — and the **LUID changes with the
   new instance** — a fresh device B recreates the texture and fence, D3D11 re-opens them, and the
   frame composites **byte-exact** again.
5. **A frame after recovery.** No panic, the old handles are gone, the new frame is byte-exact.

**How the loss was staged, and why it matters.** [`probe-p9-device-loss.md`](../spi/rendering/probe-p9-device-loss.md)
§3 prefers a *real* GPU timeout (a TDR) over invoking the recovery path directly. A TDR is
**not achievable on this guest**, for a structural reason: GVT-g does not emulate the GPU — it runs
the guest's command stream on the *host* iGPU — so an infinite dispatch hangs the host engine and
wedges `intel_gvt_wait_vgpu_idle` until a host reboot (it did; the host needed a reboot). So the
staging here is a real **removal**, not a *timeout*: stronger than invoking the recovery path
in-process, weaker than a driver timeout. On hardware with a real passed-through GPU the TDR run is
the last staging to add.

## 2. The printout

```
== P9 device loss + re-negotiation ==
attach  : device A holds a shared surface on luid 00000000:00005359; composite byte-exact (ID3D11Fence (GPU-side context4.Wait))
lose    : restarting the PnP device PCI\VEN_8086&DEV_1916&SUBSYS_070B1028&REV_07\3&11583659&0&20 (a real driver stop/start)
        : pnputil: Microsoft PnP Utility

Restarting device:         PCI\VEN_8086&DEV_1916&SUBSYS_070B1028&REV_07\3&11583659&0&20
Device restarted successfully.
        : device A removal observed: Error { code: HRESULT(0x887A0005), message: "The GPU device instance has been suspended. Use GetDeviceRemovedReason to determine the appropriate action." }
lose    : PnP restart of PCI\VEN_8086&DEV_1916&SUBSYS_070B1028&REV_07\3&11583659&0&20 — a real device stop/start, not a TDR
recover : new producer device B on luid 00000000:00098A3D, removed-reason S_OK
re-neg. : fresh handles on B; composite byte-exact (ID3D11Fence (GPU-side context4.Wait))
P9 frame after recovery: no panic, byte-exact on luid 00000000:00098A3D

SUMMARY
P6 DXGI+LUID : PASS
P6 wgpu      : PASS — LUID match on wgpu adapter 0 (Intel(R) HD Graphics 520) luid 00000000:00005359
P5 fence loop: PASS — byte-exact via ID3D11Fence (GPU-side context4.Wait)
P9 device loss: PASS — PnP restart of PCI\VEN_8086&DEV_1916&SUBSYS_070B1028&REV_07\3&11583659&0&20 — a real device stop/start, not a TDR
```

The LUIDs are per run and per instance: `…00005359` before the restart, `…00098A3D` after — the
same run shows the identity changing, which is the point of finding 4.

## 3. The outcome, and what it costs

**The pool can observe the loss and re-negotiate** — so the contract is the first of
[`probe-p9-device-loss.md`](../spi/rendering/probe-p9-device-loss.md) §5's two outcomes: on loss,
tear down, re-discover, re-negotiate, and tell the application. Three things it forces:

- **Re-enumerate, do not reuse the adapter.** The `IDXGIAdapter` is device-bound like the handles are.
  This is the finding that would have shipped as an intermittent `DXGI_ERROR_UNSUPPORTED` under load.
- **The LUID is not stable across a loss.** Adapter *matching* by LUID is fine within a boot, but the
  pool cannot cache a LUID as "the adapter" across a device loss.
- **A held `SurfaceSource` is invalid across the loss.** The application cannot keep sampling what it
  was handed; the crate should expose a device-lost event (or state the invalidity in the type), which
  is the "does the application need to be told" half of the question, answered *yes*.

## 4. What is not measured

- **A driver *timeout* (TDR).** Structural: it wedges the host GPU on GVT-g (above). The remaining
  staging for a host with a real passed-through GPU.
- **GPUI's own renderer path.** The probe stages the loss at the D3D level, not through
  `PlatformRenderer::device_lost` → `recover`, so the seam's recovery — the reworded row
  [`../spi/rendering/verification.md`](../spi/rendering/verification.md) §1 carries — is asserted
  separately, and was not what this run measured.
- **The one-sided case.** P9 §2 notes the producer's device may survive a reset of GPUI's; the probe
  uses a single device, so it measures the strong case (both gone) rather than the mixed one.
