# Migration Specification

## Purpose

Rules governing `rk migrate`, the archive-first migration from another release tool: the inventory that detects predecessor files, the archive that moves them into a workspace under the state root, the registry only these verbs write, and the close that ends the migration. The landing a migration makes belongs to [the landing specification](./SPEC-landing.md), the forge steps that follow it to [the forge setup specification](./SPEC-forge-setup.md), and the configuration key that names the open migration to [the target configuration specification](./SPEC-target-config.md). The method's why is `method/10-migration.md`.

## Requirements

### `migration:an-archive-moves-only-committed-files` — An archive moves only committed files

When `rk migrate archive` runs, the binary MUST refuse, writing nothing, while any file the inventory lists has raw bytes other than its regular-file copy on the local branch `git.trunk` names, or the trunk holds a listed file the target lacks, MUST refuse while the configuration's directory or file is a symlink or of another kind, MUST record that trunk commit as the registry's base commit, and MUST move only the files the inventory lists as archived, keeping the root files in place, because the trunk is the second copy of every moved byte, and a file it does not hold, an ignored or unintegrated one included, would be lost if the workspace were.

#### Scenario: A predecessor file carries an edit the trunk does not hold

- GIVEN a target whose `release-please-config.json` differs from its copy on the trunk, uncommitted or committed on the migration branch alone
- WHEN `rk migrate archive --apply` runs
- THEN it exits 73 naming the file, and the target and the state root are unchanged

Verify: `cargo nextest run -E 'test(an_archive_refuses_an_uncommitted_listed_file_and_writes_nothing) or test(an_ignored_predecessor_file_refuses_the_archive) or test(a_predecessor_file_that_differs_from_the_trunk_refuses_the_archive) or test(a_predecessor_file_deleted_off_the_trunk_refuses_the_archive) or test(an_archive_refuses_a_symlinked_configuration_directory) or test(an_archive_moves_every_predecessor_file_and_opens_the_migration)'`

### `migration:the-configuration-names-the-open-migration` — The configuration names the open migration

While a migration is open, `rk status` MUST report it and MUST read `classification: migration` until `rk migrate close --apply`, and a second archive MUST refuse, because the archive empties the target on purpose and every clone resumes from the committed key and the registry, never from memory.

#### Scenario: A clone reads a migrated target

- GIVEN a target whose archive opened a migration and whose branch carries the key
- WHEN `rk status --json` runs on any clone
- THEN it reports `classification: migration` and the migration's id

Verify: `cargo nextest run -E 'test(an_archive_moves_every_predecessor_file_and_opens_the_migration)'`

### `migration:a-mark-appends-and-never-rewrites` — A mark appends and never rewrites

When `rk migrate mark` records a status, the binary MUST append one dated entry to each moved source's history and MUST NOT rewrite an earlier entry, a drop MUST carry its reason, and a destination that is absolute or leaves the target MUST refuse as a usage error, because the registry is the migration's log and a history that can change proves nothing. Every source MUST carry an id no other source holds: the slug of its path, or, for a later path in byte order whose slug an earlier one took, that slug with the first free `-<n>` counter from 2.

#### Scenario: A group moves from mapped to written

- GIVEN a source filed into a group with one `mapped` entry
- WHEN the group is marked `written` with a destination
- THEN the source's history reads `pending`, `mapped`, `written`, with the earlier entries byte-identical

Verify: `cargo nextest run -E 'test(a_mark_appends_history_and_never_rewrites_an_earlier_entry) or test(a_mark_files_a_source_and_moves_a_group_appending_history) or test(colliding_source_paths_take_distinct_ids) or test(a_destination_outside_the_target_refuses)'`

### `migration:a-destructive-step-waits-for-its-snapshot` — A destructive step waits for its snapshot

While a migration is open, a setup step that removes or replaces forge state MUST refuse while the registry holds no intact snapshot of it, none or one whose file is gone or no longer matches its checksum, the archive run again MUST set an altered snapshot file aside and take the snapshot anew, and a snapshot file that cannot be read MUST stop the run with its entry kept, and the archive MUST record a step whose before-state it cannot read as unknown rather than taken, because a forge setting has no copy in Git and a step that removes it unsaved loses it.

#### Scenario: The archive ran without forge credentials

- GIVEN an open migration whose registry holds no snapshot of `default-branch`
- WHEN `rk setup step default-branch --apply` runs
- THEN it exits 73 naming the missing snapshot, changes nothing, and runs once the archive took it

Verify: `cargo nextest run -E 'test(a_target_migrates_from_its_predecessor_and_closes) or test(an_altered_forge_snapshot_refuses_its_step_and_is_taken_again) or test(an_unreadable_forge_snapshot_stops_the_resume_and_keeps_its_entry)'`

### `migration:a-rebuild-recreates-the-files-alone` — A rebuild recreates the files alone

When `rk migrate archive --rebuild --apply` runs, the binary MUST recreate the archived files from Git at the base commit, the registry's where one exists, else the first parent of the oldest commit on `HEAD`'s first-parent history that records the configuration key, a merge judged by its diff against its first parent, MUST select the regular files the inventory lists and nothing else, and only the registry's sources where one exists, checking each against the registry, and MUST NOT claim to rebuild a snapshot, because the workspace is local state another clone does not hold and only the files have a second copy. A squash integration keeps that parent's bytes, because the archive refused every file the trunk did not hold. Without `--apply`, the verb MUST report the files it would recreate and write nothing, as every other verb previews. A rebuild with a registry recreates the archived bytes, and one without a registry recreates the trunk's bytes at the merge, which a later predecessor release on the trunk may have changed.

#### Scenario: A second clone resumes a migration after the squash

- GIVEN an archive committed on a branch that was squash-merged onto the trunk and deleted, and a host that holds no workspace for it
- WHEN `rk migrate archive --rebuild --apply` runs
- THEN the archived files reappear with the bytes of the squash commit's first parent, the archive's own where no predecessor release came between, and the registry lists no snapshot

Verify: `cargo nextest run -E 'test(a_rebuild_recreates_the_archived_bytes_from_git_at_the_base_commit) or test(a_rebuild_after_a_squash_merged_branch_restores_the_archived_bytes) or test(a_rebuild_with_its_registry_survives_a_squash_merge_and_a_deleted_branch) or test(a_rebuild_lists_regular_files_alone) or test(a_rebuild_with_its_registry_recreates_its_sources_alone) or test(a_key_first_recorded_in_a_merge_commit_is_found) or test(a_key_a_merged_branch_recorded_is_found_at_the_merge)'`

### `migration:close-refuses-an-unfinished-migration` — Close refuses an unfinished migration

When `rk migrate close` runs, the binary MUST refuse while any source is neither verified nor dropped, a verified source's group names no destination, a group holding a source that is not dropped names a destination the target does not hold as a regular file inside itself, its resolved path counted, or the setup proof is not compatible, and MUST remove only the configuration key once none holds, because the close is the gate that says nothing was lost and the new release path stands.

#### Scenario: A source was never marked

- GIVEN an open migration with a source still pending
- WHEN `rk migrate close --apply` runs
- THEN it exits 73 naming the source, and the key stays

Verify: `cargo nextest run -E 'test(a_close_refuses_while_a_source_is_unfinished_and_keeps_the_key) or test(a_group_with_no_live_source_does_not_block_close) or test(a_destination_reached_through_a_symlink_out_of_the_target_does_not_count) or test(a_directory_destination_does_not_count) or test(a_target_migrates_from_its_predecessor_and_closes)'`
