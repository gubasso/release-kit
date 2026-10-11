# A knowledge base with no forge: zero technologies, no remote

- case: a-knowledge-base-with-no-forge
- classification: setup
- intent: change a setting
- provision: no
- tasks: `land`
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A Markdown knowledge base with no version file, no remote, and no release history. `rk profile --target . --json` reports zero technologies and no forge, which is a valid profile, and the operator asked only for the routing block.

## Gate

Present "State: `setup`. Request: land the convention with these answers. Tasks: `land`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `land` pre-flight: run `rk profile --target . --json`, and ask only what it leaves open, one default each, with its consequence stated.
3. Stage, `rk guide landing` step 1c: run `rk stage --target . --json --release-mode none` and hold `<root>`.
4. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. The routing block is the one row, `absent` with the word `created`. With neither a receipt nor useful history, the class is a best-effort heuristic, and no automatic overwrite can happen.
5. Land, `rk guide landing` step 4: preview `rk init --stage <root> --target .`, quote every word, then run it with `--apply` as a gated step.
6. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
7. `land` post-flight: answer each sentinel `sentinel_locations` names, and state that the landed files reach the trunk through the project's own workflow.
8. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`.
9. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
10. Report: fill the template from observation, with every answer the operator gave.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- An absent forge is a valid profile: no forge step is proposed, and `rk setup` reports each as not applicable.

## Documentation

- project documentation: the routing block in `AGENTS.md` is the one candidate under `<stage>/artifacts/`, and the production verb lands it by its recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
