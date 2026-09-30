# Repositories

The canonical list of every repository in the bite-gpui organisation, with the legacy
name each was once carried under. One place, so a name resolved once is resolved for
every document that cites it.

This file is the source of truth for **names** only. It does not restate what a
repository is *for* — that is [`references.md`](references.md) — nor the description,
topics and homepage each carries online — that is
[`online-resources.md`](online-resources.md). Name a repository by its canonical name;
cite the legacy name only when quoting an older document.

## The organisation's repositories

Nine repositories in `bite-gpui`: seven carry the project, one is the site, one is a
consumer. `bite-tools` is private; the rest are public.

| canonical | legacy | what it is |
| --- | --- | --- |
| `bite-gpui` | — | the `bite_*` branches: the architecture carried as a stack of forward commits against upstream Zed (default branch `bite_master`) |
| `bite-gpui-project` | `.meta` | **this repository** — the architecture and interface record: the layer map, the service-provider interfaces, the dated decisions, the issue tracker |
| `bite-distribution` | `.dist` | rewrites the layer stack into publishable crates and releases them to crates.io |
| `bite-tools` | `.tools` | the migration tooling: replay onto a new upstream, conflict settlement, per-target carves, measurement (private) |
| `bite-gpui.github.io` | `.website` | the marketing and user-level documentation site (Astro, served from the org's Pages root) |
| `gpui_parley` | `.parley` | `gpui_parley` — the `TextSystem` swap, a Parley-backed text system |
| `gpui_morphorm` | `.morphorm` | `gpui_morphorm` — the `LayoutEngine` swap, a morphorm-backed layout engine |
| `gpui_pass` | `.pass` | `gpui_pass` — the `FramePipeline` wrap (throttle + ledger) |
| `bite_gpui_scroll_demo` | `.uses/scroll-demo` | a consumer: the scroll-and-motion demo built on the published crates |

## The usage suite

The outside-in exercises were a **directory in the clone** (`.uses/`), not a repository.
They are becoming their own consumer repositories; `bite_gpui_scroll_demo` is the one
that has been promoted so far. The measurement policy that governs them lived in
`.uses/README.md` and has not yet found a single canonical home in the new structure.

| legacy | now |
| --- | --- |
| `.uses/` (a working directory) | the usage suite — being split into consumer repositories |
| `.uses/scroll-demo` | `bite_gpui_scroll_demo` (a repository) |
| `.uses/parley-demo`, and the other exercises | consumer repositories, each with a `benchmarks/` record |

## Not in the organisation

The upstream and lineage this is a fork of are remotes, not repositories or packages the
organisation owns:

| remote | repository |
| --- | --- |
| `upstream` / `origin` | `zed-industries/zed` — the GPUI this fork is cut from |
| `community` | `gpui-ce/gpui-ce` — the community edition, the `bite_ce_main` lineage |
| `vanuan` | `Vanuan/zed` — a personal fork remote (the org admin's) |

## The published crates

The repositories above publish to crates.io *as packages*, whose names differ from the
library names and from the repository names. That table is `bite-distribution`'s to own —
`bite-distribution/targets.toml` and `bite-distribution/docs/contract.md` §5–§6 specify it
once — and is listed with its online metadata in
[`online-resources.md`](online-resources.md) §crates.io, not restated here.
