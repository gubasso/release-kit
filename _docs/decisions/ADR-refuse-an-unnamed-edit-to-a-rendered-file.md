# Refuse an unnamed edit to a rendered file

## Context and Problem Statement

A landing replaced a recorded rendered file whatever its bytes were, because the operator authorized the migration and Git holds the recovery. An edit made after the landing therefore vanished without a word. An agent landing for the operator needs each such edit raised as a question, and the answer needs a copy that outlives the run.

## Considered Options

- `Refuse every conflict the operator did not name, and pass a named one after saving its bytes` — chosen.
- `Keep the silent replace, and leave the recovery to Git` — rejected: an edit nobody committed has no copy in Git, and one nobody saw has no question asked about it.
- `A force flag that passes every conflict at once` — rejected: it answers every question without asking any, which is the silent replace with an extra word.
- `Merge the edit into the candidate` — rejected: a merged file is reproducible from no parameter.

## Decision Outcome

Chosen option: `refuse and name`. A conflict is an unattributed file, or a recorded rendered file or region whose bytes the receipt no longer vouches for. The verb refuses every conflict before its first write, so an upgrade lands whole or not at all. `--replace <path>` passes exactly the conflict it names, after saving its bytes under the state root, outside any stage, and the report names where. A file already holding the candidate's bytes is no conflict, because a landing stopped part way leaves exactly that. `rk stage compare` shows each conflict with its provenance first.

Enforced by `landing:ownership-is-elementary`.

## Consequences

- Good: no edit leaves a target unseen, and every replaced edit has a copy the report names.
- Good: an upgrade or a repair is atomic across its conflicts.
- Bad: a target with many edits needs one answer per file, and the skill asks each one.
- Bad: saved copies accumulate under the state root until the operator removes them.

## Status

Accepted.
