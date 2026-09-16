# Keep native freshness under local integration

## Context and Problem Statement

The local-mode release workflow verified the named check on the release request's exact head commit, but it could not prove that the request still carried the trunk tip when the merge occurred. Removing GitHub's required-check rule admitted the administrator's deliberate direct push, but also removed the forge's atomic base-freshness check from the release App's merge.

A live ruleset probe established that GitHub treats the built-in repository-administrator role and an Integration as distinct bypass actor types. With the administrator role as the sole bypass actor, an administrator's direct push succeeded while a workflow token remained subject to the pull-request and required-check rules.

## Considered Options

- `Keep the strict ruleset and bypass only repository administrators` — chosen: it separates the human push authority from the release App at the native merge boundary.
- `Keep freshness in the release workflow` — rejected: checking the request head cannot make the head-to-base relationship atomic with the later merge.
- `Use a second App or remote staging branch` — rejected: either adds another identity or another long-lived coordination surface without improving the native rule.

## Decision Outcome

Chosen option: `keep the strict ruleset and bypass only repository administrators`. GitHub local integration retains the pull-request rule, required checks, and strict freshness. Its policy names the stable `repository-admin` intent, which setup maps to GitHub's built-in repository role. The release App is not a bypass actor. The workflow remains as a reconciler: it authenticates candidates, reads the exact-head check, and attempts the merge; the ruleset makes the final freshness decision.

Enforced by `git:concurrent-pull-and-merge-requests-carry-the-tested-trunk`, `target-config:an-invariant-bearing-key-carries-a-floor`, and `forge-setup:a-merge-carries-the-trunk-it-was-tested-against`.

## Consequences

- Good: stale release requests cannot merge under the release App.
- Good: local integration retains its deliberate administrator push.
- Bad: GitHub's Scorecard sees administrators excluded from the local-mode rule.

## Status

Implemented.
