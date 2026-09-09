# Prompt an upgrade from content and not from the version

## Context and Problem Statement

The landing record carries `rk_version`, stamped from `CARGO_PKG_VERSION` at every landing, adoption, and upgrade, and `rk status` printed "run 'rk upgrade'" whenever the binary's version was higher. release-plz derives the version from the commit that already carries the record, so the recorded version lags by one release the moment it ships, and a release touching no landed file told every consumer to upgrade. The record was refreshed by hand after 0.2.23, 0.3.0, and 0.3.2, and the refresh can never hold.

## Considered Options

- `Prompt from what an upgrade would change` — chosen.
- `Prompt from the payload digest` — rejected: seven of the ten payload roots reach no target, so a prose edit to `method/` moves the aggregate digest while changing no landed byte.
- `Refresh the record inside the release request` — rejected: release-plz documents no hook, force-pushes that branch on every refresh, and the landed workflow arms it for auto-merge in the same job.
- `Drop rk_version from the record` — deferred: revisit if the downgrade refusal is ever keyed on `schema_version`, which would leave the field with no reader.

## Decision Outcome

Chosen option: `prompt from what an upgrade would change` — the report projects the payload under the recorded parameters and counts the destinations an upgrade would add, drop, reclassify, or rewrite. `rk_version` keeps provenance and the downgrade refusal, and prompts nothing. The comparable tools agree: copier and cruft record a template revision and no tool version, `Cargo.lock` records a format version alone, and Terraform's `terraform_version` refuses a newer writer while prompting nothing.

Enforced by `landing:status-judges-only-under-check`.

## Consequences

- Good: the prompt is true, so a release that changes nothing for a target says nothing to it and the recorded version is free to lag.
- Bad: status renders the projection on every run, doing the work an upgrade preview does, and a pair this binary cannot project reports an unknown rather than a count.

## Status

Implemented — `pending_of` in `src/commands/status.rs`, over the projection `src/landing.rs` builds.
