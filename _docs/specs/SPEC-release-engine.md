# Release Engine Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`release-engine:the-adapter-owns-the-version-file` — The adapter owns the version file](#release-enginethe-adapter-owns-the-version-file--the-adapter-owns-the-version-file)
  - [`release-engine:one-request-per-base` — One request per base](#release-engineone-request-per-base--one-request-per-base)
  - [`release-engine:the-request-arms-where-the-projection-arms` — The request arms where the projection arms](#release-enginethe-request-arms-where-the-projection-arms--the-request-arms-where-the-projection-arms)
  - [`release-engine:the-request-never-numbers-a-candidate` — The request never numbers a candidate](#release-enginethe-request-never-numbers-a-candidate--the-request-never-numbers-a-candidate)
  - [`release-engine:the-request-pushes-as-the-bot` — The request pushes as the bot](#release-enginethe-request-pushes-as-the-bot--the-request-pushes-as-the-bot)
  - [`release-engine:a-final-version-releases-at-its-request` — A final version releases at its request](#release-enginea-final-version-releases-at-its-request--a-final-version-releases-at-its-request)
  - [`release-engine:a-version-releases-once` — A version releases once](#release-enginea-version-releases-once--a-version-releases-once)
  - [`release-engine:the-tag-is-created-last` — The tag is created last](#release-enginethe-tag-is-created-last--the-tag-is-created-last)
  - [`release-engine:every-step-converges` — Every step converges](#release-engineevery-step-converges--every-step-converges)
  - [`release-engine:a-candidate-is-a-prerelease` — A candidate is a prerelease](#release-enginea-candidate-is-a-prerelease--a-candidate-is-a-prerelease)
  - [`release-engine:the-python-version-maps-to-the-tag` — The python version maps to the tag](#release-enginethe-python-version-maps-to-the-tag--the-python-version-maps-to-the-tag)
  - [`release-engine:git-cliff-runs-at-its-pin` — git-cliff runs at its pin](#release-enginegit-cliff-runs-at-its-pin--git-cliff-runs-at-its-pin)
  - [`release-engine:the-output-declares-its-schema` — The output declares its schema](#release-enginethe-output-declares-its-schema--the-output-declares-its-schema)

<!--TOC-->

## Purpose

Rules governing the release engine that the `rk` binary runs in CI as `rk release request`, `rk release build`, and `rk release publish`, for each `(driver, forge)` pair whose driver has an adapter: today the bash and python drivers, on GitHub and GitLab. The request action keeps one release request per base branch. The build action decides whether a push releases and builds it. The publish action puts the release on the forge after the provenance and creates the tag last. Each action reads the driver, the forge, the repository, the trunk, the style, the integration, the line prefix, and the registry answer from the landing record, and takes no flag that restates one. What `rk release status` reads back belongs to `SPEC-release-state.md`, how `rk lines mint` writes a candidate belongs to `SPEC-release-candidates.md`, and the bot identity belongs to `SPEC-bot-identity.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The decisions behind these rules are `../decisions/ADR-run-the-release-through-the-rk-engine.md` and `../decisions/ADR-call-git-cliff-as-a-pinned-binary.md`, and the upstream facts are in `../reference/REFERENCE-release-engine-sources.md`.

## Requirements

### `release-engine:the-adapter-owns-the-version-file` — The adapter owns the version file

The engine MUST read and write a version through the driver adapter, which runs the ecosystem's native tool, `VERSION` for bash and `uv version` for python, and MUST NOT edit a manifest or a lock file itself, because the native tool is the one owner that keeps the version file and its lock in step.

#### Scenario: A python bump relocks

- GIVEN a uv project released at 0.1.0 and a `fix:` commit on the trunk
- WHEN `rk release request` runs
- THEN the request commit changes `pyproject.toml`, `uv.lock`, and `CHANGELOG.md` and nothing else, and `uv version 0.1.1 --no-sync` wrote the first two

Verify: `cargo nextest run -E 'test(the_python_request_relocks_with_the_bump) or test(the_build_keeps_support_files_out_of_dist) or test(a_version_reads_and_spells_each_technology) or test(a_bash_build_without_its_checksum_fails)'`

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

### `release-engine:the-output-declares-its-schema` — The output declares its schema

Each engine action MUST print, under `--json`, one document naming its schema, `rk.release-request/1`, `rk.release-build/1`, or `rk.release-publish/1`, per `distribution:machine-output-declares-its-schema`, and the build action MUST write the same document to `.rk-release/build.json`, because the jobs after the build read it there.

#### Scenario: A build skips

- GIVEN a push that releases nothing
- WHEN `rk release build --json` runs
- THEN it prints an `rk.release-build/1` document with state `skip`, writes the same document to `.rk-release/build.json`, and `rk release publish` reads it and skips

Verify: `cargo nextest run -E 'test(each_engine_report_holds_its_schema) or test(each_engine_action_prints_its_schema) or test(the_build_writes_its_report_to_the_github_output)'`
