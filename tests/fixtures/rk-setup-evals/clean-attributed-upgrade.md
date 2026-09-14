# Clean attributed upgrade: receipt and history agree

- case: clean-attributed-upgrade
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A landed target whose receipt names every destination and whose `git log` shows release-kit's landings alone touching them. The installed binary is newer than the record's `rk_version`, and the operator authorized the upgrade with cleanup.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination. Every differing destination is a recorded generated file, and its last author is the previous landing commit.
5. State the evidence class: receipt plus useful history, an attributed migration.
6. `rk guide landing` step 2c: read `<stage>/reference/CHANGELOG.md` and `<stage>/reference/guidance/` for the releases between the record and the binary.
7. `rk guide landing` step 3: preview `rk upgrade --target .`, read the `replaced`, `preserved`, and `matched` words, then run `rk upgrade --target . --apply`.
8. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks.
9. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
