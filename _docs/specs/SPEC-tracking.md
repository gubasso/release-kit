# Tracking Registry Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`tracking:the-registry-has-one-readable-shape` — The registry has one readable shape](#trackingthe-registry-has-one-readable-shape--the-registry-has-one-readable-shape)
  - [`tracking:a-perishable-source-is-registered` — A perishable source is registered](#trackinga-perishable-source-is-registered--a-perishable-source-is-registered)
  - [`tracking:an-upstream-derivation-pins-a-revision` — An upstream derivation pins a revision](#trackingan-upstream-derivation-pins-a-revision--an-upstream-derivation-pins-a-revision)
  - [`tracking:an-entry-declares-how-to-revalidate` — An entry declares how to revalidate](#trackingan-entry-declares-how-to-revalidate--an-entry-declares-how-to-revalidate)
  - [`tracking:an-overdue-entry-blocks` — An overdue entry blocks](#trackingan-overdue-entry-blocks--an-overdue-entry-blocks)
  - [`tracking:a-declared-dependent-exists` — A declared dependent exists](#trackinga-declared-dependent-exists--a-declared-dependent-exists)
  - [`tracking:an-upstream-check-does-not-edit-the-tree` — An upstream check does not edit the tree](#trackingan-upstream-check-does-not-edit-the-tree--an-upstream-check-does-not-edit-the-tree)
  - [`tracking:the-validator-checks-what-the-schema-cannot` — The validator checks what the schema cannot](#trackingthe-validator-checks-what-the-schema-cannot--the-validator-checks-what-the-schema-cannot)
- [Unenforced](#unenforced)

<!--TOC-->

## Purpose

Rules governing the tracking registry, the machine-readable record of facts that expire. The registry lives at `<root>/reference/tracking.yaml`. An entry's `path` and `dependents` are relative to the repository root, not to the documentation root. It records each perishable source: when it was last checked, how often to check it, how to revalidate it, and its dependents. A source derived from an upstream Git repository also pins the exact revision the local copy came from. The registry never becomes a second copy of the fact it tracks. This spec owns the registry's shape, its freshness gate, and the split between offline freshness and an explicit network check. The network command that compares a pinned revision to its upstream is `sdd track check`. The lifecycle reasons behind perishable facts belong to the method.

## Requirements

### `tracking:the-registry-has-one-readable-shape` — The registry has one readable shape

The registry MUST be one YAML document with `schema_version: 1` and a `tracked` array, and the parser MUST reject anything past its shape or bounds.

#### Scenario: A registry carries an alias to fan out one entry

- GIVEN a registry file using a YAML alias to repeat a mapping
- WHEN the gate parses it
- THEN it fails, because untrusted input is bounded before any semantic read

Verify: `pre-commit run tracking-registry --all-files`

### `tracking:a-perishable-source-is-registered` — A perishable source is registered

The author MUST give every document holding an expiring external fact one registry entry carrying its id, path, last_checked, cadence_days, why, revalidate steps, and dependents.

#### Scenario: A pricing page has no entry

- GIVEN a reference page holding vendor prices
- WHEN it carries no registry entry
- THEN the review adds one, because a fact that expires with no timer ages silently

Verify: reviewer confirms every document holding an expiring external fact has a registry entry

### `tracking:an-upstream-derivation-pins-a-revision` — An upstream derivation pins a revision

Where an entry derives from a Git upstream, the author MUST record a `source` object pinning the repository, reference, full object ID, and license.

#### Scenario: An entry names a branch instead of a commit

- GIVEN an entry whose `source.revision` is a branch name
- WHEN the gate validates it
- THEN it fails, because a moving reference cannot say which bytes the local copy holds

Verify: `pre-commit run tracking-registry --all-files`

### `tracking:an-entry-declares-how-to-revalidate` — An entry declares how to revalidate

The author MUST give every entry ordered `revalidate` steps that a person or an agent follows to confirm the fact against its authoritative source.

#### Scenario: An overdue entry names no recovery

- GIVEN an entry past its cadence
- WHEN a reader opens it to act
- THEN the `revalidate` steps say what to re-fetch and what to update before the date advances

Verify: reviewer confirms every entry lists ordered revalidation steps

### `tracking:an-overdue-entry-blocks` — An overdue entry blocks

If an entry's `last_checked` plus `cadence_days` is before today, then the gate MUST fail, naming the due date and the entry's revalidation steps.

#### Scenario: A 30-day entry was last checked 40 days ago

- GIVEN an entry with `cadence_days: 30` last checked 40 days ago
- WHEN the freshness gate runs
- THEN it fails naming the exact due date and the recovery steps, and it reads current bytes only

Verify: `pre-commit run tracking-registry --all-files`

### `tracking:a-declared-dependent-exists` — A declared dependent exists

The gate MUST resolve an entry's `path` and every `dependent` from the repository root, and MUST fail where one names a file that is absent or escapes the tree.

#### Scenario: A dependent path is renamed and the entry is not

- GIVEN an entry whose dependent file was renamed
- WHEN the gate resolves the dependents
- THEN it fails naming the missing path, because a dependent that does not exist tracks nothing

Verify: `pre-commit run tracking-registry --all-files`

### `tracking:an-upstream-check-does-not-edit-the-tree` — An upstream check does not edit the tree

The `sdd track check` command MUST compare a pinned revision to its upstream without writing any file, and `sdd track status` MUST report freshness with no network.

#### Scenario: An upstream moved since the pinned revision

- GIVEN a tracked Git source whose upstream advanced past `source.revision`
- WHEN `sdd track check` runs
- THEN it reports the observed revision and leaves every file byte-identical, because a person decides whether to accept the move

Verify: reviewer confirms `sdd track check` leaves the target tree byte-identical

### `tracking:the-validator-checks-what-the-schema-cannot` — The validator checks what the schema cannot

The validator MUST reject a duplicate entry ID, a duplicate dependent, a `path` or dependent that is absolute, escapes the tree, or is reached through a symlink, a `source` whose fields disagree with its kind, and a declared file that does not exist.

The companion schema at `SPEC-tracking/tracking.schema.json` holds the structural shape alone, and its own description says what that covers. A schema cannot read the filesystem or compare one field to another, so this spec claims no more of it.

#### Scenario: A schema-valid registry declares a path outside the tree

- GIVEN a registry whose entry lists the dependent `../outside.md`, which the schema accepts
- WHEN the gate runs
- THEN it fails naming the entry and the escape, because only the validator reads where a path leads

Verify: `pre-commit run tracking-registry --all-files`

## Unenforced

No command decides these rules, or a command decides only part of each. A reviewer confirms the rest.

| Rule                                                | Reviewer confirms                                           |
| --------------------------------------------------- | ----------------------------------------------------------- |
| `tracking:a-perishable-source-is-registered`        | Each perishable source the change adds has a registry entry |
| `tracking:an-entry-declares-how-to-revalidate`      | Each entry's steps let a person revalidate the source       |
| `tracking:an-upstream-check-does-not-edit-the-tree` | The upstream check left the working tree unchanged          |
