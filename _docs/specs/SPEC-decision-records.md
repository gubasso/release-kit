# Decision Records Specification

## Purpose

Rules governing decision records under `<root>/decisions/`. Covers naming, permanence, and size. The record template covers what a record's body contains. The rules a record's decision enforces live in whichever spec owns them.

## Requirements

### `decision-records:filename-carries-no-digit` — A decision record filename carries no digit

When an author creates a decision record, the author MUST name it `ADR-<slug>.md`.

#### Scenario: Two agents create a record in parallel

- GIVEN two worktrees each adding a record
- WHEN both allocate the next sequential number
- THEN two records claim one identity, which a slug name makes impossible

Verify: `pre-commit run adr-filename-shape --all-files`

### `decision-records:a-record-follows-the-heading-shape` — A record follows the heading shape

The author MUST give every `ADR-*.md` one title followed by `## Context and Problem Statement`, `## Considered Options`, `## Decision Outcome`, `## Consequences`, and `## Status`, in that order and case, and no other heading.

The documentation root's managed lint configuration holds the shape with case matched.

#### Scenario: A record gains a sixth section

- GIVEN a record whose author adds `## Notes` after its status
- WHEN the linter runs over the record
- THEN the heading-shape rule fails it, because the five sections are the whole record and a note belongs inside one of them

Verify: `pre-commit run markdownlint-cli2 --all-files`

### `decision-records:a-citation-resolves-to-a-rule` — A citation resolves to a rule

Where a record names what enforces it, the author MUST cite rule IDs the specs still define, until another record supersedes it.

A superseded record keeps the pointer it was frozen with. Its successor carries what binds now, and its own `Enforced by` line is part of the history. Without that, retiring a rule would be impossible: the record that cited it cannot be rewritten, so the citation would hold the rule alive forever.

#### Scenario: A rule moves to a domain of its own

- GIVEN a record whose `Enforced by` line cites `distribution:license-declares-both-halves`
- WHEN the rule is renamed to `release:license-declares-both-halves`
- THEN the citation resolves to nothing. The gate reports it because the pointer to what binds is the one part of a frozen record that must keep working

Verify: `pre-commit run adr-cites-a-live-rule --all-files`

### `decision-records:record-is-not-revised` — A decision record is not revised

The author MUST NOT edit a decision record to describe a later design.

Repairing a path the record names, where the file moved and the record's claim did not change, is not a revision.

#### Scenario: A rule the record established is later narrowed

- GIVEN a merged record establishing a rule
- WHEN a later change narrows that rule
- THEN the spec carries the narrowed rule and the record keeps its original wording

Verify: reviewer confirms that the change under review edits no accepted record to describe a later design

### `decision-records:merged-record-is-permanent` — A merged decision record is permanent

The author MUST NOT delete or rename a merged decision record.

A decision record of a previous convention is a migration source, not a record of this instance: a migration archives it and may merge it with others into one new record. Once a record lands in the instance, this rule binds it.

#### Scenario: A record's decision is reversed

- GIVEN a record whose choice the project later abandons
- WHEN the successor is written
- THEN the original keeps its filename, gains a status, and links its successor

Verify: `git log --diff-filter=DR --name-only --format= -- '*/decisions/ADR-*' | grep . && exit 1 || exit 0`

### `decision-records:body-stays-within-350-words` — A decision record stays within 350 words

The author MUST keep a filled decision record at or below 350 words.

A ceiling the project records for an inherited record under `SPEC-budget-debt.md` is judged instead of the cap, and it only comes down.

#### Scenario: A record grows past the cap

- GIVEN a record covering both a storage choice and a migration approach
- WHEN it exceeds the cap
- THEN it holds two decisions and becomes two records

Verify: `pre-commit run adr-word-cap --all-files`

## Unenforced

No command decides these rules, or a command decides only part of each. A reviewer confirms the rest.

| Rule                                     | Reviewer confirms                                                                                                            |
| ---------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `decision-records:record-is-not-revised` | The change under review edits no accepted record to describe a later design. A repaired path or a typo fix is not a revision |
