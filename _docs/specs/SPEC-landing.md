# Landing Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`landing:a-landing-leaves-a-record` — A landing leaves a record](#landinga-landing-leaves-a-record--a-landing-leaves-a-record)
  - [`landing:a-record-states-its-schema` — A record states its schema](#landinga-record-states-its-schema--a-record-states-its-schema)
  - [`landing:a-rendered-file-is-reproducible` — A rendered file is reproducible](#landinga-rendered-file-is-reproducible--a-rendered-file-is-reproducible)
  - [`landing:the-release-style-is-a-landing-parameter` — The release style is a landing parameter](#landingthe-release-style-is-a-landing-parameter--the-release-style-is-a-landing-parameter)
  - [`landing:a-rendered-file-carries-no-judgment` — A rendered file carries no judgment](#landinga-rendered-file-carries-no-judgment--a-rendered-file-carries-no-judgment)
  - [`landing:ownership-is-elementary` — Ownership is elementary](#landingownership-is-elementary--ownership-is-elementary)
  - [`landing:a-partial-landing-is-visible-and-rerunnable` — A partial landing is visible and rerunnable](#landinga-partial-landing-is-visible-and-rerunnable--a-partial-landing-is-visible-and-rerunnable)
  - [`landing:a-missing-receipt-is-a-classification` — A missing receipt is a classification](#landinga-missing-receipt-is-a-classification--a-missing-receipt-is-a-classification)
  - [`landing:a-seeded-file-is-never-rewritten` — A seeded file is never rewritten](#landinga-seeded-file-is-never-rewritten--a-seeded-file-is-never-rewritten)
  - [`landing:a-seeded-file-still-carries-the-invariants` — A seeded file still carries the invariants](#landinga-seeded-file-still-carries-the-invariants--a-seeded-file-still-carries-the-invariants)
  - [`landing:a-dropped-file-stays` — An unedited dropped file is retired](#landinga-dropped-file-stays--an-unedited-dropped-file-is-retired)
  - [`landing:a-target-is-never-downgraded` — A target is never downgraded](#landinga-target-is-never-downgraded--a-target-is-never-downgraded)
  - [`landing:status-judges-only-under-check` — Status judges only under check](#landingstatus-judges-only-under-check--status-judges-only-under-check)
  - [`landing:a-landing-classifies-its-target-first` — A landing classifies its target first](#landinga-landing-classifies-its-target-first--a-landing-classifies-its-target-first)
  - [`landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else](#landingan-adoption-writes-the-record-and-nothing-else--an-adoption-writes-the-record-and-nothing-else)
  - [`landing:a-block-destination-owns-its-marked-lines-alone` — A block destination owns its marked lines alone](#landinga-block-destination-owns-its-marked-lines-alone--a-block-destination-owns-its-marked-lines-alone)
  - [`landing:a-landed-hook-serves-the-release-convention-alone` — A landed hook serves the release convention alone](#landinga-landed-hook-serves-the-release-convention-alone--a-landed-hook-serves-the-release-convention-alone)
  - [`landing:the-landed-guards-hold-the-message-content` — The landed guards hold the message content](#landingthe-landed-guards-hold-the-message-content--the-landed-guards-hold-the-message-content)
  - [`landing:the-changelog-quality-gate-is-the-squash-message` — The changelog quality gate is the squash message](#landingthe-changelog-quality-gate-is-the-squash-message--the-changelog-quality-gate-is-the-squash-message)
  - [`landing:the-routing-block-bounds-the-agents-initiative` — The routing block bounds the agent's initiative and reads as plain prose](#landingthe-routing-block-bounds-the-agents-initiative--the-routing-block-bounds-the-agents-initiative-and-reads-as-plain-prose)
  - [`landing:a-landing-writes-what-the-catalog-selects` — A landing writes what the catalog selects](#landinga-landing-writes-what-the-catalog-selects--a-landing-writes-what-the-catalog-selects)
  - [`landing:the-nix-capability-is-a-recorded-opt-in` — An opt-in capability is a recorded landing parameter](#landingthe-nix-capability-is-a-recorded-opt-in--an-opt-in-capability-is-a-recorded-landing-parameter)
  - [`landing:the-flake-pair-lands-all-or-nothing` — The flake pair lands all-or-nothing](#landingthe-flake-pair-lands-all-or-nothing--the-flake-pair-lands-all-or-nothing)

<!--TOC-->

## Purpose

Rules governing what `rk init`, `rk status`, `rk upgrade`, and `rk adopt` owe a target repository: the receipt at `.release-kit/manifest.json`, the ownership kinds, and the comparisons each verb may make from them. The kinds are `rendered`, a whole file or marked region this binary generates and owns, `seeded`, a starting point the target tunes, and `state`, a file the target's own tooling moves. `rendered` is the kind's name, and generated is the word for what the kind means. Every landing is rendered afresh by the installed binary from its embedded sources and the target as they stand at invocation. Its subject is writing into a target and staying truthful about what was written, which is neither carrying the embedded sources, bound by `SPEC-distribution.md`, nor staging a candidate for study, bound by `SPEC-staging.md`, nor acting on a remote forge, bound by `SPEC-forge-setup.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The comparable tools these rules were checked against are in `../reference/REFERENCE-landing-sources.md`.

## Requirements

### `landing:a-landing-leaves-a-record` — A landing leaves a record

A successful `rk init --apply` MUST render every candidate afresh from this binary's embedded sources and the target as they stand at invocation, write `.release-kit/config.toml` before `.release-kit/manifest.json`, and write the receipt last, after every candidate file has landed through the temp-plus-rename writer, and a refused landing MUST leave the target unchanged, the receipt included. The receipt is committed with the landing: every reader it exists for, a clone, a CI job, an agent, sees only committed files, and it carries digests of committed files, nothing secret and nothing machine-specific.

#### Scenario: A whole-file destination holds unattributed bytes on apply

- GIVEN a target whose workflow destination already holds bytes and no receipt names it
- WHEN `rk init --apply` runs
- THEN it exits 73 naming the collision, no file lands, and no `.release-kit/` directory appears

Verify: `cargo nextest run -E 'test(fresh_init_preview_is_read_only_and_apply_writes_the_schema_8_receipt) or test(every_unattributed_collision_and_malformed_marker_is_collected_before_the_first_write)'`

### `landing:a-record-states-its-schema` — A record states its schema

The receipt MUST carry an integer `schema_version`, and a receipt at a version this binary does not know MUST refuse by that record schema alone, naming the record and independent of any other schema, never a best-effort read, because commands make decisions from it and must be able to say when they cannot, and at schema 10 the receipt MUST name the producing `rk_version`, the origin, the resolved target configuration by domain — the source-free profile snapshot, the Git workflow including the integration authority, and the capability requests — the remaining render parameters, and per destination the path, the kind, the placement where the destination is a marked region, and the digest of the bytes or region now present, and this binary MUST read a receipt at schemas 1 through 9 through one bounded conversion that ignores the two retired digest fields, reads the one recorded technology as the sole technology and the automatic release driver, moves each flat parameter into the domain that owns it, and reads a Git workflow naming no integration authority as `forge`, which is the authority such a receipt landed, and write schema 10 at the next successful landing, and a receipt that does not parse MUST refuse with the reason `record-invalid`, an action, and exit 73, because the receipt is the target's file to restore and never a defect of this binary.

#### Scenario: A schema 3 receipt and a schema 999 receipt meet this binary

- GIVEN one target whose receipt declares `schema_version: 3` with `payload_sha256` and per-file `baseline_sha256`, and another declaring `schema_version: 999`
- WHEN `rk upgrade --apply` runs against each
- THEN the first loads with no other release resolved and rewrites as schema 10 carrying neither retired field and stating its domains, and the second exits 73 naming the record and the version it found with nothing written

Verify: `cargo nextest run -E 'test(receipt_schemas_1_through_7_load_without_a_release_source_and_rewrite_as_schema_8) or test(an_upgrade_migrates_a_schema_1_record_to_the_current_schema) or test(a_receipt_newer_than_the_binary_refuses_by_record_schema) or test(production_outputs_carry_no_plan_bundle_or_release_selection_field) or test(a_broken_or_alien_record_fails_by_its_taxonomy)'`

### `landing:a-rendered-file-is-reproducible` — A rendered file is reproducible

A `rendered` file's landed bytes MUST be a deterministic function of the embedded sources and the recorded target configuration only, and every substituted value MUST be recorded in the manifest's `parameters`, so every re-render and comparison reads the manifest parameters alone, whatever a later configuration says. A value the renderer substitutes from one constant — the commit scope's shape — is not a parameter, so it MUST NOT be recorded, and a parameter an earlier schema recorded and this binary substitutes nowhere MUST read without it and rewrite without it. The two bash release pipelines MUST reproduce from `profile.release.build`, recorded only where it was answered. A parameter a snippet supplies its own fallback for MUST carry that fallback in the authored snippet inside removable markers rather than in the binary, so a record predating the parameter renders its file byte for byte and a per-forge wording stays the forge's own; the record's value MUST be held to the same grammar the configuration key is, because the record is what a re-render reads.

#### Scenario: The owner and the security contact substitute from the parameters

- GIVEN a landing run with `--repo acme/widget` and no `security.contact` or `security.scope`
- WHEN a workflow and `SECURITY.md` land
- THEN owner and full-path tokens resolve from the recorded `parameters.repo`, the policy keeps the forge's authored fallback with no marker left, and a later run naming a contact or a scope states it and records it

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-release-style-is-a-landing-parameter` — The release style is a landing parameter

Where the release mode is `automatic`, the release style MUST be recorded in the manifest as `trunk` or `lines`, reported by every parameter-bearing report, resolved as the runbooks' style axis, and taken from committed configuration or an invocation flag by the landing verbs; a record predating the field carries no style, and `rk upgrade` MUST refuse until `profile.release.style` in the config or `--release-style` answers it, because neither value is a compatibility-safe reading of a target nobody asked.

#### Scenario: A pre-style record upgrades

- GIVEN a landed target whose record predates the style parameter
- WHEN `rk upgrade --apply` runs with no `--style`
- THEN it refuses naming the parameter and the two values, nothing is written, and a rerun naming `--release-style trunk` records the answer

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-rendered-file-carries-no-judgment` — A rendered file carries no judgment

A sentinel needing operator judgment MUST NOT appear in a `rendered` file: a value a `rendered` file needs becomes a landing parameter, and a judgment stays in a `seeded` file or the committed configuration, where an edit is expected and costs nothing.

#### Scenario: A landing reports its remaining sentinels

- GIVEN a rust landing with the repository resolved
- WHEN `rk init --apply` reports the sentinels left to fill
- THEN every reported line sits in a `seeded` file or names an unanswered setup key in the config, and the landed workflow carries none

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:ownership-is-elementary` — Ownership is elementary

When `rk init --apply` or `rk upgrade --apply` lands, it MUST decide each destination by its recorded kind alone: an absent candidate destination is created, a recorded `rendered` whole file or a whole-file destination absent from the receipt that already holds the candidate's bytes is matched with nothing written, a recorded `rendered` whole file whose bytes the receipt vouches for, and a recorded `rendered` marked region the receipt vouches for or that already holds the candidate's bytes, is replaced from this binary's projection with every byte outside a region preserved, a recorded `rendered` whole file whose bytes match its `landing.accepted_drift` entry is preserved with the candidate's digest, not the bytes on disk, entering the new receipt, a recorded `seeded` or `state` file is preserved with its current digest entering the new receipt, and every conflict refuses before any write. A conflict is a whole-file destination present on disk, absent from the receipt, and differing from the candidate, or a recorded `rendered` file or region whose bytes the receipt no longer vouches for. A refusal collects every conflict and every malformed, doubled, or unmatched marker in one pass and routes to `rk stage compare` and the `rk-setup` skill. No flag forces past a refusal: only `--replace <path>`, repeatable, passes the conflict it names, after the verb saves that file's bytes to `replaced/<path>` under `$XDG_STATE_HOME/release-kit/snapshots/<repo>/<timestamp>/` and the report names where, and a name that is no conflict is a usage error. A saved copy never lives inside a stage, and the verb never overwrites one with other bytes. Where a landing replaces a file under `.github/workflows/`, `rk init`, `rk adopt`, `rk upgrade`, and `rk stage` MUST name each `${{ secrets.NAME }}` the replaced file reads and the rendered file does not, and each the rendered file reads and the replaced file did not, with the setup step that stores it where one does, and `rk stage` MUST carry the list as `secrets_moved` in its receipt; `GITHUB_TOKEN` is the forge's own and never named.

#### Scenario: An edited owned file and an unattributed file meet an upgrade

- GIVEN a landed target whose recorded workflow was edited by hand and whose `SECURITY.md` exists with no receipt entry
- WHEN `rk upgrade --apply` runs, then runs again with `--replace` naming each conflict
- THEN the first run exits 73 naming both before any write, and the second saves both files' bytes where its report names, replaces them from the projection, and preserves the tuned seeded files

Verify: `cargo nextest run -E 'test(an_upgrade_refuses_an_edited_rendered_file_until_replace_names_it) or test(seeded_and_state_files_survive_and_their_current_digests_enter_the_receipt) or test(a_marked_region_upgrades_and_the_surrounding_bytes_survive) or test(every_unattributed_collision_and_malformed_marker_is_collected_before_the_first_write) or test(a_saved_copy_is_never_overwritten_with_other_bytes) or test(a_replaced_workflow_names_the_secrets_it_read) or test(the_stage_receipt_carries_the_moved_secrets) or test(an_unchanged_upgrade_matches_every_rendered_whole_file)'`

### `landing:a-partial-landing-is-visible-and-rerunnable` — A partial landing is visible and rerunnable

While it lands, the verb MUST hold one advisory target lock from its final evidence gathering through the receipt write, created owner-only in an owner-only namespace, opened without following a link, and verified through the opened descriptor to be a regular file before truncation, MUST open the target directory once, after the lock and before any read of the target, and perform every evidence read, detection, decision, create, replace, and receipt write relative to that opened directory without following a substituted link, and MUST replace each destination through a same-directory temp-plus-rename on a supported local filesystem. The set is not transactional. A failure may leave some whole candidate files beside the previous receipt. The verb observes a reported rename failure again, because a remote filesystem may have completed it, names every completed path it observed, and leaves the diff to Git. A rerun is supported. The verb keeps no backup, rollback, or journal, and claims no atomicity across files. The opened directory is what keeps a pathname exchanged after the lock from redirecting a read or a write outside the target.

#### Scenario: A rename stops part way after a component became a symlink

- GIVEN a landing over a recorded target stopped at its third write, and a process that replaced `.github` with a symlink to a directory outside the target after validation
- WHEN the verb exits
- THEN it exits 74 naming every completed path, the previous receipt stands, each destination holds its previous or its new bytes whole, the linked directory keeps every byte, and a rerun lands the rest

Verify: `cargo nextest run -E 'test(a_failpoint_at_every_write_boundary_leaves_whole_files_and_the_previous_receipt) or test(a_reported_rename_failure_is_observed_again_before_it_is_named) or test(two_concurrent_applies_serialize_on_one_target_lock) or test(a_non_regular_lock_entry_is_refused_and_the_lock_stays_owner_only) or test(a_component_swapped_to_a_symlink_after_validation_cannot_redirect_a_write) or test(a_target_root_exchanged_after_validation_receives_nothing) or test(a_decoy_remote_at_the_exchanged_pathname_does_not_change_the_held_projection)'`

### `landing:a-missing-receipt-is-a-classification` — A missing receipt is a classification

Where a target carries no receipt, `rk init` MUST create absent candidates, record a present whole file whose bytes equal the candidate as matched, and refuse a differing unattributed file the operator did not name, `rk adopt` MUST record only a target at the installed projection once the destinations the operator named are replaced, `rk upgrade` MUST refuse naming the missing receipt, and each of them MUST read the receipt and the tree alone, routing the best-effort migration to `rk stage` and the `rk-setup` skill.

#### Scenario: A target lost its receipt and its history

- GIVEN a repository holding landed files, no `.release-kit/manifest.json`, and a shallow clone with no useful history
- WHEN `rk upgrade --apply` and `rk init --apply` run
- THEN the upgrade refuses naming the receipt, the init refuses naming every differing unattributed file and fetches nothing, and both route to the stage and the skill

Verify: `cargo nextest run -E 'test(a_missing_receipt_routes_init_adopt_and_upgrade_by_classification)'`

### `landing:a-seeded-file-is-never-rewritten` — A seeded file is never rewritten

`rk upgrade` MUST NOT rewrite a `seeded` file: the run reports a difference from the receipt as drift, and the new receipt carries the file's current digest alone, because a `seeded` file is a starting point the target is expected to tune.

#### Scenario: A tuned configuration survives an upgrade

- GIVEN a landed target whose `release-plz.toml` the operator filled
- WHEN `rk upgrade --apply` runs
- THEN the tuned bytes survive, the run reports `drift`, and the new receipt's digest for that file matches the disk

Verify: `cargo nextest run -E 'test(seeded_and_state_files_survive_and_their_current_digests_enter_the_receipt)'`

### `landing:a-seeded-file-still-carries-the-invariants` — A seeded file still carries the invariants

`rk status` MUST judge a landed file's effective configuration against the invariants its `(technology, forge, destination)` key owns, and MUST judge, per `(technology, forge)`, target-wide relationships that read a generated file the distribution ships no copy of or a target-owned root manifest landed configuration requires — for `(rust, github)`, `.github/workflows/release.yml` and the root `Cargo.toml` against `dist-workspace.toml` — reporting each failure with a stable code, the destination to change, the reason, and the exact remediation, and `--check` MUST count each one a violation. The relationship to `landing:a-seeded-file-is-never-rewritten` is deliberate: nothing is rewritten — the files stay the target's to tune — and the narrow part the invariants own is judged. The judgment over a landed configuration MUST read the parsed configuration, never match text: a commented key, a disabled value, a defaulted phase, or an unpaired phase fails, and whitespace or key order changes nothing. For `(rust, github)`, parsed `ci = "github"` or an array containing `"github"` MUST require a root `[profile.dist]` table; an absent, unreadable, or unparsable configuration or root manifest and configuration that does not select GitHub MUST report no profile failure. The judgment over a generated file MUST read that file's own text in the grammar its generator writes, because the generator is not installed and the text is what the forge executes, and MUST report a value it cannot resolve rather than pass it, since a step nobody can read is not a step nobody runs; a presentation outside that grammar is beyond a text reader, so the generator's own check stays the whole-file proof and the rule claims no more than it holds; where the generated file or the configuration it comes from is absent, the run reports nothing, because the distribution writes neither and an absence is the generator's story rather than drift. Where the pair's generated file would report a status check on a pull request, both the configuration that asks for it and the generated file that carries the trigger MUST fail, because the forge resolves a job's `needs` inside one workflow file and the trunk protection requires one project-owned context beside the title check, so a job in the generated file is in no gate's `needs` and the forge merges over its failure. For `rust` on either forge, `release-plz.toml` MUST read `publish` and `git_only` from `[workspace]` and from each `[[package]]` that sets either, a package value over the workspace one, and MUST fail `publish = false` without `git_only = true`, `git_only = true` beside `publish = true`, and a `git_only` that disagrees with the resolved `profile.release.registry`, an absent answer read as `crates-io`, because release-plz then never proposes past the registry version or refuses the pair. For `(rust, github)`, each target of `dist-workspace.toml` MUST be built before a merge, because the generated workflow starts on the tag: the run MUST report `dist-target-unbuilt` for each target no job the gate needs builds with `dist build`, the gate being the one job whose check context is `setup.required_check` with its `needs` followed in its workflow, which MUST run on `pull_request` or `merge_group`. A target counts only where a judged job with no condition names its triple, takes it from a literal matrix list, or consumes `matrix.dist_args` from the `artifacts_matrix` of a judged `dist plan --output-format=json`, narrowed to the triples a `--target` names. A check context naming no job or several MUST report `gate-unresolved`, a gate off requests `gate-not-on-requests`, and a build the reader cannot resolve `dist-build-unreadable`; an empty check context judges every job of every pull-request workflow, and it and a build under a condition or in a workflow whose request trigger carries a path filter are limitations the report states and `--check` does not count.

#### Scenario: A landed target turns attestations off and reports a second check

- GIVEN a landed rust/github target whose `dist-workspace.toml` sets `github-attestations = false` and leaves `pr-run-mode` at a value other than `skip`, beside a generated `.github/workflows/release.yml` that triggers on `pull_request`
- WHEN `rk status` and `rk status --check` run
- THEN both report each failure with its code and remediation — the disabled attestation, the run mode, and the workflow's request trigger — the plain run exits 0, the check exits 1, and neither file is touched

Verify: `cargo nextest run -E 'binary(cli) or test(the_release_plz_invariant_names_each_contradiction) or test(a_seeded_release_plz_toml_that_agrees_passes) or test(a_dist_target_no_gated_job_builds_is_reported) or test(the_binding_dist_build_job_covers_every_target) or test(a_gate_named_by_its_check_context_resolves_to_its_job) or test(a_conditional_dist_build_counts_no_target)'`

### `landing:a-dropped-file-stays` — An unedited dropped file is retired

For each destination of the receipt that this binary's projection stops producing, a landing MUST remove a whole `rendered` file whose bytes still match the receipt's digest and name it `retired`, MUST keep every other one on disk and name it `dropped`, a seeded or state file, a marked region, an edited or irregular file, and one the target's state withholds, and MUST leave both out of the rewritten receipt, and `rk stage`, `rk stage compare`, and the upgrade preview MUST report the decision the apply takes, because a file release-kit wrote and nobody changed is release-kit's to take back, while a workflow left behind keeps running beside the one that replaced it.

#### Scenario: A newer binary drops two workflows

- GIVEN a receipt naming two workflows this binary's projection no longer produces, one unedited and one edited since the landing
- WHEN `rk stage`, the preview, and `rk upgrade --apply` run
- THEN each names the first `retired` and the second `dropped`, the apply removes the first and keeps every byte of the second, and the receipt lists neither

Verify: `cargo nextest run -E 'test(a_dropped_unedited_destination_is_retired_on_upgrade) or test(a_dropped_edited_destination_stays_on_upgrade) or test(a_seeded_or_state_dropped_destination_is_never_removed) or test(stage_compare_preview_and_apply_agree_on_each_dropped_case) or test(a_destination_retired_by_the_installed_version_leaves_the_disk_and_the_receipt) or test(a_dropped_destination_is_retired_unless_the_target_edited_it) or test(the_stage_receipt_and_human_output_snapshot_hold)'`

### `landing:a-target-is-never-downgraded` — A target is never downgraded

Where the record names an `rk_version` newer than this binary's, `rk upgrade` MUST refuse and name the version to install, because rewriting a newer landing with older bytes is not an upgrade.

#### Scenario: An old binary meets a new landing

- GIVEN a record whose `rk_version` is above this binary's version
- WHEN `rk upgrade --apply` runs
- THEN it exits 73 telling the operator to install the matching release, and nothing is written

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:status-judges-only-under-check` — Status judges only under check

Plain `rk status` MUST report and exit 0 for every reportable state — drift, staleness, unresolved sentinels, invariant failures, a pending projection, and no landing at all — and `rk status --check` MUST compute the identical report and exit 1 exactly on a violation: drift to a `rendered` file that no `landing.accepted_drift` entry matches, a record whose own parameters do not reproduce its recorded bytes or its recorded destination set, an invalid or missing landing, an unresolved judgment sentinel, an invariant failure under `landing:a-seeded-file-still-carries-the-invariants`, or a setup proof code under `setup-proof:the-status-check-holds-the-proof`. Seeded drift, pin staleness, a pending projection, and committed configuration the landing verbs have yet to take up stay informational in both modes. A pending projection is what the report MUST route an upgrade from, counted as the destinations this binary's projection under the recorded parameters would add, drop, reclassify, or rewrite; the recorded `rk_version` names the binary that wrote the record and MUST prompt nothing on its own, because a release that changes no landed file leaves the target with nothing to take and the two facts answer different questions. In both modes the report MUST also state whether the target is a non-bare Git work tree, each unresolved sentinel by file and line beside the count, and each path a 0.4.x release left under the state root with whether it holds an operation that release could still apply, so a reader stops on a field and never searches the disk by hand.

#### Scenario: The same target, judged and not

- GIVEN a landed target recorded by an older `rk` whose destinations this binary projects byte for byte, with a tuned seeded file and an edited rendered file
- WHEN `rk status` and `rk status --check` run
- THEN both print the same report, the plain run exits 0, the check exits 1 naming the rendered drift in its violations, and neither counts a pending destination nor routes to an upgrade the version gap alone does not earn

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-landing-classifies-its-target-first` — A landing classifies its target first

`rk status --json` MUST report exactly one `classification` by one rule, writing nothing, touching no network, and exiting 0 on every value: with no record, `adopt` where a release-kit marked region or a rendered file holding this binary's bytes is present, else `migration` where another tool's release marker or a landed destination is present, `needs-decision` where tags or a second long-lived branch exist with no mechanism behind them, and `setup` where none of these holds; with a record, `upgrade` where this binary is newer than the record, `drift` where `rk status --check` fails, `current` where it passes, and null where the record is newer than this binary. An open migration reads `migration` before every other value, with or without a record, except a record newer than this binary, which still reads null. `rk assess` MUST apply the same rule to a target with no record and report its evidence with the verdicts `greenfield`, `brownfield`, and `needs-decision`, because a target already releasing somehow that reads as a fresh start is how a repository ends up with two release paths.

#### Scenario: A landing lost its record and gained a changelog

- GIVEN a repository holding release-kit's marked regions, a `CHANGELOG.md`, and no `.release-kit/manifest.json`
- WHEN `rk status --target . --json` runs
- THEN it classifies `adopt` and exits 0, because the release-kit regions outrank the changelog that alone would read as another tool's

Verify: `cargo nextest run -E 'test(/^assess_/) or test(status_reports_each_of_the_seven_classifications)'`

### `landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else

`rk adopt` MUST verify every `rendered` destination byte for byte against this binary's projection, refuse listing every mismatch and every missing expected file the operator did not name with `--replace` in one run, save each named file's bytes before it writes the candidate there, and end a successful pass with the config and then the receipt, last, inside `.release-kit/`, with its origin stating the adoption, leaving every destination the operator did not name untouched.

#### Scenario: A target an older release landed is adopted

- GIVEN a repository running the convention with no record, one workflow holding an older release's bytes, and a stage taken for it
- WHEN `rk adopt --stage <root> --apply` runs, then runs again with `--replace` naming the workflow
- THEN the first run refuses naming the workflow, and the second saves its bytes, writes the candidate there, writes the manifest with `origin` set to `adopt`, and leaves every other destination unchanged

Verify: `cargo nextest run -E 'test(an_adoption_replaces_only_the_named_file_and_the_copy_outlives_the_stage) or test(a_missing_receipt_routes_init_adopt_and_upgrade_by_classification)'`

### `landing:a-block-destination-owns-its-marked-lines-alone` — A block destination owns its marked lines alone

A block-placed artifact MUST own exactly the lines between its markers: a landing splices the block into the target's document — fresh where none exists, in place where marked, under the owning key otherwise — and MUST refuse by name a document that offers the block no place, leaving the target unchanged, because rewriting a document the target owns is not a landing. The glossary at the target root is such a destination, and its surrounding document is the target's own vocabulary rather than one release-kit found: a landing creates the file where none exists, appends the block where the file exists unmarked, replaces the block in place where it is marked, and writes outside the markers never.

#### Scenario: A hooks file with no repos list

- GIVEN a target whose `.pre-commit-config.yaml` exists carrying no `repos:` line
- WHEN `rk init --apply` runs
- THEN it exits 73 naming the file, no file lands, and no `.release-kit/` directory appears

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-landed-hook-serves-the-release-convention-alone` — A landed hook serves the release convention alone

The hook block MUST carry the hooks the release process needs, the status check and, wherever `profile.release.mode` is not `none`, the tag guard, and only under the workflow guards the commit contract, the local mirrors of the forge protections, and the trunk guards of local integration, never general hygiene, which stays the target's own, and every third-party hook it names MUST be pinned in `versions.toml`.

#### Scenario: The block's hooks are enumerated

- GIVEN a landed `.pre-commit-config.yaml` block
- WHEN its hooks are read beside `rk versions`
- THEN each hook maps to a rule the method states, the workflow hooks appear only where the record selects the workflow guards, the tag guard appears only where the record names a release, and each third-party repository the block names carries a registry pin

Verify: `cargo nextest run -E 'binary(cli) or test(workflow_only)'`

### `landing:the-landed-guards-hold-the-message-content` — The landed guards hold the message content

Where the target selects the workflow guards, the landed commit-msg hook MUST refuse a message referencing a git-ignored path, carrying agent attribution, or naming a scope outside the one shape the landed title check admits, exempting only the release bot's request, recognized by exactly the title shape the landed title check admits for the bot. The scope's shape is a gate and its vocabulary is not: no landing records a scope list, the landed hooks require that a scope is present wherever the record names a release, as `project-profile:a-release-less-landing-carries-no-release-text` binds, and the routing block carries the guidance an author reads to choose the word. Where the forge carries the request's description onto the trunk, the landed required check MUST refuse a body matching the same patterns — the bot's request exempt whole, since the check has no ignore rules to consult — and the two pattern copies MUST be held equal by test.

#### Scenario: A message references an internal planning artifact

- GIVEN a commit whose body names a path the repository git-ignores, and a second whose subject scope reads `Specs Ugly` where `record/ids` would have passed
- WHEN the landed rk-message hook judges each message
- THEN each is refused, the first naming the line and the ignored path before it becomes the trunk's permanent record, the second naming the scope and the shape it must take

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-changelog-quality-gate-is-the-squash-message` — The changelog quality gate is the squash message

The changelog's quality MUST be held at the point the entry is generated from — under `squash` the squash title the landed title check holds to the scoped convention and the body the landed content guard judges, and under `rebase` each commit message the landed commit gate judges — because the bot regenerates the release request on every trunk push and drops a hand edit on its branch. A hand correction on the request branch MUST be the last step before the operator orders the merge.

#### Scenario: An entry reads badly on the open request

- GIVEN a trunk-style project whose open release request carries an entry that misstates a change
- WHEN the operator corrects it
- THEN the correction is a commit on the request branch pushed as the last step before the merge, because any trunk push before the merge regenerates the request without it

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-routing-block-bounds-the-agents-initiative` — The routing block bounds the agent's initiative and reads as plain prose

The routing block MUST carry no line that tells the project how to start work, integrate a branch, or lay out its working copies, in any capability state, because that workflow is the project's own, and where the target selects the workflow guards, the block MUST state that an agent acting in the target guides and never drives: that a request to change code authorizes the file changes alone, and that a git or forge action — creating, switching or deleting a branch, creating or removing a worktree, committing, pushing, tagging, and opening, updating or merging a pull request among them — happens only where the operator's request named that action. It is the one landed line no mechanism enforces — a hook and a forge protection bound the end state and cannot tell an agent from a person — so the target carries it as a sentence rather than leaving an agent to discover it by refusal. Every line of the block MUST also read as plain prose — a simple tense, no contraction, no modal outside can, will and must, no semicolon, no dash splicing two statements, and no sentence over 25 words — because the target owns none of these lines, so a prose gate the target runs over its own `AGENTS.md` finds a defect it cannot repair and the renderer is the only place that can answer it. Under the workflow guards, the block MUST name the glossary destination and list the terms release-kit owns there, because a separate file loads into no session on its own and a term the agent cannot see is a term the operator cannot use. Under `profile.release.mode = "none"` the block MUST take its workflow variant, with no `## Releases` heading and no line that gives a version, a changelog, or a tag as a reason, and the glossary MUST carry no release term.

#### Scenario: The block is read for what it authorizes

- GIVEN the routing block release-kit renders into a target's `AGENTS.md`
- WHEN the test suite reads it
- THEN the guarded block carries the line bounding the agent's initiative, no block carries a line ordering an agent to branch, commit, or merge on its own or naming a workflow helper, no line a prose gate would report, and the workflow variant names no version, changelog, or tag

Verify: `cargo nextest run -E 'binary(cli) or test(workflow_only)'`

### `landing:a-landing-writes-what-the-catalog-selects` — A landing writes what the catalog selects

Every destination a landing writes MUST belong to exactly one capability the catalog selected for the target's resolved configuration, every capability MUST be reported with one of `selected`, `not-requested`, `not-applicable`, `unavailable`, `unknown`, and `withheld`, and a selected release automation this release does not carry at the target's driver and forge MUST refuse the apply before any write, naming the dimensions and the tuples it does carry, because a record stating an automation nothing landed is a false receipt.

#### Scenario: A profile asks for a release this binary cannot land

- GIVEN a GitLab target whose release is automatic and driven by `go`, which no binding carries
- WHEN `rk init` previews and then applies
- THEN the preview reports `release.automation` unknown with the bindings this release carries, the apply exits 73 before any write, and no `.release-kit/` directory appears

Verify: `cargo nextest run -E 'test(an_unknown_driver_reports_its_automation_and_apply_refuses) or test(python_on_gitlab_lands_and_reads_clean) or test(an_occupied_gitlab_root_pipeline_withholds_the_title_gate) or test(a_target_with_no_technology_and_no_forge_lands_the_guards)'`

### `landing:the-nix-capability-is-a-recorded-opt-in` — An opt-in capability is a recorded landing parameter

A landing MUST include an opt-in capability's destinations only under an explicit request recorded under `capabilities`, defaulting off, with a record predating the parameter reading as opt-out, except the workflow guards, because the projection must stay reproducible from the record: without the parameter, `status` cannot tell an absent-because-not-wanted file from a drifted one, and `upgrade` cannot decide whether to add the files. Four capabilities are bound by this rule: the Nix destinations, the OpenSSF Scorecard workflow, which posts to a public API and reads repository metadata, the code scanning workflow, whose parameter names the analyzer, and the workflow guards. Each stays a target's choice rather than a guard this method requires. The workflow guards are the one capability whose old record does not read as opt-out: a record predating the parameter carries a pending answer that reads as kept, `rk upgrade` MUST report it once with its default and keep the guards unless the operator answers off, and the rewritten record MUST state the answer, because every target that landed before the capability carries the guards and no record is stripped of them without a question. A capability whose files are technology-independent and which one forge alone can run MUST ship them in that forge's shared zone, so the parameter records the target's answer on either forge and the projection lands nothing the forge cannot execute; a capability whose files read one language MUST ship them in that binding's own pair instead, and a landing for a binding that ships none MUST report the capability unavailable by name and omit its destinations, per [the project profile specification](./SPEC-project-profile.md). A capability whose parameter names a provider MUST land exactly the destination that provider owns, and where a provider the target's dimensions can land carries terms that bind the codebase it runs over, the landing MUST read the licence the binding declares and refuse the pair by name, writing nothing and naming a provider that carries no such condition, because a workflow whose terms the codebase does not satisfy is a licence violation this convention does not commit on a target's behalf; a licence that lapses after the landing MUST report as a warning under a stable code that `rk status --check` exits 0 on, because the target is not broken and the licensing decision is the operator's.

#### Scenario: An old record meets a newer binary

- GIVEN a landed target whose record predates the Nix, Scorecard, code scanning, and workflow guards parameters
- WHEN `rk upgrade` runs
- THEN no destination of the first three joins the landing, the workflow guards stay with their pending answer reported, and the rewritten record states each answer explicitly

Verify: `cargo nextest run -E 'test(a_pre_nix_record_upgrades_to_nothing_unrequested) or test(/scorecard/) or test(an_upgrade_keeps_the_guards_of_a_record_that_predates_them)'`

### `landing:the-flake-pair-lands-all-or-nothing` — The flake pair lands all-or-nothing

A landing MUST land the seed `flake.nix` and its matching `flake.lock` as a pair only where the target carries neither, withholding the pair with the reason named where either exists, because a seed lock beside a foreign flake describes the wrong input graph; the seeded package expression still lands, and a crate shape the seed does not support withholds the whole capability by name. The capability MUST land no CI file, because a job proving the build holds a merge only where the forge's own gate reaches it — inside the workflow the required check needs on GitHub, and inside the child pipeline the rendered parent triggers on GitLab, since that forge deep-merges an include and only a separate configuration isolates — and both of those files are the target's own. The seeded flake and the landed release declaration name the one system this repository dogfoods, `x86_64-linux` and `x86_64-unknown-linux-gnu`, bound by `packaging:an-advertised-system-is-a-proven-system`.

#### Scenario: A target with its own flake opts in

- GIVEN a rust target carrying a `flake.nix` of its own
- WHEN `rk init --nix-packaging --apply` runs
- THEN `nix/package.nix` lands, the pair is withheld with the reason reported, no workflow is written, the withheld destinations stay out of the record, and a later `rk upgrade` reproduces the same decision

Verify: `cargo nextest run -E 'test(a_target_with_its_own_flake_keeps_it_and_the_pair_is_withheld)'`
