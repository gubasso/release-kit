# Tuned seeded file: the target's configuration survives

- case: tuned-seeded-file
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A landed target whose `release-plz.toml` the operator filled after the first landing. The receipt records it as `seeded`.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`. The stage lists the file under `seeded present`, so a landing keeps it.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination. The artifact differs from the disk, and the difference is the operator's tuning.
5. State the evidence class: receipt plus useful history. Propose no change to the file. The landing will print `drift` or `preserved` and carry the current digest into the receipt.
6. `rk guide landing` step 3: preview `rk upgrade --target .`, read `drift` for the file and no `replaced`, then run `rk upgrade --target . --apply`.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks. The tuned bytes differ from the artifact by design, and the check reports the seeded drift as informational.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- The tuning is never brought to the artifact's bytes: a seeded file is the target's after the first landing.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
