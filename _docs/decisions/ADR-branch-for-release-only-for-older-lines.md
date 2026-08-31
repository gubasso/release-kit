# Branch for release only for older lines

## Context and Problem Statement

Some consumers cannot be rolled forward: pinned self-hosted versions, support contracts on an old line, sign-off gates before a ship. Releasing from the trunk cannot produce a patch-only release, so the method needs a second style — without reopening the door to long-lived integration branches.

## Considered Options

- `Just-in-time release branches, cherry-pick only, never merged back` — chosen.
- `A standing release branch reused across versions` — rejected: an eternal branch diverges into exactly the second integration point the trunk model removes.
- `Pre-created release branches for every release` — rejected: machinery ahead of need; a line can be cut retroactively from its tag the day a backport is actually requested.

## Decision Outcome

Chosen option: `just-in-time lines` — `release/<major>.<minor>` is cut from a chosen trunk commit, not necessarily the tip, and possibly retroactively from a tag. Every change reaches it from the trunk by cherry-pick after landing on the trunk first; the branch is never merged back, is protected while alive, and is deleted once its tags pin its commits. The four failure modes the walkthrough chapter names — fixing on the branch then merging down, merging instead of cherry-picking, one eternal branch, branch-to-branch merges — are each excluded by construction.

## Consequences

- Good: a patch-only release exists exactly where someone needs it and nowhere else.
- Good: a dead line costs nothing; its tags keep its commits reachable after the branch is deleted.
- Bad: each active line duplicates the CI pipeline, and that real cost is the argument for defaulting to the trunk.

## Status

Accepted.
