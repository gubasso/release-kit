# Move the procedure's how into the runbooks

## Context and Problem Statement

The richest form of the release procedure lived in `_docs/guides/release/` — this repository's own setup and release guides, validated end to end twice. But `_docs/` ships in no crate, so the binary served a thinner procedure than the repository held, and the same steps were restated in the method chapters, the runbooks, the guides, and the skills, each copy free to drift. The operator authors the procedure and needs one place to own it — a place the product also ships.

## Considered Options

- `Promote the guides' content into the shipped runbooks and reduce the guides to an overlay` — chosen.
- `Elect _docs/guides/release/ as the canon where it sits` — rejected: it is one `(rust, github)` instance, it ships nowhere, and no test derives anything from it.
- `Generate the runbooks from the guides` — rejected: the zones differ by axis coverage, not by rendering, so a generator would need the judgment the rewrite applies once.

## Decision Outcome

Chosen option: the chapter and its runbook state each procedure exactly once, as a pair — the chapter owns each step's why, the runbook owns its how — sharing the numbered spine a parity test holds. Substeps elaborate a step and never add one. Axis depth moves to its owner: web-UI walkthroughs to the forge document, registry forms to the binding. The forge scripts stay the hand form of the automated steps, printed by `rk setup script`. Skills route by served name and step number and restate nothing. The dogfood guides keep only coordinates, deviations, and the proof transcript.

Enforced by `distribution:a-runbook-renders-the-spine` and `distribution:a-skill-routes-and-never-restates`.

## Consequences

- Good: the shipped binary serves the full procedure; every derivative routes to one owner; the parity and substep tests catch a fork.
- Bad: runbooks outgrow the chapter line cap, carved out deliberately; two-axis values bridge through placeholders like `<publish workflow>`.

## Status

Implemented — `runbooks/` carries the how, `rk guide backport` joins the served set, and `rk guide` substitutes `<tech>`.
