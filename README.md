# Release Kit

A canonical release workflow: one technology-agnostic method, per-technology bindings, and the `rk` CLI that carries and lands both.

## What this is

Two products in one repository. The method is what a reader loads: [method/](./method/README.md) states the five-stage spine, the invariants, and the recovery paths, and [bindings/](./bindings/README.md) states where rust, python, and bash differ. The distribution is what a project installs: the `rk` binary, built from `src/`, embeds the whole payload — the method, the bindings, the landable files under `snippets/`, the agent skills, and the pinned-tool registry — and lands the deterministic files with `rk init`.

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
- Ask a landed project about itself: `rk status`, with `--check` for a CI gate; take a newer payload with `rk upgrade`; record a pre-record project with `rk adopt`.
- See the pinned tools and their freshness: `rk versions`, and `rk versions --check` to compare each pin upstream.
- Prove what the binary carries: `rk payload`, with `--json` for the machine form.
- Check the host and load the whole surface: `rk doctor` and `rk usage`.
- Install the agent skills at user scope: `rk skill install` previews; `--apply` writes `~/.claude/skills/` and `~/.agents/skills/`, plus what they share at `~/.local/state/release-kit/skills/shared/`.
- Follow the recipe: `rk guide setup` once per repository, `rk guide release` for every release.
- Execute the repository-side setup: `rk setup --target .` previews; `--apply` runs; `rk setup check` proves it.
- Read a forge's specifics: `rk forge github` or `rk forge gitlab`.
- Audit what a setup run did: `rk runs list` and `rk runs show <id>`.
- Release this repository itself: [_docs/guides/release/](./_docs/guides/release/README.md) carries the setup once and the release every time.

This repository dogfoods its own convention. Its own instance of the procedure, with every command spelled out by hand beside the `rk` step that runs it, is [_docs/guides/release/](./_docs/guides/release/README.md).

## License

The method is under [CC BY 4.0](./LICENSE-CC-BY-4.0) and the distribution that lands it is under the [MIT License](./LICENSE-MIT). [LICENSE](./LICENSE) states which side each file falls on, and `rk license` prints the terms the binary carries.
