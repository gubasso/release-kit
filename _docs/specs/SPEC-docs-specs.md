# Specifications Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`docs-specs:requirement-carries-five-parts` — A requirement carries five parts](#docs-specsrequirement-carries-five-parts--a-requirement-carries-five-parts)
  - [`docs-specs:statement-uses-an-ears-pattern` — A statement uses one EARS pattern](#docs-specsstatement-uses-an-ears-pattern--a-statement-uses-one-ears-pattern)
  - [`docs-specs:rule-id-is-unique-and-slugged` — A rule ID is a slug pair and is unique](#docs-specsrule-id-is-unique-and-slugged--a-rule-id-is-a-slug-pair-and-is-unique)
  - [`docs-specs:rule-id-outlives-its-sentence` — A rule ID survives rewording](#docs-specsrule-id-outlives-its-sentence--a-rule-id-survives-rewording)
  - [`docs-specs:requirement-carries-a-verification` — A requirement carries a verification](#docs-specsrequirement-carries-a-verification--a-requirement-carries-a-verification)
  - [`docs-specs:spec-stays-within-300-lines` — A spec stays within 300 lines](#docs-specsspec-stays-within-300-lines--a-spec-stays-within-300-lines)
  - [`docs-specs:verification-names-a-live-hook` — A verification names a live hook](#docs-specsverification-names-a-live-hook--a-verification-names-a-live-hook)
  - [`docs-specs:prohibitions-are-capped` — Prohibitions are capped and paired](#docs-specsprohibitions-are-capped--prohibitions-are-capped-and-paired)
  - [`docs-specs:unenforced-rules-are-declared` — An unenforced rule is declared](#docs-specsunenforced-rules-are-declared--an-unenforced-rule-is-declared)

<!--TOC-->

## Purpose

Rules governing specification files under `_docs/specs/`. Covers the requirement block, its grammar, its identifier, and its verification. Where a spec is placed and how it is named belong to `SPEC-docs-foundations.md`; the markdown a spec is written in belongs to `SPEC-docs-format.md`.

## Requirements

### `docs-specs:requirement-carries-five-parts` — A requirement carries five parts

The author MUST give every requirement a title, a rule ID, a statement, a scenario, and a verification line.

#### Scenario: A requirement is added during a rushed change

- GIVEN a new rule the project wants to bind
- WHEN the author writes a title and a statement only
- THEN the rule cannot be cited or checked, and the gate rejects the spec

Verify: ``for f in docs/specs/SPEC-*.md _docs/specs/SPEC-*.md; do [ -e "$f" ] || continue; r=$(grep -cE '^### `[a-z0-9-]+:[a-z0-9-]+` . ' "$f"); h=$(grep -c '^### ' "$f"); v=$(grep -c '^Verify: ' "$f"); [ "$r" = "$h" ] && [ "$r" = "$v" ] || exit 1; done``

### `docs-specs:statement-uses-an-ears-pattern` — A statement uses one EARS pattern

The author MUST write every requirement statement as one sentence in an EARS pattern carrying an RFC 2119 keyword.

#### Scenario: A preference is written as a requirement

- GIVEN an author who prefers short records
- WHEN they write "records should be kept short"
- THEN the statement names no actor and no threshold, and the gate rejects it

Verify: ``rg -UIo -r '$1' '^### `[a-z0-9-]+:[a-z0-9-]+`[^\n]*\n\n([^\n]+)' . --glob 'SPEC-*.md' | rg -v '(MUST|SHALL|SHOULD|MAY|REQUIRED)' | grep . && exit 1 || exit 0``

### `docs-specs:rule-id-is-unique-and-slugged` — A rule ID is a slug pair and is unique

The author MUST identify every requirement as `<spec-slug>:<rule-slug>`, unique across the project.

#### Scenario: Two worktrees add a rule about the same subject

- GIVEN two branches each adding a requirement
- WHEN both choose the same rule slug
- THEN the duplicate is a real conflict about one subject, and the gate reports it

Verify: `pre-commit run spec-rule-id-unique --all-files`

### `docs-specs:rule-id-outlives-its-sentence` — A rule ID survives rewording

Where a requirement statement is rewritten, the author MUST keep its existing rule ID.

#### Scenario: A statement is clarified after review

- GIVEN a rule cited by commits and review comments
- WHEN its sentence is rewritten for clarity
- THEN the ID is unchanged and every existing citation still resolves

Verify: reviewer confirms no rule ID changed alongside a reworded statement

### `docs-specs:requirement-carries-a-verification` — A requirement carries a verification

The author MUST give every requirement a `Verify:` line that exits non-zero when the rule is violated, or names the human procedure that decides it.

#### Scenario: A rule no command can decide

- GIVEN a rule requiring that a scenario names the contested case
- WHEN no command can judge it
- THEN the `Verify:` line names the reviewer procedure and the rule is listed as unenforced

Verify: reviewer confirms each requirement's `Verify:` line names a command or a named human procedure

### `docs-specs:spec-stays-within-300-lines` — A spec stays within 300 lines

The author MUST keep a spec at or below 300 authored lines, and MUST give one carrying more than 100 a generated table of contents.

#### Scenario: A domain accumulates requirements

- GIVEN a spec approaching the cap
- WHEN another requirement arrives
- THEN the domain has split, and the excess becomes a spec of its own rather than a longer file

Verify: `pre-commit run spec-size-cap --all-files`

### `docs-specs:verification-names-a-live-hook` — A verification names a live hook

Where a requirement's verification runs a hook, the author MUST name a hook the project still defines.

#### Scenario: A hook is renamed

- GIVEN a rule whose `Verify:` line runs `pre-commit run chapter-size-cap`
- WHEN the hook is renamed and the spec is not
- THEN the rule is enforced by nothing while still reading as gated, and the gate reports it

Verify: `pre-commit run spec-verify-hooks-exist --all-files`

### `docs-specs:prohibitions-are-capped` — Prohibitions are capped and paired

The author MUST keep a spec at or below five prohibitions, each paired with the action that replaces it.

#### Scenario: A spec accumulates prohibitions

- GIVEN a spec stating eight things not to do
- WHEN an agent applies it
- THEN some prohibitions are dropped unpredictably, and the gate rejects the spec

Verify: `for f in docs/specs/SPEC-*.md _docs/specs/SPEC-*.md; do [ -e "$f" ] || continue; n=$(rg -c 'MUST NOT|SHALL NOT' "$f" || echo 0); [ "$n" -le 5 ] || exit 1; done`

### `docs-specs:unenforced-rules-are-declared` — An unenforced rule is declared

Where no command can decide a rule, the author MUST list it as unenforced.

#### Scenario: A judgment rule is written as if gated

- GIVEN a rule requiring one term per concept
- WHEN no command can identify synonyms
- THEN the rule is listed as unenforced and asked at review instead

Verify: reviewer confirms each rule without a machine-checkable command appears in the unenforced table
