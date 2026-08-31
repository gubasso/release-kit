# 00 — Model

A release is a promotion, not a push. Work integrates continuously on one trunk; releasing is a separate decision a human approves by merging one pull request, and automation executes everything after that merge.

## The spine

Five stages. No technology changes them.

1. Capture intent. Every change lands with a machine-readable statement of its release impact: Conventional Commits, or per-pull-request changeset files.
2. A bot maintains a release request. It keeps one pull request open against the trunk, carrying the version bump and the rewritten changelog, and refreshes it as work lands, so the proposed release always describes the trunk's tip. While the request is open, nothing is public.
3. Merging the release request is the release decision. The trunk takes no direct push and requires its passing check, so the quality bar and the release decision sit on the same merge button, and closing the request abandons a release at no cost.
4. Tag and publish. Automation tags the push that lands the bump and publishes to the registry. The tag mirrors the committed version; no hand ever authors it.
5. Build, attest, and attach artifacts. What the artifacts are is the binding's answer: a dedicated builder in its own workflow, the registry distributions themselves, or a tarball the release page carries. Whatever they are, the run that builds them also signs a statement of where they came from, so a consumer can check the origin without trusting the page the download came from.

## The trunk

`master` is the only permanent branch and the repository default. This is trunk-based development: one branch called the trunk, and resistance to any other long-lived branch. Every change reaches it through a short-lived branch — a day or two, one author — squash-merged after review and CI and deleted, so one pull request is one commit and the history stays linear. Conventional Commits are enforced on the squash title, because the bot derives the version and the changelog from the trunk's commits.

The trunk is always releasable. Unfinished work still lands, dark: a feature flag keeps incomplete code out of every execution path, and a refactor too large to flag proceeds by branch by abstraction. What looks like a need for a second long-lived branch is one of three needs with better answers: staging unfinished work is flags, a stabilization period is a just-in-time release branch, and a place to integrate before production is an environment. Branches isolate code; environments isolate deployment.

## The short-lived branch

The branch dies at the squash merge and its name never enters history, so the name has one job: routing while the branch is alive — telling a reviewer what the work is, and telling an issue or a ticket which branch serves it. Two forms do that job, and they compose.

- The type prefix, `<type>/<slug>`, with the type mirroring the Conventional Commit type the squash title will carry: `feat/oauth-login`, `fix/empty-csv-upload`.
- The issue-linked name, `<issue-id>-<slug>`, the shape both supported forges mint when they generate the branch from an issue. Prefer letting the forge mint it — one command links the branch, its pull request, and the issue's closing, and the forge document carries that command.

A tracker outside the forge, Jira being the common case, matches its issue keys anywhere in a branch name, so its key rides inside either form: `fix/PROJ-412-empty-csv`. Whichever form a project picks, the branch name binds nothing downstream: the squash title, not the branch name, is what the bot and the history read.

## The one pull request

The bot's release request is the gate. Nothing is public until its one reviewed merge: the changelog entry can still be corrected on the request's branch while it is open, merging it is what the tag, the publish, and the artifact build key on, and a release abandoned before the merge is a closed pull request with nothing to clean up.

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

Default to the trunk. Cut the first release branch the day someone actually needs a backport — retroactively, from the tag — never ahead of the need.

## What a technology changes

Only four axes vary between technologies: which file states the version, which bot maintains the release request, which registry receives the publish and how it authenticates, and which tool builds the artifacts. [The diff surface](./05-diff-surface.md) names them; a binding is those four answers plus the files that wire them.
