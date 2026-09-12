# Reconcile Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`reconcile:every-landing-write-comes-from-one-plan` — Every landing write comes from one plan](#reconcileevery-landing-write-comes-from-one-plan--every-landing-write-comes-from-one-plan)
  - [`reconcile:a-plan-carries-one-classification-and-its-findings` — A plan carries one classification and its findings](#reconcilea-plan-carries-one-classification-and-its-findings--a-plan-carries-one-classification-and-its-findings)
  - [`reconcile:an-operation-names-digests-and-never-bytes` — An operation names digests and never bytes](#reconcilean-operation-names-digests-and-never-bytes--an-operation-names-digests-and-never-bytes)
  - [`reconcile:readiness-is-the-worst-precondition` — Readiness is the worst precondition](#reconcilereadiness-is-the-worst-precondition--readiness-is-the-worst-precondition)
  - [`reconcile:every-observed-field-cites-evidence` — Every observed field cites evidence](#reconcileevery-observed-field-cites-evidence--every-observed-field-cites-evidence)
  - [`reconcile:the-fingerprint-binds-the-semantic-inputs` — The fingerprint binds the semantic inputs](#reconcilethe-fingerprint-binds-the-semantic-inputs--the-fingerprint-binds-the-semantic-inputs)
  - [`reconcile:plan-is-read-only-and-offline-by-default` — Plan is read-only and offline by default](#reconcileplan-is-read-only-and-offline-by-default--plan-is-read-only-and-offline-by-default)
  - [`reconcile:a-stored-plan-is-owner-only-and-pruned` — A stored plan is owner-only and pruned](#reconcilea-stored-plan-is-owner-only-and-pruned--a-stored-plan-is-owner-only-and-pruned)
  - [`reconcile:apply-revalidates-before-the-first-write` — Apply revalidates before the first write](#reconcileapply-revalidates-before-the-first-write--apply-revalidates-before-the-first-write)
  - [`reconcile:apply-proceeds-on-ready-alone` — Apply proceeds on ready alone](#reconcileapply-proceeds-on-ready-alone--apply-proceeds-on-ready-alone)
  - [`reconcile:an-apply-is-one-transaction-with-the-record-last` — An apply is one transaction with the record last](#reconcilean-apply-is-one-transaction-with-the-record-last--an-apply-is-one-transaction-with-the-record-last)
  - [`reconcile:every-front-lands-through-the-engine` — Every front lands through the engine](#reconcileevery-front-lands-through-the-engine--every-front-lands-through-the-engine)
  - [`reconcile:compatibility-is-declared-in-the-bundle-and-evaluated-per-axis` — Compatibility is declared in the bundle and evaluated per axis](#reconcilecompatibility-is-declared-in-the-bundle-and-evaluated-per-axis--compatibility-is-declared-in-the-bundle-and-evaluated-per-axis)
  - [`reconcile:guidance-ships-in-the-bundle-filtered-and-covered` — Guidance ships in the bundle, filtered and covered](#reconcileguidance-ships-in-the-bundle-filtered-and-covered--guidance-ships-in-the-bundle-filtered-and-covered)

<!--TOC-->

## Purpose

Rules governing the plan: the one typed document every landing write comes from. `rk reconcile plan` observes a target, resolves one release through the seam `SPEC-release-bundle.md` binds, computes the plan, and prints it. Setup, migration, upgrade, and drift are classifications of that plan, not separate implementations. This domain binds the plan's shape, its classification, its operations, its readiness policy, its provenance, its fingerprint, and what the planning verb may read. `SPEC-landing.md` binds what a landing owes a target, and the comparisons it names are what the planner computes. `SPEC-target-config.md` binds the configuration the planner resolves against. Applying a plan is bound here too: the store, the revalidation, the transaction, and the fronts that land through the engine. The two declarations a bundle carries beyond its bytes, `compatibility.toml` and `guidance/`, are read through the seam and bound here as the preconditions and the section they become.

## Requirements

### `reconcile:every-landing-write-comes-from-one-plan` — Every landing write comes from one plan

The engine MUST compute one typed document, `rk.plan/2`, that keeps five kinds apart: the evidence it observed, the analysis it derived, the policy each precondition carries, the decisions the operator owns, and the postconditions that prove completion. The planner MUST be a pure function of its inputs, with the clock as an input, so the same observation, the same bundles, and the same selected decisions produce the same plan and the same fingerprint.

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

Every precondition MUST carry a requirement from `advisory`, `decision-required`, and `required`, and the plan's readiness MUST be derived from the worst precondition: `blocked` where a required one does not hold, `needs-decision` where a decision-required one waits on a decision the operator has not selected, and `ready` otherwise, with an advisory precondition never counting. A gap is honest and is not permission, so a not-observed evaluation MUST count the same as an unsatisfied one under the requirement it carries. A decision MUST carry a stable id, its choices with their consequences, and its selected answer, and selecting it MUST satisfy the precondition that names it. A decision's choices are the whole of what answers it, so `--decide` MUST refuse an id no decision carries and an answer the named decision does not declare, naming what it takes, and a precondition MUST count only a declared answer as selected, because an answer outside the closed set is fingerprinted like any other and would be accepted again on every recomputation, leaving an apply to write with no choice ever taken. A repository nothing answered MUST be a required precondition that does not hold, because resolution stands the preview placeholder in for it so a plan can render at a target with no origin remote, and a rendered file carrying that placeholder names nobody's project.

#### Scenario: A first landing with no mode answered

- GIVEN an empty target planned with no `--workflow`, no configuration, and no decision
- WHEN the plan prints
- THEN its readiness is `needs-decision` naming `workflow-mode`, and the same call with `--decide workflow-mode=worktree` is `ready` with the answer selected

Verify: `cargo nextest run -E 'test(readiness_is_the_worst_precondition) or test(a_decision_required_precondition_resolves_when_its_decision_is_selected) or test(an_edited_rendered_file_is_a_blocked_conflict) or test(an_unresolved_repository_blocks_the_plan) or test(an_unrecognized_decision_answer_refuses_naming_the_choices) or test(every_decision_the_planner_asks_is_in_the_catalogue)'`

### `reconcile:every-observed-field-cites-evidence` — Every observed field cites evidence

Every observed value MUST be an item in the plan's evidence ledger with its kind, its producer, its instant, its digest where it is bytes, and its collection method, and every section of the observed state and the release MUST cite the items it rests on through `evidence_refs`, because a field that depends on the record, the disk, the bundle, and a fetch at once is honest only when it cites each one. Where the recorded release's bundle cannot be read, the baseline MUST be reported as not observed with the reason, never as absent, and a bundle the cache holds but cannot verify MUST read the same way rather than failing the plan, because the candidate is what an apply writes and refuses unverified while the recorded release only says what the target started from, so a plan nobody can compute leaves the operator no way to accept the gap.

#### Scenario: The recorded release is not in the cache

- GIVEN a landed target whose record names a payload this engine does not carry and the release cache does not hold
- WHEN `rk reconcile plan --json` runs without `--fetch`
- THEN the release's baseline reads `not-observed` naming the recorded version, the `baseline-observed` precondition carries the same reason, and every other section still cites its evidence

Verify: `cargo nextest run -E 'test(every_observed_field_cites_evidence) or test(a_missing_baseline_is_not_observed_with_its_reason) or test(an_unsealed_cached_baseline_is_not_observed_with_its_reason)'`

### `reconcile:the-fingerprint-binds-the-semantic-inputs` — The fingerprint binds the semantic inputs

The plan's fingerprint MUST be one digest over a canonical text of the target's absolute path, the candidate bundle's digest and schema, the record and configuration digests, every operation's kind, path, and before and after digests in execution order, every required precondition's evaluation, and every selected decision, and MUST exclude timestamps, presentation text, advisory evaluations, and any inline-versus-digest choice in a view, because approval binds to it and an apply refuses on any difference. The target MUST be resolved to an absolute path before the plan is computed and stored, because a relative target resolves against whichever directory an apply runs in, and two checkouts holding the same bytes are still two targets. Operation lines MUST NOT be sorted, because apply stages in the plan's own order and a reordering that moves the record ahead of the payload it describes is a different plan.

#### Scenario: One answer changes

- GIVEN an empty target planned twice, once with `--decide workflow-mode=worktree` and once with `branches`
- WHEN the two fingerprints are compared
- THEN they differ, while two plans at different instants over the same inputs share one

Verify: `cargo nextest run -E 'test(plans_differing_only_in_excluded_fields_share_a_fingerprint) or test(changing_a_selected_decision_changes_the_fingerprint) or test(a_stored_plan_refuses_to_apply_to_another_checkout) or test(a_reordered_stored_plan_refuses)'`

### `reconcile:plan-is-read-only-and-offline-by-default` — Plan is read-only and offline by default

`rk reconcile plan` MUST write nothing into the target, MUST write under the state root the stored plan alone, MUST read the embedded bundle unless `--to` names another release, and MUST reach the network in exactly two cases: a `--to` selector the binary does not carry, and `--fetch` for a recorded release the cache does not hold. The baseline MUST be the embedded bundle where the record names its payload, the release cache where it holds the recorded version, the crates venue under `--fetch`, and not observed otherwise.

#### Scenario: The network is gone

- GIVEN a landed target and a curl that fails every call
- WHEN `rk reconcile plan --json` runs with no `--to` and no `--fetch`
- THEN it exits 0 with a plan, the fetch log is empty, the target is byte-identical afterwards, and the state root holds the plan and nothing else new

Verify: `cargo nextest run -E 'test(reconcile_plan_is_offline_by_default) or test(reconcile_plan_persists_and_prints_an_id)'`

### `reconcile:a-stored-plan-is-owner-only-and-pruned` — A stored plan is owner-only and pruned

The engine MUST store every plan it computes under `<state root>/plans/<plan-id>/` with the plan, the request that computed it, and every blob it names by digest, MUST create that directory owner-only, MUST keep the newest 20 plans and prune the rest after every persist, and MUST answer `show` or `apply` on an id the store no longer holds with a refusal naming the retention rule, because a plan carries bytes from the target and a store nobody bounded grows forever. A plan is ephemeral: it is never committed and never posted to a forge.

#### Scenario: A plan is shown after the store pruned it

- GIVEN a stored plan whose directory the store no longer holds
- WHEN `rk reconcile show <plan-id>` runs
- THEN it exits 66 naming the id and the retention rule, and prints no empty plan

Verify: `cargo nextest run -E 'test(the_store_is_owner_only) or test(show_renders_a_stored_plan_and_refuses_a_pruned_one)'`

### `reconcile:apply-revalidates-before-the-first-write` — Apply revalidates before the first write

`rk reconcile apply` MUST compute the same plan again over the stored request, reading the candidate as the release the plan froze, served from the release cache by its exact version and never the selector resolved again, and refusing with `bundle-unverified` where the cache no longer holds that release, MUST compare the stored fingerprint, the stored document recomputed, and the fresh fingerprint, MUST refuse on any difference naming every field or destination that moved in one pass, MUST verify that every blob an operation names still digests to the name the store filed it under, and MUST verify that every destination still holds the digest its operation's `before` names, all before the first write, because the stored plan is what the operator reviewed and an apply that acted on anything else would execute what nobody read. The blob check is what turns a digest filename into a claim the bytes have to keep, since the store reads a blob back by that filename alone.

#### Scenario: The record moved between plan and apply

- GIVEN a stored plan over a landed target and a record edited after the plan was computed
- WHEN `rk reconcile apply <plan-id>` runs
- THEN it exits 73 with the reason `state-drift` naming the record, and the target is byte-identical afterwards

Verify: `cargo nextest run -E 'test(/^apply_refuses_after_/) or test(no_write_happens_before_revalidation_passes) or test(apply_never_resolves_the_selector_again) or test(apply_refuses_a_corrupted_blob_with_the_target_unchanged)'`

### `reconcile:apply-proceeds-on-ready-alone` — Apply proceeds on ready alone

`rk reconcile apply` MUST proceed on a plan whose readiness is `ready` and on no other, MUST name the unresolved decision ids for a plan that needs a decision and the failed required preconditions for a blocked one, and MUST offer no flag that makes a gap into a pass, because a gap is honest and is not permission. The freshly computed plan MUST be gated as well as the stored one, after revalidation so the more specific difference is named first, because canonicalization excludes decision-required evaluations and unselected decisions: a precondition that turns decision-required between plan and apply leaves every canonical line identical and only the fresh readiness sees it.

#### Scenario: A first landing with no mode answered is applied

- GIVEN a stored plan over an empty target whose readiness is `needs-decision`
- WHEN `rk reconcile apply <plan-id>` runs
- THEN it exits 73 with the reason `plan-not-ready` naming `workflow-mode`, and the target is byte-identical afterwards

Verify: `cargo nextest run -E 'test(apply_refuses_needs_decision_naming_the_ids) or test(apply_refuses_blocked_naming_the_preconditions) or test(every_apply_refusal_names_a_reason_from_the_closed_set) or test(apply_refuses_when_the_world_needs_a_new_decision)'`

### `reconcile:an-apply-is-one-transaction-with-the-record-last` — An apply is one transaction with the record last

`rk reconcile apply` MUST stage every write beside its destination before the first rename, MUST rename in operation order with the record last, MUST refuse before staging a plan whose operations do not end with exactly one record write, because staging follows the plan's own order and a record that lands before the payload it describes leaves a target claiming files it does not hold, MUST leave each destination holding either its previous bytes or its new ones when a rename fails and name every destination that landed before it, MUST run every postcondition the plan carries and report each outcome, and MUST journal the run under `rk runs` with the plan id, the fingerprint, and every operation's outcome. An apply MUST hold its target alone, from the observation the fresh plan is computed from through its postconditions, and MUST refuse with `target-busy` where another run holds it, naming what does and leaving the target unchanged, because two applies that each validate against the same target and then commit over each other leave one plan's files beside another's record, which no destination digest either of them checked would have shown. The lock MUST live outside the target, since a target's cleanliness is judged byte by byte, and an apply that cannot take the lock MUST refuse before it stages rather than proceed unheld, because a guard that silently does nothing still reads as a guard at its call site and a second lock location would exclude nobody. A postcondition that fails after the writes landed MUST exit 1 with the reason `postcondition-failed`; the standing `rk status --check` MUST be reported beside the checks and never fail the apply, because it judges the whole target, sentinels the operator still owes included.

#### Scenario: A rename stops part way

- GIVEN a stored plan over an empty target and a commit stopped at its second rename
- WHEN `rk reconcile apply <plan-id>` runs
- THEN it exits 74 naming the destination that stopped it and the one that landed, no record exists, no destination is half-written, and the journal carries the stop

Verify: `cargo nextest run -E 'test(an_interrupted_apply_leaves_each_destination_whole_and_journals_it) or test(the_record_is_written_last) or test(the_record_is_the_last_operation) or test(postconditions_run_and_a_failure_is_reported) or test(an_apply_lands_in_the_runs_journal) or test(the_apply_exit_codes_match_the_matrix) or test(a_second_apply_against_one_target_refuses_while_the_first_holds_it) or test(one_run_holds_a_target_at_a_time) or test(an_apply_refuses_when_it_cannot_take_the_target) or test(a_host_with_no_state_root_refuses)'`

### `reconcile:every-front-lands-through-the-engine` — Every front lands through the engine

`rk init`, `rk upgrade`, and `rk adopt` MUST compute their plan with a fixed intent, `setup`, `upgrade`, or `adopt`, and MUST land on `--apply` through the one execution path `rk reconcile apply` takes, with the store and the journal best effort because a front is one process with no review window. The landing a front produces MUST be the landing the engine produces under the same request, file for file, and the adopt intent MUST plan the configuration and the record and no other write. A pin a one-fact manager records MUST move as an `update-pin` operation of the plan, and the flake pin MUST stay the sync verb's, because that move needs nix and the network an offline apply never has. The adopt intent MUST plan no `update-pin` and MUST report a stale pin as an advisory precondition instead, because a manager file sits outside `.release-kit/` and an adoption writes the record alone.

#### Scenario: The three fronts and the engine land the same target

- GIVEN an empty target, a landed target one release behind, and a pre-record target matching the payload
- WHEN each front lands with `--apply` beside `rk reconcile plan` and `rk reconcile apply` under the same request
- THEN every file digests the same, and the records differ in their instant and their origin word alone

Verify: `cargo nextest run -E 'test(init_upgrade_and_adopt_produce_the_same_landing_through_the_engine) or test(adoption_writes_no_pin)'`

### `reconcile:compatibility-is-declared-in-the-bundle-and-evaluated-per-axis` — Compatibility is declared in the bundle and evaluated per axis

A bundle MUST declare in `compatibility.toml` what a landing needs beyond the payload schema, and a bundle that carries no file MUST read as no requirement beyond the schema. The planner MUST evaluate each axis into a precondition with a requirement, because every axis has stranded a target that discovered it at setup time. An engine below the declared minimum is `required` and blocks naming the engine to install. The binding's generator absent or below its `versions.toml` pin is `required` where the plan rewrites an artifact the target already holds, because that artifact's generated output must be regenerated, and `advisory` otherwise, because a first landing writes the artifact and generates nothing yet. A manager file present that names no release-kit, whether it names it without a version or does not name it at all, is `decision-required` under `pin-manager` where a recorded target is behind the candidate, because a pin move through a manager the target does not wire is a decision and not an operation. A forge reported below its declared floor is `required` where the plan writes that forge's pipeline and `advisory` otherwise, and a forge not asked is `advisory`. A declared intermediate release between the record and the candidate is `required` and blocks naming the version to pass through.

#### Scenario: A bundle names an intermediate release the upgrade would skip

- GIVEN a landed target two releases behind and a candidate bundle whose `compatibility.toml` names the release between them as intermediate
- WHEN `rk reconcile plan --json` runs
- THEN the plan is `blocked` on `intermediate-release:<version>` naming that version and `--to <version>`, and the same target one release behind plans with no such precondition

Verify: `cargo nextest run -E 'test(a_bundle_without_compatibility_requires_only_its_schema) or test(an_engine_below_requirement_is_blocked_naming_the_engine) or test(a_missing_generator_blocks_only_when_a_generated_artifact_is_planned) or test(an_unwired_manager_is_a_decision) or test(a_manager_file_naming_no_release_kit_asks_the_pin_decision) or test(a_forge_below_floor_blocks_only_when_a_forge_fact_affects_an_operation) or test(a_skipped_intermediate_version_is_blocked_naming_it) or test(the_payload_carries_twelve_roots)'`

### `reconcile:guidance-ships-in-the-bundle-filtered-and-covered` — Guidance ships in the bundle, filtered and covered

A bundle MUST carry under `guidance/` one file per release that needs an operator step, named by the version that introduces the change and naming the destinations it concerns and whether the action is an operator step or a plan operation, and MUST embed every such file whole, because the files are small text and a window nobody declared is a silent gap. The planner MUST select the files above the recorded release up to the candidate, MUST keep the ones naming a destination the target has and count the rest as excluded, and MUST report the coverage as one of `not-needed`, `covered`, `partial` naming the release above which the bundle describes every release, and `unavailable`, where `covered` with no step means no applicable steps and is never `unavailable`. Partial coverage MUST be `decision-required` under `partial-guidance`, and unavailable guidance MUST be `required` where the plan writes a rendered file and `advisory` otherwise. A release that changes a landed destination and ships no guidance file MUST be named by the authoring gate, unless `compatibility.toml` records it under `guidance.no_steps`. The interval MUST be derived from the record and the bundle alone, offline.

#### Scenario: A landed target far behind plans an upgrade

- GIVEN a landed target whose record predates the release the bundle describes from, carrying an `.envrc` a guidance file names
- WHEN `rk reconcile plan --json` runs with a curl that fails every call
- THEN the fetch log is empty, the guidance reads `partial` naming that release with the `.envrc` step selected, the readiness is `needs-decision` naming `partial-guidance`, and `--decide partial-guidance=accept` yields `ready` with a different fingerprint

Verify: `cargo nextest run -E 'test(a_release_with_no_steps_reports_no_applicable_steps) or test(guidance_is_filtered_against_the_targets_destinations_with_a_count) or test(partial_guidance_is_a_decision_and_a_selected_decision_resolves_it) or test(unavailable_guidance_for_a_planned_rendered_file_blocks) or test(the_changelog_interval_is_derived_offline) or test(every_guidance_file_names_its_destinations)'`
