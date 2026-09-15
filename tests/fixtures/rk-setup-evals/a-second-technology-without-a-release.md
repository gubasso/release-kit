# A second technology: one driver, two technologies

- case: a-second-technology-without-a-release
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A repository whose backend is a Rust crate and whose front end is a React application. `rk profile --target .` names one technology, rust, because only `Cargo.toml` is a version file this release reads; the operator states that the React application ships with the crate and releases nothing of its own. The release is automatic and driven by rust, and the operator asked for the workflow to be set up with cleanup.

## Route

1. `rk guide landing` step 1a: run `rk --version` and `rk self-depend status --target .`. The installed version is the one the operator chose.
2. `rk profile --target .`: read the answers by domain. `profile.technologies` names rust from the observation, `profile.release` is automatic and driven by rust, and the capability selection lists the release automation at `(rust, <forge>)`. Confirm the second technology with the operator and add it: a technology in the profile is a fact about the project, and only the driver decides what releases.
3. `rk guide landing` step 1c: run `rk stage --target . --technology rust --technology react` and hold `<stage>`.
4. `rk guide landing` step 2a: read `<stage>/stage.json` and `<stage>/artifacts/`. The candidate set is the driver's own: a second technology that ships no release automation adds no destination, which the capability selection states rather than implying.
5. `rk guide landing` step 2b: with neither a receipt nor useful history the class is a best-effort heuristic, and here nothing is uncertain: every destination is absent, so no ownership question arises and no automatic overwrite can happen.
6. `rk guide landing` step 3: preview the command `rk profile` rendered, with both technologies and one driver, read every `created` word, then run it with `--apply`. Production renders afresh.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .` and the project's checks.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names; branch, commit, and request actions stay the operator's.
- The second technology is the operator's answer, never an inference from a file this release does not read as a version file.
- Every uncertain ownership decision is shown to the operator, and an absent destination raises none.
- Nothing is copied out of `<stage>/artifacts/`, and `stage.json` is offered to no verb.

## Documentation

- project documentation: the routing block in `AGENTS.md` and `SECURITY.md` are candidates the production verb lands by their recorded kind.
- reference corpus: `<stage>/reference/` is read and not copied into the project.
