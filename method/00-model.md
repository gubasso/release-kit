# 00 — Model

A release is a promotion, not a push. Work integrates continuously on one trunk; releasing is a decision a human makes about one pull request — per release by merging it, or once and standing, by instructing the forge to merge it the moment every required check is green — and automation executes everything after that merge.

## The spine

Five stages. No technology changes them.

1. Capture intent. Every change lands with a machine-readable statement of its release impact: Conventional Commits, or per-pull-request changeset files.
2. A bot maintains a release request. It keeps one pull request open against the trunk, carrying the version bump and the rewritten changelog, and refreshes it as work lands, so the proposed release always describes the trunk's tip. While the request is open, nothing is public.
3. Merging the release request is the release decision. The release request always integrates at the forge and requires its passing check, so the quality bar and the release decision sit on the same merge button. That holds whichever authority carried the implementations there; [integration](./12-integration.md) owns the second axis. A project makes that decision per release or once: under the trunk style's standing arm the forge holds the merge until every required check passes and merges then, so the check bar never moves — what moves is when the human answered, not whether. The arm judges the one named check alone, so that check stands for the whole workflow: a job it does not need never held a merge.
4. Tag and publish. Automation tags the push that lands the bump and publishes to the registry. The tag mirrors the committed version; no hand ever authors it.
5. Build, attest, and attach artifacts. What the artifacts are is the binding's answer: a dedicated builder in its own workflow, the registry distributions themselves, or a tarball the release page carries. Whatever they are, the run that builds them also signs a statement of where they came from, so a consumer can check the origin without trusting the page the download came from.

## The trunk

`master` is the only permanent branch and the repository default. This is trunk-based development: one branch called the trunk, and resistance to any other long-lived branch. Every change reaches it through a short-lived branch — a day or two, one author — squash-merged after its gates pass and deleted, so one implementation is one commit and the history stays linear. Which authority performs that squash is the recorded integration mode, `local` by default; [integration](./12-integration.md) owns it, and the release request merges at the forge either way. Conventional Commits are enforced on the squash title by two landed mechanisms, because the bot derives the version and the changelog from the trunk's commits: the setup asserts that the forge takes the squash message from the request's title, and a required title check holds that title to the scoped convention. Every local commit follows the same convention through the landed commit-msg hook, so the squash title is never the first conventional line an author writes. Where the forge carries a body, the request's description becomes the commit's body, and the landed content guard holds it: no reference to a git-ignored artifact, no agent attribution, with the bot's release request exempt by its title.

The trunk is always releasable. Unfinished work still lands, dark: a feature flag keeps incomplete code out of every execution path, and a refactor too large to flag proceeds by branch by abstraction. What looks like a need for a second long-lived branch is one of three needs with better answers: staging unfinished work is flags, a stabilization period is a just-in-time release branch, and a place to integrate before production is an environment. Branches isolate code; environments isolate deployment.

## The short-lived branch

The branch dies at the squash merge and its name never enters history, so the name has one job: routing while the branch is alive — telling a reviewer what the work is, and telling an issue or a ticket which branch serves it. Two forms do that job, and they compose.

- The type prefix, `<type>/<slug>`, with the type mirroring the Conventional Commit type the squash title will carry: `feat/oauth-login`, `fix/empty-csv-upload`.
- The issue-linked name, `<issue-id>-<slug>`, the shape both supported forges mint when they generate the branch from an issue. Work an issue already names takes that name, and no other: `rk issue start <issue>` mints the branch at the forge, seats it, and links it, so the branch, its pull request, and the issue's closing hang together. [The issue runbook](../runbooks/issue.md) is the procedure and the forge documents carry each forge's mechanism.

A tracker outside the forge, Jira being the common case, matches its issue keys anywhere in a branch name, so its key rides inside either form: `fix/PROJ-412-empty-csv`. Where the branch is checked out is the project's checkout mode, a linked worktree by default; [worktrees](./08-worktrees.md) owns it. Whichever form a project picks, the branch name binds nothing downstream: the squash title, not the branch name, is what the bot and the history read. The landed branch-name hook holds the routing to these two forms while the branch lives — it changes nothing about what the name binds.

## The one pull request

The bot's release request is the gate. Nothing is public until its one checked merge: merging it is what the tag, the publish, and the artifact build key on. What the request costs to stop depends on the style. An unarmed request is abandoned by closing it, with nothing to clean up, and its changelog entry can still be corrected on its branch while it is open. An armed request — the trunk style's default — is stopped by disarming it first, one command before the last check goes green, and a release that has already merged is not stopped at all: it is withdrawn, which [recovery](./04-recovery.md) owns and which costs a yank and a fix-forward. That is the trunk style's real price, and it buys a release that ships without waiting on anyone.

## The two styles

Releasing from the trunk is the default: every release ships the trunk's tip, a fix reaches users by rolling forward, and exactly one version is alive in the world. [Release from trunk](./06-release-from-trunk.md) walks one release and one bug fix end to end.

Branching for a release exists for older lines. A `release/<major>.<minor>` branch is cut just in time from a chosen trunk commit — chosen, not necessarily the tip — takes changes from the trunk only by cherry-pick, is never merged back, and is deleted once its tags pin the commits. [Branch for release](./07-branch-for-release.md) walks the whole life of one line.

| Question                                       | If yes             |
| ---------------------------------------------- | ------------------ |
| Can every user be on the same version at once? | Release from trunk |
| Do you ship several times a week or more?      | Release from trunk |
| Do customers self-host or pin versions?        | Branch for release |
| Do you owe someone a patch-only release?       | Branch for release |
| Does a sign-off gate stand before a ship?      | Branch for release |

Which style a project runs is a recorded landing parameter: `rk status` reports it, the runbooks resolve their `On trunk:` and `On lines:` variants from it, and `profile.release.style` in `.release-kit/config.toml` states the next landing’s answer, taken up by `rk upgrade --apply` or overridden by `--release-style <style>` — the same axis the checkout mode already rides. [Release lines](./09-release-lines.md) owns the second style's whole life.

Default to the trunk. Cut the first release branch the day someone actually needs a backport — retroactively, from the tag — never ahead of the need.

## The target configuration

`.release-kit/config.toml` is the target configuration: the input a person edits, typed by the domain that owns each answer. The landing verbs resolve an invocation flag, then the configured answer, then a compatible record, then observation, then a compiled default, per field. They write the resolved configuration before the manifest. The manifest records what landed, source-free, and every comparison re-renders from its values alone. Editing the configuration makes an input pending: `rk status` reports it in both modes, and `--check` keeps it informational until a landing takes it up.

| Domain              | The question it answers                                                  | Owner                            |
| ------------------- | ------------------------------------------------------------------------ | -------------------------------- |
| project identity    | Which repository is this?                                                | `[project]`                      |
| project profile     | What technologies, forge, and release intent does it have?               | `[profile]`, `[profile.release]` |
| Git workflow        | How are topic branches opened and carried into the configured trunk?     | `[git]`                          |
| capability requests | Which optional release-kit products does the target request?             | `[capabilities]`                 |
| security policy     | Where do reports go, and what response does the project promise?         | `[security]`                     |
| setup declaration   | Which otherwise applicable setup steps does the target exclude, and why? | `[setup]`                        |
| protection policy   | Which protected branch and tag patterns apply?                           | `[protection]`                   |
| landing state       | What did release-kit resolve and write?                                  | `.release-kit/manifest.json`     |

The Git workflow domain carries three answers: the trunk's name, the checkout mode that says where a topic branch opens, and the integration mode that says which authority carries it onto the trunk. [Worktrees](./08-worktrees.md) owns the second and [integration](./12-integration.md) owns the third.

The project profile states what the project is: zero or many technologies, an optional forge, and a release intent of `automatic`, `external`, or `none`. `automatic` names the driver among the technologies, the style, and the line prefix. `external` names a release the project runs through a process release-kit does not drive, so no bot-operate chapter applies to it. `none` states that nothing releases. The profile is not the setup declaration, not the Git workflow, and not the landing state: a setup exclusion, a checkout mode, and a recorded destination are answers of their own domains.

A capability is one complete release-kit product selected from the profile's dimensions and the explicit requests: `git.guards` for every target, `git.title-check` where a forge is present, `security.reporting-policy` where requested, `release.automation` at one driver and one forge where the release is automatic, and `packaging.nix`, `supply-chain.scorecard`, and `supply-chain.code-scanning` where requested. Availability belongs to a capability at its dimensions, never to a category alone. A capability whose activation a target-owned file blocks is withheld with its reason and the one edit the operator makes. [The project profile specification](../_docs/specs/SPEC-project-profile.md) binds the vocabulary, and `rk profile` reports what a target resolves to and what the catalog selects.

## What a technology changes

Only four axes vary between technologies: which file states the version, which bot maintains the release request, which registry receives the publish and how it authenticates, and which tool builds the artifacts. [The diff surface](./05-diff-surface.md) names them; a binding is those four answers plus the files that wire them.
