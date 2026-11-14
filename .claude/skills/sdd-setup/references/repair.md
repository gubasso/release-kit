# Task: repair

Restore a managed file or a marker block whose bytes differ from the record, a deleted file included. The gate picks it for the `drift` state. An edited adopted file is never drift, because the project owns it.

## Pre-flight

1. Read `managed_drift` from `sdd status --target . --json`, and run `sdd verify --target .` to name each drifted file or marker block.
2. Where the failure is a gate citing a rule inside a document, hand it to the `sdd-write-docs` skill: that is authoring, not drift.

## Execution

Fill the landing pipeline with:

- Stage: `sdd stage --target . --json`, which reads the recorded values.
- Compare: route every drifted file or block by the pipeline's own table. Keep saves a copy, then replaces it. Discard replaces it.
- Land: `sdd upgrade --stage <root> --dry-run` to preview, then `sdd upgrade --stage <root>` as the gated step, with one `--replace <path>` per answer.

## Post-flight

1. The Prove step passed, and `sdd status --target . --json` reads `classification` `current`.
2. Route each saved copy by the rule at the end of the landing pipeline.
