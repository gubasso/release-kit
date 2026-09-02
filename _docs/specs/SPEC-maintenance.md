# Maintenance Specification

## Purpose

Rules governing the local-repository housekeeping the release convention leaves behind: the branches a squash merge retires in every clone, the `rk branches prune` verb that reports and deletes them, and the post-merge reminder hook `rk setup step branch-reminder` installs. Its subject is the operator's own clone — local refs, local hooks — which is neither calling a remote API on the operator's behalf, `SPEC-forge-setup.md`, nor landing payload files into a target's worktree, `SPEC-landing.md`; the one forge call here reads proof and configures nothing. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification.

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

The prune verb MUST delete nothing without `--apply`, and its report MUST name the deletion as the operator's action.

#### Scenario: An agent reads the post-merge report

- GIVEN a report the reminder hook printed after a pull
- WHEN an agent reads it
- THEN the closing line states that deleting a branch is the operator's action, so the agent states the command and waits to be asked

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
