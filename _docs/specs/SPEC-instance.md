# Instance Specification

## Purpose

Rules a project owes the instance installed in it. Covers the installation record the installer writes into the project and every ownership check reads back. Every rule here is one the project itself can violate, and every verification runs with what the install wires. How the installer produces that record, and what the binary owes at install and upgrade time, belong to `SPEC-distribution.md`, which no instance adopts.

## Requirements

### `instance:the-manifest-stays-readable` — The manifest stays readable

The project MUST keep `.spec-driven-docs/manifest.json` present and valid against its schema, because every ownership check reads that record before it can judge anything.

#### Scenario: A manifest is hand-edited until it no longer parses

- GIVEN an installed instance whose manifest is edited by hand
- WHEN the edit leaves the record missing, truncated, or at an older schema version
- THEN the hook fails naming the manifest, because a record no check can read disables every ownership check at once

Verify: `pre-commit run instance-manifest --all-files`
