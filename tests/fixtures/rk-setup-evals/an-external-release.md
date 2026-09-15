# An external release: the guards land, the automation does not

- case: an-external-release
- landing: yes
- receipt: missing
- history: none
- legacy: none
- stage: cleaned

## Situation

A GitHub repository that releases through a process the operator's organization owns, outside release-kit. The operator asked for the commit contract, the title gate, and the reporting policy, and said plainly that the release path stays where it is. `rk profile --target .` proposes an automatic release from the crate it observed; the operator answers `external`, and the report then names the release automation as not requested.

## Route

1. `rk guide landing` step 1a: run `rk --version`. The installed version is the one the operator chose.
2. `rk profile --target .`: read the proposal, ask the release mode, and record the answer. An external release is the target's own: no bot is configured, and `rk method operate` is not the chapter for it.
3. `rk guide landing` step 1c: run `rk stage --target . --release-mode external --reporting-policy` and hold `<stage>`.
4. `rk guide landing` step 2a: read `<stage>/stage.json`. The candidates are the guards, the title gate, and the policy; no release workflow and no bot configuration appear.
5. `rk guide landing` step 2b: with neither a receipt nor useful history the class is a best-effort heuristic, so every uncertain ownership decision is shown to the operator and no automatic overwrite happens. The target's existing release files belong to no capability here, and nothing proposes touching them.
6. `rk guide landing` step 3: preview `rk init --release-mode external --reporting-policy --target .`, read every word, then run it with `--apply`. Production renders afresh.
7. `rk guide landing` step 4: compare `git diff --stat` with `<stage>/artifacts/`, then run `rk status --check --target .`. The target's own release files are untouched, which the diff shows.
8. `rk guide landing` step 5a: show `rk stage clean <stage>`. Run it only because the request's authority includes cleanup.

## Authority

- The request authorizes the file changes and the `rk` verbs it names, and names no release action at all.
- Never route an external release to the bot-based operate chapter, and never propose the release-bot setup steps: `rk setup` reports each as not applicable.
- Nothing is copied out of `<stage>/artifacts/`, and `stage.json` is offered to no verb.

## Documentation

- project documentation: `SECURITY.md`, the title gate, and the routing block are candidates the production verb lands by their recorded kind.
- reference corpus: `<stage>/reference/method` is read for what an external release leaves out, and is not copied into the project.
