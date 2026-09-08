# Let the forge name an issue's branch

## Context and Problem Statement

The convention already preferred the forge-minted `<issue-id>-<slug>`, and nothing carried that preference into a real branch. An operator says "address issue 57" and an agent invents `fix/something`. No hook catches it, because no hook sees the request. The forges are not symmetric either: GitHub names the branch on the server, and GitLab renders its name in the web UI from a project setting no API endpoint resolves.

## Considered Options

- `Mint at the forge on GitHub, and reproduce GitLab's rendering` — chosen.
- `Render the name in the client on both forges` — rejected: it is what `glab mr create --create-source-branch` does, and it applies no template, transliterates nothing, squeezes nothing, and truncates nothing. Two clients then produce two branches for one issue.
- `glab mr create --related-issue --create-source-branch` — rejected: it also opens a merge request before the first commit exists.
- `Let the operator pass a name` — rejected: an escape is the hole this closes.

## Decision Outcome

`rk issue start` never invents a name. On GitHub the mint passes no name, because the mutation's name input is optional and defaults to the issue number and title, and the branch is read back from the issue's linked branches. On GitLab the project's `issue_branch_template` is read and rendered as `Issue#to_branch_name` renders it, confidential case and hundred-character cut included.

Enforced by `issue-branch:the-forge-names-the-branch` and `issue-branch:a-customized-template-is-read-not-assumed`.

## Consequences

- Good: one issue has one branch, whoever starts the work, and a rerun adopts instead of duplicating.
- Good: the branch, its request, and the issue's closing hang together with no keyword in the body.
- Bad: the two forges reach one outcome by different mechanisms, so the GitLab arm carries a reproduction upstream can move under it.
- Bad: that reproduction's transliteration rests on a table, so an unusual script can render differently. The verb reports the case.

## Status

Implemented — `src/issue.rs` and `src/commands/issue.rs`. The GitHub read path is proven live; the mint and the GitLab rendering are not, and `_docs/reference/REFERENCE-issue-branch-sources.md` records which is which.
