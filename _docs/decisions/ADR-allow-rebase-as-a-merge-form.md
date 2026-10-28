# Allow rebase as a merge form

## Context and Problem Statement

The method took squash as the only merge form: every branch reached the trunk as one commit. Some projects keep the commits of a branch instead. They rebase the branch on the trunk and fast-forward the trunk to it, so the history stays linear and keeps each step. `rk integrate`, the forge settings, and the prune verbs admitted no fast-forward.

## Considered Options

- `a recorded merge form, squash by default` — chosen.
- `squash only` — rejected: a project that keeps its branch commits leaves the convention or writes its own integration.
- `rebase only for a branch with one commit` — rejected: it equals a squash with the branch message, and it gives a branch of several commits no path.

## Decision Outcome

Chosen option: `a recorded merge form, squash by default`. `git.merge_form` takes `squash` or `rebase`, under local and forge integration. Squash gives the trunk one commit for each branch. Rebase keeps every commit of the branch. Both keep the trunk linear, and neither writes a merge commit.

Under rebase every commit reaches the trunk with its own message, so `rk integrate` and a forge commit gate judge each one with the commit-msg judgment, the gate in place of the title gate. The forge settings, the floors, and the observers derive the expected merge settings from the resolved form. A record without the parameter reads as `squash`, so a landed project keeps its behavior. A branch that the trunk reaches is merged, so the prune verbs take that reachability as a third proof.

Enforced by `git:the-merge-form-is-a-landing-parameter`, `git:a-rebase-integration-publishes-the-branch-tip`, `project-profile:the-forge-gate-follows-the-merge-form`, and `maintenance:one-merge-proof-authorizes-both-removals`.

## Consequences

- Good: a project keeps its branch commits under every gate.
- Good: a branch fast-forwarded by hand or by GitLab is pruned with no ledger and no request.
- Bad: under a release, each commit of a rebased branch counts on its own in the changelog.
- Bad: a GitHub rebase merge writes new commits, so there the merged request stays the only prune proof.

## Status

Accepted
