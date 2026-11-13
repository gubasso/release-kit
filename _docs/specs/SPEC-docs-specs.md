# Specifications Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`docs-specs:requirement-carries-five-parts` — A requirement carries five parts](#docs-specsrequirement-carries-five-parts--a-requirement-carries-five-parts)
  - [`docs-specs:statement-uses-an-ears-pattern` — A statement uses one EARS pattern](#docs-specsstatement-uses-an-ears-pattern--a-statement-uses-one-ears-pattern)
  - [`docs-specs:a-spec-follows-the-heading-shape` — A spec follows the heading shape](#docs-specsa-spec-follows-the-heading-shape--a-spec-follows-the-heading-shape)
  - [`docs-specs:rule-id-is-unique-and-slugged` — A rule ID is a slug pair and is unique](#docs-specsrule-id-is-unique-and-slugged--a-rule-id-is-a-slug-pair-and-is-unique)
  - [`docs-specs:rule-id-outlives-its-sentence` — A rule ID survives rewording](#docs-specsrule-id-outlives-its-sentence--a-rule-id-survives-rewording)
  - [`docs-specs:requirement-carries-a-verification` — A requirement carries a verification](#docs-specsrequirement-carries-a-verification--a-requirement-carries-a-verification)
  - [`docs-specs:spec-stays-within-300-lines` — A spec stays within 300 lines](#docs-specsspec-stays-within-300-lines--a-spec-stays-within-300-lines)
  - [`docs-specs:verification-names-a-live-hook` — A verification names a live hook](#docs-specsverification-names-a-live-hook--a-verification-names-a-live-hook)
  - [`docs-specs:prohibitions-are-capped` — Prohibitions are capped and paired](#docs-specsprohibitions-are-capped--prohibitions-are-capped-and-paired)
  - [`docs-specs:unenforced-rules-are-declared` — An unenforced rule is declared](#docs-specsunenforced-rules-are-declared--an-unenforced-rule-is-declared)
- [Unenforced](#unenforced)

<!--TOC-->

## Purpose

Rules governing specification files under `<root>/specs/`. Covers the requirement block, its grammar, its identifier, and its verification. Where a spec is placed and how it is named belong to `SPEC-docs-foundations.md`. The markdown a spec is written in belongs to `SPEC-docs-format.md`.

## Requirements

### `docs-specs:requirement-carries-five-parts` — A requirement carries five parts

The author MUST give every requirement a title, a rule ID, a statement, a scenario, and a verification line.

#### Scenario: A requirement is added during a rushed change

- GIVEN a new rule the project wants to bind
- WHEN the author writes a title and a statement only
- THEN the rule cannot be cited or checked, and the gate rejects the spec

Verify: ``for f in docs/specs/SPEC-*.md _docs/specs/SPEC-*.md; do [ -e "$f" ] || continue; r=$(grep -cE '^### `[a-z0-9-]+:[a-z0-9-]+` . ' "$f"); h=$(grep -c '^### ' "$f"); v=$(grep -c '^Verify: ' "$f"); [ "$r" = "$h" ] && [ "$r" = "$v" ] || exit 1; done``, and a reviewer confirms that each scenario names the contested case

### `docs-specs:statement-uses-an-ears-pattern` — A statement uses one EARS pattern

The author MUST write every requirement statement as one sentence in an EARS pattern carrying an RFC 2119 keyword.

#### Scenario: A preference is written as a requirement

- GIVEN an author who prefers short records
- WHEN they write "records are best kept short"
- THEN the statement names no actor and no threshold, and the gate rejects it

Verify: ``rg -UIo -r '$1' '^### `[a-z0-9-]+:[a-z0-9-]+`[^\n]*\n\n([^\n]+)' . --glob 'SPEC-*.md' | rg -v '(MUST|SHOULD|MAY)' | grep . && exit 1 || exit 0``, and a reviewer confirms that the statement is one sentence naming a subject that can act

### `docs-specs:a-spec-follows-the-heading-shape` — A spec follows the heading shape

The author MUST give every `SPEC-*.md` one title, then `## Purpose`, then `## Requirements` with at least one requirement, and after the requirements at most one further level-two heading, `## Unenforced`, each written in the case shown here.

The documentation root's managed lint configuration holds the order up to the requirements as `["?", "## Purpose", "## Requirements", "+"]` with case matched. The trailing `+` covers the requirement and scenario headings, so a spec with no requirement fails, but it admits any later heading too. The `spec-trailing-section` gate holds what follows the requirements: one named section is the whole allowance, and a diagnostic or a background section belongs in reference or explanation.

#### Scenario: A spec gains a section after its requirements

- GIVEN a spec whose author adds `## Background` after its last requirement
- WHEN the hooks run over the spec
- THEN `spec-trailing-section` fails naming the heading and where it moves, because the lint shape alone admits it

Verify: `pre-commit run markdownlint-cli2 --all-files && pre-commit run spec-trailing-section --all-files`

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

A ceiling or an exception the project records for an inherited spec under `SPEC-budget-debt.md` is judged instead, and it only comes down.

#### Scenario: A domain accumulates requirements

- GIVEN a spec approaching the cap
- WHEN another requirement arrives
- THEN the domain has split, and the excess becomes a spec of its own rather than a longer file

Verify: `pre-commit run spec-size-cap --all-files`

### `docs-specs:verification-names-a-live-hook` — A verification names a live hook

Where a requirement's verification runs a hook, the author MUST name a hook the project still defines. A verification runs a hook as `prek run <hook>` or as `pre-commit run <hook>`, and a new verification uses `prek run <hook>`.

#### Scenario: A hook is renamed

- GIVEN a rule whose `Verify:` line runs `prek run chapter-size-cap`
- WHEN the hook is renamed and the spec is not
- THEN the rule is enforced by nothing while still reading as gated, and the gate reports it

Verify: `pre-commit run spec-verify-hooks-exist --all-files`

### `docs-specs:prohibitions-are-capped` — Prohibitions are capped and paired

The author MUST keep a spec at or below five prohibitions, each paired with the action that replaces it.

#### Scenario: A spec accumulates prohibitions

- GIVEN a spec stating eight things not to do
- WHEN an agent applies it
- THEN some prohibitions are dropped unpredictably, and the gate rejects the spec

Verify: `for f in docs/specs/SPEC-*.md _docs/specs/SPEC-*.md; do [ -e "$f" ] || continue; n=$(rg -c 'MUST NOT' "$f" || echo 0); [ "$n" -le 5 ] || exit 1; done`

### `docs-specs:unenforced-rules-are-declared` — An unenforced rule is declared

Where no command decides a rule, or a part of it, the author MUST list that rule in the `## Unenforced` table of the spec that states it, with the columns `Rule` and `Reviewer confirms`.

Each row names one rule of its own spec as inline code, and its second cell says what a reviewer confirms. The list sits beside the rules it qualifies, so no unenforced rule appears in two lists.

#### Scenario: A judgment rule is written as if gated

- GIVEN a rule requiring one term per concept
- WHEN no command can identify synonyms
- THEN the rule's own spec lists it under `## Unenforced`, and the review asks it

Verify: reviewer confirms each rule whose `Verify:` line names a reviewer appears in the `## Unenforced` table of its own spec

## Unenforced

No command decides these rules, or a command decides only part of each. A reviewer confirms the rest.

| Rule                                            | Reviewer confirms                                                                     |
| ----------------------------------------------- | ------------------------------------------------------------------------------------- |
| `docs-specs:requirement-carries-five-parts`     | Each scenario names the contested case, not a restatement                             |
| `docs-specs:statement-uses-an-ears-pattern`     | The statement is one sentence and names a subject that can act                        |
| `docs-specs:rule-id-outlives-its-sentence`      | No rule ID changed alongside a reworded statement                                     |
| `docs-specs:requirement-carries-a-verification` | Each `Verify:` line names a command or a named human procedure                        |
| `docs-specs:unenforced-rules-are-declared`      | Each rule whose `Verify:` line names a reviewer appears in this table of its own spec |
