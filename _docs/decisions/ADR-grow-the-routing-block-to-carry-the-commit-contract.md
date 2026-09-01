# Grow the routing block to carry the commit contract

## Context and Problem Statement

`ADR-route-the-agent-and-land-no-prose.md` fixed the routing block at four lines and set the bar for growth: a fifth line needs the scrutiny a new spec rule gets. The block routed an agent to the convention but said nothing an agent needs before its first write: that work branches before it starts, that the request's title becomes the trunk's commit message, and which scopes the project accepts. An agent asked to change code while the checkout sits on `master` had no landed sentence telling it to branch first.

## Considered Options

- `Three more lines in the block` — chosen.
- `Leave the block at four lines` — rejected: the commit contract now has landed enforcement — the title check, the hook block — and enforcement an agent discovers only by tripping it is a worse teacher than one landed sentence.
- `A landed prose chapter` — rejected already by the prior record; nothing here reopens it.

## Decision Outcome

Chosen option: `three more lines`, each clearing the prior record's bar by pairing with the mechanism that enforces it. Branch-first pairs with the branch-name and trunk-commit hooks; the title-is-the-message line pairs with the title check and the squash-title setting under `forge-setup:the-setup-asserts-the-squash-title-source`; the every-commit line pairs with the landed commit-msg hook and renders the scope list from the landing's `scopes` parameter, so the block stays a deterministic function of payload plus parameters per `landing:a-rendered-file-is-reproducible`.

This record amends the prior one and restates its bound: the block is seven lines of routing and contract, and a further line still needs the scrutiny a new spec rule gets — a line that names no enforcing mechanism does not clear it.

## Consequences

- Good: the agent-on-master case is answered by the first sentence an agent reads, and mechanically by the hooks when it does not read.
- Bad: the block now varies by a parameter, so two targets' blocks are no longer byte-identical; the record's `parameters.scopes` is what makes each reproducible.

## Status

Implemented — `src/landing.rs` carries the grown block.
