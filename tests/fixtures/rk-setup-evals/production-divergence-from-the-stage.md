# Production divergence: a created file differs from its artifact

- case: production-divergence-from-the-stage
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A landed target where the operator changed `security.contact` in `.release-kit/config.toml` between the stage and the landing, so the landed `SECURITY.md` differs from `<stage>/artifacts/SECURITY.md`.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination.
5. State the evidence class: receipt plus useful history.
6. `rk guide landing` step 3: preview `rk upgrade --target .`, then run `rk upgrade --target . --apply`.
7. `rk guide landing` step 4a: `diff -q` reports `SECURITY.md` differs from its artifact. Inspect the deviation: the config key changed after the stage, and the landing rendered the new value. Report the cause. Where the benchmark itself must be refreshed, stage again into another directory with `rk stage --target . --output <dir>`.
8. `rk guide landing` step 4b: run `rk status --check --target .`, `rk setup check --target .`, and the project's checks.
9. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup. Every stage the task wrote is named, and each is cleaned by its own path.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- A deviation is explained from observation, never repaired by copying the artifact over the landed file.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
