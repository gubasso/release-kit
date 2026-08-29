# Make the mechanical sentinel a parameter

## Context and Problem Statement

The landed workflows carried one hand-filled value — the repository owner, as an `OWNER` token behind a `TODO(release-kit)` marker, nine occurrences across three GitHub workflows plus one per GitLab pipeline. Filling them is required by the setup, and every fill made the landed file differ from its payload, so a configured target could never be compared against what release-kit ships: the byte comparison in `rk init` refused on exactly the edits the setup demanded, and no upgrade path could exist.

## Considered Options

- `Substitute the value at landing, recorded as a parameter` — chosen.
- `Keep the sentinels and drop the byte comparison` — rejected: coherent, but it abandons drift detection entirely, and with it `rk status`, `rk upgrade`, and `rk adopt`.
- `Keep the sentinels and the refusal` — rejected: it accepts that a configured target can never re-land, which is the failure this decision exists to remove.

## Decision Outcome

Chosen option: `a landing parameter`. The owner derives from the target's remote in the same detection pass the setup uses, with `--repo` as the override, and the resolved path is recorded whole in the manifest's `parameters.repo` — so a `rendered` file's bytes stay a deterministic function of payload plus recorded parameters, and every later command can re-render the candidate and compare. The sentinel report keeps its job for judgment sentinels only, which live in `seeded` files where an edit is expected; for a rust landing the reported count drops from nine to one.

Enforced by `landing:a-rendered-file-is-reproducible` and `landing:a-rendered-file-carries-no-judgment`.

## Consequences

- Good: a configured target is comparable for its whole life, which is what makes ownership, status, and upgrade possible.
- Good: one less class of hand-filled value that can land wrong silently.
- Bad: `rk init --apply` now requires a resolvable repository path, so a repository with no remote must pass `--repo`.

## Status

Implemented — `src/landing.rs` renders the substitution, and the record carries `parameters.repo`. Takes up the deferred option in `ADR-land-snippets-with-sentinel-placeholders.md`, whose judgment sentinels stay as decided there.
