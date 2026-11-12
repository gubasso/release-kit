# Retire an unedited dropped destination

## Context and Problem Statement

A landing left every destination the installed binary stopped producing on disk, as the target's own file. The python pair moved from release-please to the rk engine, so an upgrade stops landing `.github/workflows/release-please.yml`. Left in place, that workflow runs on every push and opens a second release request beside the engine's.

## Considered Options

- `Remove a dropped whole rendered file whose bytes the receipt still vouches for` — chosen.
- `Keep every dropped file, and leave its removal to a guidance step` — rejected: a missed step leaves two release mechanisms running, and nothing reports the second.
- `Remove every dropped file` — rejected: an edited, seeded, or state file holds the target's own bytes.
- `Save a copy, then remove` — rejected: an unedited file holds only bytes its release reproduces.

## Decision Outcome

Chosen option: remove an unedited dropped file. A whole `rendered` file the projection no longer produces, whose bytes match the receipt's digest, is `retired`: the landing removes it, and it leaves the receipt. Every other such destination is `dropped`: it stays on disk and leaves the receipt. That covers a seeded or state file, a marked region, an edited file, and one the target's state withholds. `rk stage`, the comparison, the upgrade preview, and the apply decide through one function. The rule covers a capability turned off too, and `rk status --check` names an edited release workflow that stays.

Enforced by `landing:a-dropped-file-stays` and `release-engine:no-predecessor-workflow-runs-beside-the-engine`.

## Consequences

- Good: an upgrade that moves a pair to another release tool leaves one release mechanism.
- Good: a capability turned off takes its unedited workflow with it.
- Bad: an upgrade now deletes files, so the preview and the stage name each one first.
- Bad: an edited workflow stays, and only the status check and the guidance reach it.

## Status

Implemented: [`src/landing/apply.rs`](../../src/landing/apply.rs), the function `dropped`.
