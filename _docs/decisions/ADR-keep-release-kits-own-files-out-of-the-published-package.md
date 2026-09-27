# Keep release-kit's own files out of the published package

## Context and Problem Statement

release-plz attributes a commit to a crate only where the commit changes a file `cargo package --list` prints, for the changelog and for the decision to release. Consumers shipped the landing record, the configuration, and landed blocks inside their crates, so every `rk upgrade` asked for a release with no code change, and a release-intent commit touching only excluded files reached neither the changelog nor a release, with nothing saying so.

## Considered Options

- `a proof-only package-check fault naming each packaged release-kit path and its exclude entry, plus an integrate warning` — chosen.
- `rk edits the consumer's Cargo.toml` — rejected: the target owns that file, and release-kit edits no owned manifest.
- `a satisfied result carrying a limitation` — rejected: a note does not stop the release it names, and an apply drops limitations.
- `a release_commits filter in the seeded release-plz.toml` — rejected: it filters by message rather than by file, it changes which code changes release, and a seeded file reaches no existing target.
- `refusing the integration` — rejected: a commit the bot cannot see can be deliberate, so a warning is enough.

## Decision Outcome

Chosen option: `a proof-only package-check fault naming each packaged release-kit path and its exclude entry, plus an integrate warning` — the listing already answers what ships, so the check that reads it states the fault and its fix, and the verb that writes the trunk message warns while the operator can still retype it.

Enforced by `forge-setup:a-package-check-states-policy-reach` and `git:a-local-integration-warns-of-an-uncounted-release`.

## Consequences

- Good: a landing no longer releases a crate whose code did not change.
- Good: a release-intent commit the bot will not count is named before it reaches the trunk.
- Bad: every Rust consumer edits its `Cargo.toml` once, and that edit itself releases once.

## Status

Implemented; `src/cargo_package.rs`, `src/setup/observe.rs`, `src/commands/integrate.rs`, `bindings/rust.md`, and `guidance/0.8.5.md` enact it.
