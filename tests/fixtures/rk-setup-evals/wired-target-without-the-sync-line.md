# Wired target: the pin is there and nothing moves it

- case: wired-target-without-the-sync-line
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A repository with no release mechanism, no tag, and no `.release-kit/manifest.json`, whose tool manager already pins release-kit and whose `.envrc` carries no sync line. `rk assess --target .` reports `greenfield`. `rk self-depend status --target . --json` reports the manager under `wired` and reports `envrc_sync` false. The operator asked for the release workflow to be set up, with cleanup, and answered the technology, the workflow mode, and the release style.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target . --json`. Hold `wired`, `envrc`, and `envrc_sync`. The installed version is the one the operator chose, and no step selects, fetches, or installs another.
2. `rk assess --target .`: the verdict is `greenfield`, so the arrival loads `rk method setup` and `rk guide setup`. The landing is that runbook's step 4a.
3. `rk guide landing` step 1c: run `rk stage --target . --checkout-mode <mode> --release-style <style>` and hold `<stage>`.
4. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`. The manager file the target owns is an omission the stage names, never a candidate.
5. `rk guide landing` step 2b: the evidence class is neither receipt nor history, which on this tree is a best-effort heuristic with one destination present: the manager file is the target's own, so no automatic overwrite can happen and the stage withholds it.
6. `rk guide landing` step 3: preview the command `rk profile` rendered, with the canonical flags alone, read every `created` word, then run it with `--apply`.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.
9. The landing is green and the pin still has nothing moving it. Ask the operator about the freshness wire before the task closes, and state what the line does: on directory entry, at most once a day, it asks the wired manager's pin to move forward and leaves a diff to review. On an acceptance, run `rk self-depend add --manager <wired> --target . --json` with the manager the report named, and place the served line as a gated step. On a refusal, record the answer in the report and close.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- The `.envrc` edit is the operator's to approve: the exact line is printed, the answer is waited for, and no file the target owns is written before it.
- Every uncertain ownership decision is shown to the operator, and the one destination present here is the manager file the landing withholds.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the guidance and the chapters the comparison raises. It is not copied into the project.
