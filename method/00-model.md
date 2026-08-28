# 00 — Model

A release is a promotion, not a push. Work integrates continuously on one branch; releasing is a separate, gated decision that automation executes and a human approves by merging one pull request.

## The spine

Six stages. No technology changes them.

1. Capture intent. Every change lands with a machine-readable statement of its release impact: Conventional Commits, or per-pull-request changeset files.
2. A bot maintains a release request. It opens a pull request against the integration branch that bumps the committed version and rewrites the changelog. Merging it publishes nothing.
3. A gate. Automation cuts a branch pinned at the merged release commit and opens it as a pull request into the release branch. Merging the gate is the release decision.
4. Tag and publish. Automation tags the merge and publishes to the registry. The tag mirrors the committed version; no hand ever authors it.
5. Build and attach artifacts. What the artifacts are is the binding's answer: a dedicated builder in its own workflow, the registry distributions themselves, or a tarball the release page carries.
6. Back-merge, so the release branch and the integration branch end equal and the next release diffs cleanly.

## The two pull requests

The two branches are `develop`, where work integrates, and `master`, which exists only to be released from. A release takes two pull requests across them.

The first is the release request the bot maintains against `develop`. It carries the version bump and the changelog entry, it is the last point a changelog correction can reach the release, and merging it publishes nothing.

The second is the gate. When the release request merges, `develop` carries a version no tag names yet, so automation cuts `release/v<version>` at exactly that commit and opens it into `master`. The head is a branch pinned to one commit, never `develop` itself: a pull request tracks its head branch, so a `develop` head would silently absorb every later push into the release and publish commits the changelog never described.

Merging the gate is what tags and publishes. `master` takes no direct push and requires a passing check, so the release decision and the quality bar sit on the same merge button.

## Why the gate publishes, not the bot

The inverted design — the bot publishes when the release request merges, and `master` is fast-forwarded onto the tag afterwards — puts the publish before the human gate, so the gate approves something already public and can no longer stop it. Gating first means nothing is public until the one reviewed merge, and a gate closed in time costs nothing to abandon.

## What a technology changes

Only four axes vary between technologies: which file states the version, which bot maintains the release request, which registry receives the publish and how it authenticates, and which tool builds the artifacts. [The diff surface](./05-diff-surface.md) names them; a binding is those four answers plus the files that wire them.
