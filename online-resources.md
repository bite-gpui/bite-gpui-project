# Online resources

Every place this project exists on the two services that matter — **crates.io** (the
packages), **github.com** (the repositories) — and the one place it publishes a page,
**github.io**. This is the record of *online metadata*: the name, description and
topics each resource carries, so all of it can be kept in step with the architecture
and checked without opening a browser.

Names, versions and the target table are not restated here: they are specified once in
`.dist` (`targets.toml`, `docs/contract.md` §5–§6) and the repositories are listed in
[`references.md`](references.md). What this file adds is the part that only exists on
the services — the description and topics on each repository, the description and
keywords on each crate, and which of those can still be changed.

Read it before editing anything public, because the two halves behave differently:

- **GitHub metadata is editable.** `gh repo edit` changes a repository's description,
  homepage and topics at any time.
- **crates.io metadata is immutable per published version.** There is no route to edit
  a published description, homepage, keyword or category — verified against
  crates.io's own router, where a crate exposes only *search*, *owners* and *version
  listing*, and a version only *list*. A value reaches crates.io by being in
  `Cargo.toml` when the version is uploaded, and changing it means publishing a new
  version. Everything below is therefore current state, not a plan; where a field is
  wrong for the next publish, it is marked in [Sync state](#sync-state).

Reviewed 2026-09-27, against `gh` 2.46.0 (account `Vanuan`) and the crates.io API.

## The organisation

| | |
| --- | --- |
| org | [`github.com/bite-gpui`](https://github.com/bite-gpui) |
| admin | `Vanuan` (John Yani) — the only admin, and the crates.io owner of every package below |
| org description | *bite-gpui rearchitects Zed's GPUI into a clean stack of layer crates and publishes them to crates.io so forks can install rather than vendor* |
| site | <https://bite-gpui.github.io> — the org's Pages root, served from `bite-gpui.github.io`, and the profile's Website field |
| repositories | 8 public, 1 private (`tools`) |

The upstream this is a fork of is not a bite-gpui resource: `zed-industries/zed` is
where GPUI comes from, and `gpui-ce/gpui-ce` is the community lineage the `bite_ce_main`
target transplants onto. Both are remotes on the main clone, not packages or
repositories this organisation owns — [`references.md`](references.md#upstream-and-the-ce-lineage).

## crates.io

**33 packages published.** Thirty are the Zed-lineage distribution's own set, published
from the `.dist` pipeline against the `bite_*` branches and so all sharing one
repository; three are the out-of-tree *swap* and *wrap* crates, each with a repository of
its own. No community-edition (`bite-gp-ce-*`) name has been published. Every package is
Apache-2.0 and owned by the user `Vanuan`.

A package's published name is not its library name — the distribution's invariant keeps
them apart (`.dist/docs/contract.md` §5). Both are given, because a consumer's manifest
line names the package while `use …` in the source still names the library.

### The Zed lineage — 30 packages

Repository `bite-gpui/bite-gpui`; the list is `.dist/pipeline/naming.py`'s map. All are
at **1.21.0** (the `bite_v1.21.0` release; `1.20.203` precedes them and
`0.0.0-reserved` is the name reservation on `bite-gpui`).

| package | library | description as published | keywords · categories | homepage |
| --- | --- | --- | --- | --- |
| `bite-gpui` | `gpui` | Zed's GPU-accelerated UI framework | desktop, gui, immediate · gui | <https://gpui.rs> |
| `bite-gp-authoring` | `gpui_authoring` | Zed's declarative UI authoring layer | desktop, gui, immediate · gui | <https://gpui.rs> |
| `bite-gp-runtime` | `gpui_runtime` | Zed's process harness for starting and driving a GPUI application | desktop, gui, immediate · gui | <https://gpui.rs> |
| `bite-gp-types` | `gpui_types` | `gpui_types` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-engine` | `gpui_engine` | `gpui_engine` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-engine-default` | `gpui_engine_default` | `gpui_engine_default` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-platform` | `gpui_platform` | `gpui_platform` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-shared-string` | `gpui_shared_string` | `gpui_shared_string` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-macros` | `gpui_macros` | Macros used by gpui | — | — |
| `bite-gp-gpui-util` | `gpui_util` | `gpui_util` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-util` | `util` | A collection of utility structs and functions used by Zed and GPUI | — | — |
| `bite-gp-util-macros` | `util_macros` | Utility macros for Zed | — | — |
| `bite-gp-collections` | `collections` | Standard collection types used by Zed and GPUI | — | — |
| `bite-gp-refineable` | `refineable` | A macro for creating 'refinement' types that can be used to partially initialize or mutate a complex struct | — | — |
| `bite-gp-derive-refineable` | `derive_refineable` | A derive macro for creating refinement types in Rust | — | — |
| `bite-gp-sum-tree` | `sum_tree` | A sum tree data structure, a concurrency-friendly B-tree | — | — |
| `bite-gp-http-client` | `http_client` | A HTTP client library for Zed and GPUI | — | — |
| `bite-gp-scheduler` | `scheduler` | `scheduler` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-path` | `path` | `path` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-zlog` | `zlog` | `zlog` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-ztracing` | `ztracing` | `ztracing` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-ztracing-macro` | `ztracing_macro` | `ztracing_macro` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-perf` | `perf` | A tool for measuring Zed test performance, with too many Clippy lints | — | — |
| `bite-gp-linux` | `gpui_linux` | `gpui_linux` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-macos` | `gpui_macos` | `gpui_macos` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-apple` | `gpui_apple` | `gpui_apple` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-windows` | `gpui_windows` | `gpui_windows` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-wgpu` | `gpui_wgpu` | `gpui_wgpu` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-web` | `gpui_web` | `gpui_web` — part of the bite-gpui rearchitecture of zed's gpui | — | — |
| `bite-gp-media` | `media` | Bindings to macos media handling APIs for Zed | — | — |

Two things the table shows at a glance, both by-products of staging rather than choices:

- **Twenty packages carry no description of their own.** Their upstream manifests have
  none, so `.dist/pipeline/stage.py:628` fills in
  `"<package> — part of the bite-gpui rearchitecture of zed's gpui"`. The other ten
  inherited a real one from Zed.
- **Twenty-six carry no keywords or category**, and **homepages point at
  <https://gpui.rs>** — upstream Zed's own site — on the three that have one at all,
  inherited from the same manifests. `gpui.rs` is not this project's domain. Staging
  has since been taught to write the project's own site, so this is the last published
  state rather than the next one — see [Sync state](#sync-state).

The upstream names are what a reader of the crates.io page sees; the fork's identity is
only in the package name and the repository link.

### The out-of-tree crates — 3 packages

Each is a second implementation of a seam (a *swap*) or a decorator on the frame
pipeline (a *wrap*), and each publishes from its own repository by tag — see
[`architecture/extension-tiers.md`](architecture/extension-tiers.md). Their metadata is
hand-written in the manifest, so it is complete where the lineage's is not.

| package | library | version | description | keywords · categories | repository |
| --- | --- | --- | --- | --- | --- |
| `bite-gp-parley` | `gpui_parley` | 1.21.1 | A Parley-backed TextSystem for GPUI | gui, layout, shaping, text · gui | `bite-gpui/gpui_parley` |
| `bite-gp-morphorm` | `gpui_morphorm` | 1.21.2 | A morphorm-backed LayoutEngine for GPUI | flexbox, gui, layout, morphorm · gui | `bite-gpui/gpui_morphorm` |
| `bite-gp-pass` | `gpui_pass` | 1.21.3 | A throttling, self-accounting FramePipeline for GPUI | frame-rate, gui, profiling, throttling · gui | `bite-gpui/gpui_pass` |

None of the three has a `homepage` on crates.io yet — it is a publish-time field, and
the source manifests have since been given the site, so it lands on each crate's next
publish. On GitHub they point at the site now (see [Sync state](#sync-state)).

## github.com

Nine repositories in `bite-gpui`: seven carry the project, one is the site, one is a
consumer. The state below is after the 2026-09-27 sync; how it was set and what cannot
be changed this way is in [Sync state](#sync-state).

| repository | visibility | description | topics | homepage |
| --- | --- | --- | --- | --- |
| `bite-gpui` | public | Zed's GPUI re-cut into a stack of installable layer crates, published to crates.io so forks can depend on them instead of vendoring | crates-io, gpui, gui, rust, ui-framework, zed | <https://bite-gpui.github.io/> |
| `bite-gpui-project` | public | The architecture and interface record for the bite-gpui rearchitecture: the layer map, the service provider interfaces, the dated decisions and the issue tracker | architecture, documentation, gpui, rust | <https://bite-gpui.github.io/> |
| `bite-gpui.github.io` | public | The bite-gpui site — the modular re-architecture of GPUI, told through a burger metaphor | astro, github-pages, gpui, gui, website | <https://bite-gpui.github.io/> |
| `gpui_parley` | public | A Parley-backed TextSystem for GPUI | gpui, gui, parley, rust, text-shaping | <https://bite-gpui.github.io/> |
| `gpui_morphorm` | public | A morphorm-backed LayoutEngine for GPUI | gpui, gui, layout, morphorm, rust | <https://bite-gpui.github.io/> |
| `gpui_pass` | public | A throttling, self-accounting FramePipeline for GPUI | frame-pipeline, gpui, gui, profiling, rust, throttling | <https://bite-gpui.github.io/> |
| `distribution` | public | Rewrites the bite-gpui layer stack into publishable crates and releases them to crates.io: naming, staging and versioning | crates-io, gpui, publishing, release-engineering, rust | <https://bite-gpui.github.io/> |
| `tools` | private | The tooling that applies the bite-gpui architecture to a target: replay onto a new upstream, conflict settlement, per-target carves and measurement | gpui, migration, rebase, rust, tooling | <https://bite-gpui.github.io/> |
| `bite_gpui_scroll_demo` | public | A scroll-and-motion demo built on the published bite-gpui crates, with the frame cost of each effect | benchmarking, demo, gpui, rust, scroll | <https://bite-gpui.github.io/demos> |

What each is, in the project's own framing:

| repository | what it holds | its own documentation |
| --- | --- | --- |
| `bite-gpui` | the `bite_*` branches: the architecture carried as a stack of forward commits against upstream Zed (default branch `bite_master`) | the branches themselves; the map is this directory |
| `bite-gpui-project` | **this repository** — the architecture and interface record | [`README.md`](README.md) |
| `bite-gpui.github.io` | the marketing and user-level documentation site | its `README.md`, `design/DESIGN-SYSTEM.md` |
| `gpui_parley` | `gpui_parley` — the `TextSystem` swap, and the crate's published home | its `README.md`, `benchmarks/` |
| `gpui_morphorm` | `gpui_morphorm` — the `LayoutEngine` swap | its `README.md`, `benchmarks/` |
| `gpui_pass` | `gpui_pass` — the `FramePipeline` wrap (throttle + ledger) | its `README.md`, `benchmarks/` |
| `distribution` | the distribution engine: staging branches into publishable crates and releasing them (`.dist/`) | `DESIGN.md`, `docs/` |
| `tools` | the migration tooling: replay onto a new upstream, conflict settlement, per-target carves and measurement (`.tools/`) | `docs/workflows.md`, `docs/architecture.md`, `docs/rebase-handoff.md` |
| `bite_gpui_scroll_demo` | a consumer: the scroll-and-motion demo built on the published crates | its `README.md`, `benchmarks/` |

`bite_gpui_scroll_demo` is the odd one out — it belongs to the usage suite (`.uses/`),
not to the project's own machinery, and it is the org's one repository that consumes
the published crates rather than producing something. Its clone under `.uses/scroll-demo`
was repointed at the published crates while the repository itself was not, so the two
have diverged (`.uses/README.md`, "Still open").

Two tag conventions are in use, and no repository publishes a GitHub Release — the tag
is the record. `bite-gpui` tags each branch release (`bite_<version>`, pre-releases
included); each out-of-tree crate tags the version it published (`v1.21.1`, `v1.21.2`,
`v1.21.3`) and its release workflow reads that tag as the version to upload;
`distribution` carries no tags and tags from a run when a target is released. The
version each tag names is `.dist`'s to specify, not this file's.

## github.io

One site, served from the org root because the repository is named for the Pages
domain: <https://bite-gpui.github.io>. A static Astro build with no client-side
framework and no runtime dependencies, published on every push to `main` through
GitHub Actions (`.website/README.md`, "Deployment").

| route | source | what it covers |
| --- | --- | --- |
| `/` | `src/pages/index.astro` | the marketing one-pager: the burger journey, capabilities, benchmarks, quickstart |
| `/parley` | `parley.astro` | swapping the text engine, and the measured trade that comes with it |
| `/demos` | `demos.astro` | the projects built on the published crates, filed by which boundary each tests |
| `/compare` | `compare.astro` | the comparisons that can be run, and what the project will not claim |

It is the only site; there is no per-crate or per-repository Pages build. `docs.rs`
renders API documentation for every crate automatically, and is not configured here.

## Sync state

### GitHub — editable, synced 2026-09-27

Set with `gh repo edit`: a description on the seven repositories that had none, a
homepage on the seven that lacked one, and topics on eight. Every repository's website
is now the project's own site, `https://bite-gpui.github.io/` — one URL, rather than the
per-crate deep links (`/parley`, `/demos`) it started as. Two rules kept it safe — a
description already matching its crate's (`gpui_parley`, `gpui_morphorm`) was left
alone, and topics were only added, never removed. The org profile's Website field was
normalized to `https://bite-gpui.github.io` in the same pass. The resulting values are
the [github.com](#githubcom) table above.

### crates.io — only on the next publish

A published version's description, homepage, keywords and categories cannot be edited —
`PATCH /api/v1/crates/{name}` accepts only `trustpub_only`, and
`PATCH /api/v1/crates/{name}/{version}` only the `yanked` state. The value is fixed at
upload, so the lineage's twenty missing descriptions, twenty-six missing keyword sets and
three `gpui.rs` homepages can only change when the next version is published. `.dist`
already tracks this as `docs/decisions.md` §13.5, "crate metadata and ownership — partly
done", which names `homepage`, `keywords` and `categories` as inherited-from-source
today.

**The website link (`homepage`) is the cheapest of those, because it needs no per-crate
judgement.** crates.io shows it as the *Homepage* link, beside *Repository*:

- **The 30 lineage packages** get it from one line in `.dist/pipeline/stage.py`, in the
  per-crate loop beside the `repository` write (`stage.py:635`):
  `set_key(lines, "package", "homepage", json.dumps("https://bite-gpui.github.io/"))`.
  `set_key` replaces an existing value and inserts one when absent, so it both overrides
  the inherited `https://gpui.rs` and supplies the 27 that have none — for every target
  and lineage at once. The staged tree is derived, so no `bite_*` branch changes.
  Supplying it from a top-level field in `targets.toml`, the way `repository` already is
  (`targets.py` fills each target's `url` from it), is the tidier variant.
- **The 3 out-of-tree crates** are edited in their own `[package]` tables (`.parley`,
  `.morphorm`, `.pass`), then a version bump and a tag each — their release workflow
  requires the tag to name the manifest's version.

`repository` needs nothing: staging already writes the GitHub URL on every lineage crate,
and the out-of-tree crates already name their own repositories. The GitHub repositories'
Website fields are a *separate* field, set to the same site in the [github.com](#githubcom)
table above.

The same stage.py loop is where `keywords`, `categories` and a richer description would
go if the lineage's metadata is to be closed out — the description is currently
fill-only-if-empty, so an override needs `set_key` unconditionally, not a fill. Those
remain open; the website link does not. **The source change is applied** (2026-09-27):
`targets.toml` carries a top-level `homepage`, `targets.py` materialises it per target,
`stage.py` writes it, and the three out-of-tree manifests carry it — all of which takes
effect only at the next publish. The `.dist` records were moved with it
(`docs/decisions.md` §13.5, `docs/staging.md` §7).

## Re-checking

```sh
# every repository's description, topics and homepage
gh repo list bite-gpui --limit 100 --json name --jq '.[].name' | while read -r r; do
  gh api "repos/bite-gpui/$r" --jq '{name,description,homepage,topics}'
done
```

```sh
# every published package's description, homepage, keywords and categories
curl -sS -H 'User-Agent: bite-gpui' 'https://crates.io/api/v1/crates?q=bite-gp&per_page=100' \
  | python3 -c 'import sys,json;print(" ".join(c["name"] for c in json.load(sys.stdin)["crates"]))' \
  | tr ' ' '\n' | while read -r n; do
      curl -sS -H 'User-Agent: bite-gpui' "https://crates.io/api/v1/crates/$n" \
        | python3 -c 'import sys,json;c=json.load(sys.stdin)["crate"];print(c["name"],c.get("description"),c.get("homepage"),c.get("keywords"),c.get("categories"))'
    done
```
