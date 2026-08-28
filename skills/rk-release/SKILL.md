---
name: rk-release
description: Operates a release in a project that carries the release-kit workflow, from landing the work to verifying the published version, including changelog correction and recovery. Use when asked to cut a release, ship a version, fix a changelog entry before release, recover a stuck or failed release, yank a version, or hand-publish while CI is down. Triggers include release, cut a release, publish a version, yank, and release recovery.
license: CC-BY-4.0
compatibility: Requires the rk binary on PATH; install with cargo install release-kit or cargo binstall release-kit. Operating a release needs push access to the repository and its forge CLI authenticated.
---

# rk-release

Cut a release through the release-kit convention. The sequence is `rk method operate`; the technology's concrete commands are in `rk binding <tech>`; when something goes wrong, `rk method recovery` owns the way back.

## The shape

Two pull requests. The release request on the integration branch carries the bump and the changelog and publishes nothing. The gate into the release branch is the release: merging it tags and publishes. Never author a tag, never push the release branch, never re-author a published version.

## Cut a release

1. Read the sequence once per session: `rk method operate`.
2. Land the work with its release intent captured and the check suite green, then push.
3. Before merging the release request, compare its changelog entry against the commit range since the previous tag; correct it on the request's branch. This is the last point a correction reaches the release.
4. Merge the release request, wait for the gate, merge the gate as a merge commit once its checks are green.
5. Back-merge, wait for the artifact build to finish, then verify: registry, release page, tag, branches, installed binary — the operate chapter lists the checks in order.

## When it goes wrong

Route by symptom through `rk method recovery`:

| Symptom                                       | Path                                                |
| --------------------------------------------- | --------------------------------------------------- |
| A published version is defective              | Fix forward, then withdraw the bad version          |
| The release request merged and no gate opened | Re-run the gate job on the same commit              |
| The changelog shipped wrong                   | Amend on the integration branch; ships next release |
| CI is down and the release cannot wait        | The three-move hand-publish, enforcement off first  |
| The release page is empty                     | Re-run the artifact workflow on the same tag        |
| A released artifact has no attestation        | Re-run the artifact workflow on the same tag        |

## Defaults

- The gate branch is pinned; work landing meanwhile ships in the next release, and that is never a reason to rush a merge.
- A verify step that fails right after the gate merge is usually timing: the artifact builder creates the release page minutes after the tag.
- Prefer the smallest recovery that returns to the happy path; never surgery on tags, published versions, or the release branch.
- A hand-uploaded artifact carries no provenance, even when CI built the file. Treat that release as unfinished and re-run the artifact workflow on its tag once CI is back.
