# Land snippets with sentinel placeholders

## Context and Problem Statement

The landed files carry project-specific values: the repository owner, secret names, an environment name. `rk init` must land files that are correct everywhere except those values, and the values must not land silently wrong.

## Considered Options

- `Literal files carrying TODO(release-kit) sentinel lines that apply reports` — chosen.
- `A template engine substituting values from flags or prompts` — rejected: it buys a templating language, an escaping story, and an interactive surface to solve a handful of one-line edits an agent or a human makes better in context.
- `Deriving the values from the target's git remote` — deferred: revisit for the owner value alone if sentinel-filling proves error-prone; secrets and environments stay undeterminable from a checkout.

## Decision Outcome

Chosen option: `literal files with sentinels` — the payload under `snippets/` is byte-for-byte what lands, so reading the repository is reading what a target receives. Every sentinel is a grep-able `TODO(release-kit)` marker; `rk init --apply` scans the landed files and prints each marker with its path and line, so a landing is never silently half-configured. The rk-setup skill owns filling them.

## Consequences

- Good: the snippets stay valid, runnable-looking files a reader can review without mentally executing a template.
- Good: the lander stays a copy with a conflict check, small enough to hold no bugs of its own.
- Bad: a forgotten sentinel fails at workflow runtime, not at land time; the printed report is the only guard.

## Status

Accepted.
