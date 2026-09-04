# Advertise only natively proven systems in the flake

## Context and Problem Statement

A flake's per-system output set is a claim that those systems build. `nix flake check` on a Linux runner checks the current system only, and `--all-systems` evaluates the others without building or running them, so a four-system list backed by one runner is a promise nobody tests.

## Considered Options

- Advertise only the systems CI natively builds and smokes — chosen.
- Advertise all four default systems with a native runner matrix across Linux and macOS — deferred: revisit when a macOS runner joins CI and the tag gate can run natively there.
- Advertise all four and let a Linux `--all-systems` evaluation stand in — rejected: evaluation proves no build, and a broken Darwin package would ship inside a support claim.

## Decision Outcome

Chosen option: advertise only natively proven systems — the flake lists `x86_64-linux` and `aarch64-linux`, the two systems the forge's hosted runners build and smoke on every change, and the list grows only together with the CI matrix. A short list that builds beats a long one that evaluates; macOS and Windows consumers keep the dist archives and crates.io.

Enforced by `packaging:an-advertised-system-is-a-proven-system`.

## Consequences

- Good: every advertised system is one a red CI can actually defend, and the tag gate can be run natively on each.
- Bad: Darwin users get no flake output until a macOS runner joins CI, although the crate and the dist archives still cover them.

## Status

Implemented: the system list in `flake.nix`, the matrix in `.github/workflows/ci.yml`.
