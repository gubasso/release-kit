# Instance Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`instance:the-manifest-stays-readable` — The manifest stays readable](#instancethe-manifest-stays-readable--the-manifest-stays-readable)
  - [`instance:the-agents-block-stays-managed` — The managed AGENTS block stays intact](#instancethe-agents-block-stays-managed--the-managed-agents-block-stays-intact)
  - [`instance:the-tracking-registry-stays-valid` — The tracking registry stays valid](#instancethe-tracking-registry-stays-valid--the-tracking-registry-stays-valid)
  - [`instance:the-lint-configuration-composes` — The lint configuration composes](#instancethe-lint-configuration-composes--the-lint-configuration-composes)
  - [`instance:the-root-lint-configuration-loads-relative-links` — The root lint configuration loads relative-links](#instancethe-root-lint-configuration-loads-relative-links--the-root-lint-configuration-loads-relative-links)
  - [`instance:the-project-declares-what-its-gates-judge` — The project declares what its gates judge](#instancethe-project-declares-what-its-gates-judge--the-project-declares-what-its-gates-judge)
  - [`instance:the-managed-block-agrees-with-the-declaration` — The managed block agrees with the declaration](#instancethe-managed-block-agrees-with-the-declaration--the-managed-block-agrees-with-the-declaration)

<!--TOC-->

## Purpose

Rules a project owes the instance installed in it. Covers the installation record the installer writes into the project, and the managed regions the installer keeps inside project-owned files. It also covers the tracking registry the project owns, and every ownership check that reads these back. Every rule here is one the project itself can violate, and every verification runs with what the install wires. How the installer produces that record, and what the binary owes at install and upgrade time, belong to `SPEC-distribution.md`, which no instance adopts. The writing convention the project follows belongs to `SPEC-writing-style.md` and `sdd method writing-style`. The registry's own shape belongs to `SPEC-tracking.md`.

## Requirements

### `instance:the-manifest-stays-readable` — The manifest stays readable

The project MUST keep `.spec-driven-docs/manifest.json` present and readable, because every ownership check reads that record before it can judge anything.

A record written by an older release is readable: the tool carries a bounded reader for each shape it published, and the next successful landing writes the current one. A record no reader can parse is the failure this rule names.

#### Scenario: A manifest is hand-edited until it no longer parses

- GIVEN an installed instance whose manifest is edited by hand
- WHEN the edit leaves the record missing or truncated
- THEN the hook fails naming the manifest, because a record no check can read disables every ownership check at once

Verify: `pre-commit run instance-manifest --all-files`

### `instance:the-agents-block-stays-managed` — The managed AGENTS block stays intact

The project MUST keep the marked documentation block inside the root `AGENTS.md` intact. The install writes the documentation and writing-style routing into that region, so an edit inside the markers is a conflict.

#### Scenario: An editor rewrites the marked block

- GIVEN a root `AGENTS.md` carrying the managed documentation block
- WHEN an author edits a line between the markers
- THEN `sdd verify` reports the tampered block, because the region belongs to the install and content outside the markers is the project's own

Verify: `sdd verify --target .`

### `instance:the-tracking-registry-stays-valid` — The tracking registry stays valid

The project MUST keep `<root>/reference/tracking.yaml` valid against its schema and free of overdue or dangling entries, because the freshness gate reads it on every commit.

#### Scenario: A tracked entry falls past its cadence

- GIVEN an adopted tracking registry with one entry
- WHEN the entry's cadence elapses without a revalidation
- THEN the gate fails naming the due date and the recovery steps, and the project revalidates the source before advancing the date

Verify: `pre-commit run tracking-registry --all-files`

### `instance:the-lint-configuration-composes` — The lint configuration composes

The project MUST NOT keep a `.markdownlint.*` file at the documentation root, or a markdownlint configuration of either family anywhere beneath `<root>/specs/` or `<root>/decisions/`, and MUST fold the rules it wants into its configuration at the repository root instead.

The documentation root's managed configuration carries the heading shapes, and markdownlint-cli2 lets a configuration in those locations replace it. A configuration at the repository root merges beneath it, so it cannot disable the shapes, and the project keeps it.

Verification counts a configuration link by its own filename, including a dangling link. It does not scan through a linked directory. It fails naming a documentation root, `specs/`, or `decisions/` reached through a link, because the linter reads the configuration beneath that link and no check sees it.

#### Scenario: A project relaxes a rule for its specs

- GIVEN a project that adds `.markdownlint-cli2.jsonc` beside its specs to relax one rule
- WHEN `sdd verify` runs
- THEN it fails naming that path, because the nested file replaces the delivered overrides and no spec there is held to its shape any longer

Verify: `sdd verify --target .`

### `instance:the-root-lint-configuration-loads-relative-links` — The root lint configuration loads relative-links

The project MUST keep `markdownlint-rule-relative-links` loaded through a `.markdownlint-cli2.*` file at the repository root and `relative-links` enabled in the rules that file's directory resolves, because that rule is what fails a relative link whose target does not exist.

A `.markdownlint.*` file at the root replaces the CLI2 file's `config`, so the rule is enabled there where one exists. `sdd lint` names each configuration that does not load or enable the rule, and `sdd lint prove` proves the rule fires.

#### Scenario: A project's own root configuration replaces the seed

- GIVEN a repository root holding its own `.markdownlint-cli2.jsonc` without the rule
- WHEN `sdd lint prove` runs
- THEN it fails, because a broken relative link lints clean and no other check sees the missing rule

Verify: `sdd lint prove`

### `instance:the-project-declares-what-its-gates-judge` — The project declares what its gates judge

The project MAY state, in `.spec-driven-docs/config.yaml`, which paths no delivered gate judges and which filters a named gate takes, and the tool MUST apply that statement to every route a subject path reaches a gate by. A declaration that does not parse, or that names a gate this version does not deliver, MUST fail once and name the key, never fall back to the default.

The declaration scopes the gates `sdd` delivers. The Markdown linter takes its scope from the `ignores` of its own configuration, so a Markdown path it must skip goes there, and `reserved:` does not reach it.

#### Scenario: Another tool owns a region of a file at the project root

- GIVEN a project that records that path under `reserved:`
- WHEN any delivered gate runs, whether pre-commit passes the path or the gate walks to it
- THEN no gate judges the path, and `sdd gate --explain <path>` names `reserved` as the layer that decided

Verify: `sdd gate --explain .spec-driven-docs/config.yaml`

### `instance:the-managed-block-agrees-with-the-declaration` — The managed block agrees with the declaration

The managed pre-commit block is rendered from the declaration, so the project MUST NOT edit the block, and the per-gate wiring in the block MUST match what the declaration renders. `sdd hooks --apply` is what brings a stale block back into agreement, and it works at the same version.

The documentation block in the root author-instructions file carries the writing-style route the declaration selects, so the same verb rewrites it and the same check holds it.

#### Scenario: The declaration is edited and nothing else is run

- GIVEN a project that adds an exclusion to its declaration
- WHEN `sdd verify` runs
- THEN it fails naming the gate whose wiring disagrees and the command that fixes it, because an upgrade returns early at the same version and would never reach the block

Verify: `sdd verify --target .`
