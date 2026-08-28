# AGENTS

This repository is the canonical knowledge product for the release-kit workflow. `README.md` routes readers; this file routes agents to the rules that bind their work.

## Before acting

- Load each `_docs/specs/SPEC-<domain>.md` affected by the work.
- Apply stated rules and cite their `<domain>:<rule>` IDs in reports and failures.
- Do not load `_docs/decisions/` unless someone asks why a rule exists.
- Update the owning method chapter or binding in the same change as behavior.
- Update a skill in the same change as the behavior it describes.

## Ownership boundaries

- `method/` and `bindings/` are the canon prose; `snippets/` and `versions.toml` are the landable payload.
- `src/` is the distribution: the `rk` binary embeds `method/`, `bindings/`, `snippets/`, `skills/`, `versions.toml`, and the licenses at compile time, so canon and binary cannot drift.
- Every pinned tool is declared once, in `versions.toml`; a snippet pin changes together with its registry entry.
- `_docs/` is this repository's own spec-driven-docs instance plus its decisions; it never ships in the crate.
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
- Every subcommand lands with its integration tests in `tests/cli.rs`.
- Run `just check` before handoff. It lints, tests, and lands the rust files into a scratch target.
- `Cargo.toml` is the release source of truth. Write Conventional Commits; release-plz derives the version, the changelog, and the tag. Never author a tag: this repository runs its own convention, `rk method operate`.
- Manage dependencies through cargo (`cargo add`, `cargo remove`, `cargo update`); never hand-edit versions in `Cargo.toml`.

## Routing

- The method spine and recovery paths: `method/README.md`.
- Technology specifics: `bindings/README.md`.
- What lands in a target: `snippets/`, served by `rk snippet --list`.
- Pinned tools and freshness: `versions.toml`, served by `rk versions`.
- Docs format and budgets: `sdd spec docs-format`; this repository is an sdd instance.
