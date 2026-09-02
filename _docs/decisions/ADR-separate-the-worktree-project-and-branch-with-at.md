# Separate the worktree project and branch with @

## Context and Problem Statement

The sibling name `<project>-<flattened branch>` made the boundary between its two halves unparsable — `-` is legal in both — and a worktree indistinguishable from a sibling repository named after a prefix of another (`foo` beside `foo-charts` reads as a worktree of `foo`). Which separator makes the boundary parse, and where does the change reach?

## Considered Options

- `@ as the separator` — chosen.
- `=` — rejected: gwq precedent, but option-shaped on many command lines and rarer in the wild as a directory character.
- `--` — rejected: still `-`, so the boundary stays a guess wherever either half contains one.
- `+` — rejected: no worktree-naming precedent, and glob-adjacent in some tools.
- `_` — rejected: this operator's tree already spends `_` on the group prefix, so it would blur the other boundary.
- `A container directory setting` — rejected: [the layout record](./ADR-default-to-the-worktree-workflow.md) derives the parent from the standing main worktree, and a setting would be a second source of truth for a fact the filesystem states; the admitted layouts are documented instead.

## Decision Outcome

Chosen option: `@ as the separator` — `release-kit@feat-rk-message` beside `release-kit`. `@` is a used convention for exactly this (worktrees as siblings at `NServiceBus@feature-x`; gwq's `repo=branch` template is the same idea), shell-safe unquoted, and near-absent from project names, so the boundary parses in both directions. The change is host-side only: no landed file carries the shape, so no target re-renders and no adopting project moves — an operator's standing seat under the old name is refused on the next `rk worktree add` with `git worktree move` named as the move. Enforced by `maintenance:a-worktree-path-derives-from-project-and-branch`.

## Consequences

- Good: the project/branch boundary parses; a worktree and a sibling repository stop colliding; migration is one named move per seat.
- Bad: flattening stays non-injective — `feat/a-b` and `feat-a/b` still collide, and the refusal by name remains the answer.

## Status

Implemented — `src/worktree.rs` derives it, `method/08-worktrees.md` states it.
