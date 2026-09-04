# 08 — Worktrees

A project chooses its working-copy form once, at setup, and records the choice in the landing: the workflow mode, `worktree` or `branches`. The mode is a landing parameter — recorded in `.release-kit/manifest.json`, rendered into the committed hook block and routing block, reported by `rk status`, judged by `rk status --check`, and changed only through the landing verbs — so every clone and every agent sees the same mode, and changing it is a visible, reviewed re-landing, never an ad hoc local toggle.

`worktree` is the default and the first-class form: every code-changing branch lives in a linked worktree at `../<project>@<flattened branch>`, and the main checkout commits nothing — not on a branch, not detached — just as the forge trunk takes no direct push. `branches` is the supported alternative: short-lived branches are worked in the main checkout, worktrees remain available and fully functional beside them, and nothing refuses either form.

## Why worktrees are first-class

Parallel work forces the worktree form regardless of mode. Two writers — human or coding agent — cannot share one checkout's HEAD, index, and uncommitted files, so every concurrently-worked branch takes its own worktree: one branch, one worktree, one writer. Git's own refusal to check one branch out twice is the lock, and `rk` never overrides it. In either mode, concurrent work takes worktrees; the mode decides only whether the main checkout may also be a writing seat.

## The naming rule

A worktree's path derives from the project and the branch: the branch name flattens by replacing every `/` with `-`, and the worktree is the sibling `../<project>@<flattened branch>`. Flattening is not injective — `feat/a-b` and `feat-a/b` derive the same directory — so `rk worktree add` refuses a collision by name and never suffixes silently. A worktree made by hand at some other path works and is reported off-path by `rk worktree list`, never refused; `rk worktree add` for its branch refuses to make a second seat and names `git worktree move` to the derived path as the move.

## The layouts the sibling rule admits

The sibling parent is the main checkout's parent, whatever it is: `rk` derives it from the standing main worktree and stores nothing, so the layout is chosen by where the project is cloned and needs no configuration.

- Flat — the main checkout beside its peer projects, worktrees interleaving with them. The default, and what most projects want.
- Workspace — the main checkout inside a directory scoped to this project alone, holding its seats and whatever else the project keeps beside them, a plans directory for example: `release-kit.ws/release-kit` with `release-kit.ws/release-kit@feat-x` beside it. Every worktree keeps the project name in its basename, so editor, tmux, and fuzzy-finder titles stay meaningful — the failure mode of layouts that name the inner directories after the branch alone. Promotion is a move per seat, at any time; the [runbook](../runbooks/worktree.md) states the commands.
- Detached root — the main checkout untouched and the worktrees somewhere else entirely. Reported off-path by `rk worktree list`, never refused, fully functional; `rk worktree add` will not produce those paths, so the operator makes and moves them.
- Unsupported: the bare-repo workspace — a bare repository with peer checkouts under it. `rk` refuses a bare main record: the sibling derivation has no main checkout to compose with, and the convention rests on one main worktree that commits nothing.

No directory suffix is canonical: the workspace's name is the operator's, and `rk` neither reads a suffix nor refuses one. This repository's own operator writes `<project>.ws`, which is a convention and not a rule.

## The sequence

1. Create the worktree from its source — an adopted local branch, a remote tip, a base, or the trunk.
2. Prepare its environment — a worktree is a fresh checkout.
3. Land through the one path: commit, push, pull request, squash merge.
4. Prune after the merge: forge-confirmed, worktree before branch.
5. Promote to a workspace — optional: a layout move per seat, never required.

`rk worktree add` resolves the source by precedence: an existing local branch is adopted into its worktree, a lone matching remote tip becomes a local tracking branch — which is how a forge-minted issue branch or the release bot's branch is seated from its real tip, never silently recreated from the trunk — and anything else is created from `--base` or the refreshed trunk, with a release line always taking an explicit base because a line is cut from a tag, never the tip.

Pruning rests on the same proof as branch pruning: a merged request whose recorded head equals the branch's tip, re-observed at the moment of action. A tip, a lock, or dirt that moved after verification keeps the worktree; the worktree is removed before its branch, so a failed removal leaves the branch and its work untouched; and a forge that cannot answer keeps everything. The step-by-step form is [the worktree runbook](../runbooks/worktree.md), `rk guide worktree`.

## Changing the mode

The mode change is an upgrade with exactly one overridden parameter: `rk upgrade --workflow <mode> --apply` rewrites the two blocks and the record from what the record already states, and the committed diff is the visible change, reaching every clone through the trunk like any change. A plain `rk upgrade` keeps the recorded mode across payload versions.

The blocks are branch-versioned files, so a bare branch opened before a change to `worktree` mode does not carry the guard until it takes the trunk's tip; the change protects the future, not the past. The transition closes the gap in order: land the mode change on the trunk through its pull request; move the main checkout to `master` and pull, so the main checkout itself is guarded from here on; adopt each open bare branch into its worktree with `rk worktree add <branch> --apply` — the main checkout is off it, so the adoption is clean. A branch that must keep committing before it merges is guarded either way, because hooks are installed per clone, not per branch; it rebases onto the trunk only where its own tree must show an agent the new blocks. Switching to `branches` while worktrees exist needs no procedure: the verbs are mode-free and every worktree keeps working.

## The escape and its cost

Every desk-level mirror dies to `--no-verify`, and the forge cannot see local topology: the worktree mode's guard is honest about both. Its one named escape is the sweep: a CI checkout is commonly detached on the main worktree, so a worktree-mode target's `pre-commit run` sweep sets `SKIP=no-commit-to-branch,rk-worktree-location` in its environment. The same escape serves deliberate main-checkout surgery, stated here once.

## What a worktree does not isolate

A worktree isolates the working tree — HEAD, index, uncommitted files — and nothing else on the machine: ports, services, containers, and shared build caches are still shared. Hooks are per clone: the block is committed, `pre-commit install` arms it once in the clone, and the armed hooks fire in every linked worktree through the common git dir.

## Enforcement distances

The forge protections are the enforcement, identical in both modes and blind to local topology; the mode picks which desk-level mirrors stand. In `worktree` mode the main checkout mirrors the trunk protection locally — `master` refused by the trunk guard, every other commit by the location guard — and in `branches` mode both forms stay open. The two-distances doctrine of [setup](./02-setup.md) holds unchanged.

## Harnesses

A coding-agent harness that creates worktrees of its own is pointed at `rk worktree add`, so its branches carry the grammar, its paths the naming rule, and its cleanup the forge-confirmed path.

## Where this connects

The branch forms are [the model](./00-model.md); the mode choice is made in [setup](./02-setup.md); the landing path the seats feed is [operate](./03-operate.md); a release line's worktree follows [branch for release](./07-branch-for-release.md).
