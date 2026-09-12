# Apply only a stored plan

## Context and Problem Statement

A plan is computed, reviewed, and approved. What does the approval bind to, and what may an apply execute? An apply that recomputes intent from the tree at execution time can act on something the operator never saw. A plan gate written as prose asks an agent to validate, and nothing fails when the agent is wrong in a way that looks like being right.

## Considered Options

- Store every plan and apply only a stored one, revalidated first — chosen.
- Apply the plan the front just computed, unstored — rejected: it leaves no audit trail and no window for a second reader.
- Trust a plan the operator declares approved, with a flag past a gap — rejected: a gap is honest and is not permission, and no flag makes it one.
- A receipt store beside the journal — deferred: revisit if the journal's events prove too little for recovery.

## Decision Outcome

Chosen option: store every plan and apply only a stored one. A plan gate is a prompt and a revalidation is a program, and the difference matters exactly when an agent is wrong in a way that looks like being right. The apply recomputes the plan over the stored request, compares the fingerprints, verifies every `before` digest, and refuses before the first write, naming what moved. It proceeds on `ready` alone. Every write is staged and renamed in order with the record last, so an interruption leaves each destination whole. The run lands in the journal. The three fronts land through the same path.

Enforced by `reconcile:a-stored-plan-is-owner-only-and-pruned`, `reconcile:apply-revalidates-before-the-first-write`, `reconcile:apply-proceeds-on-ready-alone`, `reconcile:an-apply-is-one-transaction-with-the-record-last`, and `reconcile:every-front-lands-through-the-engine`.

## Consequences

- Good: an approval binds to a digest; a refused apply names what moved; one execution path serves four verbs.
- Bad: the store holds bytes from the target under the state root, so it is owner-only and pruned, and a plan is never committed or posted.

## Status

Implemented: `src/plan/store.rs`, `src/plan/apply.rs`, `src/atomic.rs`, `src/commands/reconcile.rs`, `src/commands/init.rs`, `src/commands/upgrade.rs`, `src/commands/adopt.rs`.
