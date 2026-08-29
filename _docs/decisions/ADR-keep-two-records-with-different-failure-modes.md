# Keep two records with different failure modes

## Context and Problem Statement

The binary now writes two records: the user-scope skill record at `src/skills/record.rs`, which resolves every unreadable shape to an empty record, and the landing record at `.release-kit/manifest.json`. One code base with two record types, one lenient and one strict, looks like an inconsistency waiting for a cleanup — so the difference has to be deliberate and on record, or someone will unify them.

## Considered Options

- `Two records, each with the failure mode its job requires` — chosen.
- `One record type for both` — rejected: one parser must then pick one failure mode, and either the skill install starts refusing on records nothing verifies against, or the landing commands start guessing from records they could not read.
- `Make the skill record strict too` — rejected: the skill record is state that only grants the benefit of the doubt; the worst outcome of losing it is a `--force` prompt, and a refusal there would block installs over a file nobody depends on.

## Decision Outcome

Chosen option: `two records` — because what each answers differs in kind. The skill record answers "are these bytes ones we wrote?", nothing verifies against it, and the cost of losing it is a prompt, so every unreadable shape resolves to empty, silently. The landing record is a manifest: `rk status` reports from it and `rk upgrade` decides three-way comparisons from it, so it earns a parser that can fail, a stated integer `schema_version`, and a refusal that names the record — an unknown schema is never best-effort read.

Enforced by `landing:a-record-states-its-schema`.

## Consequences

- Good: each record fails the way its readers need — the lenient one never blocks an install, the strict one never lets an upgrade act on a guess.
- Bad: two record implementations to maintain, and the difference must be re-explained to anyone proposing the unification this record exists to answer.

## Status

Implemented — `src/landing/manifest.rs` is the strict parser, `src/skills/record.rs` the lenient one.
