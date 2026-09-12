# Read the rk pin through a manager axis

## Context and Problem Statement

`rk depend` lands any other project into a target through a four-manager matrix: flake, mise, asdf, devbox. `rk self-depend` read one manager, the flake, and reported every other target as `no-flake`. So `rk` offered every other project through a matrix and offered itself through one manager. Two questions are arguable: where the manager list lives, and whether a status over that list judges.

## Considered Options

- One manager enum in the binary, owned by the self-depend axis and shared with `rk depend`, with detection in one module — chosen.
- The manager list as prose in the setup skill, read by the agent — rejected: a prose copy drifts at the first new manager, and nothing fails when it does.
- Two enums, one per verb — rejected: the verbs read the same files, and a manager present for one and absent for the other is a lie.
- Direnv as a fifth manager — rejected: `.envrc` loads a shell and records no version, so a sync would have nothing to move; it is a reported state beside the list.
- A judging status, exit 1 on `not-wired` — rejected: the verb has no `--check` mode, and `landing:status-judges-only-under-check` already splits reporting from judging for the landing.

## Decision Outcome

Chosen option: one enum, one detection, one entry per manager in the report, absent ones included. An absent manager is a fact, never a fault. `.envrc` sits outside the list. The status exits 0 for every state it reports.

Enforced by `packaging:the-pin-is-read-through-a-manager-axis`.

## Consequences

- Good: a fifth manager is one variant plus its matrix rows; `rk depend assess` and `rk self-depend status` cannot disagree on what a target carries; the flake keeps its two facts under its own entry.
- Bad: the status schema moved to its second version, and a consumer reading the flat flake fields reads the flake entry instead; `add` and `sync` still serve the flake alone until the venue matrix lands.

## Status

Implemented: `src/self_depend/manager.rs`, `src/self_depend.rs`, `src/commands/self_depend.rs`, `src/depend/target.rs`, `src/cli/depend.rs`.
