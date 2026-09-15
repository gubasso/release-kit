# GitLab, release-less: a complete title gate and no release

- case: a-gitlab-project-that-releases-nothing
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A GitLab project that merges through merge requests and publishes nothing. It carries no `.gitlab-ci.yml` of its own. The operator asked for the commit contract and the merge-request title gate, with cleanup. `rk profile --target .` names the forge from the remote, no release-bearing technology, and a release mode of `none`.

## Route

1. `rk guide landing` step 1a: run `rk --version`. The installed version is the one the operator chose.
2. `rk profile --target .`: the capability selection reports the title gate as selected, naming both the fragment and the minimal root pipeline that activates it, and reports the release automation as not requested.
3. `rk guide landing` step 1c: run `rk stage --target . --release-mode none` and hold `<stage>`.
4. `rk guide landing` step 2a: read `<stage>/stage.json` and `<stage>/artifacts/`. The root pipeline is a candidate here because no release automation ships one; read it before it lands, because it is the file the forge executes.
5. `rk guide landing` step 2b: with neither a receipt nor useful history the class is a best-effort heuristic. The target owns no root pipeline, so no ownership question arises and no automatic overwrite can happen; a target that owned one would see the gate withheld and the include line named.
6. `rk guide landing` step 3: preview `rk init --release-mode none --target .`, read every `created` word, then run it with `--apply`. Production renders afresh.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`. Say plainly that the gate is blocking only once the project setting requires the pipeline, which `rk guide setup` step 3 covers and the operator runs.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names; the project setting that makes the pipeline required is a forge action the operator takes.
- Say merge request in this project's instructions, because the forge's own term is what its reader sees.
- Every uncertain ownership decision is shown to the operator, and an absent destination raises none.
- Nothing is copied out of `<stage>/artifacts/`, and `stage.json` is offered to no verb.

## Documentation

- project documentation: the title fragment and the root pipeline are candidates the production verb lands by their recorded kind.
- reference corpus: `<stage>/reference/forges` is read for what this forge enforces, and is not copied into the project.
