# Agent instructions — `bite-gpui-project`

What an agent should know before editing this repository: the layout it is part of,
how the citations are verified, and the traps that make an edit go stale or wrong.

## What this repository is

`bite-gpui-project` is the architecture and interface record for the bite-gpui
rearchitecture — the service-provider interfaces the layers are wired through, the layer
map, the dated decisions, and the cross-repository issue tracker. It is its own
repository, checked out as a parallel working copy beside the others: `bite-gpui` (the
source branches), `bite-distribution`, `bite-tools`, `bite-gpui.github.io`, and the three
out-of-tree crates (`gpui_parley`, `gpui_morphorm`, `gpui_pass`).

The same pieces are described from two points of view, and only these files between them
list both: [`repositories.md`](repositories.md) is the canonical list of repository names
(and the legacy name each came from), [`references.md`](references.md) says what each
piece is for as a *repository*, and [`online-resources.md`](online-resources.md) is the
*online* metadata each carries. Check `repositories.md` before assuming where a sibling
lives, and cite repositories by canonical name — use a legacy name only when quoting an
older document.

## Non-negotiable conventions

These are in `README.md` §Conventions and are load-bearing; violating one is the
kind of error that is only caught much later.

- **Every claim about the code cites it, as `crate/file.rs:line`.** No assertion
  about behaviour without a `path:line` a reader can check in seconds.
- **One canonical ref.** All line numbers resolve against `bite_v1.22.0-pre`, not
  against whatever a checkout happens to be on. Changing that ref invalidates every
  citation and means a full re-derivation pass.
- **Do not restate a fact specified once elsewhere.** Naming, versions and the crate
  table live in `bite-distribution` (`targets.toml`, `docs/contract.md`); link to them. A
  second copy is a second copy to keep in sync.
- **A settled question is written down as settled**, with the rejected alternative. Do
  not re-open something already decided without reading the decision record first; the
  expensive failure is re-arguing a question whose answer and reason are already recorded.

## Verifying citations — `script/check-citations`

The one tool in this repository, and the thing an agent must run after any edit that
touches or adds a `path:line` citation. It **prints, does not judge**: it resolves every
citation against the canonical ref and shows the line each now points at. A blank line is
usually drift; only "unresolved", "out of range" and "missing" fail.

Two facts determine whether it works, and both are easy to get wrong:

1. **It resolves the source against the zed clone, not against this repository.**
   The cited files (`crates/gpui_authoring/…`, `crates/gpui_engine/…` and the rest) live
   in the `bite_*` branches of the `bite-gpui` repository. The script derives the git
   root from its own location, so it resolves the source only when it sits inside that
   clone; checked out as its own repository it finds the documents but cannot resolve
   the source, because the branches are not there. Run it from inside the `bite-gpui`
   checkout for a real pass.

2. **The canonical ref is a remote-tracking branch in a fresh clone.** The script now
   falls back from the bare name `bite_v1.22.0-pre` to `origin/bite_v1.22.0-pre`, so a
   fresh clone resolves without a local branch. Do not "fix" this by removing the
   fallback.

The current citation count and the ref's tip are `spi/rendering/milestones.md` §5, kept
there rather than here so it is not a second copy.

## What belongs where

- `architecture/` — the layer map, the frame's data flow, the boundaries. What an agent
  might add here is ruled by `architecture/README.md` §"Belongs here" and §"Does not
  belong here".
- `spi/` — the per-seam trait contracts and their `Application` bootstrap. An SPI is a
  *seam*: publishing one is a permanent commitment, so do not add a seam or a method to
  one without first reading `architecture/extension-tiers.md` (swap vs. wrap) and the
  `decisions/0001-no-third-swap.md` record.
- `decisions/` — numbered, dated records; unnumbered files are *evidence*, not decisions.
- `issues/` — work that spans repositories. One file per issue, `NNNN-short-title.md`.
- `spi/rendering-project/` — nothing but symlinks into the real documents. Do not write a
  real document here; opening a symlink edits the original.
