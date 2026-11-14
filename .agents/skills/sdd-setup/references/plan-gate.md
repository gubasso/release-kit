# The plan gate

Standing instructions for the whole task, not one-time steps. Every spec-driven-docs skill drives operations that write files into a repository or under the user's home. As a result, each one plans, validates that plan against what actually knows, and only then executes.

This gate opens after the pre-flight gate has run, and takes its findings as inputs: a failed hard probe is why there is no plan yet, a failed soft probe other than `git` and `prek` is a gated step or a stated gap, and a failed `git` or `prek` probe is an install step for the operator that removes no step. That file runs whatever the request carries. This one has a flag.

Hold all three phases for the rest of the task, and apply them to every further request in the same session. Without `--no-plan`, phase 3 runs in a later turn than phase 1, after the plan is approved. With the flag, the phases run in order in the current turn.

## 1. Plan

Do this before the first change of any kind: a file written into a target repository or under the user's home, any verb run with `--apply`, or `sdd upgrade` run without `--dry-run`. The agent's own plan file is not such a change. Claude Code's plan mode writes one, and this phase depends on it.

1. Enter plan mode. In Claude Code that is the `EnterPlanMode` tool. In an agent that has no plan mode, state the plan in the reply instead and take the operator's answer before acting.
2. Research read-only. Read the skill, the chapters and specs it routes to, and the target's own report. `sdd status --target . --json` observes and never writes. Do not edit, and do not run any verb with `--apply`.
3. Write the plan. It states, in this order:
   - The ordered `sdd` verbs to run, each with its flags.
   - The files the run writes, rewrites, or removes.
   - Every step gated for the operator, with the exact command they run and what it changes.
   - The verification command that closes the task.
   - The open risks and the assumptions the plan rests on.
4. Ask what the plan cannot decide. Use `AskUserQuestion` for a choice that changes the work: the profile, the docs root, the docs scratch, whether an existing document is rewritten or retired. Do not use it to ask whether the plan is acceptable. The docs scratch is the one question that carries concrete candidate paths, because the corpus names it by its variable everywhere else.
5. Present the plan and end the turn. In Claude Code that is the `ExitPlanMode` tool, whose approval prompt is the gate. Do not pre-approve that tool: approving it automatically is the same as having no gate.

When the request carries `--no-plan`, replace this phase's approval turn: do not call `EnterPlanMode` or `ExitPlanMode`, because plan mode's read-only hold blocks phase 3 and only that approval prompt releases it. Do the same read-only research in the current turn, state the ordered plan in the reply, then continue into phase 2 without ending the turn. Phases 2 and 3 run in full.

## 2. Validate

The plan is a claim about what will happen. Check it against something that knows, never against your own confidence.

1. Preview every verb that has one. `sdd stage --target .` renders the candidate this binary carries into a directory beside the target and writes nothing into it, so the comparison is bytes rather than a claim. `sdd init` and `sdd skill uninstall` write nothing without `--apply`. `sdd upgrade` writes by default, so its preview is `sdd upgrade --dry-run`, and the flag stays on until the plan is approved.
2. Validate every action that has no preview (rewriting a document, retiring a file the project authored) against what states it instead: the placement chapter for where each fact belongs, and a read-only observation of the current state. `sdd status --target .` and `sdd verify --target .` observe and never write.
3. Compare both against the plan: the destinations, their count, and the steps in their order.
4. Where they disagree, stop. Say what differs, and return to phase 1. Never reconcile a surprise by widening the plan silently.

## 3. Execute

1. Run the verbs in the planned order, one at a time. Never batch one write behind another.
2. Re-observe after each one, and report what the command returned rather than that it succeeded.
3. Gate every step the operator must run by hand: print the exact command, say what it changes and why, wait, then re-observe before continuing.
4. Close on the verification command the plan named. `sdd verify --target .` judges the instance, and the delivered gates judge the tree.
5. Where execution shows the plan was wrong, stop and re-plan. Do not expand the scope of an approved plan. A landing refuses before its first write when a destination holds bytes no record accounts for, naming each one. That refusal is the check failing the run, not an obstacle to work around.
6. Where a landing stops partway, read what it reported: the destinations that hold candidate bytes, and the record that still stands. Run it again rather than repairing by hand, and compare the result against the stage.
