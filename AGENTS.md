# AGENTS

This repository is the canonical knowledge product for the release-kit workflow. `README.md` routes readers; this file routes agents to the rules that bind their work.

## Before acting

- Load each `_docs/specs/SPEC-<domain>.md` affected by the work.
- Apply stated rules and cite their `<domain>:<rule>` IDs in reports and failures.
- Do not load `_docs/decisions/` unless someone asks why a rule exists.
- Update the owning method chapter or binding in the same change as behavior.
- Update a skill in the same change as the behavior it describes.
- Run `sdd verify` before handoff.

## Ownership boundaries

- `method/` and `bindings/` are the canon prose; `snippets/` and `versions.toml` are the landable payload.
- `runbooks/`, `forges/`, and `setup/<forge>/` are host-side payload: served by `rk guide` and `rk forge`, executed by `rk setup`, and landed into no target. `SPEC-forge-setup.md` binds how the setup acts on a forge.
- `snippets/` is scoped by `(technology, forge)` pair, and `rk init` selects the pair; a pair may honestly land fewer files than another.
- Every landable file has a declared kind in `src/landing.rs` — `rendered`, `seeded`, or `state` — and a landing writes `.release-kit/manifest.json` into the target, last. `SPEC-landing.md` binds the record and every verb that reads it.
- `src/` is the distribution: the `rk` binary embeds every root in `src/payload_roots.rs` and the licenses at compile time, so canon and binary cannot drift.
- `skills/` installs at user scope only, and `rk init` lands none: an agent resolves a skill by name across scopes, so a second copy is a second entry under one name. `SPEC-distribution.md` binds what the installer may write there.
- Every pinned tool is declared once, in `versions.toml`; a snippet pin changes together with its registry entry.
- `_docs/` is this repository's own spec-driven-docs instance plus its decisions; it never ships in the crate.
- `_docs/specs/` and this repository's integration with the instance are instance-owned; `.spec-driven-docs/` belongs to the sdd canon.
- Keep each durable fact in one owner and link to it elsewhere.
- `LICENSE` splits terms on the product boundary: CC BY 4.0 for the method, MIT for the distribution.

## Authoring

- Keep the root digest at or below 100 lines and subtree digests at or below 150 lines.
- Keep chapters at or below 200 lines and decision records at or below 350 words.
- Use headings, lists, tables, fenced blocks with a language, inline code, and links. Use no bold or italic text.
- Keep prose unwrapped: one source line per paragraph or list item.
- State what is true now. Decision records are the only history-bearing document class.
- Keep exploratory material in `.draft/`; promotion is a rewrite into the owning zone.

## Executable artifacts

- Rust follows the exobrain CLI conventions: clap derive in `src/cli/`, one handler per subcommand in `src/commands/`, typed errors with a tested exit-code matrix in `src/error.rs`.
- Every handler emits through the output boundary in `src/output.rs`; no direct printing in `commands/`, and every machine output carries a versioned, snapshot-tested schema per `distribution:machine-output-declares-its-schema`.
- Every subcommand lands with its integration tests in `tests/cli.rs`.
- Run `just check` before handoff. It lints, tests, and lands the rust files into a scratch target.
- `Cargo.toml` is the release source of truth. Write Conventional Commits; release-plz derives the version, the changelog, and the tag. Never author a tag: this repository runs its own convention, `rk method operate`.
- Manage dependencies through cargo (`cargo add`, `cargo remove`, `cargo update`); never hand-edit versions in `Cargo.toml`.

## Routing

- The method spine and recovery paths: `method/README.md`.
- Technology specifics: `bindings/README.md`.
- Forge specifics and the bot-identity walkthroughs: `forges/README.md`, served by `rk forge`.
- The operator recipes: `runbooks/README.md`, served by `rk guide`.
- The executable repository-side setup: `rk setup`, with `rk runs` over its journals.
- What lands in a target: `snippets/`, served by `rk snippet --list`.
- What a landed target reports about itself: `rk status`, with `--check` as the judging mode; `rk upgrade` takes it to a newer payload; `rk adopt` records a pre-record target.
- Pinned tools and freshness: `versions.toml`, served by `rk versions`; `rk versions --check` is the one verb that fetches.
- The payload's identity and digests: `rk payload`, with `--json` as the machine form.
- Host readiness and the whole command surface: `rk doctor` and `rk usage`.
- Docs format and budgets: `sdd spec docs-format`; this repository is an sdd instance.
- What the binary carries and writes outside a target: `_docs/specs/SPEC-distribution.md`, served by `rk skill --help`.
