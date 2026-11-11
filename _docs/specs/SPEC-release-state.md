# Release State Specification

## Purpose

Rules governing what `rk release status` reports about a landed target's release: the recorded release style and Git workflow, the open release request, the last release, the release lines, the symptoms a stuck release shows, and the wait on the runs one commit starts. The boundary against `SPEC-landing.md` is the release itself: that spec binds what a landing wrote and whether it stayed truthful, and this one binds where the release process stands on the forge and the registry. Acting on that state, merging, holding, or re-running, is the operator's, and no rule here performs it. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification.

## Requirements

### `release-state:a-forge-field-reads-unknown-without-credentials` — A forge field reads unknown without credentials

If the runtime cannot read the forge or the registry, then every field that read serves MUST read `unknown`, the report MUST name why, and `rk release status` MUST still report the recorded fields and exit 0, asking for no credential.

#### Scenario: The forge CLI is missing on the host

- GIVEN a landed rust target and no usable `gh`
- WHEN `rk release status --json` runs
- THEN the style, the integration authority, and the checkout mode read from the record, the request and the last release read `unknown` field by field, and the command exits 0

Verify: `cargo nextest run -E 'test(release_status_reads_unknown_without_credentials)'`

### `release-state:the-report-joins-the-record-and-the-forge` — The report joins the record and the forge

`rk release status` MUST report, in one `rk.release-status/1` document and writing nothing, the recorded release style, integration authority, checkout mode, and trunk read offline, the open release request with its number, its proposed version, and whether it is armed, its checks, and whether it is behind the trunk, the last release with its tag and whether it is published and its artifacts verified, and every release line `rk lines list` reports. Where the pair publishes to no registry, `profile.release.registry = "none"`, a release MUST read published once its release page exists, and the report MUST read no registry.

#### Scenario: The release request is open, unarmed, and behind the trunk

- GIVEN a landed target whose release request proposes 1.2.3, carries no auto-merge, has a failing check, and was cut three commits before the trunk
- WHEN `rk release status --json` runs
- THEN the request reads `open`, number and version stated, armed `no`, checks `failing`, and behind the trunk `yes`, beside the last release and its read-back

Verify: `cargo nextest run -E 'test(release_status_reports_each_request_state) or test(release_status_reads_the_release_page_for_a_crate_with_no_registry)'`

### `release-state:a-symptom-names-its-recovery` — A symptom names its recovery

When the observed state shows a merged release request whose version has no tag or no published version, a published version whose release page carries no artifact, or an artifact no attestation covers, the report MUST list the symptom under its stable code, `request-merged-nothing-published`, `published-no-artifacts`, or `artifact-without-attestation`, with its evidence and the `rk method recovery` section that owns the path. Each release tag newer than the newest complete release of its own line, with no complete page itself, MUST be listed as `tag-without-release`: the trunk and each release line keep their own cutoff, a complete page is one with artifacts for a pair that builds them and one that exists for any other, and the read walks each line newest first and stops at its first complete page, so a tag older than a complete release of its line drops out and an accepted gap needs no record. Where the forge's tag list was read, each `v<version>` tag the clone holds, a candidate included, MUST be compared with it by commit, an annotated tag by the commit it points to: a tag the forge holds at another commit MUST be listed as `local-tag-differs` with both commits, and a tag the forge does not hold as `local-tag-not-on-forge` with its commit, each naming "A clone holds a stale release tag", stating that the forge holds the true release tags, and ending with its refresh, the single-tag forced fetch for the first and the pruning fetch of every `v*` tag for the second. A tag only the forge holds is no symptom.

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

Verify: `cargo nextest run -E 'test(release_status_reports_each_detected_symptom) or test(every_tag_after_the_newest_complete_release_is_reported) or test(each_release_line_keeps_its_own_cutoff) or test(local_tag_drift_reports_a_moved_and_a_local_only_tag) or test(release_status_reports_a_local_tag_at_another_commit) or test(release_status_compares_a_prerelease_and_ignores_a_non_version_tag) or test(release_status_reports_a_local_tag_the_forge_lacks) or test(an_annotated_local_tag_compares_by_its_commit) or test(a_tag_only_the_forge_holds_is_no_symptom) or test(release_status_compares_no_tag_when_the_forge_list_is_unread) or test(the_recovery_chapter_names_the_stale_local_tag)'`

### `release-state:a-wait-selects-runs-by-commit` — A wait selects runs by commit

When `rk release status --wait <sha>` runs, the verb MUST select the publish run and the artifact run by that commit and the push event, never by recency, wait until both conclude or the timeout passes, report the release they made, and exit 1 after the report where a run concluded without success.

#### Scenario: A newer run reports on a request while the release builds

- GIVEN a commit whose publish run succeeded and whose artifact run is still building, and a newer run of the publish workflow on a release request
- WHEN `rk release status --wait <sha> --json` runs
- THEN it waits for the artifact run on that commit, ignores the newer run, and reports the tag, published, and artifacts verified once both concluded

Verify: `cargo nextest run -E 'test(release_status_wait_selects_the_runs_on_the_commit)'`
