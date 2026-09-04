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

<!--TOC-->

## Purpose

Rules governing the Nix packaging surface of this repository: the flake outputs, the package expression under `nix/`, and the CI proof behind the support claim. The boundary against `SPEC-distribution.md` is the artifact: that spec binds what the installed `rk` binary carries and writes, and this one binds how a consumer obtains that binary through the flake. The files `rk init` lands into a target are bound by `SPEC-landing.md`.

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
