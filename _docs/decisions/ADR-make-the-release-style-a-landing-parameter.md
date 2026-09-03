# Make the release style a landing parameter

## Context and Problem Statement

The method carries two release styles, but the choice lived as prose in the model's table. Nothing observable recorded which style a project runs, so a clone, a CI job, and an agent could not know whether the release request stands armed, whether a release line is expected, or which runbook variant applies.

## Considered Options

- `A committed landing parameter, mirroring the workflow mode` — chosen.
- `Infer the style from the presence of a release/* branch` — rejected: a branch is state, not a decision, and retiring the last line would silently change the style.
- `A per-clone configuration toggle` — rejected: the style decides what a shared workflow does, so a clone-local answer puts different truths in different clones.
- `Prose only` — rejected: nothing observable; every reader guesses.

## Decision Outcome

Chosen option: `a landing parameter` — `parameters.style` is `trunk` or `lines` at manifest schema 3, set by `rk init --style` with `trunk` the default, changed only by `rk upgrade --style`, reported by `rk status`, and resolving the runbooks' `On trunk:` and `On lines:` variants. It renders into the landed release workflow as the one substituted value that arms or does not arm the bot's request, so a style change is a one-word reviewed diff. A record predating the field carries no style, and an upgrade refuses until the operator names one: neither value is a compatibility-safe reading of a target nobody asked. Enforced by `landing:the-release-style-is-a-landing-parameter`.

## Consequences

- Good: one committed answer every reader shares, and a style change is a reviewable diff.
- Bad: every pre-style landing pays one `--style` flag on its next upgrade.

## Status

Accepted.
