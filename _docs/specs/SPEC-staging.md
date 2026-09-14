# Staging Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`staging:production-never-reads-a-stage` — Production never reads a stage](#stagingproduction-never-reads-a-stage--production-never-reads-a-stage)
  - [`staging:a-stage-is-one-target-specific-candidate` — A stage is one target-specific candidate](#staginga-stage-is-one-target-specific-candidate--a-stage-is-one-target-specific-candidate)
  - [`staging:the-output-path-has-one-precedence` — The output path has one precedence](#stagingthe-output-path-has-one-precedence--the-output-path-has-one-precedence)
  - [`staging:a-visible-stage-is-complete` — A visible stage is complete](#staginga-visible-stage-is-complete--a-visible-stage-is-complete)
  - [`staging:the-artifacts-tree-holds-the-proposed-bytes` — The artifacts tree holds the proposed bytes](#stagingthe-artifacts-tree-holds-the-proposed-bytes--the-artifacts-tree-holds-the-proposed-bytes)
  - [`staging:the-reference-tree-is-the-installed-knowledge` — The reference tree is the installed knowledge](#stagingthe-reference-tree-is-the-installed-knowledge--the-reference-tree-is-the-installed-knowledge)
  - [`staging:the-stage-receipt-is-explanatory-metadata` — The stage receipt is explanatory metadata](#stagingthe-stage-receipt-is-explanatory-metadata--the-stage-receipt-is-explanatory-metadata)
  - [`staging:cleanup-removes-only-a-stage-that-names-itself` — Cleanup removes only a stage that names itself](#stagingcleanup-removes-only-a-stage-that-names-itself--cleanup-removes-only-a-stage-that-names-itself)
  - [`staging:cleanup-holds-what-it-validated-open` — Cleanup holds what it validated open](#stagingcleanup-holds-what-it-validated-open--cleanup-holds-what-it-validated-open)

<!--TOC-->

## Purpose

Rules governing the candidate stage: the disposable directory `rk stage` writes so an agent can read the complete candidate this installed binary would land in one target, beside the version-matched knowledge that explains it. A stage is evidence. It is created before a production landing, kept through it, and removed only by `rk stage clean`. The boundary against `SPEC-landing.md` is the write: that spec binds what production writes into a target and how it records it, and nothing here writes into a target. The boundary against `SPEC-distribution.md` is the source: that spec binds what the binary carries, and a stage copies from those embedded sources alone.

## Requirements

### `staging:production-never-reads-a-stage` — Production never reads a stage

`rk init`, `rk upgrade`, `rk adopt`, and `rk status` MUST render from the installed binary and the target alone, and MUST leave every stage unread, unnamed, and in place.

#### Scenario: A stage sits under the state root while production lands

- GIVEN a stage holding an unreadable sentinel file and a target the same binary stages
- WHEN `rk init --apply` and `rk status` run against the target
- THEN neither opens a file under the stage, neither output names the stage, and the stage survives byte for byte

Verify: `cargo nextest run -E 'test(production_commands_neither_read_nor_remove_a_stage)'`

### `staging:a-stage-is-one-target-specific-candidate` — A stage is one target-specific candidate

When `rk stage` runs, it MUST materialize this binary's projection for the one target it reads, under the resolved stage root alone, and MUST write nothing inside the target.

#### Scenario: A stage is created beside a landed target

- GIVEN a landed target and no stage
- WHEN `rk stage --target <path>` runs
- THEN the target's tree digests the same before and after, and every written path sits below the resolved stage root

Verify: `cargo nextest run -E 'test(stage_writes_only_below_the_resolved_stage_root) or test(a_stage_holds_every_projected_artifact_byte_for_byte_and_every_reference_root)'`

### `staging:the-output-path-has-one-precedence` — The output path has one precedence

`rk stage` MUST resolve its output directory as `--output` first, then a target and version directory below `RK_STAGE_ROOT`, then a target and version directory below the private state root, and MUST print the resolved path in human and `--json` output.

#### Scenario: The flag and the variable are both set

- GIVEN `RK_STAGE_ROOT` set and `--output <dir>` given
- WHEN `rk stage` runs
- THEN the stage lands at `<dir>`, both outputs print that path, and nothing appears below `RK_STAGE_ROOT`

Verify: `cargo nextest run -E 'test(stage_output_precedence_is_flag_then_env_then_state_root)'`

### `staging:a-visible-stage-is-complete` — A visible stage is complete

`rk stage` MUST refuse an existing nonempty output directory, build the stage under a fresh sibling, and rename it into place only after every file and `stage.json` are written, with the default root and the receipt created owner-only.

#### Scenario: A run stops before the receipt

- GIVEN a stage run interrupted after its first artifact
- WHEN the output path is listed
- THEN no directory stands at the resolved path, and a second run creates the whole stage

Verify: `cargo nextest run -E 'test(an_existing_nonempty_output_directory_refuses_byte_identically) or test(a_visible_stage_is_complete) or test(the_default_stage_root_is_owner_only)'`

### `staging:the-artifacts-tree-holds-the-proposed-bytes` — The artifacts tree holds the proposed bytes

The `artifacts/` tree MUST hold every candidate destination at its target-relative path with the complete proposed bytes, a marked-region destination as the complete spliced document.

#### Scenario: The routing block is staged for a target with its own `AGENTS.md`

- GIVEN a target whose `AGENTS.md` carries operator prose around the marked block
- WHEN `rk stage` runs
- THEN `artifacts/AGENTS.md` is the whole proposed document, its bytes outside the markers equal to the target's, and its bytes equal to what a production landing would write

Verify: `cargo nextest run -E 'test(a_stage_holds_every_projected_artifact_byte_for_byte_and_every_reference_root) or test(a_splice_artifact_is_a_complete_proposed_document)'`

### `staging:the-reference-tree-is-the-installed-knowledge` — The reference tree is the installed knowledge

The `reference/` tree MUST hold `CHANGELOG.md`, `guidance/`, `method/`, `bindings/`, `runbooks/`, `forges/`, the `rk-setup` skill, and the shared resources that skill routes to, copied from the installed binary and from nowhere else.

#### Scenario: An agent asks what the stage may teach it

- GIVEN a stage from the installed binary
- WHEN its `reference/` tree is listed
- THEN every named root is present at the binary's bytes, and no `_docs/`, test, or source file appears

Verify: `cargo nextest run -E 'test(a_stage_holds_every_projected_artifact_byte_for_byte_and_every_reference_root) or test(the_reference_tree_carries_no_docs_tests_or_source)'`

### `staging:the-stage-receipt-is-explanatory-metadata` — The stage receipt is explanatory metadata

`stage.json` MUST declare `schema: "rk.stage/1"` and carry the installed `rk` version, the canonical target identity, the resolved `stage_root`, the resolved parameters, the current receipt version where readable, and one entry per candidate or omission, and every landing command MUST refuse it as input.

#### Scenario: A stage receipt is offered to a landing verb

- GIVEN a complete stage
- WHEN an operator passes its path or its receipt to `rk init`, `rk upgrade`, or `rk adopt`
- THEN no flag takes it, and the landing renders afresh from the binary and the target

Verify: `cargo nextest run -E 'test(the_stage_receipt_and_human_output_snapshot_hold) or test(a_stage_receipt_is_accepted_by_no_landing_command)'`

### `staging:cleanup-removes-only-a-stage-that-names-itself` — Cleanup removes only a stage that names itself

`rk stage clean <path>` MUST resolve its argument without following a final symlink, and MUST refuse the filesystem root, a home directory, the target repository root, any ancestor of the target, a symlink, a directory without `stage.json`, a receipt outside `rk.stage/1`, and a receipt whose canonical `stage_root` differs from the argument.

#### Scenario: A symlink points at a real stage

- GIVEN a symlink whose target is a valid stage
- WHEN `rk stage clean <symlink>` runs
- THEN it refuses naming the link, the link and the stage stay, and `rk stage clean <stage>` then removes exactly that directory

Verify: `cargo nextest run -E 'test(stage_clean_refuses_every_protected_or_ambiguous_path_and_deletes_one_valid_stage) or test(stage_clean_after_a_production_landing_removes_the_stage_and_no_target_byte)'`

### `staging:cleanup-holds-what-it-validated-open` — Cleanup holds what it validated open

While it removes a stage, `rk stage clean` MUST hold the validated parent and stage directory open and delete through those descriptors alone, so a path swapped after validation redirects no deletion.

#### Scenario: The stage path is swapped for a link during removal

- GIVEN a valid stage and a process that replaces its path with a symlink to another directory after validation
- WHEN `rk stage clean` runs
- THEN the validated directory is removed and the linked directory keeps every byte

Verify: `cargo nextest run -E 'test(stage_clean_stays_confined_after_an_adversarial_swap)'`
