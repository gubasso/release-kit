# 02 — Setup

Bootstrapping one repository onto the convention. Once per repository, in this order; every step names what it proves before the next one starts. The binding for the project's technology supplies the concrete tools, files, and registry; `rk init` lands the deterministic files. The command form of this chapter is the setup runbook, `rk guide setup`, and `rk setup` executes the forge-side steps.

## 0. Gate the package metadata

Run the registry's dry-run packaging check first, before anything that needs credentials. It catches the common rejects — a missing description, an invalid category — with no token and no remote configuration, and every later step assumes the package is publishable.

## 1. Make the trunk the sole long-lived branch

Make `master` the repository default, so the bot's release request targets it with no configuration, and the only long-lived branch: merge in and delete every other one. Work that kept a second branch alive lands on the trunk behind a flag instead. Then have the forge delete a branch when its merge lands, so keeping the trunk sole costs no one a habit. Locally, a post-merge reminder hook closes the same loop from the clone's side: after a pull, `rk branches prune` names the merged branches the clone still holds, and deleting one stays the operator's action.

## 2. Let automation act

Grant the repository's CI permission to write and to open pull requests. Provide a bot identity — on GitHub, an App installed on the repository — and store its credentials as repository secrets. The bot identity is what makes the tag push retrigger workflows: a tag pushed with the default CI token starts nothing, which silently skips the artifact build.

## 3. Protect the trunk and the tags

Two protections owned by every repository, and a third where older lines exist, all held by configuration that a script can verify:

- `master` takes no direct push and no force-push, requires a pull request carrying the named passing check and the landed title check, and offers squash as the only merge method, with the squash message taken from the request's title — asserted, because the forge's one-commit fallback is the branch commit's own subject.
- Where the forge requires the title check by name and runs the trunk's own copy of it — GitHub — land step 4 and merge it first, then apply this protection: requiring the check before it exists on the trunk blocks the very request that would land it. A rerun updates the protection in place. The forge that requires the whole pipeline instead needs no ordering, and what its pipeline-borne gate enforces is stated in that forge's document.
- Release tags are immutable: `v*` can be neither moved nor deleted, and the pattern already covers the rc tags a release line mints.
- Where a project keeps older lines, `release/*` cannot be force-pushed or deleted while a line is alive; deletion becomes safe only once the line's tags pin its commits.

## 4. Land the workflow files

`rk init` lands the binding's files: the bot configuration, the publish workflow, the artifact-builder configuration, the title check, and the commit-contract hook block spliced into `.pre-commit-config.yaml`. The landing takes `--scopes`, the Conventional Commit scopes the project accepts, and renders them into the title check, the commit hook, and the routing block. Fill the sentinel placeholders it reports, install the hook stages the block's first line names, and hold the invariant that the publish workflow filename is the one the registry will be told. The hooks and step 3's forge protections are the same rules at two distances: the forge enforces, the hooks give the refusal at the desk, and `--no-verify` bypassing a hook changes what an author sees, never what the forge accepts.

## 5. Publish the first version by hand

Trusted publishing attaches to an existing package, so the first version goes up with a token: scoped to publishing new versions of exactly this package, shortest expiry, created for this step.

## 6. Register the trusted publisher

Register owner, repository, and the publish workflow's filename with the registry. Then revoke the bootstrap token, so the package has exactly one publishing path.

## 7. Prove the automated path

Cut one release end to end through [operate](./03-operate.md). Its verify step passing — the registry serves the new version, the tag and the trunk name the same commit, and the provenance verifies where the binding declares it — is the proof the next step depends on.

## 8. Require trusted publishing

Turn on the registry's enforcement, now that one OIDC release has proven the path. From here every token publish is rejected; the hand-publish escape in [recovery](./04-recovery.md) starts by turning this off.
