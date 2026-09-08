# Refuse the merge queue and name its consequence

## Context and Problem Statement

GitHub's merge queue is a ruleset rule of type `merge_queue`. The setup already faulted it, because it faults every rule it does not own, but the text said only `an unowned rule is present: merge_queue`. That names the rule and nothing an operator can act on. The consequence is sharp: a queued repository holds every request until a check triggering on `merge_group` reports, the payload lands no workflow carrying that trigger, and the queue drops the request when its CI timeout expires.

## Considered Options

- Keep the generic fault and give `merge_queue` its own text — chosen.
- Own the queue: land a workflow triggering on `merge_group` — rejected: the queue re-tests a speculative merge, and this convention's gate is the request's own pipeline.
- Add `merge_queue` to the owned rule set — rejected: that set also drives the missing-rule fault, so every target without a queue would report one missing.
- Add a step of its own — rejected: a broken trunk protection is not a separate concern, and a new step would owe GitLab an answer it does not have.

## Decision Outcome

One arm in the ruleset fault loop, keyed on `merge_queue`, whose text states three things: a queue is enabled on the trunk, this convention lands no workflow triggering on `merge_group`, and `rk setup step protect-trunk --apply` rewrites the ruleset without it. Every other unowned kind keeps the generic text. `forges/github.md` states the position beside the gate, so an operator reads the refusal before the check ever runs.

Enforced by `forge-setup:an-unowned-protection-names-its-consequence`.

## Consequences

- Good: an operator who enabled a queue reads why it is refused and how to undo it, from the check alone.
- Good: the refusal costs one arm, and a target with no queue is unaffected.
- Bad: a project that wants a queue must leave this convention rather than set a flag. Reopening that is a method change.

## Status

Implemented: the `merge_queue` arm in `src/setup/observe.rs`, and the merge-queue bullet in `forges/github.md`.
