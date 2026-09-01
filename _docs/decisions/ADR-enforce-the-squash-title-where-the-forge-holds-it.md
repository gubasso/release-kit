# Enforce the squash title where the forge holds it

## Context and Problem Statement

The model states that Conventional Commits are enforced on the squash title; nothing enforced it. GitHub's `squash_merge_commit_title` has no documented default, and unset, a one-commit pull request offers that commit's own subject as the squash title, so a branch commit named `wip` lands on the trunk as `wip`, the bot computes no bump, and everything release-kit checks stays green. No check anywhere validated a request's title.

## Considered Options

- `The repository setting plus a required title check` — chosen.
- `The title check alone` — rejected: the check gates the offered title, but with the title source unset the merged message can still be the branch commit's subject, which no check ever saw.
- `The repository setting alone` — rejected: the setting makes the title the message, but an unconventional title then lands verbatim; the two halves close each other's hole.
- `A third-party title action` — rejected: a fifteen-line regex job is identical on both forges and adds no supply-chain surface; GitLab has no equivalent action anyway.

## Decision Outcome

Chosen option: `the repository setting plus a required title check`. `protect-trunk` asserts `squash_merge_commit_title=PR_TITLE` on GitHub and `squash_commit_template=%{title}` on GitLab, so the request's title is the trunk's message; the shared snippet zone lands `pr-title.yml` and `mr-title.yml`, holding that title to a scoped Conventional Commit rendered from the landing's `scopes` parameter, with the bot's own release-request titles as a fixed alternative. The trunk ruleset requires the title job beside the project's named check; GitLab's whole-pipeline requirement makes its job blocking without registration.

Enforced by `forge-setup:the-setup-asserts-the-squash-title-source` and observed by `rk setup check`, which faults any other value.

## Consequences

- Good: the model's enforcement claim is literally true on both forges, at the merge and at the setting the merge reads.
- Bad: a project whose title vocabulary changes edits its scope list through `rk upgrade --scopes`, not ad hoc.

## Status

Implemented — `setup/<forge>/protect-trunk`, `src/setup/observe.rs`, and `snippets/_shared/`.
