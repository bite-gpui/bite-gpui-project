# 0001 — No third swap: `FramePipeline` stays an open seam

- **Decided:** 2026-09-26
- **Status:** decided
- **Evidence:** [`tracy-swap-scope.md`](tracy-swap-scope.md),
  [`../spi/input-policy-seam.md`](../spi/input-policy-seam.md)

## Decision

`bite-gpui` ships two swaps — `bite-gp-parley` (TextSystem) and `bite-gp-morphorm`
(LayoutEngine). **No third swap is added**, no new version slot is opened, and the
public statement that `FramePipeline` is an open extension boundary stands unchanged.

## What was tried

A `FramePipeline` swap backed by `governor`'s GCRA, on the theory that frame pacing is
a rate-limiting problem. It was built and tested. Its own tests are what killed it.

### The advantage is not where it was claimed

At a rate the refresh period divides, a rate limiter and the in-tree
`ThrottledPipeline` admit the same frames: at 60 fps on 60 Hz both admit every ask, at
30 fps both admit every second ask. Where they differ, the limiter is *worse*. A
target equal to the refresh rate needs one interval of slack to hold, and a burst of
one delivers 30 fps for a 60 fps request — 0.3 ms of ask jitter is enough to trigger
it.

### Its one real advantage is a rate-expression capability, not pacing

A rate the refresh period does not divide *is* deliverable. 24 fps on 60 Hz comes out
as the 3:2 pulldown — holds of three refreshes and two, alternating — because the
limiter carries the half-refresh across frames, where a rule that remembers only the
previous frame cannot. Measured over five seconds of a 60 Hz ask grid: 61 frames for
the limiter at 24 fps requested, against 60 (30 fps) or 40 (20 fps) for the shipped
rule transcribed onto the same clock. Neither of the latter is 24.

That result is real and it is the strongest thing found. It is also narrow: it matters
where an exact non-divisor rate is wanted, which in practice means 24 and 25 fps
content.

### The boundary is wrong

`FramePipeline::should_render` is consulted *before* a frame's work. A pipeline can
defer work; it cannot touch presentation. It has no swapchain, no vblank and no
timestamp, and `WindowMetrics` carries no refresh rate. Rate-limiting is an ingress
concern — stochastic, unbounded arrival — and this is the egress door: periodic and
locked to the display.

### The ingress case has no seam, and the seam has a trap

There is no event seam on `Application`; its five are platform, layout, frame pipeline
and text system. Gating input would mean wrapping `Platform` *and* `PlatformWindow`,
two of the largest traits in the layer. Worse, a filtering gate would blind
`InputRateTracker`, which counts events that reached *dispatch* — so coalescing
pointer input can suppress the VRR sustain the tracker exists to enable. Both are
written up in `../spi/input-policy-seam.md`, which is parked rather than proposed.

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
| ship the governor swap as a `FramePipeline` | worse than the shipped cap at ordinary rates, and unable to pace presentation. The tests are in `.governor/tests/` |
| open the `InputPolicy` seam now | it would exist principally to host a limiter whose measured benefit is narrower than its apparent scope, and no input-flood case has been reproduced |
| ship `puffin` telemetry instead | pure Rust, so the C++ objection does not apply — but it needs the same version-slot decision, and its advantage still cannot be exercised in CI |
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
