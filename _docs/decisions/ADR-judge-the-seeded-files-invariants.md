# Judge the seeded file's invariants

## Context and Problem Statement

`dist-workspace.toml` is `seeded`, and three facts combined into a blind spot: an adoption records a seeded file as found, an upgrade never rewrites one and freezes its recorded baseline, and status compares disk against the record, never against the payload's current seed. A seed improvement therefore reaches no landed target, and this repository — origin `adopt` — exited `rk status --check` clean through three releases while carrying no attestation configuration at all.

## Considered Options

- `Judge the effective configuration under status --check` — chosen.
- `Reclassify dist-workspace.toml as rendered` — rejected: targets legitimately choose platforms, installers, and install paths; owning the file to hold four keys confiscates the rest.
- `Advance the recorded baseline on upgrade` — rejected: an untouched file would then read as drift, inverting the signal's meaning.
- `Match a substring` — rejected: satisfied by a commented key, a `false` value, or the exact default-phase defect the seed fix addressed.
- `Report seed movement generically` — rejected: that signal already existed as informational seeded drift and went unread for three releases; a generic line is not a judgment.

## Decision Outcome

Chosen option: judge, never rewrite. A new module owns the rule set, keyed by `(technology, forge, destination)` so a second pair sharing a destination cannot inherit the wrong rule, and evaluated over parsed TOML. Each failure carries a stable code, the destination, the reason, and the exact remediation. Plain `rk status` reports and exits 0; `--check` counts each failure a violation. The machine report becomes `rk.status/2` with `invariant_failures` in both modes. `landing:a-seeded-file-is-never-rewritten` stands untouched beside the new rule. Enforced by `landing:a-seeded-file-still-carries-the-invariants` and the extended `landing:status-judges-only-under-check`.

## Consequences

- Good: the blind spot that let this repository violate its own invariant through three releases now fails the check the day it recurs, in every landed target.
- Good: the target keeps every freedom the seed contract promised.
- Bad: the rule set is payload knowledge inside the binary and must move together with the seed it judges.

## Status

Accepted.
