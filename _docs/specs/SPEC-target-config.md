# Target Configuration Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`target-config:the-config-is-input-and-the-record-is-the-record` — The config is input and the record is the record](#target-configthe-config-is-input-and-the-record-is-the-record--the-config-is-input-and-the-record-is-the-record)
  - [`target-config:a-config-states-its-schema` — A config states its schema](#target-configa-config-states-its-schema--a-config-states-its-schema)
  - [`target-config:an-unknown-key-refuses` — An unknown key refuses](#target-configan-unknown-key-refuses--an-unknown-key-refuses)
  - [`target-config:a-flag-overrides-and-a-landing-writes-back` — A flag overrides and a landing writes back](#target-configa-flag-overrides-and-a-landing-writes-back--a-flag-overrides-and-a-landing-writes-back)
  - [`target-config:an-absent-config-changes-nothing` — An absent config changes nothing](#target-configan-absent-config-changes-nothing--an-absent-config-changes-nothing)
  - [`target-config:an-invariant-bearing-key-carries-a-floor` — An invariant-bearing key carries a floor](#target-configan-invariant-bearing-key-carries-a-floor--an-invariant-bearing-key-carries-a-floor)
  - [`target-config:the-effective-protection-follows-the-profile` — The effective protection follows the profile](#target-configthe-effective-protection-follows-the-profile--the-effective-protection-follows-the-profile)
  - [`target-config:a-release-less-template-carries-no-release-table` — A release-less template carries no release table](#target-configa-release-less-template-carries-no-release-table--a-release-less-template-carries-no-release-table)
  - [`target-config:the-trunk-branch-has-one-owner` — The trunk branch has one owner](#target-configthe-trunk-branch-has-one-owner--the-trunk-branch-has-one-owner)
  - [`target-config:a-setup-fact-is-committed-once` — A setup fact is committed once](#target-configa-setup-fact-is-committed-once--a-setup-fact-is-committed-once)
  - [`target-config:an-exclusion-narrows-scope-and-not-policy` — An exclusion narrows scope and not policy](#target-configan-exclusion-narrows-scope-and-not-policy--an-exclusion-narrows-scope-and-not-policy)
  - [`target-config:an-unanswered-key-is-absent-and-not-empty` — An unanswered key is absent and not empty](#target-configan-unanswered-key-is-absent-and-not-empty--an-unanswered-key-is-absent-and-not-empty)
  - [`target-config:an-untaken-config-is-reported-and-not-judged` — An untaken config is reported and not judged](#target-configan-untaken-config-is-reported-and-not-judged--an-untaken-config-is-reported-and-not-judged)
  - [`target-config:a-host-install-is-a-recorded-answer` — A host install is a recorded answer](#target-configa-host-install-is-a-recorded-answer--a-host-install-is-a-recorded-answer)
  - [`target-config:an-open-migration-is-one-top-level-key` — An open migration is one top-level key](#target-configan-open-migration-is-one-top-level-key--an-open-migration-is-one-top-level-key)

<!--TOC-->

## Purpose

The committed answers in `.release-kit/config.toml`, their strict reader, comment-preserving writer, and policy floors. What the domains mean and how their values resolve belongs to [the project profile specification](./SPEC-project-profile.md) and [the Git specification](./SPEC-git.md); the landing record and the projection remain governed by [the landing specification](./SPEC-landing.md).

## Requirements

### `target-config:the-config-is-input-and-the-record-is-the-record` — The config is input and the record is the record

The landing verbs MUST resolve every class P value by one precedence — an invocation flag, the committed configuration, a compatible record, the target's observation, then a compiled default — and record every resolved answer, while comparisons project from the record alone.

#### Scenario: The configuration changes after landing

- GIVEN a record and an edited configuration
- WHEN a comparison renders its candidate
- THEN the candidate uses the record and the edit stays an input to the next landing

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains)'`

### `target-config:a-config-states-its-schema` — A config states its schema

The config reader MUST require integer `schema_version = 2`, read a schema 1 file through one bounded migration into the schema 2 domains, and refuse malformed content naming the file and parse position, or an unsupported schema naming the file and version.

#### Scenario: A newer schema reaches an older reader

- GIVEN a configuration declaring schema 999
- WHEN the reader loads it
- THEN the reader refuses naming `.release-kit/config.toml` and the supported schema, while a schema 1 file migrates and reads

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains)'`

### `target-config:an-unknown-key-refuses` — An unknown key refuses

When a hand-written config carries an unknown key, the reader MUST refuse naming the key and its nearest known spelling.

#### Scenario: A protection key is misspelled

- GIVEN a config carrying `trunk_rulesett`
- WHEN the reader parses the protection table
- THEN the refusal names that key and suggests `trunk_ruleset`

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains)'`

### `target-config:a-flag-overrides-and-a-landing-writes-back` — A flag overrides and a landing writes back

When a landing applies a class P invocation flag, the verb MUST write that key back through the comment-preserving editor, with class N names and class F policy left to their use-time readers. Exactly three class F keys are excepted, because `git.integration` decides them and a configuration carrying one authority's keys beside the other's mode is one its own floor table refuses: `protection.bypass_actors`, `protection.owned_trunk_rules`, and `protection.gitlab.push_access_level` travel with that parameter where they still match a compiled authority's tuple, keep whatever a target narrowed or widened them to otherwise, and are excluded from the comparison that reports untaken configuration, because the record holds no baseline for a floored policy. Every write of the configuration MUST refuse, writing nothing, where `.release-kit` or `.release-kit/config.toml` is a symlink or of another kind, because a write through one would land outside the target.

#### Scenario: The configuration directory is a symlink

- GIVEN a target whose `.release-kit` is a symlink to a directory elsewhere
- WHEN any verb writes the configuration
- THEN it exits 73 naming the path, and the directory the link names is unchanged

#### Scenario: The integration authority changes

- GIVEN a landed target whose protection keys are one authority's compiled tuple
- WHEN a landing resolves the other authority
- THEN all three keys are written to that authority's tuple, the configuration passes its own floor table, and status reports it aligned

#### Scenario: A style flag overrides a commented key

- GIVEN a commented `profile.release.style` value
- WHEN a landing applies a different style flag
- THEN the config carries the flag value and retains its comments and table ordering

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains)'`

### `target-config:an-absent-config-changes-nothing` — An absent config changes nothing

Where the config is absent, its reader MUST return absence so existing detection, recorded parameters and compiled defaults retain their compatibility meanings.

#### Scenario: A target predates configuration

- GIVEN a target with an older manifest and no configuration
- WHEN a reader loads the config
- THEN it returns absence without creating a file

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains)'`

### `target-config:an-invariant-bearing-key-carries-a-floor` — An invariant-bearing key carries a floor

The config reader MUST judge class F values through a floor table selected by the resolved `git.integration` mode, naming each key, its minimum and its source heading in `rk method invariants`, accept stricter values, and refuse a weaker value naming all three. Both tables MUST carry the request rule, required-check rule, and strict status-check policy. Under GitHub local integration the table MUST require exactly the stable `repository-admin` bypass, so an administrator may make the deliberate direct push while integrations remain governed; under GitLab it MUST refuse a push access level broad enough to name every writer. `protection.bypass_actors` MUST reach only the ruleset carrying the request and required-check rules: the deletion and force-push rules are installed with an empty bypass no key states, so no answer in this file can widen who may rewrite the trunk. The reader MAY accept the former compiled local tuple only so a landing or setup can migrate it to the current policy rather than strand an installed target. The merge floors `protection.allowed_merge_methods`, `protection.github.squash_title_source`, `protection.github.squash_body_source`, and `protection.gitlab.squash_option` MUST accept the values of the resolved `git.merge_form`: under `squash`, exactly squash, the request's title and body as the squash sources, and `squash_option = "always"`, and under `rebase`, exactly rebase and `squash_option = "never"`, where the two squash source floors do not apply. Every heading either table cites MUST exist in the invariants chapter.

#### Scenario: A target permits an additional merge method

- GIVEN a policy allowing squash and merge commits under `git.merge_form = "squash"`
- WHEN the config reader checks the policy
- THEN it refuses naming `protection.allowed_merge_methods`, exactly squash, and `rk method invariants`

#### Scenario: A rebase policy is judged under each merge form

- GIVEN a policy allowing rebase alone, with `protection.gitlab.squash_option = "never"`
- WHEN the config reader checks it under `rebase` and then under `squash`
- THEN the rebase check passes, and the squash check refuses naming `protection.allowed_merge_methods` and exactly squash

#### Scenario: One policy is judged under each mode

- GIVEN a policy with all four owned trunk rules and no bypass actor
- WHEN the config reader checks it under `forge` and then under `local`
- THEN the forge check passes, the local check refuses naming `protection.bypass_actors`, and adding `repository-admin` makes the local check pass

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains) or test(merge_form)'`

### `target-config:the-effective-protection-follows-the-profile` — The effective protection follows the profile

The setup MUST derive one effective protection from the configuration and the resolved profile, and the forge setters, the observer, and both `protections-check` scripts MUST read that one result, so a canonical apply never faults its own output. The tag protection MUST follow the release mode alone: `release.mode = "none"` requires no tag protection under either integration mode. The GitHub required status check and the GitLab pipeline requirement MUST follow the forge gate alone: where `project-profile:the-forge-gate-follows-the-merge-form` selects no gate, the GitHub trunk ruleset requires no status check and the GitLab trunk drops `only_allow_merge_if_pipeline_succeeds`, because no workflow reports a check and no pipeline runs, while no-deletion and no-force-push hold in every case. The expected merge settings MUST follow the resolved `git.merge_form`.

#### Scenario: A workflow-only target at each forge

- GIVEN a target landed with `--workflow-only` and `git.merge_form = "rebase"`, configured at GitHub and at GitLab
- WHEN `rk setup check` and each `protections-check` run
- THEN both pass with no release tag protection, no required status check, and no required pipeline, and each forge allows only the rebase form

#### Scenario: A release-bearing target loses a protection

- GIVEN a target with an automatic release whose tag protection or pipeline requirement is missing
- WHEN `rk setup check` runs
- THEN it faults naming the missing protection, as before

Verify: `cargo nextest run -E 'test(release_less) or test(merge_form)'`

### `target-config:a-release-less-template-carries-no-release-table` — A release-less template carries no release table

Under `profile.release.mode = "none"`, the configuration a landing writes MUST leave out the tag protection keys `protection.tag_ruleset`, `protection.tag_pattern`, and `protection.lines_ruleset`, and the `[setup.bot]` table, because no step of a release-less target reads them, and MUST leave out the `[security]` table unless the target requests the reporting policy, whose landed `SECURITY.md` reads that table's contact and response.

#### Scenario: A workflow-only landing writes its configuration

- GIVEN a target with a GitLab remote and no configuration
- WHEN `rk init --workflow-only --apply` writes the configuration
- THEN the file carries no tag protection key, no `[security]`, and no `[setup.bot]`, and it passes its own floor table

#### Scenario: A release-less landing requests the reporting policy

- GIVEN the same target
- WHEN `rk init --workflow-only --reporting-policy --apply` writes the configuration
- THEN the file keeps `[security]` for the landed `SECURITY.md`, carries no tag protection key and no `[setup.bot]`, and it passes its own floor table

Verify: `cargo nextest run -E 'test(release_less)'`

### `target-config:the-trunk-branch-has-one-owner` — The trunk branch has one owner

Every trunk consumer MUST read `git.trunk` through the setup context or the shared config accessor, whose compiled default is `master`, and every landed artifact naming a branch MUST carry the rendered trunk and the rendered `profile.release.line_prefix` rather than either literal, so the binary's behavior and the landed bytes name one branch.

#### Scenario: A target names main as its trunk

- GIVEN a configuration with `git.trunk = "main"`
- WHEN a consumer requests the trunk
- THEN the accessor returns main

#### Scenario: A target on its own trunk lands a workflow that runs there

- GIVEN a landed target whose configuration sets `git.trunk = "main"`
- WHEN the landing renders the release workflow and the hook block
- THEN the release trigger and every branch guard name main, and the record carries it

Verify: `cargo nextest run -E 'test(config) or test(the_trunk_branch_comes_from_the_config) or test(the_line_prefix_comes_from_the_config)'`

### `target-config:a-setup-fact-is-committed-once` — A setup fact is committed once

A setup fact the operator would otherwise retype MUST resolve from the committed configuration where no flag answers it, and an invocation flag MUST override it. The release gate's two answers resolve this way on GitHub alone, because GitLab requires its whole pipeline through one project setting, names no individual check, and refuses either answer as a usage error; the release-line protection runs in a full apply where `setup.release_lines` asks for it; the retired long-lived branches come from `setup.retired_branches`; and the bot App's public identifier comes from `setup.bot.app_id` where the environment carries none.

The two answers are class P landing parameters and MUST divide as follows. `setup.required_check` names the context the trunk ruleset requires in both integration modes and the context the rendered gate also judges under local integration. `setup.required_workflow` names the workflow whose completion wakes that gate, by the workflow's literal `name` and never by its filename, because a `workflow_run` trigger cannot name a check and cannot omit the workflow. One shape renders that gate: GitHub, an automatic release, the trunk style, and local integration together. Where no flag, configuration, or compatible record answers a key at that shape, the resolution MUST take this convention's own pair, `gate` and `ci`, so a landing renders a gate that names something rather than a hole no event can satisfy, and MUST write the resolved answer back to the configuration and record it, so the decision is visible and reproducible rather than compiled. Every other shape MUST resolve both to empty, except that under forge integration `setup.required_check` stays the context the trunk ruleset requires, which is the project's own answer, and `protect-trunk` keeps refusing until one is named. A setup check MUST fault where the workflow named by `setup.required_workflow` does not carry the one pull-request job reporting `setup.required_check`, because a gate that wakes on one workflow and judges a check another workflow reports either wakes too early or never wakes at all.

#### Scenario: A project commits its required check

- GIVEN a configuration with `setup.required_check = "gate"` and no flag
- WHEN a GitHub setup runs
- THEN the step receives that check, and the refusal that names the key does not fire

#### Scenario: Neither the file nor the flag names the check

- GIVEN a GitHub target whose `setup.required_check` is empty and no flag
- WHEN a full apply runs
- THEN the refusal names `setup.required_check` before it names the flag

#### Scenario: A locally integrated GitHub trunk release answers neither

- GIVEN a fresh GitHub target with an automatic release, the trunk style, and local integration, and no flag or committed answer for either key
- WHEN a landing verb applies
- THEN it records `gate` and `ci`, writes both into the configuration, and a setup check faults where the target's own workflows do not carry that pair

Verify: `cargo nextest run -E 'test(required_check) or test(release_gate) or test(the_release_lines_step_runs_when_the_config_asks) or test(retired_branches_come_from_the_config)'`

### `target-config:an-exclusion-narrows-scope-and-not-policy` — An exclusion narrows scope and not policy

A target states the setup steps it does not run in `setup.excluded_steps`, as a step id against the reason a report prints. The reader MUST refuse an id that names no setup step, naming it and its nearest known step, MUST refuse an exclusion that states no reason, and MUST judge every floor unchanged, because an exclusion says which steps this target runs and says nothing about what the method requires of the steps it does run. Nothing in this file needs a landing to be read: the setup verbs load it wherever it is, so the target class that lands no file can still declare its model. [The setup specification](./SPEC-forge-setup.md) binds what a run does with the declaration.

#### Scenario: A target excludes a step and weakens a policy

- GIVEN a configuration excluding `protect-trunk` and permitting merge commits
- WHEN the reader loads it
- THEN it refuses naming `protection.allowed_merge_methods`, because the exclusion lifts no floor

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains)'`

### `target-config:an-unanswered-key-is-absent-and-not-empty` — An unanswered key is absent and not empty

Where the resolved answers omit a class P key — the repository of a project with no forge, and the driver, style, and line prefix of a release that is not automatic — the writer MUST leave the key out of the file rather than write it empty, and MUST leave out a table every one of whose keys it omitted.

#### Scenario: A release-less project with no forge lands

- GIVEN a target whose profile names no forge and whose release mode is `none`
- WHEN the landing writes the configuration
- THEN the file carries no `repo`, no `driver`, no `style`, no `line_prefix`, and no empty `[project]` header

Verify: `cargo nextest run -E 'test(the_landed_config_template_round_trips) or test(a_target_with_no_technology_and_no_forge_lands_the_guards) or test(a_committed_empty_forge_outranks_the_record_and_the_remote) or test(a_pruned_header_leaves_no_comment_behind) or test(a_retired_key_drops_the_template_comment_and_keeps_the_operators)'`

### `target-config:an-untaken-config-is-reported-and-not-judged` — An untaken config is reported and not judged

When a committed class P answer differs from the record's, status MUST report that untaken configuration by its full key path in both modes and keep it informational under `--check`.

#### Scenario: Only configuration differs

- GIVEN a healthy landing whose configured style changed
- WHEN status runs with and without `--check`
- THEN both report `profile.release.style` as pending and exit 0

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record) or test(a_schema_1_config_migrates_into_its_domains)'`

### `target-config:a-host-install-is-a-recorded-answer` — A host install is a recorded answer

Where a project obtains `rk` from each host's own install rather than a manager file, the configuration MUST record it in the `acquisition` table as `source = "host"` and `minimum`, the lowest release a host may run, and a `minimum` that is no release version MUST refuse as `config-invalid`.

A manager file records its own pin, so the table exists only for the one method that leaves no file behind, and a configuration that omits it records no host decision. Every landing verb rewrites an existing configuration and keeps the table.

#### Scenario: A host answer names no version

- GIVEN a configuration whose `acquisition.minimum` reads `soon`
- WHEN any verb that reads the configuration runs
- THEN it refuses as `config-invalid` naming `acquisition.minimum`, and nothing is written

Verify: `cargo nextest run -E 'test(a_malformed_acquisition_minimum_refuses_as_config_invalid) or test(a_host_answer_survives_the_landing_and_keeps_the_configuration)'`

### `target-config:an-open-migration-is-one-top-level-key` — An open migration is one top-level key

While a migration is open, the configuration MUST name it in one top-level `migration` key beside `schema_version`, which only `rk migrate archive` sets and only `rk migrate close` removes, whose value MUST be the UTC second the archive opened, `%Y%m%dT%H%M%SZ`, followed by `-<n>` from 2 up where an earlier archive of the target took that second, and every other byte MUST be what a greenfield landing of the same answers writes, because the key is how every clone sees the open migration and its removal must leave nothing of the migration behind.

#### Scenario: A migrated target closes

- GIVEN a target whose archive set the key and whose greenfield landing rendered the rest
- WHEN `rk migrate close --apply` removes the key
- THEN the file equals the one a greenfield landing of the same answers writes

#### Scenario: Two archives open in one second

- GIVEN a migration workspace that already holds the stamp of this second
- WHEN a second archive claims its workspace in the same second
- THEN it takes the stamp with `-2`, and the key names that identifier, while a value that is no identifier refuses as `config-invalid`

Verify: `cargo nextest run -E 'test(a_migrated_landing_writes_the_greenfield_configuration_beside_the_key) or test(a_target_migrates_from_its_predecessor_and_closes) or test(a_second_archive_in_one_second_takes_the_next_free_identifier) or test(an_identifier_is_a_stamp_and_an_optional_counter) or test(a_migration_key_that_is_no_identifier_refuses)'`
