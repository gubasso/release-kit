# Spec to Code Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`spec-to-code:a-spec-may-lead-its-code` — A spec may lead its code](#spec-to-codea-spec-may-lead-its-code--a-spec-may-lead-its-code)
  - [`spec-to-code:a-spec-change-is-typed` — A spec change is typed](#spec-to-codea-spec-change-is-typed--a-spec-change-is-typed)
  - [`spec-to-code:an-entry-document-cites-rule-ids` — An entry document cites rule IDs](#spec-to-codean-entry-document-cites-rule-ids--an-entry-document-cites-rule-ids)
  - [`spec-to-code:unenacted-rules-are-the-backlog` — Unenacted rules are the backlog](#spec-to-codeunenacted-rules-are-the-backlog--unenacted-rules-are-the-backlog)
  - [`spec-to-code:a-comment-cites-the-rule` — A comment cites the rule it satisfies](#spec-to-codea-comment-cites-the-rule--a-comment-cites-the-rule-it-satisfies)
  - [`spec-to-code:a-gate-message-cites-the-rule` — A gate message cites the rule it enforces](#spec-to-codea-gate-message-cites-the-rule--a-gate-message-cites-the-rule-it-enforces)
  - [`spec-to-code:a-comment-names-no-record` — A comment names no decision record](#spec-to-codea-comment-names-no-record--a-comment-names-no-decision-record)
  - [`spec-to-code:a-suppression-names-its-case` — A suppression names its known-issue case](#spec-to-codea-suppression-names-its-case--a-suppression-names-its-known-issue-case)
  - [`spec-to-code:a-permanent-exception-states-its-reason` — A permanent exception states its reason](#spec-to-codea-permanent-exception-states-its-reason--a-permanent-exception-states-its-reason)

<!--TOC-->

## Purpose

Rules governing the seam between a spec and the work that implements it. Covers requirements written before their behavior exists, how an entry document in the plan zone cites the rules it enacts, and how coverage is derived. The shape of a requirement is covered by the specs specification; how a spec changes is covered by its lifecycle rules.

The plan zone's path is the one value in this specification a project declares for itself, because the planning tool owns the record and this framework names no planning tool. This project keeps its entry documents outside the checkout and declares the zone as `env`, so the environment variable the instance canon names carries the path at run time. Every command here is layout-independent.

Where that variable is unset, the zone resolves to nothing and the typed-clause gate reports nothing. No clone carries the zone, so a reviewer holds the clause shape instead.

## Requirements

### `spec-to-code:a-spec-may-lead-its-code` — A spec may lead its code

Where a requirement's behavior does not yet exist, the author MUST represent that state only by its failing verification command.

#### Scenario: A domain is specified before it is built

- GIVEN a spec merged with three requirements and no implementation
- WHEN a reader runs the three verification commands
- THEN the three failures are the backlog, and no marker in the spec restates them

Verify: `rg -in '^status:' . --glob 'SPEC-*.md' && exit 1 || exit 0`

### `spec-to-code:a-spec-change-is-typed` — A spec change is typed

When an entry document cites a spec change, the author MUST write `ADDED`, `MODIFIED`, or `REMOVED` immediately before the inline-code rule ID.

#### Scenario: A clause names a type but garbles the ID

- GIVEN an entry document carrying `ADDED auth-token-expiry`
- WHEN the shape gate runs
- THEN the clause fails, because the ID token is not `` `<spec-slug>:<rule-slug>` ``

Verify: `pre-commit run spec-change-is-typed --all-files`

### `spec-to-code:an-entry-document-cites-rule-ids` — An entry document cites rule IDs

When a unit of work changes agreed behavior, the author MUST cite each affected rule ID in the work's entry document.

#### Scenario: A diff changes a spec the entry document never names

- GIVEN a change that rewords a requirement
- WHEN the entry document carries no `MODIFIED` clause for its ID
- THEN review rejects the change, because no command can see the omission

Verify: reviewer compares the spec diff against the entry document's typed clauses

### `spec-to-code:unenacted-rules-are-the-backlog` — Unenacted rules are the backlog

The author MUST derive the set of unenacted rules from the specs and the plan zone on every ask.

#### Scenario: Someone proposes a coverage file

- GIVEN a request for a rules-to-work index under `_docs/reference/`
- WHEN the same set is derivable by comparing spec IDs against cited IDs
- THEN the file is refused, because a stored copy drifts on the next change to either side

Verify: reviewer confirms no document stores the agreed-to-enacted mapping

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

Where a suppression masks a defect outside this repository, the author MUST name the `KI-<slug>` case at the suppression.

#### Scenario: A suppression names a case that no record defines

- GIVEN an expected failure whose reason is `KI-vendor-drops-the-body`
- WHEN no record under known-issues carries that name
- THEN the suppression fails, because a mask nobody can look up never gets removed

Verify: `pre-commit run suppression-names-its-case --all-files`

### `spec-to-code:a-permanent-exception-states-its-reason` — A permanent exception states its reason

Where a suppression masks no external defect, the author MUST state its reason at the suppression and MUST NOT name a case. The reason counts in either of two positions. Where the tool that honors the form defines a reason position, the reason written there counts. Where the tool defines none, the reason MUST carry the `sdd: permanent` marker.

Only a position the tool defines counts. Prose near a suppression carries no reason, so a comment above one satisfies this rule only through the marker. The reason reaches no further than the suppression that carries it, and whitespace states nothing.

#### Scenario: A permanent exception is given a case anyway

- GIVEN a suppression over a construct this project chose deliberately and keeps
- WHEN the author writes a record for it to satisfy the case rule
- THEN that record carries a retirement condition nobody can meet, which is the mask the case rule exists to prevent

#### Scenario: A generated artifact states its reason in its own idiom

- GIVEN a workflow another project generates, carrying `# zizmor: ignore[dangerous-triggers] <reason>`
- WHEN this repository must not edit those bytes
- THEN the suppression passes on the tool's own reason, because requiring the marker there would couple this repository to that generator

Verify: `pre-commit run suppression-names-its-case --all-files`
