# Backport runbook

The nine steps of [branch for release](../method/07-branch-for-release.md) as commands: the chapter owns each step's why, this page owns its how. Only when someone on an older version needs a fix without the newer work — the line is a second trunk, taking the same protection, the same release request, and the same verification, and only the cut and the cherry-pick are new. `<line>` is the line's `major.minor`, and `<repo>` is the project path, filled in by `rk guide backport` where detection resolves it. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## 1. Cut the line from its tag

On branches:

```bash
git fetch origin --tags --force
git checkout -b release/<line> "v<version>" && git push -u origin release/<line>
# check: the branch is created at the tag and pushed with its upstream set
# already cut: the checkout refuses the existing branch; git checkout release/<line> and go to 2
```

On worktree:

```bash
git fetch origin --tags --force
rk worktree add release/<line> --base "v<version>" --apply && cd ../<project>-release-<line>
git push -u origin release/<line>
# check: the line takes its explicit base and its own seat; the main checkout stays on master
# already cut: the add adopts the existing branch instead, and reports satisfied when its seat stands
```

## 2. Protect the release lines

Once per repository, covering every `release/*` line.

```bash
rk setup step protect-release-lines --target . --apply
# check: reports applied the first time and satisfied after; 3 proves it either way
```

## 3. Prove the protection

```bash
rk setup check --target .
# check: protect-release-lines reports satisfied
```

## 4. Fix on the trunk first

Test plus fix, one request, squash-merged through the trunk's one path — `rk guide setup` step 4 owns that path. Note the resulting trunk commit; 5 carries it across.

## 5. Cherry-pick only that commit onto the line

On branches:

```bash
git checkout release/<line> && git cherry-pick <commit> && git push
# check: the pick lands cleanly and the push is a plain push; nothing else travels
```

On worktree:

```bash
cd ../<project>-release-<line> && git cherry-pick <commit> && git push
# check: the pick lands in the line's own seat; nothing else travels
```

## 6. Wait for the line's CI

On github:

```bash
gh run watch --repo <repo> --exit-status \
  "$(gh run list --repo <repo> --workflow <ci workflow> \
     --commit "$(git rev-parse HEAD)" --limit 1 --json databaseId -q '.[0].databaseId')"
# check: completes with 'success'; the CI workflow is the project's own
```

On gitlab:

```bash
glab ci status --wait
# check: the line's pipeline passes
```

## 7. Read the line's own release request

On github:

```bash
gh pr list --repo <repo> --state open --base release/<line>
# check: a release request proposing the line's patch version
```

On gitlab:

```bash
glab mr list
# check: a release request against the line, proposing its patch version
```

## 8. Release the line

Run `rk guide release` steps 3 to 6 against the line, with `--base release/<line>` and `origin/release/<line>` wherever they name the trunk; the same automation tags the line's patch there.

## 9. Delete the branch when the line dies

```bash
git push origin --delete release/<line>
# check: the line's tags still resolve; the chapter owns why they make the deletion safe
```

Then the local half — a release line is protected from every automatic prune, so its seat and branch retire by hand:

```bash
git worktree remove ../<project>-release-<line>
git branch -D release/<line>
# check: rk worktree list no longer names the seat
```
