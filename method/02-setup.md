# 02 — Setup

Bootstrapping one repository onto the convention. Once per repository, in this order; every step names what it proves before the next one starts. The binding for the project's technology supplies the concrete tools, files, and registry; `rk init` lands the deterministic files. The command form of this chapter is the setup runbook, `rk guide setup`, and `rk setup` executes the forge-side steps.

## 0. Gate the package metadata

Run the registry's dry-run packaging check first, before anything that needs credentials. It catches the common rejects — a missing description, an invalid category — with no token and no remote configuration, and every later step assumes the package is publishable.

## 1. Shape the branches

Make `develop` the default branch, so pushes to it drive the release bot. Create `master` at `develop`'s tip and remove any other long-lived branch, before the protections exist: creating a branch is a direct push, which the protected `master` refuses.

## 2. Let automation act

Grant the repository's CI permission to write and to open pull requests. Provide a bot identity — on GitHub, an App installed on the repository — and store its credentials as repository secrets. The bot identity is what makes the tag push retrigger workflows: a tag pushed with the default CI token starts nothing, which silently skips the artifact build.

## 3. Protect the branches and the tags

Three protections, all held by configuration that a script can verify:

- `master` takes no direct push, requires the passing check the gate shows, and merges only as a merge commit.
- `develop` cannot be force-pushed or deleted. It carries no required status check, because one would also reject the push that opens a release; the release is gated on `master` instead.
- Release tags are immutable: `v*` can be neither moved nor deleted.

## 4. Land the workflow files

`rk init` lands the binding's files: the bot configuration, the publish workflow, and the artifact-builder configuration. Fill the sentinel placeholders it reports, and hold the invariant that the publish workflow filename is the one the registry will be told.

## 5. Publish the first version by hand

Trusted publishing attaches to an existing package, so the first version goes up with a token: scoped to publishing new versions of exactly this package, shortest expiry, created for this step.

## 6. Register the trusted publisher

Register owner, repository, and the publish workflow's filename with the registry. Then revoke the bootstrap token, so the package has exactly one publishing path.

## 7. Prove the automated path

Cut one release end to end through [operate](./03-operate.md). Its verify step passing — the registry serves the new version, the tag exists, the branches agree — is the proof the next step depends on.

## 8. Require trusted publishing

Turn on the registry's enforcement, now that one OIDC release has proven the path. From here every token publish is rejected; the hand-publish escape in [recovery](./04-recovery.md) starts by turning this off.
