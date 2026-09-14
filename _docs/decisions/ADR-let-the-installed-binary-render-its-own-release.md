# Let the installed binary render its own release

## Context and Problem Statement

One `rk` binary fetches, verifies, caches, and interprets another release, stores a plan of every write, and revalidates it before applying. The workflow needs none of that, because the operator already owns the update of `rk` and the manager that does it. The question is which binary answers for a release, and what a landing does without a stored plan.

## Considered Options

- The installed binary renders only its own embedded sources, into a disposable stage for study and afresh into production, with one receipt written last and Git as recovery. Chosen.
- An old engine reading a newer release as data. Rejected: every release becomes a protocol the old engine must parse.
- `rk` installing a second candidate binary. Rejected: acquisition belongs to the operator and the tool manager.
- Applying staged bytes into production. Rejected: the stage would become state production depends on.
- A stored plan with fingerprint and revalidation. Rejected: the stage shows the proposed bytes, and Git shows what landed.
- Full automatic migration. Rejected: judgment belongs to the agent and the project's checks.
- Inline per-file version watermarks. Rejected: the receipt is the one watermark, and markers mark a region alone.
- Blind replacement of an unattributed file. Rejected: a file the receipt cannot vouch for is the target's own.

## Decision Outcome

Chosen option: candidate-owned rendering, because the installed binary is the only implementation that must understand the candidate, and rendering afresh keeps production independent of any stage. Production applies elementary ownership: create an absent candidate, replace a recorded generated file, preserve a seeded or state file, replace a marked region alone, refuse an unattributed collision.

Enforced by `staging:production-never-reads-a-stage`, `landing:ownership-is-elementary`, `landing:a-partial-landing-is-visible-and-rerunnable`, and `packaging:the-operator-selects-the-installed-version`.

## Consequences

- Good: one release per binary, no network on the landing path, and a comparison surface an agent can keep.
- Bad: a target two releases behind receives no intermediate step, and a partial landing is visible rather than rolled back.

## Status

Proposed. Supersedes `ADR-read-every-release-through-one-seam`, `ADR-ship-compatibility-and-guidance-in-the-bundle`, `ADR-apply-only-a-stored-plan`, and `ADR-plan-every-landing-write-as-one-typed-document`.
