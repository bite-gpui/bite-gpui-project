# 0001 — A citation checker

- **Opened:** 2026-09-26
- **Status:** closed — `script/check-citations` is built and every citation resolves against the canonical ref
- **Touches:** every document under `.meta` that cites source, and a `bite_*` checkout

## The problem

`.meta` cites the source rather than quoting it — `crates/gpui_authoring/src/app.rs:2791`
— because a citation is what lets a reader check a claim in seconds. Line numbers drift,
and nothing checks them.

One session caught **ten** wrong line numbers by hand: five in the first draft of
`architecture/reactive-layer.md`, and five more in the same file where I had cited
impl-block starts instead of function definitions. Reading the document did not catch
them. Re-deriving every number with `grep -n` did.

Scale today: **72 citations carrying a line number, across 7 documents.**

| document | citations |
| --- | --- |
| `architecture/reactive-layer.md` | 32 |
| `spi/input-policy-seam.md` | 15 |
| `decisions/tracy-swap-scope.md` | 13 |
| `spi/README.md` | 5 |
| `issues/0001-check-citations.md` | 5 |
| `spi/dual-path-render-extension.md` | 1 |
| `architecture/frame-flow.md` | 1 |

That excludes the citations that name a file without a line, which are deliberately
outside this check — `crates/gpui_authoring/src/window/frame_pipeline.rs` is stable, and
a line number there would be noise.

## What the checker has to handle

1. **Two citation forms.** A repo-relative path
   (`crates/gpui_authoring/src/window.rs:4242`) and a bare filename (`app.rs:654`) that
   relies on the reader's context. Bare names are ambiguous across crates: `window.rs`
   resolves to `gpui_authoring` or `gpui_platform`, and `lib.rs` and `frame.rs` appear in
   several crates. The checker resolves a bare name under `gpui_authoring/src`, and
   anything from another crate has to say so — which is the rule `check-citations`
   enforces by reporting the rest as unresolved.
2. **Ranges.** `app.rs:2798-2810`, which is a claim about a span rather than a line.
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
`path:line` followed by that line's current content — all 73 in one pass — would have
caught every one of the ten errors, because a wrong number is obvious when the line is in
front of you. It needs no change to how the documents are written.

The stricter version — fail when the cited line does not contain what the document claims
— needs the citation to carry the expected token (`app.rs:654 pending_effects`), which is
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

## Resolution

`script/check-citations` is built, the canonical ref is recorded in `README.md`, and every
citation in the repository resolves against it: **73 citations, 73 resolved, 0 unresolved,
0 out of range, 0 missing, 0 on blank lines.**

The migration is the interesting part. Adopting a ref did not only need a convention
written down — it exposed that most of the citations in `architecture/reactive-layer.md`
and `spi/input-policy-seam.md` had been derived from the checkout, which was on
`bite_v1.14.x`, and were therefore wrong against the ref the documents were meant to
describe. Ten bare filenames also had to become full paths, because `span.rs` and
`state.rs` resolve to nothing under `gpui_authoring`.

**What this does not fix.** The checker prints; it does not judge. It proves a file exists
and a line is in range, and it flags a blank line, which is usually drift. It cannot tell a
correct line number from a plausible wrong one — and the migration above was verified by
reading the printed lines, not by the tool. The stricter form, where a citation carries its
expected token so a wrong number fails and a moved one can be relocated automatically, is
the next step, and it is a change to the documents rather than to the tool.

Not opened as its own issue, because the trigger is a second ref change: the next time the
ref moves, this migration has to be redone by hand, and that is when the token convention
pays for itself.
