# Require a fresh branch before a merge

## Context and Problem Statement

[Issue 107](https://github.com/gubasso/release-kit/issues/107) recorded an armed v0.1.10 request merging after a breaking change reached the trunk: the patch tag included the break, its changelog did not, and the immutable version lost the `0.y` minor break signal. GitHub allowed this stale merge while GitLab's `merge_method=ff` already refused it. Release contents are computed, so updating ancestry alone cannot repair them.

## Considered Options

- Strict status checks — chosen: refuse the stale merge until the bot recomputes.
- Merge queue — rejected: [the queue decision](./ADR-refuse-the-merge-queue-and-name-its-consequence.md) refuses its different gate; availability is limited to organization-owned public repositories or GitHub Enterprise Cloud, forcing an organization move for personal targets.
- Guard job — rejected: its green result describes when it ran; trunk movement does not rerun it.
- Disarm by default — rejected: bot self-repair preserves hands-off releases, so disarming adds an unnecessary human gate.

## Decision Outcome

Set `strict_required_status_checks_policy=true` and fault false or absent values. The API makes strictness inert without a required check; this convention requires two, making the flag and checks one setting in practice. GitLab satisfies the requirement through its existing merge method.

Enforced by `forge-setup:a-merge-carries-the-trunk-it-was-tested-against`; [sources](../reference/REFERENCE-forge-setup-sources.md) carry the dated verification.

## Consequences

- Good: the armed release waits for recomputation. Bot-only GitHub commits refresh by force-push; a non-bot commit after the first closes and reopens the request instead. The workflow re-arms using the fresh request number on every refresh, so both paths preserve the hands-off release.
- Bad: an ordinary stale request needs one deliberate update and another check run; a busy trunk serializes merges and can require repeated updates. Refusal itself dispatches no workflow.

## Status

Implemented: [the protection](../../setup/github/protect-trunk), [observation](../../src/setup/observe.rs), and [drawn model](../../method/06-release-from-trunk.md#why-an-armed-release-waits).
