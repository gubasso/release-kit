# Move the release gate from the ruleset to the workflow

## Context and Problem Statement

One trunk rule did two jobs. The required-status-checks rule stopped an unreviewed push, and it also held the release request until continuous integration concluded. Nothing named the second job. Local integration drops that rule, because a ruleset carrying a required check rejects the direct push outright, so the release request became mergeable the moment it opened. One release published while its own checks were still running, and the next could not arm at all.

## Considered Options

- `A gate in the release workflow, woken by a workflow completing` — chosen.
- `Judge the waking workflow's own conclusion` — rejected: a job may report on a request without voting on it, so a workflow conclusion is false where such a job fails.
- `Wake on a check run completing, which needs one answer` — rejected on evidence: a check run a GitHub Actions job creates starts no workflow, because events raised under the default token are suppressed.
- `Wait inside the release job with a watch command` — rejected: it holds a runner for the length of the pipeline.
- `Restore a required check on the trunk` — rejected on evidence: such a rule rejects the direct push local integration exists to make.

## Decision Outcome

Chosen option: `a gate in the release workflow` — under local integration on GitHub the rendered release workflow wakes when the named workflow completes, proves the request's identity from the forge rather than from the event, judges the named check for the exact head commit, and merges under the release App. Two answers are committed: one names what wakes the gate, the other what the gate believes. Forge integration is unchanged. Enforced by `git:the-release-request-integrates-at-the-forge` and `target-config:a-setup-fact-is-committed-once`.

## Consequences

- Good: the named check holds the release request under both authorities, and the gate reconciles a request that was already green.
- Bad: the release workflow carries reconciliation the ruleset carried for free, and the release App needs one more read permission.

## Status

Accepted.
