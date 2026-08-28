# Decision Records Specification

## Purpose

Rules governing decision records under `_docs/decisions/`. Covers naming, permanence, and size. What a record's body contains is covered by the record template; the rules a record's decision enforces live in whichever spec owns them.

## Requirements

### `decision-records:filename-carries-no-digit` — A decision record filename carries no digit

When an author creates a decision record, the author MUST name it `ADR-<slug>.md`.

#### Scenario: Two agents create a record in parallel

- GIVEN two worktrees each adding a record
- WHEN both allocate the next sequential number
- THEN two records claim one identity, which a slug name makes impossible

Verify: `pre-commit run adr-filename-shape --all-files`

### `decision-records:a-citation-resolves-to-a-rule` — A citation resolves to a rule

Where a record names what enforces it, the author MUST cite rule IDs the specs still define.

#### Scenario: A rule moves to a domain of its own

- GIVEN a record whose `Enforced by` line cites `distribution:license-declares-both-halves`
- WHEN the rule is renamed to `release:license-declares-both-halves`
- THEN the citation resolves to nothing and the gate reports it, because the pointer to what binds is the one part of a frozen record that must keep working

Verify: `pre-commit run adr-cites-a-live-rule --all-files`

### `decision-records:record-is-not-revised` — A decision record is not revised

The author MUST NOT edit a decision record to describe a later design.

#### Scenario: A rule the record established is later narrowed

- GIVEN a merged record establishing a rule
- WHEN a later change narrows that rule
- THEN the spec carries the narrowed rule and the record keeps its original wording

Verify: `git log --format=%H -- '*/decisions/ADR-*' | head -50 | xargs -I{} git show --stat {}`

### `decision-records:merged-record-is-permanent` — A merged decision record is permanent

The author MUST NOT delete or rename a merged decision record.

#### Scenario: A record's decision is reversed

- GIVEN a record whose choice the project later abandons
- WHEN the successor is written
- THEN the original keeps its filename, gains a status, and links its successor

Verify: `git log --diff-filter=DR --name-only --format= -- '*/decisions/ADR-*' | grep . && exit 1 || exit 0`

### `decision-records:body-stays-within-350-words` — A decision record stays within 350 words

The author MUST keep a filled decision record at or below 350 words.

#### Scenario: A record grows past the cap

- GIVEN a record covering both a storage choice and a migration approach
- WHEN it exceeds the cap
- THEN it holds two decisions and becomes two records

Verify: `pre-commit run adr-word-cap --all-files`
