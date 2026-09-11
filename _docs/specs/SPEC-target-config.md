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
  - [`target-config:the-trunk-branch-has-one-owner` — The trunk branch has one owner](#target-configthe-trunk-branch-has-one-owner--the-trunk-branch-has-one-owner)
  - [`target-config:a-setup-fact-is-committed-once` — A setup fact is committed once](#target-configa-setup-fact-is-committed-once--a-setup-fact-is-committed-once)
  - [`target-config:an-untaken-config-is-reported-and-not-judged` — An untaken config is reported and not judged](#target-configan-untaken-config-is-reported-and-not-judged--an-untaken-config-is-reported-and-not-judged)

<!--TOC-->

## Purpose

The committed answers in `.release-kit/config.toml`, their strict reader, comment-preserving writer, and policy floors; the landing record and projection remain governed by [the landing specification](./SPEC-landing.md).

## Requirements

### `target-config:the-config-is-input-and-the-record-is-the-record` — The config is input and the record is the record

The landing verbs MUST resolve class P values from flags, committed configuration, detection and compiled defaults in that order, and record every value substituted into landed bytes or already carried by manifest parameters, while comparisons project from the manifest parameters alone.

#### Scenario: The configuration changes after landing

- GIVEN a record and an edited configuration
- WHEN a comparison renders its candidate
- THEN the candidate uses the record and the edit stays an input to the next landing

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record)'`

### `target-config:a-config-states-its-schema` — A config states its schema

The config reader MUST require integer `schema_version = 1` and refuse malformed content naming the file and parse position, or an unsupported schema naming the file and version.

#### Scenario: A newer schema reaches an older reader

- GIVEN a configuration declaring schema 999
- WHEN the reader loads it
- THEN the reader refuses naming `.release-kit/config.toml` and the supported schema

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record)'`

### `target-config:an-unknown-key-refuses` — An unknown key refuses

When a hand-written config carries an unknown key, the reader MUST refuse naming the key and its nearest known spelling.

#### Scenario: A protection key is misspelled

- GIVEN a config carrying `trunk_rulesett`
- WHEN the reader parses the protection table
- THEN the refusal names that key and suggests `trunk_ruleset`

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record)'`

### `target-config:a-flag-overrides-and-a-landing-writes-back` — A flag overrides and a landing writes back

When a landing applies a class P invocation flag, the verb MUST write that key back through the comment-preserving editor, with class N names and class F policy left to their use-time readers.

#### Scenario: A style flag overrides a commented key

- GIVEN a commented `landing.style` value
- WHEN a landing applies a different style flag
- THEN the config carries the flag value and retains its comments and table ordering

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record)'`

### `target-config:an-absent-config-changes-nothing` — An absent config changes nothing

Where the config is absent, its reader MUST return absence so existing detection, recorded parameters and compiled defaults retain their compatibility meanings.

#### Scenario: A target predates configuration

- GIVEN a target with a schema 5 manifest and no configuration
- WHEN a reader loads the config
- THEN it returns absence without creating a file

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record)'`

### `target-config:an-invariant-bearing-key-carries-a-floor` — An invariant-bearing key carries a floor

The config reader MUST judge class F values through one floor table naming each key, its minimum and its source heading in `rk method invariants`, accept stricter values, and refuse a weaker value naming all three.

#### Scenario: A target permits an additional merge method

- GIVEN a policy allowing squash and merge commits
- WHEN the config reader checks the policy
- THEN it refuses naming `protection.allowed_merge_methods`, exactly squash, and `rk method invariants`

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record)'`

### `target-config:the-trunk-branch-has-one-owner` — The trunk branch has one owner

Every trunk consumer MUST read `project.trunk` through the setup context or the shared config accessor, whose compiled default is `master`, and every landed artifact naming a branch MUST carry the rendered trunk and the rendered `setup.line_prefix` rather than either literal, so the binary's behavior and the landed bytes name one branch.

#### Scenario: A target names main as its trunk

- GIVEN a configuration with `project.trunk = "main"`
- WHEN a consumer requests the trunk
- THEN the accessor returns main

#### Scenario: A target on its own trunk lands a workflow that runs there

- GIVEN a landed target whose configuration sets `project.trunk = "main"`
- WHEN the landing renders the release workflow and the hook block
- THEN the release trigger and every branch guard name main, and the record carries it

Verify: `cargo nextest run -E 'test(config) or test(the_trunk_branch_comes_from_the_config) or test(the_line_prefix_comes_from_the_config)'`

### `target-config:a-setup-fact-is-committed-once` — A setup fact is committed once

A setup fact the operator would otherwise retype MUST resolve from the committed configuration where no flag answers it, and an invocation flag MUST override it. The required check resolves this way on GitHub alone, because GitLab names no individual check and refuses a supplied one; the release-line protection runs in a full apply where `setup.release_lines` asks for it; the retired long-lived branches come from `setup.retired_branches`; and the bot App's public identifier comes from `setup.bot.app_id` where the environment carries none.

#### Scenario: A project commits its required check

- GIVEN a configuration with `setup.required_check = "gate"` and no flag
- WHEN a GitHub setup runs
- THEN the step receives that check, and the refusal that names the key does not fire

#### Scenario: Neither the file nor the flag names the check

- GIVEN a GitHub target whose `setup.required_check` is empty and no flag
- WHEN a full apply runs
- THEN the refusal names `setup.required_check` before it names the flag

Verify: `cargo nextest run -E 'test(required_check) or test(the_release_lines_step_runs_when_the_config_asks) or test(retired_branches_come_from_the_config)'`

### `target-config:an-untaken-config-is-reported-and-not-judged` — An untaken config is reported and not judged

When committed class P configuration differs from recorded parameters, status MUST report that untaken configuration in both modes and keep it informational under `--check`.

#### Scenario: Only configuration differs

- GIVEN a healthy landing whose configured style changed
- WHEN status runs with and without `--check`
- THEN both report the untaken style and exit 0

Verify: `cargo nextest run -E 'test(config) or test(params_from_a_record)'`
