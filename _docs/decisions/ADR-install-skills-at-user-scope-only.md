# Install skills at user scope only and decide placement per project

## Context and Problem Statement

Skills could install at user scope, at system scope, or both — and the payload names destinations owned by third-party applications, starting with the two agent skill roots. Two questions: does a system scope exist, and how is such a destination chosen?

## Considered Options

- User scope as the only mode, and placement decided per project against the owning application's documentation — chosen.
- A system staging mode with a NixOS module for skills — rejected: the agents publish four different system layouts or none, with two collision models (Codex duplicates where Claude Code shadows), so exclusivity rules would be written per agent for a convenience one command already provides.
- Home Manager or another declarative user-scope module — rejected: imperative `rk skill install` works on every host including NixOS, and store symlinks would collide with `distribution:a-skill-destination-is-a-regular-file` and the record model.
- An `rk skill shared-root` resolver so skill texts could reach the shared root from a read-only vendor directory — rejected: its only motivation was system scope; under user scope the literal shared-root path stays correct.

## Decision Outcome

User scope only: one scope is one owner per skill name, and a per-user install is reversible by its user under the record protections already built. The two roots are minimal against the matrix in `REFERENCE-skill-scopes-sources.md`. And a third-party placement is decided inside each project, against that application's current documentation with a dated citation — never inferred from this repository's conventions, never generalized across applications; the matrix is the worked example, where a default right for one agent is wrong for the other three.

Enforced by `distribution:a-skill-has-one-owner` and `placement:a-third-party-destination-names-its-source`.

## Consequences

- Good: `Scope` stays a single-variant enum with an irrefutable destructure; no staging mode, vendor tree, or system path exists; a packaged `rk` documents the per-user `rk skill install` as its one follow-up step.
- Bad: a fleet operator scripts the per-user install; reopening system scope needs per-agent exclusivity rules, though no new research.

## Status

Implemented: `src/cli/skill.rs`, `src/commands/skill.rs`, `runbooks/setup.md`, `_docs/reference/REFERENCE-skill-scopes-sources.md`.
