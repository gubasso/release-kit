# 10 — Migration

How a repository moves from another release tool to this convention without losing a byte, a forge setting, or a release path. [Setup](./02-setup.md) bootstraps a bare repository once, in order; this chapter owns the target another tool releases today. A migration is a greenfield landing with an archive in front of it: the predecessor's files move into a workspace, a clean instance lands, and what the project still needs is carried from the archive one group at a time. The command form of this chapter is the migration runbook, `rk guide migration`; `rk assess` and `rk status` read the verdict the first step depends on.

## Classification before anything lands

A migration begins with a verdict about the target, read from evidence rather than judged by feel.

- greenfield: no release mechanism and no release history. Land the workflow through [setup](./02-setup.md); there is nothing to migrate.
- brownfield: another tool's release mechanism is in place. The mechanism is archived and replaced, never overwritten in place and never left as a second release path beside the new one.
- needs-decision: release activity no recognized mechanism explains, such as tags with no tool behind them, or a long-lived branch beside the trunk. The operator says what it is before any plan claims to know.

A recorded landing is a fourth state and not a classification: it routes to `rk status`, and from there to [landing](./13-landing.md), whose stage shows what an upgrade would replace and what the report names as drift. No record does not mean no release: a brownfield target with no record is the common case, and treating it as green is how a repository ends up with two release paths at once. An open migration is its own reading: `rk status` reports `migration` from the archive until the close, whatever else the target shows.

## The workspace and the registry

A migration spans many runs, so it keeps state, and this is the one place a run does. The workspace sits under this tool's state root, at `migrations/<repo>/<id>/`, where `<id>` is the second the archive opened it, with `-2` or a higher counter where an earlier archive of the target took that second. It holds the archived files under `sources/`, the snapshots no Git holds under `refs/` and `forge/`, and `registry.yaml`. The committed configuration names the open migration with its `migration` key, so every clone sees it, and a new run resumes from `rk migrate status`, never from memory.

The registry lists every source with its tool, its kind, its checksum, and its status: `pending`, `mapped`, `written`, `verified`, or `dropped`. Only the `rk migrate` verbs write it. Each status change appends a dated entry to the source's history and never rewrites an earlier one, so the registry is also the migration's log. A group gathers the sources one predecessor mechanism spread across files, and names the destinations that now carry what the project kept. Mapping is many to many: one source can feed several destinations, several sources can merge into one, and a source can be dropped with its reason recorded.

## The sequence

1. Inventory. `rk migrate inventory` lists every predecessor file with its tool and its kind: a workflow, a release configuration, a bump script, a hook, or a manifest. The inventory is the one detector of predecessor files. The changelog, the readme, the licence, and the agent entry document stay in place and are listed as such.
2. Archive. `rk migrate archive --apply` refuses while a listed file's bytes differ from its copy on the trunk, because the trunk must hold every byte it moves: an edit to a predecessor file integrates first, and a squash integration of the migration branch then keeps the bytes a rebuild reads. It moves every predecessor file into the workspace, bundles every long-lived branch and lists the tags, takes the before-state of every destructive forge step this target runs, writes the registry with the base commit and one entry per source, and sets the configuration key. The target is then a clean greenfield.
3. Land. The landing is a true greenfield `rk init` through [landing](./13-landing.md): no collision is left, so nothing is replaced. The development environment comes first, and a predecessor bump mechanism is removed through `rk self-depend clean` after its files are archived. The landed profile can run destructive steps the unlanded target did not, so the archive runs again after the landing and takes their snapshots.
4. Map. The agent reads one predecessor tool's sources together, so it sees the whole mechanism, and proposes each source's fate: replaced by a landed file, carried into a file the project owns, or dropped with its reason. The operator approves the map, and `rk migrate mark` records it in the registry.
5. Loop. One group at a time: write what is carried, read each destination against its sources, and mark the group `written`, then `verified`. Every step until here runs on a branch the operator owns, and the branch merges inside the request's authority.
6. Forge. After the branch merges, the forge steps of [setup](./02-setup.md) run on the live forge: the old protection comes off, `single-trunk` retires the long-lived branches, and the protections go on. Each destructive step refuses while the registry holds no snapshot of it. Then publish where the setup proof is not current.
7. Close. `rk migrate close --apply` refuses while a source is unfinished, a verified source's group names no destination the target holds, or the setup proof is not current. Then it removes the configuration key, and the configuration equals what a greenfield landing of the same answers writes. The workspace stays until the operator removes it, and the first release cut through [operate](./03-operate.md) is the migration's proof.

## The loop

Execution is a loop, not a script: observe, act on the smallest gap, re-observe, continue.

- A check that cannot be read is not a pass. A forge the CLI cannot reach, a probe that did not run, a report that errored: each is the next gap, named as such.
- A failed check is never continued past. The step that failed is the whole of the next action.
- Every gated step's outcome is reported from observation, never from the operator's word alone.
- The trunk stays releasable at every stop. The archive, the landing, the map, and the loop run on the operator's branch, so the trunk keeps releasing through the predecessor until that branch merges, and the forge steps run only after it merges, when the new release path is live.

## Snapshots before removal

Three steps remove what Git cannot rebuild: a protection's removal from a live branch, a long-lived branch's deletion, and a predecessor bump mechanism's cleanup. Each runs only after what it removes is saved. The archive takes the forge before-state of every destructive step this target runs, and a step whose snapshot the registry does not hold intact refuses. The forge half needs credentials: where the runtime has none, it reads unknown, and every destructive forge step stays refused until the archive runs again on a host that holds them. The archived files have a second copy in Git, and `rk migrate archive --rebuild` recreates them at the base commit on a clone whose workspace is gone. A forge snapshot and a branch bundle have no second copy, and the registry says so.

## Changing a landing parameter

A checkout-mode or release-style change is not a migration: edit `git.checkout_mode` or `profile.release.style` in `.release-kit/config.toml`, preview with `rk upgrade`, and apply with `rk upgrade --apply`. A flag overrides the configured key and an apply writes it back while preserving comments. [Worktrees](./08-worktrees.md) owns the transition for branches open across a mode change, and [release lines](./09-release-lines.md) owns what the style changes.

## Boundary tests

- A target with a release tool and no record is brownfield, not greenfield: classify by the mechanism, never by the record.
- A target with tags and no mechanism is a question, not a verdict: the operator says what the tags are.
- A recorded target routes by its status report, whatever the corpus verdict says: every healthy landing reads as brownfield, and that is not a migration.
- A listed file the trunk does not hold as it stands stops the archive: the edit integrates first, so the trunk holds the bytes.
- A destructive step with no snapshot in the registry is the migration's next finding, not a flag to add.
- A source nobody marked keeps the migration open: the registry is what was approved, at its own size.
