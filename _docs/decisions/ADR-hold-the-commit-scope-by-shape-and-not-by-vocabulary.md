# Hold the commit scope by shape and not by vocabulary

## Context and Problem Statement

A landing recorded a closed scope vocabulary and rendered it into four files. The list did two jobs: it validated the shape of a scope, and it enumerated the words. The first is a gate. The second is guidance, and fixing it at landing time made a project pay for every area it grew: a documentation migration could not name its new area until release-kit re-rendered.

## Considered Options

- `hold the shape and drop the vocabulary` — chosen.
- `add an open mode beside the list` — rejected: two modes are two renderers, two record shapes, and two sets of prose, for a list nothing else needs once the shape is a gate.
- `keep the list and widen it on request` — rejected: it leaves the round trip in place and only changes how often it is paid.

## Decision Outcome

Chosen option: `hold the shape and drop the vocabulary` — a coding agent picks a good scope from the repository's own history, and what it needs from release-kit is the shape enforced, not the word chosen.

The title check admits `[a-z0-9._/-]+`. The landed commit hook requires that a scope is there and names no list, because `conventional-pre-commit` takes no scope pattern of its own. `rk message --check` holds the shape at the desk, so the desk and the forge judge one language. The routing block carries the guidance.

Enforced by `landing:a-rendered-file-is-reproducible` and `landing:the-landed-guards-hold-the-message-content`.

## Consequences

- Good: a project names a new area the day it creates one, and the record carries one parameter fewer.
- Bad: a project that wanted its vocabulary fixed loses that gate, and its scopes stay consistent only through the guidance and its own history.
- Bad: the record's schema moves to 5, so a record written now refuses on an older binary.

## Status

Implemented: `src/landing.rs`, `src/landing/manifest.rs`, `src/commands/message.rs`, `blocks/agents-block.md.in`, `blocks/pre-commit-block.yaml.in`, the two shared title checks, `method/02-setup.md`, and `runbooks/setup.md`.

It supersedes the scope-list clause of `ADR-enforce-the-squash-title-where-the-forge-holds-it.md` and of `ADR-grow-the-routing-block-to-carry-the-commit-contract.md`, and nothing else in either.
