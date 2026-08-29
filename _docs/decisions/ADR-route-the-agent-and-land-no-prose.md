# Route the agent and land no prose

## Context and Problem Statement

The invariants in `method/01-invariants.md` govern every landed repository and reach none of them: after `rk init`, a maintainer or agent can rename the registered workflow, grant a second workflow the OIDC permission, or push a tag, and nothing on disk says a convention forbids it until a release fails. The obvious fix is landing a restatement of the invariants beside the workflow files.

## Considered Options

- `A marked routing block in the target's AGENTS.md, and nothing else` — chosen.
- `Binary-only, as before` — rejected: the repository contains no sentence that would stop any of the violations above.
- `A landed invariants digest plus the routing block` — rejected: a landed restatement is a second copy of canon, and everything that makes it look safe — rendered, upgrade-refreshed, drift-detected — is maintenance machinery added to every consumer to keep the copy true; the maintenance path is the copy's cost, not its defense.

## Decision Outcome

Chosen option: `the routing block alone`. What the on-disk audience needs is not the invariants restated at one screen; it is to learn, before editing, that these files are owned, that a convention governs them, and where the convention lives. Four lines of routing do that: the block names `rk status`, `rk method invariants`, and the record, is spliced between markers so release-kit owns the lines and not the document, and is `rendered` — digest-tracked in the record, kept current by `rk upgrade`, its drift detectable while everything outside the markers stays the target's own. A fifth line needs the scrutiny a new spec rule gets.

Enforced by `landing:a-rendered-file-is-reproducible`, which covers the block like every rendered artifact.

## Consequences

- Good: one owner per durable fact holds — the canon stays in the binary, the target carries only the pointer.
- Bad: an environment without `rk` gets a pointer naming a tool to install, not the rules themselves; that is the truthful version of the promise.

## Status

Implemented — `src/landing.rs` carries the block and its splice.
