# Git Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`git:a-target-is-a-non-bare-repository` — A target is a non-bare repository](#gita-target-is-a-non-bare-repository--a-target-is-a-non-bare-repository)
  - [`git:the-method-is-trunk-based` — The method is trunk-based](#gitthe-method-is-trunk-based--the-method-is-trunk-based)
  - [`git:one-trunk-receives-pull-and-merge-requests` — One trunk receives pull and merge requests](#gitone-trunk-receives-pull-and-merge-requests--one-trunk-receives-pull-and-merge-requests)
  - [`git:the-trunk-name-is-a-parameter` — The trunk name is a parameter](#gitthe-trunk-name-is-a-parameter--the-trunk-name-is-a-parameter)
  - [`git:checkout-mode-selects-where-a-topic-branch-opens` — Checkout mode selects where a topic branch opens](#gitcheckout-mode-selects-where-a-topic-branch-opens--checkout-mode-selects-where-a-topic-branch-opens)
  - [`git:integration-mode-selects-the-authority-that-squashes` — Integration mode selects the authority that squashes](#gitintegration-mode-selects-the-authority-that-squashes--integration-mode-selects-the-authority-that-squashes)
  - [`git:the-release-request-integrates-at-the-forge` — The release request integrates at the forge](#gitthe-release-request-integrates-at-the-forge--the-release-request-integrates-at-the-forge)
  - [`git:the-manual-stage-is-the-pre-integrate-contract` — The manual stage is the pre-integrate contract](#gitthe-manual-stage-is-the-pre-integrate-contract--the-manual-stage-is-the-pre-integrate-contract)
  - [`git:a-check-declares-where-it-runs` — A check declares where it runs](#gita-check-declares-where-it-runs--a-check-declares-where-it-runs)
  - [`git:a-local-integration-is-a-transaction` — A local integration is a transaction](#gita-local-integration-is-a-transaction--a-local-integration-is-a-transaction)
  - [`git:git-and-forge-terms-stay-explicit` — Git and forge terms stay explicit](#gitgit-and-forge-terms-stay-explicit--git-and-forge-terms-stay-explicit)
  - [`git:concurrent-pull-and-merge-requests-carry-the-tested-trunk` — Concurrent pull and merge requests carry the tested trunk](#gitconcurrent-pull-and-merge-requests-carry-the-tested-trunk--concurrent-pull-and-merge-requests-carry-the-tested-trunk)

<!--TOC-->

## Purpose

Rules governing the Git shape every target must have and the Git workflow parameters a target records: the non-bare repository, trunk-based development as the one development method, the trunk's name, the checkout mode a topic branch opens under, the integration mode that says which authority carries it onto the trunk, and the words shared prose uses for the two forges' review units. The protection domain in [the target configuration specification](./SPEC-target-config.md) and [the forge setup specification](./SPEC-forge-setup.md) owns how each forge enforces these rules. The project profile in [the project profile specification](./SPEC-project-profile.md) owns what the project is. This spec owns how changes reach its trunk.

## Requirements

### `git:a-target-is-a-non-bare-repository` — A target is a non-bare repository

Every target MUST be a non-bare Git repository with exactly one main working tree, and a verb that seats work MUST refuse a bare repository by name, because the sibling derivation composes with a main checkout and the convention rests on one main worktree that commits nothing.

#### Scenario: A bare repository asks for a worktree

- GIVEN a bare repository with peer checkouts under it
- WHEN `rk worktree add <branch> --apply` runs
- THEN it refuses naming the bare main record, and nothing is created

Verify: `cargo nextest run -E 'test(worktree_add_refuses_a_bare_repository)'`

### `git:the-method-is-trunk-based` — The method is trunk-based

Trunk-based development MUST be the one development method every target runs, and the committed configuration MUST carry no key that selects it, because a key with one legal value restates an invariant and implies alternatives that do not exist.

#### Scenario: A configuration key for the development method is proposed

- GIVEN a proposal to add `development_method = "trunk-based"` to the target configuration
- WHEN the reviewer reads the key's legal values
- THEN the key is refused, because its one value states what `rk method invariants` already binds

Verify: `rg -n 'development_method' src blocks && exit 1 || exit 0`

### `git:one-trunk-receives-pull-and-merge-requests` — One trunk receives pull and merge requests

One configured trunk MUST receive every change through a short-lived topic branch, as one squash commit that passed every gate the project declared, so one implementation is one commit and the history stays linear. Where the recorded integration mode is `forge`, that squash MUST be a guarded pull request or merge request and the trunk MUST take no direct push.

#### Scenario: A change arrives outside a request

- GIVEN a landed forge-integration target whose trunk protection the setup owns
- WHEN a direct push to the trunk is attempted
- THEN the forge refuses it, and the landed hooks refuse the commit at the desk first

Verify: `cargo nextest run -E 'test(the_applied_trunk_ruleset_requires_a_fresh_branch) or test(a_gitlab_protect_trunk_apply_asserts_the_squash_template)'`

### `git:the-trunk-name-is-a-parameter` — The trunk name is a parameter

The trunk's name MUST be a recorded parameter, `git.trunk` in the committed configuration, and MUST default to `master` only at the final precedence tier, after a flag, the configuration, a compatible record, and observation have answered nothing.

#### Scenario: A target names main as its trunk

- GIVEN a configuration with `git.trunk = "main"`
- WHEN a landing renders the release workflow and the hook block
- THEN every branch guard and release trigger names `main`, and the record carries it

Verify: `cargo nextest run -E 'test(the_trunk_branch_comes_from_the_config) or test(the_line_prefix_comes_from_the_config)'`

### `git:checkout-mode-selects-where-a-topic-branch-opens` — Checkout mode selects where a topic branch opens

The checkout mode, `git.checkout_mode`, MUST be one of `linked-worktree` and `main-worktree`, MUST decide only which working tree a topic branch opens in, and MUST change no branch semantics: neither the branching method, nor the rebase policy, nor the merge policy.

#### Scenario: The mode value is renamed in the configuration vocabulary

- GIVEN a landed target recorded under the worktree form of the checkout mode
- WHEN a newer binary reads the record and projects the blocks
- THEN the rendered routing block, the hook block, and the hook id are byte-identical to the landed ones

Verify: `cargo nextest run -E 'test(the_checkout_mode_rename_reaches_no_landed_byte)'`

### `git:integration-mode-selects-the-authority-that-squashes` — Integration mode selects the authority that squashes

The integration mode, `git.integration`, MUST be one of `local` and `forge`, MUST be a recorded landing parameter resolved from a flag, the committed configuration, a compatible record, and a compiled default of `local` in that order, and MUST change only through a landing verb. It MUST decide only which authority performs the squash onto the trunk and MUST change no branch semantics and no checkout mode. A per-execution override MUST select the other authority for one integration without writing the record. A record predating this parameter MUST resolve to `forge`, because that is the authority such a target landed.

#### Scenario: A target landed before the axis existed

- GIVEN a record at a schema below the one that carries `git.integration`
- WHEN a landing verb resolves the parameter with no flag and no configured value
- THEN it resolves `forge`, and a fresh landing with the same silence resolves `local`

Verify: `cargo nextest run -E 'test(integration) or test(params_from_a_record)'`

### `git:the-release-request-integrates-at-the-forge` — The release request integrates at the forge

The release request MUST integrate at the forge in both integration modes, because merging it is the one event that authorizes the tag, the publish, the provenance, and the artifacts, and no verb MAY offer a local path to a release. The tag protections MUST be identical under both modes. On GitHub the trunk's strict required-check rule MUST hold the release request in both modes; local integration MUST admit its deliberate direct push through the repository-administrator bypass while leaving the release App governed by that rule. The local-mode workflow MUST additionally retry a ready request after either its named workflow or the successful push-side request job completes. It MUST authenticate the candidate from the forge's current, fully paginated answer rather than the event that woke it — open, not a draft, based on the trunk, headed from the same repository, and authored by the release identity — MUST judge the named check for the exact head commit it merges, and MUST leave the request open with a successful no-op on every other answer. A forge whose own mechanism requires the whole pipeline names no check and needs no such gate.

#### Scenario: A local-integration target reaches a release

- GIVEN a landed target whose recorded integration mode is `local`
- WHEN the release-bearing protections are resolved for it
- THEN the tag protections match a forge-integration target's, and the release still merges at the forge

#### Scenario: A release request is gated under either authority

- GIVEN two GitHub targets differing only in their recorded integration mode
- WHEN each renders its release automation
- THEN both trunks arm the request behind the strict required check, the local-integration target also renders a retry job that judges the same recorded check name, and neither merges a request whose check has not concluded successfully

Verify: `cargo nextest run -E 'test(the_tag_floors_hold_under_both_integration_modes) or test(the_release_request_is_gated_under_both_integration_modes)'`

### `git:the-manual-stage-is-the-pre-integrate-contract` — The manual stage is the pre-integrate contract

Release-kit MUST express the pre-integrate gate as one `pre-commit` run over the `manual` stage across all files, and MUST read no hook identifier, revision, language, or test category from the target's own hook configuration, because `pre-commit` admits no custom stage and the project owns everything outside the marked block. Release-kit MUST land no continuous-integration workflow for the project's own checks and MUST judge no parity between those checks and the hook stages.

#### Scenario: A project assigns its own checks to stages

- GIVEN a target whose hook configuration puts an expensive suite on the `manual` stage
- WHEN `rk integrate` runs its gate
- THEN it invokes the stage as one command and names no hook the project declared

Verify: `cargo nextest run -E 'test(the_gate_invokes_the_manual_stage)'`

### `git:a-check-declares-where-it-runs` — A check declares where it runs

A project MUST assign every check both a scope and a boundary moment, and the landed guidance MUST teach that assignment. Scope MUST be one of `both`, `local-only`, or `ci-only`. The moment MUST be a native `pre-commit` stage where the check has a local path, and a named remote boundary where the scope is `ci-only`, because a check that is not a hook has no honest hook stage. A check classified `both` MUST have a local execution path and a remote execution path of equivalent coverage rather than identical commands, and a check that is neither a hook nor a recorded `ci-only` check is unclassified, which is a defect in the project's own policy. This requires nothing of release-kit's readers: a scope declared forward by the author at the check duplicates nothing and infers nothing, where a parity claim derived backward from a workflow file infers intent that no file states, which is why `git:the-manual-stage-is-the-pre-integrate-contract` still refuses the second and this rule does not reintroduce it.

#### Scenario: A project puts an expensive suite on the manual stage alone

- GIVEN a project whose continuous integration invokes the default `pre-commit` stage while its expensive suite is declared on `manual` alone
- WHEN the landed guidance is read
- THEN it teaches that the two sides of the boundary must invoke the same stages, and no binary judges the project's workflow file to reach that conclusion

Verify: `cargo nextest run -E 'test(a_landed_sweep_note_invites_the_projects_own_skips) or test(a_landed_integrate_note_names_both_axes)'`

### `git:a-local-integration-is-a-transaction` — A local integration is a transaction

A local integration MUST leave the trunk where it stood unless every check passed, and MUST reach that by publishing late rather than by undoing: it MUST gate before it builds, MUST build the squash commit as an object no ref names, and MUST publish it with one compare-and-swap carrying the trunk tip observed before the gate ran, so a trunk that moved under the gate refuses with nothing written and no tip to restore. It MUST observe the branch before the gate, use that one object for both the tree it integrates and the tip its evidence certifies, and re-observe it after the gate, refusing on any movement, so the tree the gate judged is the tree that reaches the trunk. It MUST read and judge its evidence ledger before it builds anything, and MUST stage that evidence before the publication, so every fallible part of writing it happens with the trunk unmoved. It MUST apply the whole landed `commit-msg` judgment to the trunk message, because the command that writes the commit fires no hook. It MUST take a lock so a second integration in the same clone refuses rather than waits, and it MUST never push and never force-push.

#### Scenario: The trunk moves while the gate runs

- GIVEN a local integration whose trunk advanced between the tip it observed and its publication
- WHEN the compare-and-swap runs
- THEN it refuses naming both tips, no ref moved, and the staged evidence is inert because the trunk does not reach the commit it names

#### Scenario: The branch advances while the gate runs

- GIVEN a local integration whose branch takes another commit after the gate passed
- WHEN the branch is re-observed
- THEN it refuses naming both tips, because the gate judged a tree that is no longer the branch's, and nothing reached the trunk

#### Scenario: A prune reads the evidence a refused publication staged

- GIVEN the ledger entry that refusal left behind
- WHEN a prune verb reads it
- THEN it ignores the entry and the branch keeps everything, because the proof is the trunk carrying the work and never the ledger saying so

Verify: `cargo nextest run -E 'test(integrate)'`

### `git:git-and-forge-terms-stay-explicit` — Git and forge terms stay explicit

Shared prose MUST say pull request or merge request, forge-specific prose MUST use its own forge's term, prose MUST write Git workflow, CI workflow, and release workflow in full rather than a bare workflow, and only internal code MAY name the common review unit `ForgeRequest`.

#### Scenario: A chapter names the review unit for both forges

- GIVEN a method chapter describing how a change reaches the trunk
- WHEN it names the review unit
- THEN it says pull request or merge request, and no served document carries `ForgeRequest`

Verify: `rg -ln 'ForgeRequest' method bindings runbooks forges skills skill-shared blocks guidance && exit 1 || exit 0`

### `git:concurrent-pull-and-merge-requests-carry-the-tested-trunk` — Concurrent pull and merge requests carry the tested trunk

A merge MUST carry the trunk it was tested against. On GitHub the protection domain enforces it through `protection.strict_required_status_checks` in both integration modes. A local integrator's repository-administrator role MAY bypass that rule for the deliberate trunk push, but the release App MUST NOT be a bypass actor, so its stale release request remains refused. On GitLab the protection domain enforces freshness through `protection.gitlab.merge_method = "ff"`, so a rebase is part of the GitLab path. The protection domain owns both mappings, and this convention refuses a merge queue on GitHub because the landed CI workflows carry no `merge_group` trigger and a queued merge would wait on a check that never reports.

#### Scenario: The two forges enforce one rule

- GIVEN a GitHub target whose strict policy is false and a GitLab target whose merge method is not `ff`
- WHEN `rk setup check` runs on each
- THEN `protect-trunk` faults on both, each naming its own forge's setting

#### Scenario: A local release request becomes stale

- GIVEN a locally integrated GitHub target whose administrator advanced the trunk after the release request's check passed
- WHEN the release App attempts the squash merge
- THEN the strict required-check rule refuses it because the App is not the repository-administrator bypass actor

Verify: `cargo nextest run -E 'test(a_loose_status_check_policy_faults) or test(the_merge_queue_fault_names_its_consequence) or test(the_strict_status_check_policy_holds_the_shape)'`
