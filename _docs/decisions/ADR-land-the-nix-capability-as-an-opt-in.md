# Land the Nix capability as a recorded opt-in

## Context and Problem Statement

release-kit packages itself through its flake; the capability becomes something `rk init` lands into targets. Three questions: is it on by default, what happens when a target already has a flake, and what does it promise?

## Considered Options

- A recorded `--nix` opt-in, off by default, with an all-or-nothing flake pair — chosen.
- On by default — rejected: a packaging surface is a decision, and `projection()` derives the file set from recorded parameters; an unrecorded default would make `status` unable to tell absent-because-not-wanted from drifted.
- Landing the seed pair beside an existing flake — rejected: the seed lock describes the seed's input graph, wrong from the first evaluation, and a rendered workflow checking a flake release-kit did not author is a green badge proving nothing.
- Splicing `packages.default` into an existing flake — rejected: the block-splice mechanism handles line-structured files, not Nix attrsets.
- Promising nixpkgs presence — rejected: a registry submission drags a human maintainer commitment into someone else's repository; distribution tiers beyond the flake are the target's own later step.

## Decision Outcome

`--nix` on `init` and `adopt`, `--nix`/`--no-nix` on `upgrade`, recorded in the manifest (schema 4; an older record reads as opt-out). The matrix: `(rust, github)` full; `(rust, gitlab)` the expression and the pair without a CI job; other technologies land nothing — each smaller product reported, never an error. Where the target carries a `flake.nix` or `flake.lock`, the pair and the workflow are withheld with the reason named and the seeded `nix/package.nix` still lands; a crate shape the seed does not support (a workspace root, no `[package]`) withholds everything by name.

Enforced by `landing:the-nix-capability-is-a-recorded-opt-in`, `landing:the-flake-pair-lands-all-or-nothing`, and `packaging:the-landable-capability-promises-a-buildable-flake`.

## Consequences

- Good: every verb reconstructs the intended file set from the record; a target's own flake is never disturbed; the promise is scoped to what CI can prove.
- Bad: a target with its own flake integrates `nix/package.nix` by hand; the capability's proof arrives only with the rendered workflow.

## Status

Implemented: `src/landing.rs`, the landing verbs, `snippets/rust/*/`.
