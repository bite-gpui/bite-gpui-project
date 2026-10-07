# P9: device loss and re-negotiation

- **Status:** **run — PASS** (2026-10-07), on the Windows probe guest with a real GVT-g adapter;
  recorded in [`../../decisions/windows-device-loss-probe.md`](../../decisions/windows-device-loss-probe.md).
  Gates W5, and is a W5 **acceptance case** as much as a probe
  ([`surface-plan.md`](surface-plan.md) W5, [`interop-crate.md`](interop-crate.md) §4).
- **Question:** when GPUI's renderer loses and recreates its device, can the interop pool tear down and
  re-negotiate without crashing the host process — and does the application need to be told?
- **Gates:** W5.

## 1. Why this probe exists

A Windows driver resets on sleep, a monitor unplug, a DPI change or a GPU timeout (TDR). GPUI recreates
its device — the seam has `device_lost` and `recover` on `PlatformRenderer`, and
[`renderer-seam.md`](renderer-seam.md) §5.6 states that *"recovery is therefore self-sufficient on the
returned renderer"*. **The pool is not self-sufficient.** Every shared NT handle, fence and texture
view it holds is bound to the *old* device and becomes a dangling reference the moment the device is
gone, and nothing in the design says what happens then. The failure is a crash in the host process, on
a laptop lid, in production.

## 2. What is already known, so the probe does not re-measure it

- **The renderer recovers.** `device_lost()` → `recover()`, and a frame after recovery draws — the row
  [`verification.md`](verification.md) §1 already asserts.
- **The pool's state is device-bound.** A shared handle and a shared fence belong to the device that
  made them; recreating GPUI's device invalidates both.
- **The re-negotiation is one-sided.** The producer's own device may survive a reset of GPUI's — only
  GPUI's is recreated — so the pool cannot wait for the producer to notice; it must drive the teardown
  itself.

## 3. What it must measure, and where

**Hardware.** Windows. **The reset.** Force the renderer through `device_lost` → `recover` — either a
*real* reset (a deliberate GPU timeout) or the renderer's recovery path invoked directly while the pool
holds a surface. **Record which**: a simulated reset is weaker evidence than a driver reset, and the
distinction belongs in the printout.

**Ran as:** a real **device removal** — a PnP restart of the adapter (`pnputil /restart-device`), which
stops and re-adds the driver. A *timeout* is not achievable on a GVT-g guest: GVT-g runs the guest's
work on the host GPU, so a hung dispatch wedges the host engine; see
[`../../decisions/windows-device-loss-probe.md`](../../decisions/windows-device-loss-probe.md) §1.

## 4. The probes

1. **Attach and composite.** A producer pool holds a surface, and a frame with it composites.
2. **Lose the device.** The renderer reports loss (`device_lost`), and recovers.
3. **Who notices?** Does the renderer *signal* the loss to the pool, or must the pool poll? *This is
   the crate's hook, and the answer decides whether the API grows a callback.*
4. **Re-negotiate.** Drop the old handles, re-open on the new device, recreate the fences and views;
   the producer re-renders. *Pass: no stale handle is ever used.*
5. **A frame after recovery.** The composite is correct, the old handles are gone, and the process did
   not panic.

## 5. What each outcome closes

- **The pool can observe and re-negotiate** → the contract is: on loss, tear down, re-negotiate, and
  (probably) tell the application; record whether the app must re-supply anything — a device, a pool,
  or nothing.
- **The pool cannot observe it** → the API must expose a device-lost event the application subscribes
  to, and document that any `SurfaceSource` held across the loss is **invalid** — the honest contract
  when the handle cannot survive.

**Answered: the first, with a caveat the run exposed — the re-negotiation must *re-enumerate*.** The
pool cannot reuse the adapter it held (a PnP restart recreates the instance, and the LUID changes with
it), and a held `SurfaceSource` does not survive the loss, so the application must be told. The full
result is [`../../decisions/windows-device-loss-probe.md`](../../decisions/windows-device-loss-probe.md).

## 6. Hazards it must not mistake for an answer

- **A simulated reset is not a driver reset.** A `device_lost` call staged in code exercises the
  bookkeeping but not the driver's behaviour; a real TDR is the stronger run.
- **The producer's device surviving does not save the handles** — they are GPUI's, and they died with
  GPUI's device.
- **A "recover then re-create" ordering bug** looks like an intermittent crash under load, not a
  failure at the moment of loss; the probe must run the *order*, not only the event.
- **Rare on macOS and Linux, but the same contract.** A Metal or Vulkan device can be lost too; the
  pool's obligation does not become platform-specific just because the trigger is.

## 7. What it produces

A printout, filed as a `decisions/` evidence record beside
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) — the acceptance case
[`surface-plan.md`](surface-plan.md) §2 P9 names and the contract
[`interop-crate.md`](interop-crate.md) §4 states, with the row
[`verification.md`](verification.md) §1 carries.
