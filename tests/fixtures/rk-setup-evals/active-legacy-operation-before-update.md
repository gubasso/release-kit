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
3. An active operation stops the update. Report the plan by id and ask the operator to finish it or explicitly abandon it with the installed binary before the tool is replaced. Nothing below runs until then.
4. Record every inactive plan, result, run journal, and release cache path under the state root as local evidence, with why each is obsolete. Remove nothing.

## Authority

- Updating `rk` is the operator's move through the project's manager. This skill never chooses, installs, updates, downgrades, or fetches a version.
- Finishing or abandoning the stored plan is an action of the installed binary, named to the operator as its exact command.
- Inactive legacy state is named by exact path and is never removed without explicit cleanup authorization.

## Documentation

- project documentation: no candidate exists yet, because no stage is written before the update.
- reference corpus: the 0.4.x binary's own documentation is what this step reads. It is not copied into the project.
