# References

Where each piece lives. Remotes as configured on 2026-09-26, read with
`git remote -v` in each directory.

## The work, by concern

| concern | location | repository |
| --- | --- | --- |
| the rearchitecture itself | the twelve `bite_*` branches | `git@github.com:bite-gpui/bite-gpui.git` |
| publishing those branches | `.dist/` | `git@github.com:bite-gpui/distribution.git` |
| migration scripts | `.tools/` | `git@github.com:bite-gpui/tools.git` |
| architecture and SPI record | `.meta/` | `git@github.com:bite-gpui/bite-gpui-project.git` |
| user-facing site | `.website/` | `https://github.com/bite-gpui/bite-gpui.github.io` |
| outside-in usage exercises | `.uses/` | none — a directory in the clone, excluded, not a repository |
| the published swaps | `.parley/`, `.morphorm/` | `bite-gpui/gpui_parley.git`, `bite-gpui/gpui_morphorm.git` |
| a swap that does not ship | `.governor/` | none — untracked in the clone; see [`decisions/0001-no-third-swap.md`](decisions/0001-no-third-swap.md) |

## Upstream and the ce lineage

Remotes on the main clone, for orientation:

| remote | URL | what |
| --- | --- | --- |
| `bite-gpui` | `git@github.com:bite-gpui/bite-gpui.git` | this project |
| `upstream`, `origin` | `https://github.com/zed-industries/zed.git` | zed |
| `community` | `git@github.com:gpui-ce/gpui-ce.git` | the ce lineage |
| `vanuan` | `git@github.com:Vanuan/zed.git` | a personal fork remote |

## Naming and versions

Deliberately not restated here. They are specified once, in `.dist/`, and a second
copy would go stale:

- crate naming (`gpui_X` → `bite-gp-X`) and the version scheme
  (`major.minor.(patch * 100 + amendment)`) — `.dist/docs/contract.md` §5–§6
- the target table, and therefore the full list of published names —
  `.dist/targets.toml`
- publishing decisions — `.dist/docs/decisions.md` §13

## Excluded from the clone

`.dist/`, `.tools/`, `.website/`, `.uses/`, `.parley/`, `.morphorm/` and `.meta/` are
excluded via `.git/info/exclude`. Each is a separate concern, and committing any of
them to a `bite_*` branch would duplicate it across twelve branches.

`.governor/` was untracked and visible in `git status` (0 tracked files, 13
untracked entries) and has now joined the same exclude list, matching its siblings.
It holds no published artifact: it is the working copy of the swap that
`decisions/0001-no-third-swap.md` decided against shipping.

Worth knowing what lives there and nowhere else: `.governor/tests/` is the **only
executable evidence** for that decision's numbers — 61 frames per two seconds for the
limiter at 24 fps, against 60 and 40 for the shipped rule transcribed onto the same
clock. If the directory is deleted the record survives, but its figures become
unverifiable. That is a choice to make deliberately.
