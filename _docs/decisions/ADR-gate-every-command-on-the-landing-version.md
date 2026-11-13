# Gate every command on the landing version

## Context and Problem Statement

Once the skills land at a project's pin, `rk_version` in `.release-kit/manifest.json` names the rk whose skills and files the project carries. A binary of another version that writes the target, or reads it and acts, works against texts it did not ship. `rk status` prompted an upgrade only from content, so nothing stopped that.

## Considered Options

- `One class per command, and a check before the handler` — chosen.
- `Warn on every mismatch` — rejected: a warning does not stop a write that mixes two releases.
- `Refuse on every mismatch` — rejected: the hooks of a project run read-only checks, and a pin move followed by a commit would block until `rk upgrade` lands.
- `An escape hatch that skips the gate` — deferred: revisit if a real case needs one.

## Decision Outcome

Chosen option: `one class per command, and a check before the handler`. Each action is `exempt`, `pre-landing`, `upgrade`, `read-only`, or `writing`. A target with no record refuses and names `rk init`. A binary newer than the record refuses a write and names `rk upgrade`, and lets a read run with one warning line. A record newer than the binary refuses and names the rk of the project's own environment. The gate never writes and never moves a pin.

cargo-dist refuses a mismatched `cargo-dist-version` with "Rerun 'dist init' to update to this version", and uv and ruff refuse a failed `required-version`. Prisma warns. The split between a refused write and a warned read is this convention's own policy.

Enforced by `version-gate:every-command-has-a-gate-class` and `version-gate:a-gated-command-names-its-fix`.

## Consequences

- Good: no rk of another release writes a landed target.
- Bad: a release that changes no landed file still asks for `rk upgrade`, which then moves only `rk_version`. [The content-based prompt](./ADR-prompt-an-upgrade-from-content-and-not-from-the-version.md) avoided that cost. This record accepts it, because the skills now follow `rk_version`.

## Status

Accepted
