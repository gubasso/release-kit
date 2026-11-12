# Publish a candidate as a registry prerelease

## Context and Problem Statement

A release line exists so a human can validate artifacts before a ship. The rc-tag record kept candidates off every registry, so a validator could not install one the way a user installs a release, and no landed binding could mint one.

## Considered Options

- `A registry prerelease minted by rk and published by the pair's release half` — chosen.
- `rc tags that publish nothing` — rejected: the candidate is not installable through the ecosystem's own resolver.
- `rk creates the rc tag` — rejected: release-plz skips any package whose tag already exists, so it would publish nothing.

## Decision Outcome

Chosen option: a registry prerelease. A person mints a candidate on purpose, through `workflow_dispatch` on GitHub or a manual job on GitLab. `rk lines mint` reads the line's remote tags, writes the next `-rc.<n>` version into the manifest, commits, and pushes. The push is the lock, and `rk` creates no tag. The pair's release half tags, builds, attests, and publishes the candidate as a prerelease, or creates only the tag and the page where the pair has no registry. Semantic versioning orders a prerelease below its release, so a candidate never shadows the stable version.

On a release-plz pair, the rc job runs `release-plz release` with a config derived at run time from `release-plz.toml`, which differs in `release_always = true` alone. The job lives in the workflow the trusted publisher already names. release-plz never writes the stable version itself, so `rk lines mint --promote` writes it, only while the line holds no commit after the newest candidate. The line's release request then proposes it, and a person merges that request.

## Consequences

- Good: a validator installs the candidate with the ecosystem's own command.
- Good: one verb serves every binding, and each pair keeps owning its tag.
- Bad: a yanked candidate keeps its version string, so an rc number is single-use.
- Bad: the line's history carries a commit for every candidate and for the promotion.

## Status

Implemented for the minting verb and the two rust pairs: `rk lines mint` mints and promotes, and the rust GitHub workflow and GitLab pipeline run it and publish the candidate. The bash and python pairs are not wired yet, as issues 73, 74, and 75 track.
