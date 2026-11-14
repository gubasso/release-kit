# Task: tune-gates

Change what the gates judge, the `reserved:` and `gates:` entries of the declaration, and the lint configuration that serves them. The two travel together, because each reserved path that holds Markdown also needs a lint ignore. Read `sdd docs gates` for how the four scope layers and the two lint configuration families compose.

## Pre-flight

1. Run `sdd lint --target . --json`. Each finding carries its `path`, its `rule`, and the `edit` it needs. A configuration the binary cannot read reads `unknown`, and you inspect it yourself.
2. Ask once: "Does another tool render or hash any path here?" Each path the operator names is a reserved path.
3. Where the report finds nothing and the operator reserves nothing, end the task with no write.

## Execution

Scope:

- With no instance yet, pass each reserved path to the Stage step of `land` or `migrate` as `--reserve <path>`.
- With an instance, edit `reserved:` or `gates:` in the declaration that `paths.active.destinations.declaration` names, then run `sdd hooks --target . --apply`. It validates the declaration and renders the managed block.

Lint, one gated edit per finding, each proposed with the rule ID it cites:

1. Fold a stray configuration into the root `.markdownlint-cli2.*` configuration, then remove the stray file.
2. Load and enable `relative-links`, and keep every existing entry. With a `.markdownlint-cli2.*` file, merge `markdownlint-rule-relative-links` into its `customRules`. With a `.markdownlint.*` file alone, keep it and add a companion `.markdownlint-cli2.*` file at the repository root that carries the loader, because `customRules` belongs to the `.markdownlint-cli2.*` family ([upstream configuration contract](https://github.com/DavidAnson/markdownlint-cli2/blob/v0.24.0/README.md#configuration)). Enable `"relative-links": true` in the `.markdownlint.*` file where one exists, otherwise in the `config` of the `.markdownlint-cli2.*` file.
3. Add each reserved path that holds Markdown to `ignores` in the root `.markdownlint-cli2.*` configuration, the companion file included.

## Post-flight

1. Run `sdd lint prove --target .`. A broken relative link fails with `relative-links`, a valid link passes, and the scratch file is gone. A broken link that lints clean is a failed proof.
2. Run `sdd gate --explain <path>` for each reserved path, and confirm each reads as reserved.
