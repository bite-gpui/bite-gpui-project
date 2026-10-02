# The packed Windows probe

- **Status:** proposed — **not written.** The one binary a Windows run executes, printing every
  Windows-side measurement W5 needs, so one run answers as many questions as possible.
- **Why packed:** the probes have no cheap home. This box is Linux, the guest
  ([`../../issues/0009-windows-probe-vm.md`](../../issues/0009-windows-probe-vm.md)) is the only real
  device, and CI is the other place they can go — each run is expensive, each check is cheap, so they
  share a binary.
- **Read with:** [`probe-p5-fence-loop.md`](probe-p5-fence-loop.md),
  [`probe-p6-adapter-luid.md`](probe-p6-adapter-luid.md),
  [`probe-p9-device-loss.md`](probe-p9-device-loss.md), and
  [`verification.md`](verification.md) §2 (the PR-2 sufficiency row).
- **Gates:** W5 — and, through P6, the shape of `gpui-interop`'s `attach`/`Adapter`.

## What one run must print

Each prints `PASS` / `FAIL` / `SKIP` with its evidence, so a partial answer is still recorded:

1. **P6 — adapter identity.** GPUI's Direct3D 11 adapter LUID (read through DXGI,
   `IDXGIAdapter::GetDesc`), every wgpu adapter's LUID (`Adapter::as_hal::<Dx12>()`), and whether a
   LUID match exists. With two adapters present — the case a single-GPU machine hides — record
   **which one GPUI is on and which one wgpu picks**, and whether a failed match is a hard share
   failure or a silent cross-adapter copy.
2. **P5 — the fence loop.** A second device renders into a shared NT handle, the bridge opens it, and
   an `ID3D12Fence`↔`ID3D11Fence` handshake orders the two queues; read back **byte for byte**, no
   tear, no keyed-mutex stall.
3. **P9 — device loss.** Drop GPUI's device (`PlatformRenderer::device_lost` → `recover`) while a
   surface is held, then re-attach; the bridge must tear down and re-negotiate without crashing the
   host process.
4. **The PR-2 sufficiency test.** An `ID3D11Texture2D` cleared to a known colour, as an SRV through
   `surface()` in a `div()`, read back **byte for byte** — no bridge, no second device. This is the
   claim that PR 2 and the device accessor are *sufficient*, and it is also what PR 2 should carry.

## What its outcome decides

- **P6 match works** → `attach` finds the adapter; the API stays a one-liner.
- **No reliable match** → `attach` **takes a caller-supplied adapter/device**; the crate's `Adapter`
  is an input, not only an output. This is the answer the crate's shape needs, which is why the probe
  runs before the crate is finished.

## Where it runs

| where | answers | caveat |
| --- | --- | --- |
| `windows-latest` CI | P5, P9, the sufficiency test; P6's *mechanics* | WARP — **not representative** for P6 (probe §5) |
| the interop VM ([issue 0009](../../issues/0009-windows-probe-vm.md)) | all of it, on a real driver | one real GPU; the true two-GPU hybrid case still needs hardware |

## What it is not

Not the crate, and not a test suite. It is a probe that prints; the checks that harden into
assertions move into `gpui-interop`'s `tests/` as their probes clear —
`tests/same_device.rs` from (4), `tests/bridge.rs` from (1)–(3).
