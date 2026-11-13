# 08 — Worktrees

The checkout mode, `linked-worktree` or `main-worktree`, says where a topic branch opens. It is a Git workflow parameter: `git.checkout_mode` in the target configuration, recorded in `.release-kit/manifest.json` and reported by `rk status`. Only the landing verbs change it. So every clone and every agent sees the same mode, and a change of mode is a visible, reviewed re-landing. A landing does not ask for it. It defaults to `linked-worktree` and sets where the helper verbs `rk worktree add` and `rk issue start` open a topic branch. Where the target opted into the workflow guards, it also decides whether the location guard lands in the hook block, which `rk status --check` judges.

`linked-worktree` is the default and the first-class form. Every code-changing branch lives in a linked worktree at `../<project>@<flattened branch>`. The main checkout commits nothing, on a branch or detached, just as the forge trunk takes no direct push. `main-worktree` is the supported alternative. The repository's original working tree switches to each short-lived branch, and the work happens there. Worktrees remain available and fully functional beside it, and nothing refuses either form.

## Why worktrees are first-class

Parallel work forces the worktree form in either mode. Two writers, human or coding agent, cannot share these parts of one checkout:

- Its HEAD.
- Its index.
- Its uncommitted files.

So every branch under concurrent work takes its own worktree: one branch, one worktree, one writer. Git's own refusal to check one branch out twice is the lock, and `rk` never overrides it. In either mode, concurrent work takes worktrees. The mode decides only whether the main checkout can also be a writing seat.

## The naming rule

The path of a worktree derives from the project and the branch. The branch name flattens: every `/` becomes `-`. The worktree is the sibling `../<project>@<flattened branch>`. Flattening is not injective: `feat/a-b` and `feat-a/b` derive the same directory. So `rk worktree add` refuses a collision by name and never adds a suffix silently. A worktree that an operator made by hand at another path works. `rk worktree list` reports it off-path and never refuses it. For its branch, `rk worktree add` refuses to make a second seat and names `git worktree move` to the derived path as the move. A branch started from an issue reaches its seat through this same derivation and these same refusals. So nothing in the rule changes when the forge chose the name.

## The layouts the sibling rule admits

The sibling parent is the parent of the main checkout, whatever it is. `rk` derives it from the standing main worktree and stores nothing. So where the operator clones the project chooses the layout, and the layout needs no configuration.

- Flat: the main checkout beside its peer projects, with worktrees between them. This is the default, and most projects want it.
- Workspace: the main checkout inside a directory for this project alone. The directory holds the seats and whatever else the project keeps beside them, such as a plans directory: `release-kit.ws/release-kit` with `release-kit.ws/release-kit@feat-x` beside it. Every worktree keeps the project name in its basename, so editor, tmux, and fuzzy-finder titles stay meaningful. Layouts that name the inner directories after the branch alone lose that. Promotion is a move per seat, at any time. The [runbook](../runbooks/worktree.md) states the commands.
- Detached root: the main checkout untouched, and the worktrees somewhere else entirely. `rk worktree list` reports them off-path and never refuses them, and they are fully functional. `rk worktree add` does not produce those paths, so the operator makes and moves them.
- Unsupported: the bare-repo workspace, a bare repository with peer checkouts under it. `rk` refuses a bare main record. The sibling derivation has no main checkout to compose with, and the convention rests on one main worktree that commits nothing.

No directory suffix is canonical. The name of the workspace is the operator's, and `rk` neither reads a suffix nor refuses one. This repository's own operator writes `<project>.ws`, which is a convention and not a rule.

## The sequence

1. Create the worktree from its source. The source is one of these:
   - An adopted local branch.
   - A remote tip.
   - A base.
   - The trunk.
2. Prepare its environment. A worktree is a fresh checkout.
3. Integrate through the authority and the merge form that the record names. [Integration](./12-integration.md) owns both paths and both forms.
4. Prune after the integration, worktree before branch, on the proof that the mode leaves.
5. Optionally, promote to a workspace. This is a layout move per seat, and it is never required.

`rk worktree add` resolves the source by precedence:

- It adopts an existing local branch into its worktree.
- A lone matching remote tip becomes a local tracking branch. This seats a forge-minted issue branch or the release bot's branch from its real tip, and never recreates it from the trunk silently.
- It creates anything else from `--base` or the refreshed trunk. A release line always takes an explicit base, because a line starts from a tag, never the tip.

Pruning rests on the same proof as branch pruning. Three proofs are admissible:

- A merged request whose recorded head equals the tip of the branch.
- A locally recorded integration whose recorded branch tip equals it.
- A trunk that reaches the current tip of the branch, after the branch moved from where it started.

The third proof needs no record and no forge answer. The branch's own reflog proves the creation: its entry with an all-zero old object ID names the commit where the branch started. A fresh branch still at that commit is not merged, only new. A branch whose creation entry expired gives no third proof. The third proof holds after each of these:

- A local rebase integration.
- A GitLab fast-forward merge.
- A hand `git merge --ff-only`.

Each one keeps the commits of the branch. A squash writes a new commit, and a GitHub rebase merge writes new commits with new SHAs. After either one, the trunk does not reach the tip, and the merged request or the ledger entry stays the proof.

The prune observes every proof again at the moment of action. Any of these keeps the worktree when it moved after verification:

- A tip.
- A lock.
- Dirt.

Because the worktree is removed before its branch, a failed removal leaves the branch and its work untouched. A forge that cannot answer keeps every candidate whose only proof is its answer. A local integration record or the reachable-tip proof still holds without it. The step-by-step form is [the worktree runbook](../runbooks/worktree.md), `rk guide worktree`.

## Changing the mode

The mode change is an upgrade with exactly one overridden parameter. `rk upgrade --checkout-mode <mode> --apply` rewrites the record, and the hook block under the workflow guards, from what the record already states. The committed diff is the visible change, and it reaches every clone through the trunk like any change. A plain `rk upgrade` keeps the recorded mode across releases.

The blocks are branch-versioned files. A bare branch opened before a change to `linked-worktree` mode does not carry the guard until it takes the trunk's tip. The change protects the future, not the past. Close the gap in this order:

- First, land the mode change on the trunk through its pull request.
- Then move the main checkout to the trunk. Pull. From here on, the guard holds the main checkout itself.
- Last, adopt each open bare branch into its worktree with `rk worktree add <branch> --apply`. The main checkout is off the branch, so the adoption is clean.

A branch that must keep committing before it merges is guarded either way, because hooks install per clone, not per branch. It rebases onto the trunk only where its own tree must show an agent the new blocks. A switch to `main-worktree` while worktrees exist needs no procedure. The verbs are mode-free, and every worktree keeps working.

## The escape and its cost

`--no-verify` defeats every desk-level mirror, and the forge cannot see local topology. The guard of the linked-worktree mode is honest about both. Its one named escape is the sweep. A CI checkout is commonly detached on the main worktree. So the sweep of a linked-worktree target with its hook manager sets `SKIP=no-commit-to-branch,rk-worktree-location` in its environment. The same escape serves deliberate surgery on the main checkout. This chapter states it once, here.

## What a worktree does not isolate

A worktree isolates only the three parts of a checkout that the section on why worktrees are first-class lists. It isolates nothing else on the machine. These stay shared:

- Ports.
- Services.
- Containers.
- Build caches.

Hooks are per clone. The block is committed, and the install command of the project's hook manager arms it once in the clone. The armed hooks then fire in every linked worktree through the common git dir.

## Enforcement distances

The forge protections are the enforcement. They are identical in both checkout modes and blind to local topology. Where the target opted into the workflow guards, the checkout mode picks which desk-level mirrors stand. Under the guards, the trunk guard refuses a trunk commit in either checkout mode and either integration mode. In `linked-worktree` mode, the location guard also refuses every other commit in the main checkout. In `main-worktree` mode, a branch commit in the main checkout stays open. The location guard is integration-blind. It exits zero on the trunk in either integration mode, and it still refuses a branch commit in the main checkout. A local integration passes both guards, because `rk integrate` moves the trunk ref and fires no commit hook. [Integration](./12-integration.md) answers which guards render at all. The two-distances doctrine of [setup](./02-setup.md) holds unchanged.

## Harnesses

Point a coding-agent harness that creates its own worktrees at `rk worktree add`. The harness then gets these:

- Its branches carry the grammar.
- Its paths follow the naming rule.
- Its cleanup takes the forge-confirmed path.

## Where this connects

- The branch forms are [the model](./00-model.md).
- The mode choice happens in [setup](./02-setup.md).
- The integration that a seat feeds is [integration](./12-integration.md).
- The release path from there is [operate](./03-operate.md).
- The worktree of a release line follows [branch for release](./07-branch-for-release.md).
