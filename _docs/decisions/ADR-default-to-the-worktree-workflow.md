# Default to the worktree workflow

## Context and Problem Statement

Parallel work — human or coding agent — cannot share one checkout's HEAD, index, and uncommitted files, and the convention needed a working-copy form agents and clones could all see. Is worktree-first a mode, a mandate, or a habit, and where does the choice live?

## Considered Options

- `A committed landing parameter, defaulting to worktree` — chosen.
- `Modeless interchangeability` — rejected: nothing observable; agents cannot know the intent, clones drift.
- `A per-clone opt-in step` — rejected: clone-local where the premise demands repository settings.
- `An unconditional worktree mandate` — rejected: the branches workflow is legitimate.
- `A landed hook reading a local config toggle` — rejected: landed bytes with invisible local behavior.
- `A detached-HEAD pass-through in the guard` — rejected: a side door through the invariant; the sweep skip is the honest cost.
- `A raw post-checkout reminder pointing at worktrees` — rejected: redundant under the guard in worktree mode, a nag in branches mode.
- `An aggregator reminder verb` — rejected: surface without behavior; drift-and-reapply owns hook evolution.
- `A mode-changing rk init on recorded targets` — rejected: init's refusal of recorded targets is landed design; the change is an upgrade.

## Decision Outcome

Chosen option: `A committed landing parameter, defaulting to worktree` — recorded in the manifest, rendered into the committed blocks, judged by `rk status --check`, changed only through `rk upgrade --workflow`: every clone sees one answer, and a change is a reviewed diff. In worktree mode the main checkout commits nothing, detached included — the trunk protection's local mirror — with git's checkout-exclusivity as the concurrency lock. Enforced by `maintenance:the-workflow-mode-is-a-landing-parameter` and `maintenance:worktree-mode-guards-the-main-checkout`.

## Consequences

- Good: one committed answer; the guard whole, its cost stated.
- Bad: the rendered-block surface doubles per mode, contained by each block being a pure function of the parameter. A hook body is version-coupled to the binary; the per-verb probe under `maintenance:the-reminder-is-silent-on-a-binary-that-cannot-prune` decouples it — the consequence [the reminder-channel record](./ADR-remind-from-a-raw-git-hook.md) missed.

## Status

Implemented — `src/landing/manifest.rs` records the mode, `src/landing.rs` renders it, `method/08-worktrees.md` states it.
