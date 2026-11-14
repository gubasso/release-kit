# Task: migrate

Archive the old corpus, land a greenfield instance, and rebuild the documentation from the archive, one group at a time. The gate picks it for the `migration` state, which an open migration keeps until it closes. Read `sdd docs migration` for the procedure, the registry, and why each step holds.

## Pre-flight

1. Read `migration` from `sdd status --target . --json`. Where it names an id, the migration is open: run `sdd migrate status --target . --json`, and resume at the first step its sources have not finished.
2. Run `sdd migrate inventory --target . --json`, and hold every document with its kind and placement.
3. Confirm with the operator that the inventory names every document they know of.
4. Where the operator asks for the incremental scope by name, follow `sdd docs migration` for it. Skip the archive, and land with `sdd init --stage <root> --incremental`, which keeps the corpus and records its debt. The map, the loop, and the close do not run.

## Execution

1. Archive. Show the preview of `sdd migrate archive --target .`, then run `sdd migrate archive --target . --apply` as a gated step. Where it refuses a document that differs from the trunk's copy, the operator integrates its edit onto the trunk first. Where the remote's `HEAD` names no trunk, add `--trunk <ref>`. Where this clone holds no workspace for an open migration, run `sdd migrate archive --target . --rebuild --apply`.
2. Land. Fill the landing pipeline with the choices `land` asks:
   - Stage: `sdd stage --target . --profile <profile> --docs-scratch <scratch> --writing-style <style> --json`, with one `--reserve <path>` per reserved path and `--acquisition host` where `provision` holds a host answer.
   - Land: `sdd init --stage <root>` to preview, then `sdd init --stage <root> --apply` as the gated step.
3. Map. Read the sources from the workspace in related sets. Tag each decision record of the previous convention by topic, and propose one group per topic and per set that becomes one destination. Present the map and wait for the operator's approval, then record it:
   - `sdd migrate mark <source> mapped --group <group> --tag <topic>`
   - `sdd migrate mark <group> mapped --destination <path>`
   - `sdd migrate mark <source> dropped --reason "<why>"`, for each drop the operator approved.
4. Loop, one group at a time:
   1. Read every source of the group from the workspace into this context.
   2. Write its destinations under the rules of the `sdd-write-docs` skill, each new document named by a slug. A group of decision records becomes one record whose considered options name each old decision. An in-place source is rewritten where it sits.
   3. Run the gates over the destinations, and read each destination against its sources.
   4. Run `sdd migrate mark <group> written`, then `sdd migrate mark <group> verified`.

A discovery is a document the inventory missed, a split the map did not name, or a deletion nobody approved. It returns to the map, and is never handled inline.

## Post-flight

1. Run `sdd migrate close --target .`. Where it names an unfinished source, return to the loop.
2. Run `sdd migrate close --target . --apply` as a gated step, then confirm that `sdd status --target . --json` reads `classification` `current`. Where `provision` held a host answer, `sdd self-depend status --target . --json` reads `host`.
3. List the workspace path under "Saved before changing" in the report. Keeping or removing the archive is the operator's step, by that exact path.
