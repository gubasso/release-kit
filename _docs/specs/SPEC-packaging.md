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
  - [`packaging:a-devshell-bump-is-all-or-nothing` — A devshell bump is all or nothing](#packaginga-devshell-bump-is-all-or-nothing--a-devshell-bump-is-all-or-nothing)
  - [`packaging:the-unattended-caller-never-fails-the-shell` — The unattended caller never fails the shell](#packagingthe-unattended-caller-never-fails-the-shell--the-unattended-caller-never-fails-the-shell)
  - [`packaging:add-serves-a-template-and-edits-no-owned-flake` — Add serves a template and edits no owned flake](#packagingadd-serves-a-template-and-edits-no-owned-flake--add-serves-a-template-and-edits-no-owned-flake)
  - [`packaging:a-wired-target-runs-one-bump-mechanism` — A wired target runs one bump mechanism](#packaginga-wired-target-runs-one-bump-mechanism--a-wired-target-runs-one-bump-mechanism)
  - [`packaging:the-cleanup-removes-only-what-it-can-judge` — The cleanup removes only what it can judge](#packagingthe-cleanup-removes-only-what-it-can-judge--the-cleanup-removes-only-what-it-can-judge)

<!--TOC-->

## Purpose

Rules governing the Nix packaging surface of this repository: the flake outputs, the package expression under `nix/`, the CI proof behind the support claim, and the consumer half — how a project pins that flake as its devshell dependency through `rk devshell` and keeps the pin fresh. The boundary against `SPEC-distribution.md` is the artifact: that spec binds what the installed `rk` binary carries and writes, and this one binds how a consumer obtains that binary through the flake. The files `rk init` lands into a target are bound by `SPEC-landing.md`.

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

The landed Nix capability MUST promise exactly a package expression that evaluates for the supported crate shape, a flake that builds where the seed pair landed, and a CI check that proves that build — and MUST NOT promise presence in nixpkgs or any registry, because a registry submission carries a human maintainer commitment into someone else's repository. The support matrix degrades honestly: a pair that lands fewer files reports the smaller product, never an error.

#### Scenario: An operator asks what the capability shipped

- GIVEN a target that opted in with `rk init --nix`
- WHEN the operator reads the landing report and the runbook
- THEN the promise is the build and its proof, with registry distribution named as the target's own later step

Verify: `cargo nextest run -E 'test(nix)'`

### `packaging:the-consumer-pin-has-two-facts-and-one-mover` — The consumer pin has two facts and one mover

Where a consumer pins release-kit as a flake input, the binary MUST treat the tag in `flake.nix` as the version and the `release-kit` node in `flake.lock` as the content, and `rk devshell sync` MUST move both in one run, because a tag without its lock is a promise the shell has not kept and nothing else in the tree may name an rk version.

#### Scenario: A sync moves the pin

- GIVEN a consumer whose pin is behind the latest release
- WHEN `rk devshell sync --apply` runs
- THEN the tag in `flake.nix` and the locked node in `flake.lock` both name the new release, and the report names the same `from` and `to`

Verify: `cargo nextest run -E 'test(devshell_sync_apply_rewrites_the_pin_updates_the_lock_and_builds)'`

### `packaging:a-devshell-bump-is-all-or-nothing` — A devshell bump is all or nothing

Where any step of a bump fails — the pin rewrite, the lock refresh, the system probe, or the build that fences it — the binary MUST return both files to their previous contents and name the failing step; an interrupted run MUST recover on the next run from its marker, because the crate forbids the signal handler a shell trap would need, and that is the one departure from the shell version.

#### Scenario: The build fails against the consumer's nixpkgs

- GIVEN a pin that does not build against the consumer's own nixpkgs
- WHEN the sync reaches the build step
- THEN both files are byte-identical to what they held before, the report names `build` as the failed step, and the next run finds no marker

Verify: `cargo nextest run -E 'test(a_failed_devshell_build_restores_both_files) or test(an_interrupted_transaction_is_recovered_on_the_next_run)'`

### `packaging:the-unattended-caller-never-fails-the-shell` — The unattended caller never fails the shell

Under `--caller envrc`, every reported outcome of `rk devshell sync` MUST exit 0 with the outcome in the report, because the line runs on every directory entry and a shell that refuses to start over a stale pin is worse than one that says so.

#### Scenario: The network is down on directory entry

- GIVEN a consumer entering the directory with no network
- WHEN the `.envrc` line runs
- THEN the run reports `unreachable`, writes nothing, and exits 0, and the shell starts

Verify: `cargo nextest run -E 'test(every_envrc_path_exits_zero)'`

### `packaging:add-serves-a-template-and-edits-no-owned-flake` — Add serves a template and edits no owned flake

`rk devshell add` MUST print the fragments with their anchors and placements and MUST NOT edit a `flake.nix` or `.envrc` the target owns, seeding a file only where the target has none, because a lexical observation does not justify a write into another project's Nix file and the splice into Nix attrsets was rejected by decision.

#### Scenario: A target owns its flake

- GIVEN a target with a `flake.nix` of its own
- WHEN `rk devshell add --apply` runs
- THEN the flake is byte-identical, the run exits 73 naming the reason, and the fragments still print for the operator or the agent to apply

Verify: `cargo nextest run -E 'test(devshell_add_apply_refuses_a_flake_the_target_owns)'`

### `packaging:a-wired-target-runs-one-bump-mechanism` — A wired target runs one bump mechanism

The binary MUST report a target as `ready` only where the pin is wired and no artifact of a predecessor bump mechanism remains, because two mechanisms over the same two files fight or silently undo each other and the wiring is a replacement, never an addition.

#### Scenario: A hand-rolled bump sits beside a wired pin

- GIVEN a target whose `flake.nix` carries the pin and whose `scripts/` still holds the hand-rolled bump
- WHEN `rk devshell status` runs
- THEN the state is `superseded` and the leftovers list names the script, and `ready` follows only once the list is empty

Verify: `cargo nextest run -E 'test(devshell_status_names_a_predecessor_mechanism_beside_a_wired_pin) or test(a_clean_target_reports_ready_and_an_empty_manual_list)'`

### `packaging:the-cleanup-removes-only-what-it-can-judge` — The cleanup removes only what it can judge

`rk devshell clean` MUST remove a catalog file only when its content matches, rewrite only the `.envrc` line the verb owns, and, where it leaves a leftover in place, name its file, its line, and the reason with the file byte-identical, because a recipe body, a Nix package list, and a CI step carry structure a line scan cannot judge.

#### Scenario: The justfile recipe stays

- GIVEN a target whose justfile carries the hand-rolled bump recipe
- WHEN `rk devshell clean --apply` runs
- THEN the justfile is byte-identical and the report's `manual` list names it by file, line, and reason

Verify: `cargo nextest run -E 'test(devshell_clean_apply_leaves_the_justfile_and_the_flake_and_names_them) or test(a_catalog_file_matches_on_its_content_and_not_on_its_name_alone)'`
