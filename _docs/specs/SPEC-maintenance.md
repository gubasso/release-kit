# Maintenance Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`maintenance:gone-is-a-candidate-not-proof` — Gone is a candidate, not proof](#maintenancegone-is-a-candidate-not-proof--gone-is-a-candidate-not-proof)
  - [`maintenance:a-checked-out-branch-is-never-pruned` — A checked-out branch is never pruned](#maintenancea-checked-out-branch-is-never-pruned--a-checked-out-branch-is-never-pruned)
  - [`maintenance:forge-unavailability-never-authorizes-deletion` — Forge unavailability never authorizes deletion](#maintenanceforge-unavailability-never-authorizes-deletion--forge-unavailability-never-authorizes-deletion)
  - [`maintenance:a-preview-precedes-every-deletion` — A preview precedes every deletion](#maintenancea-preview-precedes-every-deletion--a-preview-precedes-every-deletion)
  - [`maintenance:a-report-closes-only-on-what-is-still-owed` — A report closes only on what is still owed](#maintenancea-report-closes-only-on-what-is-still-owed--a-report-closes-only-on-what-is-still-owed)
  - [`maintenance:the-workflow-mode-is-a-landing-parameter` — The workflow mode is a landing parameter](#maintenancethe-workflow-mode-is-a-landing-parameter--the-workflow-mode-is-a-landing-parameter)
  - [`maintenance:worktree-mode-guards-the-main-checkout` — Worktree mode guards the main checkout](#maintenanceworktree-mode-guards-the-main-checkout--worktree-mode-guards-the-main-checkout)
  - [`maintenance:branches-mode-refuses-nothing` — Branches mode refuses nothing](#maintenancebranches-mode-refuses-nothing--branches-mode-refuses-nothing)
  - [`maintenance:a-worktree-path-derives-from-project-and-branch` — A worktree path derives from project and branch](#maintenancea-worktree-path-derives-from-project-and-branch--a-worktree-path-derives-from-project-and-branch)
  - [`maintenance:a-worktree-is-removed-before-its-branch` — A worktree is removed before its branch](#maintenancea-worktree-is-removed-before-its-branch--a-worktree-is-removed-before-its-branch)
  - [`maintenance:one-merge-proof-authorizes-both-removals` — One merge proof authorizes both removals](#maintenanceone-merge-proof-authorizes-both-removals--one-merge-proof-authorizes-both-removals)
  - [`maintenance:a-prune-report-covers-cleanup-not-inventory` — A prune report covers cleanup, not inventory](#maintenancea-prune-report-covers-cleanup-not-inventory--a-prune-report-covers-cleanup-not-inventory)
  - [`maintenance:an-unobservable-branch-is-never-a-candidate` — An unobservable branch is never a candidate](#maintenancean-unobservable-branch-is-never-a-candidate--an-unobservable-branch-is-never-a-candidate)
  - [`maintenance:a-dirty-or-locked-worktree-is-never-removed` — A dirty or locked worktree is never removed](#maintenancea-dirty-or-locked-worktree-is-never-removed--a-dirty-or-locked-worktree-is-never-removed)
  - [`maintenance:a-branch-deletion-is-compare-and-swap` — A branch deletion is compare-and-swap](#maintenancea-branch-deletion-is-compare-and-swap--a-branch-deletion-is-compare-and-swap)
  - [`maintenance:a-line-is-cut-from-an-explicit-base` — A line is cut from an explicit base](#maintenancea-line-is-cut-from-an-explicit-base--a-line-is-cut-from-an-explicit-base)
  - [`maintenance:a-line-is-never-retired-before-its-tags` — A line is never retired before its tags](#maintenancea-line-is-never-retired-before-its-tags--a-line-is-never-retired-before-its-tags)
  - [`maintenance:the-reminder-is-silent-on-a-binary-that-cannot-prune` — The reminder is silent on a binary that cannot prune](#maintenancethe-reminder-is-silent-on-a-binary-that-cannot-prune--the-reminder-is-silent-on-a-binary-that-cannot-prune)
  - [`maintenance:a-foreign-hook-is-never-clobbered` — A foreign hook is never clobbered](#maintenancea-foreign-hook-is-never-clobbered--a-foreign-hook-is-never-clobbered)
  - [`maintenance:the-reminder-never-blocks-a-pull` — The reminder never blocks a pull](#maintenancethe-reminder-never-blocks-a-pull--the-reminder-never-blocks-a-pull)

<!--TOC-->

## Purpose

Rules governing the local-repository housekeeping the release convention leaves behind: the two local resources a squash merge retires in every clone — branches and the worktrees that seat them — the `rk branches prune` and `rk worktree` verbs that report and retire them, the workflow mode that decides which desk-level guards the landing renders, and the post-merge reminder hook `rk setup step branch-reminder` installs. Its subject is the operator's own clone — local refs, local worktrees, local hooks — which is neither calling a remote API on the operator's behalf, `SPEC-forge-setup.md`, nor deciding what a landing writes into a target, `SPEC-landing.md`; the one forge call here reads proof and configures nothing, and the mode's record and rendering mechanics belong to the landing while the rules the rendered guards enforce belong here. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The upstream documentation behind these rules is in `../reference/REFERENCE-maintenance-sources.md`.

## Requirements

### `maintenance:gone-is-a-candidate-not-proof` — Gone is a candidate, not proof

When `rk branches prune --apply` deletes a branch, the deletion MUST rest on a merged request whose recorded head equals the branch's tip, never on the gone upstream alone.

#### Scenario: A branch advanced after its request merged

- GIVEN a branch whose request merged at commit A, whose local tip moved to B, and whose remote branch the forge then deleted
- WHEN `rk branches prune --apply` runs
- THEN the merged request records A, the tip is B, no proof covers B, and the branch stays

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-checked-out-branch-is-never-pruned` — A checked-out branch is never pruned

The prune verb MUST keep the current branch, a branch checked out in any worktree, the trunk, and every `release/*` line out of the candidate set, whatever their upstreams say.

#### Scenario: A gone branch is checked out in a linked worktree

- GIVEN a branch whose upstream is gone and whose checkout lives in a linked worktree
- WHEN `rk branches prune --apply` runs
- THEN the branch is reported worktree-bound with its worktree path and is not deleted, because its worktree owns the cleanup

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:forge-unavailability-never-authorizes-deletion` — Forge unavailability never authorizes deletion

If the forge cannot answer for a candidate, then the prune verb MUST report that candidate as unknown and keep it, in every mode.

#### Scenario: The forge is down during an apply

- GIVEN two candidates, one the forge confirms before the outage and one it cannot answer for
- WHEN `rk branches prune --apply` runs
- THEN the confirmed branch is deleted, the unanswered branch is kept and reported unknown, and the exit stays 0

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-preview-precedes-every-deletion` — A preview precedes every deletion

The prune verbs MUST delete nothing without `--apply`, and a report in which some reported row still names a move the operator may make MUST close by naming that move as the operator's action.

#### Scenario: An agent reads the post-merge report

- GIVEN a report the reminder hook printed after a pull
- WHEN an agent reads it
- THEN the closing line states that deleting a branch is the operator's action, so the agent states the command and waits to be asked

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-report-closes-only-on-what-is-still-owed` — A report closes only on what is still owed

The closing operator line MUST ride the reported rows, never the mode: it prints while at least one row still names a move the operator may make — every kept, candidate, judged, and failure row, and a finished row whose detail reports residue — and it MUST drop from an apply that finished everything it named and from an empty report, and every failure row's detail MUST name its recovery.

#### Scenario: An apply finishes everything it named

- GIVEN one confirmed branch and its apply deleting it cleanly
- WHEN the report renders
- THEN no row still names a move, and the report closes without the operator line

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:the-workflow-mode-is-a-landing-parameter` — The workflow mode is a landing parameter

The working-copy mode MUST be recorded in the manifest, rendered into the landed blocks, reported by every parameter-bearing report, judged by `rk status --check`, and changed only through the landing verbs; a record predating the field MUST read as `branches`, and adoption MUST default to `branches`, so no upgrade or adoption ever imposes a guard the project did not choose.

#### Scenario: A pre-mode record upgrades

- GIVEN a target whose record predates the workflow parameter
- WHEN `rk upgrade --apply` runs
- THEN the rewritten record states `branches`, and the landed blocks carry no worktree guard

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:worktree-mode-guards-the-main-checkout` — Worktree mode guards the main checkout

In the worktree mode the landed hook block MUST refuse every commit in the main checkout, detached HEAD included — `master` through the trunk guard, everything else through the location guard — and MUST pass every linked worktree by topology; the mirror is desk-level, honest about `--no-verify`, and its one named escape is the sweep-skip pair the block's comment states.

#### Scenario: The main checkout tries a detached commit

- GIVEN a worktree-mode target's main checkout on a detached HEAD
- WHEN the location guard runs at commit time
- THEN it refuses, naming `rk worktree add` as the way to a seat

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:branches-mode-refuses-nothing` — Branches mode refuses nothing

In the branches mode the landed blocks MUST carry no location guard, and the worktree verbs MUST behave identically in both modes, so both working-copy forms work and the mode gates only what the landed blocks render.

#### Scenario: A branches-mode project uses a worktree

- GIVEN a branches-mode target and a branch seated through `rk worktree add`
- WHEN work proceeds in the main checkout and the worktree alike
- THEN nothing refuses either form, and `rk worktree prune` retires the merged worktree the same way

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-worktree-path-derives-from-project-and-branch` — A worktree path derives from project and branch

`rk worktree add` MUST derive the path as the sibling `../<project>@<flattened branch>` with `@` separating the two halves and every `/` in the branch flattened to `-`, MUST refuse a collision by name and never suffix silently, MUST refuse a branch whose standing seat is off the derived path with `git worktree move` named as the move, and an off-path worktree MUST be reported by `rk worktree list` and never refused.

#### Scenario: Two branches flatten to one directory

- GIVEN a seated `feat/a-b` and a request to seat `feat/a/b`
- WHEN `rk worktree add feat/a/b --apply` runs
- THEN it refuses naming the occupying branch, and nothing is created

#### Scenario: A branch stands seated off the derived path

- GIVEN a branch seated at a path the derivation does not produce
- WHEN `rk worktree add <branch> --apply` runs
- THEN it refuses naming `git worktree move` as the move, and nothing is created

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-worktree-is-removed-before-its-branch` — A worktree is removed before its branch

`rk worktree prune --apply` MUST remove the worktree before deleting its branch, with each outcome independent: a failed removal MUST leave the branch and its configuration untouched, and a branch that outlives its removed worktree MUST be reported truthfully with `rk worktree add <branch> --apply` named as the recovery.

#### Scenario: Dirt makes git refuse the removal

- GIVEN a confirmed worktree whose removal git refuses
- WHEN the apply runs
- THEN the row reports the failure with its recovery, and the branch and its configuration survive whole

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:one-merge-proof-authorizes-both-removals` — One merge proof authorizes both removals

The worktree prune MUST rest on the same merged-request predicate as the branch prune — a merged request whose recorded head equals the branch's tip — and MUST re-observe at the moment of action: a tip, a lock, or dirt that arrived after verification keeps the worktree, and a forge that cannot answer keeps everything.

#### Scenario: The tip moves between verification and the apply

- GIVEN a worktree the forge confirmed whose branch then advanced
- WHEN `rk worktree prune --apply` acts
- THEN the re-observation keeps the worktree and nothing is removed

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-prune-report-covers-cleanup-not-inventory` — A prune report covers cleanup, not inventory

A `rk worktree prune` row MUST exist only for a stale record or a worktree whose branch observation is gone or missing — never for the main worktree or a healthy linked one; inventory is `rk worktree list`'s. This is what lets the quiet form be silent in a healthy clone, which the reminder's clean-clone guarantee rests on.

#### Scenario: A healthy clone with one active seat

- GIVEN a main checkout and one linked worktree on a live branch
- WHEN `rk worktree prune --quiet` runs
- THEN it prints nothing

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:an-unobservable-branch-is-never-a-candidate` — An unobservable branch is never a candidate

A worktree whose branch observation is missing or malformed MUST be kept with the reason naming the missing observation, and a branch inventory that does not parse at all MUST refuse the run before any judgment.

#### Scenario: A seat whose branch ref vanished

- GIVEN a linked worktree whose branch the branch listing no longer covers
- WHEN `rk worktree prune` runs
- THEN the row is kept naming the missing observation, and nothing is guessed into a candidate

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-dirty-or-locked-worktree-is-never-removed` — A dirty or locked worktree is never removed

The prune MUST keep a dirty worktree — untracked files count — and MUST keep a locked one unconditionally, missing directory included: a lock is someone's statement, and `rk` never unlocks.

#### Scenario: A locked record whose directory is gone

- GIVEN a locked worktree record whose directory was deleted by hand
- WHEN `rk worktree prune --apply` runs
- THEN the record is kept intact, and only unlocked missing-directory records clear

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-branch-deletion-is-compare-and-swap` — A branch deletion is compare-and-swap

Every branch deletion in both prune verbs MUST go through one shared helper: `git update-ref -d` carrying the verified tip, then the `branch.<name>` configuration cleanup, so a tip that moved is refused rather than lost and a reused name inherits nothing stale.

#### Scenario: The tip moves inside the residual window

- GIVEN a verified tip the branch no longer holds
- WHEN the shared deletion runs
- THEN the compare-and-swap refuses, and the branch and its work survive

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-line-is-cut-from-an-explicit-base` — A line is cut from an explicit base

`rk lines open` MUST require an explicit base and MUST NOT fall back to the trunk's tip, because a line is a snapshot of a chosen commit and a tip-cut line silently ships whatever landed that morning.

#### Scenario: A line is opened with no base

- GIVEN a request to open `release/1.1` with no `--base`
- WHEN `rk lines open 1.1` runs
- THEN it refuses naming the flag and the tag form the chapter recommends, and nothing is created

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-line-is-never-retired-before-its-tags` — A line is never retired before its tags

`rk lines retire` MUST refuse while any commit the line holds and the trunk does not is unreachable from the line's tags, and MUST leave the remote branch's deletion to the operator, because a tag is what keeps a line's commits reachable after its branch dies and an untagged retirement garbage-collects the line.

#### Scenario: A line's tip is untagged

- GIVEN `release/1.1` whose tip cherry-pick landed after the last `v1.1.z` tag
- WHEN `rk lines retire 1.1 --apply` runs
- THEN it refuses naming the commits no tag reaches, the seat and both branches survive, and the report names tagging as what would make the retirement safe

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:the-reminder-is-silent-on-a-binary-that-cannot-prune` — The reminder is silent on a binary that cannot prune

The installed hook MUST print nothing when the `rk` on the `PATH` does not carry the verb it is about to call, and MUST probe each verb separately, so a missing binary, one too old for a verb, and one that renamed it all fail identically and silently while a capable verb's own diagnostics still reach the operator.

#### Scenario: A clone's rk predates a verb

- GIVEN a clone carrying the reminder on a host whose `rk` predates `rk worktree prune`
- WHEN a pull merges
- THEN the hook runs the capable verb, prints nothing for the incapable one, and exits 0

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:a-foreign-hook-is-never-clobbered` — A foreign hook is never clobbered

If a post-merge hook without the release-kit marker exists, then `rk setup step branch-reminder` MUST refuse and leave the file's bytes unchanged.

#### Scenario: A husky-managed hooks directory holds its own post-merge hook

- GIVEN `core.hooksPath` naming a directory whose `post-merge` carries no release-kit marker
- WHEN the step applies
- THEN it refuses, names the manual merge as the way forward, and the existing hook keeps its bytes

Verify: `cargo nextest run -E 'binary(cli)'`

### `maintenance:the-reminder-never-blocks-a-pull` — The reminder never blocks a pull

The installed hook MUST exit 0 whatever happens inside it, and MUST print nothing when the clone holds no gone branch.

#### Scenario: The rk binary is uninstalled after the hook landed

- GIVEN a clone carrying the reminder hook on a host where `rk` left the `PATH`
- WHEN a pull merges
- THEN the hook finds no `rk`, prints nothing, and exits 0

Verify: `cargo nextest run -E 'kind(lib)'`
