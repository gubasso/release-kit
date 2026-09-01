# Land the convention hooks as a marked block

## Context and Problem Statement

The forge protections refuse a violation minutes after the push; the desk learns nothing until then. Only the squash title was held to the commit contract, so the title was the first conventional line an author wrote. release-kit lints its own commits and shipped that discipline to no target.

## Considered Options

- `A marked block spliced into the target's .pre-commit-config.yaml` — chosen.
- `A whole-file seed` — rejected: most targets already own a hook config; a seed overwrites it or lands nothing.
- `Embedding the checks in rk` — rejected: the ecosystem's hook manager already runs at the right stages; release-kit owns the block's lines, not the tool.

## Decision Outcome

Chosen option: `the marked block`, rendered and spliced under the target's `repos:` key, carrying six hooks, each mirroring a named rule: the scoped conventional message on every commit, no commit on the trunk, no push to the trunk ref, no hand-authored `v*` tag, the documented branch-name forms, and `rk status --check` on the owned surfaces. General hygiene stays the target's own per `landing:a-landed-hook-serves-the-release-convention-alone`, the splice owns only its marked lines per `landing:a-block-destination-owns-its-marked-lines-alone`, and the third-party hooks carry `versions.toml` pins.

Three honest limits: every mirror dies to `--no-verify`: the forge protections remain the enforcement, the hooks the refusal at the desk — the same rules at two distances; a force-push has no mirror, because git tells a pre-push hook nothing about it, so the ruleset's `non_fast_forward` rule stays its owner; and the branch guard reads the checkout, so a trunk CI sweep runs with `SKIP=no-commit-to-branch` — CI commits nothing.

The branch-name hook shifts no model fact: the name still binds nothing downstream; it only holds the routing to the documented forms.

## Consequences

- Good: a `wip` commit, a commit on `master`, and a hand-pushed tag are refused in seconds in every landed target.
- Bad: the commit-msg and pre-push stages run only where installed; the block's first line carries the install command.

## Status

Implemented — `src/landing.rs` carries the block and its splice.
