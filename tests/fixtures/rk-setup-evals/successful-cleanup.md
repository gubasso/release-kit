# Successful cleanup: the stage goes last

- case: successful-cleanup
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A landed target whose upgrade landed and whose every check passed. The operator's request carried cleanup.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination.
5. State the evidence class: receipt plus useful history.
6. `rk guide landing` step 3: preview `rk upgrade --target .`, then run `rk upgrade --target . --apply`.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks. Every check passes.
8. `rk guide landing` step 5a: show the exact `rk stage clean <stage>`. Run it, because the request's authority includes cleanup. Read `removed <stage>` and the recovery command it prints.
9. Report the landed files for the operator to commit through the trunk's one path. The stage is not among them.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- Without cleanup in the request, the command is shown and not run, and the stage stays with its path reported.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
