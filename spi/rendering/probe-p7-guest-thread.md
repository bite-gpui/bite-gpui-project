# P7: guest thread viability

- **Status:** proposed — **not run.** Gates W6, the guest runner
  ([`surface-plan.md`](surface-plan.md) W6, [`surfaces.md`](surfaces.md) §4). It lives with the work it
  gates and becomes a `decisions/` evidence record when it runs.
- **Question:** can the offscreen contract run on a worker thread and hand a **shareable surface** to
  a foreign loop — across the `!Send` boundary the renderer is built on?
- **Gates:** W6.
- **Companion:** [`probe-p1-reverse-bridge.md`](probe-p1-reverse-bridge.md), the guest path's *reverse
  transport*; this is the guest path's *threading*.

## 1. Why this probe exists

Guest Mode is *"headless GPUI on a worker thread"* ([`surfaces.md`](surfaces.md) §4): the host owns the
window and the cadence, and the GPUI thread runs a headless context. Two things make that a question
rather than a plan:

- **The renderer is `!Send` by construction** — the factory holds an `Rc`, and the wgpu slot is an
  `Rc<RefCell<…>>`, a same-thread affordance
  ([`producer-reach.md`](producer-reach.md) §2; [`verification.md`](verification.md) §1's
  thread-affinity row).
- **The offscreen contract returns bytes, not a surface.** `render_scene` renders into a target and
  `read_pixels` copies it to the CPU (`crates/gpui_engine/src/renderer.rs:80`, `:102`, `:110`);
  [`surfaces.md`](surfaces.md) §4 says the GPU path hands back a *surface* instead — and whether the
  contract has a method for that is the open bit.

## 2. What is already known, so the probe does not re-measure it

- **The offscreen contract is built** (W1): `render_scene`, `read_pixels`, `render_scene_to_image`.
- **What crosses a boundary is a `Send + Sync` handle**, not the renderer — the shape
  `ImportedTextureHandle` already takes (`Arc<dyn Any + Send + Sync>`).
- **The `!Send` factory is a guard, not an obstacle.** The type system forbids moving the renderer
  across threads; the guest runner must be designed so the renderer stays put and only a handle crosses.

## 3. What it must measure, and where

**Hardware.** Any platform — wgpu on Linux is the easiest to run headless. **Harness.** A scratch
crate: a worker thread running a headless GPUI context, and a fake host loop that samples it.

## 4. The probes

1. **Spawn.** Run a headless GPUI context on a worker thread, executors ticking.
2. **Render.** Drive a frame through the offscreen contract.
3. **The crossing.** Hand the frame to the host loop as a **surface** (zero-copy) and, separately, as
   **bytes** (a readback); measure both. *This is the shape a guest frame takes.*
4. **The boundary.** Keep the renderer on its thread; cross only a `Send + Sync` handle. *Record
   whether the contract exposes such a handle — an `offscreen_surface_handle`, as the design sketch
   had — or whether it must be added.*
5. **Ordering.** The host samples after the GPUI thread signals; confirm no tear and no stall.

## 5. What each outcome closes

- **A surface hand-off works across the thread boundary** → W6 is buildable as planned, and the
  contract either already has the hand-off or gains it.
- **The contract exposes only bytes** → W6's GPU path needs a core addition
  (`offscreen_surface_handle`), which is upstreamable alongside the surface work — and the guest runner
  can still ship the CPU path while it lands.
- **The `!Send` boundary is the shape, not a wall**: the renderer is pinned to its thread and the
  handle crosses — the same rule `0004` already applies to a producer.

## 6. Hazards it must not mistake for an answer

- **The `!Send` factory.** A design that moves the renderer across threads will not compile — the
  boundary is the *handle*, not the renderer.
- **The `Rc<RefCell<…>>` slot is same-thread**, so the guest thread owns its own context; it must not
  be reached from the host thread.
- **A stalled executor looks like a hang, not a wrong frame.**
- **Readback always works; the surface hand-off is the zero-copy path and the one that can fail** — a
  probe that only reads bytes proves half of the question.

## 7. What it produces

A printout, filed as a `decisions/` evidence record beside
[`0005`](../../decisions/0005-external-rendering-unifies-under-surface.md) — the W6 shape, and (if it
turns out to be missing) an `offscreen_surface_handle` addition to `SceneRenderer` for
[`surface-plan.md`](surface-plan.md) W6.
