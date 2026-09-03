# Arm the release request with standing auto-merge

## Context and Problem Statement

The one-request gate holds the quality bar and the release decision on one merge button, but the merge click is a per-release human act. A project that ships continuously wants that decision made once, without moving the check bar it rides on.

## Considered Options

- `Standing auto-merge armed at request creation` — chosen.
- `A scheduled job that merges green requests` — rejected: a cron releases on a schedule, which the one-request record already rejected.
- `A bot merging with an admin bypass` — rejected: a bypass skips the required checks, and the checks are the whole gate.
- `Keep the per-release merge only` — rejected: it makes continuous release unreachable, and per-release merging survives as the lines style and as a disarmed request.

## Decision Outcome

Chosen option: `standing auto-merge` — under the trunk style the bot's release request carries auto-merge from the moment it exists, armed by the bot identity on every refresh, so the forge merges it the instant every required check passes. The human decision moves from per release to once at landing, and revoking it for one release is disarming the request. Enforced by `landing:the-arming-identity-is-the-bot` and `forge-setup:the-setup-permits-a-request-to-merge-itself`.

## Consequences

- Good: a green trunk ships itself, and the required checks are inherent to the arm — the forge merges nothing red.
- Bad: the changelog-correction window closes, and stopping a release becomes a timed disarm; [withdrawing](./ADR-withdraw-a-release-by-disarming-the-request.md) owns that cost.

## Status

Accepted.
