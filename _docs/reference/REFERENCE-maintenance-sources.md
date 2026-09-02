# Maintenance sources

The upstream documentation behind `_docs/specs/SPEC-maintenance.md`, `rk branches prune`, the branch-reminder setup step, and the maintenance rows in the forge documents. Each entry records what was verified and when.

## GitHub, on the pull requests a commit belongs to

Verified 2026-09-02 against `https://docs.github.com/en/rest/commits/commits` (API version 2022-11-28). `GET /repos/{owner}/{repo}/commits/{commit_sha}/pulls` "lists the merged pull request that introduced the commit to the repository", and for a commit not on the default branch returns the merged and open pull requests associated with it; each item carries `merged_at` (a timestamp or null) and `head.sha`. Bearing: the confirmation predicate — merged means `merged_at` non-null, and only `head.sha` equal to the local tip proves the tip the forge reviewed is the tip the clone still holds.

## GitLab, on the merge requests a commit belongs to

Verified 2026-09-02 against `https://docs.gitlab.com/ee/api/commits.html`. `GET /projects/:id/repository/commits/:sha/merge_requests` returns the merge requests associated with a commit; each carries `state` (`merged` among its values) and `sha`, "the SHA of the merge request" — the source-branch head at last push. Bearing: the GitLab arm of the same predicate, and the fail-safe it implies — a branch amended after its merge no longer matches `sha` and stays.

## git, on the hooks and the refs

Verified 2026-09-02 against `https://git-scm.com/docs/githooks`, `https://git-scm.com/docs/git-for-each-ref`, `https://git-scm.com/docs/git-update-ref`, and `https://git-scm.com/docs/git-rev-parse`, with the ref behavior probed live on git 2.51. `post-merge` is invoked by `git merge`, which includes a fast-forwarding `git pull`, and does not run when the merge fails on conflicts; no hook fires when a forge deletes a remote branch, which is why the reminder rides the pull. `%(upstream:track)` renders the literal `[gone]` in a `for-each-ref` format string when the configured upstream ref no longer exists — plumbing output, unlike the localized `git branch -vv` porcelain — and `%(worktreepath)` is non-empty exactly for a branch checked out in some worktree. `git update-ref -d <ref> <old-oid>` deletes the ref only after verifying it still holds `<old-oid>`, which is the compare-and-delete the apply path uses so a branch that moved after verification is refused rather than lost. `git rev-parse --git-path hooks` resolves the hooks directory through gitfiles, linked worktrees, and `core.hooksPath`, relative to the directory it runs in.
