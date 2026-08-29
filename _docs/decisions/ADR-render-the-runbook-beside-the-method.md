# Render the runbook beside the method

## Context and Problem Statement

The method chapters are technology-agnostic by contract and the bindings carry only technology differences, so a command sequence identical across technologies has no home. An operator following `rk method operate` gets nine ordered paragraphs and zero commands; the one screen of commands a person actually follows with a gate open did not exist in the product.

## Considered Options

- `Add a runbooks root, held to the spine by a parity test` — chosen.
- `Put the commands in the method` — rejected: `method/03-operate.md` states its own contract that the binding supplies the concrete commands, and a 130-line command block would swamp a 25-line chapter.
- `Put them in each binding` — rejected: the same merge command is identical for rust, python, and bash, so it is not a technology difference; three copies of one sequence drift within a release.
- `Template the runbook from the method plus per-binding data` — rejected for now: it needs a template layer this CLI does not have, and a parity test achieves the same guarantee for a fraction of the work. Worth revisiting if a third and fourth runbook appear.

## Decision Outcome

Chosen option: `a runbooks root`. `runbooks/release.md` and `runbooks/setup.md` render the operate and setup chapters as commands, served by `rk guide` with the project path, forge, and technology filled in from detection and nothing else — `<release pr>` stays a placeholder, because a stale number merges someone else's work where a visible placeholder fails loudly. A runbook carries commands, their order, and the checks; every why stays in the chapter it links to, and a runbook never introduces a step the method does not have. Enforced by `distribution:a-runbook-renders-the-spine`.

## Consequences

- Good: one place to go for the recipe, in any repository on the host, always matching the installed binary.
- Good: the parity test stops the fourth prose zone from becoming a fourth source of truth.
- Bad: forge and technology branching costs a small variant grammar in `rk guide`.

## Status

Accepted.
