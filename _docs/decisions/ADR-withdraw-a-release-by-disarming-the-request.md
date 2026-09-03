# Withdraw a release by disarming the request

## Context and Problem Statement

The model promised that closing the release request abandons a release at no cost. Under standing auto-merge the forge merges the moment the last required check goes green, so closing becomes a race, and after the merge there is nothing left to close.

## Considered Options

- `Disarm first, then close; after the merge, recover` — chosen.
- `A required-approval rule so a human still clicks` — rejected: it reintroduces the per-release act the arm exists to remove.
- `A hold label the arming job reads before arming` — deferred: revisit if operators report disarming most releases.
- `No stop at all` — rejected: a release convention with no stop is not a convention.

## Decision Outcome

Chosen option: `disarm, then close` — the hold on an armed request is one command run before its last check turns green, and the request then waits like any unarmed one: correctable, closable, abandonable at no cost. A release that already merged is not held but withdrawn — yank plus fix-forward, the path recovery already owns. The zero-cost abandon survives whole for the lines style and for any disarmed request. The next bot refresh may re-arm what an operator disarmed, and the runbook states that beside the disarm. The correction window a disarm reopens is the exception; the rule that replaces the window is `landing:the-changelog-quality-gate-is-the-squash-message`.

## Consequences

- Good: the stop is one command, and the cost after the merge is named rather than implied.
- Bad: abandoning a trunk-style release is a timed action, and a slow operator pays the recovery price instead of a closed request.

## Status

Accepted.
