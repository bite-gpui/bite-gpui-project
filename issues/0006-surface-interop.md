- **Opened:** 2026-10-01
- **Status:** open
- **Touches:** `bite-gpui/bite-gpui` (two upstream PRs and the Windows/Linux arms), a new `gpui-interop` repository, [`../decisions/0005-external-rendering-unifies-under-surface.md`](../decisions/0005-external-rendering-unifies-under-surface.md), [`../spi/rendering/surfaces.md`](../spi/rendering/surfaces.md), [`../spi/rendering/surface-plan.md`](../spi/rendering/surface-plan.md)

# The surface implementation, across repositories

## The problem

The surface work has no single branch to live on. It is two upstream pull requests, two out-of-tree
renderer arms, a downstream companion crate, and a set of probes that decide whether that crate is
worth writing at all. No one branch can own it, which is why it is here.

The decision is [`0005`](../decisions/0005-external-rendering-unifies-under-surface.md), the design is
[`surfaces.md`](../spi/rendering/surfaces.md), and the order and the probes are
[`surface-plan.md`](../spi/rendering/surface-plan.md) §1 and §2. This issue is the coordination those
three do not carry: what goes where, what has to be decided before what, and what "done" means. It does
not restate them. The downstream crate it names is planned in
[`../spi/rendering/interop-crate.md`](../spi/rendering/interop-crate.md). Its gating probes are specified in [`../spi/rendering/probe-p1-reverse-bridge.md`](../spi/rendering/probe-p1-reverse-bridge.md),
[`../spi/rendering/probe-p2-macos-adoption.md`](../spi/rendering/probe-p2-macos-adoption.md) and
[`../spi/rendering/probe-p3-dmabuf-import.md`](../spi/rendering/probe-p3-dmabuf-import.md).

## What is already built

The offscreen contract — `PixelBuffer`, and `render_scene` split from `read_pixels` — is W1 and is
built; the three renderer arms, `device_any` and the runnable demo are on the canonical ref. What the
unification adds is the *entry point* (W2–W4) and the *bridge* (W5–W6), not the engine. So the two
upstream PRs are additive, and nothing here blocks a release of the current line.

## The questions to settle

1. **Where `gpui-interop` lives.** A new repository — which gains a line in
   [`../repositories.md`](../repositories.md) and [`../online-resources.md`](../online-resources.md) —
   or a crate in an existing one. The governing ruling is that it sits *above* the facade and adds
   nothing to core ([`../architecture/extension-tiers.md`](../architecture/extension-tiers.md)).
2. **The published payload types.** `0004` names the per-backend payloads as published surface. A crate
   that must name `ID3D11ShaderResourceView` and a `wgpu` adapter in one function has to pin *which*
   crate publishes them, and at which version — the commitment `0004` flags under "what would reopen
   this".
3. **Upstream first, or branch first.** Whether W1's hygiene and W2's Windows arm are opened as upstream
   PRs before anything else, or carried on a branch until they land. W2 is what upstream asked for, so a
   first step is to open it.
4. **What P1 gates.** P1 — the reverse bridge direction (D3D11 → `wgpu`) — is scoped to the guest
   runner (W6), not the crate (W5): Host Mode's direction is already measured, and a Direct3D 11
   producer uses the default `DirectXRenderer` (same device). A P1 failure costs Guest Mode, not the
   crate.
5. **The probe homes.** P1–P9 each run where their hardware is, and each becomes a `decisions/` evidence
   record beside `0005`/`0002`; the ones that can be asserted become tests
   ([`../spi/rendering/verification.md`](../spi/rendering/verification.md) §4).
6. **The device-loss contract.** When GPUI's device is recreated after a driver reset (P9), the pool
   must re-negotiate rather than hold a stale handle; deciding that up front keeps it out of the API's
   assumptions.

## What would close it

The two upstream PRs merged; `gpui-interop` published and reachable from the facade; the probes filed
as evidence; and [`milestones.md`](../spi/rendering/milestones.md) §4's M5–M7 checked off. Until then
this issue tracks the cross-repository half that no branch owned.
