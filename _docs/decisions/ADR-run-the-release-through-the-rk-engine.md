# Run the release through the rk engine

## Context and Problem Statement

No upstream tool keeps one release request on both GitHub and GitLab and also keeps `uv.lock` in step with the version, and the bash pairs carried their release logic as untyped shell in two templates ([the comparison](../comparison-docs/COMPARISON-release-tools-for-a-uv-project.md)). The python pair on GitLab had no release path at all.

## Considered Options

- `The release engine in rk` — chosen.
- `An upstream tool per forge, release-please and releaser-pleaser` — rejected: two tools with two configurations, both blind to `uv.lock`, and releaser-pleaser has one maintainer.
- `A tool that releases on push, such as python-semantic-release, commitizen, or semantic-release` — rejected: `ADR-gate-the-release-on-one-pull-request.md` gates every release on one request.
- `A shell template per pair` — rejected: untyped logic in four copies, tested only by string matches.
- `A second binary` — rejected: a second pin, a second package, and a second install path for a small gain in size.

## Decision Outcome

Chosen option: the release engine in `rk` — the binary already reads the record, the tags, the release pages, and the registries, so the engine adds the writes and keeps one pin and one package.

The engine serves each `(driver, forge)` pair that no upstream tool covers completely, through a driver adapter that writes the version with the ecosystem's own tool. An upstream tool serves a pair where it is complete, so rust keeps release-plz. The actions mirror release-plz's split: `rk release request` keeps the request, and `rk release build` and `rk release publish` release around the provenance and the registry upload that stay in CI.

Enforced by `release-engine:one-request-per-base` and `release-engine:a-version-releases-once`.

## Consequences

- Good: python on both forges and bash on both forges share one model, one test suite, and one set of guards.
- Good: the request relocks `uv.lock` with the bump, because `uv version` writes both.
- Bad: release-kit owns release logic that an upstream tool would otherwise maintain.
- Bad: a release job needs the `rk` binary at the version that landed the workflow.

## Status

Implemented: [`src/release_engine.rs`](../../src/release_engine.rs).
