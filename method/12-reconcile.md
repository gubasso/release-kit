# 12 — Reconcile

How every release-kit write reaches a repository: one plan, one decision, one apply. [Setup](./02-setup.md) owns why a bare repository takes the convention in its order, and [migration](./10-migration.md) owns why a repository that already releases is carried across with a findings inventory. This chapter owns what the two share once bytes move: the plan the engine computes, the decisions the operator answers, and the apply that executes exactly that plan or refuses. The command form of this chapter is the reconcile runbook, `rk guide reconcile`, and `rk reconcile` is the engine it drives.

## One plan for every write

A plan is one typed document that states what a landing will do before it does it. The engine observes the repository, resolves one release, and computes the plan from those two things and nothing else. `rk reconcile plan` prints it and stores it. `rk reconcile apply` executes it. `rk init`, `rk upgrade`, and `rk adopt` keep their names and land through the same path, each with its intent fixed. No verb walks the payload on a path of its own, because three paths for one rule are three places for the rule to drift.

The plan keeps five kinds of content apart, and an addition inside one kind changes the shape of no other.

- Evidence is what the engine observed: the repository, the landing record, the configuration, the host, and the forge where the operator opted in. Every observed field cites the evidence it rests on.
- Analysis is what the engine derived from the evidence: the operations, the compatibility facts, and the guidance selected for this target.
- Policy is the requirement every precondition carries: advisory, decision-required, or required.
- Decisions are the questions the operator owns, each with a stable id, its choices, and the answer selected.
- Postconditions are the checks that prove the apply completed.

An operation names a path, a kind, and the digest of what it writes. It never carries bytes and never carries a command. The bytes live in the plan's store, so the apply writes exactly what the operator read, and nothing stored can run.

## Classification

Every plan carries one word from a closed set and the findings behind it. The word is what a reader routes on. The findings are what the word compresses.

- `setup`: an empty target. Every destination is an operation.
- `migration`: a target with another tool's release marker, a payload destination already present, or release activity no mechanism explains. [Migration](./10-migration.md) owns what happens before and after the landing.
- `upgrade`: a recorded target whose owned files stand as the record left them. A target already at the release is an upgrade with no operation, and the operations alone tell the two apart.
- `drift`: a recorded target with an owned file edited or missing. The plan is blocked until the file is reconciled.
- `invalid`: a record this engine cannot read.

A version gap alone is not work. `rk status` reports the payload a newer binary would land as pending and judges nothing until `--check` asks. The plan says the same thing with its operations: a recorded target whose files already match the candidate has nothing to take, whatever version the record names.

## The routing limit, corrected

The question "what would release X do to this repository" once had one answer: obtain release X and run it. The engine reads a release bundle through one seam, so a running engine describes any release it can read. `rk reconcile plan --to <version>` computes the plan for that release with this binary, and `rk payload --release <version>` shows what that bundle carries. Obtaining another binary is required in exactly one case: a bundle whose payload schema is newer than this engine's, which the plan refuses by naming the engine to install.

## Readiness, and why a gap is not permission

Every precondition carries a requirement, and the plan's readiness is the worst precondition. A required precondition that does not hold blocks the plan. A decision-required precondition waits on the operator until the decision it names is selected. An advisory precondition never counts. A value the engine could not observe counts the same as a value observed and found wanting, under whatever requirement its precondition carries.

That last rule is the one that matters. A gap is honest. It says the engine does not know, and it says nothing about safety. An apply that treated a stated gap as a pass would act on the operator's silence. So the engine offers no flag that turns a gap into a pass, and the only way past a decision-required precondition is the decision, selected by id and answer and frozen into the plan.

## The fingerprint

Approval binds to a fingerprint: one digest over the candidate bundle's identity, the record and configuration digests, every operation's kind, path, and before and after digests, every required precondition's evaluation, and every selected decision. Timestamps, presentation text, and advisory evaluations are excluded, so two plans over the same inputs at different instants share one fingerprint.

The apply recomputes the plan over the stored request, compares the stored fingerprint against the fresh one, and refuses on any difference by naming what moved. The record edited after the plan, a destination changed, a bundle replaced, or a decision answered differently is each a refusal, with the target byte-identical afterwards. A refusal is never a silent re-plan. The operator reads the fresh plan and approves it as a new thing.

## The fronts

`rk init` plans with the setup intent and refuses a target that carries a record. `rk upgrade` plans with the upgrade intent and needs one. `rk adopt` plans the configuration and the record and no other write, because adoption verifies the disk against the candidate and blesses nothing. Each front lands on `--apply` through the path `rk reconcile apply` takes. The landing a front produces is the landing the engine produces under the same request, file for file.

A front is one process with no review window between its plan and its apply. `rk reconcile plan` followed by `rk reconcile apply` is the same landing with the window open. An agent takes the open form, because the plan is what the operator approves.

## The downgrade refusal

A record newer than the engine is refused. Under the seam that is a schema question: an engine reads any bundle whose payload schema is at or below its own, and a record written by a newer schema names the engine to install. Nothing about commits follows from the refusal. A project may move its `rk` pin in the same commit as the landing, or in a separate `build(deps)` commit before it. The separate commit is a recommended policy, because a revert then reverses one thing, and the chapter says so as policy and not as a consequence of the refusal.

## The sequence

1. Observe. Read the target the way [migration](./10-migration.md) reads it, and read what the plan will cite.
2. Request a plan. Name the release where the request names one. The default is this binary's own bundle, offline.
3. Read the classification and the readiness. The word names the chapter that owns the surrounding procedure. The readiness names what stands between the plan and the apply.
4. Resolve each decision. The plan states the question and the consequence of each answer before it asks. An answer the operator already gave in the request is honored, and a second question is asked only for a decision the plan names or an action outside what was authorized.
5. Reconcile what the plan refuses. An owned file the target edited, a seeded file the target tuned, and a generated artifact behind its configuration are three different answers, and none widens the plan.
6. Apply. The engine revalidates, writes every operation as one transaction with the record last, runs the postconditions, and journals the run.
7. Read the result. The postconditions, the journal entry, and the status report are what say it landed, never the operator's word.
8. Move the pin. A pin a one-fact manager records moves as an operation of the plan. The flake pin moves through `rk self-depend sync`, because that move needs nix and the network an offline apply never has.

## What reconcile is not

A workflow-mode or release-style change is a named migration, not a consequence of a newer release arriving. It is a plan under new parameters, gated by a decision the plan names, and [migration](./10-migration.md) owns it as an inventory entry with its own verification.

Forge state is evidence in the plan, never an operation of it. `--observe forge` reads the trunk's tip and stamps it, and a forge fact enters as a precondition or a postcondition. Every forge change stays a gated `rk setup step`, journaled the way it is today.

## Boundary tests

- A recorded target at the release is an upgrade with no operation, not a setup and not a fault.
- A stated gap is a not-observed evaluation under its requirement, never a pass.
- A refusal at apply is a fresh plan to read, never a flag to add.
- An edited owned file is a blocked drift, and keeping the edit is a change to the payload, not a change to the plan.
- A newer record is a schema question answered by the engine to install, and it forces no commit shape.
