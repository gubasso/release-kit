# A knowledge base: no technology, no forge, no release

- case: a-knowledge-base-with-no-forge
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A documentation repository with no version file, no origin remote, and nothing to publish. The operator asked for the commit contract and the branch guards, with cleanup. `rk profile --target .` names no technology, no forge, and a release mode of `none`; the capability selection reports the local guards as selected and every forge capability as not applicable.

## Route

1. `rk guide landing` step 1a: run `rk --version`. The installed version is the one the operator chose.
2. `rk profile --target .`: read the answers by domain, and confirm the release mode with the operator. A repository that publishes nothing is a valid target, so nothing here is a gap to fill.
3. `rk guide landing` step 1c: run `rk stage --target . --release-mode none` and hold `<stage>`.
4. `rk guide landing` step 2a: read `<stage>/stage.json`. The candidates are `AGENTS.md`, `GLOSSARY.md`, and `.pre-commit-config.yaml`; the omissions name every forge capability with the value that decided it.
5. `rk guide landing` step 2b: with neither a receipt nor useful history the class is a best-effort heuristic. Every candidate is a marked region or an absent file, so no ownership question arises and no automatic overwrite can happen.
6. `rk guide landing` step 3: preview `rk init --release-mode none --target .`, read every `created` word, then run it with `--apply`. Production renders afresh.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`. A guards-only target reads as healthy.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names; every git action stays the operator's.
- No forge step is proposed: `rk setup` reports each as not applicable, and naming one by hand refuses.
- Every uncertain ownership decision is shown to the operator, and an absent destination raises none.
- Nothing is copied out of `<stage>/artifacts/`, and `stage.json` is offered to no verb.

## Documentation

- project documentation: the routing block in `AGENTS.md` and the glossary are the whole landing, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read and not copied into the project.
