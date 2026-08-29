# Make the forge an axis

## Context and Problem Statement

`method/05-diff-surface.md` stated that only four axes vary between technologies, and none of them was the forge. Every landed workflow file assumes GitHub Actions. An unstated assumption becomes load-bearing the moment executable setup ships: a second forge would then have no place for its differences, and the chapter would be wrong rather than incomplete.

## Considered Options

- `Add a fifth axis, with GitHub and GitLab supported from the start` — chosen.
- `Assume one forge and say so` — rejected: it leaves a GitLab project no home for its differences and makes the axis a retrofit under shipped setup scripts, the expensive order.
- `Put forge differences inside each binding` — rejected: the forge is orthogonal to the technology, so every binding would restate every forge and one fact would gain as many owners as there are technologies.

## Decision Outcome

Chosen option: `add a fifth axis, with GitHub and GitLab supported from the start`. The four existing axes are technology answers; the forge varies independently of all of them, so a project's configuration is a `(technology, forge)` pair. The chapter carries the pair table, and an axis answer may be nothing: cargo-dist generates CI for GitHub Actions only, so `(rust, gitlab)` has no artifact builder, and `bindings/rust.md` states it rather than implying parity.

Three further GitLab findings bound what that side may promise, stated where each binds as the forge surface lands: tag protection stops accident but not an Owner or Maintainer, the merge gate is a project-wide pipeline requirement naming no check, and crates.io trusted publishing covers GitLab.com only.

## Consequences

- Good: a project's pair is stated, so a second forge is an added tree rather than a rewrite.
- Good: what a forge cannot enforce is canon, not a surprise during a first release.
- Bad: every setup step and workflow snippet must eventually exist per forge, roughly doubling that surface.

## Status

Accepted — the canon carries the axis; the executable forge setup and the rules enforcing step parity land with the setup work.
