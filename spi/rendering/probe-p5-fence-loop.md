# P5: the fence bridge, end to end

- **Status:** proposed — **not run.** The probe that gates the *ordering* half of W5
  ([`surface-plan.md`](surface-plan.md) W5, [`interop-crate.md`](interop-crate.md) §4). It lives with
  the work it gates and becomes a `decisions/` evidence record when it runs.
- **Question:** on Windows, can an `ID3D12Fence` be shared with a Direct3D 11 device, so a producer
  *signals* and a consumer *waits* **GPU-side** — with no CPU stall and no keyed-mutex serialization?
- **Gates:** W5.
- **Companions:** [`probe-p6-adapter-luid.md`](probe-p6-adapter-luid.md) (the adapter the fence is
  bound to) and [`probe-p9-device-loss.md`](probe-p9-device-loss.md) (what a reset does to a fence).

## 1. Why this probe exists

[`../../decisions/shared-surface.md`](../../decisions/shared-surface.md) §2 probe 5 renders through
wgpu into an app-allocated shared resource and reads it back on D3D11 — but it **carries no fence
between the two devices**; it clears, *polls* and reads. Its macOS fence probe orders two buffers on
*one* device, which is the pool's case, not the cross-device one; its §4 lists cross-device
synchronisation as unmeasured. Without a shared fence, the only safe D3D11↔D3D12 orderings are a keyed
mutex (which serialises the two queues) or a CPU wait (which stalls the frame). This probe asks whether
the hardware path exists, so the crate's `submit` can be a signal rather than a lock.

## 2. What is already known, so the probe does not re-measure it

- **The handle and adapter are measured elsewhere.** A shared resource needs a shared handle (measured)
  and a matching physical adapter ([`probe-p6-adapter-luid.md`](probe-p6-adapter-luid.md)).
- **One device, one queue needs no fence.** The same-device route is ordered by submission; this is the
  two-device case only ([`surfaces.md`](surfaces.md) §2).
- **Direct3D 11.4 mirrors Direct3D 12's fence API.** `ID3D11Device5`/`ID3D11DeviceContext4` expose
  `CreateFence` and `OpenSharedFence`, the counterpart of `ID3D12Device::CreateSharedHandle` for a
  fence — *this is what route A of the probe establishes, not assumes*.

## 3. What it must measure, and where

**Hardware.** Windows, two devices on one adapter — the setup `P1`/`P6` already build: a
Direct3D 12/`wgpu` producer and a Direct3D 11 consumer. **Harness.** A scratch crate.

## 4. The probes

1. **Export the fence.** The producer creates an `ID3D12Fence` and exports it with
   `CreateSharedHandle`. *Prints the handle.*
2. **Open it on the consumer.** On Direct3D 11.4, open the shared fence as an `ID3D11Fence` — and
   record the **device version / feature level** it needs, because that is the portability limit.
3. **Signal / wait, GPU-side.** The producer signals after rendering into the shared texture; the
   consumer waits on the fence *before* sampling, as a queue signal/wait, not a CPU poll. *Pass: no
   tear, no `Sleep`, no `Map`-and-check.*
4. **The alternative, measured.** Run the same loop with a **keyed mutex** and record the cost — the
   two queues can no longer overlap. *This is the number that decides whether the fence is worth it.*
5. **The ring, over several frames.** acquire → render → signal → wait → sample → release, repeated;
   *Pass: no tear, no stall, bounded latency.*

## 5. What each outcome closes

- **A shared fence opens and orders** → the pool orders on a fence; `submit` is a signal and the
  consumer a wait, and the crate needs no host-side lock.
- **Direct3D 11 cannot open a Direct3D 12 shared fence on the target** → the crate falls back to a
  keyed mutex (with its serialization) or a CPU wait (with its stall); the plan records which, per
  platform and device version.
- **A result on WARP** is *mechanically possible on one adapter*, the caveat every Windows probe here
  carries.

## 6. Hazards it must not mistake for an answer

- **A fence is adapter-bound.** A fence matches only on the same physical adapter as the resource —
  the same LUID hazard `P6` measures.
- **Device version.** Shared fences need Direct3D 11.4 and a feature level; a "shared fences do not
  work" verdict on an old device is a portability bound, not a wall.
- **A CPU wait hides a missing fence.** The probe must measure the *stall*, not only correctness —
  the failure mode is a working picture at the cost of the frame it was meant to save.
- **Device loss interacts.** A reset invalidates the fence like any handle
  ([`probe-p9-device-loss.md`](probe-p9-device-loss.md)).

## 7. What it produces

A printout, filed as a `decisions/` evidence record beside
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md), and the
[`surface-plan.md`](surface-plan.md) §2 entry for P5 replaced by a link to it.
