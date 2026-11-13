# Release State Specification

## Purpose

This spec governs what `rk release status` reports about the release of a landed target. It covers the recorded release style and Git workflow, the open release request, the last release, and the release lines. It also covers the symptoms of a stuck release and the wait on the runs that one commit starts. The release itself is the boundary against `SPEC-landing.md`. That spec binds what a landing wrote and whether it stayed truthful. This spec binds where the release process stands on the forge and the registry. The operator acts on that state with a merge, a hold, or a rerun, and no rule here performs that action. No adopting project adopts this spec, because a project cannot violate a rule about how `rk` behaves and cannot run the verification.

## Requirements

### `release-state:a-forge-field-reads-unknown-without-credentials` — A forge field reads unknown without credentials

If the runtime holds no credential for the forge or the registry, or the forge or the registry does not answer, then every field that read serves MUST read `unknown`, the report MUST name why, and `rk release status` MUST still report the recorded fields and exit 0, asking for no credential. A forge CLI that is missing or below its floor is no such boundary: the verb stops before its first read under `tool-requirements:a-verb-checks-its-tools-first`.

#### Scenario: The forge refuses the runtime's credential

- GIVEN a landed rust target and a `gh` that holds no credential the forge accepts
- WHEN `rk release status --json` runs
- THEN the style, the integration authority, and the checkout mode read from the record, the request and the last release read `unknown` field by field, and the command exits 0

#### Scenario: The forge CLI is missing on the host

- GIVEN a landed rust target and no `gh` on the host
- WHEN `rk release status --json` runs
- THEN it refuses before any read, naming gh and how to install it

Verify: `cargo nextest run -E 'test(release_status_reads_unknown_without_credentials) or test(release_status_stops_without_its_forge_cli)'`

### `release-state:the-report-joins-the-record-and-the-forge` — The report joins the record and the forge

`rk release status` MUST report, in one `rk.release-status/1` document and writing nothing, the recorded release style, integration authority, checkout mode, and trunk read offline, the open release request with its number, its proposed version, and whether it is armed, its checks, and whether it is behind the trunk, the last release with its tag and whether it is published and its artifacts verified, and every release line `rk lines list` reports. Where the pair publishes to no registry, `profile.release.registry = "none"`, a release MUST read published once its release page exists, and the report MUST read no registry.

#### Scenario: The release request is open, unarmed, and behind the trunk

- GIVEN a landed target whose release request proposes 1.2.3, carries no auto-merge, has a failing check, and was cut three commits before the trunk
- WHEN `rk release status --json` runs
- THEN the request reads `open`, number and version stated, armed `no`, checks `failing`, and behind the trunk `yes`, beside the last release and its read-back

Verify: `cargo nextest run -E 'test(release_status_reports_each_request_state) or test(release_status_reads_the_release_page_for_a_crate_with_no_registry)'`

### `release-state:an-armed-request-fails-the-report` — An armed request fails the report

Where the open release request stands armed to merge itself, the report MUST list the symptom `request-armed` with the request's number and the call that disarms it, naming the recovery section "A release request stands armed", and `rk release status` MUST exit non-zero after it rendered the whole report, because a release happens only on the operator's order and an arm that an older rk left standing would release without it. `rk status` and `rk status --check` MUST stay offline and MUST NOT judge an arm.

#### Scenario: An older rk left the request armed

- GIVEN a landed target whose open release request carries auto-merge on GitHub, or merges when its pipeline succeeds on GitLab
- WHEN `rk release status --json` runs
- THEN the report lists `request-armed` with the disarm call, and the verb exits non-zero

Verify: `cargo nextest run -E 'test(release_status_fails_on_an_armed_request) or test(release_status_reports_each_request_state)'`

### `release-state:the-operator-merges-through-the-verb` — The operator merges through the verb

`rk release merge` MUST read the one open release request against the trunk, the trunk tip, the request's head, the required check or the head pipeline for that head, and the proposed version, and without `--apply` MUST write and dispatch nothing, in one `rk.release-merge/1` document. It MUST refuse where no request or several requests stand open against the trunk, and under `--apply` where the head's check has not passed, because the merge job would refuse it anyway. Under `--apply` it MUST dispatch the landed merge job with exactly the values it read and a fresh correlation, MUST wait for that one run and for no other, and MUST report the merge commit and the `rk release status --wait` command that follows the release. It MUST read whether the request merged from the request itself after the run, and MUST report a merge even where the run then failed, because a cleanup can fail after the write. It MUST make no merge call itself, because the job proves the operator's order as the release App.

#### Scenario: The operator merges the trunk's release request

- GIVEN a landed target with one open release request whose check passed
- WHEN `rk release merge --apply` runs while another run of the same workflow concludes
- THEN it dispatches the merge job with the request, the trunk tip, the head, and a correlation, waits for the run that carries that correlation, and reports the merge

Verify: `cargo nextest run -E 'test(release_merge_previews_without_apply) or test(release_merge_refuses_zero_or_two_candidates) or test(the_wait_follows_only_the_correlated_run) or test(release_merge_dispatches_nothing_for_a_red_check) or test(the_merge_outcome_is_read_from_the_request)'`

### `release-state:a-symptom-names-its-recovery` — A symptom names its recovery

When the observed state shows a merged release request whose version has no tag or no published version, a published version whose release page carries no artifact, or an artifact no attestation covers, the report MUST list the symptom under its stable code, `request-merged-nothing-published`, `published-no-artifacts`, or `artifact-without-attestation`, with its evidence and the `rk method recovery` section that owns the path. Where the tag is absent and the registry already holds the version, the evidence MUST say so, because a published version closes the un-bump and leaves the rerun at the same commit. Each release tag newer than the newest complete release of its own line, with no complete page itself, MUST be listed as `tag-without-release`: the trunk and each release line keep their own cutoff, a complete page is one with artifacts for a pair that builds them and one that exists for any other, and the read walks each line newest first and stops at its first complete page, so a tag older than a complete release of its line drops out and an accepted gap needs no record. Where the forge's tag list was read, each `v<version>` tag the clone holds, a candidate included, MUST be compared with it by commit, an annotated tag by the commit it points to: a tag the forge holds at another commit MUST be listed as `local-tag-differs` with both commits, and a tag the forge does not hold as `local-tag-not-on-forge` with its commit, each naming "A clone holds a stale release tag", stating that the forge holds the true release tags, and ending with its refresh, the single-tag forced fetch for the first and the pruning fetch of every `v*` tag for the second. A tag only the forge holds is no symptom.

#### Scenario: The last release request merged and no tag followed

- GIVEN a forge whose newest merged release request proposed 1.2.3 and whose tags stop at v1.2.2
- WHEN `rk release status --json` runs
- THEN the report lists `request-merged-nothing-published` with the request number and the missing tag, and names "The release request merged and nothing published"

#### Scenario: Two tags followed the last complete release

- GIVEN a forge whose tags are v0.6.6 with a complete page, and v0.6.7 and v0.6.8 with no page
- WHEN `rk release status --json` runs
- THEN the report lists `tag-without-release` for v0.6.8 and v0.6.7 and names "The tag exists and the release does not", and a complete v0.6.9 retires both

#### Scenario: The clone kept a release tag that moved on the forge

- GIVEN a clone that holds v0.1.0 at one commit and a forge that holds v0.1.0 at another
- WHEN `rk release status --json` runs
- THEN the report lists `local-tag-differs` with v0.1.0, both commits, and `git fetch origin '+refs/tags/v0.1.0:refs/tags/v0.1.0'`, and names "A clone holds a stale release tag"

Verify: `cargo nextest run -E 'test(release_status_reports_each_detected_symptom) or test(release_status_reads_the_python_page_on_each_forge) or test(a_page_of_support_files_alone_is_no_verified_release) or test(every_tag_after_the_newest_complete_release_is_reported) or test(each_release_line_keeps_its_own_cutoff) or test(local_tag_drift_reports_a_moved_and_a_local_only_tag) or test(an_untagged_version_the_registry_holds_says_so) or test(release_status_reports_a_local_tag_at_another_commit) or test(release_status_compares_a_prerelease_and_ignores_a_non_version_tag) or test(release_status_reports_a_local_tag_the_forge_lacks) or test(an_annotated_local_tag_compares_by_its_commit) or test(a_tag_only_the_forge_holds_is_no_symptom) or test(release_status_compares_no_tag_when_the_forge_list_is_unread) or test(the_recovery_chapter_names_the_stale_local_tag)'`

### `release-state:a-wait-selects-runs-by-commit` — A wait selects runs by commit

When `rk release status --wait <sha>` runs, the verb MUST select the publish run and the artifact run by that commit and the push event, never by recency, wait until both conclude or the timeout passes, report the release they made, and exit 1 after the report where a run concluded without success.

#### Scenario: A newer run reports on a request while the release builds

- GIVEN a commit whose publish run succeeded and whose artifact run is still building, and a newer run of the publish workflow on a release request
- WHEN `rk release status --wait <sha> --json` runs
- THEN it waits for the artifact run on that commit, ignores the newer run, and reports the tag, published, and artifacts verified once both concluded

Verify: `cargo nextest run -E 'test(release_status_wait_selects_the_runs_on_the_commit) or test(release_status_waits_for_the_python_release_run) or test(an_engine_wait_on_gitlab_follows_the_release_line_pipeline)'`
