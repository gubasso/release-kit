# Release only on the operator's order

## Context and Problem Statement

Under the trunk style the landed automation armed the release request as it opened, or a gate job merged it once its check passed, so the automation made the one release decision by itself. A `build` commit cut a release, and a later push regenerated a request and dropped a hand correction to its changelog.

## Considered Options

- `No job arms or merges; the operator orders each trunk release` — chosen.
- `Keep the arm and hold a release with a label the job reads` — rejected: the default still releases, and a hold is one more step on every change.

## Decision Outcome

Chosen option: `no job arms or merges`.

1. A release happens only on the operator's order, in every combination of driver, forge, integration mode, and style. The release request stays open and collects changes.
2. A changelog correction is the last step before the merge, because the bot regenerates the request on every trunk push.
3. A landed project migrates through `rk upgrade` and the setup steps: the gate job and its grant go, the `merge-on-order` step disarms each open release request, and `rk setup check` and `rk release status` prove it.
4. The operator orders a trunk release through `rk release merge`, which dispatches a merge job that runs as the release App, so the strict check of the trunk ruleset holds at the write. A release line keeps the merge by a person.
5. `rk status` stays offline. `rk release status` judges a live armed request and exits non-zero on it.
6. `setup.required_workflow` retires and refuses by name, and an old record that carries it still loads.

Enforced by `git:the-release-request-integrates-at-the-forge`, `release-engine:the-request-never-arms`, `forge-setup:an-armed-release-request-faults`, `release-state:an-armed-request-fails-the-report`, `target-config:a-retired-key-refuses-by-name`, and `landing:the-changelog-quality-gate-is-the-squash-message`.

## Consequences

- Good: a change lands without a release, and the operator decides when the open request ships.
- Good: a hand correction to the changelog survives to the merge.
- Bad: every trunk release costs the operator one command.

## Status

Accepted. Supersedes [standing auto-merge](./ADR-arm-the-release-request-with-standing-auto-merge.md), [the workflow release gate](./ADR-move-the-release-gate-from-the-ruleset-to-the-workflow.md), and [withdrawing a release by disarming](./ADR-withdraw-a-release-by-disarming-the-request.md).
