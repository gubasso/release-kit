# Land a workflow without a release

## Context and Problem Statement

Some projects want the Git workflow of the convention and no release. `release.mode = "none"` already lands no version file, no changelog, and no release pipeline. The landing still wrote a title gate, a tag hook, a routing block and a glossary that give versions and changelogs as reasons, and configuration tables that never apply. Under local integration no hook refused a hand commit on the trunk, and the forge setup still asked for tag protection and a required pipeline.

## Considered Options

- `a preset flag, and a release-less landing with no release text` — chosen.
- `a new value of a mode key that names the workflow-only profile` — rejected: the key would have one legal value, which `git:the-method-is-trunk-based` refuses for a configuration key.
- `an empty forge, so no forge text lands` — rejected: it also drops forge pruning and `rk issue start`, which a release-less project still uses.

## Decision Outcome

Chosen option: `a preset flag, and a release-less landing with no release text`. `--workflow-only` on `rk init` and `rk upgrade` sets `release.mode = "none"`, `git.integration = "local"`, `git.checkout_mode = "linked-worktree"`, and `capabilities.workflow_guards = true`, and records nothing new.

Under `release.mode = "none"`, the routing block takes a workflow variant, the glossary drops its release term, the tag hook does not land, a commit scope is preferred and not required, and the setup requires no tag protection. The title gate lands only under forge integration or an automatic release. Under local integration the trunk guard lands at the pre-commit stage, with pre-push guards against a force push and a merge commit on the trunk. Where no forge gate lands, the forge requires no status check and no pipeline.

Enforced by `project-profile:the-workflow-only-preset-expands-to-recorded-answers`, `project-profile:a-release-less-landing-carries-no-release-text`, `maintenance:local-integration-guards-the-trunk`, and `target-config:the-effective-protection-follows-the-profile`.

## Consequences

- Good: one command lands the workflow of the convention and nothing of the release.
- Good: the trunk guard holds under local integration, as the worktree mode promised.
- Bad: a release-less project that later adds a release changes several answers, not one.

## Status

Accepted
