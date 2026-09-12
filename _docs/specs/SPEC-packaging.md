# Packaging Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`packaging:the-flake-serves-the-binary` — The flake serves the binary](#packagingthe-flake-serves-the-binary--the-flake-serves-the-binary)
  - [`packaging:the-version-has-one-owner` — The version has one owner](#packagingthe-version-has-one-owner--the-version-has-one-owner)
  - [`packaging:the-package-source-carries-every-root` — The package source carries every root](#packagingthe-package-source-carries-every-root--the-package-source-carries-every-root)
  - [`packaging:an-advertised-system-is-a-proven-system` — An advertised system is a proven system](#packagingan-advertised-system-is-a-proven-system--an-advertised-system-is-a-proven-system)
  - [`packaging:the-checks-carry-the-nix-side-signal` — The checks carry the Nix-side signal](#packagingthe-checks-carry-the-nix-side-signal--the-checks-carry-the-nix-side-signal)
  - [`packaging:the-wrapper-carries-the-hard-tools` — The wrapper carries the Hard tools](#packagingthe-wrapper-carries-the-hard-tools--the-wrapper-carries-the-hard-tools)
  - [`packaging:the-derivation-mirrors-the-probe-registry` — The derivation mirrors the probe registry](#packagingthe-derivation-mirrors-the-probe-registry--the-derivation-mirrors-the-probe-registry)
  - [`packaging:a-launcher-resolves-through-one-owner` — A launcher resolves through one owner](#packaginga-launcher-resolves-through-one-owner--a-launcher-resolves-through-one-owner)
  - [`packaging:the-landable-capability-promises-a-buildable-flake` — The landable capability promises a buildable flake](#packagingthe-landable-capability-promises-a-buildable-flake--the-landable-capability-promises-a-buildable-flake)
  - [`packaging:the-consumer-pin-has-two-facts-and-one-mover` — The consumer pin has two facts and one mover](#packagingthe-consumer-pin-has-two-facts-and-one-mover--the-consumer-pin-has-two-facts-and-one-mover)
  - [`packaging:the-pin-is-read-through-a-manager-axis` — The pin is read through a manager axis](#packagingthe-pin-is-read-through-a-manager-axis--the-pin-is-read-through-a-manager-axis)
  - [`packaging:the-venue-and-the-manager-cross-in-one-matrix` — The venue and the manager cross in one matrix](#packagingthe-venue-and-the-manager-cross-in-one-matrix--the-venue-and-the-manager-cross-in-one-matrix)
  - [`packaging:a-pin-bump-is-all-or-nothing` — A pin bump is all or nothing](#packaginga-pin-bump-is-all-or-nothing--a-pin-bump-is-all-or-nothing)
  - [`packaging:the-unattended-caller-never-fails-the-shell` — The unattended caller never fails the shell](#packagingthe-unattended-caller-never-fails-the-shell--the-unattended-caller-never-fails-the-shell)
  - [`packaging:add-serves-a-fragment-and-edits-no-owned-file` — Add serves a fragment and edits no owned file](#packagingadd-serves-a-fragment-and-edits-no-owned-file--add-serves-a-fragment-and-edits-no-owned-file)
  - [`packaging:a-wired-target-runs-one-bump-mechanism` — A wired target runs one bump mechanism](#packaginga-wired-target-runs-one-bump-mechanism--a-wired-target-runs-one-bump-mechanism)
  - [`packaging:the-cleanup-removes-only-what-it-can-judge` — The cleanup removes only what it can judge](#packagingthe-cleanup-removes-only-what-it-can-judge--the-cleanup-removes-only-what-it-can-judge)

<!--TOC-->

## Purpose

Rules governing the Nix packaging surface of this repository: the flake outputs, the package expression under `nix/`, the CI proof behind the support claim, and the consumer half — how a project obtains `rk` through the tool manager it already runs, from the venue that manager can consume, read and moved by `rk self-depend`. The boundary against `SPEC-distribution.md` is the artifact: that spec binds what the installed `rk` binary carries and writes, and this one binds how a consumer obtains that binary. The files `rk init` lands into a target are bound by `SPEC-landing.md`.

## Requirements

### `packaging:the-flake-serves-the-binary` — The flake serves the binary

The flake MUST expose the built binary as `packages.<system>.default` with `meta.mainProgram` naming `rk`, because the package name and the binary name differ and `nix run` resolves the binary through that attribute.

#### Scenario: A host with only Nix installs the tool

- GIVEN a machine with Nix and nothing else
- WHEN `nix run github:gubasso/release-kit -- --version` runs against a tagged revision
- THEN the flake builds the package and executes `rk`, with no `apps` output and no second install step

Verify: `nix run . -- --version`

### `packaging:the-version-has-one-owner` — The version has one owner

The author MUST NOT write a version literal into a `.nix` file, reading the name, version, and metadata from `Cargo.toml` through `lib.importTOML` instead, because release-plz derives the version from commits and a restated string drifts at the first release.

#### Scenario: A release bumps the crate version

- GIVEN a release commit that edits the version in `Cargo.toml`
- WHEN the flake builds that revision
- THEN the package carries the new version with no `.nix` edit in the release

Verify: `rg -n '"[0-9]+\.[0-9]+\.[0-9]+"' flake.nix nix/ | grep . && exit 1 || exit 0`

### `packaging:the-package-source-carries-every-root` — The package source carries every root

The package build MUST fail naming the path when its source omits a root declared in `src/payload_roots.rs` or a license file `src/embedded.rs` embeds, because a filtered source that drops a root still produces a binary that builds and lies.

#### Scenario: A source filter is narrowed later

- GIVEN a package expression whose source filter newly omits a payload root
- WHEN `nix build` runs
- THEN the build fails naming the missing root, before any smoke command that would never notice

Verify: `grep -q 'src/payload_roots.rs' nix/package.nix`

### `packaging:an-advertised-system-is-a-proven-system` — An advertised system is a proven system

Where the flake advertises a system, CI MUST natively build and run the flake's checks on that system, because an output set is a support promise and a cross-evaluated check builds nothing.

#### Scenario: A system joins the flake's list without a runner

- GIVEN a change that adds a system the CI matrix does not run natively
- WHEN the change is reviewed
- THEN either a native runner joins the matrix in the same change or the system stays out of the list

Verify: reviewer confirms the flake's system list and the CI matrix name the same systems

### `packaging:the-checks-carry-the-nix-side-signal` — The checks carry the Nix-side signal

The flake's `checks` MUST build the package and smoke the served payload, because `nix flake check` builds only the `checks` output and the crate's test suite, which drives real git and forge CLIs the sandbox lacks, stays out of the package build.

#### Scenario: A payload regression survives the build

- GIVEN a change that breaks what the binary serves without breaking compilation
- WHEN `nix flake check` runs
- THEN the smoke check fails offline, on the built package rather than on an evaluation

Verify: `nix flake check`

### `packaging:the-wrapper-carries-the-hard-tools` — The wrapper carries the Hard tools

The installed package MUST wrap `rk` with a `PATH` suffix supplying every executable the Hard probes require and no soft tool, because a package that finds no git hands a broken tool to a host with only Nix, while the soft tools multiply the closure for capabilities `rk doctor` already reports with a repair line. The suffix form MUST let an operator's own binary and every `RK_*_BIN` override win over the wrapped one.

#### Scenario: A host with only Nix runs a git verb

- GIVEN the built package on a host whose `PATH` is empty and whose home is writable
- WHEN `rk doctor` runs
- THEN the git and sh probes pass from the wrapper's suffix, and `RK_GIT_BIN` still substitutes the git

Verify: `nix flake check` — the smoke check asserts on the probe lines, with only `PATH` cleared

### `packaging:the-derivation-mirrors-the-probe-registry` — The derivation mirrors the probe registry

The wrapper's package list MUST agree with the Hard tool registry in `src/probes.rs`, held by an in-tree test that fails naming the divergence, because the two lists live in two languages, nothing derives one from the other, and a mirrored contract without a failing check ships its first divergence.

#### Scenario: A Hard tool joins the registry without the wrapper

- GIVEN a change that adds a Hard executable probe without extending the wrapper's package list
- WHEN the test suite runs
- THEN the mirror test fails naming the tool the wrapper lost

Verify: `cargo nextest run -E 'test(the_package_wrapper_mirrors_the_hard_tool_registry)'`

### `packaging:a-launcher-resolves-through-one-owner` — A launcher resolves through one owner

Production code MUST NOT launch `git` or `sh` by literal name outside the shared resolvers in `src/probes.rs`, because a direct launch bypasses the `RK_GIT_BIN` and `RK_SH_BIN` overrides silently and hides a runtime dependency from the registry the wrapper mirrors.

#### Scenario: A new call site launches git directly

- GIVEN a change that adds a `Command::new("git")` outside the resolver
- WHEN the test suite runs
- THEN the scan test fails naming the file and line

Verify: `cargo nextest run -E 'test(every_git_and_sh_launch_resolves_through_the_shared_resolver)'`

### `packaging:the-landable-capability-promises-a-buildable-flake` — The landable capability promises a buildable flake

The landed Nix capability MUST promise exactly a package expression that evaluates for the supported crate shape, a flake that builds where the seed pair landed, and a served job proving that build for every pair whose forge leaves the target a pipeline to add it to, and MUST NOT promise presence in nixpkgs or any registry, because a registry submission carries a human maintainer commitment into someone else's repository. The capability lands no CI file of its own: a job proving the build holds a merge only inside the pipeline the forge's gate reads, and that pipeline is the target's — the gated workflow on GitHub, and the child pipeline the rendered parent triggers on GitLab. The support matrix degrades honestly: a pair whose forge leaves the target no such pipeline reports the smaller product, never an error.

#### Scenario: An operator asks what the capability shipped

- GIVEN a target that opted in with `rk init --nix`
- WHEN the operator reads the landing report and the runbook
- THEN the promise is the build, and the job that proves it in the target's own pipeline on each supported forge, with registry distribution named as the target's own later step

Verify: `cargo nextest run -E 'test(nix)'`

### `packaging:the-consumer-pin-has-two-facts-and-one-mover` — The consumer pin has two facts and one mover

Where a consumer pins release-kit as a flake input, the binary MUST treat the tag in `flake.nix` as the version and the `release-kit` node in `flake.lock` as the content, and `rk self-depend sync` MUST move both in one run, because a tag without its lock is a promise the shell has not kept and nothing else in the tree may name an rk version. Where the wired manager records one fact, the sync MUST move that one fact in place and name the same `from` and `to` in the form the manager records, because the manager's own install takes the version from there and a second file would be a second pin.

#### Scenario: A sync moves the pin

- GIVEN a consumer whose pin is behind the latest release
- WHEN `rk self-depend sync --apply` runs
- THEN the tag in `flake.nix` and the locked node in `flake.lock` both name the new release, and the report names the same `from` and `to`

Verify: `cargo nextest run -E 'test(self_depend_sync_apply_rewrites_the_pin_updates_the_lock_and_builds) or test(self_depend_sync_moves_a_mise_pin)'`

### `packaging:the-pin-is-read-through-a-manager-axis` — The pin is read through a manager axis

The binary MUST read how a target obtains `rk` through one manager enum shared with `rk depend`, and `rk self-depend status` MUST report one entry per manager in the enum's order, absent ones included, each naming whether its file mentions release-kit and the version it records, because `rk` offers every other project through a four-manager matrix and a manager list restated in prose drifts at the first new manager. The binary MUST report `.envrc` outside the list, because direnv loads a shell and pins nothing: a host install under an `.envrc` is a reported state with no version to move, never a manager. The status MUST exit 0 for every state it reports, because the verb has no `--check` mode and a report is not a verdict, the same split `landing:status-judges-only-under-check` draws. A new manager is one variant plus its rows in each matrix, and no skill or chapter restates the list.

#### Scenario: A mise target carries no flake

- GIVEN a target whose `mise.toml` pins release-kit and which carries no `flake.nix`
- WHEN `rk self-depend status --json` runs
- THEN the mise entry reports `pinned` with its version, the flake entry reports `absent` rather than a fault, `wired` names mise, and the run exits 0

Verify: `cargo nextest run -E 'test(every_manager_in_the_enum_is_detected_once) or test(a_target_with_one_manager_chooses_it) or test(the_envrc_is_reported_outside_the_manager_list) or test(one_detection_serves_both_verbs)'`

### `packaging:the-venue-and-the-manager-cross-in-one-matrix` — The venue and the manager cross in one matrix

The binary MUST hold the venues `rk` publishes to as one enum, crossed with the manager enum in one matrix that classifies every pair as a rendered fragment or as manual with a reason from a closed set, because the venue and the manager are two axes and a venue no manager can consume is an honest row rather than a missing feature. A venue MUST enter the enum only where this repository's own release publishes to it under CI, the same claim `packaging:an-advertised-system-is-a-proven-system` makes for a system, and a venue that no per-project manager can pin, such as a system package built by a distribution service, MUST be recorded by its reason alone and take no variant. Every venue in the enum MUST carry a dated citation from that venue's own documentation in `_docs/reference/REFERENCE-packaging-sources.md`. A new venue is one variant plus its rows, and no fragment is a source literal: every text a pair renders is a file under `blocks/`.

#### Scenario: A pair the matrix cannot wire

- GIVEN a target on asdf, for which no published plugin installs release-kit
- WHEN `rk self-depend add --manager asdf` runs
- THEN the report names the pair `manual` with the reason `asdf-plugin-unknown`, nothing is written, and no plugin name is invented

Verify: `cargo nextest run -E 'test(every_pair_in_the_matrix_is_classified_once) or test(every_manual_pair_names_a_closed_reason) or test(the_asdf_rows_are_manual_with_their_reason) or test(every_venue_names_a_dated_citation) or test(no_fragment_is_a_source_literal)'`

### `packaging:a-pin-bump-is-all-or-nothing` — A pin bump is all or nothing

Where any step of a bump fails — the pin rewrite, the lock refresh, the system probe, or the build that fences it — the binary MUST return both files to their previous contents and name the failing step; an interrupted run MUST recover on the next run from its marker, because the crate forbids the signal handler a shell trap would need, and that is the one departure from the shell version.

#### Scenario: The build fails against the consumer's nixpkgs

- GIVEN a pin that does not build against the consumer's own nixpkgs
- WHEN the sync reaches the build step
- THEN both files are byte-identical to what they held before, the report names `build` as the failed step, and the next run finds no marker

Verify: `cargo nextest run -E 'test(a_failed_devshell_build_restores_both_files) or test(an_interrupted_transaction_is_recovered_on_the_next_run)'`

### `packaging:the-unattended-caller-never-fails-the-shell` — The unattended caller never fails the shell

Under `--caller envrc`, every reported outcome of `rk self-depend sync` MUST exit 0 with the outcome in the report, because the line runs on every directory entry and a shell that refuses to start over a stale pin is worse than one that says so.

#### Scenario: The network is down on directory entry

- GIVEN a consumer entering the directory with no network
- WHEN the `.envrc` line runs
- THEN the run reports `unreachable`, writes nothing, and exits 0, and the shell starts

Verify: `cargo nextest run -E 'test(every_envrc_path_exits_zero)'`

### `packaging:add-serves-a-fragment-and-edits-no-owned-file` — Add serves a fragment and edits no owned file

`rk self-depend add` MUST print the fragments for one manager and venue pair with their anchors and placements and MUST NOT edit a manager file or an `.envrc` the target owns, seeding a file only where the target has none, because a lexical observation does not justify a write into another project's manager file and the splice into Nix attrsets was rejected by decision. The `.envrc` MUST be seeded for the flake pair alone, because `use flake` is what puts `rk` on the path there and no other manager loads through direnv by default; every other pair prints the sync line for the operator to place.

#### Scenario: A target owns its manager file

- GIVEN a target with a `flake.nix`, a `mise.toml`, or a `devbox.json` of its own
- WHEN `rk self-depend add --apply` runs for that manager
- THEN the file is byte-identical, the run exits 73 naming the reason, and the fragments still print for the operator or the agent to apply

Verify: `cargo nextest run -E 'test(self_depend_add_apply_refuses_a_manager_file_the_target_owns) or test(self_depend_add_apply_refuses_every_owned_manager_file) or test(self_depend_add_apply_seeds_a_manager_file_the_target_lacks)'`

### `packaging:a-wired-target-runs-one-bump-mechanism` — A wired target runs one bump mechanism

The binary MUST report a target as `ready` only where the pin is wired and no artifact of a predecessor bump mechanism remains, because two mechanisms over the same two files fight or silently undo each other and the wiring is a replacement, never an addition. `rk self-depend add` MUST refuse a manager other than the one already naming release-kit, because two managers naming one tool are two pins.

#### Scenario: A hand-rolled bump sits beside a wired pin

- GIVEN a target whose `flake.nix` carries the pin and whose `scripts/` still holds the hand-rolled bump
- WHEN `rk self-depend status` runs
- THEN the state is `superseded` and the leftovers list names the script, and `ready` follows only once the list is empty

Verify: `cargo nextest run -E 'test(self_depend_status_names_a_predecessor_mechanism_beside_a_wired_pin) or test(a_clean_target_reports_ready_and_an_empty_manual_list) or test(self_depend_add_refuses_a_second_bump_mechanism)'`

### `packaging:the-cleanup-removes-only-what-it-can-judge` — The cleanup removes only what it can judge

`rk self-depend clean` MUST remove a catalog file only when its content matches, rewrite only the `.envrc` line the verb owns, and, where it leaves a leftover in place, name its file, its line, and the reason with the file byte-identical, because a recipe body, a Nix package list, and a CI step carry structure a line scan cannot judge.

#### Scenario: The justfile recipe stays

- GIVEN a target whose justfile carries the hand-rolled bump recipe
- WHEN `rk self-depend clean --apply` runs
- THEN the justfile is byte-identical and the report's `manual` list names it by file, line, and reason

Verify: `cargo nextest run -E 'test(self_depend_clean_apply_leaves_the_justfile_and_the_flake_and_names_them) or test(a_catalog_file_matches_on_its_content_and_not_on_its_name_alone)'`
