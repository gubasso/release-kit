# Validate a candidate with rc tags

## Context and Problem Statement

A release line exists because someone must validate artifacts before a ship — yet the prior shape's candidate, an open gate branch, was mutable state a human had to shepherd, and it carried no installable artifacts for the validation it was supposed to serve.

## Considered Options

- `Pre-release tags on the release line` — chosen.
- `A candidate branch held open as the validation state` — rejected: a branch is mutable state with no artifact attached, so the thing being validated can move under the validator.
- `Publishing a candidate version to the registry` — rejected: a registry treats every published version as immutable and resolvable, so a candidate a user can install is already a release.

## Decision Outcome

Chosen option: `rc tags` — automation tags `v<version>-rc.<n>` on the release line; the tag builds the installers a human validates and publishes nothing to any registry. Semantic versioning orders a pre-release below its release, and package resolvers exclude pre-releases from ordinary requirements, so a candidate can never shadow the version it precedes. The `v*` tag protection already covers rc tags, so a candidate is immutable and an rc number is single-use; a validation finding lands on the trunk, crosses by cherry-pick, and mints the next rc.

## Consequences

- Good: a candidate is a pinned, immutable, installable artifact rather than a branch state.
- Good: validation and release ride the same automation path.
- Bad: each finding costs a full rc cycle — cherry-pick, tag, rebuild, revalidate.

## Status

Implemented — `method/09-release-lines.md` states the cycle, `runbooks/release-lines.md` carries its commands, and `rk lines rc` reports a line's candidates.
