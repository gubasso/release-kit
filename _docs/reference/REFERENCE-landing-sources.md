# Landing Sources

External sources behind `SPEC-landing.md`: how comparable tools record what they generated into a project, how they judge whether it is still theirs, and what each one does when it is not. Each entry states what the source says and which rule it bears on.

Verified against the listed sources on 2026-08-28 and re-checked on 2026-08-29; the arming entries verified on 2026-09-03.

## cargo-dist, on generated files that refuse to drift

`dist init` is designed to be rerun repeatedly, preserving settings while handling updates and migrations. `dist generate --check` errors if generating would change the file's contents, ignoring newline style, and most commands run that check on startup, so an out-of-date or hand-edited workflow file is an error rather than a surprise. `allow-dirty = ["ci"]` is the documented escape for someone who genuinely must hand-edit.

- <https://github.com/axodotdev/cargo-dist>
- <https://axodotdev.github.io/cargo-dist/>
- <https://github.com/axodotdev/cargo-dist/blob/main/CHANGELOG.md>

Bearing: `landing:a-rendered-file-is-reproducible` and `landing:an-upgrade-refuses-on-owned-drift`. This is also the exemplar release-kit already teaches its own users about in `bindings/rust.md`, so adopting the same shape for its own landed files is consistent with what it asks of them. The general stance is the load-bearing part: a tool that generates a file should be able to say whether that file is still what it generated, which is what the recorded digests are for.

## projen, on markers, anti-tamper, and the sweep

Generated files carry a magic marker, and any file carrying it is cleaned up automatically once it is no longer generated. Most generated files are marked read-only, and an anti-tamper check runs in CI to confirm they were not modified during a build.

- <https://github.com/projen/projen>
- <https://projen.io/docs/introduction/the-projen-workflow/>

Bearing: the sweep-on-drop behaviour release-kit already implements for skills as `distribution:an-install-sweeps-what-the-payload-dropped`, arrived at independently, and the argument that a landing record is what would make the same sweep possible for landed files.

The read-only-file approach is the half not worth copying. Landed files carry sentinels the operator must fill, so they are explicitly not read-only — which is also why `landing:a-dropped-file-stays` is the opposite choice from projen's, and deliberately so.

## copier and cruft, on the receipt living in the generated project

copier writes an answers file into the generated project holding both the answers and the template version that produced it. Its update regenerates from the current template using those answers, diffs that against the actual project to extract local changes, applies migrations, then re-applies the local modifications, leaving conflict markers where both sides moved. cruft writes a record whose two load-bearing fields are the template and the commit.

- <https://copier.readthedocs.io/en/stable/updating/>
- <https://cruft.github.io/cruft/>

Bearing: `landing:a-landing-leaves-a-record` and `landing:a-record-states-its-schema`. The shape of what this family records is unanimous and worth naming: an opaque identity, a commit or a tag, never a per-file version.

Warning worth carrying: copier documents that hand-editing the answers file tricks the tool into believing the wrong version generated the project. A record needs the same warning and, better, a way to detect the tampering rather than only forbidding it, which is what the per-file digests give `rk status --check`.

## pre-commit, on immutable pins and an explicit update verb

The revision field must point at a fixed tag; mutable references are unsupported and are never updated after first install. The autoupdate verb is the explicit action that rewrites the configuration to the latest released versions and converts a mutable revision to an immutable one. Versions of additional dependencies must still be updated by hand.

- <https://github.com/pre-commit/pre-commit/issues/1354>
- <https://github.com/pre-commit/pre-commit/issues/3521>
- <https://docs.renovatebot.com/modules/manager/pre-commit/>

Bearing: the discipline `versions.toml` already applies, and the argument that a landing record holds an immutable identity plus an explicit update command — `rk upgrade` — rather than a floating reference that quietly re-resolves.

## Terraform, on a committed lock file of versions and checksums

Project configuration records the exact selected versions and their checksums in a lock file that is committed and reviewed like any other change. The shared cache is an optimization, not the record.

- <https://developer.hashicorp.com/terraform/language/files/dependency-lock>
- <https://developer.hashicorp.com/terraform/cli/commands/providers/lock>

Bearing: the digest-bearing record, and the separation between a materialization cache, which is regenerable and authoritative for nothing, and a record that must be believed. It is also the precedent for committing the record rather than treating it as local state.

## Adoption, and why none of it is blind

Import brings pre-existing infrastructure under management by recording it rather than recreating it. A provider option exists specifically to take ownership of a resource that already exists, and a Helm proposal covers the same ground through server-side apply. No member of this family adopts by overwriting what it finds.

- <https://helm.sh/community/hips/hip-0023/>
- <https://developer.hashicorp.com/terraform/cli/commands/plan>

Bearing: `landing:an-adoption-writes-the-record-and-nothing-else`. Adoption records both digests for a file the target may edit, refuses outright on a mismatch in a file the payload owns, and changes no target file — which is the strictest reading of this family's shared rule rather than a departure from it.

## pre-commit, on the stages and environment the landed hooks lean on

Verified 2026-09-01. A hook declares its `stages`, and a stage's hooks run only where that hook type is installed — `pre-commit install --hook-type commit-msg --hook-type pre-push` — which `default_install_hook_types` makes the default for a repository. Pre-push hooks receive `PRE_COMMIT_REMOTE_BRANCH` carrying the full remote ref being pushed, so a local hook can refuse a push to `refs/heads/master` or a `refs/tags/v*` tag; pushes that delete a ref intentionally skip the hooks (pre-commit issue 3050), and git tells a pre-push hook nothing about `--force`, so a force-push has no local mirror. `pre-commit/pre-commit-hooks` ships `no-commit-to-branch`, protecting `main` and `master` by default with `--branch` and `--pattern` overrides; it reads the current branch rather than a commit event, so a `pre-commit run` sweep over a checked-out protected branch fails the same way a commit would. The `SKIP` environment variable, a comma-separated list of hook ids, is pre-commit's documented way to skip named hooks for one invocation and reports them as skipped rather than silently omitting them, which is how a CI sweep keeps the guard out of a context that commits nothing.

`compilerla/conventional-pre-commit` checks a commit message against Conventional Commits at the `commit-msg` stage, with `--strict`, `--force-scope`, and a comma-delimited `--scopes` list; `crate-ci/committed` offers `allowed_scopes` but no option to require a scope, which is what decided between them.

- <https://pre-commit.com/#pre-commit-configyaml---top-level>
- <https://pre-commit.com/#pre-push>
- <https://pre-commit.com/#temporarily-disabling-hooks>
- <https://github.com/pre-commit/pre-commit/issues/3050>
- <https://github.com/pre-commit/pre-commit-hooks>
- <https://github.com/compilerla/conventional-pre-commit>
- <https://github.com/crate-ci/committed/blob/master/docs/reference.md>
- <https://git-scm.com/docs/githooks>

Bearing: `landing:a-landed-hook-serves-the-release-convention-alone`. Every mirror the block carries rests on a documented mechanism above, the two honest limits — `--no-verify` and the invisible force-push — are stated by the same sources, and the two third-party hooks are pinned in `versions.toml` like every other snippet pin.

## Arming the release request

Three upstream facts carry the arming steps the landed release workflows render. GitHub states that with the exception of `workflow_dispatch` and `repository_dispatch`, events triggered by `GITHUB_TOKEN` do not create workflow runs at all, so an arm made with the default token merges a bump that starts no publish. The release-plz action at the pinned commit declares a `pr` output — the release request it opened or refreshed, a JSON object carrying `number`, `head_branch`, `base_branch`, and `html_url` — and release-plz refreshes by force-push on GitHub and by closing the outdated request and opening a fresh one on GitLab, which is why an arm is re-applied on every run. The release-please action sets its outputs dynamically rather than in `action.yml`: `prs_created` is true if any pull request was created or updated, and `pr` is a JSON string of the PullRequest object, unset when none exists.

- <https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows>
- <https://release-plz.dev/docs/github/output>
- <https://github.com/release-plz/action/blob/2eb1d8bcb770b4c48ccfaad919734b38b51958c9/action.yml>
- <https://github.com/googleapis/release-please-action/blob/45996ed1f6d02564a971a2fa1b5860e934307cf7/README.md>

Bearing: `landing:the-arming-identity-is-the-bot`, both scenarios, and the arming steps in every landed release workflow. The default-token fact is the single most load-bearing citation in the arming design: it is why the arm sits in the job that already mints the bot token.
