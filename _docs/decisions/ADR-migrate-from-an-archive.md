# Migrate from an archive

## Context and Problem Statement

A migration from another release tool ran in place: the agent landed over the predecessor's files and reconciled each one by hand. The forge settings lived only on the forge, no run recorded what it decided, and a run that stopped midway left a state no verb could read back.

## Considered Options

- `archive every predecessor file, land a greenfield instance, and carry what the project keeps one group at a time, with a registry only the verbs write` — chosen.
- `keep the in-place migration and log its decisions in prose` — rejected: a log nothing reads proves nothing, and an in-place landing still collides with every predecessor file.
- `delete the predecessor files and trust Git` — rejected: Git holds the files, never the forge settings or the branches a migration retires.

## Decision Outcome

Chosen option: `archive every predecessor file, land a greenfield instance, and carry what the project keeps one group at a time, with a registry only the verbs write`. The archive moves the files into a workspace under the state root and takes the snapshots no Git holds, so the landing meets no collision and needs no `--replace`. The registry records each source's status and its history, the committed configuration names the open migration for every clone, and the close is the gate. The workspace is the one state a run keeps, because a migration spans many runs.

Checked against the snapshot and replace rules of the landing record, `ADR-snapshot-forge-state-before-a-destructive-step`.

Enforced by `migration:an-archive-moves-only-committed-files`, `migration:a-destructive-step-waits-for-its-snapshot`, and `migration:close-refuses-an-unfinished-migration`.

## Consequences

- Good: no byte and no forge setting is lost, and a resumed run reads where it stands from `rk migrate status`.
- Bad: the forge snapshots need credentials, so a host without them leaves the destructive steps refused until a host with them runs the archive again.

## Status

Implemented: `src/migrate.rs`, `src/commands/migrate.rs`, `src/commands/setup.rs`, `method/10-migration.md`, `runbooks/migration.md`.
