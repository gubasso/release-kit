# Failed verification: the stage stays as evidence

- case: failed-verification-with-stage-retained
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: kept

## Situation

A landed target whose upgrade landed, and whose `rk status --check --target .` then reports an invariant failure in `dist-workspace.toml` that the project must decide on.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination.
5. State the evidence class: receipt plus useful history.
6. `rk guide landing` step 3: preview `rk upgrade --target .`, then run `rk upgrade --target . --apply`.
7. `rk guide landing` step 4b: `rk status --check --target .` exits 1 and names the invariant, the destination, and the remediation. Report the finding and the exact remediation. Repair it in the target where the request authorizes the edit, then run `rk upgrade --target . --apply` again, because a repair is a fresh production run and never a hand copy from the stage.
8. The check still fails on a decision the operator owns. `rk guide landing` step 5a is not reached. The task leaves the stage in place as recoverable evidence and says so, with its path.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- `rk stage clean <stage>` is not shown while a check fails. The stage remains recoverable evidence until every check passes.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
