# Maintenance sources

The upstream documentation behind `_docs/specs/SPEC-maintenance.md`, `rk branches prune`, the branch-reminder setup step, and the maintenance rows in the forge documents. Each entry records what was verified and when.

## GitHub, on the pull requests a commit belongs to

Verified 2026-09-02 against `https://docs.github.com/en/rest/commits/commits` (API version 2022-11-28). `GET /repos/{owner}/{repo}/commits/{commit_sha}/pulls` "lists the merged pull request that introduced the commit to the repository", and for a commit not on the default branch returns the merged and open pull requests associated with it; each item carries `merged_at` (a timestamp or null) and `head.sha`. Bearing: the confirmation predicate — merged means `merged_at` non-null, and only `head.sha` equal to the local tip proves the tip the forge reviewed is the tip the clone still holds.

## GitLab, on the merge requests a commit belongs to

Verified 2026-09-02 against `https://docs.gitlab.com/ee/api/commits.html`. `GET /projects/:id/repository/commits/:sha/merge_requests` returns the merge requests associated with a commit; each carries `state` (`merged` among its values) and `sha`, "the SHA of the merge request" — the source-branch head at last push. Bearing: the GitLab arm of the same predicate, and the fail-safe it implies — a branch amended after its merge no longer matches `sha` and stays.

## git, on the hooks and the refs

Verified 2026-09-02 against `https://git-scm.com/docs/githooks`, `https://git-scm.com/docs/git-for-each-ref`, `https://git-scm.com/docs/git-update-ref`, and `https://git-scm.com/docs/git-rev-parse`, with the ref behavior probed live on git 2.51. `post-merge` is invoked by `git merge`, which includes a fast-forwarding `git pull`, and does not run when the merge fails on conflicts; no hook fires when a forge deletes a remote branch, which is why the reminder rides the pull. `%(upstream:track)` renders the literal `[gone]` in a `for-each-ref` format string when the configured upstream ref no longer exists — plumbing output, unlike the localized `git branch -vv` porcelain — and `%(worktreepath)` is non-empty exactly for a branch checked out in some worktree. `git update-ref -d <ref> <old-oid>` deletes the ref only after verifying it still holds `<old-oid>`, which is the compare-and-delete the apply path uses so a branch that moved after verification is refused rather than lost. `git rev-parse --git-path hooks` resolves the hooks directory through gitfiles, linked worktrees, and `core.hooksPath`, relative to the directory it runs in.

## git, on the worktrees

Verified 2026-09-02 against `https://git-scm.com/docs/git-worktree`. `git worktree list` "lists the details of each worktree; the main worktree is listed first, followed by each of the linked worktrees", and `--porcelain -z` terminates each attribute with NUL, with the attributes `worktree`, `HEAD`, `branch`, `bare`, `detached`, `locked [reason]`, and `prunable [reason]`. `git worktree add` refuses a branch that is already checked out in another worktree unless forced, which is the checkout-exclusivity the convention uses as its concurrency lock. `git worktree remove` refuses an unclean or locked worktree unless forced. `git worktree prune` prunes worktree information whose directory is gone, gated by `gc.worktreePruneExpire` unless `--expire <time>` overrides it — the reason the apply passes `--expire now` — and a locked worktree's administrative files are not removed. `git worktree lock` records a reason, and `git worktree repair` re-links records after a move. `git worktree move` refuses a main working tree (`is a main working tree`), so a main checkout moves as a plain filesystem move, after which `repair` run inside the moved main working tree re-establishes each linked worktree's broken `.git` file — probed live on git 2.55. Bearing: the container-promotion step in the worktree runbook rests on this refusal and repair direction; also the porcelain parser's ordering assumption and attribute vocabulary, the stale sweep's `--expire now` form, and the keep-a-lock-unconditionally guard.

## git, on ref-name validity

Verified 2026-09-02 against `https://git-scm.com/docs/git-check-ref-format`. `git check-ref-format --branch <name>` checks a branch name's validity, refusing among others consecutive dots, a component ending in `.lock`, and names that are option-shaped without the documented escapes. Bearing: the convention grammar is necessary, not sufficient — it admits names git refuses — so `rk worktree add` runs this check after its own matcher, and a name that would fail at `git worktree add` fails at the preview with git's own reason.

## git, on fetch and the refspec

Verified 2026-09-02 against `https://git-scm.com/docs/git-fetch`. A plain `git fetch origin` updates the remote-tracking refs through the remote's configured refspec, while `git fetch origin <branch>` fetches the named ref into `FETCH_HEAD` and fails outright when the remote has no such ref — the ordinary case for a new branch — and a source-only refspec is no promise that a remote-tracking ref moved. Bearing: the apply's refresh is one plain `git fetch origin`, best-effort, followed by resolving `refs/remotes/origin/<branch>` to an exact object name.

## pre-commit, on hooks in linked worktrees

Verified 2026-09-02 against `https://github.com/pre-commit/pre-commit/issues/808` and the pre-commit changelog for 1.10.5. Installing from inside a linked worktree once wrote the hook where git never looks; the defect was fixed in pre-commit 1.10.5 (2018), and since then one `pre-commit install` in a clone arms the hooks for the main checkout and every linked worktree alike, because the hooks live in the common git dir. Bearing: no install-location rule is needed or stated, and the canon's per-clone-once claim rests on this.
