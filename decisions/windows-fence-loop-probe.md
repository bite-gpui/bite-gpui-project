# Evidence: the Windows fence-loop probe (P5)

- **Evidence for** the ordering half of W5 and `gpui-interop`'s Windows transport
  ([`../spi/rendering/interop-crate.md`](../spi/rendering/interop-crate.md) §4,
  [`../spi/rendering/probe-p5-fence-loop.md`](../spi/rendering/probe-p5-fence-loop.md)). Kept as
  evidence: what the design does with the answer is the crate's `windows` module, not a patch here.
- **The question:** on Windows, can an `ID3D12Fence` be shared with a Direct3D 11 device, so a producer
  *signals* and a consumer *waits* **GPU-side** — with no CPU stall and no keyed-mutex serialization?
  **Answered: yes — the shared fence opens as an `ID3D11Fence` and the consumer waits with
  `ID3D11DeviceContext4::Wait`, and the shared texture round-trips byte-exact.**
- **The probe** is [`../script/interop-probe`](../script/interop-probe), run on the Windows probe guest
  ([`../issues/0009-windows-probe-vm.md`](../issues/0009-windows-probe-vm.md)): a real GVT-g Intel HD
  520 adapter (`8086:1916`), not WARP.

## 1. What it answered

1. **Export the fence.** The producer creates an `ID3D12Fence` with `D3D12_FENCE_FLAG_SHARED` and
   exports it with `ID3D12Device::CreateSharedHandle` — the handle the printout names.
2. **Open it on the consumer.** Direct3D 11 opens it: `ID3D11Device5::OpenSharedFence` returns an
   `ID3D11Fence`. The device version P5 §4.2 asks to record is therefore present — the Direct3D 11.4
   interfaces (`ID3D11Device5` / `ID3D11DeviceContext4`), not a lower one.
3. **Signal / wait, GPU-side.** The consumer orders with `ID3D11DeviceContext4::Wait(fence, value)`, a
   **queue** wait — no `Sleep`, no `Map`-and-check, no CPU poll.
4. **The alternative (keyed mutex).** **Not measured.** The fence path worked, so the keyed-mutex cost
   P5 §4 would compare against was not the one taken.
5. **The ring, over several frames.** **Not measured as a ring** — the run does one
   produce → signal → wait → read pass, and it is byte-exact.

## 2. The printout

```
== P5 fence loop ==
D3D12 device on adapter luid 00000000:00005379
ID3D12Fence shared handle: HANDLE(0x334)
shared texture handle: HANDLE(0x310)  (64x64 R8G8B8A8_UNORM committed, heap flag SHARED)
D3D12 cleared RGBA [0.0, 1.0, 0.0, 1.0], queued signal value 1
D3D11 opened the shared texture (OpenSharedResource1) and made an SRV
order: ID3D12Fence shared handle opened as ID3D11Fence; context4.Wait(fence, 1) — GPU-side
readback: expected byte-exact RGBA [0, 255, 0, 255]; first pixel [0, 255, 0, 255]; 0/4096 mismatched
P5 fence loop: PASS — shared texture read back byte-exact on D3D11 (adapter luid 00000000:00005379)
```

## 3. The outcome, and what it costs

**A shared fence opens and orders** → the pool orders on a fence; `submit` is a signal and the consumer
a wait, and the crate needs no host-side lock
([`../spi/rendering/probe-p5-fence-loop.md`](../spi/rendering/probe-p5-fence-loop.md) §5's first
outcome). `gpui-interop`'s Windows module (`SharedSurface`, `Fence`) is built on it, and its consumer half — the open, the view and the wait — is the renderer's.

## 4. What is not measured

- **The keyed-mutex cost** (P5 §4) — the number that would decide whether the fence is *worth* it. Not
  taken, because the fence path worked; the fallback number is still owed if a device without Direct3D
  11.4 is ever a target.
- **The multi-frame ring** (P5 §5) — one pass, not a ring, so latency under repetition is unmeasured.
- **WARP vs hardware** — this run is on real hardware (a GVT-g vGPU), which is the stronger case; the
  CI caveat the other Windows probes carry does not apply here.
