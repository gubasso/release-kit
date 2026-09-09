# Known Issues Specification

## Purpose

Rules governing known-issue records — the zone that holds an external defect this project works around. Covers the case id, the two state axes, the retirement condition, the mechanism walkthrough, and the body a filed record carries. Where the zone sits and how a suppression cites a record belong to `SPEC-spec-to-code.md`; the markdown a record is written in belongs to `SPEC-docs-format.md`.

## Requirements

### `known-issues:case-id-is-a-slug` — A case id is a slug

The author MUST name every known-issue record `KI-<slug>.md`, with a slug of lowercase letters, digits and hyphens that does not open with a digit.

#### Scenario: Two branches record a vendor defect in parallel

- GIVEN two worktrees each adding a record
- WHEN both allocate the next sequential number
- THEN two records claim one identity, which a slug name makes impossible

Verify: `pre-commit run ki-filename-shape --all-files`

### `known-issues:a-record-carries-one-state` — A record carries one state

The author MUST give every known-issue record exactly one `state:` value, from `investigating`, `mitigated`, `masked` and `monitoring`.

#### Scenario: A record states a handling no one defined

- GIVEN a record carrying `state: closed`
- WHEN a reader sorts the zone by how each case is handled
- THEN the record answers with a word the method never defined, and the gate rejects it

Verify: `pre-commit run ki-state --all-files`

### `known-issues:a-record-carries-one-filing-state` — A record carries one filing state

The author MUST give every known-issue record exactly one `filing:` value, from `gathering`, `ready`, `filed` and `deferred`.

#### Scenario: A team asks which cases it can file today

- GIVEN a zone whose records state only how each case is handled
- WHEN someone looks for the cases whose evidence is ready to file
- THEN every record has to be read in full, because no field carries the answer

Verify: `pre-commit run ki-filing --all-files`

### `known-issues:a-record-carries-its-retirement-condition` — A record carries its retirement condition

The author MUST give a masked record a non-empty `retire_when:` value, and MUST NOT give a mitigated record one.

#### Scenario: A workaround outlives its bug

- GIVEN a masked record whose workaround has no stated exit
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

Where `filing:` is `filed`, the author MUST name the issue in `upstream:` and keep the filed text in a `## Report` section.

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
