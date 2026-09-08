# Issue Branch Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`issue-branch:the-forge-names-the-branch` — The forge names the branch](#issue-branchthe-forge-names-the-branch--the-forge-names-the-branch)
  - [`issue-branch:a-customized-template-is-read-not-assumed` — A customized template is read, not assumed](#issue-brancha-customized-template-is-read-not-assumed--a-customized-template-is-read-not-assumed)
  - [`issue-branch:a-confidential-issue-keeps-its-title-out-of-the-branch` — A confidential issue keeps its title out of the branch](#issue-brancha-confidential-issue-keeps-its-title-out-of-the-branch--a-confidential-issue-keeps-its-title-out-of-the-branch)
  - [`issue-branch:a-name-the-grammar-refuses-stops-before-any-write` — A name the grammar refuses stops before any write](#issue-brancha-name-the-grammar-refuses-stops-before-any-write--a-name-the-grammar-refuses-stops-before-any-write)
  - [`issue-branch:a-mint-is-idempotent` — A mint is idempotent](#issue-brancha-mint-is-idempotent--a-mint-is-idempotent)
  - [`issue-branch:the-preview-writes-nothing` — The preview writes nothing](#issue-branchthe-preview-writes-nothing--the-preview-writes-nothing)
  - [`issue-branch:a-reference-must-agree-with-the-clone` — A reference must agree with the clone](#issue-brancha-reference-must-agree-with-the-clone--a-reference-must-agree-with-the-clone)
  - [`issue-branch:the-seat-follows-the-recorded-mode` — The seat follows the recorded mode](#issue-branchthe-seat-follows-the-recorded-mode--the-seat-follows-the-recorded-mode)
  - [`issue-branch:a-forge-failure-leaves-the-clone-unchanged` — A forge failure leaves the clone unchanged](#issue-brancha-forge-failure-leaves-the-clone-unchanged--a-forge-failure-leaves-the-clone-unchanged)

<!--TOC-->

## Purpose

Rules governing one issue becoming one branch and one working copy: the `rk issue start` verb, the name each forge produces for an issue, and how that name reaches a seat in the operator's clone. Its subject is per-issue and it writes remote state on the operator's behalf, which distinguishes it from both neighbours: the local housekeeping a merge leaves behind is `SPEC-maintenance.md`, whose own purpose sends a remote write elsewhere, and the one-time configuration of a forge is `SPEC-forge-setup.md`, which acts per project rather than per issue. What a landing writes and records stays with `SPEC-landing.md`, and the doctor's probe surface stays with `SPEC-distribution.md`; this spec reads the recorded workflow mode and reports a probe rather than owning either. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The upstream documentation behind these rules is in `../reference/REFERENCE-issue-branch-sources.md`.

## Requirements

### `issue-branch:the-forge-names-the-branch` — The forge names the branch

When `rk issue start` resolves an issue to a branch, the name MUST come from the forge's own rules and MUST NOT be composed by this binary from its own conventions.

#### Scenario: A GitHub issue with no linked branch is minted

- GIVEN an issue the forge links no branch to
- WHEN `rk issue start <issue> --apply` runs against GitHub
- THEN the mint passes no branch name, because the mutation's optional name defaults to the issue number and title, and the name reported is the one read back from the issue's linked branches

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:a-customized-template-is-read-not-assumed` — A customized template is read, not assumed

Where the forge renders no name itself, the verb MUST read the project's own branch name template and reproduce the forge's rendering of it, rather than composing a name from the default shape.

#### Scenario: A GitLab project sets a branch name template

- GIVEN a project whose `issue_branch_template` is `%{id}-%{branch_creator}-%{title}`
- WHEN `rk issue start <issue> --apply` runs
- THEN the project setting is read, the creator is read only because the template names it, and the branch created carries the rendered name rather than the default one

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:a-confidential-issue-keeps-its-title-out-of-the-branch` — A confidential issue keeps its title out of the branch

Where an issue is confidential, the rendered name MUST be the forge's title-free form and the project's template MUST NOT apply.

#### Scenario: A confidential GitLab issue under a template

- GIVEN a confidential issue in a project whose template is `%{id}-%{title}`
- WHEN `rk issue start <issue>` runs
- THEN the name is `<iid>-confidential-issue`, the template does not apply, and the report states both, because a branch name is public where the issue is not

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:a-name-the-grammar-refuses-stops-before-any-write` — A name the grammar refuses stops before any write

If a rendered name falls outside the landed branch grammar, then the run MUST stop before the forge write and before any local change, and MUST name the setting that produced the name.

#### Scenario: A template renders a name the grammar refuses

- GIVEN a project whose template is `feature/%{id}-%{title}`, where `feature` is not a Conventional Commit type
- WHEN `rk issue start <issue> --apply` runs
- THEN the run refuses, names the rendered name and the template setting, creates no branch at the forge, and leaves the clone unchanged

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:a-mint-is-idempotent` — A mint is idempotent

While the forge already carries a branch for an issue, the verb MUST adopt it and MUST NOT create a second one.

#### Scenario: The verb runs twice on the same issue

- GIVEN an issue whose branch and worktree a previous apply already produced
- WHEN `rk issue start <issue> --apply` runs again
- THEN nothing is minted, the standing seat is reported, and the run succeeds, because an agent that lost track of its own state must be able to rerun safely

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:the-preview-writes-nothing` — The preview writes nothing

While `--apply` is absent, the verb MUST write nothing to the forge and nothing to the clone.

#### Scenario: A preview against an issue with no linked branch

- GIVEN an issue the forge links no branch to
- WHEN `rk issue start <issue>` runs without `--apply`
- THEN no mint is attempted, no branch is created locally, and the report says the name comes from the forge at the apply, because a preview that guessed a name would print one the apply might not produce

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:a-reference-must-agree-with-the-clone` — A reference must agree with the clone

If an issue reference names a project or a host that disagrees with the target's own remote, then the run MUST refuse before any forge call.

#### Scenario: An issue URL from another project

- GIVEN a clone whose origin is one project, and an issue URL naming another
- WHEN `rk issue start <url>` runs
- THEN the run refuses as a usage error naming both projects, because the branch would otherwise be minted on one project and seated in another

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:the-seat-follows-the-recorded-mode` — The seat follows the recorded mode

The verb MUST seat the branch the way the target's recorded workflow mode states, and MUST report which source decided the mode.

#### Scenario: A target recorded in branches mode

- GIVEN a landed target whose record states the branches mode
- WHEN `rk issue start <issue> --apply` runs
- THEN the branch is checked out in the main checkout rather than seated in a worktree, and the report names the landing record as the source

Verify: `cargo nextest run -E 'binary(cli)'`

### `issue-branch:a-forge-failure-leaves-the-clone-unchanged` — A forge failure leaves the clone unchanged

If any forge call fails, then the run MUST return before the first local mutation and MUST state that the target is unchanged.

#### Scenario: The forge answers an error

- GIVEN a forge CLI that fails on the issue read
- WHEN `rk issue start <issue> --apply` runs
- THEN the run fails, no branch and no worktree are created, and the diagnostic states the target as unchanged

Verify: `cargo nextest run -E 'binary(cli)'`
