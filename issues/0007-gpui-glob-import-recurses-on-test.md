# 0007 — `use gpui::*;` glob-imports the `test` macro and recurses forever

- **Opened:** 2026-10-01
- **Status:** open
- **Touches:** the `bite-gpui` facade `crates/gpui/src/gpui.rs:43`, the re-export at
  `crates/gpui_authoring/src/gpui_authoring.rs:124`, and the published usage claim on
  `bite-gpui.github.io`

## The problem

`use gpui::*;` is not a harmless glob. The facade re-exports `gpui_authoring`'s whole
surface (`crates/gpui/src/gpui.rs:43`), which includes the `test` **attribute macro**
(`crates/gpui_authoring/src/gpui_authoring.rs:124`). Importing it as a glob pulls
`test` into the macro namespace, where it **shadows the builtin `#[test]`**.

The consequence is an infinite expansion, not a compile error. `#[gpui::test]` expands
to an outer `#[test] fn …`, and because `test` is now in scope, that generated
`#[test]` re-resolves to `gpui::test` instead of the builtin — so the proc-macro
invokes itself again, forever. It surfaces as
`recursion limit reached while expanding #[test]`, and `-Zmacro-backtrace` OOMs
(exit 137) because the expansion genuinely does not terminate.

## The evidence

Observed against `bite-gpui` v1.21.0 in the sandbox (2026-10-01):

- An **empty-bodied** test — `#[gpui::test] fn minimal(cx) { let _ = cx; }` — still
  recursed, so the failure is not in the test body. The macro source generates a single
  bounded `#[test]`, so the culprit had to be name resolution.
- The official example imports explicitly plus `prelude::*`, never the bare glob.
- Switching to `use gpui::{…}; use gpui::prelude::*;` fixed it immediately.

## The rule

Never write `use gpui::*;` in this codebase. Write the imports out, or at most
`use gpui::prelude::*;`.

## What would close it

- The guidance is written where a consumer will meet it — the published usage docs on
  `bite-gpui.github.io`, which currently say `use gpui::*` compiles unchanged — and the
  contradiction corrected there.
- Decided whether to harden the facade itself (for example, not re-exporting `test` in
  the glob, or documenting the footgun at the re-export) or to leave it as a known
  consumer rule.
