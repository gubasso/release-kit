# Wired target: the pin is there and nothing moves it

- case: wired-target-without-the-sync-line
- classification: setup
- intent: set up or none
- provision: first
- tasks: `forge-settings`, `land`, `publish`
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A GitHub repository with a crate, no release mechanism, no tag, and no `.release-kit/manifest.json`, whose tool manager already pins release-kit and whose `.envrc` carries no sync line. `rk status --target . --json` reads `classification` `setup`. `rk self-depend status --target . --json` names the manager under `wired` and reads `envrc_sync` false. The operator asked for the release workflow to be set up, with cleanup.

## Gate

Present "State: `setup`. Request: set up. Tasks: `provision`, `forge-settings`, `land`, `publish`. Proceed?" and wait for the operator.

## Route

1. General pre-flight: run the pre-flight gate, then read `rk status --target . --json`, `rk self-depend status --target . --json`, and `rk setup status --target . --json`.
2. `provision` runs first, because the pin has nothing moving it. Step A asks nothing, because `wired` already answers how the project obtains `rk`.
3. `provision` Step B: ask the operator about the freshness line before the landing closes, and state what it does: on directory entry, at most once a day, it asks the wired manager's pin to move forward and leaves a diff to review.
4. On yes, run `rk self-depend add --manager <wired> --target . --json` and place the served line in `.envrc` as a gated edit. On a refusal, record the answer in the report and continue.
5. `forge-settings`: list the steps `rk setup status --target . --json` reads as not `satisfied`, preview each with `rk setup step <name> --target . --json`, and run each with `--apply` as a gated step, in the runbook's order. `install-bot` reads `unknown` without the App key, which is no question.
6. `land` pre-flight: run `rk profile --target . --json` and ask only what it leaves open, one default each: the release style, the integration mode, and each capability, the workflow guards listed before the question.
7. Stage, `rk guide landing` step 1c: run `rk stage --target . --json --technology rust --release-style trunk --integration local` and hold `<root>`.
8. Compare, `rk guide landing` step 2b: run `rk stage compare <root> --target . --json`. Every row reads `absent` with the word `created`, so nothing is decided. With neither a receipt nor useful history, the class is a best-effort heuristic, and no automatic overwrite can happen. The manager file the target owns is no candidate, so the stage withholds it.
9. Land, `rk guide landing` step 4: preview `rk init --stage <root> --target .`, quote every word, then run it with `--apply` as a gated step.
10. Prove, `rk guide landing` step 5a: run `rk stage compare <root> --target . --json` again. Every `rendered` and `region` row reads `equal`.
11. `land` post-flight: answer each sentinel `sentinel_locations` names, take `rk guide setup` 4b and 4d, and state that the landed files reach the trunk through the project's own workflow.
12. `publish`: ask, stated plainly, before the first upload, because it is permanent. Print 5b and 5c for the operator, then §6. Stop at 7d with "Next: cut one release with the `rk-release` skill".
13. General post-flight: run `rk status --check --target .`, then `pre-commit run --hook-stage manual --all-files`. `rk self-depend status --target . --json` reads `envrc_sync` as the operator answered.
14. Clean: print `rk stage clean <root>`, wait, and run it only because the request's authority includes cleanup.
15. Report: fill the template from observation, with the answer to the freshness line recorded.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb but through `--stage <root>`, which reads its parameters alone. Production renders afresh from the stage's parameters.
- The `.envrc` edit is the operator's to approve: the exact line is printed, the answer is waited for, and no file the target owns is written before it.
- The first upload, the trusted publisher, and every credential step are printed for the operator and never run by the agent.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the guidance and the chapters the comparison raises, and `rk guide setup` for the pin order in its Prerequisites. Each is not copied into the project.
