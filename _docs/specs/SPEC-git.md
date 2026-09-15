# Git Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`git:a-target-is-a-non-bare-repository` — A target is a non-bare repository](#gita-target-is-a-non-bare-repository--a-target-is-a-non-bare-repository)
  - [`git:the-method-is-trunk-based` — The method is trunk-based](#gitthe-method-is-trunk-based--the-method-is-trunk-based)
  - [`git:one-trunk-receives-pull-and-merge-requests` — One trunk receives pull and merge requests](#gitone-trunk-receives-pull-and-merge-requests--one-trunk-receives-pull-and-merge-requests)
  - [`git:the-trunk-name-is-a-parameter` — The trunk name is a parameter](#gitthe-trunk-name-is-a-parameter--the-trunk-name-is-a-parameter)
  - [`git:checkout-mode-selects-where-a-topic-branch-opens` — Checkout mode selects where a topic branch opens](#gitcheckout-mode-selects-where-a-topic-branch-opens--checkout-mode-selects-where-a-topic-branch-opens)
  - [`git:git-and-forge-terms-stay-explicit` — Git and forge terms stay explicit](#gitgit-and-forge-terms-stay-explicit--git-and-forge-terms-stay-explicit)
  - [`git:concurrent-pull-and-merge-requests-carry-the-tested-trunk` — Concurrent pull and merge requests carry the tested trunk](#gitconcurrent-pull-and-merge-requests-carry-the-tested-trunk--concurrent-pull-and-merge-requests-carry-the-tested-trunk)

<!--TOC-->

## Purpose

Rules governing the Git shape every target must have and the Git workflow parameters a target records: the non-bare repository, trunk-based development as the one development method, the trunk's name, the checkout mode a topic branch opens under, and the words shared prose uses for the two forges' review units. The protection domain in [the target configuration specification](./SPEC-target-config.md) and [the forge setup specification](./SPEC-forge-setup.md) owns how each forge enforces these rules. The project profile in [the project profile specification](./SPEC-project-profile.md) owns what the project is. This spec owns how changes reach its trunk.

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

One configured trunk MUST receive every change through a short-lived topic branch and one guarded pull request or merge request, squash-merged, so one request is one commit and the history stays linear.

#### Scenario: A change arrives outside a request

- GIVEN a landed target whose trunk protection the setup owns
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

### `git:git-and-forge-terms-stay-explicit` — Git and forge terms stay explicit

Shared prose MUST say pull request or merge request, forge-specific prose MUST use its own forge's term, prose MUST write Git workflow, CI workflow, and release workflow in full rather than a bare workflow, and only internal code MAY name the common review unit `ForgeRequest`.

#### Scenario: A chapter names the review unit for both forges

- GIVEN a method chapter describing how a change reaches the trunk
- WHEN it names the review unit
- THEN it says pull request or merge request, and no served document carries `ForgeRequest`

Verify: `rg -ln 'ForgeRequest' method bindings runbooks forges skills skill-shared blocks guidance && exit 1 || exit 0`

### `git:concurrent-pull-and-merge-requests-carry-the-tested-trunk` — Concurrent pull and merge requests carry the tested trunk

A merge MUST carry the trunk it was tested against. On GitHub the protection domain enforces it through `protection.strict_required_status_checks`. On GitLab the protection domain enforces it through `protection.gitlab.merge_method = "ff"`, so a rebase is part of the GitLab path to freshness. The protection domain owns both mappings, and this convention refuses a merge queue on GitHub because the landed CI workflows carry no `merge_group` trigger and a queued merge would wait on a check that never reports.

#### Scenario: The two forges enforce one rule

- GIVEN a GitHub target whose strict policy is false and a GitLab target whose merge method is not `ff`
- WHEN `rk setup check` runs on each
- THEN `protect-trunk` faults on both, each naming its own forge's setting

Verify: `cargo nextest run -E 'test(a_loose_status_check_policy_faults) or test(the_merge_queue_fault_names_its_consequence) or test(the_strict_status_check_policy_holds_the_shape)'`
