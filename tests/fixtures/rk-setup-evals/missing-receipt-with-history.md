# Missing receipt with history: a history-guided heuristic

- case: missing-receipt-with-history
- landing: yes
- receipt: missing
- history: useful
- legacy: none
- stage: cleaned

## Situation

A repository running the convention with no `.release-kit/manifest.json`, whose `git log` shows when each workflow arrived and who touched it since. `rk assess --target .` reports `brownfield`, so the arrival loads `rk method migration` and `rk guide migration`.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk assess --target .`: `brownfield`. The findings inventory of `rk guide migration` step 1 is written before any apply.
3. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
4. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`. The stage prints a `collision` line for each present file whose bytes differ from the candidate, because no receipt names them.
5. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination. `git log` attributes each file's arrival and each later edit.
6. State the evidence class: history without receipt, a history-guided heuristic. Propose bringing to the candidate only the files the log attributes to the convention's landing, and ask about every file it does not. No automatic overwrite: the production verb refuses every unattributed collision, and the plan names each file before the operator answers.
7. `rk guide landing` step 2d: bring the attributed files to the candidate's bytes by hand, within the request's authority.
8. `rk guide landing` step 3: preview `rk adopt --target . --checkout-mode <mode> --release-style <style>`, read `matches` for every rendered destination, then run it with `--apply`. Where a file still differs, `rk init` and `rk adopt` refuse and the finding returns to the inventory.
9. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks.
10. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- Every file the log does not attribute is a question, asked with `AskUserQuestion` and its consequence stated.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
