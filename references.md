# References

Where each piece lives, and what is in it. Remotes as configured on 2026-09-26, read
with `git remote -v` in each directory.

This is where everything lives as a *repository*. Where the same pieces live *online* —
the crates.io packages, the GitHub repositories and the Pages site, with the description
and topics each carries — is [`online-resources.md`](online-resources.md). The canonical
repository names, with the legacy name each came from, are
[`repositories.md`](repositories.md).

## What lives where

The project is five efforts in five places. This framing is the tools repository's
(`bite-tools/README.md`), kept here too because it is the fastest orientation for anyone who
arrives at this repository first.

| effort | what it is | where | its documents |
| --- | --- | --- | --- |
| **the architecture** | GPUI re-cut into layer crates, carried as a stack of forward commits against upstream | the `bite_*` branches of `bite-gpui/bite-gpui` | [`architecture/layer-stack.md`](architecture/layer-stack.md) and the rest of this repository |
| **the tooling** | applying the architecture to a target — replay, conflict settlement, per-target carves — and measuring the result | `bite-tools` — `bite-gpui/bite-tools` | `docs/workflows.md` (all three operations, end to end), `docs/architecture.md` (building and releasing the stack), `docs/rebase-handoff.md` (the replay playbook), `docs/reference/`, `docs/journal/` |
| **distribution** | rewriting the stack into publishable crates and releasing them to crates.io | `bite-distribution` — `bite-gpui/bite-distribution` | `DESIGN.md` §1–§2, `docs/contract.md` §3–§6, `docs/staging.md` §7–§9, `docs/verification.md` §10, `docs/release.md` §11–§12, `docs/decisions.md` §13, `targets.toml` |
| **the website** | marketing and user-level documentation | `bite-gpui.github.io` — `bite-gpui/bite-gpui.github.io` | `design/DESIGN-SYSTEM.md`, `src/` (Astro) |
| **actual uses** | outside-in exercises: a demo app, a text system, a layout engine, an out-of-tree renderer, pipeline/runtime experiments | the usage suite and separate repositories | `README.md` (the measurement policy), `parley-demo/`, `scroll-demo/`, each with a `benchmarks/` record |

### The out-of-tree crates

| crate | tier | what it is | repository | here |
| --- | --- | --- | --- | --- |
| `bite-gp-parley` | swap | replaces `TextSystem` | `bite-gpui/gpui_parley` | `gpui_parley` |
| `bite-gp-morphorm` | swap | replaces `LayoutEngine` | `bite-gpui/gpui_morphorm` | `gpui_morphorm` |
| `bite-gp-pass` | wrap | throttles the frame pipeline by a rate policy and keeps a ledger of what its passes cost — published as `1.21.3` | `bite-gpui/gpui_pass` | `gpui_pass` |

Each carries `src/`, `tests/`, `examples/`, `benches/` and a dated `benchmarks/` record.
See [`architecture/extension-tiers.md`](architecture/extension-tiers.md) for the two
tiers, [`spi/README.md`](spi/README.md) for the seams a swap implements, and
[`decisions/0001-no-third-swap.md`](decisions/0001-no-third-swap.md) for why the swap
tier stops at two.

### Inside `bite-tools`

Worth knowing before opening it. `docs/` is three kinds, kept apart on purpose —
**live** (describes the project as it is now), **journal** (a dated record of a session,
evidence rather than instruction, never edited to match a later layout) and
**reference** (durable per-target material). The root `README.md` is a catalogue of
every script by group — `replay/`, `conflicts/`, `carve/`, `measure/`, `reports/`,
`packaging/`, `runs/`, `repo/` — each entry the script's own one-line summary.

## Upstream and the ce lineage

Remotes on the main clone, for orientation:

| remote | URL | what |
| --- | --- | --- |
| `bite-gpui` | `git@github.com:bite-gpui/bite-gpui.git` | this project |
| `upstream`, `origin` | `https://github.com/zed-industries/zed.git` | zed |
| `community` | `git@github.com:gpui-ce/gpui-ce.git` | the ce lineage |
| `vanuan` | `git@github.com:Vanuan/zed.git` | a personal fork remote |

## Naming and versions

Deliberately not restated here. They are specified once, in `bite-distribution`, and a
second copy would go stale:

- crate naming (`gpui_X` → `bite-gp-X`) and the version scheme
  (`major.minor.(patch * 100 + amendment)`) — `bite-distribution/docs/contract.md` §5–§6
- the target table, and therefore the full list of published names —
  `bite-distribution/targets.toml`
- publishing decisions — `bite-distribution/docs/decisions.md` §13

## The working copies

Each effort is its own repository, checked out as a parallel working copy beside the
others rather than nested inside the clone: `bite-gpui` (the source), `bite-distribution`,
`bite-tools`, `bite-gpui.github.io`, `gpui_parley`, `gpui_morphorm`, `gpui_pass`, and this
repository. The usage suite (`bite_gpui_scroll_demo` and the other exercises) is a
separate set of consumer repositories. Committing any of these working copies into a
`bite_*` branch would duplicate it across twelve branches, so they stay apart by living
in their own repositories.

| working copy | its own repository? |
| --- | --- |
| `bite-gpui`, `bite-distribution`, `bite-tools`, `bite-gpui.github.io`, `gpui_parley`, `gpui_morphorm`, `gpui_pass`, this repository | yes |
| the usage suite | the demos are separate repositories; `.uses/` itself is a working directory, not a repository |

`gpui_pass` is the wrap's repository — `gpui_pass`, which throttles a window's frame rate
and keeps a ledger of what its passes cost. It was renamed from `gpui_governor` when the
rate limiter it was first built on turned out to be the wrong tool for the seam, and grew
the ledger half afterwards; it is the crate in the second tier of
[`architecture/extension-tiers.md`](architecture/extension-tiers.md), and it is why
[`decisions/0001-no-third-swap.md`](decisions/0001-no-third-swap.md) reads as a decision
about *swaps* specifically. `gpui_pass/tests/` holds the numbers that decision's "What would
reopen this" rests on — 121 frames in five seconds for the pass at 24 fps on a 60Hz ask
grid, against 150 (30 fps) and 100 (20 fps) for the shipped rule transcribed onto the same
clock — and `gpui_pass/benchmarks/2026-09-27-frame-pass.md` records the run. If the
repository is deleted the record survives, but its figures become unverifiable. That is a
choice to make deliberately.
