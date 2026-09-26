# Scoping `bite-gp-tracy`: a telemetry `FramePipeline` swap

**Status: dropped.** `tracy-client-sys` compiles `tracy/TracyClient.cpp` with
`cc::Build::cpp(true)`, and a C++ toolchain requirement is not acceptable for a
published `bite-gp-*` crate, so this swap does not ship and no fourth version slot is
opened for it. The note is kept as the record of what the swap would have been and
why it stops here, so the ground is not re-covered.

If the objection is specifically to C++ rather than to telemetry-as-a-swap, `puffin`
is the pure-Rust alternative (in-process, no `-sys` crate) — but it would need these
same decisions re-made, including the version slot, and its advantage still would not
be exercisable in CI. Read §1 and §6 before starting it.

**Original scope follows.** Written after the `InputPolicy` seam proposal, per the
same investigation.

Everything below that names a `tracy_client` item was read from the published source
in the local registry — `tracy-client 0.18.3` and `tracy-client-sys 0.27.0`, both
already present in the `zed` workspace lock. No API here is from memory.

---

## 1. What it would claim, and the honest weakness

`bite-gp-parley` and `bite-gp-morphorm` each carry a headline number: parley a text
capability, morphorm `0.00 px` delta and `3.4×–3.8×`. **`bite-gp-tracy` cannot carry
one of the same kind**, and that is the first thing to decide, because it is what
"earns its spot on `/swaps`" has meant so far.

Tracy's value is a *capability*, not a measurement: an out-of-process nanosecond
timeline with frame markers, scheduler migrations and per-frame zoom, instead of the
in-process sums and counts that `InstrumentedPipeline` accumulates. The measurable
part is only its **cost**: nanoseconds per span, which is a number you pay, not a
number you win.

That matters for CI. Morphorm's facade test is what caught a transparent window; the
governor's rate tests are what caught a wrong burst preset. A tracy swap has no
equivalent gate, because it is only useful with an external GUI attached, and a
disconnected client is a no-op. So a published artefact whose advantage cannot be
exercised in CI needs a different entry argument than the first two swaps.

## 2. Crate identity, per the distribution contract

Following `.dist/docs/contract.md` §5 (`gpui_X` → `bite-gp-X`) and the same shape as
the other two:

```toml
[package]
name = "bite-gp-tracy"
version = "1.21.?"        # see below
edition = "2024"
license = "Apache-2.0"

[lib]
name = "gpui_tracy"       # invariant 2: the code says `use gpui_tracy::…`
path = "src/gpui_tracy.rs"

[features]
demo = ["dep:gpui", "gpui/platform"]
test-support = ["dep:gpui", "gpui/platform", "gpui/test-support"]
```

**The version slot is undecided.** §6 of the contract is
`major.minor.(patch * 100 + amendment)`, which gives 1.21.1 to parley and 1.21.2 to
morphorm. 1.21.3 was reserved for `bite-gp-governor`. If governor does not ship,
tracy should take 1.21.3; if it does, tracy takes 1.21.4. Do not pick one until that
is settled — a published version cannot be reused.

**Blocker: `tracy-client` is a stub without its `enable` feature.** `Client::start()`
returns `Self(())` and does nothing unless the dependency's `enable` feature is on:

```rust
// tracy-client-0.18.3/src/state.rs:27-35
pub fn start() -> Self {
    #[cfg(not(feature = "enable"))]
    return Self(());
    ...
}
```

So the crate must *forward* this feature rather than enable it, because a published
`bite-gp-*` dependency must not switch profiling on in a consumer's release build. The
shape to decide: a `tracy` feature that forwards `tracy-client/enable`, off by default,
documented in the README. Every claim in §5 depends on it — without `enable`,
`running()` is always `None` and both tracy columns collapse into the floor column.

**Blocker: `tracy-client-sys` compiles C++.** From its `build.rs`:

```rust
let mut builder = set_feature_defines(cc::Build::new());
builder
    .file("tracy/TracyClient.cpp")
    .cpp(true);
```

So any consumer of this swap builds and links the Tracy C++ client. Morphorm's CI
action states the opposite posture for its own crate — *"Nothing the library links is
a system library"* — and both existing swaps are pure Rust. A C++ toolchain
requirement is a deliberate distribution decision, not an implementation detail, and
it should be made before scaffolding rather than after.

## 3. Module layout

Keeping to the "one logical component per file, no `mod.rs`" rule:

```
bite-gp-tracy/
  Cargo.toml
  README.md
  src/gpui_tracy.rs        # the crate root: trait impl, docs, builder ext
  src/spans.rs             # the span locations, one per pass, and the guard helper
  tests/facade.rs          # the swap installs and frames still draw (the morphorm lesson)
  benches/frame_pipeline.rs# the cost column, measured against the in-tree decorator
  examples/tracy_demo.rs   # a window that streams a few frames to a running Tracy GUI
  benchmarks/<date>-tracy-swap.md
```

## 4. The real API, and where the sketch needs correcting

Three corrections to the sketch this scope was requested from, all from the published
source:

1. **`WindowContext` does not exist.** The trait methods take `window: &mut Window<'_>`
   and `cx: &mut App`.
2. **`layout_roots` and `paint_roots` take roots.** They are
   `layout_roots(&mut self, window, roots: &mut PreparedRoots, cx)` and
   `paint_roots(&mut self, window, roots: PreparedRoots, cx)`.
3. **`Client::zone(...)` is not the 0.18 API.** There is no `zone` method on `Client`.
   Spans are created from a `&'static SpanLocation`:

```rust
// tracy-client-0.18.3/src/span.rs:62
impl Client {
    pub fn span(self, loc: &'static SpanLocation, callstack_depth: u16) -> Span
}
```

and a `SpanLocation` comes from the `span_location!` macro
(`span.rs:207`), which caches one `Lazy` per call site:

```rust
let location: &'static tracy_client::SpanLocation =
    tracy_client::span_location!("some name");
```

`Span` is an RAII guard — `impl Drop for Span` (`span.rs:181`) emits the span end — so
the binding **must be named and must outlive the pass it measures**. Binding it to
`_` drops it immediately and the span measures nothing:

```rust
fn begin_frame(&mut self, window: &mut Window<'_>, cx: &mut App) {
    // `_span`, not `_`: `_` drops the guard at the end of the statement.
    let _span = Client::running().map(|client| {
        client.span(span_location!("gpui::begin_frame"), 0)
    });
    self.inner.begin_frame(window, cx);
}
```

Other pinned facts worth having:

- `Client::start() -> Client` and `Client::running() -> Option<Client>`
  (`state.rs:27`, `state.rs:38`). The `Option` is what makes a disconnected client
  cheap: no span is created at all.
- `frame_mark()` is a free function (`frame.rs:171`), re-exported at the crate root
  (`lib.rs:30`), alongside `Frame::frame_mark(&self)` (`frame.rs:61`).
- `frame_mark()` *closes* a frame in Tracy's timeline, so it belongs at the end of a
  frame — `end_frame`, not `begin_frame` as sketched — if the marker bars are to line
  up with the passes they contain.
- `Client` is passed by value to `span` and derives nothing, so it is **not** `Copy` —
  but `Client::running()` constructs a fresh `Client(())` on each call
  (`state.rs:38-44`), so calling it per pass costs a unit construction behind a boolean
  check rather than a clone. There is no handle to hold on to and nothing to cache.
- `Span::emit_value` / `emit_text` / `emit_color` (`span.rs:150-171`) are available if a
  pass wants to attach numbers (node counts, layout iterations) to its own span.
- `ProfiledAllocator<T>` (`lib.rs:220`) is a separate capability (allocation tracking)
  and is out of scope for a pipeline decorator.

## 5. Benchmark design

Per `.uses/README.md`, a benchmark has two columns: two implementations of one trait
through the same call. Three here, same `FramePipeline` call, per frame:

| column | what it is |
| --- | --- |
| `StandardImmediatePipeline` | the floor |
| `StandardImmediatePipeline.instrumented(metrics)` | the in-tree decorator, `Instant::now()` per phase |
| `TracyPipeline<StandardImmediatePipeline>` | this crate, no client running (`Client::running() == None`) |

And a fourth, once, to bound the real cost: the same `TracyPipeline` with
`Client::start()` called and no server attached, which exercises span creation rather
than the `Option` short-circuit. The difference between the two tracy columns is the
number that actually belongs in the README, because it is the one a user pays. **Run
the benchmark with the `tracy` feature on**, or `enable` is absent, `running()` is
always `None`, and the two tracy columns are identical to the floor.

Reported as ns per frame and ns per span. The harness is a headless frame loop — the
pipeline's decision is measurable through `WindowMetrics` without a display, which is
how the governor crate's benchmark was kept buildable here.

What this cannot produce is a figure analogous to morphorm's, and the README should
not pretend otherwise. The claim is: *adds N ns per frame disconnected; gives back a
nanosecond external timeline that per-phase accumulators cannot.*

## 6. `InstrumentedPipeline` is the thing to beat

Read from `crates/gpui_runtime/src/pipeline.rs:195-230`: it wraps each root pass in
`Instant::now()`, adds the elapsed time to a running sum in `PhaseMetrics`, and
increments a pass counter — inside `Rc<RefCell<PhaseMetrics>>`, on the main thread, in
process.

The genuine differences are therefore:

- **Aggregate vs. individual.** Sums and counts give an average; they cannot show the
  one frame from three minutes ago that hitched, nor which of two passes in that frame
  stalled.
- **In-process vs. out-of-process.** Tracy's overhead per zone is a check plus a
  write to a shared ring buffer; aggregation and graph rendering happen in the Tracy
  GUI.
- **What the OS did.** Scheduler migration and preemption are not visible to an
  `Instant` delta — a long `layout_roots` and a preempted `layout_roots` look identical.
- **Frame cadence as a first-class row.** `frame_mark` gives continuous cadence bars to
  read pass durations against.

If a reviewer finds none of those decisive, `InstrumentedPipeline` already covers the
case, and the swap should not ship. That is the honest test of "earns its spot".

## 7. What would falsify it

- **Overhead is not small.** If the disconnected column is more than a few tens of ns
  per frame, or the connected column is more than a few µs, a telemetry decorator is
  competing with the thing it measures. Measure before publishing.
- **The C++ dependency is unacceptable for the distribution.** Then the swap cannot
  ship as a `bite-gp-*` crate at all, whatever its merits.
- **`InstrumentedPipeline` is enough in practice**, per §6.

## 8. Recommendation

Sequence it behind the version-slot decision and the C++ question, and do the
overhead measurement *first* — it is one benchmark file and it decides the crate.
Then, if it proceeds: scaffold `src/`, `tests/facade.rs` (the swap installs and a
frame still draws — the check that caught morphorm's invisible window), and the
benchmark record. The demo is the only part that cannot be verified here, for the
same reason the governor demo could not: no display.
