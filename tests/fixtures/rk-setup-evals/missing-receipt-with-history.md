# Missing receipt with history: an older landing, adopted

- case: missing-receipt-with-history
- classification: adopt
- intent: set up or none
- provision: no
- tasks: `adopt`, then the setup tasks
- landing: yes
- receipt: missing
- history: useful
- legacy: none
- stage: cleaned

## Situation

A repository an older release landed before the record existed: its marked regions and its rendered files are present, and `.release-kit/manifest.json` is not. `rk status --target . --json` reads `classification` `adopt`. `rk setup status --target . --json` reads every step `satisfied` and the proof `compatible`, so no setup task follows. The operator asked to set it up.

## Gate

Present "State: `adopt`. Request: set up. Tasks: `adopt`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `adopt` pre-flight: run `rk profile --target . --json`, and ask the release style explicitly.
3. Stage, `rk guide landing` step 1c: run `rk stage --target . --json --release-style trunk` and hold `<root>`.
4. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. No receipt exists, so provenance comes from Git alone, a history-guided heuristic. Group the rows by `edited_after_added`: the untouched group takes one answer, replace them all with each copy saved, and the one file edited after it gets its own question. No automatic overwrite happens.
5. Land, `rk guide landing` step 4: preview `rk adopt --stage <root> --target .`, quote every word, then run it with `--apply` and one `--replace <path>` per answered file as a gated step.
6. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
7. `adopt` post-flight: `rk status --target . --json` reads `classification` `current`, so no `upgrade` follows.
8. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`.
9. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
10. Report: fill the template from observation, with every answer the operator gave.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- Every saved copy is named in the report under "Saved before changing".

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
