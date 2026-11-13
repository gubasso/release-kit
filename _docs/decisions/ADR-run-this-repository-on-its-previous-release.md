# Run this repository on its previous release

## Context and Problem Statement

This repository put `target/debug` on `PATH` and kept rk out of its devshell. The reason was stale sources: a binary embeds the sources it was built from, so an older rk judged the current snippets against old copies and reported false drift. The cost was that the rk under development also managed the repository's own landing. After release-plz bumps `Cargo.toml`, that binary is newer than `.release-kit/manifest.json` until a later commit takes the landing, and the version gate refuses in that window.

## Considered Options

- `Run a pinned earlier release` — chosen.
- `Build rk here and pin none, as before` — rejected: the gate refuses after each release.
- `Exempt this repository from the gate` — rejected: an exemption keyed on one repository is a second code path that no consumer runs.

## Decision Outcome

Chosen option: `run a pinned earlier release`. The devshell carries the rk of a `release-kit` flake input at a released tag. That rk runs the hooks, the worktree verbs, the integration, and the repository's own landing. The rk built from the tree runs only where the tree is under test, and those callers name `target/debug/rk` or `cargo run`. After a release publishes, one change moves the pin and runs `rk upgrade`, as in every consumer.

Compilers work this way. rustc builds with its stage0 compiler, "by default the very recent beta rustc" (rustc-dev-guide, read 2026-11-13). Go builds with an earlier Go from `GOROOT_BOOTSTRAP`, for example Go 1.22 for Go 1.24 (go.dev/doc/install/source, read 2026-11-13).

The pinned binary judges only its own landing against its own sources, and the tests judge the tree's snippets. That answers the stale-source reason.

Enforced by `distribution:this-repository-runs-its-previous-release`.

## Consequences

- Good: this repository meets every rule that a consumer meets, the version gate included.
- Bad: a change to the landing reaches this repository one release late.
- Bad: each pin move nests the pinned release's own `release-kit` input in `flake.lock`, until the self-pin input sets `inputs.release-kit.follows = ""`.

## Status

Accepted
