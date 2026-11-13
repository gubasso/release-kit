# Release Engine Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`release-engine:the-adapter-owns-the-version-file` — The adapter owns the version file](#release-enginethe-adapter-owns-the-version-file--the-adapter-owns-the-version-file)
  - [`release-engine:one-request-per-base` — One request per base](#release-engineone-request-per-base--one-request-per-base)
  - [`release-engine:the-request-arms-where-the-projection-arms` — The request arms where the projection arms](#release-enginethe-request-arms-where-the-projection-arms--the-request-arms-where-the-projection-arms)
  - [`release-engine:the-request-never-numbers-a-candidate` — The request never numbers a candidate](#release-enginethe-request-never-numbers-a-candidate--the-request-never-numbers-a-candidate)
  - [`release-engine:a-request-follows-the-packed-files` — A request follows the packed files](#release-enginea-request-follows-the-packed-files--a-request-follows-the-packed-files)
  - [`release-engine:a-request-needs-a-releasing-commit` — A request needs a releasing commit](#release-enginea-request-needs-a-releasing-commit--a-request-needs-a-releasing-commit)
  - [`release-engine:a-rust-target-releases-one-package` — A rust target releases one package](#release-enginea-rust-target-releases-one-package--a-rust-target-releases-one-package)
  - [`release-engine:an-advisory-takes-a-releasing-type` — An advisory takes a releasing type](#release-enginean-advisory-takes-a-releasing-type--an-advisory-takes-a-releasing-type)
  - [`release-engine:the-bash-release-packs-its-declaration` — The bash release packs its declaration](#release-enginethe-bash-release-packs-its-declaration--the-bash-release-packs-its-declaration)
  - [`release-engine:the-request-pushes-as-the-bot` — The request pushes as the bot](#release-enginethe-request-pushes-as-the-bot--the-request-pushes-as-the-bot)
  - [`release-engine:a-final-version-releases-at-its-request` — A final version releases at its request](#release-enginea-final-version-releases-at-its-request--a-final-version-releases-at-its-request)
  - [`release-engine:a-version-releases-once` — A version releases once](#release-enginea-version-releases-once--a-version-releases-once)
  - [`release-engine:the-tag-is-created-last` — The tag is created last](#release-enginethe-tag-is-created-last--the-tag-is-created-last)
  - [`release-engine:every-step-converges` — Every step converges](#release-engineevery-step-converges--every-step-converges)
  - [`release-engine:a-candidate-is-a-prerelease` — A candidate is a prerelease](#release-enginea-candidate-is-a-prerelease--a-candidate-is-a-prerelease)
  - [`release-engine:the-python-version-maps-to-the-tag` — The python version maps to the tag](#release-enginethe-python-version-maps-to-the-tag--the-python-version-maps-to-the-tag)
  - [`release-engine:git-cliff-runs-at-its-pin` — git-cliff runs at its pin](#release-enginegit-cliff-runs-at-its-pin--git-cliff-runs-at-its-pin)
  - [`release-engine:no-predecessor-workflow-runs-beside-the-engine` — No predecessor workflow runs beside the engine](#release-engineno-predecessor-workflow-runs-beside-the-engine--no-predecessor-workflow-runs-beside-the-engine)
  - [`release-engine:the-output-declares-its-schema` — The output declares its schema](#release-enginethe-output-declares-its-schema--the-output-declares-its-schema)

<!--TOC-->

## Purpose

Rules governing the release engine that the `rk` binary runs in CI as `rk release request`, `rk release build`, and `rk release publish`, for each `(driver, forge)` pair whose driver has an adapter: today the bash and python drivers, on GitHub and GitLab. The request action keeps one release request per base branch. The build action decides whether a push releases and builds it. The publish action puts the release on the forge after the provenance and creates the tag last. `rk release eligible` answers, for every driver, whether a change is worth a release, and a rust request job runs it before release-plz; the commit-message rule for a security advisory follows the same classifier. Each action reads the driver, the forge, the repository, the trunk, the style, the integration, the line prefix, and the registry answer from the landing record, and takes no flag that restates one. What `rk release status` reads back belongs to `SPEC-release-state.md`, how `rk lines mint` writes a candidate belongs to `SPEC-release-candidates.md`, and the bot identity belongs to `SPEC-bot-identity.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The decisions behind these rules are `../decisions/ADR-run-the-release-through-the-rk-engine.md`, `../decisions/ADR-call-git-cliff-as-a-pinned-binary.md`, `../decisions/ADR-retire-an-unedited-dropped-destination.md`, and `../decisions/ADR-open-a-release-request-only-for-packed-changes.md`, and the upstream facts are in `../reference/REFERENCE-release-engine-sources.md`.

## Requirements

### `release-engine:the-adapter-owns-the-version-file` — The adapter owns the version file

The engine MUST read and write a version through the driver adapter, which runs the ecosystem's native tool, `VERSION` for bash and `uv version` for python, and MUST NOT edit a manifest or a lock file itself, because the native tool is the one owner that keeps the version file and its lock in step.

#### Scenario: A python bump relocks

- GIVEN a uv project released at 0.1.0 and a `fix:` commit on the trunk
- WHEN `rk release request` runs
- THEN the request commit changes `pyproject.toml`, `uv.lock`, and `CHANGELOG.md` and nothing else, and `uv version 0.1.1 --no-sync` wrote the first two

Verify: `cargo nextest run -E 'test(the_python_request_relocks_with_the_bump) or test(the_build_keeps_support_files_out_of_dist) or test(a_version_reads_and_spells_each_technology))'`

### `release-engine:one-request-per-base` — One request per base

The request action MUST keep at most one open release request per base branch, on the branch `rk-release/<base>`, MUST refresh it by force-pushing that branch and editing the request's title and description, and MUST close it and delete its branch when nothing remains to release, because a request named for its version opens a second request whenever a later commit moves the version.

#### Scenario: A feature after a fix moves the open request

- GIVEN an open request proposing 0.1.1 after a `fix:` commit
- WHEN a breaking change lands and `rk release request` runs again
- THEN the same request proposes 0.2.0 on the same branch, and no second request opens

Verify: `cargo nextest run -E 'test(the_request_opens_one_request_and_refreshes_it_in_place) or test(the_request_closes_a_stale_request_when_nothing_remains) or test(an_open_request_on_a_later_page_is_refreshed_and_closed)'`

### `release-engine:the-request-arms-where-the-projection-arms` — The request arms where the projection arms

Where the record's style is `trunk` and the base is the trunk, the request action MUST arm the request on every refresh with the bot credential, except on GitHub under local integration, where the rendered gate job merges it, and MUST leave a request under the `lines` style or on a release line unarmed, because the engine arms exactly where the landed projection arms.

#### Scenario: A locally integrated GitHub target

- GIVEN a GitHub target recorded with the trunk style and local integration
- WHEN `rk release request` opens the request
- THEN the request is not armed, and the same target under forge integration, or on GitLab, is armed

Verify: `cargo nextest run -E 'test(the_request_arms_only_where_the_projection_arms) or test(only_the_trunk_style_on_the_trunk_arms)'`

### `release-engine:the-request-never-numbers-a-candidate` — The request never numbers a candidate

The request action MUST propose nothing while the version is a prerelease, MUST propose a final version that has no release unchanged except the `0.0.0` baseline, which takes git-cliff's proposal, MUST propose git-cliff's bumped version for a released final version, and MUST refuse a prerelease proposal, because only `rk lines mint` writes or strips an rc number. A version is released only where the forge holds a release of its tag that is not a draft: a tag alone or a GitHub draft is no release, and a release read that fails MUST refuse with `observation-incomplete` before any write.

#### Scenario: A line promotes its candidate

- GIVEN a line whose version is `1.4.3-rc.1`
- WHEN the request runs, then `rk lines mint --promote` writes `1.4.3`, and the request runs again
- THEN the first run proposes nothing, the second proposes `1.4.3`, and after `1.4.3` releases a git-cliff proposal of `v1.4.4-rc.1` refuses

Verify: `cargo nextest run -E 'test(the_request_never_numbers_a_candidate) or test(a_bash_baseline_releases_through_its_first_request) or test(the_proposal_follows_the_candidate_shape) or test(the_request_keeps_a_final_version_whose_tag_has_no_release) or test(a_failed_release_read_refuses_the_request)'`

### `release-engine:a-request-follows-the-packed-files` — A request follows the packed files

Where the base carries a released final version, the request action MUST propose a version only where the release packs a different set of files at the release tag and at `HEAD`, or where a file the release packs differs between the two, as `git diff --quiet <tag> HEAD -- <packed files>` answers. Otherwise it MUST report "no packed file changed since `<tag>`" and close a stale request. The packed files MUST be the union of the files the release packs at `HEAD` and at the tag, each a tracked file of its revision. Each side MUST drop every destination its own revision's `.release-kit/manifest.json` names, and every path under `.release-kit/`, also where an `include` pattern ships one. The bash adapter MUST list the files the declaration of that revision selects, read from that revision's `.release-kit/config.toml`, per `release-engine:the-bash-release-packs-its-declaration`, with no build. A revision whose configuration names neither `include` nor `exclude` packs every tracked file, so the commit that first narrows the set opens one request. Where neither side holds a file, the request MUST open nothing, close a stale request, and report "no packed file exists yet: `profile.release.build.include` matches no tracked file". The python adapter MUST list the members of the sdist `uv build --sdist` writes, below its top directory and without `PKG-INFO` and `pyproject.toml.orig`. Each sdist build MUST run in a temporary worktree outside the checkout, removed afterwards also on failure. git-cliff MUST take one `--include-path` per packed file for the version, the changelog, and the release notes, and one for the driver's version file, because git-cliff filters the commits before it matches the tags, and only the version file is sure to change in the commit a release tags. Where the two sets differ and the two declarations differ, git-cliff MUST also take `.release-kit/config.toml` for that run, so the commit that changed the set counts toward the version, the changelog, and the notes. The request MUST write only the new section, through `git-cliff --unreleased --prepend CHANGELOG.md`, or `-o CHANGELOG.md` where the file does not exist, and every earlier section MUST keep its bytes, because a released section records what that release shipped. A version with no release MUST read no packed list for the version, and reads the files the release packs at `HEAD` only for `release-engine:a-request-needs-a-releasing-commit`. This follows release-plz, which counts only the commits that touch a file Cargo packs and counts a crate whose packed list changed, because a commit outside the package changes nothing a release ships. The declaration plays the part of the `include` and `exclude` fields of `Cargo.toml`, a file every crate packs, so a change of the set it selects counts as Cargo's would. The one departure is the drop of rk's own files: they configure the release and never ship as the product's code, so the commit that takes an rk release opens no request.

#### Scenario: A build commit outside the package

- GIVEN a bash target released at `v0.1.0` whose `exclude` leaves out its `Makefile`
- WHEN a `build:` commit changes the `Makefile` and `rk release request` runs
- THEN it proposes nothing, reports "no packed file changed since `v0.1.0`", and closes an open request

#### Scenario: A declaration that widens the package

- GIVEN a bash target released at `v0.1.0` whose `include` leaves out a readme that no commit has changed since
- WHEN a commit adds the readme to `include` and `rk release request` runs
- THEN it proposes `0.1.1`, and the changelog section names that commit

#### Scenario: A fix and its revert

- GIVEN a `fix:` on a packed file and a later commit that reverts it
- WHEN `rk release request` runs
- THEN it proposes nothing, although git-cliff alone would propose the patch

Verify: `cargo nextest run -E 'test(a_commit_outside_the_package_opens_no_request) or test(a_fix_in_a_packed_file_opens_a_patch_request) or test(a_change_reverted_before_the_release_opens_no_request) or test(a_packed_file_added_or_removed_counts) or test(the_commit_that_takes_an_rk_release_opens_no_request) or test(a_destination_rk_started_or_stopped_managing_follows_its_own_manifest) or test(a_declaration_change_counts_on_its_own_side) or test(the_packed_set_counts_as_a_set) or test(a_declaration_that_matches_nothing_yet_opens_no_request) or test(the_python_list_holds_for_every_backend) or test(the_release_notes_follow_the_packed_files) or test(the_changelog_keeps_its_released_sections) or test(the_bash_request_jobs_install_no_build_packages) or test(the_packed_list_arithmetic_holds)'`

### `release-engine:a-request-needs-a-releasing-commit` — A request needs a releasing commit

Where `release-engine:a-request-follows-the-packed-files` finds a packed change, the request action MUST propose a version only where at least one commit of `<tag>..HEAD` that touched the packed files is a releasing commit, and otherwise MUST report "none of the `<n>` commits that touched the packed files since `<tag>` is a releasing commit" and close a stale request. A releasing commit has the type `feat`, `fix`, or `perf`, or marks a breaking change with `!` or a `BREAKING CHANGE:` or `BREAKING-CHANGE:` footer. The commits read MUST include those that changed `.release-kit/config.toml` where its declaration changed the packed set, and MUST leave out the version file, which only the release commit changes. A version with no release, set by hand or promoted from a candidate, MUST be judged the same way against the newest stable release tag below `HEAD`, never a candidate's tag, and MUST keep its declared version once that judgment holds. Where the checkout holds no stable release tag, the request MUST read the commits of the whole history that touched the files the release packs at `HEAD`, and MUST propose the declared version only where one of them is a releasing commit. A commit that does not release MUST stay in the changelog. The engine, `rk integrate`, and the commit-message rule MUST share one classifier. The packed change says what ships, and the type says whether the change is worth a release, which is release-plz's model.

#### Scenario: A build commit on a packed file

- GIVEN a bash target released at `v0.1.0` and an open release request
- WHEN a `build:` commit changes a packed file and `rk release request` runs
- THEN it proposes nothing, names the one commit that is not a releasing commit, and closes the request

#### Scenario: A first release with only chores

- GIVEN a target at an unreleased `0.1.0` whose every commit is a `chore:`
- WHEN `rk release request` runs, and again after a `fix:` on a packed file
- THEN the first run proposes nothing, and the second proposes `0.1.0`

Verify: `cargo nextest run -E 'test(a_build_commit_on_a_packed_file_opens_no_request) or test(a_fix_commit_on_a_packed_file_opens_a_request) or test(a_breaking_refactor_opens_a_request) or test(a_perf_commit_opens_a_request) or test(a_feat_outside_the_packed_files_opens_no_request) or test(a_fix_packaging_edit_that_ships_an_existing_file_opens_a_request) or test(a_chore_edit_of_the_declaration_opens_no_request) or test(an_initial_release_needs_a_releasing_commit) or test(a_hand_bumped_version_needs_a_releasing_packed_change) or test(release_intent_classifies_perf_and_breaking) or test(the_judgment_needs_one_releasing_commit)'`

### `release-engine:a-rust-target-releases-one-package` — A rust target releases one package

`rk release eligible` MUST answer the rule of `release-engine:a-request-needs-a-releasing-commit` for every driver, MUST write nothing, and MUST read no forge, so it runs outside CI; the last release is the release tag git holds. For a rust target it MUST judge the one workspace member release-plz 0.4.3 releases: a member whose release-plz `release` key is not `false`, and whose Cargo `publish` field lets it publish or that is `git_only` and not an example-only package with no `publish`. A helper member that sets `publish = false` under a registry release is no released member, and a single `git_only` package that sets `publish = false` is one. Where release-plz releases more than one member, the verb MUST refuse, naming the limit the tag shape `v{{ version }}` assumes. The packed files MUST be what `cargo package --list` prints at `HEAD` and at the last release tag, each in a temporary worktree, and `Cargo.lock` only for a package with a binary target, read from the real commits that changed it. The request job of both rust pipelines MUST run the verb before release-plz `release-pr` and skip `release-pr` with the logged reason where the answer is no, and `release-plz.toml` MUST gain no key. release-plz judges the lock by meaning and changes only a released entry, so an added lock entry can answer eligible and still open nothing.

#### Scenario: A lock-only security update

- GIVEN a rust binary released at `v0.1.0` and a `fix(deps):` commit that moves a dependency the release already had and changes only `Cargo.lock`
- WHEN the request job runs
- THEN `rk release eligible` answers yes, and release-plz opens the patch

Verify: `cargo nextest run -E 'test(a_lock_only_fix_deps_opens_a_request) or test(an_added_lock_entry_is_eligible_but_opens_nothing) or test(a_workspace_with_two_released_packages_refuses) or test(a_helper_member_with_publish_empty_is_skipped) or test(a_single_package_with_publish_empty_is_judged) or test(a_lock_only_fix_deps_with_an_advisory_opens_a_request) or test(a_lock_only_chore_deps_opens_no_request) or test(a_build_commit_rides_along_in_the_changelog) or test(rk_release_eligible_answers_without_a_forge) or test(release_plz_selects_the_released_members) or test(the_rust_request_job_skips_release_pr_when_not_eligible)'`

### `release-engine:an-advisory-takes-a-releasing-type` — An advisory takes a releasing type

Where the target names a release, `rk message --check`, and with it the landed `rk-message` commit-msg hook and `rk integrate`, MUST report the class `advisory-type` for a message that names a security advisory, `RUSTSEC-<year>-<serial>`, `GHSA-` and three groups of four, or `CVE-<year>-<serial>`, under a type that is not a releasing commit per `release-engine:a-request-needs-a-releasing-commit`. The landed title check on each forge MUST refuse the same title and body, and under the rebase merge form the landed commit check MUST refuse such a commit of the request, because there the commit check is the one every commit reaches the trunk through. Both MUST exempt the release bot's own request. A security update always releases, so its type is `fix(deps)`, also for an update that changes only a shipped lock file.

#### Scenario: A security update typed as a chore

- GIVEN a target that names a release
- WHEN a commit message reads `chore(deps): bump foo` and names `RUSTSEC-2026-0001` in its body
- THEN `rk message --check` exits 1 with an `advisory-type` finding, and the same message typed `fix(deps):` passes

Verify: `cargo nextest run -E 'test(rk_message_refuses_an_advisory_without_a_releasing_type) or test(the_title_check_refuses_an_advisory_without_a_releasing_type) or test(the_github_commit_gate_refuses_an_advisory_without_a_releasing_type) or test(the_gitlab_commit_gate_refuses_an_advisory_without_a_releasing_type)'`

### `release-engine:the-bash-release-packs-its-declaration` — The bash release packs its declaration

A bash release MUST ship the tracked files of `HEAD` that `profile.release.build.include` and `profile.release.build.exclude` select, with the semantics of Cargo's `include` and `exclude` fields: gitignore-style patterns, where `include` overrides `exclude`, a `!` inside `include` carves out an exception, and neither key ships every tracked file. `VERSION` MUST always ship. A destination the revision's record names, and `.release-kit/`, MUST ship only where an `include` pattern names it. `rk release build` MUST pack that set from the git objects of `HEAD` into `dist/<name>-<version>.tar.gz` under the one top directory `<name>-<version>/`, beside its `.sha256`, where `<name>` is the last segment of the project path. The archive MUST be reproducible: members sorted by path, modes from the git modes, owner and group 0 with empty names, every time `SOURCE_DATE_EPOCH`, and a gzip header with no name and a zero time. Where `profile.release.build.verify` names a command, the build MUST run it in the unpacked archive in a temporary directory and MUST refuse before any upload, tag, or release where it fails. It MUST refuse a package that holds nothing past `VERSION`, naming `profile.release.build.include`. `rk release build --list` MUST print the files the package holds at `HEAD`, write nothing, and read no forge. This follows `cargo package`, which packs and verifies a crate from its own declared files.

#### Scenario: Two builds of one commit

- GIVEN a bash target whose `exclude` leaves out `docs/` and an untracked file in the working tree
- WHEN `rk release build` runs twice at one commit
- THEN both runs write the same archive bytes and the same `.sha256`, and neither archive holds `docs/` or the untracked file

Verify: `cargo nextest run -E 'test(the_declaration_follows_cargo_semantics) or test(the_bash_archive_is_the_declared_set) or test(the_bash_archive_builds_byte_for_byte_twice) or test(a_failing_verify_command_refuses_before_any_write) or test(a_declaration_that_matches_nothing_yet_opens_no_request) or test(the_list_prints_the_package_and_writes_nothing)'`

### `release-engine:the-request-pushes-as-the-bot` — The request pushes as the bot

The request action MUST push and arm with the bot credential the job sets, `GH_TOKEN` on GitHub and `GITLAB_TOKEN` on GitLab, and MUST refuse with `default-ci-token` before any push where that credential is the default CI token, `GITHUB_TOKEN` or `CI_JOB_TOKEN`, alone or under the bot's name, because a push with the default token starts no pipeline and the release half would never run.

#### Scenario: A job forwards only the default token

- GIVEN a GitLab job that sets `CI_JOB_TOKEN` and no `GITLAB_TOKEN`
- WHEN `rk release request` runs
- THEN it exits 73 naming `CI_JOB_TOKEN`, and nothing is pushed

Verify: `cargo nextest run -E 'test(the_request_refuses_the_default_ci_token) or test(only_the_default_token_alone_is_the_default_token) or test(the_request_commits_as_the_identity_the_forge_names)'`

### `release-engine:a-final-version-releases-at-its-request` — A final version releases at its request

The build action MUST release a final version only at the merge or squash commit of a merged request whose head is `rk-release/<base>`, read from the forge, MUST release a candidate only at the commit that writes it, and MUST skip with success the commit that introduces the version file, the `0.0.0` baseline, and every other push, because a version that releases wherever it appears releases on a hand edit or on the promote push.

#### Scenario: The promote push lands on a line

- GIVEN a line whose version moves from `1.4.3-rc.1` to `1.4.3` in the promote push
- WHEN `rk release build` runs at that push
- THEN it skips with exit 0, and the merge of the line's request releases `1.4.3`

Verify: `cargo nextest run -E 'test(the_build_releases_only_at_the_request_merge) or test(the_mint_push_releases_a_candidate) or test(the_promote_push_does_not_release) or test(the_build_skips_the_baseline_and_a_new_version_file) or test(a_fresh_project_releases_through_its_first_request) or test(the_trigger_reads_the_commit_before_the_forge) or test(the_github_release_job_can_read_the_request_merge) or test(each_engine_pair_releases_a_promotion)'`

### `release-engine:a-version-releases-once` — A version releases once

When a push is a release trigger, the build action MUST skip with success where the forge holds a published release of the version at another commit, MUST continue as `release` where no release and no tag of it exist, MUST continue as `resume` where a release, a draft, or a tag of it sits at this commit, and MUST refuse naming the recovery section "The tag exists and the release does not" where the tag names another commit or the tag read fails, and the publish action MUST ask again before it writes, because a rerun can start later than its build.

#### Scenario: An un-bump returns to a released version

- GIVEN `v0.1.0` published at commit A, and a later commit that writes 0.1.0 again
- WHEN `rk release build` runs at that commit
- THEN it skips with exit 0, naming commit A

Verify: `cargo nextest run -E 'test(the_build_skips_an_ordinary_push_after_a_release) or test(the_build_skips_an_un_bump) or test(the_build_refuses_a_conflicting_tag) or test(a_failed_tag_read_refuses) or test(each_release_state_gets_one_outcome) or test(a_release_on_a_later_page_is_found)'`

### `release-engine:the-tag-is-created-last` — The tag is created last

The publish action MUST create the tag only through the step that publishes the forge release, after the build and the provenance: on GitHub it MUST create a draft at the commit, which creates no tag, upload every asset, and then publish the draft, and on GitLab it MUST upload every package file and then send one release create with the commit as `ref` and every link, because neither forge publishes atomically and a tag created first would point at a release that may never exist.

#### Scenario: A GitLab upload fails

- GIVEN a release whose second package upload fails
- WHEN `rk release publish` runs
- THEN it exits with the failure, no tag and no release exist, and a rerun publishes both

Verify: `cargo nextest run -E 'test(the_publish_creates_the_tag_and_the_release_last_on_each_forge) or test(a_failed_gitlab_upload_leaves_no_tag) or test(the_bash_github_workflow_attests_before_it_creates_the_release) or test(the_bash_pipelines_carry_no_release_shell)'`

### `release-engine:every-step-converges` — Every step converges

A rerun at the same commit MUST skip what the forge holds with the same bytes, MUST create what is missing, MUST replace an asset with other bytes only on an unpublished GitHub draft, and MUST refuse naming its recovery section where a published asset, a GitLab package file, or a file PyPI serves for the version holds other bytes than the rebuild, and MUST leave every existing tag where it stands, because a release page or a registry that serves other bytes than the attested build is a defect that only a new version repairs. A provenance bundle `<artifact>.sigstore.json` that a published GitHub release or the GitLab package registry already holds beside an artifact with this build's bytes MUST be kept and reported as kept, because each attestation run signs new bundle bytes for the same artifact, and the publish verifies no signature.

#### Scenario: A page lost one asset

- GIVEN a published release at this commit whose page lacks the `.sha256`
- WHEN `rk release publish` runs again
- THEN it uploads that one file and writes nothing else

Verify: `cargo nextest run -E 'test(the_build_resumes_at_the_same_commit) or test(a_rerun_publishes_a_same_commit_draft) or test(a_rerun_repairs_a_partial_release) or test(the_publish_refuses_an_asset_whose_digest_differs) or test(the_build_refuses_a_rebuild_that_differs_from_pypi) or test(a_full_rerun_keeps_the_provenance_of_a_published_release) or test(a_full_rerun_creates_the_gitlab_release_after_an_interrupted_create) or test(a_held_bundle_is_kept_only_beside_a_matching_artifact) or test(an_asset_on_a_later_page_is_found) or test(the_python_upload_takes_only_the_distributions)'`

### `release-engine:a-candidate-is-a-prerelease` — A candidate is a prerelease

The publish action MUST publish a candidate on GitHub as a prerelease that is not marked latest, and on GitLab as a release whose description names the candidate on its first line, because a GitLab release page carries no prerelease badge.

#### Scenario: A candidate is published on GitLab

- GIVEN a line whose mint push wrote `1.4.3-rc.1`
- WHEN `rk release build` and `rk release publish` run
- THEN the GitLab release `v1.4.3-rc.1` opens with a line naming it a release candidate

Verify: `cargo nextest run -E 'test(a_candidate_publishes_as_a_prerelease_on_each_forge)'`

### `release-engine:the-python-version-maps-to-the-tag` — The python version maps to the tag

The python adapter MUST map a PEP 440 version to the semver-shaped tag and back, `1.4.3` to `v1.4.3` and `1.4.3rc2` to `v1.4.3-rc.2`, because every pair tags the same shape, which `rk lines mint` and `rk release status` read.

#### Scenario: A python candidate is tagged

- GIVEN a python version `1.4.3rc2`
- WHEN the adapter names its tag and reads the tag back
- THEN the tag is `v1.4.3-rc.2`, and it reads back as `1.4.3rc2`

Verify: `cargo nextest run -E 'test(the_python_version_maps_to_the_tag_and_back) or test(the_python_release_tags_a_candidate_in_the_line_shape)'`

### `release-engine:git-cliff-runs-at-its-pin` — git-cliff runs at its pin

The engine MUST run git-cliff at the version the embedded `versions.toml` pins, MUST refuse with `tool-off-pin` before any write where `git-cliff --version` reports another, and MUST NOT pass `--bump`, and where the target records a development environment, that environment MUST carry git-cliff at the pin and the uv the target requires, because a recorded environment is the project's one source of its tools and the changelog must move only with the registry row.

#### Scenario: The runner carries an older git-cliff

- GIVEN a job whose git-cliff reports a version the registry does not pin
- WHEN `rk release request` or `rk release build` runs
- THEN it exits 73 with `tool-off-pin`, naming the pin, and nothing is pushed

Verify: `cargo nextest run -E 'test(the_engine_refuses_git_cliff_off_its_pin) or test(every_engine_tool_download_verifies_its_checksum) or test(the_python_uv_version_follows_the_target_pin)'`

### `release-engine:no-predecessor-workflow-runs-beside-the-engine` — No predecessor workflow runs beside the engine

Where a bash or python target on GitHub holds the engine workflow `.github/workflows/release.yml`, `rk status --check` MUST count a violation with the code `predecessor-release-workflow` for each release workflow of another tool that the migration inventory lists and that stands beside it as a regular file, and MUST name the file, its tool, and the guidance that removes it, because each such workflow runs its own release on the same pushes and opens a second release request.

#### Scenario: An upgrade kept an edited predecessor workflow

- GIVEN a python target on GitHub whose edited release workflow from its previous release tool the upgrade kept as dropped
- WHEN `rk status --check` runs
- THEN it exits 1 naming the workflow and its tool, and it passes once the operator deletes the file

Verify: `cargo nextest run -E 'test(status_check_faults_a_predecessor_workflow_beside_the_engine)'`

### `release-engine:the-output-declares-its-schema` — The output declares its schema

Each engine action MUST print, under `--json`, one document naming its schema, `rk.release-request/2`, `rk.release-build/2`, `rk.release-eligible/1`, or `rk.release-publish/1`, per `distribution:machine-output-declares-its-schema`, and the build action MUST write the same document to `.rk-release/build.json`, because the jobs after the build read it there.

#### Scenario: A build skips

- GIVEN a push that releases nothing
- WHEN `rk release build --json` runs
- THEN it prints an `rk.release-build/2` document with state `skip`, writes the same document to `.rk-release/build.json`, and `rk release publish` reads it and skips

Verify: `cargo nextest run -E 'test(each_engine_report_holds_its_schema) or test(each_engine_action_prints_its_schema) or test(the_build_writes_its_report_to_the_github_output) or test(the_eligible_report_holds_its_schema)'`
