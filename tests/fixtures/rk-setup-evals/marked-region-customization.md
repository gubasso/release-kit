# Marked region customization: the routing block was edited

- case: marked-region-customization
- classification: drift
- intent: fix
- provision: no
- tasks: `repair`
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A target at this binary's version whose `AGENTS.md` routing block was edited inside its markers. `rk status --check --target . --json` names rendered drift on `AGENTS.md`, so `classification` reads `drift`. The operator asked to fix the drift.

## Gate

Present "State: `drift`. Request: fix the drift. Tasks: `repair`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `upgrade` pre-flight: read `config.pending` and `legacy_state` in the status report.
3. `repair` pre-flight: run `rk status --check --target . --json` and sort the violations by kind. The one violation is rendered drift.
4. Stage, `rk guide landing` step 1c: run `rk stage --target . --json` and hold `<root>`.
5. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. The region row reads `differs`, provenance `reduced`, and the word `collision`. Ask whether to keep the edit or discard it. Both answers save a copy first, and every byte outside the markers stays.
6. Land, `rk guide landing` step 4: preview `rk upgrade --stage <root> --target .`, quote every word, then run it with `--apply` and `--replace AGENTS.md` as a gated step.
7. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
8. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`.
9. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
10. Report: fill the template from observation, with every answer the operator gave.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- The operator's own lines outside the markers are never touched.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
