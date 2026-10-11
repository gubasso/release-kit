# Land no other project into a target

## Context and Problem Statement

`rk depend` took any other project as a dependency of a target: it read the source's channels and the target's tool manager, then served fragments for one of four managers. That made release-kit the owner of how every other project lands in its consumers. The source project owns its channels and changes them without telling release-kit, so a matrix kept here goes stale at the source's next release.

## Considered Options

- `remove the dependency feature whole, and keep the consumer pin` — chosen.
- `keep the verbs and drop the skill` — rejected: the verbs still claim knowledge the source owns.
- `move the matrix into a separate tool` — deferred: revisit if several projects ask for one shared installer that they feed.

## Decision Outcome

Chosen option: `remove the dependency feature whole, and keep the consumer pin`. A project owns how it lands its setup and its configuration in a consumer. The skill, the verbs, their modules, their blocks, the chapter, the runbook, the spec, and the source reference leave together, so no surface names a verb the binary no longer has.

`rk self-depend` stays, because there release-kit is the other project: it lands its own pin into a consumer, which is the principle this record states. It shares no code with the removed modules.

Chapter number 11 stays unused, because renumbering breaks every link to chapters 12 and 13.

Enforced by no spec rule. A test in `tests/cli.rs` finds no live surface naming the feature outside the decision records and the changelog.

## Consequences

- Good: about 3,700 lines of Rust, thirteen blocks, and a skill leave.
- Bad: a target that used the verb now takes a dependency through its manager's own documentation.

## Status

Implemented: `src/cli.rs`, `src/commands.rs`, `src/main.rs`, `tests/cli.rs`.
