# Comparison Documents Specification

## Purpose

Rules governing a comparison document — the table-shaped document that decides between candidates. Covers the verdict vocabulary, the legend, dating, references, and pipe escaping. The prose around a table and the size budget it sits under belong to `SPEC-docs-format.md`.

## Requirements

### `comparison-docs:a-verdict-carries-its-word` — A verdict carries its word

The author MUST write every verdict as its symbol followed by its word, from the legend's fixed set.

#### Scenario: A row carries one annotated verdict and one bare one

- GIVEN a row whose first cell reads `✅ yes` and whose second reads `❌`
- WHEN a reader scans the column
- THEN the bare symbol is guessed at, and the gate rejects the row

Verify: `pre-commit run comparison-verdict-word --all-files`

### `comparison-docs:a-comparison-carries-a-legend` — A comparison carries a legend

The author MUST give every document containing a comparison table a `Legend:` line.

#### Scenario: A table arrives with symbols and no key

- GIVEN a table using six verdict symbols
- WHEN no legend defines them
- THEN each reader assigns their own meaning, and the gate rejects the document

Verify: `pre-commit run comparison-legend --all-files`

### `comparison-docs:every-table-is-dated` — Every table is dated

The author MUST give every document containing a comparison table a `Verified: <YYYY-MM-DD>` line.

#### Scenario: A verdict ages out

- GIVEN a table stating a candidate lacks a feature
- WHEN the candidate ships it a release later
- THEN nothing tells a reader the table predates the release, and the gate rejects the document

Verify: `pre-commit run comparison-dated-tables --all-files`

### `comparison-docs:a-cell-carries-one-reference` — A cell carries one reference

The author MUST keep a comparison cell to at most one reference link.

#### Scenario: A cell accumulates evidence

- GIVEN a verdict supported by three sources
- WHEN all three are linked in the cell
- THEN the column stops being scannable, and the evidence belongs in the scenario below the table

Verify: `pre-commit run comparison-one-reference-per-cell --all-files`

### `comparison-docs:table-pipes-are-escaped` — Table pipes are escaped

The author MUST escape a literal pipe inside inline code in a table row.

#### Scenario: A cell quotes a shell pipeline

- GIVEN a cell holding `` `a | b` ``
- WHEN the renderer splits the row on the pipe
- THEN the table gains a phantom column, and the gate rejects the row

Verify: `pre-commit run comparison-escaped-pipes --all-files`
