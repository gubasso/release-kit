# Guard the message content at three points

## Context and Problem Statement

A trunk commit body leaked a git-ignored planning path: `protect-trunk` sets `squash_merge_commit_message=PR_BODY`, making the request's description the trunk commit's body, and no gate read it — the title check reads the title, the commit-msg hook judged header shape, the observation read the title source. Attribution would have passed as silently. Where is the content held?

## Considered Options

- `keep PR_BODY and gate the content at three points` — chosen.
- `drop the body from the squash message` — rejected: the description is the trunk's context; losing it trades the record's value for the guard's job.
- `a single forge-side gate` — rejected: the workflow runs without a checkout and cannot consult `check-ignore`.
- `a single local hook` — rejected: every local mirror dies to `--no-verify`.

## Decision Outcome

Chosen option: `keep PR_BODY and gate the content at three points` — the landed `rk-message` commit-msg hook judges every message against `blocks/message-guards`; the `pr-title` workflow's second step greps the request body against the same patterns, duplicated because the gate has no checkout, an agreement test holding the copies equal; and the observation faults a squash message source that is not `PR_BODY`. The bot's request is recognized by exactly the title check's bot alternative — release-plz authors that body, generated-with line and co-author included — and each gate exempts what it must: the hook skips the attribution class alone, still running the ignored-path class; the forge step, with no ignore rules to consult, skips whole. Enforced by `landing:the-landed-guards-hold-the-message-content` and `forge-setup:the-setup-asserts-the-squash-body-source`.

## Consequences

- Good: the leak class is refused at the desk, the merge, and the observed settings; GitLab needs none of it, since `%{title}` puts no body on the trunk.
- Bad: the limits are real — a semantic leak in prose stays a review concern, `--no-verify` kills the hook, the forge gate greps only the fixed list, and `check-ignore` can flag a body legitimately naming an ignored directory: rephrase, or the finding stands.

## Status

Implemented — the hook lands in the block; `pr-title.yml` carries the gate.
