# Release Kit

A canonical release workflow: one technology-agnostic method, per-technology bindings, and the `rk` CLI that carries and lands both.

## What this is

Two products in one repository. The method is what a reader loads: [method/](./method/README.md) states the five-stage spine, the invariants, and the recovery paths, and [bindings/](./bindings/README.md) states where rust, python, and bash differ. The distribution is what a project installs: the `rk` binary, built from `src/`, embeds the whole payload — the method, the bindings, the landable files under `snippets/`, the spliced block texts under `blocks/`, the agent skills, and the pinned-tool registry — and lands the deterministic files with `rk init`.

The design in one sentence: work integrates on one trunk, a bot maintains one release pull request that bumps the version and the changelog, and merging it is the release that tags, publishes, and builds the artifacts.

## Install

```bash
cargo install release-kit
```

The binary is `rk`.

## Quick paths

- Read the method: [method/README.md](./method/README.md), or `rk method --list` anywhere.
- Read a technology binding: [bindings/](./bindings/README.md), or `rk binding rust`.
- Land the workflow in a project: `rk init --tech rust --target .` previews; `--apply` writes the files and the landing record.
- See the candidate before it lands: `rk stage --target .` writes this binary's proposed files and its version-matched knowledge into a disposable stage, kept through the landing and removed only by `rk stage clean <path>`.
- Ask a landed project about itself: `rk status`, with `--check` for a CI gate; take this binary's projection with `rk upgrade`, which replaces every recorded generated file, preserves every seeded and state file, and rewrites the receipt last; record a pre-record project already at the projection with `rk adopt`; classify a project before anything lands with `rk assess`. Each of these renders afresh from the installed binary and the target, reads no stage and no other release, and refuses an unattributed whole-file collision before its first write; `rk reconcile plan` and `rk reconcile apply` remain the stored-plan path until their removal.
- See the pinned tools and their freshness: `rk versions`, and `rk versions --check` to compare each pin upstream.
- Take `rk` through the tool manager a project already runs and keep it fresh: `rk self-depend add` prints the fragments for one manager and venue pair and seeds the file the project lacks; `rk self-depend sync` moves the pin from `.envrc`; `rk self-depend clean` removes what a hand-rolled bump left.
- Take another project as a dependency: `rk depend assess --source <path>` reads how it distributes itself and how this project manages its tools; `rk depend add --kind dev|prod` prints the fragment or the native command, and `--apply` seeds a manager file only where none exists.
- Prove what the binary carries: `rk payload`, with `--json` for the machine form and `--release <version>` for another release's bundle, fetched once and verified against the registry.
- Check the host and load the whole surface: `rk doctor` and `rk usage`.
- Install the agent skills at user scope: `rk skill install` previews; `--apply` writes `~/.claude/skills/` and `~/.agents/skills/`, plus what they share at `~/.local/state/release-kit/skills/shared/`.
- Follow the recipe: `rk guide setup` once per repository, `rk guide release` for every release, `rk guide backport` for a fix crossing to an older line, `rk guide release-lines` for the line's own life, `rk guide reconcile` for every landing write from plan to apply.
- Start work an issue names: `rk issue start <issue>` previews; `--apply` mints the branch at the forge and seats it, with `rk guide issue` as the procedure.
- Execute the repository-side setup: `rk setup --target .` previews; `--apply` runs; `rk setup check` proves it.
- Read a forge's specifics: `rk forge github` or `rk forge gitlab`.
- Audit what a setup run did: `rk runs list` and `rk runs show <id>`.
- Release this repository itself: `rk guide` carries the procedure; [_docs/guides/release/](./_docs/guides/release/README.md) carries this repository's coordinates over it.

This repository dogfoods its own convention. The procedure lives in the shipped runbooks, served by `rk guide`; [_docs/guides/release/](./_docs/guides/release/README.md) is this repository's overlay — the coordinates to export once, its deviations, and the transcript of a release cut end to end.

## License

The method is under [CC BY 4.0](./LICENSE-CC-BY-4.0) and the distribution that lands it is under the [MIT License](./LICENSE-MIT). [LICENSE](./LICENSE) states which side each file falls on, and `rk license` prints the terms the binary carries.
