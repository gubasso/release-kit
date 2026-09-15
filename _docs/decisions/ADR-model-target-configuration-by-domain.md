# Model the target configuration by domain

## Context and Problem Statement

`rk init` resolved one technology and one forge, selected one binding pair, and stored unrelated answers under `[project]`, `[landing]`, and `[setup]`. That shape cannot describe a Rust backend with a Python tool, a repository that releases nothing, or a repository with no forge, and it mixes facts about the project with choices about development and release-kit products.

## Considered Options

- `Typed domains with a capability catalog` — chosen.
- `A pseudo-technology named none` — rejected: it invents a category to describe an absence, and every reader that keys on the technology then carries a special case for it.
- `A uniform category bag` — rejected: facts, methods, requests, and state have different validity and precedence rules, and one bag gives them one rule.

## Decision Outcome

Chosen option: `typed domains with a capability catalog`. The committed file is the target configuration, and the project profile is one domain in it. The profile states what the project is: zero or many technologies, an optional forge, and a release intent of `automatic`, `external`, or `none`. The Git workflow states the trunk and the checkout mode. Capability requests state which optional products the target wants. Fixed method requirements have no key. A landing derives from the whole configuration through a catalog whose availability belongs to a capability at its dimensions, so a repository with no technology and no release still receives the local Git workflow guards.

Enforced by `project-profile:the-target-configuration-is-typed-by-domain`, `project-profile:availability-belongs-to-a-capability-at-its-dimensions`, and `git:the-method-is-trunk-based`.

## Consequences

- Good: a human or an agent identifies every answer by domain, and a fact copied into two tables cannot go stale in one of them.
- Good: a release-less or forge-less repository is a valid target rather than an exception.
- Bad: the configuration schema and the record schema both move, so every landed target migrates once.

## Status

Accepted.
