- **Opened:** 2026-09-27
- **Status:** open
- **Touches:** `bite-gpui/bite-gpui`, `bite-gpui/distribution`, `.meta/issues/0002-ci-for-fork-prs.md`

# A scheduled build, so the pull-request gate can stay small

## The problem

The pull-request gate is deliberately minimal. `.github/workflows/bite-ci.yml` is three
jobs: the target table and the naming rule, called from `bite-gpui/distribution`, and a
`cargo check -p gpui` on `macos-14` and on `windows-latest` — the two platforms where a
backend's build script can compile its shaders, and the two the release pipeline's
`verify.yml` cannot reach, since its legs are Linux and macOS.

Everything expensive is left out of it, and each omission is a real gap:

- **`--all-features`.** The release gate's clippy runs `--all-targets --all-features`
  over the closure, because that is the only configuration that compiles the code behind
  `bench-support` and `profiler` — checking default features alone reports that code as
  dead. The pull-request job checks default features.
- **wasm32.** `cargo check --target wasm32-unknown-unknown -p gpui_web` was in the
  render-extension spike's CI sketch and is in nothing now.
- **Clippy.** The release gate's compile *is* clippy; the pull-request job runs `cargo
  check`, so lints that would fail a release pass a pull request.
- **Tests.** Nothing in the pull-request gate runs one.
- **The packaging checks.** `preflight` runs `checks/check_reads.py` and
  `checks/apple_build_check.py`, which caught the `gpui_apple` vendored-shader defect.
  A pull request runs neither.

## What exists to build on

- Standard GitHub-hosted runners are **free** for a public repository, so the cost of a
  wider run is wall-clock and queue time, not money. On a cold cache each native job is
  about six minutes; an all-features closure build is much larger.
- `distribution`'s `ci.yml` already stages and verifies one target per lineage, and
  `verify.yml` is the one definition of what verifying a target means — including the
  receipt `release.yml` reads. Verifying a target is exactly the "wider than a pull
  request" run being asked for here.
- `distribution`'s `targets.toml` already enumerates every branch that is published, so
  a schedule there does not need a list kept by hand.
- **`schedule` only runs on the default branch**, and only when the file is on it
  (`docs.github.com/…/events-that-trigger-workflows#schedule`). The fork's default
  branch is `bite_master`; `distribution`'s is `main`. A scheduled workflow therefore
  cannot be carried per release branch — it has to be on the default branch and check
  the other branches out itself. That is fine: they are branches of one repository.

## The questions to settle

1. **Where it lives.** A `schedule:` workflow in the fork, carried on `bite_master` and
   materialised by replay, which checks each `bite_*` branch out in a matrix; or a
   `schedule:` in `distribution`, which already knows the branches and can call
   `verify.yml` per target rather than restating what verifying means. The second reuses
   the release gate, and has one wrinkle: staging resolves a version from the branch's
   tag and refuses an untagged tip, so the *released* lines can be verified while
   `bite_master`, which publishes on CalVer, and any branch mid-amendment cannot.
2. **Cadence.** Nightly catches drift on `bite_master` — which follows upstream's `main`
   — sooner and costs queue time every day; weekly keeps it quiet.
3. **Scope.** At least the four omissions above, and the Path A probe if it becomes a
   regression test rather than the question it was (`decisions/windows-path-a-probe.md`).
4. **Report or gate.** A scheduled run that reports and a check that blocks are
   different things, and making the scheduled result required is a separate decision
   from running it.

## What would close it

A `schedule:` workflow exists, runs green against branches that are not its own, and its
result is somewhere a maintainer looks — and `.github/workflows/bite-ci.yml` is still the
three jobs it is, so a pull request stays cheap.
