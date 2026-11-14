# The landing pipeline

`land`, `migrate`, `upgrade`, and `repair` fill these five steps. Each task file names its stage flags, its landing verb, and its own Prepare items. Run the task pre-flight before step 1 and the task post-flight after step 5.

## 1. Stage

1. Run `sdd stage --target . --json` with the flags the task file names.
2. Hold the receipt: its `root`, its `version`, and its `artifacts`.

The stage writes nothing into the target. Keep it until the general post-flight cleans it, because a failure is diagnosed against it.

## 2. Compare

1. Run `sdd stage compare <root> --target . --json`, with `<root>` from the receipt.
2. Route each row by what the landing decides for it: its `state`, `ownership`, `vouched`, and `replaceable`. Show its `provenance` with each question, as how much the tool can say about who wrote the bytes. Where `history.available` is false, the Git half of provenance is missing, and `history.reason` says why.

| Row                                                                     | What follows                                                                                                   |
| ----------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `equal`, or `absent` with `replaceable` false                           | nothing to decide                                                                                              |
| `adopted` and `differs`                                                 | nothing to decide, because the landing keeps the project's bytes                                               |
| `managed` or `integration` with `vouched` true                          | the landing refreshes it, or takes a `retired` row back                                                        |
| `replaceable` true                                                      | ask the operator, as a gated step, whether to keep the edit or discard it; for `absent`, whether to restore it |
| `managed` or `integration` with `vouched` false and `replaceable` false | a refusal: report it, because no `--replace` name resolves it; the operator moves the file aside               |

Each folder in `refused_packages` holds a skill package that no record attributes. The landing refuses it whole, and no `--replace` name resolves it, so report it and ask the operator to move the folder aside.

Name in `--replace` only a path whose `replaceable` is true. Both answers name it, and `--replace` saves a copy before it overwrites. Never land over such a file without an answer.

An adopted file keeps the project's bytes, so its row reads `equal` even where this release changed the shipped copy. A changed spec reaches the project only by a port. Run `sdd status --target . --json` once. For each adopted file in the directory at `paths.active.destinations.specs.path`:

1. Run `sdd spec <name>` to print the shipped copy, with `<name>` the file name without `SPEC-` and `.md`. Where it equals the held file, nothing is to port.
2. Read the held file's entry in the instance manifest, at `paths.active.destinations.manifest.path`. Where the file's `sha256sum` still equals its `baseline_sha256`, the project never edited it, and the shipped copy replaces it whole.
3. Otherwise, read what changed in the spec in the release notes of each version after the recorded `canon_version`. Diff the held file against the shipped copy, list each shipped change those notes name, and keep each edit the project made.
4. Ask the operator, as a gated step, to approve each port. Write the approved ports at the Prepare step, so the Land step records their bytes.

## 3. Prepare

Write only what the project owns, and write it before the landing:

- the answers the operator gave at the Compare step,
- the Prepare items the task file names,
- the `tune-gates` edits, where the composition includes them.

## 4. Land

1. Run the landing verb of the task in its preview form, and show that output in the plan. Never list the destinations by hand.
2. After the operator approves, run the landing verb as a gated step.

## 5. Prove

1. Run `sdd stage compare <root> --target . --json` again.
2. Confirm that every `managed` and `integration` destination reads `equal`. An `adopted` destination can differ, because the landing kept the bytes of the project.
3. Carry the result into the proof line of the report.

Route each kept copy in the task post-flight: a lint configuration edit goes to `tune-gates`, and anything else goes to "Still the operator's" in the report. A discarded copy stays where `--replace` saved it, and the report lists it.
