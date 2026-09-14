# Linux on x86_64 is the only supported target

## Context and Problem Statement

The release built five archives and two installers, the flake advertised two systems, and every seeded flake copied the second system into the target. Nothing ran the macOS, Windows, or ARM binaries: no test, no smoke, no operator. The landing replacement that follows implements one filesystem contract, and every unsupported platform is a branch it would carry untested. The question is which platforms the product claims.

## Considered Options

- One target, `x86_64-unknown-linux-gnu` for release archives and `x86_64-linux` for Nix outputs, dogfooded here and landed as the one default into every Rust target — chosen.
- Keep the five-target release and the two-system flake as they stand — rejected: an artifact nobody runs is a support promise nobody keeps, and `packaging:an-advertised-system-is-a-proven-system` already forbids an output no runner proves.
- Keep the breadth and mark the untested platforms as unsupported source builds — rejected: an archive on the release page reads as support whatever the text beside it says.
- Narrow the release and leave the landed seeds wide — rejected: a seed advertises what the landed CI natively proves, and the landed workflow proves one runner.

## Decision Outcome

Chosen option: one target, because a platform the project cannot run is a claim it cannot keep, and the landing replacement is honest only when it implements the one environment it tests. The cut is a breaking support change. A project may widen its own `dist-workspace.toml` and flake after landing; `rk` neither generates nor tests another platform.

Enforced by `packaging:an-advertised-system-is-a-proven-system`.

## Consequences

- Good: one release archive, one shell installer, one Nix system, one CI proof, and no untested branch in the landing code that follows.
- Good: every seeded flake and dependency seed states the same one-system claim the repository itself proves.
- Bad: an operator on macOS, Windows, or ARM has no release artifact and no supported path.
- Bad: widening later costs a native runner per system and a new decision.

## Status

Implemented: `dist-workspace.toml`, `flake.nix`, `.github/workflows/ci.yml`, `snippets/rust/github/dist-workspace.toml`, `blocks/depend-seed-flake.nix.in`, `blocks/self-depend-seed-flake.nix.in`.
