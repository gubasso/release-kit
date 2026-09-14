# Marked region: the operator's prose around the block survives

- case: marked-region-customization
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A landed target whose `AGENTS.md` carries the operator's own sections above and below the release-kit markers, and whose block the installed release changes.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`. `<stage>/artifacts/AGENTS.md` is the whole proposed document: the operator's bytes outside the markers, the new block inside them.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination. The diff touches the marked region alone.
5. State the evidence class: receipt plus useful history. Propose the region replacement and quote the diff outside the markers, which is empty.
6. `rk guide landing` step 3: preview `rk upgrade --target .`, read `replaced` for `AGENTS.md`, then run `rk upgrade --target . --apply`.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks. Every byte outside the markers equals the previous commit.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
