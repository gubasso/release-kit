# Own the consumer pin in the binary

## Context and Problem Statement

A host install serves one `rk` to the whole machine and nothing keeps it fresh. The first consumer pinned release-kit as a flake input and bumped it from `.envrc` with 679 lines every next consumer would copy. Four questions are arguable: where the transaction lives, how it enters a flake, what an interruption does, and what becomes of a mechanism already in place.

## Considered Options

- The transaction in the `rk` binary, as `rk devshell`, with the wiring a replacement — chosen.
- A script landed into each consumer — rejected: tested nowhere, and copied everywhere.
- Splicing the fragments into a flake the target owns — rejected: a lexical scan does not justify a write into another project's Nix file; the printed fragments serve an agent as well.
- Trapping the signal to restore on interrupt — rejected: the crate forbids `unsafe`, so no handler exists; a durable marker and the next run's recovery cover the case.
- Coexisting with a predecessor mechanism — rejected: two mechanisms bump the same two files on the same trigger with two locks that do not know each other, so the second either fights the first or silently undoes it; no correct behavior exists to fall back on.

## Decision Outcome

Chosen option: the binary owns the transaction and the cleanup — one implementation, one test suite, one `.envrc` line per consumer, and ready means nothing of the predecessor is left.

The verb records nothing in `.release-kit/manifest.json`: it is not a landing verb and `.envrc` is not a landable kind, so `rk devshell status` is the reporter instead.

Enforced by `packaging:the-consumer-pin-has-two-facts-and-one-mover`, `packaging:a-devshell-bump-is-all-or-nothing`, `packaging:the-unattended-caller-never-fails-the-shell`, `packaging:add-serves-a-template-and-edits-no-owned-flake`, `packaging:a-wired-target-runs-one-bump-mechanism`, and `packaging:the-cleanup-removes-only-what-it-can-judge`.

## Consequences

- Good: a consumer keeps one line and three fragments; the transaction, lock, stamp, and fence are tested once.
- Bad: an owned flake takes the fragments by hand; an interrupted bump stays half-moved until the next entry; a broken pin waits a day unless the operator runs the sync.

## Status

Implemented: `src/devshell.rs`, `src/commands/devshell.rs`, `blocks/devshell-*`, `runbooks/setup.md`, `skills/rk-setup/SKILL.md`, `skills/rk-migrate/SKILL.md`.
