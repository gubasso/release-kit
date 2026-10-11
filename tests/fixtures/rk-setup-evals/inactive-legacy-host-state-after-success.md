# Inactive legacy host state: removed by name after a green landing

- case: inactive-legacy-host-state-after-success
- classification: upgrade
- intent: upgrade
- provision: no
- tasks: `upgrade`
- landing: yes
- receipt: present
- history: useful
- legacy: inactive
- stage: cleaned

## Situation

A target landed by a 0.4.x release whose stored plans are all inactive. `rk status --target . --json` reads `classification` `upgrade`, and `legacy_state` lists each path with `active` false. The operator asked to upgrade, with cleanup.

## Gate

Present "State: `upgrade`. Request: upgrade. Tasks: `upgrade`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `upgrade` pre-flight: read `config.pending` and `legacy_state` in the status report.
3. Stage, `rk guide landing` step 1c: run `rk stage --target . --json` and hold `<root>`.
4. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. Every differing `rendered` row reads `attributed`, so the landing refreshes it.
5. Land, `rk guide landing` step 4: preview `rk upgrade --stage <root> --target .`, quote every word, then run it with `--apply` as a gated step.
6. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
7. `upgrade` post-flight: show each inactive legacy path by its exact path, with why each is obsolete. Remove each by name only under explicit cleanup authorization, after confirming that no active old operation remains, as `rk guide landing` 5b carries it. No `rk` verb removes this state.
8. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`.
9. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
10. Report: fill the template from observation, with every answer the operator gave.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- Legacy state is never removed without explicit cleanup authorization.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
