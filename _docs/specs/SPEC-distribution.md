# Distribution Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`distribution:the-payload-roots-are-declared-once` — The payload roots are declared once](#distributionthe-payload-roots-are-declared-once--the-payload-roots-are-declared-once)
  - [`distribution:the-published-crate-carries-every-root` — The published crate carries every root](#distributionthe-published-crate-carries-every-root--the-published-crate-carries-every-root)
  - [`distribution:machine-output-declares-its-schema` — Machine output declares its schema](#distributionmachine-output-declares-its-schema--machine-output-declares-its-schema)
  - [`distribution:skills-are-part-of-the-payload` — Skills are part of the payload](#distributionskills-are-part-of-the-payload--skills-are-part-of-the-payload)
  - [`distribution:a-skill-has-one-owner` — A skill has one owner](#distributiona-skill-has-one-owner--a-skill-has-one-owner)
  - [`distribution:a-skill-obeys-the-portable-format` — A skill obeys the portable format](#distributiona-skill-obeys-the-portable-format--a-skill-obeys-the-portable-format)
  - [`distribution:skill-install-previews-before-writing` — A skill install previews before writing](#distributionskill-install-previews-before-writing--a-skill-install-previews-before-writing)
  - [`distribution:a-stale-skill-is-not-a-conflict` — A stale skill is not a conflict](#distributiona-stale-skill-is-not-a-conflict--a-stale-skill-is-not-a-conflict)
  - [`distribution:a-skill-install-restores-on-failure` — A skill install restores on failure](#distributiona-skill-install-restores-on-failure--a-skill-install-restores-on-failure)
  - [`distribution:an-install-sweeps-what-the-payload-dropped` — An install sweeps what the payload dropped](#distributionan-install-sweeps-what-the-payload-dropped--an-install-sweeps-what-the-payload-dropped)
  - [`distribution:a-skill-destination-is-a-regular-file` — A skill destination is a regular file](#distributiona-skill-destination-is-a-regular-file--a-skill-destination-is-a-regular-file)
  - [`distribution:skill-uninstall-removes-only-what-it-wrote` — A skill uninstall removes only what it wrote](#distributionskill-uninstall-removes-only-what-it-wrote--a-skill-uninstall-removes-only-what-it-wrote)

<!--TOC-->

## Purpose

Rules governing what the `rk` binary carries and what it writes outside a target repository. The distribution is one installed binary embedding the method, the bindings, the snippets, the skills, and the pinned-tool registry, and every rule here binds whoever authors that binary. The files `rk init` lands inside a target are governed by the invariants in `method/01-invariants.md`, which an adopting project owns; the documentation this repository writes about itself is governed by `SPEC-instance.md`. No adopting project adopts this spec: its subject is the installer, so a project holding these rules would hold obligations it cannot violate and verifications it cannot run.

## Requirements

### `distribution:the-payload-roots-are-declared-once` — The payload roots are declared once

Every authored root the binary carries MUST be named in one inventory that the embed, the build script's change tracking, and the package-contents check all read.

#### Scenario: A payload root is embedded without entering the inventory

- GIVEN a new root embedded in `src/embedded.rs` and absent from the inventory in `src/payload_roots.rs`
- WHEN the test suite runs
- THEN the agreement test fails naming both files, before a development build can serve stale bytes for a root the build script does not watch

Verify: `cargo nextest run -E 'kind(lib)'`

### `distribution:the-published-crate-carries-every-root` — The published crate carries every root

The published package MUST contain every payload root, and the check MUST run before a release rather than at a consumer.

#### Scenario: An exclude entry is broadened and removes a payload root

- GIVEN a `Cargo.toml` `exclude` entry that newly matches a payload root
- WHEN `just check` runs its build gate
- THEN the package-contents test fails naming the root, before `cargo publish` can ship a crate that fails to compile at the consumer

Verify: `cargo nextest run --run-ignored ignored-only -E 'test(the_published_crate_carries_every_root)'`

### `distribution:machine-output-declares-its-schema` — Machine output declares its schema

Every machine-readable output the binary emits MUST carry a versioned schema held by a snapshot test, so a consumer is told when the shape changes rather than discovering it, and every failure MUST carry a `reason` from the one closed, append-only vocabulary beside the exit-code matrix.

#### Scenario: A field is renamed in a machine report

- GIVEN a serialized report whose field name an edit changes
- WHEN the test suite runs
- THEN the snapshot test fails naming the schema, and the change becomes a deliberate schema-version bump instead of a silent parser break at some agent

Verify: `cargo nextest run -E 'kind(lib)'`

### `distribution:skills-are-part-of-the-payload` — Skills are part of the payload

The distribution MUST embed every skill authored under `skills/` and serve it byte-identically, so a binary carries the skills of its own version and no project fetches them.

#### Scenario: A skill is edited without rebuilding the payload list

- GIVEN a skill directory under `skills/` whose `SKILL.md` changed
- WHEN the binary is rebuilt and `rk skill show <name>` runs
- THEN its output is byte-identical to the authored file, and `rk skill list` names it

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-has-one-owner` — A skill has one owner

The distribution MUST install every skill at user scope alone, and `rk init` MUST land no skill into a target, because an agent resolves a skill by name across scopes and a second copy under one name is a second entry offering the same skill.

#### Scenario: A project is initialized inside a home that already carries the skills

- GIVEN a home directory holding the installed skills and a target repository
- WHEN `rk init --tech rust --target . --apply` runs
- THEN the target carries no `.claude/skills/` or `.agents/skills/` file, so each skill name resolves to exactly one file

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-obeys-the-portable-format` — A skill obeys the portable format

Every skill MUST carry only the portable Agent Skills frontmatter fields on plain single-line values, a `name` equal to its directory name, and a body at or below 150 lines.

#### Scenario: A skill gains an agent-specific field

- GIVEN a skill edited to add a vendor-only frontmatter key
- WHEN the test suite runs
- THEN the conformance test fails and names the offending field

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:skill-install-previews-before-writing` — A skill install previews before writing

When run without `--apply`, `rk skill install` MUST list every destination and write nothing, and where a destination holds bytes neither the payload nor the user-scope record accounts for, an apply MUST refuse atomically, naming every conflict in one run.

#### Scenario: A home directory already carries an edited skill

- GIVEN `~/.claude/skills/rk-release/SKILL.md` holding bytes the user wrote
- WHEN `rk skill install --apply` runs
- THEN it exits 73 naming that destination, the file is unchanged, and the second root is untouched

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-stale-skill-is-not-a-conflict` — A stale skill is not a conflict

Where a destination holds the bytes a previous apply recorded writing there, `rk skill install --apply` MUST replace it without `--force`.

#### Scenario: A release edits a skill the user never touched

- GIVEN a home whose installed skills came from an older release of this binary
- WHEN a newer `rk skill install --apply` runs
- THEN every destination is upgraded, no conflict is reported, and `just install` stays idempotent across releases

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-install-restores-on-failure` — A skill install restores on failure

If an apply fails partway, then `rk skill install` MUST restore every destination it backed up and name both the path that failed and any it could not restore.

#### Scenario: The second skill root cannot be written

- GIVEN two roots, the second holding a destination the process cannot create
- WHEN `rk skill install --apply` has already rewritten the first root
- THEN the first root is restored to its prior bytes, so no two agents read different versions of one skill

Verify: `cargo nextest run -E 'kind(lib)'`

### `distribution:an-install-sweeps-what-the-payload-dropped` — An install sweeps what the payload dropped

Where the record vouches for a destination under the roots a run touches and the payload no longer names it, `rk skill install --apply` and `rk skill uninstall --apply` MUST remove it, and MUST leave a destination whose bytes the record does not vouch for.

#### Scenario: A release renames a skill

- GIVEN a home holding a skill directory an earlier release installed and this one dropped
- WHEN `rk skill install --apply` runs
- THEN the leftover and the directory it emptied are gone, so an agent offers one entry per skill

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-destination-is-a-regular-file` — A skill destination is a regular file

Where a destination is a symlink or is not a regular file, `rk skill install` and `rk skill uninstall` MUST refuse before writing or removing anything, whatever `--force` was given.

#### Scenario: A destination is symlinked out of the home

- GIVEN `~/.claude/skills/rk-release/SKILL.md` symlinked to a file elsewhere
- WHEN `rk skill install --apply --force` runs
- THEN it exits 73 naming the symlink, and the file it points at is unchanged

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:skill-uninstall-removes-only-what-it-wrote` — A skill uninstall removes only what it wrote

`rk skill uninstall --apply` MUST remove only the payload's own destinations and the leftovers the record vouches for, and MUST keep a directory holding anything else.

#### Scenario: A user keeps notes beside an installed skill

- GIVEN `~/.claude/skills/rk-setup/` holding the installed `SKILL.md` and a file the user added
- WHEN `rk skill uninstall --apply` runs
- THEN the `SKILL.md` is removed, the added file and its directory remain, and a re-run succeeds

Verify: `cargo nextest run -E 'binary(cli)'`
