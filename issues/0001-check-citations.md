# 0001 — A citation checker

- **Opened:** 2026-09-26
- **Status:** open
- **Touches:** every document under `.meta` that cites source, and a `bite_*` checkout

## The problem

`.meta` cites the source rather than quoting it — `crates/gpui_authoring/src/app.rs:2785`
— because a citation is what lets a reader check a claim in seconds. Line numbers drift,
and nothing checks them.

One session caught **ten** wrong line numbers by hand: five in the first draft of
`architecture/reactive-layer.md`, and five more in the same file where I had cited
impl-block starts instead of function definitions. Reading the document did not catch
them. Re-deriving every number with `grep -n` did.

Scale today: **67 citations carrying a line number, across 6 documents.**

| document | citations |
| --- | --- |
| `architecture/reactive-layer.md` | 32 |
| `spi/input-policy-seam.md` | 15 |
| `decisions/tracy-swap-scope.md` | 13 |
| `spi/README.md` | 5 |
| `spi/dual-path-render-extension.md` | 1 |
| `architecture/frame-flow.md` | 1 |

That excludes the citations that name a file without a line, which are deliberately
outside this check — `crates/gpui_authoring/src/window/frame_pipeline.rs` is stable, and
a line number there would be noise.

## What the checker has to handle

1. **Two citation forms.** A repo-relative path
   (`crates/gpui_authoring/src/window.rs:4113`) and a bare filename (`app.rs:655`) that
   relies on the reader's context. Bare names are ambiguous across crates: `window.rs`
   resolves to `gpui_authoring` or `gpui_platform`, and `lib.rs` and `frame.rs` appear in
   several crates. Either the checker gets a per-document base directory, or the
   convention should drop bare names in favour of full paths.
2. **Ranges.** `app.rs:2798-2804`, which is a claim about a span rather than a line.
3. **Targets outside the repository.** Five citations point at a third-party crate's
   published source — `tracy-client-0.18.3` read from the cargo registry, not from any
   `bite_*` branch. Those need a second resolver or an explicit marker so they are not
   silently reported as missing.
4. **Which ref the numbers are against.** This is the substantive one. Line numbers are
   per-branch, and there are twelve `bite_*` branches that differ.
   `architecture/layer-stack.md` names its ref (`bite_v1.22.0-pre`, `0ff0532eb7`);
   `frame-flow.md` and `reactive-layer.md` do not, and were verified against whatever the
   clone happened to have checked out. **The documents do not agree on a ref, and none of
   them except `layer-stack.md` says which one it used.** Decide a canonical ref — the
   newest release line, or the stack's `regenerated` branch — and record it once, in
   `README.md`'s conventions.

## The shape to start with

**Print, don't judge.** A script that extracts every citation and prints
`path:line` followed by that line's current content — all 67 in one pass — would have
caught every one of the ten errors, because a wrong number is obvious when the line is in
front of you. It needs no change to how the documents are written.

The stricter version — fail when the cited line does not contain what the document claims
— needs the citation to carry the expected token (`app.rs:655 pending_effects`), which is
a change to the documents' convention. Worth doing only if the print version turns out to
be run too rarely to matter.

## Open questions

- **Where it lives.** `.meta`, because it is this repository's own hygiene, taking the
  checkout path as an argument — or `.tools`, which already has the worktree machinery and
  a script catalogue, but is nominally about syncing source-of-truth changes. The second
  home is a worse fit for the purpose and a better fit for the tooling.
- **When it runs.** By hand while writing a document, or as CI on `.meta`? `.meta` has no
  CI today, and this would be its first reason to have any.
- **What it does about the ref** once one is chosen: resolve citations against that ref
  regardless of what is checked out (`git show <ref>:<path>`), or require the checkout to
  be on it and fail loudly otherwise.

## What would close it

A `check-citations` that runs over `.meta/**/*.md`, resolves the forms above, prints every
citation with its current line content, and exits non-zero only when a cited file does not
exist or the line is out of range. Plus the canonical ref written down in `README.md`.
