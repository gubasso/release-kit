# Missing receipt without history: a best-effort heuristic

- case: missing-receipt-without-history
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: kept

## Situation

A shallow clone with no `.release-kit/manifest.json` and one squashed commit, holding workflows that look like the convention's. `rk assess --target .` reports `brownfield`.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk assess --target .`: `brownfield`. The migration inventory is written first.
3. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
4. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`. Every present file is a `collision` line, because no receipt names it and the bytes differ.
5. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination. `git log` shows one commit and attributes nothing.
6. State the evidence class: neither receipt nor history, a best-effort heuristic. Report every collision as an uncertain ownership decision and ask the operator about each one. No automatic overwrite: nothing is brought to the candidate without an answer, and the production verb refuses every unattributed collision until then.
7. `rk guide landing` step 2d: bring only the files the operator answered for to the candidate's bytes.
8. `rk guide landing` step 3: preview `rk init --tech <tech> --target .`, then run `rk init --tech <tech> --target . --apply` only for a tree whose every collision the operator answered. Where it still names collisions, it exits 73 with nothing written, and the finding returns to the inventory.
9. `rk guide landing` step 4b: `rk status --check --target .` reports the landing, or reports none where the apply refused, and the report is quoted as it stands.
10. The task ends with questions open. `rk guide landing` step 5a is not reached. The task leaves the stage in place as recoverable evidence and says so, with its path.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- Every uncertain ownership decision is shown to the operator, and none is taken silently.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
