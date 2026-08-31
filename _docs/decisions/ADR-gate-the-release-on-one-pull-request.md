# Gate the release on one pull request

## Context and Problem Statement

With one trunk, something must still separate landing work from releasing it, and nothing may become public before a human decision. The prior shape used a second, pinned gate pull request for this; the reasoning that earned it was that a bot publishing on its own request's merge, with the release branch fast-forwarded afterwards, puts the publish before the human gate.

## Considered Options

- `The bot's release request is the gate` — chosen.
- `A separate gate pull request after the release request` — rejected: with the trunk as the default branch, the release request itself is reviewable and check-gated, so a second pull request duplicates the same merge button.
- `Publish on every trunk push once the version differs` — rejected: it releases on a schedule set by whoever merges next, not by a decision.

## Decision Outcome

Chosen option: `the release request is the gate` — the bot maintains one pull request against the trunk carrying the bump and the changelog; the required check and the review sit on its merge button; merging it is the release, and closing it abandons a release with nothing public. The prior reasoning survives translated: nothing is public until the one reviewed merge — there is simply only one merge now, and the publish still follows the human decision instead of preceding it.

## Consequences

- Good: the changelog-correction window, the quality bar, and the release decision are one reviewable surface.
- Good: abandoning a release is closing a pull request.
- Bad: a release always ships the trunk's tip; a patch-only release needs the release-line style instead.

## Status

Accepted.
