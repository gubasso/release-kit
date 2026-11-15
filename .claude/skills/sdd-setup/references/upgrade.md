# Task: upgrade

Move an instance older than the `sdd` on PATH to that version. The gate picks it for the `upgrade` state. Read `sdd docs reconcile` for the reconciliations an upgrade can raise.

## The skill that runs the upgrade

Run the upgrade from this skill as the new release ships it. After a pin move, `sdd` prints:

```text
After the pin move, load the devshell and run the upgrade through the sdd-setup skill of the sdd in that devshell, and do every migration it names. Before `sdd upgrade`, read it with `sdd skill show sdd-setup`. After it, the project's .claude/skills/sdd-setup is that skill. Never use a host copy or an older skill.
```

1. Load the devshell, then run `sdd --version`, and compare it with the pin.
2. Before `sdd upgrade` runs, read each file of this skill with `sdd skill show sdd-setup --file <path>`, because the project's copy holds the older release's skill until the upgrade writes the new one.
3. After the upgrade, read this skill from the project's copy, which the upgrade wrote.
4. Never read a copy of this skill under the home.

## Pre-flight

1. Run `sdd upgrade --target . --dry-run --json`, and hold `retired_citations`: every citation of a rule ID this binary no longer defines, with its path and line.
2. Hold `legacy_state` from `sdd status --target . --json`: what an older release left under the state root.

## Execution

Fill the landing pipeline with:

- Stage: `sdd stage --target . --json`. It reads the recorded profile, docs scratch, writing style, and reserved paths, and it refuses a different `--profile`, because a profile change is a migration.
- Compare: route every row by the pipeline's own table. Each keep-or-discard answer names the path in `--replace`.
- Prepare: re-point each retired rule-ID citation to the replacement the changelog names. The operator approves each one, because the tool cannot know which rule the author meant.
- Prepare: never edit a retired citation inside a decision record, because a record is not revised. Write a new record whose `Enforced by` paragraph cites the rules that bind now, and set the old record's status to `Superseded by` with a link to the new one. The citation gate exempts a superseded record. The operator approves each new record.
- Prepare: write each approved port of a changed adopted spec, as the Compare step of the pipeline routes it. `sdd upgrade` alone keeps the old bytes of every adopted spec.
- Land: `sdd upgrade --stage <root> --dry-run` to preview, then `sdd upgrade --stage <root>` as the gated step, with one `--replace <path>` per answered conflict. An unnamed conflict still refuses.

## Post-flight

1. The Prove step passed, and `sdd status --target . --json` reads `classification` `current`.
2. Route each saved copy by the rule at the end of the landing pipeline.
3. Offer the removal of each `legacy_state` entry whose `active` is false, by its exact path, as a gated step.
4. Where a formatter, a linter, or a spell checker excludes the landed skills, it excludes only the package folders `sdd-setup` and `sdd-write-docs` under each skill root that the `paths` section of `sdd status --target . --json` reports. The project's own skills in those roots stay under its checks. Narrow an exclude of a whole skill root to those folders, as a gated edit.
