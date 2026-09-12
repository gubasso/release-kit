# Cross the venue and the manager in one matrix

## Context and Problem Statement

`rk` publishes itself three ways: the crate, the flake at every tag, and the archives on a GitHub release. A consumer obtains it through one of four tool managers, and `rk self-depend add` served the flake pair alone. Two questions are arguable: whether the venue is a second axis or a list of supported pairs, and what a pair no manager can consume reports.

## Considered Options

- Two enums crossed in one matrix, every pair a rendered fragment or manual with a closed reason — chosen.
- One list of supported pairs — rejected: a venue no manager consumes has no row, so the gap goes unreported.
- Importing the dependency matrix's classifier — rejected: its reasons are about an arbitrary source project, and release-kit's crate, flake output, and archives are known.
- A venue enters the enum on request — rejected: a venue is a support claim, and `packaging:an-advertised-system-is-a-proven-system` binds a claim to a CI proof.
- A system package venue as a variant now — rejected: no per-project manager pins a system package inside a repository, so every pair is manual and only the reason is recorded.

## Decision Outcome

Chosen option: two axes, one matrix, every pair classified once. A manual pair names its reason from a closed set: no nixpkgs attribute, asdf plugin, or source hash is invented. A venue joins the enum only where this repository's own release publishes to it under CI, with a dated citation. Every fragment renders from a file under `blocks/`. A target naming release-kit through one manager refuses a second.

Enforced by `packaging:the-venue-and-the-manager-cross-in-one-matrix`, `packaging:add-serves-a-fragment-and-edits-no-owned-file`, and `packaging:a-wired-target-runs-one-bump-mechanism`.

## Consequences

- Good: a target on mise or devbox wires and moves its `rk` pin; a pair the binary cannot wire says why; a new venue is one variant plus its rows.
- Bad: the add and sync schemas moved to their second versions; only the flake pair seeds an `.envrc`.

## Status

Implemented: `src/self_depend/venue.rs`, `src/self_depend/matrix.rs`, `src/self_depend/fragments.rs`, `src/commands/self_depend.rs`, `blocks/self-depend-*.in`.
