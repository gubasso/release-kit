# Trigger a target-owned child pipeline

## Context and Problem Statement

A GitLab target had nowhere to declare a job of its own. `.gitlab-ci.yml` is a rendered file declaring the release stages alone, so a job added to it is owned drift an upgrade refuses. The merge gate here is the whole pipeline, so a target's job would gate the merge if it had a place to live. Include ordering gives it none: GitLab deep-merges included configuration and the main file wins only the keys it declares, so a target's file contributes `allow_failure`, `needs`, `retry`, `tags`, and global subkeys to jobs release-kit owns.

## Considered Options

- Trigger a child pipeline from `.gitlab/ci/project.yml` with `strategy: mirror` — chosen.
- Include the target's file at a lower precedence — rejected: the deep merge reaches every job.
- Trigger with `strategy: depend` — rejected: the reference no longer recommends it, and its status reads running while the child waits on a manual job.
- Declare a spare stage the target fills — rejected: one configuration is still one merge surface.
- Leave the pair serving no job — rejected: that is the state this decision ends.

## Decision Outcome

Both rendered parents carry one `project-jobs` bridge in the `test` stage, guarded by `exists:` so no empty downstream pipeline appears, triggering `.gitlab/ci/project.yml` with `strategy: mirror` so the child's failure reaches the merge-request pipeline. The target creates and owns that file and may nest its own stages and includes inside it. `strategy: mirror` arrived in GitLab 18.2, so 18.2 is this convention's minimum version; that floor is part of this decision, and `forge-version` refuses below it. This changes the matrix `ADR-land-the-nix-capability-as-an-opt-in.md` records for `(rust, gitlab)`: the pair can now serve a job.

Enforced by `landing:the-flake-pair-lands-all-or-nothing` and `forge-setup:a-check-reports-what-the-forge-enforces`.

## Consequences

- Good: a target's jobs gate the merge and reach nothing release-kit owns.
- Bad: one bridge job and one downstream pipeline per merge request; an instance below the floor cannot run the convention.

## Status

Implemented: `project-jobs` in both rendered GitLab parents, the extension point in `forges/gitlab.md`, and `forge-version` in `src/setup/steps.rs`.
