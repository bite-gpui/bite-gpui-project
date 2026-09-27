- **Opened:** 2026-09-27
- **Status:** open
- **Touches:** `.tools/`, `bite-gpui/bite-gpui` (PR #1), GitHub organization settings

# Running the fork's verification on `bite-gpui/bite-gpui` pull requests

## The question

The verification this project actually runs — the gates, replay, measurement and
carve operations — is driven from `.tools`. `bite-gpui/bite-gpui` is a fork of Zed, and
a pull request against it is not checked by any of that. Can the `.tools` configuration
run against `bite-gpui/bite-gpui` pull requests *without putting workflow files in
`bite-gpui/bite-gpui`*, by pointing at a workflow that lives elsewhere?

The proposal to investigate was: create an organization ruleset targeting the repo,
select *Require status checks to pass*, then *Add check → Require a workflow to run*,
pointing at `Repo-B/.github/workflows/ci.yml@main`, so that GitHub injects and runs the
other repository's workflow on this repo's pull requests.

## What the documentation shows

**The rule does not exist.** GitHub's current list of ruleset rules
(`docs.github.com/…/managing-rulesets/available-rules-for-rulesets`) has fifteen
entries: restrict creations, updates and deletions; require linear history; require
deployments; require signed commits; require a pull request; require status checks to
pass; block force pushes; the scanning, quality and coverage rules; and the file-path,
-length, -extension and -size restrictions. There is no *Require workflows to run*, and
nothing that runs a workflow. The nearest rule, *Require status checks to pass*, does
the opposite: it requires a check that something **else** has already reported. It
cannot start one.

**Organization rulesets are not available to this organization in any case.**
`docs.github.com/…/creating-rulesets-for-repositories-in-your-organization` states the
feature is "for customers on GitHub Team or GitHub Enterprise plans", and rulesets
generally are available "in public repositories with GitHub Free … and in public and
private repositories with GitHub Pro, Team, and Enterprise". `bite-gpui` holds eight
public repositories and one private (the organization API reports
`public_repos: 8, total_private_repos: 1`), so it is on Free. Repository-level rulesets
work on the public repos, but a repository ruleset can only *require* checks, not run
workflows from another repository.

## What the tree actually looks like

Two facts make the "no local workflow files" premise less useful than it sounds.

- **`bite-gpui/bite-gpui` is not free of workflow files.** Being a Zed fork, it carries
  upstream's set — around fifty files under `.github/workflows/` on
  `bite_v1.22.0-pre`, from `release.yml` to the community automation. They mostly
  *skip* on these branches, which is exactly why the fork's verification never runs.
  Adding one more workflow there is not invasive; it is one file among fifty.
- **`bite-gpui/tools` has no workflows at all.** `GET /repos/bite-gpui/tools/contents/.github/workflows`
  returns 404, and the local `.tools/.github` is empty. The tools repository is a
  private repo of scripts (`gates/`, `replay/`, `measure/`, `carve/`, `packaging/`,
  `targets/`, `docs/`), not of Actions workflows. So there is nothing in it for a caller
  to call yet, whatever the calling mechanism.

And one fact that settles the direction of the answer: **a workflow in
`bite-gpui/bite-gpui` does run.** PR #1 added
`.github/workflows/cross-target-windows.yml` to the branch and both of its jobs ran and
passed. GitHub Actions is enabled for the repository (`actions/permissions` reports
`enabled: true`, `allowed_actions: all`); what is missing is configuration, not
capability.

## What actually works

1. **A workflow in `bite-gpui/bite-gpui`.** PR #1's approach. No cross-repository
   access questions, no org settings; the cost is that the logic lives in the fork
   rather than in `.tools`. Mitigation: the workflow can do nothing but checkout and
   invoke `.tools`' scripts, so there is one place to maintain either way.
2. **A reusable workflow, with a thin caller.** Put the gate in `.tools` as a workflow
   with `on: workflow_call`, and add a six-line caller in
   `bite-gpui/bite-gpui/.github/workflows/` whose only job is
   `uses: bite-gpui/tools/.github/workflows/<verify>.yml@main`. This is the closest real
   thing to the proposal: all the configuration stays in `.tools`, and what lives in the
   fork is a stub that cannot drift.
   The constraint to check first: `tools` is **private** while `bite-gpui` is public,
   and a reusable workflow must be accessible to the caller. Same-organization access
   normally covers it, but cross-repository reuse from a private repository is exactly
   the case with an org-level control, so it should be verified from the fork with a
   throwaway caller before being relied on. It also requires adding the workflow to
   `.tools` first.
3. **An app or external runner that reports a status.** A process outside GitHub Actions
   runs the gates and posts a check with `statuses: write`; a repository ruleset then
   requires that check. This is the only route that puts literally nothing in
   `bite-gpui/bite-gpui`, and it costs a GitHub App (or a PAT in a secret), a runner,
   and a ruleset on the repository. Not worth it at this size.

## What would close it

Decide between (1) and (2). If (2), then: add the gate workflow to `.tools` with
`workflow_call`, add the stub caller to `bite-gpui/bite-gpui`, and confirm the private
cross-repository call is permitted before wiring the ruleset that makes it required.
(1) needs no decision — it is what PR #1 already does, and the honest cost is a
workflow file in the fork.
