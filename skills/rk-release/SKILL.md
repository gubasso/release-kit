---
name: rk-release
description: Operates a release in a project that carries the release-kit workflow, from landing the work to verifying the published version, including changelog correction and recovery. Use when asked to cut a release, ship a version, fix a changelog entry before release, recover a stuck or failed release, backport a fix to an older line, yank a version, or hand-publish while CI is down. Triggers include release, cut a release, publish a version, backport, yank, and release recovery.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Operating a release needs push access to the repository and its forge CLI authenticated.
---

# rk-release

Cut a release through the release-kit convention. The sequence is `rk method operate`; the technology's concrete commands are in `rk binding <tech>`; when something goes wrong, `rk method recovery` owns the way back.

## Before acting

Read `~/.local/state/release-kit/skills/shared/plan-gate.md` before the first action of a task, and hold it for the whole task. It binds three phases: plan and present the plan for approval, validate that plan against every preview and read-only source phase 2 names, then execute it.

The gate is why this skill is safe to run: every verb below writes files, changes a forge, or publishes a version, and the gate states which of those steps stay the operator's own.

When the request carries `--no-plan`, skip the approval turn only. Still state the ordered plan before acting, and still validate it as phase 2 directs.

## The shape

One pull request. The bot maintains the release request against the trunk — the version bump and the changelog — and merging it is the release: the bump push tags and publishes. Never author a tag, never push the trunk directly, never re-author a published version.

## Cut a release

1. Read the sequence once per session: `rk method operate`. Then follow `rk guide release`, which renders it as commands with the project path and technology filled in and the forge's variants selected.
2. Run `rk status --check --target .` before anything ships: drift on a file release-kit owns, or an unfilled sentinel, is fixed before a release, not during one.
3. Follow the guide's six steps in order and by number, running each check it prints; this skill adds no step of its own. The merge in its step 4 is the release itself, so it stays the operator's unless their request named it.
4. An older line takes `rk guide backport` the same way.

## When it goes wrong

Route by symptom through `rk method recovery`:

| Symptom                                          | Path                                                  |
| ------------------------------------------------ | ----------------------------------------------------- |
| A published version is defective                 | Fix forward, then withdraw the bad version            |
| The release request merged and nothing published | Re-run the release job on the same trunk commit       |
| The changelog shipped wrong                      | Amend on the trunk; ships next release                |
| An older line needs a patch                      | Cut release/<major>.<minor> from the tag, cherry-pick |
| CI is down and the release cannot wait           | The three-move hand-publish, enforcement off first    |
| The release page is empty                        | Re-run the artifact workflow on the same tag          |
| A released artifact has no attestation           | Re-run the artifact workflow on the same tag          |

A release line flows one direction: fix on the trunk first, cherry-pick only the fix, never merge the line back, and let automation tag the line's patch. `rk method branch-for-release` owns the walkthrough and the four ways the pattern breaks.

## Defaults

- Work landing while the release request is open ships in the next refresh or the next release; that is never a reason to rush a merge.
- A verify step that fails right after the release merge is usually timing: the artifact builder creates the release page minutes after the tag.
- Prefer the smallest recovery that returns to the happy path; never surgery on tags, published versions, or the trunk.
- A hand-uploaded artifact carries no provenance, even when CI built the file. Treat that release as unfinished and re-run the artifact workflow on its tag once CI is back.
