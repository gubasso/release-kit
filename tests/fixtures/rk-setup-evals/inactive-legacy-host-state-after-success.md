# Inactive legacy state: named after a passing landing

- case: inactive-legacy-host-state-after-success
- landing: yes
- receipt: present
- history: useful
- legacy: inactive
- stage: cleaned

## Situation

The operator updated `rk` from a 0.4.x release after `rk guide landing` step 1b found no active operation. The state root still holds inactive stored plans, results, run journals, and a release cache. The request's authority includes cleanup.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1b, recorded before the update: no active operation, and each inactive path written down as local evidence, with why it is obsolete.
3. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
4. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`.
5. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination.
6. State the evidence class: receipt plus useful history.
7. `rk guide landing` step 3: preview `rk upgrade --target .`, then run `rk upgrade --target . --apply`.
8. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks.
9. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.
10. `rk guide landing` step 5b: show each recorded exact path, `<state root>/plans/<id>` and the rest, and why each is obsolete. Confirm again that no active old operation remains. Remove each by name, one at a time, only under the explicit cleanup authorization the request carries. No `rk` verb removes this state, and no verb takes a directory as a recursive target.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- Inactive legacy state is named by exact path and is never removed without explicit cleanup authorization.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
