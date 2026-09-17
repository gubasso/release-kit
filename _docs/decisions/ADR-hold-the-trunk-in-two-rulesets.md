# Hold the trunk in two rulesets

## Context and Problem Statement

Local integration excuses the repository administrator from the trunk's pull-request and required-check rules so the deliberate direct push goes through. On GitHub a bypass actor is recorded on the ruleset and never on a rule, so one ruleset carrying every trunk rule hands that administrator the deletion and the force-push too.

A public probe confirmed the scope. Over a trunk whose deletion and force-push rules sat in a ruleset naming nobody, the administrator's fast-forward push was accepted while the force-push and a covered branch's deletion were refused, and an App token was refused at the merge endpoint while the request was behind its base.

## Considered Options

- `Install two rulesets` — chosen: the rules an actor may be excused from and the rules that hold against everyone become separate objects.
- `Keep one ruleset and accept the wider bypass` — rejected: an invariant that holds only against actors who are not the operator is not one.
- `Bypass with mode pull_request` — rejected: it narrows which requests an actor merges and scopes nothing to particular rules.

## Decision Outcome

Chosen option: `install two rulesets`. GitHub targets install a safety ruleset carrying deletion and force-push protection with an empty bypass, and a trunk ruleset carrying the request rule and the strict required check with the actors the recorded integration mode names. The step writes the safety ruleset first, so no run leaves a window with neither, and it writes that empty bypass itself rather than reading a key. `protection.owned_trunk_rules` still names all four rules, and each ruleset composes its half.

Enforced by `git:concurrent-pull-and-merge-requests-carry-the-tested-trunk`, `forge-setup:a-merge-carries-the-trunk-it-was-tested-against`, and `target-config:an-invariant-bearing-key-carries-a-floor`.

## Consequences

- Good: deletion and force-push protection hold against every actor in both modes.
- Good: the release App still cannot merge a stale release request.
- Bad: every target owns one more ruleset and reruns the protection step.
- Bad: an administrator keeps the authority to merge a stale request by hand.

## Status

Implemented. Supersedes [the earlier decision](./ADR-keep-native-freshness-under-local-integration.md), which kept every rule in one ruleset.
