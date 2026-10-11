# Task: upgrade

Take a record older than the installed `rk` to this binary's projection, and apply any changed answers in the same landing. The gate picks it at `alignment` `binary-newer`. `rk method landing` carries the why.

## Pre-flight

1. Read `config.pending` in `rk status --target . --json`. Ask each key the record predates, with its consequence. Ask first a key the upgrade refuses without, the release style: under `trunk`, the release request is armed and a green trunk ships on its own.
2. A record that predates the integration mode reads as `forge`. State that, offer the change once, and add the answer to the Stage step's flags.
3. A record that predates the workflow guards reads them as a pending answer. Ask once whether to keep them or drop them, with keep as the default.
4. Read `legacy_state` in the same report.
   - An entry with `active` true is an operation a 0.4.x release could still apply. Ask whether the operator finishes it with the old binary, or records that it is abandoned. The update waits for the answer. `rk guide landing` 1b carries the procedure.
   - Hold each inactive entry for the post-flight.

## Execution

Fill the pipeline:

- Stage: `rk stage --target . --json`, with the answers from the pre-flight as flags. The configuration answers the rest.
- Compare:

| Row                                        | What follows                                                                              |
| ------------------------------------------ | ----------------------------------------------------------------------------------------- |
| `rendered`, `differs`, `attributed`        | refreshed by the landing                                                                  |
| `differs`, `reduced` or `unknown`          | ask whether to keep the edit or discard it                                                |
| `retired`: this release no longer lands it | ask whether to remove it, as a gated step after a commit, or keep it as the project's own |

- Land: `rk upgrade --stage <root> --target .`, then the same with `--apply` and one `--replace <path>` per answered conflict, as a gated step.

## Post-flight

1. The Prove step passed, and `rk status --target . --json` reports `classification` `current`.
2. Regenerate what a landed configuration generates, such as the artifact plan, through the binding's step, before any check reads it. `rk guide setup` 4b carries the command.
3. Remove each retired file the operator chose to remove, as a gated step.
4. Remove each inactive legacy path by its exact path, only under explicit cleanup authorization, as `rk guide landing` 5b carries it. No `rk` verb removes this state.
5. Route each saved copy as the pipeline says.
