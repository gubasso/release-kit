# Release Ref Rules Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`release-ref-rules:only-the-release-actors-update-the-trunk` — Only the release actors update the trunk](#release-ref-rulesonly-the-release-actors-update-the-trunk--only-the-release-actors-update-the-trunk)
  - [`release-ref-rules:only-the-release-actors-create-a-release-tag` — Only the release actors create a release tag](#release-ref-rulesonly-the-release-actors-create-a-release-tag--only-the-release-actors-create-a-release-tag)
  - [`release-ref-rules:only-the-release-actors-write-a-release-branch` — Only the release actors write a release branch](#release-ref-rulesonly-the-release-actors-write-a-release-branch--only-the-release-actors-write-a-release-branch)
  - [`release-ref-rules:a-dispatch-job-serves-the-administrator-on-its-own-ref` — A dispatch job serves the administrator on its own ref](#release-ref-rulesa-dispatch-job-serves-the-administrator-on-its-own-ref--a-dispatch-job-serves-the-administrator-on-its-own-ref)
  - [`release-ref-rules:the-github-bot-secrets-live-in-the-release-environment` — The GitHub bot secrets live in the release environment](#release-ref-rulesthe-github-bot-secrets-live-in-the-release-environment--the-github-bot-secrets-live-in-the-release-environment)
  - [`release-ref-rules:protections-check-judges-the-release-rulesets` — protections-check judges the release rulesets](#release-ref-rulesprotections-check-judges-the-release-rulesets--protections-check-judges-the-release-rulesets)
  - [`release-ref-rules:the-gitlab-trunk-merges-at-maintainer` — The GitLab trunk merges at Maintainer](#release-ref-rulesthe-gitlab-trunk-merges-at-maintainer--the-gitlab-trunk-merges-at-maintainer)
  - [`release-ref-rules:the-gitlab-release-tags-are-created-at-maintainer` — The GitLab release tags are created at Maintainer](#release-ref-rulesthe-gitlab-release-tags-are-created-at-maintainer--the-gitlab-release-tags-are-created-at-maintainer)
  - [`release-ref-rules:only-maintainers-write-a-gitlab-release-branch` — Only Maintainers write a GitLab release branch](#release-ref-rulesonly-maintainers-write-a-gitlab-release-branch--only-maintainers-write-a-gitlab-release-branch)
  - [`release-ref-rules:merge-request-pipelines-cannot-read-protected-variables` — Merge request pipelines cannot read protected variables](#release-ref-rulesmerge-request-pipelines-cannot-read-protected-variables--merge-request-pipelines-cannot-read-protected-variables)
  - [`release-ref-rules:the-gitlab-bot-token-is-protected` — The GitLab bot token is protected](#release-ref-rulesthe-gitlab-bot-token-is-protected--the-gitlab-bot-token-is-protected)
  - [`release-ref-rules:protections-check-judges-the-gitlab-protections` — protections-check judges the GitLab protections](#release-ref-rulesprotections-check-judges-the-gitlab-protections--protections-check-judges-the-gitlab-protections)

<!--TOC-->

## Purpose

Rules governing who changes the refs that start a release or change the trunk: who updates the trunk, who creates a release tag, and who creates or updates a release branch, and how `protections-check` judges each rule. On GitHub, the release actors are the repository administrator and the release bot App, where an App ID resolves. On GitLab, they are the members at Maintainer or above, which the release bot is. Every other identity with write access is refused on these refs. How a run reaches the forge, what each step asserts, and which steps apply are bound by `SPEC-forge-setup.md`. How the release bot key and the App ID resolve is bound by `SPEC-bot-key.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The forge documentation these rules rest on is in `../reference/REFERENCE-forge-setup-sources.md`.

## Requirements

### `release-ref-rules:only-the-release-actors-update-the-trunk` — Only the release actors update the trunk

On GitHub, `protect-trunk` MUST install the updates ruleset that `protection.updates_ruleset` names, `<trunk>-updates` by default, after the safety ruleset and before the trunk ruleset. The ruleset MUST target the trunk branch alone, carry the `update` rule alone, and name exactly the release actors as its bypass. The trunk ruleset and the safety ruleset MUST keep their own rules and their own bypass. The release App then merges an ordered release request with an explicit bypass of the `update` rule alone, and the request rule and the required checks, which it does not bypass, still bind it. The merge job MUST choose that bypass in the merge call itself, because GitHub merges past a rule only where its bypass actor chooses to. Another identity meets the request rule on a push and the `update` rule on a merge.

#### Scenario: A trunk under forge integration

- GIVEN a GitHub target under forge integration, whose trunk ruleset names no bypass actor
- WHEN `rk setup step protect-trunk --apply` runs
- THEN the forge receives the safety ruleset, then the updates ruleset with the administrator role as its bypass, then the trunk ruleset with no bypass

Verify: `cargo nextest run -E 'test(protect_trunk_installs_the_updates_ruleset_after_the_safety_ruleset)'`

### `release-ref-rules:only-the-release-actors-create-a-release-tag` — Only the release actors create a release tag

On GitHub, `protect-tags` MUST install two tag rulesets. The immutable one MUST cover `protection.tag_pattern` alone, carry `deletion` and `update`, and name no bypass actor. The creation one MUST carry the `creation` rule alone, name exactly the release actors as its bypass, and cover every tag that starts a release. On a Rust target, release-plz names its tag `v{{ version }}` or `{{ package }}-v{{ version }}`, and the artifact workflow that the artifact builder generates starts on any tag that carries an `x.y.z` version, such as `1.2.3` or `x/1.2.3`. No ruleset pattern states that trigger exactly, so there the creation ruleset MUST cover `refs/tags/**/*[0-9].[0-9]*.[0-9]*`, which covers both release-plz shapes, and MUST NOT cover a tag with no such version. A GitHub ruleset pattern matches as `File.fnmatch` with `FNM_PATHNAME`, so `*` and a bare `**` stop at `/`, and `refs/tags/**` misses `x/1.2.3`. Elsewhere it MUST cover `protection.tag_pattern`.

#### Scenario: A Rust target on GitHub

- GIVEN a Rust target on GitHub with `protection.tag_pattern = "refs/tags/v*"`
- WHEN `rk setup step protect-tags --apply` runs
- THEN the immutable ruleset covers `refs/tags/v*` with no bypass, and the creation ruleset covers `refs/tags/**/*[0-9].[0-9]*.[0-9]*`, `1.2.3`, `x/1.2.3`, and `v1.2.3` included and `docs-snapshot` left out, with the release actors as its bypass

Verify: `cargo nextest run -E 'test(protect_tags_installs_a_creation_ruleset_beside_the_immutable_one) or test(the_tag_creation_patterns_cover_every_release_tag_trigger)'`

### `release-ref-rules:only-the-release-actors-write-a-release-branch` — Only the release actors write a release branch

Where the release mode is not `none`, `protect-release-branches` on GitHub MUST install one ruleset over the release lines under `profile.release.line_prefix` and the head branches of both release requests, `release-plz-` and `rk-release/`, whatever the driver. Each prefix MUST cover its nested names too: a prefix that ends in `/` takes `<prefix>**/*`, and another prefix takes `<prefix>*` and `<prefix>*/**/*`. The ruleset MUST carry `creation` and `update`, and name exactly the release actors as its bypass. Without the request prefixes, another identity with write access pushes a commit onto an open release request, and the release App merges it with the request.

#### Scenario: A python target that keeps no older line

- GIVEN a python target on GitHub with the default line prefix
- WHEN `rk setup step protect-release-branches --apply` runs
- THEN the ruleset covers `refs/heads/release/**/*`, `refs/heads/release-plz-*`, `refs/heads/release-plz-*/**/*`, and `refs/heads/rk-release/**/*`

Verify: `cargo nextest run -E 'test(protect_release_branches_restricts_creation_and_update_of_the_line_prefix) or test(protect_release_branches_covers_both_release_request_prefixes) or test(protect_release_branches_applies_on_github_with_a_release)'`

### `release-ref-rules:a-dispatch-job-serves-the-administrator-on-its-own-ref` — A dispatch job serves the administrator on its own ref

On GitHub, each rendered job that a dispatch starts and that mints the release App token MUST run only on its own ref. `release-candidate` MUST run on the line that its `line` input names, under `profile.release.line_prefix`, and `release-merge` on the trunk. Right after the mint and before any step that writes, each MUST read the caller's role on the repository and refuse every role below the administrator. Both jobs MUST take that proof from one block. Any identity with write access dispatches a workflow on a ref that it names, and the release App writes a release line past the release branch ruleset.

#### Scenario: A dispatch by a write identity

- GIVEN a landed GitHub release workflow, and a caller that holds the write role
- WHEN the caller dispatches `release-candidate` on its line
- THEN the job refuses after the mint and before the checkout, and names the role

Verify: `cargo nextest run -E 'test(each_github_dispatch_job_runs_only_on_its_own_ref) or test(each_github_dispatch_job_proves_an_administrator_caller) or test(a_dispatch_by_a_non_admin_refuses)'`

### `release-ref-rules:the-github-bot-secrets-live-in-the-release-environment` — The GitHub bot secrets live in the release environment

On GitHub, every job of a rendered release workflow that mints the release App token MUST name a GitHub environment: the one that `protection.release_environment` names, `release` by default. The python release job under a registry MUST keep `pypi`, because the registry's trusted publisher names that environment. The environments that hold the bot secrets MUST come from the rendered jobs, in one place, so `pypi` joins the release environment on a python target that publishes. The record MUST carry a configured name, and omit the default, so a record that takes the default keeps its bytes.

`bot-secrets` MUST store both bot secrets in each of those environments, behind a deployment policy of custom patterns: the trunk, `<line_prefix>*`, `release-plz-*`, `rk-release/<trunk>`, and `rk-release/<line_prefix>*` as branches, and `protection.tag_pattern` without `refs/tags/` as a tag. The patterns MUST come from the same prefixes as the release branch protection. The `pypi` environment takes that policy safely, because the one job that runs in it runs on a push to the trunk or a release line. The step MUST remove the repository copies only once the release workflow at the forge names an environment on every job that mints the App token, on the trunk and on each release line that the clone knows at its remote. Until then it MUST keep them, and the check MUST fail naming the refs. A private repository on the Free plan holds no environment secrets, so there the step MUST keep the repository copies and report the limitation by name. `rk setup check` and `protections-check` MUST fail repository copies, a missing environment, a wider policy, and a missing environment secret, with `rk setup --apply` as the remedy, and MUST name the Free plan limitation without failing on it.

#### Scenario: A python target that publishes

- GIVEN a python target on GitHub with a registry
- WHEN the release workflow renders
- THEN its `release` job names `pypi`, every other job that mints the App token names `release`, and the environments that hold the bot secrets are `release` and `pypi`

Verify: `cargo nextest run -E 'test(every_github_app_token_job_names_the_release_environment) or test(the_release_environments_come_from_the_rendered_jobs) or test(github_bot_secrets_move_into_the_release_environment) or test(github_repository_secrets_wait_for_every_release_ref) or test(github_free_private_keeps_repository_secrets_with_a_limitation) or test(a_full_github_apply_moves_the_old_secret_shape) or test(the_environment_policy_follows_the_release_branch_prefixes)'`

### `release-ref-rules:protections-check-judges-the-release-rulesets` — protections-check judges the release rulesets

On GitHub, `protections-check` MUST own the updates ruleset always, and the tag creation ruleset and the release branch ruleset wherever the release tags are protected. On an automatic release it MUST also judge where the bot secrets live, by `release-ref-rules:the-github-bot-secrets-live-in-the-release-environment`. A missing one MUST fail with its step as the remedy. Each MUST fail where it carries another rule, misses or adds a pattern, excludes a ref, or names a bypass actor beyond the release actors, and a missing pattern MUST be named. The Rust observation that `rk setup check` reads and the executable script MUST judge the same facts.

#### Scenario: A release branch ruleset without a request prefix

- GIVEN a release branch ruleset that covers the release lines and both `release-plz-` patterns alone
- WHEN `rk setup check` runs, and `rk setup step protections-check --apply` runs
- THEN both fail, and both name `refs/heads/rk-release/**/*`

Verify: `cargo nextest run -E 'test(protections_check_fails_on_a_release_branch_ruleset_without_a_request_prefix) or test(protections_check_fails_on_each_missing_new_ruleset) or test(protections_check_fails_on_a_wider_bypass) or test(protections_check_fails_on_an_update_ruleset_with_another_rule) or test(setup_check_judges_the_new_rulesets_through_the_rust_aggregate) or test(protections_check_passes_on_the_owned_release_rulesets)'`

### `release-ref-rules:the-gitlab-trunk-merges-at-maintainer` — The GitLab trunk merges at Maintainer

On GitLab, `protect-trunk` MUST protect the trunk with exactly one push grant at `protection.gitlab.push_access_level` and exactly one merge grant at `protection.gitlab.merge_access_level`. The floor of `protection.gitlab.merge_access_level` MUST be 40, Maintainer. A configured value below 40 MUST refuse by its key when `rk` reads the configuration, before any forge call. A Developer then neither pushes the trunk nor merges into it.

#### Scenario: A Developer merge grant

- GIVEN a GitLab target with `protection.gitlab.merge_access_level = 30`
- WHEN `rk setup step protect-trunk --apply` runs
- THEN it refuses naming `protection.gitlab.merge_access_level`, and the forge receives no call

Verify: `cargo nextest run -E 'test(a_developer_merge_grant_on_the_gitlab_trunk_refuses_before_any_forge_call) or test(a_gitlab_apply_replaces_the_default_trunk_protection)'`

### `release-ref-rules:the-gitlab-release-tags-are-created-at-maintainer` — The GitLab release tags are created at Maintainer

On GitLab, `protect-tags` MUST protect the tag name of `protection.tag_pattern`, without its `refs/tags/` prefix, with exactly one create grant at 40. Where another owned pattern stands, the step MUST protect the recorded pattern first, read it back, and only then unprotect the stale one. Where the recorded pattern stands with another grant, the step MUST unprotect it and protect it again. Both cases MUST end with a read that requires the owned grant. Where that read fails, or a protect call failed, the step MUST protect once more and fail, naming the pattern and whether it stands protected.

#### Scenario: A widened tag pattern

- GIVEN a GitLab target with `protection.tag_pattern = "refs/tags/*"`, whose `v*` stands protected
- WHEN `rk setup step protect-tags --apply` runs
- THEN the forge receives the protect call for `*`, then its read, then the unprotect call for `v*`

Verify: `cargo nextest run -E 'test(gitlab_protect_tags_protects_the_recorded_pattern) or test(gitlab_protect_tags_widens_before_it_removes_the_stale_pattern) or test(gitlab_protect_tags_repairs_a_same_name_grant_by_unprotect_then_protect) or test(gitlab_protect_tags_protects_again_and_fails_when_the_repair_does_not_stand)'`

### `release-ref-rules:only-maintainers-write-a-gitlab-release-branch` — Only Maintainers write a GitLab release branch

Where the release mode is not `none`, `protect-release-branches` on GitLab MUST protect the release lines under `profile.release.line_prefix`, `release-plz-*`, and `rk-release/*`, each as one protected branch. Each MUST carry exactly one push grant and one merge grant at 40, with force pushes off. A protection with another grant MUST be deleted and created again with the owned grants.

#### Scenario: A request prefix at Developer

- GIVEN a GitLab target whose `rk-release/*` takes a push at 30
- WHEN `rk setup step protect-release-branches --apply` runs
- THEN `rk-release/*` is created again with one push grant and one merge grant at 40

Verify: `cargo nextest run -E 'test(gitlab_protect_release_branches_protects_the_line_prefix_at_maintainer) or test(gitlab_protect_release_branches_protects_both_request_prefixes_at_maintainer) or test(gitlab_protect_release_branches_repairs_a_request_prefix_with_a_developer_grant)'`

### `release-ref-rules:merge-request-pipelines-cannot-read-protected-variables` — Merge request pipelines cannot read protected variables

On GitLab, `ci-permissions` MUST set `protect_merge_request_pipelines` to `false` where the project answer carries it, so a merge request pipeline reaches no protected variable and no protected runner. Its observation MUST be satisfied only where pipelines are enabled and the setting reads `false`, or where the attribute is absent and the instance reports a version below 18.1, which the step notes. The projects API returns the attribute from GitLab 18.10, and only to a Maintainer or above, while from 18.1 a new project has the access on. So an absent attribute on an instance at 18.1 or later MUST be unsatisfied, naming 18.10, the Maintainer role, and the setting's label, "Allow merge request pipelines to access protected variables and runners", and an absent attribute with an unreadable version MUST be unknown. The step MUST write nothing where the attribute is absent.

#### Scenario: Access turned on again

- GIVEN a GitLab project with CI enabled and `protect_merge_request_pipelines` set to `true`
- WHEN `rk setup step ci-permissions --apply` runs, and the access is turned on again later
- THEN the setting reads `false`, and the next `rk setup check` fails naming `ci-permissions`

Verify: `cargo nextest run -E 'test(gitlab_ci_permissions_turns_off_protected_variables_in_merge_request_pipelines) or test(gitlab_ci_permissions_repairs_and_check_detects_protected_variable_access) or test(gitlab_ci_permissions_notes_an_instance_without_the_setting) or test(gitlab_ci_permissions_fails_a_setting_the_reader_is_not_shown) or test(gitlab_ci_permissions_leaves_an_unexposed_setting_unknown_without_a_version)'`

### `release-ref-rules:the-gitlab-bot-token-is-protected` — The GitLab bot token is protected

On GitLab, `install-bot` and `bot-secrets` MUST store `RELEASE_BOT_TOKEN` masked and protected, so only a pipeline on a protected branch or a protected tag reads it. Every landed job that reads the token runs on the trunk or a release line, and the earlier rules protect both. The observation of `bot-secrets` MUST fail a stored token that is not protected, with `rk setup --apply` as the remedy. Where no new value is supplied and no rotation runs, the step MUST protect that token in place: it reads the stored value back, because the update call requires one, stores it again with the flag, and mints nothing. Where the stored value cannot be read back, the step MUST fail with the remedy of a supplied value or a rotation.

#### Scenario: A token stored before the flag

- GIVEN a GitLab project whose `RELEASE_BOT_TOKEN` is stored masked but not protected
- WHEN `rk setup --apply` runs
- THEN `bot-secrets` stores the same value protected, nothing is minted, and the next `rk setup check` reads `bot-secrets` and `protections-check` as satisfied

Verify: `cargo nextest run -E 'test(gitlab_bot_secrets_protects_a_stored_token_in_place) or test(every_gitlab_bot_token_write_is_protected) or test(a_full_gitlab_apply_moves_the_old_shape)'`

### `release-ref-rules:protections-check-judges-the-gitlab-protections` — protections-check judges the GitLab protections

On GitLab, `protections-check` MUST judge the tag name of `protection.tag_pattern` with exactly one create grant at 40, and each release branch pattern with one push and one merge grant at 40 and force pushes off, wherever the release tags are protected. It MUST judge the merge request pipeline setting and the protected flag of `RELEASE_BOT_TOKEN` wherever the release is automatic, where `ci-permissions` and `bot-secrets` apply. A token that is not stored is the gap of `bot-secrets` alone. Each missing or wider protection MUST fail with its pattern and its step as the remedy. The Rust observation that `rk setup check` reads and the executable script MUST judge the same facts.

#### Scenario: A missing request prefix

- GIVEN a GitLab target whose `release-plz-*` is not protected
- WHEN `rk setup check` runs, and `rk setup step protections-check --apply` runs
- THEN both fail, and both name `release-plz-*` and `protect-release-branches`

Verify: `cargo nextest run -E 'test(gitlab_protections_check_fails_on_each_missing_or_wider_protection)'`
