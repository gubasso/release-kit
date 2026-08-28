# Extract the canon from a running workflow

## Context and Problem Statement

Release-workflow knowledge lived as prose shelves in a knowledge base, written ahead of practice. The shelves drifted: they documented the bot publishing on the release-request merge and the release branch fast-forwarding onto the tag afterwards, which puts the publish before the human gate. Meanwhile one repository ran the corrected design — gate first, publish on the gate merge — end to end, with every guard earned by a failure it had actually seen.

## Considered Options

- `Generalize the workflow that spec-driven-docs runs, and keep it the reference implementation` — chosen.
- `Rewrite the knowledge-base shelves in place` — rejected: prose with no running instance behind it is what drifted the first time.
- `Design the canon fresh from the tools' documentation` — rejected: the guards that matter — the pinned gate branch, the app token, the enforcement switch — were each learned from a live failure no tool document names.

## Decision Outcome

Chosen option: `Generalize the running workflow` — every sentence in the method traces to behavior a repository exercises, so a claim that stops being true shows up as a broken release rather than as silent rot. The technology-agnostic spine and the per-technology bindings are the two layers; the diff surface between them is held to four axes.

## Consequences

- Good: the canon inherits corrections the reference implementation already paid for, including the recovery paths its happy-path guide once lacked.
- Good: a gap found while operating the reference flows into the canon instead of a private fix.
- Bad: the canon leans toward one forge and one registry family until other bindings run somewhere real.

## Status

Accepted.
