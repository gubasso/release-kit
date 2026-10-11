# Spec to Code Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`spec-to-code:a-spec-may-lead-its-code` — A spec may lead its code](#spec-to-codea-spec-may-lead-its-code--a-spec-may-lead-its-code)
  - [`spec-to-code:a-comment-cites-the-rule` — A comment cites the rule it satisfies](#spec-to-codea-comment-cites-the-rule--a-comment-cites-the-rule-it-satisfies)
  - [`spec-to-code:a-gate-message-cites-the-rule` — A gate message cites the rule it enforces](#spec-to-codea-gate-message-cites-the-rule--a-gate-message-cites-the-rule-it-enforces)
  - [`spec-to-code:a-comment-names-no-record` — A comment names no decision record](#spec-to-codea-comment-names-no-record--a-comment-names-no-decision-record)
  - [`spec-to-code:a-suppression-names-its-case` — A suppression names its known-issue case](#spec-to-codea-suppression-names-its-case--a-suppression-names-its-known-issue-case)

<!--TOC-->

## Purpose

Rules governing the traceability between a spec and the code that implements it. Covers requirements written before their behavior exists, how code and gates cite the rules they satisfy, and how coverage is derived. The shape of a requirement is covered by the specs specification. How a spec changes is covered by its lifecycle rules.

## Requirements

### `spec-to-code:a-spec-may-lead-its-code` — A spec may lead its code

Where a requirement's behavior does not yet exist, the author MUST represent that state only by its failing verification command.

#### Scenario: A domain is specified before it is built

- GIVEN a spec merged with three requirements and no implementation
- WHEN a reader runs the three verification commands
- THEN the three failures are the backlog, and no marker in the spec restates them

Verify: `rg -in '^status:' . --glob 'SPEC-*.md' && exit 1 || exit 0`

### `spec-to-code:a-comment-cites-the-rule` — A comment cites the rule it satisfies

Where a comment cites an agreement, the author MUST write `SATISFIES` or `VERIFIES` followed by the rule ID.

#### Scenario: A comment cites a rule that no spec defines

- GIVEN a comment carrying `SATISFIES auth:token-expiry-is-bounded`
- WHEN no spec defines that ID
- THEN the citation fails, because a citation that resolves to nothing is a fabrication

Verify: `rg -oI "(SATISFIES|VERIFIES) [a-z0-9-]+:[a-z0-9-]+" . --glob '!{docs,_docs,method}/**' | rg -o "[a-z0-9-]+:[a-z0-9-]+" | sort -u > /tmp/c; rg -oI "^### .[a-z0-9-]+:[a-z0-9-]+." . --glob 'SPEC-*.md' | rg -o "[a-z0-9-]+:[a-z0-9-]+" | sort -u > /tmp/a; comm -13 /tmp/a /tmp/c | grep . && exit 1 || exit 0`

### `spec-to-code:a-gate-message-cites-the-rule` — A gate message cites the rule it enforces

The author MUST make every rule ID a gate prints resolve to a requirement in a spec.

#### Scenario: A rule is renamed and its gate is not

- GIVEN a requirement whose ID changes during review
- WHEN the gate keeps printing the old ID
- THEN the failure message addresses nothing, and the gate reports the unresolved ID

Verify: `pre-commit run gate-message-cites-a-rule --all-files`

### `spec-to-code:a-comment-names-no-record` — A comment names no decision record

The author MUST cite an agreement in code by its rule ID rather than by naming a decision record.

#### Scenario: A branch exists because of a recorded decision

- GIVEN code whose shape was argued for in a decision record
- WHEN the author wants the reason discoverable from the code
- THEN the comment carries the rule ID the record enforces, because the record is frozen and the rule is what binds

Verify: `rg -n "^[[:space:]]*(#|//).*\bADR-[a-z0-9]" . --type-not md && exit 1 || exit 0`

### `spec-to-code:a-suppression-names-its-case` — A suppression names its known-issue case

Where a `KI-<slug>` case is cited outside the documentation root, the author MUST make that case resolve to a known-issue record.

The citation is the subject, not the suppression that carries it. This convention defines the `KI-` token and owns the records it resolves against. It defines no suppression syntax, so no gate here reads one. A suppression that names no case states its reason where its own linter reads one. `spec-to-code.md` names the lints that enforce that reason, and the case where a language has none.

#### Scenario: A record is deleted while a suppression still cites it

- GIVEN an expected failure whose reason is `KI-vendor-drops-the-body`
- WHEN a commit deletes the record of that name
- THEN the citation fails, because a mask nobody can look up never gets removed

Verify: `pre-commit run suppression-names-its-case --all-files`
