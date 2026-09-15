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
  - [`landing:a-dropped-file-stays` — A dropped file stays](#landinga-dropped-file-stays--a-dropped-file-stays)
  - [`landing:a-target-is-never-downgraded` — A target is never downgraded](#landinga-target-is-never-downgraded--a-target-is-never-downgraded)
  - [`landing:status-judges-only-under-check` — Status judges only under check](#landingstatus-judges-only-under-check--status-judges-only-under-check)
  - [`landing:a-landing-classifies-its-target-first` — A landing classifies its target first](#landinga-landing-classifies-its-target-first--a-landing-classifies-its-target-first)
  - [`landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else](#landingan-adoption-writes-the-record-and-nothing-else--an-adoption-writes-the-record-and-nothing-else)
  - [`landing:a-block-destination-owns-its-marked-lines-alone` — A block destination owns its marked lines alone](#landinga-block-destination-owns-its-marked-lines-alone--a-block-destination-owns-its-marked-lines-alone)
  - [`landing:a-landed-hook-serves-the-release-convention-alone` — A landed hook serves the release convention alone](#landinga-landed-hook-serves-the-release-convention-alone--a-landed-hook-serves-the-release-convention-alone)
  - [`landing:the-landed-guards-hold-the-message-content` — The landed guards hold the message content](#landingthe-landed-guards-hold-the-message-content--the-landed-guards-hold-the-message-content)
  - [`landing:the-arming-identity-is-the-bot` — The arming identity is the bot](#landingthe-arming-identity-is-the-bot--the-arming-identity-is-the-bot)
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

The receipt MUST carry an integer `schema_version`, and a receipt at a version this binary does not know MUST refuse by that record schema alone, naming the record and independent of any other schema, never a best-effort read, because commands make decisions from it and must be able to say when they cannot, and at schema 10 the receipt MUST name the producing `rk_version`, the origin, the resolved target configuration by domain — the source-free profile snapshot, the Git workflow including the integration authority, and the capability requests — the remaining render parameters, and per destination the path, the kind, the placement where the destination is a marked region, and the digest of the bytes or region now present, and this binary MUST read a receipt at schemas 1 through 9 through one bounded conversion that ignores the two retired digest fields, reads the one recorded technology as the sole technology and the automatic release driver, moves each flat parameter into the domain that owns it, and reads a Git workflow naming no integration authority as `forge`, which is the authority such a receipt landed, and write schema 10 at the next successful landing.

#### Scenario: A schema 3 receipt and a schema 999 receipt meet this binary

- GIVEN one target whose receipt declares `schema_version: 3` with `payload_sha256` and per-file `baseline_sha256`, and another declaring `schema_version: 999`
- WHEN `rk upgrade --apply` runs against each
- THEN the first loads with no other release resolved and rewrites as schema 10 carrying neither retired field and stating its domains, and the second exits 73 naming the record and the version it found with nothing written

Verify: `cargo nextest run -E 'test(receipt_schemas_1_through_7_load_without_a_release_source_and_rewrite_as_schema_8) or test(an_upgrade_migrates_a_schema_1_record_to_the_current_schema) or test(a_receipt_newer_than_the_binary_refuses_by_record_schema) or test(production_outputs_carry_no_plan_bundle_or_release_selection_field)'`

### `landing:a-rendered-file-is-reproducible` — A rendered file is reproducible

A `rendered` file's landed bytes MUST be a deterministic function of the embedded sources and the recorded target configuration only, and every substituted value MUST be recorded in the manifest's `parameters`, so every re-render and comparison reads the manifest parameters alone, whatever a later configuration says. A value the renderer substitutes from one constant — the commit scope's shape — is not a parameter, so it MUST NOT be recorded, and a parameter an earlier schema recorded and this binary substitutes nowhere MUST read without it and rewrite without it. A parameter a snippet supplies its own fallback for MUST carry that fallback in the authored snippet inside removable markers rather than in the binary, so a record predating the parameter renders its file byte for byte and a per-forge wording stays the forge's own; the record's value MUST be held to the same grammar the configuration key is, because the record is what a re-render reads.

#### Scenario: The owner and the security contact substitute from the parameters

- GIVEN a landing run with `--repo acme/widget` and no `security.contact`
- WHEN a workflow and `SECURITY.md` land
- THEN owner and full-path tokens resolve from the recorded `parameters.repo`, the policy keeps the forge's authored fallback with no marker left, and a later run naming a contact states it and records it

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-release-style-is-a-landing-parameter` — The release style is a landing parameter

Where the release mode is `automatic`, the release style MUST be recorded in the manifest as `trunk` or `lines`, reported by every parameter-bearing report, rendered into the landed release workflow as the one value that arms or does not arm the bot's request, resolved as the runbooks' style axis, and taken from committed configuration or an invocation flag by the landing verbs; a record predating the field carries no style, and `rk upgrade` MUST refuse until `profile.release.style` in the config or `--release-style` answers it, because neither value is a compatibility-safe reading of a target nobody asked.

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

When `rk init --apply` or `rk upgrade --apply` lands, it MUST decide each destination by its recorded kind alone: an absent candidate destination is created, a destination the receipt records as a `rendered` whole file is replaced from this binary's projection even where its bytes changed, a recorded `seeded` or `state` file is preserved with its current digest entering the new receipt, a recorded marked region is replaced alone with every byte outside it preserved, a whole-file destination absent from the receipt and already holding the candidate's bytes is recorded as matched, and a whole-file destination present on disk, absent from the receipt, and differing from the candidate refuses before any write. A recorded file is replaced because the operator and the agent authorized the migration and Git holds the recovery. A refusal collects every unattributed collision and every malformed, doubled, or unmatched marker in one pass and routes to `rk stage` and the `rk-setup` skill. The verbs offer no force flag: an unattributed file becomes landable through the agent's migration alone, which brings the target to the projection or records it through `rk adopt`.

#### Scenario: An edited owned file and an unattributed file meet an upgrade

- GIVEN a landed target whose recorded workflow was edited by hand and whose `SECURITY.md` exists with no receipt entry
- WHEN `rk upgrade --apply` runs
- THEN it exits 73 naming `SECURITY.md` before any write, and once the agent records or removes that file a rerun replaces the workflow from the projection and preserves the tuned seeded files

Verify: `cargo nextest run -E 'test(an_upgrade_replaces_a_recorded_generated_file_whose_bytes_differ) or test(seeded_and_state_files_survive_and_their_current_digests_enter_the_receipt) or test(a_marked_region_upgrades_and_the_surrounding_bytes_survive) or test(every_unattributed_collision_and_malformed_marker_is_collected_before_the_first_write)'`

### `landing:a-partial-landing-is-visible-and-rerunnable` — A partial landing is visible and rerunnable

While it lands, the verb MUST hold one advisory target lock from its final evidence gathering through the receipt write, created owner-only in an owner-only namespace, opened without following a link, and verified through the opened descriptor to be a regular file before truncation, MUST open the target directory once, after the lock and before any read of the target, and perform every evidence read, detection, decision, create, replace, and receipt write relative to that opened directory without following a substituted link, and MUST replace each destination through a same-directory temp-plus-rename on a supported local filesystem. The set is not transactional. A failure may leave some whole candidate files beside the previous receipt. The verb observes a reported rename failure again, because a remote filesystem may have completed it, names every completed path it observed, and leaves the diff to Git. A rerun is supported. The verb keeps no backup, rollback, or journal, and claims no atomicity across files. The opened directory is what keeps a pathname exchanged after the lock from redirecting a read or a write outside the target.

#### Scenario: A rename stops part way after a component became a symlink

- GIVEN a landing over a recorded target stopped at its third write, and a process that replaced `.github` with a symlink to a directory outside the target after validation
- WHEN the verb exits
- THEN it exits 74 naming every completed path, the previous receipt stands, each destination holds its previous or its new bytes whole, the linked directory keeps every byte, and a rerun lands the rest

Verify: `cargo nextest run -E 'test(a_failpoint_at_every_write_boundary_leaves_whole_files_and_the_previous_receipt) or test(a_reported_rename_failure_is_observed_again_before_it_is_named) or test(two_concurrent_applies_serialize_on_one_target_lock) or test(a_non_regular_lock_entry_is_refused_and_the_lock_stays_owner_only) or test(a_component_swapped_to_a_symlink_after_validation_cannot_redirect_a_write) or test(a_target_root_exchanged_after_validation_receives_nothing) or test(a_decoy_remote_at_the_exchanged_pathname_does_not_change_the_held_projection)'`

### `landing:a-missing-receipt-is-a-classification` — A missing receipt is a classification

Where a target carries no receipt, `rk init` MUST create absent candidates, record a present whole file whose bytes equal the candidate as matched, and refuse a differing unattributed file, `rk adopt` MUST record only a target the agent already brought to the installed projection, `rk upgrade` MUST refuse naming the missing receipt, and each of them MUST read the receipt and the tree alone, routing the best-effort migration to `rk stage` and the `rk-setup` skill.

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

`rk status` MUST judge a landed file's effective configuration against the invariants its `(technology, forge, destination)` key owns, and MUST judge, per `(technology, forge)`, target-wide relationships that read a generated file the distribution ships no copy of or a target-owned root manifest landed configuration requires — for `(rust, github)`, `.github/workflows/release.yml` and the root `Cargo.toml` against `dist-workspace.toml` — reporting each failure with a stable code, the destination to change, the reason, and the exact remediation, and `--check` MUST count each one a violation. The relationship to `landing:a-seeded-file-is-never-rewritten` is deliberate: nothing is rewritten — the files stay the target's to tune — and the narrow part the invariants own is judged. The judgment over a landed configuration MUST read the parsed configuration, never match text: a commented key, a disabled value, a defaulted phase, or an unpaired phase fails, and whitespace or key order changes nothing. For `(rust, github)`, parsed `ci = "github"` or an array containing `"github"` MUST require a root `[profile.dist]` table; an absent, unreadable, or unparsable configuration or root manifest and configuration that does not select GitHub MUST report no profile failure. The judgment over a generated file MUST read that file's own text in the grammar its generator writes, because the generator is not installed and the text is what the forge executes, and MUST report a value it cannot resolve rather than pass it, since a step nobody can read is not a step nobody runs; a presentation outside that grammar is beyond a text reader, so the generator's own check stays the whole-file proof and the rule claims no more than it holds; where the generated file or the configuration it comes from is absent, the run reports nothing, because the distribution writes neither and an absence is the generator's story rather than drift. Where the pair's generated file would report a status check on a pull request, both the configuration that asks for it and the generated file that carries the trigger MUST fail, because the forge resolves a job's `needs` inside one workflow file and the trunk protection requires one project-owned context beside the title check, so a job in the generated file is in no gate's `needs` and the forge merges over its failure.

#### Scenario: A landed target turns attestations off and reports a second check

- GIVEN a landed rust/github target whose `dist-workspace.toml` sets `github-attestations = false` and leaves `pr-run-mode` at a value other than `skip`, beside a generated `.github/workflows/release.yml` that triggers on `pull_request`
- WHEN `rk status` and `rk status --check` run
- THEN both report each failure with its code and remediation — the disabled attestation, the run mode, and the workflow's request trigger — the plain run exits 0, the check exits 1, and neither file is touched

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-dropped-file-stays` — A dropped file stays

A destination this binary's projection stops producing MUST be left in place, named in the upgrade's output and in the stage receipt, and left out of the rewritten receipt, because a file release-kit stops shipping is a file the target owns from that moment.

#### Scenario: A newer binary drops a workflow

- GIVEN a receipt naming a destination this binary's projection no longer produces
- WHEN `rk upgrade --apply` runs
- THEN the file survives on disk, the output names it dropped, and the receipt no longer lists it

Verify: `cargo nextest run -E 'test(a_destination_retired_by_the_installed_version_stays_on_disk_and_leaves_the_receipt) or test(the_stage_receipt_and_human_output_snapshot_hold)'`

### `landing:a-target-is-never-downgraded` — A target is never downgraded

Where the record names an `rk_version` newer than this binary's, `rk upgrade` MUST refuse and name the version to install, because rewriting a newer landing with older bytes is not an upgrade.

#### Scenario: An old binary meets a new landing

- GIVEN a record whose `rk_version` is above this binary's version
- WHEN `rk upgrade --apply` runs
- THEN it exits 73 telling the operator to install the matching release, and nothing is written

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:status-judges-only-under-check` — Status judges only under check

Plain `rk status` MUST report and exit 0 for every reportable state — drift, staleness, unresolved sentinels, invariant failures, a pending projection, and no landing at all — and `rk status --check` MUST compute the identical report and exit 1 exactly on a violation: drift to a `rendered` file, a record whose own parameters do not reproduce its recorded bytes or its recorded destination set, an invalid or missing landing, an unresolved judgment sentinel, or an invariant failure under `landing:a-seeded-file-still-carries-the-invariants`. Seeded drift, pin staleness, a pending projection, and committed configuration the landing verbs have yet to take up stay informational in both modes. A pending projection is what the report MUST route an upgrade from, counted as the destinations this binary's projection under the recorded parameters would add, drop, reclassify, or rewrite; the recorded `rk_version` names the binary that wrote the record and MUST prompt nothing on its own, because a release that changes no landed file leaves the target with nothing to take and the two facts answer different questions.

#### Scenario: The same target, judged and not

- GIVEN a landed target recorded by an older `rk` whose destinations this binary projects byte for byte, with a tuned seeded file and an edited rendered file
- WHEN `rk status` and `rk status --check` run
- THEN both print the same report, the plain run exits 0, the check exits 1 naming the rendered drift in its violations, and neither counts a pending destination nor routes to an upgrade the version gap alone does not earn

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-landing-classifies-its-target-first` — A landing classifies its target first

Where a setup or migration task finds no landing record at a target, the skills and the shared pre-flight gate MUST route by `rk assess`, which MUST report its evidence and exactly one verdict — `brownfield` where another tool's release marker or a landed destination is already present, `greenfield` where no release mechanism and no release history exists, and `needs-decision` where tags or a second long-lived branch exist with no mechanism behind them — writing nothing, touching no network, and exiting 0 on every verdict, because a target already releasing somehow that reads as a fresh start is how a repository ends up with two release paths.

#### Scenario: A target releasing through another tool carries no record

- GIVEN a repository holding a release tool's configuration and no `.release-kit/manifest.json`
- WHEN `rk assess --target . --json` runs
- THEN the report classifies `brownfield`, names the marker, and exits 0, so the routing skill loads the migration procedure instead of landing the convention beside the tool

Verify: `cargo nextest run -E 'test(/^assess_/)'`

### `landing:an-adoption-writes-the-record-and-nothing-else` — An adoption writes the record and nothing else

`rk adopt` MUST verify every `rendered` destination byte for byte against this binary's projection, refuse listing every mismatch and every missing expected file in one run, and end a successful pass by writing only inside `.release-kit/`, the config and then the receipt, last, with its origin stating the adoption, leaving every landed destination untouched.

#### Scenario: A pre-record target is adopted

- GIVEN a repository running the convention with no record, matching what this binary renders
- WHEN `rk adopt --apply` runs
- THEN the manifest appears with `origin` set to `adopt`, the config appears beside it, every landed destination stays unchanged, and `rk status` then reports the landing

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-block-destination-owns-its-marked-lines-alone` — A block destination owns its marked lines alone

A block-placed artifact MUST own exactly the lines between its markers: a landing splices the block into the target's document — fresh where none exists, in place where marked, under the owning key otherwise — and MUST refuse by name a document that offers the block no place, leaving the target unchanged, because rewriting a document the target owns is not a landing. The glossary at the target root is such a destination, and its surrounding document is the target's own vocabulary rather than one release-kit found: a landing creates the file where none exists, appends the block where the file exists unmarked, replaces the block in place where it is marked, and writes outside the markers never.

#### Scenario: A hooks file with no repos list

- GIVEN a target whose `.pre-commit-config.yaml` exists carrying no `repos:` line
- WHEN `rk init --apply` runs
- THEN it exits 73 naming the file, no file lands, and no `.release-kit/` directory appears

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-landed-hook-serves-the-release-convention-alone` — A landed hook serves the release convention alone

The hook block MUST carry only hooks enforcing the release convention's own rules — the commit contract and the local mirrors of the forge protections — never general hygiene, which stays the target's own, and every third-party hook it names MUST be pinned in `versions.toml`.

#### Scenario: The block's hooks are enumerated

- GIVEN a landed `.pre-commit-config.yaml` block
- WHEN its hooks are read beside `rk versions`
- THEN each hook maps to a rule the method states, and each third-party repository the block names carries a registry pin

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-landed-guards-hold-the-message-content` — The landed guards hold the message content

The landed commit-msg hook MUST refuse a message referencing a git-ignored path, carrying agent attribution, or naming a scope outside the one shape the landed title check admits, exempting only the release bot's request, recognized by exactly the title shape the landed title check admits for the bot. The scope's shape is a gate and its vocabulary is not: no landing records a scope list, the landed hooks require that a scope is present, and the routing block carries the guidance an author reads to choose the word. Where the forge carries the request's description onto the trunk, the landed required check MUST refuse a body matching the same patterns — the bot's request exempt whole, since the check has no ignore rules to consult — and the two pattern copies MUST be held equal by test.

#### Scenario: A message references an internal planning artifact

- GIVEN a commit whose body names a path the repository git-ignores, and a second whose subject scope reads `Specs Ugly` where `record/ids` would have passed
- WHEN the landed rk-message hook judges each message
- THEN each is refused, the first naming the line and the ignored path before it becomes the trunk's permanent record, the second naming the scope and the shape it must take

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-arming-identity-is-the-bot` — The arming identity is the bot

Where the recorded style arms the release request, the landed workflow MUST arm it with the bot identity's token, MUST NOT arm it with the forge's default CI token, and MUST re-arm on every refresh of the request, because a merge made under the default token starts no workflow — leaving the bump merged, untagged, and unpublished with nothing reporting a failure — and because a forge that refreshes by replacing the request drops the arming with the request it replaces.

#### Scenario: The arm is made with the default CI token

- GIVEN a release request armed by a job authenticating as the forge's default CI token
- WHEN every required check passes and the forge merges
- THEN the bump lands, no workflow run starts, no tag and no publish follow, and nothing reports it — which arming under the bot identity is what prevents

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-changelog-quality-gate-is-the-squash-message` — The changelog quality gate is the squash message

Where the recorded style arms the release request, the changelog's quality MUST be held at the point the entry is generated from — the squash title the landed title check holds to the scoped convention and the body the landed content guard judges — because an armed request offers no window in which a human edit on its branch survives to the merge.

#### Scenario: An entry reads badly on an armed request

- GIVEN an armed trunk-style project whose generated entry misstates a change
- WHEN the operator looks for the correction window
- THEN there is none while the request stands armed: the correction is a disarm before the checks finish, or a changelog commit landed on the trunk that ships with the next release

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:the-routing-block-bounds-the-agents-initiative` — The routing block bounds the agent's initiative and reads as plain prose

The routing block MUST state that an agent acting in the target guides and never drives: that a request to change code authorizes the file changes alone, and that a git or forge action — creating, switching or deleting a branch, creating or removing a worktree, committing, pushing, tagging, and opening, updating or merging a pull request among them — happens only where the operator's request named that action. It is the one landed line no mechanism enforces — a hook and a forge protection bound the end state and cannot tell an agent from a person — so the target carries it as a sentence rather than leaving an agent to discover it by refusal. Every line of the block MUST also read as plain prose — a simple tense, no contraction, no modal outside can, will and must, no semicolon, no dash splicing two statements, and no sentence over 25 words — because the target owns none of these lines, so a prose gate the target runs over its own `AGENTS.md` finds a defect it cannot repair and the renderer is the only place that can answer it. The block MUST name the glossary destination and list the terms release-kit owns there, because a separate file loads into no session on its own and a term the agent cannot see is a term the operator cannot use.

#### Scenario: The block is read for what it authorizes

- GIVEN the routing block release-kit renders into a target's `AGENTS.md`
- WHEN the test suite reads it
- THEN it carries the line bounding the agent's initiative, no line ordering an agent to branch, commit, or merge on its own, and no line a prose gate would report

Verify: `cargo nextest run -E 'binary(cli)'`

### `landing:a-landing-writes-what-the-catalog-selects` — A landing writes what the catalog selects

Every destination a landing writes MUST belong to exactly one capability the catalog selected for the target's resolved configuration, every capability MUST be reported with one of `selected`, `not-requested`, `not-applicable`, `unavailable`, `unknown`, and `withheld`, and a selected release automation this release does not carry at the target's driver and forge MUST refuse the apply before any write, naming the dimensions and the tuples it does carry, because a record stating an automation nothing landed is a false receipt.

#### Scenario: A profile asks for a release this binary cannot land

- GIVEN a GitLab target whose release is automatic and driven by `python`
- WHEN `rk init` previews and then applies
- THEN the preview reports `release.automation` unavailable with the available tuples, the apply exits 73 before any write, and no `.release-kit/` directory appears

Verify: `cargo nextest run -E 'test(python_on_gitlab_reports_unavailable_and_apply_refuses) or test(an_occupied_gitlab_root_pipeline_withholds_the_title_gate) or test(a_target_with_no_technology_and_no_forge_lands_the_guards)'`

### `landing:the-nix-capability-is-a-recorded-opt-in` — An opt-in capability is a recorded landing parameter

A landing MUST include an opt-in capability's destinations only under an explicit request recorded under `capabilities`, defaulting off, with a record predating the parameter reading as opt-out, because the projection must stay reproducible from the record: without the parameter, `status` cannot tell an absent-because-not-wanted file from a drifted one, and `upgrade` cannot decide whether to add the files. Three capabilities are bound by this rule: the Nix destinations, the OpenSSF Scorecard workflow, which posts to a public API and reads repository metadata, and the code scanning workflow, whose parameter names the analyzer. Each stays a target's choice rather than a guard this method requires. A capability whose files are technology-independent and which one forge alone can run MUST ship them in that forge's shared zone, so the parameter records the target's answer on either forge and the projection lands nothing the forge cannot execute; a capability whose files read one language MUST ship them in that binding's own pair instead, and a landing for a binding that ships none MUST report the capability unavailable by name and omit its destinations, per [the project profile specification](./SPEC-project-profile.md). A capability whose parameter names a provider MUST land exactly the destination that provider owns, and where a provider the target's dimensions can land carries terms that bind the codebase it runs over, the landing MUST read the licence the binding declares and refuse the pair by name, writing nothing and naming a provider that carries no such condition, because a workflow whose terms the codebase does not satisfy is a licence violation this convention does not commit on a target's behalf; a licence that lapses after the landing MUST report as a warning under a stable code that `rk status --check` exits 0 on, because the target is not broken and the licensing decision is the operator's.

#### Scenario: An old record meets a newer binary

- GIVEN a landed target whose record predates either parameter
- WHEN `rk upgrade` runs
- THEN no destination of that capability joins the landing, and the rewritten record states the opt-out explicitly

Verify: `cargo nextest run -E 'test(a_pre_nix_record_upgrades_to_nothing_unrequested) or test(/scorecard/)'`

### `landing:the-flake-pair-lands-all-or-nothing` — The flake pair lands all-or-nothing

A landing MUST land the seed `flake.nix` and its matching `flake.lock` as a pair only where the target carries neither, withholding the pair with the reason named where either exists, because a seed lock beside a foreign flake describes the wrong input graph; the seeded package expression still lands, and a crate shape the seed does not support withholds the whole capability by name. The capability MUST land no CI file, because a job proving the build holds a merge only where the forge's own gate reaches it — inside the workflow the required check needs on GitHub, and inside the child pipeline the rendered parent triggers on GitLab, since that forge deep-merges an include and only a separate configuration isolates — and both of those files are the target's own. The seeded flake and the landed release declaration name the one system this repository dogfoods, `x86_64-linux` and `x86_64-unknown-linux-gnu`, bound by `packaging:an-advertised-system-is-a-proven-system`.

#### Scenario: A target with its own flake opts in

- GIVEN a rust target carrying a `flake.nix` of its own
- WHEN `rk init --nix-packaging --apply` runs
- THEN `nix/package.nix` lands, the pair is withheld with the reason reported, no workflow is written, the withheld destinations stay out of the record, and a later `rk upgrade` reproduces the same decision

Verify: `cargo nextest run -E 'test(a_target_with_its_own_flake_keeps_it_and_the_pair_is_withheld)'`
