# Integration runbook

The four steps of [integration](../method/12-integration.md) as commands: the chapter owns each step's why, this page owns its how. The recorded integration mode selects the variants below, and `rk integrate` resolves the same mode the same way. `<project>` is the main checkout's directory name, filled in by `rk guide integration` where detection resolves it. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

The recorded merge form, `git.merge_form`, decides what reaches the trunk under either mode. `squash`, the default, writes one commit for the branch, with the message you give it. `rebase` rebases the branch on the trunk tip and fast-forwards the trunk to it, so every commit of the branch arrives with its own message and no new commit is written. Choose `squash` where one implementation reads best as one trunk commit. Choose `rebase` where every commit of the branch is already a conventional step worth keeping on the trunk. `rk init --merge-form <form>` records the answer, `rk upgrade --merge-form <form> --apply` changes it, and a project that never answered keeps `squash`. [Integration](../method/12-integration.md) owns the choice.

## 1. Ready the branch in its seat

The implementation is committed on its short-lived branch, in the seat [the worktree runbook](./worktree.md) created. Integration starts from a clean seat, because the gate judges the tree it finds.

```bash
cd ../<project>@<type>-<slug>
git status --porcelain                   # check: empty; a dirty seat refuses at step 3
```

## 2. Run the gate the boundary needs

The project assigns every check to the earliest stage that can run it, and the stage before the boundary carries the full suite. Run it before the boundary rather than at it, so a failure costs nothing:

```bash
pre-commit run --hook-stage manual --all-files
# check: every hook reports Passed or Skipped
```

On local:

This is the stage `rk integrate` invokes. Running it here first is the same command against the same tree.

On forge:

The full branch suite is the `pre-push` stage, which git fires on step 3's push. The `manual` stage is an operator sweep here and holds nothing back.

## 3. Integrate

On local:

```bash
rk integrate <type>/<slug>                               # preview: names the rebase, the gate, and the squash
rk integrate <type>/<slug> -m "<type>(<scope>): <subject>" --apply
# check: the report names the trunk's new commit and records the integration
```

The message becomes the trunk's commit message, so it is a Conventional Commit with a scope wherever the record names a release. Under `release.mode = "none"` the scope is preferred and not required, because no changelog reads it. `git commit-tree` fires no hook, and `rk integrate` holds the message to the same convention the landed `commit-msg` stage does. Every refusal leaves the trunk at the tip it started from. The rebase form, which takes no message, is 3b.

On forge:

```bash
rk integrate <type>/<slug> --forge --apply               # runs the pre-push stage through the push
# check: the report names the request command for this forge
```

The request itself is yours to open, with the command the report names. Under `squash` its title becomes the trunk's commit message, so it follows the same convention as a local message, and `rk message --check --kind body` judges the body before you post it. The forge holds the merge behind its required check.

A local publication reads the trunk and the worktree that holds it again right before it moves anything, and a difference refuses. Where no worktree holds the trunk, the move is one compare-and-swap of the trunk ref. Where one does, it is that worktree's fast-forward, confirmed afterwards from the trunk's reflog or from the fast-forward's own report of the move in full object names: Git offers no compare-and-swap for a checked-out branch that also moves its worktree, so a writer outside `rk` can still move the trunk in between. The trunk is then already published. Where the move started elsewhere, or where neither source can confirm its start, `rk integrate` exits 1 with the reason `state-drift`, its report naming the published commit and its diagnostic naming the commit the move started at or why its start is unconfirmed. Nothing is rolled back: read the commits between the observed tip and that commit with the `git log` the diagnostic names, and decide whether they belong on the trunk.

### 3a. A refusal and its recovery

| The refusal names            | What to do                                                     |
| ---------------------------- | -------------------------------------------------------------- |
| a dirty seat                 | commit or stash in the seat, then re-run                       |
| a branch off the grammar     | rename the branch to `<type>/<slug>` or the forge-minted form  |
| a diverged trunk             | fast-forward the trunk, or push it, then re-run                |
| a rebase conflict            | resolve it in the seat, commit, then re-run                    |
| the manual stage             | fix what the hooks reported, commit, then re-run               |
| a trunk that moved           | re-run; the second attempt rebases onto the tip that arrived   |
| a trunk checked out since    | re-run, or remove the new worktree of the trunk first          |
| another run holding the lock | wait for it; two integrations in one clone write one trunk ref |
| `-m` under the rebase form   | drop `-m`; every commit of the branch keeps its own message    |
| a branch commit's message    | reword that commit in the seat, then re-run                    |

### 3b. The rebase form

Under `git.merge_form = "rebase"`, the local integration takes no message:

```bash
rk integrate <type>/<slug>                               # preview: names the rebase, the gate, and each commit it judges
rk integrate <type>/<slug> --apply
# check: the report names the branch tip as the trunk's new commit; no new commit was written, and none is a merge commit
# exit 73 with the reason merge-form-mismatch: a -m was given; drop it
```

Every commit between the trunk and the branch tip becomes a trunk commit, so `rk integrate` judges each message by the landed `commit-msg` judgment before it moves anything, and names the first commit it refuses. Its rebase flattens every merge and moves no other branch, whatever the clone's Git configuration asks, and a range that still holds a merge commit refuses with the trunk unmoved. The trunk then carries the same commits as the branch, in order. At the forge, the request command asks for no squash, and a commit gate judges every commit of the request on each push in place of the title gate.

## 4. Push the trunk and prune

On local:

The trunk push is separate and deliberate, and it takes the fast-forward form alone. On GitHub, authenticate as a repository administrator: that is the one role the local-mode trunk ruleset permits to bypass its request rule. The push stays a fast-forward because the second ruleset refuses a rewrite from that administrator too. Push when you decide to; several integrations may ride one push, and the release bot reads each commit's intent separately, counting only the commits [its binding](../bindings/README.md) attributes to the package; `rk integrate` warns where it can prove one will not count.

```bash
git -C ../<project> push origin master   # check: a fast-forward; never --force
rk worktree prune --verify               # check: confirmed, locally integrated as <commit>
rk worktree prune --apply                # check: pruned; the seat and the branch went together
```

Where another writer won the push, fetch, replay the implementation onto the new tip, run the gates again, and push again. Nothing here force-pushes.

On forge:

```bash
git fetch --prune origin
rk worktree prune --verify               # check: confirmed against the merged request
rk worktree prune --apply                # check: pruned; the seat and the branch went together
```

After a local rebase integration, a GitLab fast-forward merge, or a hand `git merge --ff-only`, the trunk reaches the branch tip itself, so the verify may name the reachable tip as the proof. A GitHub rebase merge writes new commits, so there the merged request stays the proof. [The worktree runbook](./worktree.md) step 4 lists the three proofs.

### 4a. When the push turns the trunk red

The local path cannot see the remote checks before the push, because the push is what starts them. Watch the run the push started and the release request it refreshed. A failure leaves the release request open, so nothing publishes: the trunk's strict rule is the boundary, and the landed `release-gate` job logs which `setup.required_check` result it read before returning a successful no-op. Repair it as a fresh implementation, integrated the same way: no local integration is erased, and nothing is reverted by hand.

## 5. Release

Unchanged by the mode. The release request integrates at the forge in both, and merging it is what tags and publishes: `rk guide release` owns the sequence. A project whose record states `release.mode = "none"` has no release request, and its integration ends at step 4.
