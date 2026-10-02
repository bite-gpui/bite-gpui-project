# Issues

The tracker for work that spans repositories — the things with no single branch to
live on.

An issue that concerns one branch belongs on that branch's tracker. An issue that
concerns the *relationship* between branches, or between a branch and a published
crate, has no home there and lives here.

## Convention

One file per issue, `NNNN-short-title.md`, numbered in order of opening. It opens with
a small header block:

```
- **Opened:** 2026-09-26
- **Status:** open | blocked | closed
- **Touches:** `.meta/spi/…`, `.dist/…`, branch `bite_v1.21.x`
```

then what the problem is, what has been tried, and what would close it.

Closed issues are kept. A tracker that deletes its history is a tracker whose
questions get re-argued.

## Index

| issue | status | subject |
| --- | --- | --- |
| [`0001-check-citations.md`](0001-check-citations.md) | closed | 73 citations carry a line number and nothing verified them; `script/check-citations` now resolves every one against `bite_v1.22.0-pre` |
| [`0002-ci-for-fork-prs.md`](0002-ci-for-fork-prs.md) | open | the CI in `bite-gpui/distribution` never runs on `bite-gpui/bite-gpui` pull requests; a thin caller into `distribution`'s `pr-checks` entry point answers it and is on `bite_v1.22.0-pre`, leaving the ruleset gate and the other branches |
| [`0003-scheduled-builds.md`](0003-scheduled-builds.md) | open | the pull-request gate is deliberately minimal; what it leaves out — `--all-features`, wasm32, clippy, the tests, the packaging checks — needs a scheduled build to run on |
| [`0004-inspector-token-upstream.md`](0004-inspector-token-upstream.md) | open | the stack removes `inspector_id` from `Element` while upstream still threads it, so every sync re-pays the divergence on a public trait; ported to upstream's tree and pushed as `gpui-drop-inspector-token` on `vanuan/zed`, with the pull request still to open |
| [`0005-view-retention.md`](0005-view-retention.md) | open | retained (incremental) view rendering is being built outside the stack — Zed's view tree (`#63800`, split for landing, prerequisites already on `main`) and `gpui-fast#2` (merged) — and is not a seam, so it is the first use the layer rulings cannot answer; adopt, defer or reject, and in which shape |
| [`0006-surface-interop.md`](0006-surface-interop.md) | open | the surface implementation spans an upstream repo and a downstream `gpui-interop` crate — two upstream PRs, two out-of-tree arms, the bridge, and the probes that gate them; the coordination the decision (`0005`), the design (`surfaces.md`) and the plan (`surface-plan.md`) do not carry |
| [`0007-gpui-glob-import-recurses-on-test.md`](0007-gpui-glob-import-recurses-on-test.md) | open | `use gpui::*;` glob-imports the `test` attribute macro, which shadows the builtin `#[test]` and makes `#[gpui::test]` recurse forever; import explicitly instead |
