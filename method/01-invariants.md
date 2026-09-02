# 01 — Invariants

What holds in every binding. A tool that cannot satisfy one of these is disqualified, whatever else it offers.

## The committed version leads

One committed file states the version, and the tag mirrors it. The bot bumps that file deterministically; a tag is derived, never authored. A tool that cannot bump the committed file — one that only tags — inverts the source of truth and is out.

Under semantic versioning before 1.0.0, the minor position is the breaking position: `0.y` to `0.(y+1)` is a breaking release, and `0.y.z` to `0.y.(z+1)` is not. Intent capture and the bot's bump rules follow that mapping until the project commits to 1.0.0.

## The first publish is manual

Trusted publishing attaches to a package that already exists in the registry, so the first version is published by hand with a token scoped to publishing new versions of exactly this package, on the shortest expiry offered, and revoked as soon as the automated path is proven.

## The publish workflow is named once

The registry's trusted-publisher registration matches the repository owner, the repository name, and the workflow filename. It is branch-agnostic: changing the trigger branch needs no reconfiguration, and renaming the file breaks publishing silently until the next release attempt.

One workflow file publishes, and only that file carries the OIDC token permission. The artifact builder is a different workflow on a different trigger and is never registered: artifact generators claim generic filenames, so the publish workflow is named for the tool that performs the publish.

## A published version is immutable

The registry refuses a second upload of the same version, and a moved tag serves two payloads under one name. A defect ships as the next version, and the defective one is withdrawn so new consumers stop resolving to it. [Recovery](./04-recovery.md) owns the sequence.

## Trunk is written through pull requests only

`master` takes no direct push and no force-push, requires the named passing check, and merges only by squash, so one pull request is one commit and the history stays linear. Nothing in the pipeline writes the branch outside a merge, so the ruleset names no bypass actor; the bot's bump rides the same merge button as everyone's work.

## The trunk is always releasable

Any trunk commit can be released, at any moment, because unfinished work ships dark: a feature flag keeps incomplete code out of every execution path, and a change too large to flag proceeds by branch by abstraction. A habit that needs a stabilization branch to make the trunk trustworthy is treating the symptom; the check that gates every merge is what makes the trunk trustworthy.

## A release line flows one direction

Where an older line exists, changes reach its `release/<major>.<minor>` branch from the trunk by cherry-pick, and nothing merges back: a fix lands on the trunk first, then crosses alone. A tag outlives its branch — the branch is deleted once its tags pin the commits, never merged to die. An rc tag on a line publishes to no registry; it exists to build the artifacts a human validates.

## Enforcement is a separate switch

Configuring a trusted publisher permits OIDC publishing; requiring trusted publishing rejects every token publish. Enforcement is turned on only after one automated release has proven the OIDC path, and it is what an emergency hand-publish must first turn off — [recovery](./04-recovery.md) carries that conflict.

## Provenance rides the publish identity

Trusted publishing answers who may publish. A build attestation answers what was built, binding the artifact's digest to the workflow, repository, and commit that produced it. These are two questions, and configuring the first does not answer the second: a channel that authenticates the upload and stores no attestation leaves a consumer nothing to verify.

The identity that authorizes the publish is what signs the artifacts, in the same run that built them. A binding takes the provenance its channel already offers, by default and without ceremony, and never stands up a second signing scheme with its own keys and its own rotation to compensate.

The floor is a build-provenance attestation over every artifact a consumer downloads. It is provenance and not safety: it says where a file came from, never that the code in it is sound. Where a channel offers no attestation at all, the binding states that rather than letting checksums imply a guarantee they do not carry. The level each channel reaches is named rather than left ambiguous: GitHub artifact attestations and GitLab's signed runner statement are both SLSA Build Level 2, and Build Level 3 stays out of reach on either builder — it requires provenance the build's own steps cannot forge, signing material they cannot reach, and isolated, ephemeral build environments, which an ordinary workflow on either platform does not get.
