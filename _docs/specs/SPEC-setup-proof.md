# Setup Proof Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`setup-proof:current-evidence-overrides-the-proof` — Current evidence overrides the proof](#setup-proofcurrent-evidence-overrides-the-proof--current-evidence-overrides-the-proof)
  - [`setup-proof:an-unavailable-credential-is-an-observation-boundary` — An unavailable credential is an observation boundary](#setup-proofan-unavailable-credential-is-an-observation-boundary--an-unavailable-credential-is-an-observation-boundary)
  - [`setup-proof:a-checkpoint-records-only-a-complete-observation` — A checkpoint records only a complete observation](#setup-proofa-checkpoint-records-only-a-complete-observation--a-checkpoint-records-only-a-complete-observation)
  - [`setup-proof:the-proof-carries-no-secret-or-machine-coordinate` — The proof carries no secret or machine coordinate](#setup-proofthe-proof-carries-no-secret-or-machine-coordinate--the-proof-carries-no-secret-or-machine-coordinate)
  - [`setup-proof:the-status-reads-the-proof-offline` — The status reads the proof offline](#setup-proofthe-status-reads-the-proof-offline--the-status-reads-the-proof-offline)
  - [`setup-proof:one-report-owns-every-classification` — One report owns every classification](#setup-proofone-report-owns-every-classification--one-report-owns-every-classification)

<!--TOC-->

## Purpose

Rules governing what a setup observation leaves behind and how a runtime reads it: the typed report one observation produces, the committed `.release-kit/setup-proof.json` a complete observation writes, the offline status that judges it, and how a check reads a credential it cannot reach. The boundary against `SPEC-forge-setup.md` is the observation itself: that spec binds what each step asserts and how a run reaches the forge, and this one binds what a run's result means once it exists. The proof is neither desired configuration, bound by `SPEC-target-config.md`, nor a landing receipt, bound by `SPEC-landing.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification.

## Requirements

### `setup-proof:current-evidence-overrides-the-proof` — Current evidence overrides the proof

If a check that could observe a step finds it unsatisfied, then `rk setup check` MUST report the step unsatisfied, exit 1, name the remedy, and state that the observation supersedes any committed proof.

#### Scenario: The App is uninstalled after a proof was committed

- GIVEN a compatible committed proof and an installation the App no longer has
- WHEN `rk setup check` runs with the App's credentials
- THEN `install-bot` reports unsatisfied, the verdict names `--apply` for it, and the verdict says the observation supersedes the committed proof

Verify: `cargo nextest run -E 'test(a_step_found_wrong_now_supersedes_a_compatible_proof)'`

### `setup-proof:an-unavailable-credential-is-an-observation-boundary` — An unavailable credential is an observation boundary

While `rk setup check` runs, a named key file the runtime cannot reach MUST leave `install-bot` unknown and every other step observed, and a check whose only gaps are unknown steps MUST exit 1 with the reason `observation-incomplete` and prescribe no apply; `rk setup --apply` and `rk setup checkpoint` MUST refuse the same key before any mutation or write.

#### Scenario: The key lives on the operator's host and not in the container

- GIVEN a fully set-up target and `RK_BOT_PRIVATE_KEY_FILE` naming a path absent from this runtime
- WHEN `rk setup check` runs
- THEN `install-bot` reports unknown, `protections-check` reports ok, the verdict says no setup defect was inferred, and no line prescribes `--apply`

Verify: `cargo nextest run -E 'test(a_check_reads_an_unreachable_key_as_an_observation_boundary) or test(apply_and_checkpoint_refuse_an_unreachable_key_and_every_mode_refuses_a_wrong_one)'`

### `setup-proof:a-checkpoint-records-only-a-complete-observation` — A checkpoint records only a complete observation

`rk setup checkpoint` MUST write `.release-kit/setup-proof.json` in one rename, and only where no applicable step is unsatisfied or unknown; an incomplete observation MUST leave an earlier proof byte for byte and create no file where none stood.

#### Scenario: A checkpoint runs without the App's credentials

- GIVEN a committed proof and a runtime with no App credentials
- WHEN `rk setup checkpoint` runs
- THEN it exits 1, says the proof was not written, and the committed proof is unchanged

Verify: `cargo nextest run -E 'test(a_checkpoint_refuses_an_incomplete_observation_and_keeps_the_earlier_proof) or test(only_a_complete_observation_is_checkpointable)'`

### `setup-proof:the-proof-carries-no-secret-or-machine-coordinate` — The proof carries no secret or machine coordinate

The proof MUST carry the setup contract and each step's normalized state and stable limitation alone, and MUST carry no key material, token, credential path, forge response text, process output, or path of the machine that wrote it.

#### Scenario: A checkpoint runs with the key and the App identifier exported

- GIVEN a checkpoint that read a key file and minted an App token
- WHEN the written proof is searched for the key path, its bytes, the App identifier, a token prefix, and the fixture's home and journal paths
- THEN none of them appears

Verify: `cargo nextest run -E 'test(a_checkpoint_records_a_complete_observation_that_the_status_reads_offline)'`

### `setup-proof:the-status-reads-the-proof-offline` — The status reads the proof offline

`rk setup status` MUST judge the committed proof against the target's current setup contract with no forge call and no credential, and MUST report `compatible`, `stale` with each changed field named, `invalid` with its reason, or `absent`, where a newer binary alone leaves a proof compatible.

#### Scenario: The required check changes after the proof

- GIVEN a committed proof written with the required check `test-check`
- WHEN `rk setup status --required-check another-check` runs
- THEN it reports `stale` and names `target.required_check`, and no forge CLI was called

Verify: `cargo nextest run -E 'test(the_status_names_what_moved_since_the_proof) or test(each_unreadable_proof_names_its_own_reason) or test(a_changed_contract_names_each_field)'`

### `setup-proof:one-report-owns-every-classification` — One report owns every classification

One observation MUST produce one typed report, which keeps each step's applicability apart from its observed state, and the human lines, the events, the check's verdict, and the proof MUST all read that report.

#### Scenario: One unknown row joins an otherwise complete report

- GIVEN a report holding a satisfied row, a limited row, a skipped row, and a row the target excludes
- WHEN one unknown or one unsatisfied row joins it
- THEN the report stops being checkpointable, and each observer answer keeps its own word through the classification

Verify: `cargo nextest run -E 'test(only_a_complete_observation_is_checkpointable) or test(an_observation_keeps_its_distinctions)'`
