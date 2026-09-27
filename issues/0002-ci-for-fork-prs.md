- **Opened:** 2026-09-27
- **Status:** open
- **Touches:** `bite-gpui/distribution` (the CI), `bite-gpui/bite-gpui` (PR #1), `.tools` (transplant and rebase only)

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

1. **A workflow in `bite-gpui/bite-gpui`.** What PR #1 did, and it ran: both of its jobs
   went green. The fork is a Zed fork and already carries around fifty workflow files
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

## What would close it

Pick (1) or (2). (1) needs no change to `distribution` at all and can be done now; (2)
keeps the driver to a six-line stub but needs a pull-request entry point added to
`distribution` and the `./`-local-action question settled first. If the fork's gate is
meant to be *the same* gate as `verify.yml`, prefer (2); if it is a narrower check —
the cross-target check PR #1 added, say — (1) is honest and immediate.
