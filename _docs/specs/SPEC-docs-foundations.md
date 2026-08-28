# Documentation Foundations Specification

## Purpose

The artifact model this project's documentation follows, its precedence order, and where each artifact goes. Covers which artifact owns a fact, what wins when two disagree, and how artifacts are named and placed. The shape of a spec and the shape of a record are covered by their own specs.

## Requirements

### `docs-foundations:spec-states-the-present` — A change to current behavior updates the owning spec

When current behavior changes, the author MUST update the owning spec in the same change.

#### Scenario: A decision record is added without a spec edit

- GIVEN a change that alters how the project behaves
- WHEN the author records the reasoning and leaves the spec untouched
- THEN the spec no longer states the present and the change is incomplete

Verify: reviewer confirms a behavior change in the diff is reflected in a spec

### `docs-foundations:spec-wins-over-record` — A spec outranks a decision record

If a spec and a decision record disagree, then the reader MUST follow the spec.

#### Scenario: A record describes a design the project has moved past

- GIVEN a record stating the project uses one storage engine
- WHEN the spec states it uses another
- THEN the spec is current and the record is history, and neither is edited to agree

Verify: reviewer confirms no record was edited to match a later spec

### `docs-foundations:specs-are-centralized` — Specs are centralized under the docs root

The author MUST place every spec at `<root>/specs/SPEC-<domain>.md`.

#### Scenario: A directory holding governed content is reorganized

- GIVEN a spec placed beside the content it governs
- WHEN the directory is renamed
- THEN the spec governs a path that no longer exists

Verify: `find . -name 'SPEC-*.md' -not -path './.git/*' -not -path './target/*' -not -path './docs/specs/*' -not -path './_docs/specs/*' | grep . && exit 1 || exit 0`

### `docs-foundations:artifact-filenames-carry-a-kind-prefix` — A fixed-kind file carries an uppercase kind prefix

Where this framework fixes a file's kind, the author MUST name it `<KIND>-<slug>.md` with the kind in uppercase.

#### Scenario: A directory holds two kinds of file

- GIVEN a decisions directory holding records and the template that seeds them
- WHEN an agent lists it
- THEN `ADR-` and `TEMPLATE-` separate them without opening either

Verify: `find . \( -path '*/specs/*' -o -path '*/decisions/*' \) -name '*.md' -not -path './.git/*' -not -path './target/*' | rg -v '/(SPEC|ADR|KI|TEMPLATE)-' | grep . && exit 1 || exit 0`

### `docs-foundations:a-kind-prefix-carries-a-slug` — A prefixed filename carries a slug, not a counter

Where a filename carries a kind prefix, the author MUST follow the prefix with the slug that identifies the file rather than an allocated number.

#### Scenario: Two branches each add a record

- GIVEN two branches that each add the next numbered record
- WHEN they merge
- THEN both files claim one identity, which a slug drawn from the subject cannot do

Verify: `find . -name '*-*.md' -not -path './.git/*' -not -path './target/*' | rg '/(ADR|KI)-[0-9]' | grep . && exit 1 || exit 0`

### `docs-foundations:companion-artifacts-share-the-spec-name` — A spec's supporting artifacts sit in a directory named for it

Where a requirement names a supporting artifact, the author MUST place that artifact in `<root>/specs/SPEC-<domain>/`.

#### Scenario: A verification command needs a schema

- GIVEN a requirement whose `Verify:` line validates a file against a JSON Schema
- WHEN the schema has no reader who arrives without the spec
- THEN it sits in the spec's companion directory rather than in reference

Verify: `for d in docs/specs/*/ _docs/specs/*/; do [ -e "$d" ] || continue; n=$(basename "$d"); [ -f "$(dirname "${d%/}")/$n.md" ] && [ -n "$(ls -A "$d")" ] || exit 1; done`
