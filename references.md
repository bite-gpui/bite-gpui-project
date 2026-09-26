# References

Where each piece lives, and what is in it. Remotes as configured on 2026-09-26, read
with `git remote -v` in each directory.

## What lives where

The project is five efforts in five places. This framing is the tools repository's
(`.tools/README.md`), kept here too because it is the fastest orientation for anyone who
arrives at `.meta` first.

| effort | what it is | where | its documents |
| --- | --- | --- | --- |
| **the architecture** | GPUI re-cut into layer crates, carried as a stack of forward commits against upstream | the `bite_*` branches of `bite-gpui/bite-gpui` | [`architecture/layer-stack.md`](architecture/layer-stack.md) and the rest of this repository |
| **the tooling** | applying the architecture to a target — replay, conflict settlement, per-target carves — and measuring the result | `.tools` — `bite-gpui/tools` | `docs/workflows.md` (all three operations, end to end), `docs/architecture.md` (building and releasing the stack), `docs/rebase-handoff.md` (the replay playbook), `docs/reference/`, `docs/journal/` |
| **distribution** | rewriting the stack into publishable crates and releasing them to crates.io | `.dist` — `bite-gpui/distribution` | `DESIGN.md` §1–§2, `docs/contract.md` §3–§6, `docs/staging.md` §7–§9, `docs/verification.md` §10, `docs/release.md` §11–§12, `docs/decisions.md` §13, `targets.toml` |
| **the website** | marketing and user-level documentation | `.website` — `bite-gpui/bite-gpui.github.io` | `design/DESIGN-SYSTEM.md`, `src/` (Astro) |
| **actual uses** | outside-in exercises: a demo app, a text system, a layout engine, an out-of-tree renderer, pipeline/runtime experiments | `.uses` (a directory, not a repository) and separate repositories | `README.md` (the measurement policy), `parley-demo/`, `scroll-demo/`, each with a `benchmarks/` record |

### The published swaps

| crate | what it replaces | repository | here |
| --- | --- | --- | --- |
| `bite-gp-parley` | `TextSystem` | `bite-gpui/gpui_parley` | `.parley/` |
| `bite-gp-morphorm` | `LayoutEngine` | `bite-gpui/gpui_morphorm` | `.morphorm/` |

Each carries `src/`, `tests/`, `examples/`, `benches/` and a dated `benchmarks/` record.
See [`spi/README.md`](spi/README.md) for the seam each one implements, and
[`decisions/0001-no-third-swap.md`](decisions/0001-no-third-swap.md) for why there is no
third.

### Inside `.tools`

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

Deliberately not restated here. They are specified once, in `.dist/`, and a second copy
would go stale:

- crate naming (`gpui_X` → `bite-gp-X`) and the version scheme
  (`major.minor.(patch * 100 + amendment)`) — `.dist/docs/contract.md` §5–§6
- the target table, and therefore the full list of published names —
  `.dist/targets.toml`
- publishing decisions — `.dist/docs/decisions.md` §13

## The working copies, and what is excluded

`.dist/`, `.tools/`, `.website/`, `.uses/`, `.parley/`, `.morphorm/`, `.meta/` and
`.governor/` are excluded in the zed clone via `.git/info/exclude`. Each is a separate
concern, and committing any of them to a `bite_*` branch would duplicate it across twelve
branches.

| directory | its own repository? |
| --- | --- |
| `.dist/`, `.tools/`, `.website/`, `.parley/`, `.morphorm/`, `.meta/` | yes |
| `.uses/` | no — a directory in the clone |
| `.governor/` | no — untracked, holds no published artifact |

`.governor/` is the working copy of the swap that
[`decisions/0001-no-third-swap.md`](decisions/0001-no-third-swap.md) decided against
shipping. Worth knowing what lives there and nowhere else: `.governor/tests/` is the
**only executable evidence** for that decision's numbers — 61 frames per two seconds for
the limiter at 24 fps, against 60 and 40 for the shipped rule transcribed onto the same
clock. If the directory is deleted the record survives, but its figures become
unverifiable. That is a choice to make deliberately.
