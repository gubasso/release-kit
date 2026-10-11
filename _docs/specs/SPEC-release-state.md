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

`rk release status` MUST report, in one `rk.release-status/1` document and writing nothing, the recorded release style, integration authority, checkout mode, and trunk read offline, the open release request with its number, its proposed version, and whether it is armed, its checks, and whether it is behind the trunk, the last release with its tag and whether it is published and its artifacts verified, and every release line `rk lines list` reports.

#### Scenario: The release request is open, unarmed, and behind the trunk

- GIVEN a landed target whose release request proposes 1.2.3, carries no auto-merge, has a failing check, and was cut three commits before the trunk
- WHEN `rk release status --json` runs
- THEN the request reads `open`, number and version stated, armed `no`, checks `failing`, and behind the trunk `yes`, beside the last release and its read-back

Verify: `cargo nextest run -E 'test(release_status_reports_each_request_state)'`

### `release-state:a-symptom-names-its-recovery` — A symptom names its recovery

When the observed state shows a merged release request whose version has no tag or no published version, a published version whose release page carries no artifact, or an artifact no attestation covers, the report MUST list the symptom under its stable code, `request-merged-nothing-published`, `published-no-artifacts`, or `artifact-without-attestation`, with its evidence and the `rk method recovery` section that owns the path.

#### Scenario: The last release request merged and no tag followed

- GIVEN a forge whose newest merged release request proposed 1.2.3 and whose tags stop at v1.2.2
- WHEN `rk release status --json` runs
- THEN the report lists `request-merged-nothing-published` with the request number and the missing tag, and names "The release request merged and nothing published"

Verify: `cargo nextest run -E 'test(release_status_reports_each_detected_symptom)'`

### `release-state:a-wait-selects-runs-by-commit` — A wait selects runs by commit

When `rk release status --wait <sha>` runs, the verb MUST select the publish run and the artifact run by that commit and the push event, never by recency, wait until both conclude or the timeout passes, report the release they made, and exit 1 after the report where a run concluded without success.

#### Scenario: A newer run reports on a request while the release builds

- GIVEN a commit whose publish run succeeded and whose artifact run is still building, and a newer run of the publish workflow on a release request
- WHEN `rk release status --wait <sha> --json` runs
- THEN it waits for the artifact run on that commit, ignores the newer run, and reports the tag, published, and artifacts verified once both concluded

Verify: `cargo nextest run -E 'test(release_status_wait_selects_the_runs_on_the_commit)'`
