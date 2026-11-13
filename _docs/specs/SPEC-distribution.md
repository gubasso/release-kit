# Distribution Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`distribution:the-distribution-roots-are-declared-once` — The distribution roots are declared once](#distributionthe-distribution-roots-are-declared-once--the-distribution-roots-are-declared-once)
  - [`distribution:the-published-crate-carries-every-root` — The published crate carries every root](#distributionthe-published-crate-carries-every-root--the-published-crate-carries-every-root)
  - [`distribution:guidance-names-the-version-the-release-mints` — Guidance names the version the release mints](#distributionguidance-names-the-version-the-release-mints--guidance-names-the-version-the-release-mints)
  - [`distribution:machine-output-declares-its-schema` — Machine output declares its schema](#distributionmachine-output-declares-its-schema--machine-output-declares-its-schema)
  - [`distribution:a-human-faced-artifact-is-authored-text` — A human-faced artifact is authored text](#distributiona-human-faced-artifact-is-authored-text--a-human-faced-artifact-is-authored-text)
  - [`distribution:the-distribution-names-no-other-project` — The distribution names no other project](#distributionthe-distribution-names-no-other-project--the-distribution-names-no-other-project)
  - [`distribution:a-runbook-renders-the-spine` — A runbook renders the spine](#distributiona-runbook-renders-the-spine--a-runbook-renders-the-spine)
  - [`distribution:a-skill-routes-and-never-restates` — A skill routes and never restates](#distributiona-skill-routes-and-never-restates--a-skill-routes-and-never-restates)
  - [`distribution:a-forge-document-answers-its-own-axis` — A forge document answers its own axis](#distributiona-forge-document-answers-its-own-axis--a-forge-document-answers-its-own-axis)
  - [`distribution:skills-are-part-of-the-payload` — Skills are part of the payload](#distributionskills-are-part-of-the-payload--skills-are-part-of-the-payload)
  - [`distribution:a-skill-has-one-owner` — A skill has one owner](#distributiona-skill-has-one-owner--a-skill-has-one-owner)
  - [`distribution:a-landing-carries-the-skills` — A landing carries the skills](#distributiona-landing-carries-the-skills--a-landing-carries-the-skills)
  - [`distribution:an-unattributed-skill-folder-is-never-replaced` — An unattributed skill folder is never replaced](#distributionan-unattributed-skill-folder-is-never-replaced--an-unattributed-skill-folder-is-never-replaced)
  - [`distribution:a-skill-obeys-the-portable-format` — A skill obeys the portable format](#distributiona-skill-obeys-the-portable-format--a-skill-obeys-the-portable-format)
  - [`distribution:a-skill-plans-before-it-acts` — A skill plans before it acts](#distributiona-skill-plans-before-it-acts--a-skill-plans-before-it-acts)
  - [`distribution:a-skill-checks-its-host-before-it-plans` — A skill checks its host before it plans](#distributiona-skill-checks-its-host-before-it-plans--a-skill-checks-its-host-before-it-plans)
  - [`distribution:shared-skill-artifacts-have-one-home` — Shared skill artifacts have one home](#distributionshared-skill-artifacts-have-one-home--shared-skill-artifacts-have-one-home)
  - [`distribution:a-skill-is-readable-before-a-landing` — A skill is readable before a landing](#distributiona-skill-is-readable-before-a-landing--a-skill-is-readable-before-a-landing)
  - [`distribution:the-doctor-names-a-shadowing-user-copy` — The doctor names a shadowing user copy](#distributionthe-doctor-names-a-shadowing-user-copy--the-doctor-names-a-shadowing-user-copy)
  - [`distribution:a-skill-destination-is-a-regular-file` — A skill destination is a regular file](#distributiona-skill-destination-is-a-regular-file--a-skill-destination-is-a-regular-file)
  - [`distribution:skill-uninstall-removes-only-what-it-wrote` — A skill uninstall removes only what it wrote](#distributionskill-uninstall-removes-only-what-it-wrote--a-skill-uninstall-removes-only-what-it-wrote)
  - [`distribution:this-repository-runs-its-previous-release` — This repository runs its previous release](#distributionthis-repository-runs-its-previous-release--this-repository-runs-its-previous-release)

<!--TOC-->

## Purpose

Rules governing what the `rk` binary carries, what it writes outside a target repository, the skill packages that every landing writes into a target, and how this repository takes its own previous release. The distribution is one installed binary embedding the method, the bindings, the snippets, the blocks, the guidance, the skills, and the pinned-tool registry, read directly by every command that renders, stages, serves, or installs them, and every rule here binds whoever authors that binary. The binary answers for its own release alone: the candidate a stage shows and the landing production writes both come from these embedded sources, bound by `SPEC-staging.md` and `SPEC-landing.md`. The files `rk init` lands inside a target are governed by the invariants in `method/01-invariants.md`, which an adopting project owns; the documentation this repository writes about itself is governed by `SPEC-instance.md`. No adopting project adopts this spec: its subject is the installer, so a project holding these rules would hold obligations it cannot violate and verifications it cannot run. The external sources these rules were checked against are in `../reference/REFERENCE-distribution-sources.md`.

## Requirements

### `distribution:the-distribution-roots-are-declared-once` — The distribution roots are declared once

Every authored root the binary carries, `guidance/` included as the versioned reference material a stage copies, MUST be named in one inventory that the embed, the build script's change tracking, the package-contents check, the projection, the staging command, the reader commands, and the skill installer all read directly.

#### Scenario: A distribution root is embedded without entering the inventory

- GIVEN a new root embedded in `src/embedded.rs` and absent from the inventory in `src/distribution_roots.rs`
- WHEN the test suite runs
- THEN the agreement test fails naming both files, before a development build can serve stale bytes for a root the build script does not watch

Verify: `cargo nextest run -E 'kind(lib)'`

### `distribution:the-published-crate-carries-every-root` — The published crate carries every root

The published package MUST contain every distribution root, and the check MUST run before a release rather than at a consumer.

#### Scenario: An exclude entry is broadened and removes a distribution root

- GIVEN a `Cargo.toml` `exclude` entry that newly matches a distribution root
- WHEN `just check` runs its build gate
- THEN the package-contents test fails naming the root, before `cargo publish` can ship a crate that fails to compile at the consumer

Verify: `cargo nextest run --run-ignored ignored-only -E 'test(the_published_crate_carries_every_root)'`

### `distribution:guidance-names-the-version-the-release-mints` — Guidance names the version the release mints

A release that changes a landed destination MUST carry coverage naming the version that release mints, as a guidance file or a recorded silence, and the gate holding it MUST prove that exact version where the checkout carries a readable release candidate and MUST prove the weaker above-the-last-tag statement otherwise, naming which of the two it proved.

#### Scenario: A guidance file names a version the release will not mint

- GIVEN a checkout whose package version states the candidate 0.6.2 over the last tag v0.6.1, and coverage naming 0.7.0 alone
- WHEN the authoring gate runs
- THEN it fails naming the version the release mints and the file to add, rather than accepting a file that reaches no target because every agent selects guidance by the version a target records

Verify: `cargo nextest run -E 'test(a_candidate_release_rejects_coverage_naming_another_version)'`

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

### `distribution:the-distribution-names-no-other-project` — The distribution names no other project

Nothing the distribution carries or serves MAY name a specific project, repository, or organization other than release-kit's own configuration, so a reader needs no knowledge outside this repository.

#### Scenario: A migrated script keeps one identifier from the implementation it generalized

- GIVEN a setup script or runbook carried over with a foreign variable prefix, bot identity, or guide filename left in it
- WHEN the test suite runs
- THEN the denylist test fails naming the file and the line, before a reader who lacks access to the other repository meets a reference they cannot follow

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-runbook-renders-the-spine` — A runbook renders the spine

A runbook MUST carry the same numbered steps, in the same order, as the method chapter it renders, and the pair MUST state each procedure exactly once: the chapter owns each step's why and the runbook owns its how — the commands, the checks, and the hand forms. A substep MUST elaborate a step the runbook has and MUST NOT add one. The landing runbook MUST render the landing chapter under the same rule, and the setup and migration runbooks MUST route their landing steps to it by step number rather than restate them, because the stage, land, verify, clean procedure is stated once for every landing write.

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

#### Scenario: A skill is edited and the binary is rebuilt

- GIVEN a skill directory under `skills/` whose `SKILL.md` changed
- WHEN the binary is rebuilt and `rk skill show <name>` runs
- THEN its output is byte-identical to the authored file, and `rk skill list` names it

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-has-one-owner` — A skill has one owner

The landing MUST own every skill at project scope, and every `rk` verb MUST leave the skill roots under the home unwritten, because an agent resolves a skill by name across scopes and one host copy cannot match the rk that each project pins. The version of a landed skill is the `rk_version` of the record, so a skill file carries no stamp. `rk skill uninstall` stays, and it only removes what an earlier release wrote under the home.

#### Scenario: A project is initialized inside a home that carries no skill

- GIVEN an empty home directory and a target repository
- WHEN `rk init --technology rust --target . --apply` runs, and then `rk skill install --apply`
- THEN the target carries both skills under both roots, the install refuses and names the landing, and the home holds no skill file

Verify: `cargo nextest run -E 'test(init_lands_both_skills_into_both_roots) or test(skill_install_refuses_and_names_the_landing)'`

### `distribution:a-landing-carries-the-skills` — A landing carries the skills

Every landing MUST project each file of each package that `skills::packages()` returns, once under `.claude/skills/<name>/` and once under `.agents/skills/<name>/`, byte-equal to the package, and MUST record each one as a `rendered` destination, because the record then judges drift, retires a dropped file, and prompts an upgrade with no mechanism of its own. `.claude/skills/` serves Claude Code, and `.agents/skills/` serves Codex, Gemini CLI, and Copilot CLI, as `../reference/REFERENCE-skill-scopes-sources.md` records. A landing of this repository by its pinned release follows that release's projection, under `distribution:this-repository-runs-its-previous-release`.

#### Scenario: A landing from an older release meets a newer binary

- GIVEN a target landed by a release that projected no skill
- WHEN `rk upgrade --apply` runs
- THEN both skills land under both roots, each file equals the package bytes, and `rk status --check` reports `current`

Verify: `cargo nextest run -E 'test(init_lands_both_skills_into_both_roots) or test(a_landed_skill_is_byte_equal_to_the_package) or test(an_upgrade_brings_the_skills_to_an_older_landing) or test(an_edited_landed_skill_is_drift) or test(a_skill_file_the_package_drops_is_retired)'`

### `distribution:an-unattributed-skill-folder-is-never-replaced` — An unattributed skill folder is never replaced

Where a package folder exists under a skill root and the record does not attribute it, `rk init`, `rk adopt`, and `rk upgrade` MUST refuse before the first write, name the folder, and accept no `--replace` for it, because a skill folder that the landing did not write holds the operator's own skill under one name. A full folder, a partial one, an unreadable one, and one that holds only an extra file all refuse. A package folder that does not exist is created, which covers the first landing and the adoption of a target that predates the skills.

#### Scenario: A hand-written skill shares a name

- GIVEN a target whose `.claude/skills/rk-setup/` holds only a file that the operator wrote, and no record
- WHEN `rk init --apply` runs, and then again with `--replace` naming that file
- THEN both runs exit 73 naming the folder, and the target is unchanged

Verify: `cargo nextest run -E 'test(a_landing_refuses_an_unattributed_skill_folder) or test(a_landing_creates_an_absent_skill_folder) or test(adoption_of_a_pre_skill_target_brings_the_skills)'`

### `distribution:a-skill-obeys-the-portable-format` — A skill obeys the portable format

Every skill MUST carry only the portable Agent Skills frontmatter fields on plain single-line values, and a `name` equal to its directory name. Its size is counted in characters, by `skill-format:size-is-counted-in-characters`.

#### Scenario: A skill gains an agent-specific field

- GIVEN a skill edited to add a vendor-only frontmatter key
- WHEN the test suite runs
- THEN the conformance test fails and names the offending field

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-plans-before-it-acts` — A skill plans before it acts

Every skill MUST open its pre-flight section by reading the shared pre-flight gate before any other action, and MUST read the shared plan gate in its gate section, after the task pre-flights its gate picked and before the first write, because each one drives operations that write files, mutate a forge, or publish a version, and an agent that starts acting before it has stated the sequence has no point left at which the operator can stop it. The plan gate owns what `--no-plan` changes, so no skill restates it. The gate itself MUST bound what a request authorizes — the file writes and `rk` verbs it names — leaving every branch, commit, push, tag, and pull request action to the operator unless their request named it, because a plan the operator approves states a shape and grants no standing licence over the repository's git and forge state. Where a request carries a term the target's glossary defines, which only a target that opted into the workflow guards carries, the gate MUST read that term's action list as actions the request named, and MUST plan each one as a step, because the expansion is the operator's own writing and the boundary is what the operator asked for in one word.

#### Scenario: A skill is authored with its steps ahead of the pre-flight

- GIVEN a skill whose pre-flight section does not open by reading the pre-flight gate, or whose gate section never reads the plan gate
- WHEN the test suite runs
- THEN the conformance test fails and names the skill, because an agent reading top to bottom would act before reaching a gate

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:a-skill-checks-its-host-before-it-plans` — A skill checks its host before it plans

The distribution MUST carry a shared pre-flight gate as its own artifact beside the plan gate, and every skill MUST route to it ahead of the plan gate, because a skill routes to verbs whose dependencies live outside the repository, a forge CLI, a signing tool, or a user-scope skill copy that shadows the landed one, and a plan written without observing them fails at the step nobody checked. It MUST be shared rather than authored per skill, so every skill checks the same host facts the same way. It MUST check the host alone, report a missing `rk` instead of stopping on it, and hand the status report or its diagnostic back to the skill, judging nothing in either, because a setup skill provisions the binary it runs on and only the skill knows what a finding means for its tasks. It MUST run whatever the request carries, and no flag MUST waive it, because a check a request can decline is one no skill can rely on having run. It MUST name every probe that judges the skills on the host, so the catalog and the gate cannot drift into checking different things.

#### Scenario: A skill probe is added to the catalog and the pre-flight is not updated

- GIVEN a probe judging the skills on the host that the pre-flight gate does not name
- WHEN the test suite reads the catalog's declaration and the gate
- THEN it fails and names the probe, because an agent following the gate would never read that probe's answer

#### Scenario: A skill routes to both gates

- GIVEN a skill's pre-flight and gate sections
- WHEN the test suite reads them
- THEN the pre-flight opens with the pre-flight gate and the gate section reads the plan gate, each by the path relative to the skill root, `references/<file>`, and the pre-flight states that no flag skips it

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:shared-skill-artifacts-have-one-home` — Shared skill artifacts have one home

The distribution MUST author what the skills share once, under `skill-shared/`, and every package MUST carry it as `references/<file>`, so each landed package holds every file it reads and a skill names each shared artifact by a path relative to its own root. A package is the unit that the landing writes, and the format defines no location two skills share, so a read outside the skill directory is a hop a sandboxed agent can prompt on (`../reference/REFERENCE-skill-format-sources.md`). The copies come from one embedded source, so a fix still lands once.

#### Scenario: A shared gate is edited

- GIVEN an edit to `skill-shared/plan-gate.md`
- WHEN the binary is rebuilt and a target is landed
- THEN `references/plan-gate.md` of each package under both roots equals the edited file

Verify: `cargo nextest run -E 'test(a_landed_skill_is_byte_equal_to_the_package) or test(skill_show_prints_one_reference_file)'`

### `distribution:a-skill-is-readable-before-a-landing` — A skill is readable before a landing

`rk skill show <name>` MUST print the package's `SKILL.md` byte-identical to the authored file, and `rk skill show <name> --file <path>` MUST print that file of the package byte for byte and exit 66 on a path the package lacks, because an agent in a project with no landing has no skill on disk and reads the setup skill from the binary.

#### Scenario: An agent starts in a project with no landing

- GIVEN a folder with no record, and a home with no skill copy
- WHEN `rk skill show rk-setup --file references/pre-flight-gate.md` runs
- THEN it prints the authored gate, and an unknown path exits 66

Verify: `cargo nextest run -E 'test(skill_show_prints_one_reference_file) or test(skill_show_is_byte_identical_to_the_authored_file)'`

### `distribution:the-doctor-names-a-shadowing-user-copy` — The doctor names a shadowing user copy

The probe `skill-shadow` MUST fail while a `SKILL.md` of an embedded skill exists under `~/.claude/skills/<name>/` or `~/.agents/skills/<name>/`, MUST name `rk skill uninstall --apply` as the fix, and MUST pass on a folder without `SKILL.md`, because Claude Code resolves "personal over project" and a home copy left behind wins over the landed one. Where the `SKILL.md` is an edited copy that the uninstall kept, the probe MUST tell the operator to read it and remove it by hand. It stays a soft probe, and it creates nothing it judges.

#### Scenario: An uninstall kept the operator's notes

- GIVEN `~/.claude/skills/rk-setup/` holding only a file that the operator wrote
- WHEN `rk doctor` runs
- THEN `skill-shadow` passes, and it fails once a `SKILL.md` appears there

Verify: `cargo nextest run -E 'test(the_shadow_probe_names_a_user_scope_manual) or test(doctor_reports_every_probe_and_exits_0)'`

### `distribution:a-skill-destination-is-a-regular-file` — A skill destination is a regular file

Where a destination under the home is a symlink, is not a regular file, or sits under the legacy shared root reached through a symlinked directory, `rk skill uninstall` MUST refuse before it removes anything.

#### Scenario: A destination is symlinked out of the home

- GIVEN `~/.claude/skills/rk-release/SKILL.md` symlinked to a file elsewhere
- WHEN `rk skill uninstall --apply` runs
- THEN it exits 73 naming the symlink, and the file it points at is unchanged

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:skill-uninstall-removes-only-what-it-wrote` — A skill uninstall removes only what it wrote

`rk skill uninstall --apply` MUST remove only the embedded skills' own destinations and the leftovers the record vouches for, and MUST keep a directory holding anything else.

#### Scenario: A user keeps notes beside an installed skill

- GIVEN `~/.claude/skills/rk-setup/` holding the installed `SKILL.md` and a file the user added
- WHEN `rk skill uninstall --apply` runs
- THEN the `SKILL.md` is removed, the added file and its directory remain, and a re-run succeeds

Verify: `cargo nextest run -E 'binary(cli)'`

### `distribution:this-repository-runs-its-previous-release` — This repository runs its previous release

This repository's devshell MUST carry the rk of a released tag through its `release-kit` flake input, and every caller that tests the tree MUST name `target/debug/rk` or `cargo run`, because the tool's own repository is a consumer of its previous release, and an rk built from the tree is newer than the landing record after every release. The pinned rk runs the hooks, `rk worktree`, `rk integrate`, and the repository's own landing. The rk built from the tree runs only under `cargo nextest`, `just build`, and the scratch landing. After a release publishes, one change moves the pin and runs `rk upgrade` on this repository.

#### Scenario: A release bumps the crate version

- GIVEN a merged release that moves `Cargo.toml` to a new version
- WHEN an agent commits the next change in this repository
- THEN the hooks run the pinned rk, which matches the landing record, and the version gate passes

Verify: `cargo nextest run -E 'test(every_tree_test_names_the_built_binary) or test(the_release_jobs_take_rk_from_the_environment)' && prek run the-devshell-rk-is-the-pinned-release --all-files`
