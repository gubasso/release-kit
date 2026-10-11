# Snapshot forge state before a destructive step

## Context and Problem Statement

Several setup steps change forge state no file in the repository carries: the default branch, the merge settings, the rulesets and protections, and the long-lived branches `single-trunk` deletes. A wrong run of one of them can only be undone by someone who remembers what stood before, and nothing recorded it. A migration meets these settings already in use.

## Considered Options

- `Read and save the state a step changes before its script runs, and refuse where the read fails` — chosen.
- `Rely on the forge's own audit log` — rejected: the log names that a setting changed, often without the old value, and a deleted branch's commits are not in it.
- `Snapshot the whole forge configuration once per run` — rejected: it reads far more than any step changes, and every unrelated read can fail and block the run.
- `Save only during a migration` — rejected: a first setup on an existing repository changes the same live settings.

## Decision Outcome

Chosen option: `save per step`. Each step that removes or replaces forge state reads what it changes through the observers' read-only calls and writes it to `forge/<step>.json` in the run's snapshot directory under the state root, before the script runs, and the run names the path. `single-trunk` records each branch it deletes by name and commit, with a bundle where the clone holds the commit, made in a throwaway clone so no ref of the operator's clone moves. A read that fails refuses the step before any change. The snapshot is a backup: no later run reads it to decide anything, so a run still keeps no state of its own.

Enforced by `forge-setup:a-step-is-idempotent`.

## Consequences

- Good: every destructive step leaves the operator what it replaced, at a path the report names.
- Good: a runtime that cannot read a setting cannot change it either.
- Bad: each destructive step costs extra read calls, and the snapshots accumulate until the operator removes them.

## Status

Accepted.
