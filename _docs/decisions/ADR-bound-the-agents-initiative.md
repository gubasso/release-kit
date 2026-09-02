# Bound the agent's initiative

## Context and Problem Statement

`ADR-grow-the-routing-block-to-carry-the-commit-contract.md` added a line telling an agent asked to change code on `master` to branch first. Coding agents read it as a standing order — they branch, commit, open the pull request, and merge unbidden — and it renders into every landed target. No mechanism here tells an agent from a person, so nothing stops one from driving a repository correctly and unasked.

## Considered Options

- `An authorization line in the block, and the same boundary in the shared plan gate` — chosen.
- `Drop the branch-first line and say nothing further` — rejected: an agent's own habit fills the silence, and the block would still route without saying who acts.
- `Reclassify the block as seeded so each target writes its own rule` — rejected: the block would stop carrying corrections to landed targets, and a target already customizes outside the markers.
- `A hook or an agent permission file that refuses the calls` — rejected: no hook distinguishes an agent from a person, and a permission file is one vendor's format, not the convention's.

## Decision Outcome

Chosen option: `an eighth block line plus a gate section`, amending the prior record's seven-line bound. The line enumerates the actions — branch, commit, push, tag, pull request — rather than a category an agent must infer, and states that a request to change code authorizes the file changes alone. It clears the prior record's bar in the way that record did not anticipate: no mechanism can enforce it, which is why the target carries the sentence at all. The gate carries the same boundary for the skills, so an approved plan approves a shape and grants no standing licence.

Enforced by `landing:the-routing-block-bounds-the-agents-initiative` and `distribution:a-skill-plans-before-it-acts`.

## Consequences

- Good: the operator keeps every irreversible move, the merge above all, and the agent still states the convention unprompted.
- Bad: an operator wanting the old flow asks for it, per task or once for the session.

## Status

Implemented — `src/landing.rs` carries the line; `skill-shared/plan-gate.md` carries the boundary.
