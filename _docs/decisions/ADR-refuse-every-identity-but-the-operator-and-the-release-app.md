# Refuse every identity but the operator and the release App

## Context and Problem Statement

Any identity with write access could merge a green request into the trunk, create a release tag, and push a release branch. Each starts a release or changes the trunk, which only the operator and the release App are meant to do.

## Considered Options

- `One ruleset per restriction, passed by the administrator and the release App` — chosen.
- `The update rule in the trunk ruleset, with the administrator alone as its bypass` — rejected: the release App could no longer merge the ordered release request, and under forge integration the rule would refuse every merge.
- `The creation rule in the immutable tag ruleset` — rejected: a bypass actor passes every rule of its ruleset, so the release App could then move or delete a published tag.
- `The release lines alone in the release branch ruleset` — rejected: another identity could push a commit onto an open release request, which the release App then merges.
- `Approval counts, signed commits, and CODEOWNERS` — rejected: the operator's merge is the review, and no other identity signs.

## Decision Outcome

Chosen option: `one ruleset per restriction`, because a bypass actor is recorded on a ruleset and never on a rule. The trunk takes a ruleset with `update` alone, the release tags a creation ruleset beside the immutable one, and the release lines and both request prefixes one ruleset with `creation` and `update`. Each names the administrator role and the release App. On a Rust target, the creation ruleset covers every tag, because the generated artifact workflow starts on any tag that carries a version.

Enforced by `release-ref-rules:only-the-release-actors-update-the-trunk`, `release-ref-rules:only-the-release-actors-create-a-release-tag`, `release-ref-rules:only-the-release-actors-write-a-release-branch`, and `release-ref-rules:protections-check-judges-the-release-rulesets`.

## Consequences

- Good: any other automation acts as an identity of its own, and the forge refuses it on every ref that starts a release.
- Good: the existing rulesets keep their rules.
- Bad: a GitHub target carries three more rulesets, and a Rust target refuses every tag creation outside the release actors.

## Status

Accepted.
