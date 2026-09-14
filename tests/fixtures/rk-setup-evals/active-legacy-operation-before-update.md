# Active legacy operation: a stored plan waits before the update

- case: active-legacy-operation-before-update
- landing: no
- receipt: present
- history: useful
- legacy: active
- stage: none

## Situation

The installed binary is a 0.4.x release, and the operator asked to update `rk` and upgrade the target. `rk reconcile list`, run with that installed binary, names a stored plan the operator still means to apply.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another. The installed binary is 0.4.x.
2. `rk guide landing` step 1b, with the installed 0.4.x binary: `rk reconcile list` names an active stored plan.
3. An active operation stops the update. Report the plan by id and ask the operator to apply it with the installed binary before the tool is replaced, or to decline it. Abandonment is that decision and no command: a declined plan is reclassified as inactive. Nothing below runs until the operator answers.
4. Record every inactive path under the state root as local evidence, with why each is obsolete: each stored plan at `<state root>/plans/<plan-id>/` from `rk reconcile list`, each run journal at `<state root>/runs/<run-id>/` from `rk runs list`, and the release cache at `<state root>/release/`, listed directly. Remove nothing.

## Authority

- Updating `rk` is the operator's move through the project's manager. This skill never chooses, installs, updates, downgrades, or fetches a version.
- Applying the stored plan is an action of the installed binary, named to the operator as its exact command. Declining it is the operator's decision and runs nothing.
- Inactive legacy state is named by exact path and is never removed without explicit cleanup authorization.

## Documentation

- project documentation: no candidate exists yet, because no stage is written before the update.
- reference corpus: the 0.4.x binary's own documentation is what this step reads. It is not copied into the project.
