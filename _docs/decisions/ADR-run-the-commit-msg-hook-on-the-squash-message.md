# Run the commit-msg hook on the squash message

## Context and Problem Statement

A project can hold commit length limits in its own `commit-msg` hook, such as `committed`. `rk integrate` writes the squash commit with `git commit-tree`, which fires no hook, and the landed `rk message` judgment has no length rule. So a squash title over the project's limit reached the trunk unjudged.

## Considered Options

- `run the clone's commit-msg hook on the squash message` — chosen.
- `read the limits from committed.toml and judge them in rk` — rejected: it follows one tool's file format, and every other commit-msg rule of the project stays unseen.
- `judge every commit of the rebase form too` — rejected: each branch commit already passed the hooks of its seat. A rule that the project added later would refuse a commit made before it, and only a rewrite could fix that.

## Decision Outcome

Chosen option: `run the clone's commit-msg hook on the squash message` — git runs whatever the project installed, so rk names no hook runner and reads no hook configuration, as the pre-integrate gate does.

Before anything moves, rk writes the message to a file and runs `git hook run --ignore-missing commit-msg -- <file>` in the seat. A clone with no such hook passes as before. A refusal relays the hook's output and names the line, its length, and the limit where that output carries them. A hook that rewrites the file refuses, because the trunk receives the message that rk judged. The rebase form runs no `commit-msg` hook.

Enforced by `git:the-pre-merge-commit-hook-is-the-pre-integrate-contract` and `git:a-local-integration-is-a-transaction`.

## Consequences

- Good: the squash message meets the hooks of every desk commit, with no copy of their rules in rk, and a refusal moves nothing.
- Bad: the hook runs in the caller's environment, so `rk integrate` runs from the seat's environment, where the hook finds its tools.
- Bad: a hook that edits the message, such as one that appends a trailer, refuses here. The operator gives the edited message with `-m`.

## Status

Implemented: [the integration](../../src/commands/integrate.rs) and [its chapter](../../method/12-integration.md).
