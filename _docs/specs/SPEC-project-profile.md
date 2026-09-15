# Project Profile Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`project-profile:the-target-configuration-is-typed-by-domain` — The target configuration is typed by domain](#project-profilethe-target-configuration-is-typed-by-domain--the-target-configuration-is-typed-by-domain)
  - [`project-profile:the-profile-is-typed-and-the-forge-is-optional` — The profile is typed and the forge is optional](#project-profilethe-profile-is-typed-and-the-forge-is-optional--the-profile-is-typed-and-the-forge-is-optional)
  - [`project-profile:release-intent-has-three-modes` — Release intent has three modes](#project-profilerelease-intent-has-three-modes--release-intent-has-three-modes)
  - [`project-profile:an-unknown-category-is-preserved` — An unknown category is preserved](#project-profilean-unknown-category-is-preserved--an-unknown-category-is-preserved)
  - [`project-profile:availability-belongs-to-a-capability-at-its-dimensions` — Availability belongs to a capability at its dimensions](#project-profileavailability-belongs-to-a-capability-at-its-dimensions--availability-belongs-to-a-capability-at-its-dimensions)
  - [`project-profile:a-capability-is-available-only-when-complete` — A capability is available only when complete](#project-profilea-capability-is-available-only-when-complete--a-capability-is-available-only-when-complete)
  - [`project-profile:every-field-resolves-by-one-precedence` — Every field resolves by one precedence](#project-profileevery-field-resolves-by-one-precedence--every-field-resolves-by-one-precedence)
  - [`project-profile:a-record-is-source-free` — A record is source-free](#project-profilea-record-is-source-free--a-record-is-source-free)
  - [`project-profile:the-observation-proposes-a-release-mode` — The observation proposes a release mode](#project-profilethe-observation-proposes-a-release-mode--the-observation-proposes-a-release-mode)
  - [`project-profile:an-operation-refuses-only-what-it-requires` — An operation refuses only what it requires](#project-profilean-operation-refuses-only-what-it-requires--an-operation-refuses-only-what-it-requires)
  - [`project-profile:the-profile-command-writes-nothing` — The profile command writes nothing](#project-profilethe-profile-command-writes-nothing--the-profile-command-writes-nothing)
  - [`project-profile:a-schema-one-configuration-migrates-in-place` — A schema one configuration migrates in place](#project-profilea-schema-one-configuration-migrates-in-place--a-schema-one-configuration-migrates-in-place)
  - [`project-profile:a-destination-has-one-capability-owner` — A destination has one capability owner](#project-profilea-destination-has-one-capability-owner--a-destination-has-one-capability-owner)

<!--TOC-->

## Purpose

Rules governing the project profile and the capability selection derived from it: which technologies a target has, which forge hosts it if any, what its release intent is, which optional release-kit products it requests, and how every one of those answers resolves, records, and selects the destinations a landing writes. The committed file that carries the profile is bound by [the target configuration specification](./SPEC-target-config.md). The Git workflow beside it is bound by [the Git specification](./SPEC-git.md). What a landing does with the selected destinations is bound by [the landing specification](./SPEC-landing.md). A target configuration has several domains, and the project profile is one of them: it states what the project is, and nothing about how changes reach the trunk or which setup steps the project excludes.

## Requirements

### `project-profile:the-target-configuration-is-typed-by-domain` — The target configuration is typed by domain

The committed target configuration MUST carry each answer under the table of the domain that owns it: project identity under `[project]`, the profile under `[profile]` and `[profile.release]`, the Git workflow under `[git]`, the capability requests under `[capabilities]`, the security policy under `[security]`, the setup declaration under `[setup]`, and the protection policy under `[protection]`.

#### Scenario: A reader looks for where a topic branch opens

- GIVEN a schema 2 configuration
- WHEN the reader looks for the checkout mode
- THEN it stands under `[git]`, beside the trunk, and under no profile or landing table

Verify: `cargo nextest run -E 'test(the_landed_config_template_round_trips) or test(a_schema_1_config_migrates_into_its_domains)'`

### `project-profile:the-profile-is-typed-and-the-forge-is-optional` — The profile is typed and the forge is optional

The profile MUST carry zero or many technologies in `profile.technologies` and an optional forge in `profile.forge`, and a target with no forge MUST remain valid, with every forge-dependent capability reported as not applicable and every local capability still selected.

#### Scenario: A knowledge base with no forge lands

- GIVEN a Git repository with no version file and no origin remote
- WHEN `rk init --release-mode none --apply` runs
- THEN the local Git workflow guards land, the configuration records no forge and no technology, and `rk status --check` exits 0

Verify: `cargo nextest run -E 'test(a_target_with_no_technology_and_no_forge_lands_the_guards) or test(a_committed_empty_forge_outranks_the_record_and_the_remote) or test(a_committed_empty_forge_drops_every_forge_variant_from_the_guide)'`

### `project-profile:release-intent-has-three-modes` — Release intent has three modes

`profile.release.mode` MUST be one of `automatic`, `external`, and `none`. `automatic` MUST require a forge, a repository identity, a driver present in `profile.technologies`, and a style, and `external` and `none` MUST carry no driver, no style, and no line prefix. `external` names a release the target owns through a process release-kit does not drive, and it MUST route the operator to no bot-operate chapter.

#### Scenario: A release-less profile carries a style

- GIVEN a configuration with `mode = "none"` and `style = "trunk"`
- WHEN the reader loads it
- THEN it refuses naming `profile.release.style` and the shape a `none` mode takes

Verify: `cargo nextest run -E 'test(every_invalid_release_state_names_its_key)'`

### `project-profile:an-unknown-category-is-preserved` — An unknown category is preserved

A technology or forge name the catalog does not know MUST be preserved through resolution, the configuration, and the record, provided it matches `[a-z0-9][a-z0-9-]*` after lowercase normalization, and an unknown forge MUST have no adapter: local operations proceed, and a forge operation refuses only where the requested operation requires the adapter.

#### Scenario: A forge this release does not drive

- GIVEN `profile.forge = "codeberg"` and `mode = "none"`
- WHEN `rk init --apply`, `rk status`, and `rk upgrade --apply` run
- THEN the value survives all three, the local guards land, and the forge capabilities read as not applicable

Verify: `cargo nextest run -E 'test(an_unknown_forge_survives_init_status_and_upgrade)'`

### `project-profile:availability-belongs-to-a-capability-at-its-dimensions` — Availability belongs to a capability at its dimensions

The catalog MUST answer availability for one capability at its selected dimensions and never for a category alone, so `release.automation` at `(python, github)` is available while the same capability at `(python, gitlab)` is unavailable, and every omission MUST carry one of `unavailable`, `unknown`, `not-applicable`, `not-requested`, and `withheld`.

#### Scenario: One driver on two forges

- GIVEN two automatic profiles driven by `python`, one on each forge
- WHEN the catalog selects for each
- THEN the GitHub profile selects the release automation and the GitLab profile reports it unavailable naming the available tuples

Verify: `cargo nextest run -E 'test(the_catalog_answers_availability_per_tuple)'`

### `project-profile:a-capability-is-available-only-when-complete` — A capability is available only when complete

A capability MUST be selected only where its whole contribution lands active, and where a target-owned file blocks the activation, the projection MUST land the safe prerequisite files, record the capability as withheld with its reason and the operator's one remaining edit, and refuse nothing on that account.

#### Scenario: A release-less GitLab project owns its pipeline

- GIVEN a GitLab target with `mode = "none"` and a `.gitlab-ci.yml` of its own
- WHEN `rk init --apply` runs
- THEN the title fragment lands, the root pipeline is withheld naming the `include` line the operator adds, the target's pipeline is untouched, and the landing succeeds

Verify: `cargo nextest run -E 'test(an_occupied_gitlab_root_pipeline_withholds_the_title_gate)'`

### `project-profile:every-field-resolves-by-one-precedence` — Every field resolves by one precedence

Every profile, Git workflow, and capability field MUST resolve by one precedence: an invocation flag, then the committed configuration, then a compatible record, then observation, then a compiled default where the field has one. A repeatable flag MUST replace the declared list whole, a duplicate entry MUST refuse, and every wire list MUST sort.

#### Scenario: A flag names two technologies over a configured one

- GIVEN a configuration naming `rust` alone
- WHEN `rk init --technology python --technology rust` previews
- THEN the resolved list is `python, rust`, sorted, and `--technology rust --technology rust` refuses naming the duplicate

Verify: `cargo nextest run -E 'test(a_repeatable_technology_flag_replaces_the_list_whole)'`

### `project-profile:a-record-is-source-free` — A record is source-free

The configuration and the landing record MUST carry resolved values alone and no runtime precedence source, and the release style MUST have one owner in the record, `profile.release.style`.

#### Scenario: A record is read back

- GIVEN a landed target
- WHEN its record is parsed
- THEN no field names a flag, a configuration, or an observation as the value's source, and the style appears once

Verify: `cargo nextest run -E 'test(runtime_sources_never_reach_the_config_or_the_record) or test(a_release_mode_flag_retires_the_configured_automatic_release)'`

### `project-profile:the-observation-proposes-a-release-mode` — The observation proposes a release mode

Where no flag and no configuration answer the release mode, the observation MUST propose `none` for zero release-bearing technologies, `automatic` with that driver for exactly one, and MUST report an ambiguity for more than one, which a preview reports and an apply refuses until explicit release flags answer it.

#### Scenario: A repository carries a crate and a Python package

- GIVEN a target with `Cargo.toml` and `pyproject.toml` and no configuration
- WHEN `rk init` previews and then applies
- THEN the preview names both drivers and the ambiguity, and the apply refuses until `--release-driver` answers it

Verify: `cargo nextest run -E 'test(the_observation_proposes_a_release_mode) or test(an_observed_release_survives_a_missing_forge_and_the_apply_names_the_choice)'`

### `project-profile:an-operation-refuses-only-what-it-requires` — An operation refuses only what it requires

A valid profile MAY request an unavailable capability, and an apply MUST refuse only where the selected release automation is unavailable, naming the dimensions and the available tuples, while an unavailable optional capability MUST be reported and omitted.

#### Scenario: Python on GitLab asks for automation

- GIVEN a GitLab target driven by `python` with `mode = "automatic"`
- WHEN `rk init` previews and then applies
- THEN the preview reports the automation unavailable with the available tuples, and the apply refuses before any write

Verify: `cargo nextest run -E 'test(python_on_gitlab_reports_unavailable_and_apply_refuses) or test(a_binding_with_no_scanner_reports_the_capability_unavailable) or test(a_recorded_provider_the_pair_cannot_run_is_reported_and_omitted) or test(an_unavailable_provider_is_not_judged_on_its_licence)'`

### `project-profile:the-profile-command-writes-nothing` — The profile command writes nothing

`rk profile` MUST report every effective domain value with its runtime source, the unknown categories, the selected capabilities with their status, and the complete destinations the projection selects, and MUST write nothing and judge nothing.

#### Scenario: An agent asks what a landing would select

- GIVEN a target and any flag `rk init` takes
- WHEN `rk profile --json` runs
- THEN one `rk.profile/1` object reports the values, sources, capabilities, and destinations, and the tree is unchanged

Verify: `cargo nextest run -E 'test(profile_reports_values_sources_and_the_selection_and_writes_nothing) or test(the_profile_follow_up_command_runs_on_a_landed_target)'`

### `project-profile:a-schema-one-configuration-migrates-in-place` — A schema one configuration migrates in place

The configuration reader MUST read a schema 1 file through one bounded migration into the schema 2 domains, preserving each moved value's trailing comment, every free-standing comment, and the unknown-key policy, and the next landing MUST write the migrated file.

#### Scenario: A landed target upgrades from schema one

- GIVEN a schema 1 configuration whose `landing.style` line carries a comment
- WHEN `rk upgrade --apply` runs
- THEN the written file states `schema_version = 2`, the style stands under `[profile.release]` with its comment, and `landing.workflow`'s value reads as `linked-worktree` under `[git]`

Verify: `cargo nextest run -E 'test(a_schema_1_config_migrates_into_its_domains) or test(an_empty_technology_still_clears_the_automatic_release_keys) or test(an_emptied_project_header_goes_and_its_comment_stays) or test(a_dropped_schema_1_key_keeps_the_operators_comment) or test(a_moved_key_takes_the_comment_above_it)'`

### `project-profile:a-destination-has-one-capability-owner` — A destination has one capability owner

Every embedded source MUST belong to exactly one capability, `snippets/_shared/<forge>` MUST hold the forge's technology-independent sources rather than a technology's, no zone whose name begins with an underscore MAY be selectable as a technology, and a destination two capabilities both ship MUST refuse as a projection defect naming both, never one silently winning.

#### Scenario: The shared zone is offered as a technology

- GIVEN the embedded sources carrying `snippets/_shared/`
- WHEN the catalog reads every embedded source and the drivers are listed
- THEN each source names one owner, and no listing names `_shared`

Verify: `cargo nextest run -E 'test(every_embedded_snippet_has_one_owner) or test(duplicate_whole_file_destinations_and_overlapping_marked_regions_refuse_with_the_conflicting_source_names)'`
