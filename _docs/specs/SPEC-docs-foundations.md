# Documentation Foundations Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`docs-foundations:spec-states-the-present` — A change to current behavior updates the owning spec](#docs-foundationsspec-states-the-present--a-change-to-current-behavior-updates-the-owning-spec)
  - [`docs-foundations:spec-wins-over-record` — A spec outranks a decision record](#docs-foundationsspec-wins-over-record--a-spec-outranks-a-decision-record)
  - [`docs-foundations:a-document-carries-no-personal-path` — A document carries no path into a person's home](#docs-foundationsa-document-carries-no-personal-path--a-document-carries-no-path-into-a-persons-home)
  - [`docs-foundations:a-document-owns-what-it-governs` — A document owns what it governs](#docs-foundationsa-document-owns-what-it-governs--a-document-owns-what-it-governs)
  - [`docs-foundations:specs-are-centralized` — Specs are centralized under the docs root](#docs-foundationsspecs-are-centralized--specs-are-centralized-under-the-docs-root)
  - [`docs-foundations:artifact-filenames-carry-a-kind-prefix` — A fixed-kind file carries an uppercase kind prefix](#docs-foundationsartifact-filenames-carry-a-kind-prefix--a-fixed-kind-file-carries-an-uppercase-kind-prefix)
  - [`docs-foundations:a-kind-prefix-carries-a-slug` — A prefixed filename carries a slug, not a counter](#docs-foundationsa-kind-prefix-carries-a-slug--a-prefixed-filename-carries-a-slug-not-a-counter)
  - [`docs-foundations:companion-artifacts-share-the-spec-name` — A spec's supporting artifacts sit in a directory named for it](#docs-foundationscompanion-artifacts-share-the-spec-name--a-specs-supporting-artifacts-sit-in-a-directory-named-for-it)

<!--TOC-->

## Purpose

The artifact model this project's documentation follows, its precedence order, and where each artifact goes. Covers which artifact owns a fact, what wins when two disagree, how artifacts are named and placed, and what a document may not carry out of the machine it was written on. The shape of a spec and the shape of a record are covered by their own specs.

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

### `docs-foundations:a-document-carries-no-personal-path` — A document carries no path into a person's home

The author MUST write a path into a home directory as `~/`, `$HOME/`, or a bracketed placeholder rather than as an absolute path naming a person, except in a file whose purpose is one person's environment.

#### Scenario: A walkthrough is written from the author's own terminal

- GIVEN a chapter that pastes a working command with the author's home directory in it
- WHEN a second reader follows it
- THEN the path resolves for nobody else and names someone who never agreed to be named, which the placeholder form avoids at no cost

Verify: `pre-commit run no-personal-path --all-files`

### `docs-foundations:a-document-owns-what-it-governs` — A document owns what it governs

Where a document states a rule this project's own domain owns, the author MUST state it here rather than sending the reader to another project's documentation for it.

#### Scenario: A rule is left to the project it was borrowed from

- GIVEN a convention this project requires and another project happens to document
- WHEN the spec points at that project instead of stating the rule
- THEN the requirement changes when someone else edits it, and a reader without access to that project cannot learn what binds them

Verify: reviewer confirms every rule the project owns is stated in the project's own documents

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
