# 0001 — No third swap: `FramePipeline` stays an open seam

- **Decided:** 2026-09-26
- **Status:** decided
- **Evidence:** [`tracy-swap-scope.md`](tracy-swap-scope.md),
  [`../spi/input/input-policy-seam.md`](../spi/input/input-policy-seam.md)

## Decision

`bite-gpui` ships two swaps — `bite-gp-parley` (TextSystem) and `bite-gp-morphorm`
(LayoutEngine). **No third swap is added**, no new version slot is opened, and the
public statement that `FramePipeline` is an open extension boundary stands unchanged.

## What was tried

A `FramePipeline` swap that throttles frames to a rate, on the theory that frame pacing
is a rate-limiting problem. It was built on `governor`'s GCRA and later reduced to a
schedule carried in the crate itself — `gpui_pass` in `.pass/`, which has since grown a
ledger alongside the throttle; the measurements below are the same in either shape. It was
built and tested. Its own tests are what killed it as a swap.

### The advantage is not where it was claimed

At a rate the refresh period divides, the throttle and the in-tree `ThrottledPipeline`
admit the same frames: at 60 fps on 60 Hz both admit every ask, at 30 fps both admit
every second ask. Where they differ, the throttle is *worse*. A target equal to the
refresh rate needs one interval of slack to hold, and a burst of one delivers 30 fps for
a 60 fps request — 0.3 ms of ask jitter is enough to trigger it.

### Its one real advantage is a rate-expression capability, not pacing

A rate the refresh period does not divide *is* deliverable. 24 fps on 60 Hz comes out
as the 3:2 pulldown — holds of three refreshes and two, alternating — because the
throttle carries the half-refresh across frames, where a rule that remembers only the
previous frame cannot. Measured over five seconds of a 60 Hz ask grid: 121 frames for
the throttle at 24 fps requested, against 150 (30 fps) or 100 (20 fps) for the shipped
rule transcribed onto the same clock. Neither of the latter is 24.

That result is real and it is the strongest thing found. It is also narrow: it matters
where an exact non-divisor rate is wanted, which in practice means 24 and 25 fps
content.

### The boundary is wrong

`FramePipeline::should_render` is consulted *before* a frame's work. A pipeline can
defer work; it cannot touch presentation. It has no swapchain, no vblank and no
timestamp, and `WindowMetrics` carries no refresh rate. Rate-limiting is an ingress
concern — stochastic, unbounded arrival — and this is the egress door: periodic and
locked to the display. **A throttle belongs here and a limiter does not**: clamping how
often the frame's work runs needs no library and no raster, while policing arrival rates
against the display is the platform's job, and a software limiter that tries it fights
the raster's clock rather than aligning to it.

### The ingress case has no seam, and the seam has a trap

There is no event seam on `Application`; its five are platform, layout, frame pipeline
and text system. Gating input would mean wrapping `Platform` *and* `PlatformWindow`,
two of the largest traits in the layer. Worse, a filtering gate would blind
`InputRateTracker`, which counts events that reached *dispatch* — so coalescing
pointer input can suppress the VRR sustain the tracker exists to enable. Both are
written up in `../spi/input/input-policy-seam.md`, which is parked rather than proposed.

### The remaining candidate failed on its dependency

A telemetry swap fits the seam's pass structure, and was scoped. It does not ship:
`tracy-client-sys` compiles `tracy/TracyClient.cpp` with `cc::Build::cpp(true)`, a C++
toolchain requirement in every consumer's build, against a distribution whose existing
swaps link no system libraries. It also could not have carried a headline figure of
the kind the other two swaps have — its measurable part is cost, and its advantage
needs an external GUI, so no CI check could exercise it. See `tracy-swap-scope.md`.

## Rejected alternatives

| alternative | why not |
| --- | --- |
| ship `bite-gp-pass`'s throttle as a `FramePipeline` | worse than the shipped cap at ordinary rates, and unable to pace presentation. The tests are in `.pass/tests/` |
| open the `InputPolicy` seam now | it would exist principally to host a limiter whose measured benefit is narrower than its apparent scope, and no input-flood case has been reproduced |
| ship `puffin` telemetry as a swap | the C++ objection does not apply — puffin is pure Rust — but as a swap it needs the same version-slot decision and its advantage still cannot be exercised in CI. It was instead pursued as a **wrap**: `.pass`'s `PuffinPipeline`, behind a `puffin` feature, and `tests/puffin.rs` exercises the scopes in CI. See the update below |
| change nothing and leave the site copy alone | the site already said `FramePipeline` is an open boundary, which turned out to be the accurate page |

## What would reopen this

- **A reproducible input-flood case**, with numbers, on a target platform. Then §6 of
  the seam note — a declarative `input_coalescing` on `WindowOptions`, flushed by the
  frame loop — is the small fix, and a full `InputPolicy` trait only if arbitrary
  policies turn out to be needed.
- **A request for an exact non-divisor frame rate** — 24 fps video, 25 fps PAL — where
  the in-tree `ThrottledPipeline` provably cannot deliver it. That is the one
  capability the investigation found the shipped tools lack, and it is the only
  candidate argument for a swap that survived contact with the trait.
- **The distribution accepting a C++ dependency**, which would put `tracy-client` back
  on the table.

## Update — the crate became a pass

After this decision was recorded, the crate behind it was renamed `gpui_governor` →
`gpui_pass` and grew a second half: alongside the throttle it keeps a `Ledger` — the
passes each drawn frame ran, the asks the throttle passed over, and percentiles over the
recent frames — all off the same injected clock.

That bears on one argument above, and only one. The reason a telemetry swap did not ship
was that its advantage could not be exercised in CI: it needed an external profiler's
GUI. The pass's accounting can be — with a fake clock the decisions *and* the recorded
costs are a test's, so a frame budget is an assertion. That answers the objection; it
does not by itself decide the swap.

**This decision is about swaps, and `bite-gp-pass` is not one.** It is a *wrap*: it
decorates the frame pipeline rather than replacing it, which is the second tier in
[`../architecture/extension-tiers.md`](../architecture/extension-tiers.md). So this
record does not decide whether it ships. A wrap commits the project to less than a swap
— it consumes the trait rather than freezing it, so a wrap on crates.io constrains
`FramePipeline` no more than any other consumer — and whether to publish one is a
question of its own. What is true either way: the crate has a repository of its own,
`git@github.com:bite-gpui/gpui_pass.git`, holding the version slot `1.21.3`, which has
since been released as `bite-gp-pass 1.21.3`.

## Update — the telemetry objection was answered by the tier, not the library

Puffin telemetry was rejected here twice over: as a swap it would need a version slot,
and its defining advantage — a timeline you read in a viewer — could not be exercised in
CI. The second half is false once the ceiling is a *wrap*. `.pass` now carries
`PuffinPipeline`, behind an off-by-default `puffin` feature: one scope per pass and a
frame boundary between frames. The scopes are recorded **in-process**, so
`tests/puffin.rs` turns them on, drives real frames through the facade, and reads the
frames back with `puffin::GlobalFrameView`, asserting exactly one scope per pass. The
swap that could not be gated is a decorator that is. What this does not change: no
fourth swap and `FramePipeline` still an open boundary. (The crate has since been
published: `bite-gp-pass 1.21.3`.)
