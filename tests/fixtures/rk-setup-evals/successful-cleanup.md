# Successful cleanup: the stage goes last, after every check

- case: successful-cleanup
- classification: upgrade
- intent: upgrade
- provision: no
- tasks: `upgrade`
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A target landed by an older `rk`, whose upgrade lands cleanly and whose checks pass. The operator asked to upgrade and named cleanup in the request.

## Gate

Present "State: `upgrade`. Request: upgrade. Tasks: `upgrade`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `upgrade` pre-flight: read `config.pending` and `legacy_state` in the status report.
3. Stage, `rk guide landing` step 1c: run `rk stage --target . --json` and hold `<root>`.
4. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. Every differing `rendered` row reads `attributed`, so the landing refreshes it.
5. Land, `rk guide landing` step 4: preview `rk upgrade --stage <root> --target .`, quote every word, then run it with `--apply` as a gated step.
6. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
7. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`.
8. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
9. Report: fill the template from observation, with every answer the operator gave.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- Saved copies and snapshots live outside the stage, so the cleanup removes the stage alone.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
