# Release

Day-to-day release of this repository. The procedure is the shipped release runbook; this page carries the coordinates over it and the transcript of one pass. First time here: [setup.md](./setup.md).

## Coordinates

Export these once; every command below reads them. [README.md](./README.md) says what each one is.

```bash
export OWNER=<account or organization that owns the repository>
export REPO=<repository name>
export CRATE=<package name as published to the registry>
```

## The procedure

```bash
rk guide release --repo "$OWNER/$REPO"
```

Below 1.0 only the left-most non-zero component is the incompatible one, so a breaking change moves the minor and a feature moves the patch. Never author a tag, and never move a published one — a bad release is fixed by the next version. An older line takes `rk guide backport`.

## Worked example

One pass, releasing v0.2.0 over v0.1.0, through the runbook's six steps.

```bash
$ gh pr merge 41 --repo "$OWNER/$REPO" --squash --delete-branch   # the feat PR
$ gh pr list --repo "$OWNER/$REPO" --state open
42  chore: release v0.2.0  release-plz-2026-09-02T10-11-04Z  OPEN

$ git fetch origin --tags --force
$ git log --oneline "v0.1.0^{commit}..origin/master"
a91c2e0 feat(guides): Land the manual-first release guides
3b81e07 test(distribution): Enforce the leak rule the spec already claimed
```

The entry on PR 42 names both commits; nothing to correct. Merge it:

```bash
$ gh pr checks 42 --repo "$OWNER/$REPO" --watch
test  pass  2m14s
$ gh pr merge 42 --repo "$OWNER/$REPO" --squash --delete-branch
$ git fetch origin && SHA="$(git rev-parse origin/master)" && echo "$SHA"
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d

$ gh run watch --repo "$OWNER/$REPO" --exit-status \
    "$(gh run list --repo "$OWNER/$REPO" --workflow release-plz.yml \
       --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
✓ release-plz · c3d8e10   Run release-plz completed with 'success'

$ gh run watch --repo "$OWNER/$REPO" --exit-status \
    "$(gh run list --repo "$OWNER/$REPO" --workflow release.yml \
       --event push --commit "$SHA" --limit 1 --json databaseId -q '.[0].databaseId')"
✓ Release · c3d8e10   Run Release completed with 'success'

$ cargo info "$CRATE" | grep -m1 -i '^version'
version: 0.2.0
$ git fetch origin --tags --force && git cat-file -t v0.2.0
tag
$ git rev-parse "v0.2.0^{commit}" origin/master
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d
c3d8e10f4b2a9e7d1c5b8a3f6e9d2c4b7a1f0e5d
```

The `--event push` filter on the second watch is what keeps it off the pull-request `plan` run `release.yml` also produces.
