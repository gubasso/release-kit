# Placement Specification

## Purpose

How a destination owned by a third-party application is chosen, anywhere this repository or its payload names one: an agent's skill root, an editor directory, a forge's workflow directory. The decision is made inside each project, case by case, against the owning application's current documentation — never inferred from a convention this repository follows, and never generalized from one application to another. The boundary against the neighbors: `SPEC-distribution.md` binds what the binary writes at user scope, and `SPEC-landing.md` binds how files land into a target; this spec binds how any such destination earns its name.

## Requirements

### `placement:a-third-party-destination-names-its-source` — A third-party destination names its source

A rule or document naming a destination owned by a third-party application MUST cite that application's own documentation with a dated entry in `_docs/reference/`, because vendor layouts differ and move, and a destination inferred from this repository's conventions or generalized from another application is folklore.

#### Scenario: A destination is named without its source

- GIVEN a payload destination under a third-party application's directory
- WHEN the test suite runs
- THEN the test fails unless `_docs/reference/` records that destination with a dated citation

Verify: `cargo nextest run -E 'test(every_third_party_destination_names_its_source)'`

### `placement:a-placement-is-reverified-when-relied-on` — A placement is re-verified when relied on

The author MUST re-verify a recorded third-party destination against the owning application's documentation before new work relies on it, updating the reference entry's date, because vendors move these paths and a citation is a snapshot rather than a guarantee.

#### Scenario: A later change builds on a recorded destination

- GIVEN a change that lands new files under a destination `_docs/reference/` already records
- WHEN the change is reviewed
- THEN the reference entry carries a verification date the change refreshed or confirmed

Verify: reviewer confirms the reference entry's date against the change
