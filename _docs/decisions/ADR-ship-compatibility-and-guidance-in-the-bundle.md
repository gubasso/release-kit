# Ship compatibility and guidance in the bundle

## Context and Problem Statement

A release that is authentic and intact may still need a newer engine, a generator at its pin, a manager the target does not wire, a forge above a floor, or a release passed through first. A landed target may need an instruction the changelog does not give. Where does that knowledge live, and how does it reach the plan offline?

## Considered Options

- Declare compatibility in `compatibility.toml` and ship guidance under `guidance/`, both payload roots read through the seam — chosen.
- Discover every requirement at setup time, as `rk setup` does for the GitLab floor — rejected: a plan that finds a floor while applying already promised what it cannot keep.
- Point the agent at the changelog — rejected: a changelog is written for a reader choosing whether to upgrade, and a plan is read by an agent that already decided, so the bundle owes the second text.
- Truncate guidance to a window — rejected: the files are small, and an undeclared window is a silent gap.

## Decision Outcome

Chosen option: both declarations ride in the bundle, read offline through the seam every root is read through. Each compatibility axis becomes a precondition with a requirement, and the facts land in the plan's release section. Guidance is selected by the interval from the record to the candidate, filtered to the destinations the target has, and reported with an explicit coverage. Partial coverage is a decision the operator takes knowingly. Unavailable guidance blocks a rendered write, because the operator cannot reconcile what nobody described. An authoring gate names a release that changed a landed destination and shipped no file, and `guidance.no_steps` records a deliberate silence.

Enforced by `reconcile:compatibility-is-declared-in-the-bundle-and-evaluated-per-axis` and `reconcile:guidance-ships-in-the-bundle-filtered-and-covered`.

## Consequences

- Good: a plan says what a landing needs before the first write, and an agent reads only the steps that concern the target.
- Bad: two more roots move `payload_sha256`, and a landed-destination change owes a file or a recorded silence.

## Status

Implemented: `compatibility.toml`, `guidance/`, `src/release/declared.rs`, `src/plan/compatibility.rs`, `src/plan/guidance.rs`.
