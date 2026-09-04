# Wrap the Hard tools and not the forge CLIs

## Context and Problem Statement

The packaged `rk` launches `git` from nine production sites and `sh` from its setup and probe paths, yet the derivation shipped no runtime closure: `nix run github:gubasso/release-kit` on a host with only Nix produced a binary that finds no git. The question is which external tools ride in the package's wrapper and which stay the operator's.

## Considered Options

- Wrap the Hard probe class only — `git`, and `bash` supplying `sh` — chosen.
- Wrap every probed tool including `gh`, `glab`, `curl`, `cosign`, `openssl`, `pypi-attestations` — rejected: the soft tools multiply the closure for capabilities most invocations never reach, `rk doctor` reports each with a repair line, and `nix shell nixpkgs#gh` covers the rest.
- Ship no wrapper and document git as a prerequisite — rejected: the first consumable tag would hand out a tool that is dishonest about its own needs, and the probe registry already classifies git's absence as Hard.
- Derive the wrapper list from `src/probes.rs` at build time — rejected: Nix cannot read the Rust registry, and a generator is more machinery than two entries earn; the mirror test holds the two hand-kept lists to agreement instead.

## Decision Outcome

Chosen option: wrap exactly the Hard tools with `makeBinaryWrapper` and a `--suffix PATH`, so an operator's own git wins, `RK_GIT_BIN` and `RK_SH_BIN` override the wrapped binaries, and a compiled exec wrapper serves the git-hook call path where a shell trampoline would not.

Enforced by `packaging:the-wrapper-carries-the-hard-tools`, `packaging:the-derivation-mirrors-the-probe-registry`, and `packaging:a-launcher-resolves-through-one-owner`.

## Consequences

- Good: a PATH-less invocation of the built package passes the git and sh probes, and every launcher honors one override owner.
- Bad: the wrapper list and the registry are two hand-kept mirrors; the mirror test is what keeps that safe, and it must extend with any tool that joins the Hard class.

## Status

Implemented: `src/probes.rs`, `nix/package.nix`, the smoke check in `flake.nix`.
