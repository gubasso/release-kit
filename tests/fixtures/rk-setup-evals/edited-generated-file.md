# Edited generated file: a recorded rendered destination differs

- case: edited-generated-file
- landing: yes
- receipt: present
- history: useful
- legacy: none
- stage: cleaned

## Situation

A landed target whose recorded `.github/workflows/pr-title.yml` was edited by hand after the landing. The receipt records it as `rendered`, and `git log` names the operator's commit and its reason.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose. No step selects, fetches, or installs another.
2. `rk guide landing` step 1c: run `rk stage --target .` and hold `<stage>` for the whole task.
3. `rk guide landing` step 2a: read `<stage>/stage.json`, `<stage>/artifacts/`, and `<stage>/reference/`.
4. `rk guide landing` step 2b: diff each artifact against the working tree, read `.release-kit/manifest.json`, and read `git log` and `git diff` for each differing destination. The diff shows the operator's edit, and `git log` shows who made it and why.
5. State the evidence class: receipt plus useful history. The landing will print `replaced` for this file, because a recorded generated file is replaced even where its bytes changed. Show the operator the edit that will go and where its intent belongs: the target's own configuration, a class P key, or a change to release-kit.
6. `rk guide landing` step 3: after the operator answered, preview `rk upgrade --target .`, read `replaced` for the file, then run `rk upgrade --target . --apply`. Git holds the recovery.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, `rk setup check --target .`, and the project's checks.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names. Branch, commit, push, and pull request actions are the operator's unless the request named them.
- Nothing is copied out of `<stage>/artifacts/` into the target, and `stage.json` is offered to no verb. Production renders afresh.
- The replacement is proposed in the plan with the edit quoted, and the operator approves that plan before the apply.

## Documentation

- project documentation: `SECURITY.md` and the routing block in `AGENTS.md` are candidates under `<stage>/artifacts/`, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read for the changelog, the guidance, and the chapters the comparison raises. It is not copied into the project.
