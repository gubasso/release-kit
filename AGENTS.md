# AGENTS

This repository is the canonical knowledge product for the release-kit workflow. `README.md` routes readers; this file routes agents to the rules that bind their work.

## Before acting

- Load each `_docs/specs/SPEC-<domain>.md` affected by the work.
- Apply stated rules and cite their `<domain>:<rule>` IDs in reports and failures.
- Do not load `_docs/decisions/` unless someone asks why a rule exists.
- Load `_docs/reference/REFERENCE-<domain>-sources.md` only to check a rule against the upstream documentation it rests on, or to re-verify a fact a forge or a registry may have moved.
- Update the owning method chapter or binding in the same change as behavior.
- Update a skill in the same change as the behavior it describes.
- Run `sdd verify` before handoff.

## Ownership boundaries

- `method/` and `bindings/` are the canon prose; `snippets/` and `versions.toml` are the landable payload.
- `runbooks/`, `forges/`, and `setup/<forge>/` are host-side payload: served by `rk guide` and `rk forge`, executed by `rk setup`, and landed into no target. `SPEC-forge-setup.md` binds how the setup acts on a forge.
- `snippets/` is scoped by `(technology, forge)` pair, and `rk init` selects the pair; a pair may honestly land fewer files than another.
- `snippets/_shared/<forge>` holds a forge's technology-independent files, composed into every pair at landing; it is not a technology, and a destination it shares with a pair is a payload defect.
- `blocks/` holds the whole texts the binary writes outside `snippets/` — the spliced blocks and the host-side hook body — authored as files so no human-faced artifact lives as a source literal.
- Every landable file has a declared kind in `src/landing.rs` — `rendered`, `seeded`, or `state` — and a landing writes `.release-kit/manifest.json` into the target, last. `SPEC-landing.md` binds the record and every verb that reads it.
- `src/` is the distribution: the `rk` binary embeds every root in `src/payload_roots.rs` and the licenses at compile time, so canon and binary cannot drift.
- `skills/` installs at user scope only — the only mode, not a default beside a system scope, because one scope is one owner per skill name and the vendors share no system layout — and `rk init` lands none: an agent resolves a skill by name across scopes, so a second copy is a second entry under one name. `SPEC-distribution.md` binds what the installer may write there.
- Where anything lands for a third-party application is decided inside each project, case by case, against that application's own documentation with a dated citation in `_docs/reference/` — never inferred from a convention this repository follows, and never generalized from one application to another.
- `skill-shared/` is what every skill shares, installed once to `~/.local/state/release-kit/skills/shared/` and named there by absolute path: the two agent roots make no relative path reach one file from both. The plan gate every skill routes to lives there.
- Every pinned tool is declared once, in `versions.toml`; a snippet pin changes together with its registry entry.
- `_docs/` is this repository's own spec-driven-docs instance plus its decisions; it never ships in the crate.
- `_docs/specs/` and this repository's integration with the instance are instance-owned; `.spec-driven-docs/` belongs to the sdd canon.
- Keep each durable fact in one owner and link to it elsewhere.
- `LICENSE` splits terms on the product boundary: CC BY 4.0 for the method, MIT for the distribution.

## Authoring

- Keep the root digest at or below 100 lines and subtree digests at or below 150 lines.
- Keep chapters at or below 200 lines and decision records at or below 350 words; runbooks and forge documents follow the guide rule below rather than the chapter cap, and stay as lean as the procedure allows.
- Use headings, lists, tables, fenced blocks with a language, inline code, and links. Use no bold or italic text.
- Keep prose unwrapped: one source line per paragraph or list item.
- Write guides as numbered steps in prerequisite order: every step carries its check, a manual step enumerates every field and value, and a divergent rerun names its destination.
- Verify every upstream-owned fact in a guide against an official reference and record the dated citation in `_docs/reference/`, keeping the guide lean.
- State what is true now. Decision records are the only history-bearing document class.
- Keep exploratory material in `.draft/`; promotion is a rewrite into the owning zone.

## Executable artifacts

- Rust follows the exobrain CLI conventions: clap derive in `src/cli/`, one handler per subcommand in `src/commands/`, typed errors with a tested exit-code matrix in `src/error.rs`.
- Every handler emits through the output boundary in `src/output.rs`; no direct printing in `commands/`, and every machine output carries a versioned, snapshot-tested schema per `distribution:machine-output-declares-its-schema`.
- Every subcommand lands with its integration tests in `tests/cli.rs`.
- Run `just check` before handoff. It lints, tests, and lands the rust files into a scratch target.
- `Cargo.toml` is the release source of truth. Every commit message is a scoped Conventional Commit; release-plz derives the version, the changelog, and the tag from them. Never author a tag: this repository runs its own convention, `rk method operate`.
- Manage dependencies through cargo (`cargo add`, `cargo remove`, `cargo update`); never hand-edit versions in `Cargo.toml`.

## Routing

- The method spine and recovery paths: `method/README.md`.
- Technology specifics: `bindings/README.md`.
- Forge specifics and the bot-identity walkthroughs: `forges/README.md`, served by `rk forge`.
- The procedure's how, step by step: `runbooks/README.md`, served by `rk guide`; its chapter owns each step's why, and the pair states each procedure once.
- This repository's overlay over `rk guide setup` and `rk guide release` — its coordinates, its deviations, and the proof transcript: `_docs/guides/release/README.md`. It names no account, repository, or crate: the guides are public and carry no operator's particulars.
- The executable repository-side setup: `rk setup`, with `rk runs` over its journals.
- The branches a squash merge retires in this clone: `rk branches prune`, preview by default; the post-merge reminder lands with `rk setup step branch-reminder`, bound by `_docs/specs/SPEC-maintenance.md`.
- The worktree lifecycle and the workflow mode: `rk worktree`, with `rk guide worktree` as the procedure.
- A target that already releases somehow, and its verdict before anything lands: `rk assess`, with `rk guide migration` as the procedure and `rk method migration` as its why.
- The release-line lifecycle and the release style: `rk lines`, with `rk guide release-lines` as the procedure, bound by `_docs/specs/SPEC-maintenance.md` and `_docs/specs/SPEC-landing.md`.
- What lands in a target: `snippets/`, served by `rk snippet --list`.
- What a landed target reports about itself: `rk status`, with `--check` as the judging mode; `rk upgrade` takes it to a newer payload; `rk adopt` records a pre-record target.
- Pinned tools and freshness: `versions.toml`, served by `rk versions`; `rk versions --check` and `rk devshell sync` are the two verbs that fetch.
- A consumer's `rk` from its own flake, pinned and kept fresh: `rk devshell`, with `rk guide setup` carrying the procedure, bound by `_docs/specs/SPEC-packaging.md`.
- The payload's identity and digests: `rk payload`, with `--json` as the machine form.
- Host readiness and the whole command surface: `rk doctor` and `rk usage`.
- Docs format and budgets: `sdd spec docs-format`; this repository is an sdd instance.
- What the binary carries and writes outside a target: `_docs/specs/SPEC-distribution.md`, served by `rk skill --help`.

<!-- BEGIN release-kit -->

## Releases

- This repository runs the release-kit convention; `rk method invariants` states what must stay true.
- An agent here guides and never drives: it reads this convention, tells the operator which step comes next, and takes no git or forge action — creating, switching or deleting a branch, creating or removing a worktree, committing, pushing, tagging, opening or updating or merging a pull request — unless the operator's request named that action. A request to change code authorizes the file changes alone.
- Work reaches the trunk only through a squash-merged pull request from a short-lived branch — `<type>/<slug>` mirroring the squash title's type, or the forge-minted `<issue-id>-<slug>`. Nothing is committed on `master`.
- This project works in worktrees: every code-changing branch lives in its linked worktree (`rk worktree add <branch>` creates or adopts it beside the checkout), the main checkout commits nothing, and `rk worktree prune` retires a merged worktree. One branch, one writer.
- The request's title becomes the trunk's commit message, so it MUST be a scoped Conventional Commit; the body carries the context and lands with it: it names no internal planning artifact and carries no agent attribution — the landed rk-message hook, the forge's body check, and the observed body source hold it.
- Every commit follows the same scoped convention; the landed commit-msg hook enforces it, and the scopes this project accepts are `bindings,ci,cli,deps,distribution,docs,forges,guides,guides/release,instance,landing,method,output,runbooks,setup,skills,snippets`.
- Never author a tag, and never hand-edit a generated artifact workflow.
- Run `rk status` before changing anything under `.github/workflows/` or `.gitlab-ci.yml`, or any file `.release-kit/manifest.json` names.
- The full method is `rk method --list`; the recovery paths are `rk method recovery`.

<!-- END release-kit -->
