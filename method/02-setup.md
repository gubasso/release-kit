# 02 — Setup

This chapter brings one repository onto the convention. Run it once per repository, in this order. Every step names what it proves before the next one starts. The binding for the project's technology supplies the concrete parts:

- The tools.
- The files.
- The registry.

`rk init` lands the deterministic files. The setup runbook, `rk guide setup`, is the command form of this chapter, and `rk setup` executes the forge-side steps.

Before the first step, the project decides how it obtains `rk`. One choice is one host install for the whole machine. The other is its own devshell pin: release-kit as a flake input at a release tag. The tag in `flake.nix` is the version, and the lock is its content. `rk self-depend sync --apply` moves the two forward together when the operator runs it. The `.envrc` line only checks once a week for a newer release and warns. One project runs one bump mechanism. Two mechanisms write the same two files on the same trigger with two locks that do not know each other. The second one to run either fights the first or silently undoes it. So the wiring replaces what the project carried. `rk self-depend clean` removes what a line scan can judge and names the rest. A target reads as ready only when nothing of the predecessor is left.

The setup asks for the decision and does not assume it. The decision is two questions, not one: which manager the project runs, and what moves the pin that manager records. This chapter exists to prevent two states: a pin that nothing moves, and a project with no pin at all. A setup that never raises either question reaches them through the supported path. So a setup asks both questions once and records the answers. A refusal is a decision that the project made, never a step that nobody reached.

The profile answers which steps apply before the operator does. A step reads the resolved target configuration:

- A forge step applies only where a supported forge adapter exists.
- The release half applies only where the release is automatic and its automation is available.
- The title gate or the commit gate applies where the catalog selects one.
- The reporting channel applies where the reporting policy is requested.
- The tag protection applies wherever a forge adapter exists and the release mode is not `none`.

The report shows a step that does not apply as not applicable, with the profile value that decided it, and with no operator reason. An exclusion is a different statement. It names a step that does apply and that this project chooses not to run, with the reason that a report prints. An exclusion for a step that does not apply is redundant. The report says so, and the step does no work.

A project can run part of this chapter instead of all of it. The common shape takes the trunk half and none of the release half, because the project publishes nothing. The trunk half has three parts:

- One permanent branch.
- A short-lived branch per change.
- A merge back in the recorded form.

Such a project names the steps it does not run in its own committed configuration, each with its reason. The check then judges the model that the project stated. It reports every excluded step with its reason, and the verdict counts the rest. The project states an exclusion and does not drop the step. A reader must be able to tell a chosen subset from an incomplete setup. A check that can never read clean stops being a gate. A real regression then arrives as one more line in a report that already carries several permanent ones. An exclusion changes no policy: the floors below bind every step that the project still runs.

Keep a complete observation. `rk setup checkpoint` observes every step the way `rk setup check` does. Only where every applicable step holds, it writes `.release-kit/setup-proof.json`. The file records these facts:

- Which setup contract was observed.
- When it was observed.
- Which `rk` observed it.
- The normalized result of each step.

It holds no secret and no path of the machine that ran it. The project commits it beside the setup that it proves.

Five records then answer five questions, and none substitutes for another:

- The configuration is the desired state.
- The landing record is what landed.
- The proof is the last complete observation.
- The run journal is what one run did on one host.
- `rk setup check` is what the forge answers now.

`rk setup status` judges the proof against the target offline. So a runtime that cannot hold the bot's key still learns whether the last complete observation describes this target. A step that this runtime cannot observe is unknown, which proves nothing wrong. The proof never becomes current truth. A check that holds the credentials and finds a step wrong supersedes it. Nobody can read a forge-stored workflow secret back either. So a green release run corroborates the setup but does not prove every step.

Only a committed proof closes the setup. On a target with an automatic release and a forge, `rk status --check` reads the proof offline. It counts an absent, stale, or invalid proof as a violation. So the gap meets the operator at the commit that can move the setup contract, not in a release that never comes.

## 0. Gate the package metadata

Run the registry's dry-run packaging check first, before anything that needs credentials. It catches the common rejects, such as a missing description or an invalid category, with no token and no remote configuration. Every later step assumes that the package is publishable.

The same gate asks a second question: does the artifact that a consumer downloads carry the reporting policy? A consumer who has only the package cannot follow a policy that is readable on the forge alone. The gate answers the question only where the binding has a listing command that names the published files with no credentials. Where the binding has none, the gate says so plainly. The packaging result stands, and the report names the inclusion as unproved, together with the release-time inspection that settles it. A proxy success is worse than the gap, because it retires the manual check without a replacement. Each binding states which of the two it has.

## 1. Make the trunk the sole long-lived branch

Make `master` the repository default, so that the bot's release request targets it with no configuration. Make it the only long-lived branch. Merge every other branch in. Then delete each one. Work that kept a second branch alive lands on the trunk behind a flag instead. Under the workflow guards, have the forge delete a branch when its merge lands. Keeping the trunk sole then costs no one a habit. Also have a post-merge reminder hook close the same loop from the side of the clone. After a pull, `rk branches prune` names the merged branches that the clone still holds. Deleting one stays the operator's action. Without the guards, branch cleanup is the project's own workflow.

## 2. Let automation act

Grant the repository's CI permission to write and to open pull requests. Provide a bot identity. Store its credentials as repository secrets. On GitHub, the identity is an App installed on the repository. On GitLab, it is a project access token, or a service account that is a member of this one project on a GitLab.com Free group. The bot identity makes the bot's pushes start workflows. A push with the default CI token starts nothing. So a release request that it opens runs no checks, and a tag that it pushes silently skips the artifact build.

## 3. Protect the trunk and the tags

Every repository owns two protections, with a merge-permission switch and a vulnerability intake channel beside them. A third protection applies where older lines exist. Configuration that a script can verify holds all of them:

The recorded integration mode selects which trunk protection this step installs. The reason, stated once: no forge can require a check before the push that starts it. Under `forge` integration, the whole list below applies. Under `local` integration, the trunk keeps every rule. The forge names who can push past the request rule and the required-check rule, which cannot fire on a push. On GitHub, that is the repository-administrator role, recorded as the one bypass of the ruleset that carries those two rules. On GitLab, it is the narrowest push access level that admits the push. Deletion and force-push stay in protection that no actor bypasses, the administrator included. Where a release exists, every tag protection is identical in both integration modes, because the release request integrates at the forge either way. Only the authority that holds that request until its check is green moves. [Integration](./12-integration.md) owns the axis.

A project with no release has no release request and no tag to protect. Under `release.mode = "none"`, the tag protection does not apply. Under local integration with no automatic release, no forge gate lands. So the GitHub trunk ruleset requires no status check, and the GitLab trunk requires no pipeline. No-deletion and no-force-push still hold.

- `master` takes no direct push and no force-push. It requires a pull request that carries the named passing check and the landed title check. It offers the merge method of the recorded merge form as the only one. The protection names one check. So a multi-job workflow names a gate that needs every job whose failure must hold the merge. A proof in a second workflow file is one that the gate cannot need. So a technology whose tooling generates its own workflow configures it to report on no request, and the project's gate runs the proof. Under `squash`, the squash message comes from the request's title. The setup asserts this, because the forge's one-commit fallback is the subject of the branch commit. Under `rebase`, the forge takes a rebase or fast-forward merge with a linear history. A commit gate that judges every commit of the request replaces the title check.
- On GitHub, the forge requires the title check by name and runs the trunk's own copy of it. There, land step 4 first. Merge it. Then apply this protection. A required check that does not exist on the trunk yet blocks the very request that lands it. A rerun updates the protection in place. The forge that requires the whole pipeline instead needs no ordering. That forge's document states what its pipeline-borne gate enforces.
- The forge's own version is a precondition of this protection. The convention's landed pipeline can need a feature that the forge added in a known version. An instance below that version cannot honor the merge check that this section installs. So the setup reads the version first. It refuses the protection instead of installing one that the pipeline cannot feed. A version that the setup cannot read blocks the same as one that is too old. A floor that nobody can read proves nothing either way. That forge's document names the version and the feature.
- The trunk's requests can merge themselves once every required check passes. The trunk style's standing release instruction needs this switch. The setup asserts it and does not assume it, because a repository that forbids it fails silently: the request never offers the option. A forge that carries no such switch says so, and its pipeline requirement stands in.
- Where the project releases, release tags are immutable. Nobody can move or delete `v*`, and the pattern already covers the rc tags that a release line mints.
- Where a project keeps older lines, nobody can force-push or delete `release/*` while a line is alive. Deletion becomes safe only once the line's tags pin its commits.
- A vulnerability report needs both the forge's private channel and the reporting policy that step 4 lands. A setting gives a reader no instructions, and a file alone enables no channel. The channel does not apply to private GitHub repositories. A forge that proves less names its limitation. The forge documents name the guarantees, and `rk guide setup` step 3g orders the check.

## 4. Land the workflow files

`rk init` lands the binding's files:

- The bot configuration.
- The publish workflow.
- The artifact-builder configuration.
- The reporting policy.
- The title check.
- The hook block spliced into `.pre-commit-config.yaml`.

[Landing](./13-landing.md) owns how the write reaches the repository:

- The stage.
- The investigation.
- The fresh render.
- The verification.
- The cleanup.

This step owns what the landing carries and why. It lands no CI workflow. The title check is the one landed file that reports on a request, because it is the second context that the protection requires by name. Every other proof is a job of the project's own gated pipeline, which the binding serves and the project writes.

The landing records no scope vocabulary. The title check holds a scope to one shape, made of these characters:

- Lowercase letters.
- Digits.
- `_ . / -`.

A project's areas grow without a re-landing. A new scope is a decision at the desk, not a round trip through release-kit.

A landing lands what the release process needs and nothing that tells the project how to work. The rules that only shape the project's workflow are one opt-in, the workflow guards. They are these:

- The agent and commit policy lines of the routing block.
- The glossary.
- The commit-message, branch-name, worktree, and trunk hooks.
- The merge cleanup.
- The reminder of step 1.

A fresh landing takes none of them. `rk upgrade --workflow-guards on` or `off` changes the answer later. A target that landed before the opt-in existed keeps them until its operator answers.

The landing records three more values without a question, each with a default:

- The checkout mode stays a configuration value, `linked-worktree` by default. It sets where the helper verbs open a topic branch and, under the workflow guards, whether the location hook lands. [Worktrees](./08-worktrees.md) owns the choice.
- The release style `trunk`, the default, arms the bot's request so that a green trunk ships itself. `lines` keeps the merge a per-release act, because a human validates a line's candidate. [Release lines](./09-release-lines.md) owns the choice, and `rk upgrade --release-style <style>` changes it later.
- The integration mode `local`, the default, renders a block that admits the trunk move and the trunk push that `rk integrate` needs. Under the workflow guards, the block refuses these:
  - A hand commit on the trunk.
  - A force push to it.
  - A merge commit pushed onto it.

  `forge` renders the block that refuses the trunk commit and the trunk push. [Integration](./12-integration.md) owns the choice, and `rk upgrade --integration <mode>` changes it later.

It records the merge form the same way: `squash`, the default, or `rebase`. The rebase form keeps every commit of a branch and lands a commit gate in place of the title check where a forge gate lands. The title check lands only under forge integration or an automatic release.

Fill the sentinel placeholders that the landing reports. Install the hook stages that the block's first line names. Hold the invariant that the publish workflow filename is the one the registry will be told. The landed files reach the trunk through the project's own workflow. Release-kit prescribes none of these for them:

- A commit.
- A push.
- A branch.

Under the workflow guards, the hooks and the forge protections of step 3 are the same rules at two distances. The forge enforces, and the hooks give the refusal at the desk. `--no-verify` bypasses a hook and changes what an author sees, never what the forge accepts.

The project owns everything in `.pre-commit-config.yaml` outside the marked block. It assigns each of its own checks to the earliest native `pre-commit` stage that can run it. It tags the suite that must pass before an integration with the `pre-merge-commit` stage. Its own continuous integration runs those same checks. Release-kit asks git to run the clone's `pre-merge-commit` hook, which the project's hook manager installs. Release-kit names no hook runner. It reads none of these:

- A hook identifier.
- A revision.
- A test category.

So the stage assignment stays the project's. Release-kit lands no CI workflow and judges no parity between the two. The convention is prose. The `rk-setup` skill researches the current hooks for the project's technologies, and an agent holds the rule at authoring time.

A project with no release takes this step without its release half. `rk init --workflow-only` sets these in one flag:

- `release.mode = "none"`.
- Local integration.
- The `linked-worktree` checkout mode.
- The workflow guards.

The landing then carries no version, changelog, or tag text. The other parts change too:

- The routing block takes its workflow variant.
- The glossary names no release term.
- The tag hook does not land.

A commit scope is preferred there and not required, because no changelog reads it. [Landing](./13-landing.md) owns the preset.

## 5. Publish the first version by hand

Trusted publishing attaches to an existing package, so the first version goes up with a token. Scope the token to publishing new versions of exactly this package. Give it the shortest expiry. Create it for this step.

## 6. Register the trusted publisher

Register these with the registry:

- The owner.
- The repository.
- The publish workflow's filename.

Then revoke the bootstrap token, so that the package has exactly one publishing path.

## 7. Prove the automated path

Prove the protected path first. Take one real change to the trunk through the path that the recorded integration mode names. Merge it in the recorded form. The path is a local integration that an administrator pushes, or a request whose required checks pass. This proves that the protection of step 3 admits the path that every later change takes. Then cut one release end to end through [operate](./03-operate.md). The next step depends on its verify step passing. The registry must serve the new version, and the tag and the trunk must name the same commit. The provenance must verify where the binding declares it. Then record the setup proof from a host that holds the bot's key. Commit the proof.

## 8. Require trusted publishing

Turn on the registry's enforcement, now that one OIDC release proved the path. From here, the registry rejects every token publish. The hand-publish escape in [recovery](./04-recovery.md) starts by turning this off.
