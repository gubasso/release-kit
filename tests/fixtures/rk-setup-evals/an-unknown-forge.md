# An unknown forge: readable, local, and honest about the rest

- case: an-unknown-forge
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A repository whose origin remote is a forge this release has no adapter for. The operator asked for the commit contract and the branch guards, with cleanup. `rk profile --target .` preserves the forge name, reports every forge capability as unknown with that name in the reason, and selects the local guards.

## Route

1. `rk guide landing` step 1a: run `rk --version`. The installed version is the one the operator chose.
2. `rk profile --target .`: read the capability selection out to the operator, naming the forge the profile carries and saying that this release drives github and gitlab. The value is preserved rather than refused, so the target stays describable.
3. `rk guide landing` step 1c: run `rk stage --target . --release-mode none` and hold `<stage>`.
4. `rk guide landing` step 2a: read `<stage>/stage.json`. The candidates are the local guards alone.
5. `rk guide landing` step 2b: with neither a receipt nor useful history the class is a best-effort heuristic. Every candidate is absent or a marked region, so no ownership question arises and no automatic overwrite can happen.
6. `rk guide landing` step 3: preview `rk init --release-mode none --target .`, read every `created` word, then run it with `--apply`. Production renders afresh.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`, which reads the unknown forge as recorded rather than as drift.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names.
- Propose no forge operation: `rk setup` reports each as not applicable, and naming one by hand refuses and says which adapters exist.
- Every uncertain ownership decision is shown to the operator, and an absent destination raises none.
- Nothing is copied out of `<stage>/artifacts/`, and `stage.json` is offered to no verb.

## Documentation

- project documentation: the routing block and the glossary are the whole landing, and the production verb lands them by their recorded kind.
- reference corpus: `<stage>/reference/` is read and not copied into the project.
