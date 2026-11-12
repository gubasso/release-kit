# Call git-cliff as a pinned binary

## Context and Problem Statement

The release engine needs the next version from Conventional Commits and a changelog section for each release. git-cliff computes both, and the bash pipelines already install it at a pin. It ships as a binary and as the `git-cliff-core` library.

## Considered Options

- `The git-cliff binary at its versions.toml pin` — chosen.
- `Link git-cliff-core into rk` — rejected: it pulls libgit2 and the library's dependency tree into a binary whose `Cargo.toml` lists 14 dependencies on 2026-11-11.
- `Compute the bump and the changelog in rk` — rejected: a second implementation of a convention git-cliff already reads from the landed `cliff.toml`.

## Decision Outcome

Chosen option: the git-cliff binary at its pin — `rk` keeps its short dependency list, and the landed `cliff.toml` stays the one configuration of the changelog.

The engine reads the pin from the embedded `versions.toml`, compares it with `git-cliff --version`, and refuses another version. It never passes `--bump`: it reads `--bumped-version` as a proposal, decides the number, and writes the changelog with `--tag`. Where the target records a development environment, the environment carries git-cliff at the pin and the uv the target requires, as it carries `rk`, release-plz, and `dist`. A job then installs neither tool beside the environment, and the pin check refuses an environment that drifted.

Enforced by `release-engine:git-cliff-runs-at-its-pin`.

## Consequences

- Good: the binary's dependency tree does not grow, and the changelog moves only with the registry row.
- Good: a recorded environment stays the project's one source of its tools.
- Bad: CI installs one more pinned binary where no environment is recorded.
- Bad: the engine parses git-cliff's output, so a change in that output is a change the engine must follow.

## Status

Implemented: [`src/release_engine/cliff.rs`](../../src/release_engine/cliff.rs).
