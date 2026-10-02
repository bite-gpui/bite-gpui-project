# 0004 — Upstream the element inspector token

- **Opened:** 2026-09-28
- **Status:** open
- **Touches:** upstream `zed-industries/zed`, fork `vanuan/zed`,
  [`../architecture/layer-stack.md`](../architecture/layer-stack.md)

## The problem

The stack removes `inspector_id: Option<&InspectorElementId>` from `Element`'s
`request_layout`, `prepaint` and `paint`, and publishes the identity on the `Window` for
the duration of each lifecycle call instead. Upstream still threads the parameter, so the
removal is a divergence **from upstream** on a public, implementable trait — the one named
exception to the aim, ruled in
[`../architecture/layer-stack.md`](../architecture/layer-stack.md) §"The inspector token
stays off the authoring surface".

Carrying a divergence like that is a per-sync cost, not a one-off: every upstream edit to
the 30 files that take the parameter has to be re-stripped, and every *new* upstream
element arrives parameter-full. `crates/markdown/src/markdown.rs` is the tax paid on the
last `bite_master` rebase, and the parameter is still being added to new elements upstream
— `container_query`, the spring animations and markdown all moved it after our bases. The
fork carries it on every line (measured: 30 files on `origin/main` and on every base from
`v1.14.x` to `v1.22.0-pre`, versus one publication helper in the fork, which is not an
implementor at all), and each port inherits the decision: `bite_ce_main` keeps upstream's
shape and so diverges from the reference, which is the entry in
`.tools/docs/reference/divergence-ledger.md`.

Nothing measures this. `measure/api_surface.py` keys on `pub fn` and
`pub struct|enum|trait`, so **trait methods are outside its remit** — the limitation
recorded in `.tools/docs/reference/ce-rewrites.md`, found on `Platform::get_menus` and
now hit a second time, on `Element`. The checks that do work are the compiler (every
implementor stops compiling) and a diff of the trait declaration.

## What has been tried

The removal is the patchset's own work, not a rebase artifact. In the `gpui_engine`
boundary of the 111-commit patchset (`.tools/docs/journal/gpui_textsystem-log.md`),
both dated 2026-09-14:

| commit | what it did |
| --- | --- |
| `1b356da9ae` | hid the token from the docs and **kept the parameter** — "churning every in-tree `Element` implementation to remove it would cost more than it is worth while the crate boundaries are still moving" |
| `a4af8bcd47` | removed it as the follow-up, accepting that "sixty-one implementations outside the runtime" lose a parameter per method |

It has now been ported to upstream's tree, pushed as a branch, and opened as a pull request:

| | |
| --- | --- |
| branch | `gpui-drop-inspector-token` on `vanuan/zed` |
| base | upstream `main` at `1a28cff4b4` |
| commit | `79f8cb99d0` — `gpui: Publish the element's inspector identity instead of threading a token` |
| size | 31 files, `+210 −192` — gpui 19, ui 6, editor 2, and one each in markdown, terminal_view, workspace, image_viewer; the additions over the original `+70` are the public helper and the test |
| pull request | **opened** — `https://github.com/zed-industries/zed/pull/64854` |

A cherry-pick was impossible: the patchset's commit sits on the extracted layout
(`crates/gpui_runtime/src/element.rs`) and upstream is a monolith, so the port is the same
edit against upstream's paths. It was swept mechanically — 125 parameter lines, 3
`.as_ref()` call arguments, 25 bare call arguments, 12 inline call sites, and 6 anonymous
`_: Option<&InspectorElementId>,` bindings in `key_dispatch.rs`'s test implementations —
then hand-edited at the four places where the change is semantic: the three publication
sites in `Drawable`, `Window`'s new field and its two methods, `insert_inspector_hitbox`
reading the published identity, and `div.rs`'s two `with_inspector_state(_inspector_id,
…)` sites. The one trap is that a bare `inspector_id,` line is also how
`ElementDrawPhase` stores the id between phases, so nine of them had to be excluded from
the sweep.

`with_current_inspector_state` is `pub` on both sides now. The port first shipped it
`pub(crate)` to keep the change surface-neutral — three parameters removed, nothing added
— but that quietly removed a public extension point: `App::register_inspector_element`
accepts an element author's own state type, so an `Element` defined outside `gpui` must
be able to read its own state once the token is gone. Making the helper `pub` (as
`gpui_authoring` already had it) restores that. The method is still behind
`#[cfg(any(feature = "inspector", debug_assertions))]`, so a non-inspector build's
surface is unchanged.

Validated with `cargo check --all-targets` over `gpui`, `ui`, `editor`, `markdown`,
`terminal_view`, `workspace`, `image_viewer` and `inspector_ui` — every `Element`
implementor in the tree — with no errors and no warnings, and with
`cargo test -p gpui --lib inspector`: `inspector_only_tracks_its_open_window` (the
existing end-to-end pick/edit test) plus a new `element_reaches_its_own_inspector_state`
(a custom element selects the identity the runtime published for it and reaches its own
state through `with_current_inspector_state`). The full `cargo check --workspace` and
`script/clippy` have **not** been run.

## What would close it

- **Upstream takes it.** The divergence and the per-sync tax both end, `bite_ce_main`
  aligns for free, and the exception in `architecture/layer-stack.md` becomes history
  rather than a standing trade — the aim is restored exactly. Close this issue, and keep
  the branch until the release containing it is a base we target.
- **Upstream declines, or the pull request is closed without merging.** Then the layer-stack
  ruling's own axis decides: keep carrying it and keep paying, or revert to the parameter
  and delete the exception. The revert is bounded and mechanical — `Element`'s three
  signatures, `Drawable`'s three forwarding sites, and the 21 implementations in
  `gpui_authoring`, with the 13 outside it (ui 3, markdown 3, editor 2, and one each in
  workspace, terminal_view and image_viewer, plus 2 examples) reverting to upstream's
  text.
- **A smaller first step, if a maintainer prefers it:** `1b356da9ae` alone is documentation
  and annotations with no signature change. It is close to free and establishes the
  intent, which is what the removal's own commit message says it was the follow-up to.
