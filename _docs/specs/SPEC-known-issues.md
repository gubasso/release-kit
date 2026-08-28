# Known Issues Specification

## Purpose

Rules governing known-issue records — the zone that holds an external defect this project works around. Covers the case id, the retirement condition, the mechanism walkthrough, and the body a filed record carries. Where the zone sits and how a suppression cites a record belong to `SPEC-spec-to-code.md`; the markdown a record is written in belongs to `SPEC-docs-format.md`.

## Requirements

### `known-issues:case-id-is-a-slug` — A case id is a slug

The author MUST name every known-issue record `KI-<slug>.md`, with a slug of lowercase letters, digits and hyphens that does not open with a digit.

#### Scenario: Two branches record a vendor defect in parallel

- GIVEN two worktrees each adding a record
- WHEN both allocate the next sequential number
- THEN two records claim one identity, which a slug name makes impossible

Verify: `pre-commit run ki-filename-shape --all-files`

### `known-issues:a-record-carries-its-retirement-condition` — A record carries its retirement condition

The author MUST give every known-issue record a non-empty `retire_when:` value.

#### Scenario: A workaround outlives its bug

- GIVEN a record whose workaround has no stated exit
- WHEN the upstream fix ships
- THEN nothing retires the workaround, and the next reader takes it for a design choice

Verify: `pre-commit run ki-retire-when --all-files`

### `known-issues:a-record-walks-the-mechanism` — A record walks its mechanism

The author MUST walk a known-issue record's mechanism step by step under a `## How it works` heading.

#### Scenario: A record names the defect and stops

- GIVEN a record stating only that the formatter moves the marker
- WHEN a reader arrives from a suppression's case id
- THEN the claim is unfalsifiable to everyone but its author, and the gate rejects the record

Verify: `pre-commit run ki-mechanism-walkthrough --all-files`

### `known-issues:a-filed-record-carries-its-report` — A filed record carries its report

Where `upstream:` names one issue rather than a tracker, the author MUST keep the filed text in a `## Report` section.

#### Scenario: A record is filed under deadline

- GIVEN a record written for the project and a report written into the tracker
- WHEN only the tracker holds the report
- THEN the two drift from that moment on, and the gate rejects the record

Verify: `pre-commit run ki-report-body --all-files`

### `known-issues:a-bugzilla-report-body-fits-in-79-columns` — A Bugzilla report body fits in 79 columns

Where `upstream:` names a Bugzilla tracker, the author MUST keep every line of the `## Report` body inside a fence and at or below 79 columns.

#### Scenario: An aligned table is pasted into a Bugzilla comment

- GIVEN a report whose table runs to 120 columns
- WHEN Bugzilla renders the comment as preformatted text
- THEN it wraps where Bugzilla chooses and the alignment carrying the argument is lost

Verify: `pre-commit run ki-bugzilla-report-width --all-files`
