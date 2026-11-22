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
  - [`packaging:the-operator-selects-the-installed-version` — The operator selects the installed version](#packagingthe-operator-selects-the-installed-version--the-operator-selects-the-installed-version)
  - [`packaging:a-landed-job-takes-rk-by-one-route` — A landed job takes rk by one route](#packaginga-landed-job-takes-rk-by-one-route--a-landed-job-takes-rk-by-one-route)

<!--TOC-->

## Purpose

Rules governing the Nix packaging surface of this repository: the flake outputs, the package expression under `nix/`, the CI proof behind the support claim, and the consumer half — how a project obtains `rk` through the tool manager it already runs, by that manager's own commands. The boundary against `SPEC-distribution.md` is the artifact: that spec binds what the installed `rk` binary carries and writes, and this one binds how a consumer obtains that binary. The files `rk init` lands into a target are bound by `SPEC-landing.md`.

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

The package build MUST fail naming the path when its source omits a root declared in `src/distribution_roots.rs` or a license file `src/embedded.rs` embeds, because a filtered source that drops a root still produces a binary that builds and lies.

#### Scenario: A source filter is narrowed later

- GIVEN a package expression whose source filter newly omits a distribution root
- WHEN `nix build` runs
- THEN the build fails naming the missing root, before any smoke command that would never notice

Verify: `grep -q 'src/distribution_roots.rs' nix/package.nix`

### `packaging:an-advertised-system-is-a-proven-system` — An advertised system is a proven system

Where the flake advertises a system, CI MUST natively build and run the flake's checks on that system, because an output set is a support promise and a cross-evaluated check builds nothing. The one system this repository advertises, proves, and releases for is x86_64 Linux: `x86_64-linux` as the flake system and `x86_64-unknown-linux-gnu` as the release target, with the shell installer as the one installer. Every flake and release declaration `rk` lands or seeds MUST default to that same one system, because a seed advertises what the landed proof natively runs, and `rk` MUST NOT generate, test, or claim another platform; a project widens its own files after landing (`ADR-linux-is-the-only-supported-target`).

#### Scenario: A system joins the flake's list without a runner

- GIVEN a change that adds a system the CI matrix does not run natively
- WHEN the change is reviewed
- THEN either a native runner joins the matrix in the same change or the system stays out of the list

Verify: `cargo nextest run -E 'test(a_fresh_rust_landing_advertises_only_x86_64_linux)'`

### `packaging:the-checks-carry-the-nix-side-signal` — The checks carry the Nix-side signal

The flake's `checks` MUST build the package and smoke what the binary serves, because `nix flake check` builds only the `checks` output and the crate's test suite, which drives real git and forge CLIs the sandbox lacks, stays out of the package build.

#### Scenario: A served-content regression survives the build

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

Production code MUST NOT launch `git`, `sh`, `git-cliff`, or `uv` by literal name outside the shared resolvers in `src/probes.rs`, because a direct launch bypasses the `RK_GIT_BIN`, `RK_SH_BIN`, `RK_GIT_CLIFF_BIN`, and `RK_UV_BIN` overrides silently and hides a runtime dependency from the registry the wrapper mirrors.

#### Scenario: A new call site launches git directly

- GIVEN a change that adds a `Command::new("git")` outside the resolver
- WHEN the test suite runs
- THEN the scan test fails naming the file and line

Verify: `cargo nextest run -E 'test(every_git_and_sh_launch_resolves_through_the_shared_resolver)'`

### `packaging:the-landable-capability-promises-a-buildable-flake` — The landable capability promises a buildable flake

The landed Nix capability MUST promise exactly a package expression that evaluates for the supported crate shape, a flake that builds where the seed pair landed, and a served job proving that build for every pair whose forge leaves the target a pipeline to add it to, and MUST NOT promise presence in nixpkgs or any registry, because a registry submission carries a human maintainer commitment into someone else's repository. The capability lands no CI file of its own: a job proving the build holds a merge only inside the pipeline the forge's gate reads, and that pipeline is the target's — the gated workflow on GitHub, and the child pipeline the rendered parent triggers on GitLab. The support matrix degrades honestly: a pair whose forge leaves the target no such pipeline reports the smaller product, never an error.

#### Scenario: An operator asks what the capability shipped

- GIVEN a target that opted in with `rk init --nix-packaging`
- WHEN the operator reads the landing report and the runbook
- THEN the promise is the build, and the job that proves it in the target's own pipeline on each supported forge, with registry distribution named as the target's own later step

Verify: `cargo nextest run -E 'test(nix)'`

### `packaging:the-operator-selects-the-installed-version` — The operator selects the installed version

The operator and the target's tool manager MUST choose the installed `rk` version before any staging or landing begins, and `rk stage`, `rk init`, `rk upgrade`, `rk adopt`, and `rk status` MUST run from the installed binary alone, with no fetch, resolution, or execution of another release, because acquisition is the manager's workflow, and a landing that reaches for another release is a second installer inside the tool.

#### Scenario: A target two releases behind is staged and landed offline

- GIVEN a target whose receipt names an older `rk_version`, a newer `rk` the operator installed through the target's manager, and no network
- WHEN `rk stage` and then `rk upgrade --apply` run
- THEN both complete from the installed binary, no registry, forge, or cache is consulted, and the receipt names the installed version

Verify: `cargo nextest run -E 'test(stage_and_landing_need_no_release_resolution_network_access)'`

### `packaging:a-landed-job-takes-rk-by-one-route` — A landed job takes rk by one route

Where a landed job runs `rk`, the job MUST take it from the recorded development environment when the target records one that carries `rk`, and MUST otherwise install it through the shell installer of the release whose version equals the landing binary's own, because a recorded environment is the project's one source of its tools, and the landing binary is the one version known to carry every verb its own jobs call. A recorded `dev_environment.rk = "installer"` states that the environment carries no `rk`: the job MUST then take the recorded setup, install `rk` at the landing version into a directory on the job's PATH, and still run `rk` through the run prefix, which keeps that PATH, so every tool `rk` calls comes from the environment. A job whose environment carries `rk` names no `rk` version, and the binding states that the environment must carry it.

#### Scenario: A target with no recorded environment is landed by a new release

- GIVEN a target with no development environment, landed by the `rk` release that first carries the minting verb
- WHEN the landing renders the job that mints a candidate
- THEN the job fetches `release-kit-installer.sh` from that same release, and `versions.toml` names no `rk` version

#### Scenario: A recorded environment carries no rk

- GIVEN a GitHub rust target whose configuration records `run = "nix develop --command"`, a setup step, and `rk = "installer"`
- WHEN `rk upgrade --apply` renders the job that mints a candidate
- THEN the job takes the setup step, installs `rk` at the landing version and appends its directory to `$GITHUB_PATH`, and runs `nix develop --command rk lines mint`

Verify: `cargo nextest run -E 'binary(cli)'`
