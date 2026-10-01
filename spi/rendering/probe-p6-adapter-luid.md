# P6: adapter LUID matching

- **Status:** proposed — **not run.** The probe that gates W5's *adapter selection* — and, because its
  answer is the crate's API, the one to **run first**
  ([`surface-plan.md`](surface-plan.md) §2, [`interop-crate.md`](interop-crate.md) §4).
- **Question:** can the crate select, *through wgpu*, the physical adapter GPUI's Direct3D 11 device is
  on — and if it cannot, what must the API take instead?
- **Gates:** W5, and the shape of `Interop`/`Adapter`.
- **Companions:** [`probe-p5-fence-loop.md`](probe-p5-fence-loop.md) (a fence is adapter-bound too).

## 1. Why this probe exists

Cross-device sharing needs the **same physical adapter** — a LUID match — and
[`../../decisions/windows-path-a-probe.md`](../../decisions/windows-path-a-probe.md) §5 names it as the
first hazard; [`verification.md`](verification.md) §4 records why it "cannot be asserted from inside
either device". If matching is unreliable on a two-GPU laptop, then the fallback — the **application
supplies the adapter or device** — must be in the API from the start, not retrofitted. That is why this
probe runs before the crate's shape is frozen: a late answer is a redesign
([`interop-crate.md`](interop-crate.md) §4).

## 2. What is already known, so the probe does not re-measure it

- **The adapter is readable from DXGI.** GPUI's Direct3D 11 device comes from DXGI adapter selection,
  and `IDXGIAdapter::GetDesc` carries `DXGI_ADAPTER_DESC::AdapterLuid`.
- **wgpu exposes the raw adapter.** `Adapter::as_hal::<Dx12>()` reaches the underlying
  `IDXGIAdapter`, so wgpu's adapter LUID is readable the same way.
- **wgpu has no "select by LUID".** Selection is enumerate-and-match, or `create_adapter_from_hal`
  with a caller-provided adapter — which is exactly the fallback this probe decides on.

## 3. What it must measure, and where

**Hardware.** Windows, and **ideally a two-GPU machine** (integrated + discrete, Optimus/hybrid) —
because a single-GPU machine is the case that hides the problem. **Harness.** A scratch crate.

## 4. The probes

1. **GPUI's LUID.** Read the Direct3D 11 adapter's LUID through DXGI. *Prints it.*
2. **wgpu's LUIDs.** Enumerate wgpu adapters and read each one's LUID through `as_hal`. *Prints them,
   with the adapter names.*
3. **The rule.** Match by LUID. *Record whether a match exists at all on this machine.*
4. **The two-GPU case.** With two physical adapters present, does the rule pick the same one GPUI is
   on? *Record what happens when it does not — a share that fails, or a silent cross-adapter copy.*
5. **The fallback.** If no rule is reliable, define the API: `attach` takes a caller-supplied adapter
   or device ([`interop-crate.md`](interop-crate.md) §4), and `Adapter` is caller-provided on such
   machines.

## 5. What each outcome closes

- **A reliable rule exists** → `attach` finds the adapter, and the API stays a one-liner.
- **No reliable rule, or a driver that hides the LUID** → the API **takes a caller-supplied adapter**;
  the crate's `Adapter` is an input, not only an output. This is the answer the crate shape needs.
- **A software/WARP adapter** is recorded but is not representative of hardware.

## 6. Hazards it must not mistake for an answer

- **Two-GPU laptops are the case that breaks naive matching** — a pass on a single-GPU desktop says
  nothing about them.
- **The LUID is per-adapter, not per-device.** Reading it from the adapter (not from the device being
  compared) is the trick; two independently created *devices* do not expose each other's.
- **Cross-API LUID equality is assumed, not guaranteed.** The probe confirms that DXGI's LUID and
  wgpu's are the same number, rather than trusting the documentation.

## 7. What it produces

A printout, filed as a `decisions/` evidence record beside
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md), and the
[`surface-plan.md`](surface-plan.md) §2 entry for P6 replaced by a link to it. Its answer **fixes
[`interop-crate.md`](interop-crate.md) §4's API** — which is why it runs before the crate is written.
