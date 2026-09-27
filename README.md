# `.meta` — the bite-gpui architecture record

The architecture and interface record for the bite-gpui rearchitecture: the service
provider interfaces the layers are wired through, how the layers fit together, where
the other repositories are, and the issue tracker.

It is its own repository, `git@github.com:bite-gpui/bite-gpui-project.git`. In the
zed clone it is excluded via `.git/info/exclude`, like `.dist/`, `.tools/`,
`.website/`, `.uses/`, `.parley/` and `.morphorm/`.

## Why it is separate

Four existing homes were considered, and none of them works:

| candidate | why not |
| --- | --- |
| a `bite_*` branch | there are twelve. A record that describes all of them would have to be committed to each and kept in sync through every merge — that duplication is the problem this repository solves |
| `distribution` (`.dist/`) | that repository is about *publishing*: target topologies, staging, naming, release ordering, registry limits. Its `DESIGN.md` is the architecture of the distribution *engine* and its `docs/decisions.md` (§13) is publishing decisions. Neither is the architecture of GPUI |
| `website` (`.website/`) and `uses` (`.uses/`) | those are read by consumers, not maintainers. Rationale, rejected alternatives and internal traps do not belong on a user-facing page |
| `tools` (`.tools/`) | focused on syncing source-of-truth changes — the migration scripts that rewrite a branch into a staged tree |

What is left is a record that has to be readable *across* all of the above without
being committed to any of them.

## Layout

| path | what belongs |
| --- | --- |
| [`architecture/`](architecture/README.md) | how the layers fit: the crate map, the boundaries between them, the frame's data flow |
| [`spi/`](spi/README.md) | service provider interfaces — the trait contracts a layer implements to be replaced, with the bootstrap each is installed through |
| [`decisions/`](decisions/README.md) | dated decision records: what was decided, on what evidence, and what was rejected |
| [`issues/`](issues/README.md) | the issue tracker, for work that spans repositories |
| [`references.md`](references.md) | the other repositories, branches and published names, and what each is for |
| [`online-resources.md`](online-resources.md) | the crates.io packages, GitHub repositories and Pages site, with the description and topics each carries |

## Conventions

- **Decisions are numbered and dated.** A record says what was decided, why, and what
  would reopen it. Unnumbered documents in `decisions/` are *evidence* for a numbered
  record, not decisions themselves.
- **Every claim about the code cites it** — `crate/file.rs:line`. Line numbers go stale,
  so they are resolved against **one ref**, not against whatever a checkout happens to be
  on: `bite_v1.22.0-pre`, which was `0ff0532eb7` when this convention was written.
  `script/check-citations` prints every citation with the line it currently points at.
  Changing the ref means re-deriving every citation in the repository. A bare filename
  (`app.rs:654`) resolves under `crates/gpui_authoring/src`; anything from another crate
  or a published dependency must be a full path, because `window.rs` and `lib.rs` are not
  unique across the tree.
- **Do not restate what is already specified once elsewhere.** Naming, versions and the
  crate table live in `.dist/`; link to them. A second copy of a fact is a second copy
  to keep in sync.
- **A settled question is written down as settled**, with the alternative that was
  rejected. The expensive failure is re-opening something that was already decided for
  a reason nobody recorded.
