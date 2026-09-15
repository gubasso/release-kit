# Two release-bearing technologies: the driver is the operator's answer

- case: an-ambiguous-release-observation
- landing: no
- receipt: missing
- history: none
- legacy: none
- stage: none

## Situation

A repository carrying both `Cargo.toml` and `pyproject.toml`, with no committed configuration and no record. The operator asked for the release workflow to be set up. `rk profile --target .` reports the observation as ambiguous and names both candidates: nothing in the tree says which one states the version the tag mirrors.

## Route

1. `rk guide landing` step 1a: run `rk --version`. The installed version is the one the operator chose.
2. `rk profile --target .`: the report names the ambiguity and the flag that answers it. Read it out rather than picking: a driver chosen for the operator is a release convention imposed on the wrong artifact.
3. Ask with `AskUserQuestion` which technology states the version and takes the bot, offering each candidate the report named and what each one implies for the registry and the artifacts. Stop here until the answer arrives: an apply refuses without it, and no stage is written for a landing nobody can yet describe.
4. `rk guide landing` step 1c is the next step once the operator answers, and the plan says so rather than running it.

## Authority

- The request authorizes the file changes and the `rk` verbs it names; nothing is written while the driver is unanswered.
- Never infer the driver from file order, repository name, or the larger tree.
- No stage is created here, so none is cleaned.

## Documentation

- project documentation: none is proposed while the profile is unanswered.
- reference corpus: `<stage>/reference/bindings` is what the answer will be read against once the driver is named, and is not copied into the project.
