# Distribution Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`distribution:the-payload-roots-are-declared-once` — The payload roots are declared once](#distributionthe-payload-roots-are-declared-once--the-payload-roots-are-declared-once)
  - [`distribution:the-published-crate-carries-every-root` — The published crate carries every root](#distributionthe-published-crate-carries-every-root--the-published-crate-carries-every-root)
  - [`distribution:machine-output-declares-its-schema` — Machine output declares its schema](#distributionmachine-output-declares-its-schema--machine-output-declares-its-schema)
  - [`distribution:a-human-faced-artifact-is-authored-text` — A human-faced artifact is authored text](#distributiona-human-faced-artifact-is-authored-text--a-human-faced-artifact-is-authored-text)
  - [`distribution:the-payload-names-no-other-project` — The payload names no other project](#distributionthe-payload-names-no-other-project--the-payload-names-no-other-project)
  - [`distribution:a-runbook-renders-the-spine` — A runbook renders the spine](#distributiona-runbook-renders-the-spine--a-runbook-renders-the-spine)
  - [`distribution:a-skill-routes-and-never-restates` — A skill routes and never restates](#distributiona-skill-routes-and-never-restates--a-skill-routes-and-never-restates)
  - [`distribution:a-forge-document-answers-its-own-axis` — A forge document answers its own axis](#distributiona-forge-document-answers-its-own-axis--a-forge-document-answers-its-own-axis)
  - [`distribution:skills-are-part-of-the-payload` — Skills are part of the payload](#distributionskills-are-part-of-the-payload--skills-are-part-of-the-payload)
  - [`distribution:a-skill-has-one-owner` — A skill has one owner](#distributiona-skill-has-one-owner--a-skill-has-one-owner)
  - [`distribution:a-skill-obeys-the-portable-format` — A skill obeys the portable format](#distributiona-skill-obeys-the-portable-format--a-skill-obeys-the-portable-format)
  - [`distribution:a-skill-plans-before-it-acts` — A skill plans before it acts](#distributiona-skill-plans-before-it-acts--a-skill-plans-before-it-acts)
  - [`distribution:a-skill-checks-its-host-before-it-plans` — A skill checks its host before it plans](#distributiona-skill-checks-its-host-before-it-plans--a-skill-checks-its-host-before-it-plans)
  - [`distribution:shared-skill-artifacts-have-one-home` — Shared skill artifacts have one home](#distributionshared-skill-artifacts-have-one-home--shared-skill-artifacts-have-one-home)
  - [`distribution:the-doctor-answers-for-the-installed-skills` — The doctor answers for the installed skills](#distributionthe-doctor-answers-for-the-installed-skills--the-doctor-answers-for-the-installed-skills)
  - [`distribution:skill-install-previews-before-writing` — A skill install previews before writing](#distributionskill-install-previews-before-writing--a-skill-install-previews-before-writing)
  - [`distribution:a-stale-skill-is-not-a-conflict` — A stale skill is not a conflict](#distributiona-stale-skill-is-not-a-conflict--a-stale-skill-is-not-a-conflict)
  - [`distribution:a-skill-install-restores-on-failure` — A skill install restores on failure](#distributiona-skill-install-restores-on-failure--a-skill-install-restores-on-failure)
  - [`distribution:an-install-sweeps-what-the-payload-dropped` — An install sweeps what the payload dropped](#distributionan-install-sweeps-what-the-payload-dropped--an-install-sweeps-what-the-payload-dropped)
  - [`distribution:a-skill-destination-is-a-regular-file` — A skill destination is a regular file](#distributiona-skill-destination-is-a-regular-file--a-skill-destination-is-a-regular-file)
  - [`distribution:skill-uninstall-removes-only-what-it-wrote` — A skill uninstall removes only what it wrote](#distributionskill-uninstall-removes-only-what-it-wrote--a-skill-uninstall-removes-only-what-it-wrote)

<!--TOC-->

## Purpose

Rules governing what the `rk` binary carries and what it writes outside a target repository. The distribution is one installed binary embedding the method, the bindings, the snippets, the skills, and the pinned-tool registry, and every rule here binds whoever authors that binary. The files `rk init` lands inside a target are governed by the invariants in `method/01-invariants.md`, which an adopting project owns; the documentation this repository writes about itself is governed by `SPEC-instance.md`. No adopting project adopts this spec: its subject is the installer, so a project holding these rules would hold obligations it cannot violate and verifications it cannot run. The external sources these rules were checked against are in `../reference/REFERENCE-distribution-sources.md`.

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

### `distribution:a-human-faced-artifact-is-authored-text` — A human-faced artifact is authored text

Every whole artifact the binary writes into a target or a host — a spliced block, an installed hook body — MUST originate from an authored file under `blocks/`, never from a source literal; markers, grammars, and substitution tokens stay code, and token rendering is the one transformation allowed between the authored bytes and the written ones.

#### Scenario: A new host-written text is added as a Rust string literal

- GIVEN a whole artifact body added to the sources as a string constant
- WHEN the test suite runs
- THEN the source scan fails naming the file and line, and the body moves under `blocks/` before the change lands

Verify: `cargo nextest run -E 'binary(cli) or kind(lib)'`

### `distribution:the-payload-names-no-other-project` — The payload names no other project

Nothing the distribution carries or serves MAY name a specific project, repository, or organization other than release-kit's own configuration, so a reader needs no knowledge outside this repository.

#### Scenario: A migrated script keeps one identifier from the implementation it generalized

- GIVEN a setup script or runbook carried over with a foreign variable prefix, bot identity, or guide filename left in it
- WHEN the test suite runs
- THEN the denylist test fails naming the file and the line, before a reader who lacks access to the other repository meets a reference they cannot follow

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-runbook-renders-the-spine` — A runbook renders the spine

A runbook MUST carry the same numbered steps, in the same order, as the method chapter it renders, and the pair MUST state each procedure exactly once: the chapter owns each step's why and the runbook owns its how — the commands, the checks, and the hand forms. A substep MUST elaborate a step the runbook has and MUST NOT add one.

#### Scenario: A method chapter gains a step and the runbook is not updated

- GIVEN a new numbered step in a chapter a runbook renders
- WHEN the test suite runs
- THEN the parity test fails naming the runbook, so the procedure cannot fork into two step lists

#### Scenario: A substep names a step the runbook does not have

- GIVEN a runbook edited to carry `### 9a.` where no `## 9.` exists
- WHEN the test suite runs
- THEN the substep test fails naming the file, because a substep outside every step is a new step in disguise

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-routes-and-never-restates` — A skill routes and never restates

A skill MUST route to the runbook, chapter, forge document, or binding that owns a procedure — by served name, and by step number where it addresses one step — and MUST NOT restate the owned steps' content, because a restated sequence is a second copy that drifts silently while the served one moves.

#### Scenario: A skill is edited to carry a runbook's steps inline

- GIVEN a skill whose task a served runbook already sequences
- WHEN the edit spells the runbook's steps out in the skill body
- THEN review rejects the change, because the skill's judgment lines belong to it and the sequence belongs to the runbook

Verify: reviewer confirms each skill names its sources and carries no step content a served document owns

### `distribution:a-forge-document-answers-its-own-axis` — A forge document answers its own axis

Every supported forge MUST have a document carrying its bootstrap walkthrough, its command mapping, and its limitations, and every statement in it MUST be about that forge.

#### Scenario: A forge is added with a script tree and no document

- GIVEN a new subtree under `setup/` with no `forges/<name>.md` beside it
- WHEN the test suite runs
- THEN the closure test fails, before an operator reaches the one step no command performs and finds nothing that says what to click

Verify: `cargo nextest run -E 'binary(cli)'`

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

### `distribution:a-skill-plans-before-it-acts` — A skill plans before it acts

Every skill MUST route to the shared plan gate in a section preceding every other section, and MUST state what `--no-plan` changes, because each one drives operations that write files, mutate a forge, or publish a version, and an agent that starts acting before it has stated the sequence has no point left at which the operator can stop it. The gate itself MUST bound what a request authorizes — the file writes and `rk` verbs it names — leaving every branch, commit, push, tag, and pull request action to the operator unless their request named it, because a plan the operator approves states a shape and grants no standing licence over the repository's git and forge state.

#### Scenario: A skill is authored with its steps ahead of the gate

- GIVEN a skill whose first section is a step list rather than the gate
- WHEN the test suite runs
- THEN the conformance test fails and names the skill, because an agent reading top to bottom would act before reaching the gate

#### Scenario: The gate is read for what it authorizes

- GIVEN the shared plan gate every skill names
- WHEN the test suite reads it
- THEN it carries the section bounding a request's authority, so every skill holding the gate holds that boundary

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-checks-its-host-before-it-plans` — A skill checks its host before it plans

The distribution MUST carry a shared pre-flight gate as its own artifact beside the plan gate, and every skill MUST route to it ahead of the plan gate, because a skill routes to verbs whose dependencies live outside the repository — a forge CLI, a signing tool, the skills and shared artifacts installed under the home — and a plan written without observing them fails at the step nobody checked. It MUST be shared rather than authored per skill, because `distribution:a-skill-plans-before-it-acts` puts the gate section ahead of every other section, so a per-skill pre-flight would be either unreachable or ahead of the gate bounding it. It MUST run whatever the request carries, and no flag MUST waive it — `--no-plan` moves the plan gate's approval turn and nothing else — because a check a request can decline is one no skill can rely on having run. It MUST name every probe judging the skill installation itself, so the catalog and the gate cannot drift into checking different things, and it MUST hand the task to the plan gate, so the two are one sequence rather than two entry points.

#### Scenario: A skill probe is added to the catalog and the pre-flight is not updated

- GIVEN a probe judging the skill installation that the pre-flight gate does not name
- WHEN the test suite reads the catalog's declaration and the gate
- THEN it fails and names the probe, because an agent following the gate would never read that probe's answer

#### Scenario: A skill routes to both gates

- GIVEN a skill's gate section
- WHEN the test suite reads it
- THEN it names both shared artifacts by the absolute path they install to, and states that no flag skips the pre-flight

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:shared-skill-artifacts-have-one-home` — Shared skill artifacts have one home

The distribution MUST install what the skills share once, outside the agent skill roots and under the invoking user's home, whichever agent a run selects; and `rk skill uninstall --apply` MUST keep those artifacts while any agent root still holds a skill that names them. A skill MUST name each of them by that absolute path, because the two agent roots make no relative path reach one file from both.

The location is home-relative rather than `XDG_STATE_HOME`-relative for the reason the record already states: the skills reading these artifacts live under `$HOME/.claude` and `$HOME/.agents`, which no XDG variable moves.

#### Scenario: One agent family is uninstalled while the other stays

- GIVEN a home carrying the skills under both agent roots
- WHEN `rk skill uninstall --agent codex --apply` runs
- THEN the shared artifacts remain, because the skills under `.claude/skills` still name them, and a later uninstall of the last root removes them

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:the-doctor-answers-for-the-installed-skills` — The doctor answers for the installed skills

The probe catalog MUST report whether the shared artifacts and the skills installed under the invoking user's home are the ones the running binary carries, and whether the roots an install writes accept writes at all. One installed binary serves every repository while its skills sit in three separate directories under a home, so the two can be updated apart and the home can be shared — into a container, a sandbox, or across machines — with some of those directories carried and others not. The failures that produces are silent in exactly the wrong way: a skill resolves by name and then cannot read the gate it is told to read first, or it follows a routing table naming verbs the binary on PATH does not answer. Prose cannot catch either, because the artifact that would carry the warning is the missing one.

A difference the record vouches for is a stale install and its remediation MUST be the plain apply; a difference the record cannot account for is the operator's own and its remediation MUST be the forcing one, per `distribution:a-stale-skill-is-not-a-conflict`. An absent agent root MUST NOT be a failure on its own, because `--agent` selects one family and leaves the other's root untouched. These probes MUST NOT create any destination they judge, because a preview must still be able to report a root as absent.

#### Scenario: A home carries the skills and not what they share

- GIVEN a home whose agent roots hold the skills while the shared root does not
- WHEN `rk doctor` runs
- THEN the gate probe fails, names the missing artifact, and gives the install that lands it, while the payload probe still reports the skills as installed

#### Scenario: An installed skill is not the running binary's

- GIVEN a home holding a skill whose bytes differ from the payload
- WHEN `rk doctor` runs
- THEN the payload probe fails and names the running version, asking for the forcing apply only where the record cannot vouch for the bytes it would overwrite

#### Scenario: A skill root refuses writes

- GIVEN a home whose agent root is mounted read-only
- WHEN `rk doctor` runs
- THEN the roots probe fails and names the directory, because the install that would fix the other two probes cannot run there

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

Where a destination is a symlink, is not a regular file, or sits under a shared root reached through a symlinked directory, `rk skill install` and `rk skill uninstall` MUST refuse before writing or removing anything, whatever `--force` was given.

#### Scenario: A destination is symlinked out of the home

- GIVEN `~/.claude/skills/rk-release/SKILL.md` symlinked to a file elsewhere
- WHEN `rk skill install --apply --force` runs
- THEN it exits 73 naming the symlink, and the file it points at is unchanged

#### Scenario: The shared root is reached through a symlinked directory

- GIVEN `~/.local/state/release-kit/skills` symlinked to a directory elsewhere, so every shared destination under it resolves outside the home
- WHEN `rk skill install --apply --force` runs
- THEN it exits 73 naming the symlink, and the directory it points at is unchanged, because the destination check sees only the final component and cannot see the link above it

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:skill-uninstall-removes-only-what-it-wrote` — A skill uninstall removes only what it wrote

`rk skill uninstall --apply` MUST remove only the payload's own destinations and the leftovers the record vouches for, and MUST keep a directory holding anything else.

#### Scenario: A user keeps notes beside an installed skill

- GIVEN `~/.claude/skills/rk-setup/` holding the installed `SKILL.md` and a file the user added
- WHEN `rk skill uninstall --apply` runs
- THEN the `SKILL.md` is removed, the added file and its directory remain, and a re-run succeeds

Verify: `cargo nextest run -E 'binary(cli)'`
