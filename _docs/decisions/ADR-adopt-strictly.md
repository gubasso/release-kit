# Adopt strictly

## Context and Problem Statement

Repositories landed before the record existed — this one included — have files and no manifest. `rk upgrade` refuses without a record, because there is no defensible three-way comparison against an unknown baseline, and `rk init --apply` refuses over the configured files, so the pre-record installed base is permanently outside the lifecycle unless something writes it a record.

## Considered Options

- `A strict verb of its own, rk adopt` — chosen.
- `No adopt path, manual reconciliation only` — rejected: hand-writing a manifest of digests is exactly the mechanical work a binary should do, and a hand-written one would be trusted by every later upgrade.
- `A baseline-blessing flag on rk init` — rejected: blessing whatever is on disk launders arbitrary drift into release-kit ownership — a hand-edited workflow becomes, by fiat, what release-kit wrote — and it puts two trust models under one verb, since `init` trusts the payload and refuses the disk while adoption interrogates the disk and refuses the mismatch.

## Decision Outcome

Chosen option: `rk adopt`, preview by default. It renders the candidate exactly as `rk init` would; every `rendered` destination must match byte for byte, or the whole adoption refuses listing every mismatch and every missing expected file in one run; a differing `seeded` file adopts with both digests recorded, so a later upgrade has a real baseline; a `state` file is recorded and never compared; and no target file is ever changed — the one write is the manifest, last, with `origin` recording the adoption. Mature landing tools share the shape: `terraform import` brings a resource under management, then plans against it.

Enforced by `landing:an-adoption-writes-the-record-and-nothing-else`.

## Consequences

- Good: a refusal list is an honest statement of a target's drift from canon, not an obstacle — fixing it is the adoption.
- Bad: a legitimately customized target cannot adopt as-is; its path is restoring the candidate bytes or a fresh landing with a reviewed diff.

## Status

Implemented — `src/commands/adopt.rs`; this repository is the first live case, in the dogfood phase.
