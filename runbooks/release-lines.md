# Release-lines runbook

The eight steps of [release lines](../method/09-release-lines.md) as commands: the chapter owns each step's why, this page owns its how. `<line>` is the line's `major.minor`, and `<repo>` is the project path, filled in by `rk guide release-lines` where detection resolves it; a fix crossing to a line is `rk guide backport`, and this page is the line's own life. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## 1. Record the style

```bash
rk status --target .
# check: prints the lines style
# trunk style: this project ships from the trunk; rk upgrade --target . --style lines --apply changes it, and the diff is the arming line in the release workflow
# no style in the record: the record predates the parameter; the upgrade refuses until --style names one
```

## 2. Wire the repository for lines

Automated: `rk setup step protect-release-lines --apply`, once per repository.

```bash
rk setup check --target .
# check: protect-release-lines reports satisfied
```

## 3. Open the line

The base is chosen and stated, never defaulted; the verb refuses without it.

On worktree:

```bash
git fetch origin --tags --force
rk lines open <line> --base "v<version>" --apply
rk worktree add release/<line> --apply && cd "../<project>@release-<line>"
git push -u origin release/<line>
# check: the line stands at its explicit base in its own seat, and the push publishes it with its upstream set
# already open: the open adopts the existing branch, the add adopts the existing seat, and each reports satisfied
```

On branches:

```bash
git fetch origin --tags --force
rk lines open <line> --base "v<version>" --apply && git checkout release/<line>
git push -u origin release/<line>
# check: the branch stands at its explicit base, and the push publishes it with its upstream set
# already open: the open adopts the existing branch and reports satisfied
```

## 4. Cross the fix

`rk guide backport` steps 4 to 6 own the crossing: fix on the trunk first, cherry-pick only that commit, wait for the line's own CI.

```bash
git log --oneline -1 origin/release/<line>
# check: the tip is the crossed fix and nothing else traveled
```

## 5. Read the candidate

A candidate is `v<version>-rc.<n>` on the line, minted by the binding's rc automation where one is wired; the landed release workflows tag releases, not candidates, and this verb reads what exists.

```bash
git fetch origin --tags --force
rk lines rc <line>
# check: names the newest candidate on the line and the next number a finding would mint
# nothing listed: no candidate is tagged; where the binding wires no rc path, the line's release request stays the human gate and this step reads empty
```

## 6. Validate the candidate

On github:

```bash
gh release view "v<version>-rc.<n>" --repo <repo> --json assets -q '[.assets[].name] | join(", ")'
# check: the candidate's artifacts exist; install one and use it — the human validation is the gate no command prints
```

On gitlab:

```bash
glab release view "v<version>-rc.<n>"
# check: the candidate's page stands; install what it links and use it — the human validation is the gate no command prints
```

## 7. Answer a finding with the next rc

The finding crosses like any fix — step 4 — and the rc path that minted the last candidate mints the next number.

```bash
rk lines rc <line>
# check: the newest candidate advanced by one; an rc number is single-use, because the tag protection makes a candidate immutable
```

## 8. Promote, then retire

### 8a. Promote the candidate

Run `rk guide release` steps 3 to 6 against the line; a line's request is never armed, so the merge is yours, per that runbook's step 4.

On github:

```bash
gh pr list --repo <repo> --state open --base release/<line>
# check: the line's release request proposes the validated version
```

On gitlab:

```bash
glab mr list
# check: the line's release request proposes the validated version
```

### 8b. Retire the line

Only when the line leaves production, and only behind its tags.

```bash
rk lines retire <line>
rk lines retire <line> --apply
# check: the preview names the seat and the local branch; the apply removes the seat before the branch and leaves the remote deletion to you
# refused: a commit the line holds is unreachable from its tags; tag it first, or the deletion garbage-collects it
git push origin --delete release/<line>
# check: the line's tags still resolve; the chapter owns why they make the deletion safe
```
