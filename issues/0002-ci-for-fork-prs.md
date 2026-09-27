- **Opened:** 2026-09-27
- **Status:** open — the checks run cross-repository and pass, and the caller is on `bite_v1.22.0-pre`; the file per base branch is the remaining work, with the ruleset gate and the `@pr-checks` ref after it
- **Touches:** `bite-gpui/distribution` (the CI), `bite-gpui/bite-gpui` (the caller, landed on `bite_v1.22.0-pre` via #2), `.tools` (transplant and rebase only)

# Running the CI on `bite-gpui/bite-gpui` pull requests

## The question

The CI is not in the fork. Can it run against a pull request *to* the fork without
putting workflow files in the fork, by pointing at a workflow that lives in the
repository the CI is actually in?

The proposal to investigate was: create an organization ruleset targeting the
repository, select *Require status checks to pass*, then *Add check → Require a workflow
to run*, pointing at `Repo-B/.github/workflows/ci.yml@main`, so GitHub injects and runs
the other repository's workflow on this repository's pull requests.

## That mechanism does not exist

GitHub's current list of ruleset rules
(`docs.github.com/…/managing-rulesets/available-rules-for-rulesets`) has fifteen
entries: restrict creations, updates and deletions; require linear history; require
deployments; require signed commits; require a pull request; require status checks to
pass; block force pushes; the scanning, quality and coverage rules; and the
file-path, -length, -extension and -size restrictions. There is no *Require workflows to
run*, and nothing that runs a workflow. The nearest rule, *Require status checks to
pass*, requires a check that something **else** has already reported — it cannot start
one.

Organization rulesets are unavailable here in any case: the feature is "for customers on
GitHub Team or GitHub Enterprise plans"
(`docs.github.com/…/creating-rulesets-for-repositories-in-your-organization`), and the
organization holds `public_repos: 8, total_private_repos: 1`.

So the answer to the question as asked is no — there is no ruleset, no trigger and no
reusable-workflow wiring that runs another repository's workflow with nothing at all in
this one, short of an app reporting a status (§ *An app*, below).

## Where the CI actually is

**`bite-gpui/distribution`**, and it is **public**. Four workflows:

| file | triggers | what it does |
| --- | --- | --- |
| `ci.yml` | `pull_request`, `push: main`, dispatch | validates the target table and the naming rule, lints the CI wiring, plans a matrix, and stages one target per lineage |
| `verify.yml` | dispatch, **`workflow_call`** | verifies one target's staged tree; the `verify` action is the one definition of what verifying a target means |
| `release.yml` | dispatch | stages, packages and publishes a target, refusing to publish without a verification receipt |
| `tag.yml` | dispatch | resolves a branch to its version, pushes the tag to the source repository, dispatches the release |

`.tools` is the transplant and rebase tooling — `replay`, `carve`, `gates`, `measure`,
`packaging` — with no workflows of its own, as corrected.

## What that CI covers today, and what it does not

It covers **changes to the distribution configuration**: a pull request *in
`distribution`* stages and verifies targets to prove the table, the naming rule and the
staging scripts still work against the source branches. It reads the source repository
(`bite-gpui/bite-gpui`) as an *input*, resolved from the target table — it never reacts
to a change there.

So a change to the fork is verified only after the fact, by the next distribution run,
and only for the targets that run stages. Nothing runs on the fork's own pull requests.

## What actually works

1. **A workflow in `bite-gpui/bite-gpui`.** What the fork now does, and it ran: all three of
   its jobs are green (the file is below). The fork is a Zed fork and already carries around fifty workflow files
   under `.github/workflows/` that skip on these branches, so one more is not invasive.
   Since `distribution` is **public**, that workflow can check the latter out
   (`repository: bite-gpui/distribution`, `path: .dist`) and run its `pipeline/` and
   `checks/` scripts against the fork — the logic stays in `distribution`, and the fork
   holds a thin driver. No token, no secrets, no ruleset.
2. **A thin caller into `distribution`'s reusable workflow.** Closer to the proposal:
   both repositories are public, so the private-access question that would otherwise
   block it does not arise, and `verify.yml` already declares `workflow_call`. Two
   things make it not fit as it stands, and they are the work items rather than
   obstacles:
   - **It is target-oriented.** Its input is a name from the target table, which
     `pipeline/targets.py --env` resolves to a branch, repository and version. It
     validates that table first, so an arbitrary pull-request ref — a branch no table
     has heard of — cannot be verified through it. A pull-request entry point needs a
     repo-and-ref input instead of a target name.
   - **It uses a repository-local composite action.** Its first steps fetch
     `./.github/actions/build-env`. Whether a `./` reference inside a workflow called
     from another repository resolves against the called repository or the caller is
     exactly the kind of detail that decides whether cross-repository reuse works at
     all, and it should be confirmed with a throwaway caller before anything depends on
     it.
3. **An app, or an external runner, reporting a status.** A process outside GitHub
   Actions runs the gates and posts a check with `statuses: write`; a repository ruleset
   on `bite-gpui/bite-gpui` — public, so available on Free — then requires that check.
   This is the only route with literally nothing in the fork, and it costs a GitHub App,
   a runner and a ruleset. Not worth it at this size.

## What route 2 measured

Route 2 is now wired. `bite-gpui/distribution` branch `pr-checks` carries a
`workflow_call` entry point, `.github/workflows/pr-checks.yml`, and
`bite-gpui/bite-gpui` carries the caller, `.github/workflows/bite-ci.yml`, landed on
`bite_v1.22.0-pre` via #2.
Running it answered the three questions the design left open:

1. **A cross-repository call is allowed.** A public repository may call another public
   repository's `workflow_call` workflow. No token, no secret, no app. This was the main
   risk and it is cleared.
2. **A checkout with no inputs gets the *caller's* repository.** Inside a called workflow
   `github.repository` is the caller's, so the caller's tree lands at the workspace root
   and the called repository's files are simply absent. A checkout of
   `bite-gpui/distribution` at `path: .dist` is what makes the called repository's
   scripts available here.
3. **A `./`-relative `uses:` resolves against the workspace root's tree, which in a
   called workflow is the caller's.** The first run's `uses: ./.github/actions/build-env`
   failed with *"Can't find 'action.yml' … under
   /home/runner/work/bite-gpui/bite-gpui/.github/actions/build-env"*. Putting the called
   repository at the workspace root is what a step has to do to reach its own actions;
   `ci.yml` gets that for free because it *is* the workspace root, and a called workflow
   has to do it explicitly. Not measured: whether that is enough to restore `./`
   resolution from a called workflow — the cheapest next probe if the composite actions
   are wanted.

The first version of the entry point also failed its naming step for a second reason:
`pipeline/naming.py --verify` walks the *whole* target table, so both lineages need a
checkout, and the community-edition lineage fell back to its local path and exited on
`bite_ce_main: no checkout at .tools/worktrees/wt-ce`. It now checks out the branch
`targets.py --refs --select ce` names, exactly as `ci.yml` does. The check is green:
`12 targets, 55 distinct (lineage, package) pairs, 55 published names, 0 failures`,
reported to `bite-gpui/bite-gpui#1` as *Distribution checks / target table and naming
rule*.

What the entry point covers is the part of the pipeline that reads the tree rather than
the release tag: the target table and the naming rule. Compilation is the fork's own,
and its first form — a cross-target `cargo check --target x86_64-pc-windows-msvc` — is
how the `gpui_windows` import defect was found. It has since been replaced by checks on
the real platforms (§ *The file, as built*).

## The fork still has to change

Route 2 does not do what it was proposed to do. The goal was to run the checks on the
fork's pull requests with nothing in the fork; route 2 puts a caller there, so the fork
is modified either way. That is not a defect of the wiring:

- `pull_request` sets `GITHUB_REF` to the pull request's merge branch
  (`refs/pull/N/merge`) and `GITHUB_SHA` to its last merge commit, and a run resolves the
  workflow files in that merge commit — base and head together. A file the pull request
  itself adds therefore runs for that pull request and no other.
- `workflow_call` inherits the caller's event payload, so a called workflow has to be
  called from a repository that carries a file of its own.

Nothing in the trigger set avoids that. The events that run without a workflow file on
an ordinary branch — `schedule`, `workflow_run`, `repository_dispatch`, `workflow_dispatch`
— all require the file on the **default branch**, which is worse for a project whose
branches are the releases. So the only arrangement with genuinely nothing in the fork
remains an app or an external runner posting a check (§ *An app*, above).

Measured when the caller was first written: `bite_v1.22.0-pre` carried 48 workflow files
and not `bite-ci.yml`, so the checks that passed then were green for one branch and nothing
else. It carries `bite-ci.yml` now; the other branches do not.

That leaves the shape of the file, and it is worth choosing for the twelve branches
rather than for one. The branches are regenerated by replay, so any file added to them
is re-materialised per branch and per replay; the less it says, the less there is to
keep in step. A caller that names one reusable workflow and passes its own repository
and ref is that file — changing *what* is checked then touches `distribution` alone. A
driver that names the steps to run would put this decision in twelve places.

### The file, as built

One file, `.github/workflows/bite-ci.yml`, with three jobs:

| job | runner | what it checks |
| --- | --- | --- |
| `checks / target table and naming rule` | ubuntu | the target table and the naming rule, called from `distribution` |
| `cargo check -p gpui (macos-14)` | macos-14 | the Metal backend, and the only place the Metal shaders are compiled |
| `cargo check --release -p gpui (windows-latest)` | windows-latest | the Direct3D backend, and the only place the HLSL is |

All three pass — 21 s, 5m42s, 6m23s. Standard hosted runners are free for a
public repository, so the real platforms cost wall-clock and nothing else.

Each platform is checked where its build script can do its job. `gpui_apple`'s runs
`xcrun metal`, so the macOS job compiles the shaders. `gpui_windows`'s compiles the HLSL
with `fxc` only when debug assertions are off, and the Rust includes the result only in
that profile — which is why the Windows job is a release `check`, and why it cannot be
satisfied on any other platform: its build script's branch is gated on the host too.

An earlier version did all of this from one Linux runner. It worked for Windows, whose
Rust includes the generated HLSL only in a release build, but not for Apple:
`crates/gpui_apple/src/metal_renderer.rs:36` includes `OUT_DIR/shaders.metallib`
unconditionally, and the lib is gated on the target while `build.rs` is compiled for the
host, so nothing produced it. The build script keeps a stub for that case — it is what
makes `cargo check --target aarch64-apple-darwin -p gpui_macos` work from a Linux
checkout, which is worth having locally — but CI no longer depends on it.

## What is left

- **One file per base branch.** The caller has to be committed to every `bite_*` branch
  that should gate pull requests, and to each new one as replay makes it. It is on
  `bite_v1.22.0-pre`; the rest follow by replay. That is where the fork is modified, and it
  is the whole of the modification.
- **The ref.** The caller names `…/pr-checks.yml@pr-checks`, so the fork depends on a
  branch of `distribution` that can be deleted. Merge the entry point to `main` and
  switch the caller to `@main`.
- **The gate.** A check that runs is a report, not a gate. A repository ruleset on
  `bite-gpui/bite-gpui` — public, so available on Free — would make it one, and the
  token has `admin`, so it can be created. It has to come *after* the file is on the
  branch it guards: a required check that nothing reports blocks every pull request, and
  the file is on `bite_v1.22.0-pre` and no other branch yet. All three jobs gate —
  `checks / target table and naming rule`, `cargo check -p gpui (macos-14)` and
  `cargo check --release -p gpui (windows-latest)`.
  Organization rulesets need Team or Enterprise and are out.
- **The wider checks.** The pull-request gate is deliberately small: three jobs, the
  cheapest that catch what nothing else reaches. What it leaves out — `--all-features`,
  wasm32, clippy, the tests, the packaging checks — is [issue 0003](0003-scheduled-builds.md),
  which asks for the scheduled build they belong on rather than adding them here.
- **Widening it.** `verify.yml`'s `preflight` and `code-checks` are not in the entry
  point, for two separate reasons. `preflight` reads the stage report, which only
  `stage.py` writes and a pull request cannot run. And the composite actions cannot be
  called across the boundary (above). Whether a nested `./` reference *inside* a
  composite action resolves against the action's own repository — which would let
  `verify` be called fully qualified — is the next thing to measure if a wider gate is
  wanted.
