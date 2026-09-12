# Reconcile Specification

## Purpose

Rules governing the plan: the one typed document every landing write comes from. `rk reconcile plan` observes a target, resolves one release through the seam `SPEC-release-bundle.md` binds, computes the plan, and prints it. Setup, migration, upgrade, and drift are classifications of that plan, not separate implementations. This domain binds the plan's shape, its classification, its operations, its readiness policy, its provenance, its fingerprint, and what the planning verb may read. `SPEC-landing.md` binds what a landing owes a target, and the comparisons it names are what the planner computes. `SPEC-target-config.md` binds the configuration the planner resolves against. Applying a plan is a later rule set in this domain.

## Requirements

### `reconcile:every-landing-write-comes-from-one-plan` — Every landing write comes from one plan

The engine MUST compute one typed document, `rk.plan/1`, that keeps five kinds apart: the evidence it observed, the analysis it derived, the policy each precondition carries, the decisions the operator owns, and the postconditions that prove completion. The planner MUST be a pure function of its inputs, with the clock as an input, so the same observation, the same bundles, and the same selected decisions produce the same plan and the same fingerprint.

#### Scenario: The same target is planned twice

- GIVEN a landed target and two calls to the planner with the same instant
- WHEN both plans serialize
- THEN the two documents are byte-identical, and a third call at another instant differs in its identity alone

Verify: `cargo nextest run -E 'test(the_plan_schema_is_versioned_and_snapshot_tested) or test(the_planner_is_deterministic) or test(plans_differing_only_in_excluded_fields_share_a_fingerprint)'`

### `reconcile:a-plan-carries-one-classification-and-its-findings` — A plan carries one classification and its findings

The plan MUST carry exactly one classification from the closed set `setup`, `migration`, `upgrade`, `drift`, and `invalid`, beside a list of findings that carry what the word compresses, because a reader routes on the word and reads the findings, and neither changes shape when a new finding appears. An empty target is `setup`. A target with another tool's release marker, a payload destination already present, or release activity no mechanism explains is `migration`. A recorded target whose owned files stand as the record left them is `upgrade`, whether or not the candidate changes anything. A recorded target with an owned file edited or missing is `drift`. A record this engine cannot read is `invalid`.

#### Scenario: A landed target one release behind and a landed target at the release

- GIVEN two landed targets, one whose record names the embedded payload and one whose record names an older one
- WHEN `rk reconcile plan --json` runs against each
- THEN both classify `upgrade`, the first carries no operation, and the operations alone tell them apart

Verify: `cargo nextest run -E 'test(each_of_the_six_target_states_classifies_correctly) or test(the_classification_table_covers_the_six_states) or test(a_current_target_plans_an_upgrade_with_no_operations)'`

### `reconcile:an-operation-names-digests-and-never-bytes` — An operation names digests and never bytes

Every operation MUST be one of `write-file`, `splice-block`, `remove-owned-file`, `write-record`, and `update-pin`, MUST name the digest of what it writes and the digest the destination must still hold where one exists, and MUST NOT carry bytes or a command, because the bytes live in the plan's blob store by digest and an apply that ran a stored command would execute something the operator never read.

#### Scenario: An empty target is planned

- GIVEN a directory with a version file and no landing
- WHEN `rk reconcile plan --json` runs with the identity flags
- THEN every destination the projection names appears as one operation with an `after` digest and no `before`, the record appears as `write-record`, and no operation carries a byte or a command

Verify: `cargo nextest run -E 'test(every_operation_names_digests_and_no_bytes) or test(an_empty_target_plans_a_setup_with_every_destination_written) or test(a_tuned_seeded_file_is_kept_and_its_baseline_moves)'`

### `reconcile:readiness-is-the-worst-precondition` — Readiness is the worst precondition

Every precondition MUST carry a requirement from `advisory`, `decision-required`, and `required`, and the plan's readiness MUST be derived from the worst precondition: `blocked` where a required one does not hold, `needs-decision` where a decision-required one waits on a decision the operator has not selected, and `ready` otherwise, with an advisory precondition never counting. A gap is honest and is not permission, so a not-observed evaluation MUST count the same as an unsatisfied one under the requirement it carries. A decision MUST carry a stable id, its choices with their consequences, and its selected answer, and selecting it MUST satisfy the precondition that names it.

#### Scenario: A first landing with no mode answered

- GIVEN an empty target planned with no `--workflow`, no configuration, and no decision
- WHEN the plan prints
- THEN its readiness is `needs-decision` naming `workflow-mode`, and the same call with `--decide workflow-mode=worktree` is `ready` with the answer selected

Verify: `cargo nextest run -E 'test(readiness_is_the_worst_precondition) or test(a_decision_required_precondition_resolves_when_its_decision_is_selected) or test(an_edited_rendered_file_is_a_blocked_conflict)'`

### `reconcile:every-observed-field-cites-evidence` — Every observed field cites evidence

Every observed value MUST be an item in the plan's evidence ledger with its kind, its producer, its instant, its digest where it is bytes, and its collection method, and every section of the observed state and the release MUST cite the items it rests on through `evidence_refs`, because a field that depends on the record, the disk, the bundle, and a fetch at once is honest only when it cites each one. Where the recorded release's bundle cannot be read, the baseline MUST be reported as not observed with the reason, never as absent.

#### Scenario: The recorded release is not in the cache

- GIVEN a landed target whose record names a payload this engine does not carry and the release cache does not hold
- WHEN `rk reconcile plan --json` runs without `--fetch`
- THEN the release's baseline reads `not-observed` naming the recorded version, the `baseline-observed` precondition carries the same reason, and every other section still cites its evidence

Verify: `cargo nextest run -E 'test(every_observed_field_cites_evidence) or test(a_missing_baseline_is_not_observed_with_its_reason)'`

### `reconcile:the-fingerprint-binds-the-semantic-inputs` — The fingerprint binds the semantic inputs

The plan's fingerprint MUST be one digest over a canonical text of the candidate bundle's digest and schema, the record and configuration digests, every operation's kind, path, and before and after digests, every required precondition's evaluation, and every selected decision, and MUST exclude timestamps, presentation text, advisory evaluations, and any inline-versus-digest choice in a view, because approval binds to it and an apply refuses on any difference.

#### Scenario: One answer changes

- GIVEN an empty target planned twice, once with `--decide workflow-mode=worktree` and once with `branches`
- WHEN the two fingerprints are compared
- THEN they differ, while two plans at different instants over the same inputs share one

Verify: `cargo nextest run -E 'test(plans_differing_only_in_excluded_fields_share_a_fingerprint) or test(changing_a_selected_decision_changes_the_fingerprint)'`

### `reconcile:plan-is-read-only-and-offline-by-default` — Plan is read-only and offline by default

`rk reconcile plan` MUST write nothing into the target and nothing under the state root, MUST read the embedded bundle unless `--to` names another release, and MUST reach the network in exactly two cases: a `--to` selector the binary does not carry, and `--fetch` for a recorded release the cache does not hold. The baseline MUST be the embedded bundle where the record names its payload, the release cache where it holds the recorded version, the crates venue under `--fetch`, and not observed otherwise.

#### Scenario: The network is gone

- GIVEN a landed target and a curl that fails every call
- WHEN `rk reconcile plan --json` runs with no `--to` and no `--fetch`
- THEN it exits 0 with a plan, the fetch log is empty, and the target and the state root are byte-identical afterwards

Verify: `cargo nextest run -E 'test(reconcile_plan_is_offline_by_default) or test(reconcile_plan_persists_nothing_in_this_phase)'`
