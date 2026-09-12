# Budget Debt Specification

## Purpose

Rules governing the inherited budget violations a project carries. A project that adopts this convention with documents written before it records each violation in `.spec-driven-docs/debt.yaml`, per gate, per path, per dimension, and the budget gates judge the record instead of the budget. The record can only shrink. The budgets themselves belong to `SPEC-docs-format.md`, `SPEC-docs-specs.md`, and `SPEC-decision-records.md`, and none of them moves for an inherited document.

The file names a gate, a path under it, and one entry per dimension the gate measures. A count dimension carries `ceiling: <count>`. A flag dimension carries `true` and is removed once the condition is corrected.

| Gate                 | Dimension        | Kind  |
| -------------------- | ---------------- | ----- |
| `adr-word-cap`       | `words`          | count |
| `agents-digest-size` | `lines`          | count |
| `chapter-size-cap`   | `lines`          | count |
| `spec-size-cap`      | `authored_lines` | count |
| `spec-size-cap`      | `missing_toc`    | flag  |

```yaml
schema_version: 1

chapter-size-cap:
  method/legacy-deployment.md:
    lines:
      ceiling: 417
```

Nothing delivers the file. Its absence is the empty debt. `sdd debt baseline --apply` creates it from the violations the corpus holds today, `sdd debt migrate --apply` converts the older flat list at `.spec-driven-docs/chapter-size-debt.txt`, and `sdd debt tighten --apply` lowers it to what the documents measure now. Every verb previews without `--apply`. The flat list and the file present together is a failure that names the migration, never a precedence.

## Requirements

### `budget-debt:a-recorded-dimension-only-shrinks` — A recorded dimension only shrinks

Where the debt file records a dimension of a path, the budget gate MUST judge that dimension against the record instead of the budget, and MUST fail the change when the measurement rises above its ceiling or when the record no longer matches what the gate measures.

A measurement above its ceiling is a regression. A measurement below its ceiling, a measurement within the budget, a corrected condition, and a path the gate no longer measures are stale records, and the failure names `sdd debt tighten --apply`. `tighten` lowers a ceiling to the measurement, removes a corrected exception, and never raises or reinstates anything. A stale record that only warned would permit regrowth up to the recorded ceiling.

#### Scenario: A carried chapter shrinks and then grows back

- GIVEN a chapter recorded at a ceiling of 417 lines
- WHEN an author cuts it to 380 lines, commits, and runs `sdd debt tighten --apply` as the failure names
- THEN the first commit fails naming the tightening, the ceiling becomes 380 under the second, and a later edit back to 417 fails as a regression

Verify: `for h in adr-word-cap agents-digest-size chapter-size-cap spec-size-cap; do sdd gate "$h" || exit 1; done`

### `budget-debt:debt-is-created-by-an-explicit-act` — Debt is created by an explicit act

The installer MUST write no debt file, and `sdd debt baseline` MUST refuse where a debt file exists, so a project records its inherited violations once, by its own act, and widens the record by no command afterwards.

A brownfield landing runs `sdd init --apply`, then `sdd debt baseline` to read the inherited violations, then `sdd debt baseline --apply`, then its first commit. The preview is the operator's first view of the corpus, so no failed commit is part of the path. A project that acquires a second inherited corpus after adoption fixes the documents, or tightens what it already recorded. `baseline` mints new exceptions from the current corpus and `migrate` preserves the exemptions a previous operator accepted, and the two stay separate verbs because one verb switching on the state of the filesystem would let a request for a format conversion accept violations instead.

#### Scenario: A second corpus arrives after adoption

- GIVEN a project carrying a debt file, and a merged tree adding forty oversize chapters
- WHEN an operator runs `sdd debt baseline --apply`
- THEN the verb refuses, names the fix and the tightening, and the forty chapters fail their gate until each one fits

Verify: `[ ! -f .spec-driven-docs/debt.yaml ] || ! sdd debt baseline --target .`
