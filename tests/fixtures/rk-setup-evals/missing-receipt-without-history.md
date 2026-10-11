# Missing receipt without history: a best-effort adoption

- case: missing-receipt-without-history
- classification: adopt
- intent: upgrade
- provision: no
- tasks: `adopt`
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A shallow clone of a repository an older release landed, with no record and no useful history. `rk status --target . --json` reads `classification` `adopt`. The operator asked to upgrade it.

## Gate

Present "State: `adopt`. Request: upgrade. Tasks: `adopt`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `adopt` pre-flight: run `rk profile --target . --json`, and ask the release style explicitly.
3. Stage, `rk guide landing` step 1c: run `rk stage --target . --json --release-style trunk` and hold `<root>`.
4. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. Git cannot answer, so `history.available` is false and every differing row reads `unknown`. The class is a best-effort heuristic: ask the operator about each file, and no automatic overwrite happens.
5. Land, `rk guide landing` step 4: preview `rk adopt --stage <root> --target .`, quote every word, then run it with `--apply` and one `--replace <path>` per answered file as a gated step.
6. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
7. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`.
8. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
9. Report: fill the template from observation, with every answer the operator gave.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- Every uncertain ownership decision is shown to the operator, and no file is replaced without an answer.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
