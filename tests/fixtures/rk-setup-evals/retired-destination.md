# Retired destination: a file the release stops producing

- case: retired-destination
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A landed target whose receipt names a workflow this binary's projection no longer produces.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`. The stage prints a `retired` line for the workflow: recorded, no longer produced, target-owned from the next landing.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination.
5. `rk guide landing` step 2c: read `<stage>/reference/CHANGELOG.md` and `<stage>/reference/guidance/` for the releases between the record and the binary. The changelog entry names why the destination left.
6. `rk guide landing` step 2d: the file stays on disk and leaves the receipt at the landing. Whether the migration removes it is an inventory entry for the operator, and production never deletes it.
7. `rk guide landing` step 3: preview `rk upgrade --target .`, read `released` for the file, then run `rk upgrade --target . --apply`.
8. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks. The file is still on disk and `rk status` no longer lists it.
9. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- Removing the retired file is a removal, so what it removes is committed first and the operator's request names it.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
