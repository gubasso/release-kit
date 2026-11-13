# Open a release request only for packed changes

## Context and Problem Statement

The engine proposed `git-cliff --bumped-version` since the last tag, so a `build:` commit opened a request. Cargo's [`include` and `exclude`](https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields) declare what a crate ships, and release-plz 0.4.3 counts only commits that touch a packed file ([updater](https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/command/update/updater.rs)).

## Considered Options

- `One declaration drives artifact and trigger; a releasing commit gates it` — chosen.
- `Skip commit types in cliff.toml` — rejected: a type does not say what ships.
- `A path filter alone` — rejected: reverts count, and a packed `build:` releases.
- `A recorded path list` — rejected: the package is described twice.
- `The artifact of a recorded build` — rejected: requests build twice.
- `release-plz's release_commits key` — rejected: a lock-only update carries only a synthetic `chore:` line, so a security fix never releases.

## Decision Outcome

Chosen option: Cargo's model, with two conditions. The packed change says what ships; the type says whether it is worth a release. This replaces the rejection of a type filter, and the rule that a changed set releases through the commit editing `.release-kit/config.toml`.

Bash declares `include` and `exclude` as Cargo does; python lists its sdist. A request opens only where the packed set or a packed file differs since the tag, and a commit that touched them, or the declaration that changed them, is `feat`, `fix`, `perf`, or breaking. Other commits stay in the changelog.

A rust request job runs `rk release eligible` before release-plz; `release-plz.toml` gains no key. Limits: release-plz counts a lock change only where a released entry moves, so an added entry can pass rk yet open nothing; its changelog shows a synthetic lock line; rk counts `Cargo.lock` only for binaries.

An advisory takes a releasing type wherever a commit reaches the trunk: the title check, and under rebase the commit checks.

Departure from release-plz: rk's own files never count.

Enforced by `release-engine:a-request-follows-the-packed-files`, `release-engine:a-request-needs-a-releasing-commit`, and `release-engine:a-rust-target-releases-one-package`.

## Consequences

- Good: an outside, reverted, or non-releasing change opens no request.
- Bad: bash adopters rewrite recorded build commands; python request jobs build twice.

## Status

Implemented: [`src/release_engine/packed.rs`](../../src/release_engine/packed.rs) and [`src/release_engine/eligible.rs`](../../src/release_engine/eligible.rs).
