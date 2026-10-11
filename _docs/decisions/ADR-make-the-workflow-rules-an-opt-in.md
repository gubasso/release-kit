# Make the workflow rules an opt-in

## Context and Problem Statement

Every landing wrote rules that only shape how a project works: an agent policy, the commit body and scope rules, a glossary of workflow terms, six hooks on commit messages, branch names, the working copy, and the trunk, and two setup steps that clean up merged branches. It also wrote lines telling the project how to integrate, start work, and lay out its worktrees. A release needs none of these, and a tool intervenes in a project's workflow as little as it can.

## Considered Options

- `one opt-in capability for every workflow rule, off for a fresh landing, with old records keeping them until answered` — chosen.
- `remove the workflow rules outright` — rejected: projects that run the guards today, this one among them, would lose them with no question asked.
- `one opt-in per rule` — rejected: the rules guard one workflow together, and nine answers ask more than the operator decides.

## Decision Outcome

Chosen option: `one opt-in capability for every workflow rule, off for a fresh landing, with old records keeping them until answered`. `git.workflow-guards` holds the policy lines, the glossary, the six hooks, and the two steps. `git.guards` keeps the routing lines, the scoped trunk title, the tag, workflow, status, and secret rules, and the tag and status hooks. The integration, issue, and worktree lines leave in every state, and the checkout mode stays a helper default that no landing asks for.

A record that predates the capability carries a pending answer that reads as kept. This is the one capability whose old records do not read as opt-out, because every such target carries the guards.

Enforced by `project-profile:the-workflow-rules-are-one-opt-in` and `landing:the-nix-capability-is-a-recorded-opt-in`.

## Consequences

- Good: a fresh landing writes only what the release needs, and the helper verbs stay.
- Bad: a project that relied on the removed lines writes its own, outside the markers.

## Status

Implemented: `src/profile/catalog.rs`, `src/projection.rs`, `src/commands/upgrade.rs`, `src/setup/steps.rs`, `blocks/`.
