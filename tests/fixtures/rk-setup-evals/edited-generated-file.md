# Edited generated file: a recorded rendered file someone changed

- case: edited-generated-file
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

A target landed by an older `rk` whose recorded `.github/workflows/pr-title.yml` was edited by hand after the landing. `git log` names the operator's commit and its reason. `rk status --target . --json` reads `classification` `upgrade`.

## Gate

Present "State: `upgrade`. Request: upgrade. Tasks: `upgrade`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `upgrade` pre-flight: read `config.pending` and `legacy_state` in the status report.
3. Stage, `rk guide landing` step 1c: run `rk stage --target . --json` and hold `<root>`.
4. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. The workflow reads `differs`, provenance `reduced`, and the word `collision`. Ask the operator whether to keep the edit or discard it, quoting the commit that made it. The operator keeps it.
5. Prepare, `rk guide landing` step 3: the operator's answer is recorded, and the kept edit is routed to `configure` in the post-flight.
6. Land, `rk guide landing` step 4: preview `rk upgrade --stage <root> --target .`, quote every word, then run it with `--apply` and `--replace .github/workflows/pr-title.yml` as a gated step.
7. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
8. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`.
9. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
10. Report: fill the template from observation, with every answer the operator gave.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- The landing saves the edited file's bytes before it writes, and the report names where.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
