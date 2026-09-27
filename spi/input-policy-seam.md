# An ingress seam: proposing `InputPolicy`

**Status: parked.** Not implemented, not accepted, and not being proposed upstream
now. The investigation that produced it concluded that no third swap ships, so the
seam would exist principally to host a limiter with a narrower measured benefit than
its apparent scope (§7). Kept as the record of the missing seam and its traps — the
`InputRateTracker` interaction in §5 and the flush gap in §6 in particular — so a
future attempt starts from these findings rather than from scratch.

It is a specification of a seam that `bite-gp-gate` would need in order to exist as an
installable swap, in the way `bite-gp-parley` and `bite-gp-morphorm` are.

**Line references** are to the `zed` clone, where the sources live under `crates/`
(`crates/gpui_authoring`, `crates/gpui_platform`, `crates/gpui_runtime`), which is the
same code the distribution publishes as `bite-gp-authoring`, `bite-gp-platform` and
`bite-gp-runtime`.

---

## 1. Why this exists

The investigation that produced this note set out to make a rate limiter a
`FramePipeline` swap. It concluded that the boundary is wrong:

- A `FramePipeline` can only **defer a frame's work**. It cannot pace presentation —
  the trait can reach no swapchain, vblank or timestamp, and `should_render` is
  consulted *before* the work rather than at present time. A pacer there is not
  policing the door it appears to be.
- On a display-cadence ask stream the limiter is not even better than the shipped
  `ThrottledPipeline`: at the refresh rate both admit every ask, and with a burst of
  one the limiter is strictly worse (60 fps asked on 60 Hz delivers 30). The one
  case where it wins is a rate the grid does not divide (24 on 60, as 3:2 pulldown),
  which is a rate-expression capability, not a pacing one.

The rate-limiting problem is an **ingress** problem: stochastic, unbounded arrival
that must not saturate downstream work. This architecture has no ingress seam to
attach a solution to, and this note specifies one.

## 2. What is missing, precisely

### 2.1 There is no event seam at the facade

`Application` exposes five trait seams, and none of them is about events:

| seam | shape |
| --- | --- |
| `with_platform` | `Rc<dyn Platform>` |
| `with_layout_engine` | `impl Fn() -> Box<dyn LayoutEngine> + 'static` |
| `with_frame_pipeline` | `impl Fn(WindowId) -> Box<dyn FramePipeline> + 'static` |
| `with_text_system` | `Arc<dyn TextSystem>` |
| — | (`with_assets`, `with_http_client`, `with_quit_mode`, `with_restart_arguments` — not seams) |

`crates/gpui_runtime/src/application.rs:42-125`. `gpui_runtime` itself holds no
events at all; its module doc is explicit that it is the process harness and that
"everything an application *is* once it is running lives in `gpui_authoring`, which
this crate drives rather than contains" (`crates/gpui_runtime/src/gpui_runtime.rs:1-11`).

### 2.2 Events arrive at a per-window platform callback

Input enters at `PlatformWindow::on_input`, whose callback signature is
`Box<dyn FnMut(PlatformInput) -> DispatchEventResult>`
(`crates/gpui_platform/src/platform_window.rs:132-133`). The window host wires it
straight through:

```rust
// crates/gpui_authoring/src/window.rs:2164-2172
platform_window.on_input({
    let mut cx = cx.to_async();
    Box::new(move |event| {
        handle
            .update(&mut cx, |_, window, cx| window.dispatch_event(event, cx))
            .log_err()
            .unwrap_or(DispatchEventResult::default())
    })
});
```

So the arrival point exists and is narrow. What is missing is any **hook** on it: the
closure has no policy in it, and nothing on `Application` can put one there.

### 2.3 Wrapping `Platform` is the dead end it looks like

`Platform` is the broadest trait in the layer — executors, text system, displays,
`open_window`, menus, dock menus, clipboard, credentials, keyboard layout and mapper,
notifications, prompts, thermal state, URL schemes
(`crates/gpui_platform/src/platform.rs:59-274`). It also does not carry input: it
only *creates* the window (`fn open_window`, `:93`), and input lives on the
`PlatformWindow` it returns. Filtering a mouse coordinate would therefore mean
delegating two large traits for a single callback. This is why the frame-pipeline
approach was rejected and why a dedicated seam is worth proposing instead.

### 2.4 The existing input hook is not this

`Context::observe_pending_input` (`crates/gpui_authoring/src/app/context.rs:521`)
subscribes to *pending* input — IME and dead-key state — not to raw arrival, and it
observes rather than filters. `WindowMetrics` carries geometry and window state and no
event information at all.

## 3. The proposed seam

A per-window input policy, consulted on arrival, with a flush on the frame boundary.

```rust
// Proposed. `PlatformInput` and `WindowId` are existing types.
pub trait InputPolicy: 'static {
    /// Consulted for every platform input event delivered for `window`, before
    /// `Window::dispatch_event` sees it.
    fn filter_event(&mut self, window: WindowId, event: PlatformInput) -> InputDecision;

    /// Consulted once per frame, before the frame's work, so a policy holding a
    /// coalesced event can release it. See §5 — without this, coalescing leaks.
    fn flush(&mut self, window: WindowId) -> Option<PlatformInput> {
        None
    }
}

pub enum InputDecision {
    /// Deliver this event — the same one, or a replacement.
    Admit(PlatformInput),
    /// Do not deliver this event. The policy is holding state and will release it
    /// from `flush`.
    Hold,
    /// Do not deliver this event, and hold nothing.
    Discard,
}
```

Facade hook, mirroring `with_frame_pipeline` (`crates/gpui_runtime/src/application.rs:105-109`)
and its `App::set_frame_pipeline_factory` counterpart
(`crates/gpui_authoring/src/app.rs:2937`):

```rust
impl Application {
    pub fn with_input_policy(
        self,
        policy: impl Fn(WindowId) -> Box<dyn InputPolicy> + 'static,
    ) -> Self
}
```

A factory rather than a shared `Arc` because arrival is per window and a coalescing
policy needs per-window state; this is the same reasoning that makes the frame
pipeline a factory and the text system a shared `Arc`.

The wiring site is the closure in §2.2, which gains one match:

```rust
Box::new(move |event| {
    handle
        .update(&mut cx, |_, window, cx| {
            match window.core.input_policy.filter_event(window_id, event) {
                InputDecision::Admit(event) => window.dispatch_event(event, cx),
                InputDecision::Hold | InputDecision::Discard => held_result(),
            }
        })
        .log_err()
        .unwrap_or(DispatchEventResult::default())
})
```

`WindowHost` would call `flush` where it already has a per-frame hook —
`should_render_frame` (`crates/gpui_authoring/src/window.rs:1341`) is the
narrowest existing one.

## 4. Semantics the sketch leaves open

These need answers before this is implementable, and none is obvious.

1. **`Discard` is `Hold` with nothing kept.** `InputDecision` above carries three
   variants; two would do, because a policy that discards simply holds no state. Kept
   separate only if a reviewer wants `Discard` to be documented as non-recoverable.
2. **What does a held event return to the platform?** `DispatchEventResult` is
   `{ propagate: bool, default_prevented: bool }`
   (`crates/gpui_platform/src/window.rs:610-613`). Holding is not the same as
   consuming, and claiming `propagate: false` for an event that was merely deferred
   may be observable to a backend. This is the least clear part of the proposal.
3. **Not everything is coalescible.** `PlatformInput` has 15 variants
   (`crates/gpui_platform/src/input.rs:336-365`). `MouseMove` and `ScrollWheel` are
   the coalescing targets; key, touch and `FileDrop` events must not be dropped at
   all, and a policy that does is a bug the type system does not prevent.
4. **Ordering.** A `Hold` followed by an `Admit` of a later event reorders arrival.
   Coalescing is a reordering by definition, but the constraint should be stated:
   events of one kind stay ordered, and a released event precedes nothing it did not
   precede on arrival.

## 5. The trap: a filtering gate would blind `InputRateTracker`

This is the finding that makes the seam more than a one-liner, and it is measurable
in the current code.

`InputRateTracker` (`crates/gpui_authoring/src/window.rs:1464`) latches a
one-second "high rate" sustain when enough input has arrived recently, and the frame
source uses it to keep presenting:

```rust
// crates/gpui_authoring/src/window.rs:2015-2017
let needs_present = request_frame_options.require_presentation
    || needs_present.get()
    || input_rate_tracker.borrow_mut().is_high_rate();
```

That is the VRR path: during continuous pointer input the window keeps presenting at
the display's rate instead of falling back to the inactive cadence.

**The threshold is 6 events per rolling 100 ms, not 60.** The tracker holds
`inputs_per_second: 60` and `window: 100ms`, and its test is

```rust
// crates/gpui_authoring/src/window.rs:1477-1480
let min_events = self.inputs_per_second as u128 * self.window.as_millis() / 1000;
if self.timestamps.len() as u128 >= min_events {
```

which is `60 * 100 / 1000 = 6`. So the trigger is **~60 events/second** sustained, not
600. (An earlier reading of this code as "60 events per 100 ms" overstated it by 10×,
and would have made the scheme look far more aggressive than it is.)

**The trap.** The count is fed from dispatch, not arrival:

```rust
// crates/gpui_authoring/src/window.rs:6111-6113
let caused_invalidation = self.core.invalidator.update_count() > update_count_before;
if caused_invalidation {
    self.core.input_rate_tracker.borrow_mut().record_input();
}
```

So the tracker counts what *survived* the policy. A gate that coalesces pointer input
to 60/s puts the tracker exactly on its `>=` boundary — every 100 ms window must
contain 6 events or the sustain lapses — and a gate below 60/s disables the sustain
entirely. The gate would therefore **degrade the ProMotion cadence it was added to
improve**, and no error would be raised.

Two ways out, and a reviewer should pick one:

- **Count arrivals, not dispatches.** Move `record_input()` to the `on_input`
  boundary, so the tracker sees what the OS sent and the policy's decisions are
  invisible to it. Smallest change, and it makes the tracker's meaning honest
  ("input is arriving fast") rather than accidental ("input survived filtering").
- **Have the policy report.** `filter_event` returns the decision, and the host
  records an arrival regardless of the decision. Equivalent, but spreads the
  knowledge.

Without one of these, this seam is a net regression for the case it exists to serve.

## 6. The flush problem

A pure `filter_event` hook cannot emit a coalesced event once a burst ends: there is
no timer at that seam, so the last pointer position of a gesture would be dropped
until some *later* event arrived to displace it. For a drag that ends in place, that
is a visible bug — the final position never reaches the element.

Hence `flush` in §3. But if coalescing is all that is wanted, there is a simpler
shape than a trait:

```rust
// Proposed alternative: declared, not programmed.
WindowOptions {
    input_coalescing: Option<Duration>,  // e.g. Some(Duration::from_millis(8))
    ...
}
```

The engine already owns per-window timers and a frame loop, so it can flush a
coalesced event on the frame boundary without a policy object at all. This is the
option to prefer *if* the requirement is "don't drown the tree in pointer moves". The
`InputPolicy` trait is worth having only if policies must be arbitrary — and if that
is the case, the flush hook is mandatory rather than optional.

## 7. If this lands, what `bite-gp-gate` becomes

Thin. With `InputPolicy` in place, a rate-controlled policy is a `filter_event` that
consults a rate schedule and a `flush` that releases the latest held event — on the
order of the `PassPipeline` that this investigation already wrote and tested
(`.pass/src/gpui_pass.rs`), and the rate tests carry over because the decision
is the same arrival check.

With the §6 declarative option instead, **no crate is needed at all**, and the honest
answer to "where does a rate limiter belong here?" is "nowhere in this design". That is worth
weighing before adding a seam: a seam is a permanent commitment, and this one would
exist principally to host a limiter whose measured benefit is narrower than its
apparent scope.

## 8. What would falsify this proposal

Stated so a reviewer can shoot it down cheaply:

- **If an input-rate problem is not reproducible**, there is nothing to gate. The
  motivating cases (8000 Hz pointer, PTY dumps, watcher storms) are all *plausible*,
  and none has been measured here. Two of them are also partly handled already: a
  notify burst coalesces to one dirty window, because the wake is edge-triggered on
  `dirty` and `dirty_views` is a set — though the observer effect is queued per notify
  call on the path where a window does display the entity, which
  [`../architecture/reactive-layer.md`](../architecture/reactive-layer.md) describes in
  full. And pointer input already drives the VRR sustain above rather than being
  throttled.
- **If a `Platform` decorator turns out cheap in practice**, the seam is unnecessary.
  It is not cheap in the trait's size, but nobody has tried it, and a prototype would
  settle the question better than this document.
- **If the coalescing-only requirement is the real one**, §6's declarative option
  removes the need for the trait, the policy object and the crate.

## 9. Recommendation

Do not add this seam in order to host a rate limiter. Reproduce the input-flood case
first, with numbers, on the target platform. If it reproduces:

1. Add §6's declarative `input_coalescing` if coalescing is sufficient — smallest
   change, no new trait, no new crate.
2. Add `InputPolicy` only if arbitrary policies are genuinely required, and then
   with §5's tracker fix in the same change, because without it the seam makes
   ProMotion input worse.
