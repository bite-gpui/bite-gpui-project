# Evidence: the Windows adapter-LUID probe (P6)

- **Evidence for** W5's adapter selection and the shape of `gpui-interop`'s `attach`/`Adapter`
  ([`../spi/rendering/interop-crate.md`](../spi/rendering/interop-crate.md) §4,
  [`../spi/rendering/probe-p6-adapter-luid.md`](../spi/rendering/probe-p6-adapter-luid.md)). Kept as
  evidence: what the design does with the answer is the crate's `adapter` module, not a patch here.
- **The question:** can the crate select, *through wgpu*, the physical adapter GPUI's Direct3D 11 device
  is on — and if it cannot, what must the API take instead?
  **Answered: a reliable rule exists on this machine — a LUID match between DXGI and wgpu — so `attach`
  finds the adapter and the API stays a one-liner.**
- **The probe** is [`../script/interop-probe`](../script/interop-probe), run on the Windows probe guest
  ([`../issues/0009-windows-probe-vm.md`](../issues/0009-windows-probe-vm.md)): a real GVT-g Intel HD
  520 adapter (`8086:1916`), not WARP.

## 1. What it answered

1. **GPUI's LUID.** The Direct3D 11 device is created on the adapter DXGI reports, and
   `IDXGIAdapter::GetDesc` carries its `AdapterLuid`.
2. **wgpu's LUIDs.** wgpu enumerates its DX12 adapters, and each one's LUID is readable through
   `Adapter::as_hal::<Dx12>()`.
3. **The rule.** Matching by LUID picks the same adapter: the D3D11 device's LUID and wgpu adapter 0's
   LUID are **the same number** — the cross-API equality P6 §6 says is *assumed, not guaranteed*.
4. **The two-GPU case.** **Not measured.** This guest has one hardware adapter (plus WARP), which P6 §6
   names as exactly the case that hides the problem, so this is a pass on a single-hardware-adapter
   machine.
5. **The fallback.** Not needed on the evidence, so it stays an escape rather than the default:
   `attach` may still take a caller-supplied adapter/device, but it resolves one by itself.

## 2. The printout

```
== P6 DXGI ==
adapter 0: Intel(R) HD Graphics 520  [8086:1916]  luid 00000000:00005379  128 MiB dedicated  flags 0x0
adapter 1: Microsoft Basic Render Driver  [1414:008C]  luid 00000000:000051EA  0 MiB dedicated  flags 0x0
adapter 2: Microsoft Basic Render Driver  [1414:008C]  luid 00000000:000052B5  0 MiB dedicated  flags 0x2 (software)
D3D11 device is created on adapter with luid 00000000:00005379 (Intel(R) HD Graphics 520 )
D3D11 device: adapter luid 00000000:00005379  feature level 11_1
P6 DXGI+LUID : PASS — 3 DXGI adapter(s); D3D11 device on luid 00000000:00005379 via IDXGIAdapter::GetDesc

== P6 wgpu ==
wgpu enumerated 2 DX12 adapter(s)
wgpu adapter 0: Intel(R) HD Graphics 520  backend=Dx12 type=IntegratedGpu  luid 00000000:00005379
wgpu adapter 1: Microsoft Basic Render Driver  backend=Dx12 type=Cpu  luid 00000000:000052B5
picked: D3D11 device on luid 00000000:00005379 (Intel(R) HD Graphics 520)
P6 wgpu      : PASS — LUID match between the D3D11 device (00000000:00005379) and wgpu adapter 0 (Intel(R) HD Graphics 520) luid 00000000:00005379
```

The LUIDs are per boot and per adapter instance, so the numbers are illustrative; the **match** — not
the number — is the result.

## 3. The outcome, and what it costs

**A reliable rule exists** → `attach` finds the adapter and the API stays a one-liner
([`../spi/rendering/probe-p6-adapter-luid.md`](../spi/rendering/probe-p6-adapter-luid.md) §5's first
outcome). What it does **not** settle is the two-GPU laptop: there a matching LUID is present for *both*
adapters, and the hazard becomes the *choice*, not the match. The caller-supplied escape stays in the
API for that reason — not because matching failed here.

## 4. What is not measured

- **Two physical adapters** — the case P6 §6 says breaks naive matching. The guest has one hardware
  adapter, so this is deliberately outstanding and needs a hybrid laptop.
- **A software/WARP adapter**, recorded but not representative of hardware.
