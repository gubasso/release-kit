# Serve a dependency and edit no owned manifest

## Context and Problem Statement

Taking another project as a dependency had no verb and no owner: the devshell pin covers release-kit alone, and an agent judged the source's distribution and the target's tool manager from prose. Four managers and five channels make twenty pairs, some needing knowledge no offline read has; a guessed nixpkgs attribute or asdf plugin is a broken shell.

## Considered Options

- `a read-only assessment and an add verb that serves fragments, seeds only an absent file, and prints the native command for a library` — chosen.
- `a generalization of rk devshell to any flake input` — rejected: the devshell pin is release-kit's own, with a mover and a transaction that no third-party pin shares.
- `skill prose that edits the manager file` — rejected: the judgment is untestable, and the packaging spec already refuses such a splice.
- `a mover for third-party pins` — deferred: revisit if a project asks `rk` to move a pin its manager already moves.

## Decision Outcome

Chosen option: `a read-only assessment and an add verb that serves fragments, seeds only an absent file, and prints the native command for a library`. `rk depend assess` reads both sides offline and exits 0 on every verdict; `rk depend add` renders the pair from authored blocks, seeds a manager file only where the target has none, refuses over an owned one, and for a prod dependency prints `cargo add`, `uv add`, or `npm install`. An unjudgeable pair is manual with a closed reason. A URL is refused; the clone is the operator's step.

Enforced by `dependencies:add-edits-no-file-the-target-owns`, `dependencies:a-prod-dependency-lands-through-the-native-command`, and `dependencies:an-unjudgeable-pair-is-manual-with-its-reason`.

## Consequences

- Good: every landing is printed text or a command, reviewable before a file changes.
- Good: the manager that took the pin keeps its own freshness verb; no second mover competes.
- Bad: an owned manager file is edited by hand, and a manual pair costs the operator the knowledge the binary refuses to guess.

## Status

Implemented; `src/depend/`, `src/commands/depend.rs`, `method/11-dependencies.md`, `runbooks/dependencies.md`, and `skills/rk-depend/SKILL.md` enact it.
