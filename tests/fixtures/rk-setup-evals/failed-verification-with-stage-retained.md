# Failed verification: the stage stays for the diagnosis

- case: failed-verification-with-stage-retained
- classification: upgrade
- intent: upgrade
- provision: no
- tasks: `upgrade`
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: kept

## Situation

A target landed by an older `rk` whose upgrade lands cleanly, and whose own test suite then fails on the manual stage. The operator asked to upgrade, with cleanup.

## Gate

Present "State: `upgrade`. Request: upgrade. Tasks: `upgrade`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `upgrade` pre-flight: read `config.pending` and `legacy_state` in the status report.
3. Stage, `rk guide landing` step 1c: run `rk stage --target . --json` and hold `<root>`.
4. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. Every differing `rendered` row reads `attributed`, so the landing refreshes it.
5. Land, `rk guide landing` step 4: preview `rk upgrade --stage <root> --target .`, quote every word, then run it with `--apply` as a gated step.
6. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
7. General post-flight: `rk status --check --target .` passes, and `pre-commit run --hook-stage manual --all-files` fails on the project's own test.
8. The check failed, so the run leaves the stage in place as recoverable evidence, at `<root>`, and the cleanup is not shown.
9. Report: the first failing line, the stage kept at `<root>` because a check failed, and the failing check routed to the project.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- A failing project check is the project's own: no fix is invented, and nothing is repaired inside the post-flight.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
