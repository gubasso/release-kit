# Accepted Drift Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`accepted-drift:an-accepted-drift-names-its-reason-and-its-bytes` — An accepted drift names its reason and its bytes](#accepted-driftan-accepted-drift-names-its-reason-and-its-bytes--an-accepted-drift-names-its-reason-and-its-bytes)

<!--TOC-->

## Purpose

Rules governing the landing declaration: the rendered destinations a project deliberately edits, which `[landing.accepted_drift]` in `.release-kit/config.toml` names with a reason and the digest of the accepted bytes. It is the landing-side twin of `setup.excluded_steps`. The configuration's reader and writer are bound by `SPEC-target-config.md`, and what a landing writes and status judges are bound by `SPEC-landing.md`; this spec binds what an accepted deviation changes in both. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification.

## Requirements

### `accepted-drift:an-accepted-drift-names-its-reason-and-its-bytes` — An accepted drift names its reason and its bytes

A project MAY accept drift on a rendered whole-file destination the landing selects and the receipt records only by naming the reason and the SHA-256 of the accepted bytes. An empty reason, a digest that is not 64 lowercase hex characters, a marked block, a `seeded` or `state` destination, and a destination the landing does not select MUST refuse as `config-invalid`, naming the destination and the fault. `rk status --check` MUST NOT count a file whose bytes match its entry a violation, MUST count a further edit one, naming the reason and printing the current digest, and MUST report an entry whose file matches the receipt again as stale, informationally. Accepted drift MUST exempt the file from the drift judgment alone, so an invariant failure and a setup proof code stay violations. A landing MUST preserve an accepted destination with no `--replace`, record the candidate's digest rather than the bytes on disk, and report whether the candidate moved since the previous receipt. `rk profile` MUST report the declaration in its own domain with its source.

#### Scenario: A pipeline carries a deviation the parameters cannot reach

- GIVEN a landed bash GitLab target whose `.gitlab-ci.yml` the project edited, and an entry naming the file with its reason and the edited bytes' digest
- WHEN `rk status --check` runs, then `rk upgrade --apply` twice, then the file is edited again
- THEN status reports the accepted drift and exits 0, each upgrade preserves the file and records the candidate's digest, and the further edit is a violation and a landing conflict again

Verify: `cargo nextest run -E 'test(accepted_drift) or test(status_check_reads_clean_on_accepted_drift) or test(status_check_faults_a_further_edit_under_accepted_drift) or test(status_reports_a_stale_accepted_drift_entry) or test(rk_profile_reports_the_landing_declaration_by_domain) or test(an_upgrade_preserves_an_accepted_drift_file) or test(an_upgrade_refuses_a_further_edit_under_accepted_drift) or test(an_upgrade_reports_a_moved_candidate_under_accepted_drift) or test(status_reads_accepted_drift_after_an_upgrade) or test(a_second_upgrade_preserves_an_accepted_drift_file) or test(removing_the_entry_makes_the_file_rendered_drift_again) or test(the_status_hook_passes_on_a_repository_with_accepted_drift) or test(an_upgrade_refuses_an_accepted_file_changed_mid_landing)'`
