# Greenfield: a bare repository takes the convention

- case: greenfield
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A repository with no release mechanism, no tag, no second long-lived branch, and no `.release-kit/manifest.json`. `rk assess --target .` reports `greenfield`. The operator asked for the release workflow to be set up, with cleanup, and answered the technology, the workflow mode, and the release style.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another. The report names no wired manager on a bare tree, so no freshness offer is raised here: there is no pin for a line to move.
2. `rk assess --target .`: the verdict is `greenfield`, so the arrival loads `rk method setup` and `rk guide setup`. The landing is that runbook's step 4a.
3. `rk guide landing` step 1c: run `rk stage --target . --tech <tech> --workflow <mode> --style <style>` and hold `<stage>`.
4. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`. Every candidate is a creation, because no destination exists yet.
5. `rk guide landing` step 2b: the evidence class is neither receipt nor history, which on a bare tree is a best-effort heuristic with nothing uncertain: no destination is present, so no ownership question arises and no automatic overwrite can happen.
6. `rk guide landing` step 3: preview `rk init --tech <tech> --target . --workflow <mode> --style <style>`, read every `created` word, then run it with `--apply`.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- The forge steps of `rk guide setup` steps 1 to 3 stay gated with their exact commands.
- Every uncertain ownership decision is shown to the operator, and a bare tree raises none.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
