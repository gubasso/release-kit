# Plan every landing write as one typed document

## Context and Problem Statement

`rk init`, `rk upgrade`, and `rk adopt` each walk the payload projection on their own path and decide each destination by their own rule. A routing skill then classifies the target by feel and dispatches to a second skill. Three questions are arguable: whether the verbs share one model or three, what an approval binds to, and whether a stated gap is permission.

## Considered Options

- One typed plan, `rk.plan/1`, computed by a pure planner, with a readiness policy and a fingerprint — chosen.
- A context document the verbs compose for an agent to read — rejected: prose is not a contract, and an apply that trusts a stated gap acts on what nobody verified.
- A separate upgrade workflow beside setup and migration — rejected: it adds a fourth path to the three that already drift.
- Typed forge operations inside the plan — deferred: revisit once a journaled apply exists for local operations.

## Decision Outcome

Chosen option: one typed plan, because three verbs walking one projection on three paths is three places for a rule to drift, and one document every write comes from is one. The plan keeps five kinds apart: evidence, analysis, policy, decisions, and postconditions. Classification is one word from five, beside findings. Operations are a closed set that names digests and never bytes. Readiness is the worst precondition, and a gap is honest and is not permission. Provenance attaches to claims through an evidence ledger. The fingerprint binds the semantic inputs, so an apply can refuse on any difference. `rk reconcile plan` is read-only and offline by default.

Enforced by `reconcile:every-landing-write-comes-from-one-plan`, `reconcile:a-plan-carries-one-classification-and-its-findings`, `reconcile:an-operation-names-digests-and-never-bytes`, `reconcile:readiness-is-the-worst-precondition`, `reconcile:every-observed-field-cites-evidence`, `reconcile:the-fingerprint-binds-the-semantic-inputs`, and `reconcile:plan-is-read-only-and-offline-by-default`.

## Consequences

- Good: one comparison rule for every verb; an approval binds to a digest; a reader routes on one word and reads the findings.
- Bad: the front verbs still land through their own paths until apply exists, so two implementations of the comparison coexist for one phase.

## Status

Implemented: `src/plan/`, `src/commands/reconcile.rs`, `src/commands/assess.rs`.
