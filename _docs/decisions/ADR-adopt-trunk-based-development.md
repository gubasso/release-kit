# Adopt trunk-based development

## Context and Problem Statement

The method released through two long-lived branches and two pull requests: work integrated on `develop`, a bot opened a release request there, and automation cut a gate branch pinned at the bump whose merge into `master` tagged and published, followed by a back-merge. The shape paid for integration twice, created a freeze window around the promotion, ran no CI on the gate candidate's own branch, forced merge commits onto `master` to prevent divergence, and required a custom `open-release-gate` orchestration in every workflow solely to fight the bots' default-branch targeting.

## Considered Options

- `Trunk-based development: one permanent branch, releases from it` — chosen.
- `Keep the two-branch gated flow` — rejected: the second integration point is the big-bang merge the first exists to avoid, and every cost above is structural to it.
- `Keep two branches but drop the pinned gate` — rejected: a gate whose head tracks the integration branch silently absorbs later pushes into the release.

## Decision Outcome

Chosen option: `trunk-based development` — `master` is the sole long-lived branch and the default, always releasable, written only through squash-merged pull requests; unfinished work ships dark behind flags; the bot maintains one release pull request against the trunk and merging it is the release. The gate orchestration, the back-merge, and the merge-commit requirement dissolve, because the constraints they compensated for no longer exist.

## Consequences

- Good: one integration point, no freeze window, and the bots' default-branch targeting is correct by construction.
- Good: revert and bisect stay trivial on a linear one-commit-per-pull-request history.
- Bad: the discipline moves into the work itself — feature flags and branch by abstraction become required practice rather than optional technique.
- Bad: a patch-only release needs a second style, recorded separately.

## Status

Accepted.
