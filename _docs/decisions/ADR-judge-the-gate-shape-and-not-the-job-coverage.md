# Judge the gate shape and not the job coverage

## Context and Problem Statement

The observation tried to prove that the required check covers every job a pull request reports. Three findings undo it. The proof cannot be made: whether a job is meant to block a merge is intent, and no file states it. No exemplar makes it — `astral-sh/ruff`'s gate names 11 of 35 jobs, `astral-sh/uv`'s 6 of 25, both deliberately, and Kubernetes Prow declares its voting set with `always_run` and `optional`. And the output could not reach its reader: it rode as a limitation on a passing step, and this convention's standing arm means nobody reads the check list.

## Considered Options

- Judge the gate's own shape and move coverage into prose — chosen.
- Keep the coverage proof — rejected: it faults a benchmark that reports and does not vote.
- Promote that limitation to a fault — rejected: the same objection, louder.
- Declare the voting set in a file of our own, as Prow does — rejected: a second source of truth.

## Decision Outcome

Five faults on the gate replace the limitation: no job reports the required context, more than one does, the condition is neither `always()` nor a readable `!cancelled()`, `needs` is not a literal list, and the trigger filters the request away. They are faults: a required check that cannot report is a broken trunk protection. `!cancelled()` joins `always()` because `rust-lang/cargo` uses it deliberately. Coverage becomes the convention `forges/github.md` states: `gate.needs` is the voting list. The local `uses: ./...` resolver is withdrawn, not deferred: `needs` matches a job id inside one file, so a resolved context would feed nothing.

Enforced by `forge-setup:the-required-check-is-shaped-to-report`.

## Consequences

- Good: a target running a non-voting job on a pull request passes, the ordinary case.
- Good: a gate that cannot report stops the setup instead of describing itself.
- Bad: the convention accepts the residual risk its standing arm creates, and proves shape, not blocking.

## Status

Implemented: the narrowed reader and its faults under `src/setup/`, and the convention in `forges/github.md`.
