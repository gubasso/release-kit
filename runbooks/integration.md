# Integration runbook

This page gives the four steps of [integration](../method/12-integration.md) as commands. The chapter owns the why of each step, and this page owns the how. The recorded integration mode selects the variants below, and `rk integrate` resolves the same mode the same way. `<project>` is the directory name of the main checkout. `rk guide integration` fills it in where detection resolves it. The commands are the operator's to run. An agent serves a runbook and states the command. It runs one only where the operator's request named that step.

The recorded merge form, `git.merge_form`, decides what reaches the trunk under either mode:

- `squash`, the default, writes one commit for the branch, with the message you give it.
- `rebase` rebases the branch on the trunk tip and fast-forwards the trunk to it. Every commit of the branch arrives with its own message, and no new commit is written.

Choose `squash` where one implementation reads best as one trunk commit. Choose `rebase` where every commit of the branch is already a conventional step worth keeping on the trunk. `rk init --merge-form <form>` records the answer. `rk upgrade --merge-form <form> --apply` changes it. A project that never answered keeps `squash`. [Integration](../method/12-integration.md) owns the choice.

## 1. Ready the branch in its seat

The implementation is committed on its short-lived branch, in the seat that [the worktree runbook](./worktree.md) created. Integration starts from a clean seat, because the gate judges the tree it finds.

```bash
cd ../<project>@<type>-<slug>
git status --porcelain                   # check: empty; a dirty seat refuses at step 3
```

## 2. Run the gate the boundary needs

The project assigns every check to the earliest stage that can run it. The stage before the boundary carries the full suite.

On local:

The boundary is the integration. Its gate is the clone's `pre-merge-commit` hook, which `rk integrate` asks git to run on the staged squash. Before you integrate, make sure that the clone installs that hook. On git 2.54 or newer, list it:

```bash
git hook list pre-merge-commit
# check: git names at least one hook; "no hooks found" means the hook manager's install did not arm the stage
```

On older git, `git hook list` does not exist. That is not a missing hook. Read the install output of the project's hook manager instead. prek prints `` Installed Git hook at `.git/hooks/pre-merge-commit` ``. pre-commit prints `pre-commit installed at .git/hooks/pre-merge-commit`.

The hook manager selects the files from the staged change. It skips a check whose files the branch did not touch. prek runs each nested `.pre-commit-config.yaml` as a project of its own. A project that runs prek lists each fixture directory that holds such a file in a root `.prekignore`.

On forge:

The full branch suite is the `pre-push` stage. git fires it on the push of step 3. The `pre-merge-commit` stage does not fire. The `manual` stage is an operator sweep that holds nothing back.

## 3. Integrate

On local:

```bash
rk integrate <type>/<slug>                               # preview: names the rebase, the gate, and the squash
rk integrate <type>/<slug> -m "<type>(<scope>): <subject>" --apply
# check: the report names the trunk's new commit and records the integration
```

The message becomes the trunk's commit message. So it is a Conventional Commit, with a scope wherever the record names a release. Under `release.mode = "none"`, the scope is preferred and not required, because no changelog reads it. `git commit-tree` fires no hook. So `rk integrate` holds the message to the same convention as the landed `commit-msg` stage. Every refusal leaves the trunk at the tip it started from. The rebase form takes no message. Section 3b covers it.

On forge:

```bash
rk integrate <type>/<slug> --forge --apply               # runs the pre-push stage through the push
# check: the report names the request command for this forge
```

You open the request yourself, with the command that the report names. Under `squash`, its title becomes the trunk's commit message, so it follows the same convention as a local message. Judge the body with `rk message --check --kind body` before you post it. The forge holds the merge behind its required check.

A local publication reads the trunk again right before it moves anything, and the worktree that holds it. A difference refuses. Where no worktree holds the trunk, the move is one compare-and-swap of the trunk ref. Where one does, the move is that worktree's fast-forward. `rk integrate` then confirms the move from the trunk's reflog, or from the full object names in the fast-forward's report. Git offers no compare-and-swap for a checked-out branch that also moves its worktree. So a writer outside `rk` can still move the trunk in between, and the trunk is then already published.

Where the move started elsewhere, or neither source can confirm its start, `rk integrate` exits 1 with the reason `state-drift`. Its report names the published commit. Its diagnostic names the commit where the move started, or why the start is unconfirmed. Nothing is rolled back. In that case:

1. Run the `git log` that the diagnostic names.
2. Read the commits in the range that it names.
3. Decide whether they belong on the trunk.

Where the move started elsewhere, the range runs from the observed tip to that start: the other writer's commits. Where the start is unconfirmed, the diagnostic also names `git reflog` of the trunk. Read it to find what else moved the trunk.

### 3a. A refusal and its recovery

| The refusal names            | What to do                                                         |
| ---------------------------- | ------------------------------------------------------------------ |
| a dirty seat                 | commit or stash in the seat, then re-run                           |
| a branch off the grammar     | rename the branch to `<type>/<slug>` or the forge-minted form      |
| a diverged trunk             | fast-forward the trunk, or push it, then re-run                    |
| a rebase conflict            | resolve it in the seat, commit, then re-run                        |
| the pre-merge-commit hook    | fix what the hook reported, commit, then re-run                    |
| no pre-merge-commit hook     | install it with the project's hook manager, then re-run            |
| a hook that committed        | keep its commit, the repair, `git reset -q --hard`, a clean status |
| a hook that staged a change  | the repair, then `git reset -q`, then settle the changed files     |
| a seat detached at the trunk | the repair, then re-run                                            |
| a trunk that moved           | re-run; the second attempt rebases onto the tip that arrived       |
| a trunk checked out since    | re-run, or remove the new worktree of the trunk first              |
| another run holding the lock | wait for it; two integrations in one clone write one trunk ref     |
| `-m` under the rebase form   | drop `-m`; every commit of the branch keeps its own message        |
| a branch commit's message    | reword that commit in the seat, then re-run                        |

The repair is `git -C <seat> symbolic-ref HEAD refs/heads/<branch>`. It leaves a commit that a hook made on no ref, so keep that commit first with `git branch <name> HEAD`. Re-run only when `git status --porcelain` prints nothing. [Recovery](../method/04-recovery.md) gives each case in full.

### 3b. The rebase form

Under `git.merge_form = "rebase"`, the local integration takes no message:

```bash
rk integrate <type>/<slug>                               # preview: names the rebase, the gate, and each commit it judges
rk integrate <type>/<slug> --apply
# check: the report names the branch tip as the trunk's new commit; no new commit was written, and none is a merge commit
# exit 73 with the reason merge-form-mismatch: a -m was given; drop it
```

Every commit between the trunk and the branch tip becomes a trunk commit. So `rk integrate` judges each message by the landed `commit-msg` judgment before it moves anything. It names the first commit it refuses. Its rebase flattens every merge and moves no other branch, whatever the clone's Git configuration asks. A range that still holds a merge commit refuses, with the trunk unmoved. The trunk then carries the same commits as the branch, in order. At the forge, the request command asks for no squash. A commit gate judges every commit of the request on each push, in place of the title gate.

## 4. Push the trunk and prune

On local:

The trunk push is separate and deliberate, and it takes the fast-forward form alone. On GitHub, authenticate as a repository administrator. The local-mode trunk ruleset lets only that role bypass its request rule. The push stays a fast-forward, because the second ruleset refuses a rewrite from that administrator too.

Push when you decide to. Several integrations can ride one push. The release bot reads the intent of each commit separately. It counts only the commits that [its binding](../bindings/README.md) attributes to the package. `rk integrate` warns where it can prove that a commit will not count.

```bash
git -C ../<project> push origin master   # check: a fast-forward; never --force
rk worktree prune --verify               # check: confirmed, locally integrated as <commit>
rk worktree prune --apply                # check: pruned; the seat and the branch went together
```

Where another writer won the push:

1. Fetch.
2. Replay the implementation onto the new tip.
3. Run the gates again.
4. Push again.

Nothing here force-pushes.

On forge:

```bash
git fetch --prune origin
rk worktree prune --verify               # check: confirmed against the merged request
rk worktree prune --apply                # check: pruned; the seat and the branch went together
```

After a local rebase integration, a GitLab fast-forward merge, or a hand `git merge --ff-only`, the trunk reaches the branch tip itself. So the verify can name the reachable tip as the proof. A GitHub rebase merge writes new commits, so there the merged request stays the proof. Step 4 of [the worktree runbook](./worktree.md) lists the three proofs.

### 4a. When the push turns the trunk red

The local path cannot see the remote checks before the push, because the push starts them. Watch the run that the push started, and the release request that it refreshed. A failure leaves the release request open, so nothing publishes. The trunk's strict rule is the boundary, and no landed job merges the request. Repair the failure as a fresh implementation, integrated the same way. No local integration is erased, and nothing is reverted by hand.

## 5. Release

The mode changes nothing here. The release request integrates at the forge in both modes, and merging it tags and publishes. `rk guide release` owns the sequence. A project whose record states `release.mode = "none"` has no release request, and its integration ends at step 4.
