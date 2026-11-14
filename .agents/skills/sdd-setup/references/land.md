# Task: land

The first landing in a target that holds no instance and no corpus. The gate picks it for the `setup` state.

## Pre-flight

Ask each choice once, in the plan, with its default:

| Choice        | Default                                                        | Exception                                                               |
| ------------- | -------------------------------------------------------------- | ----------------------------------------------------------------------- |
| profile       | `codebase`                                                     | `knowledge-base` where the repository is a corpus of notes with no code |
| docs scratch  | the entry of kind `observed` in `paths.proposals.docs_scratch` | ask the operator where there is none; `none` is allowed                 |
| writing style | `builtin`                                                      | `project:<path>` where the project carries a style document             |

The reserved paths come from the `tune-gates` pre-flight, which runs before the Stage step.

## Execution

Fill the landing pipeline with:

- Stage: `sdd stage --target . --profile <profile> --docs-scratch <scratch> --writing-style <style> --json`, with one `--reserve <path>` per reserved path and `--acquisition host` where `provision` recorded a host decision.
- Prepare: no item of its own.
- Land: `sdd init --stage <root>` to preview, then `sdd init --stage <root> --apply` as the gated step. It reads every choice from the receipt, so the landing cannot differ from the stage.

The ignore entry for the docs scratch is an environment item of the general post-flight, because `sdd init` writes nothing into the scratch.

## Post-flight

1. The Prove step passed.
2. `sdd status --target . --json` reads `classification` `current`. Where `provision` held a host answer, `sdd self-depend status --target . --json` reads `host`.
3. The report lists the files `sdd init` wrote, with the Prove step and `current` as the proof, and its Next line routes to the `sdd-write-docs` skill to start writing.
