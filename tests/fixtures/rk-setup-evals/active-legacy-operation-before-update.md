# Active legacy operation: a 0.4.x plan still waits to be applied

- case: active-legacy-operation-before-update
- classification: upgrade
- intent: upgrade
- provision: no
- tasks: `upgrade`
- landing: no
- receipt: present
- history: useful
- legacy: active
- stage: none

## Situation

A target landed by a 0.4.x release. `rk status --target . --json` reads `classification` `upgrade`, and `legacy_state` lists one stored plan with `active` true. The operator asked to upgrade.

## Gate

Present "State: `upgrade`. Request: upgrade. Tasks: `upgrade`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `upgrade` pre-flight: read `config.pending` and `legacy_state` in the status report.
3. `legacy_state` names an active operation, so the update waits for the answer. Ask whether the operator finishes it with the installed 0.4.x binary, per `rk guide landing` 1b, or records that it is abandoned. Remove nothing.
4. The operator has not answered, so the run stops the update here and reports the active operation.
5. Report: fill the template from observation, and name the active operation under "Still the operator's".

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Legacy state is never removed without explicit cleanup authorization.

## Documentation

- project documentation: no candidate is staged, because the update waits for the operator's answer.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
