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
| [`0002-ci-for-fork-prs.md`](0002-ci-for-fork-prs.md) | open | the `.tools` verification never runs on `bite-gpui/bite-gpui` pull requests; the ruleset rule that was proposed to fix it does not exist, and the real options are a workflow in the fork or a thin caller into `.tools` |
