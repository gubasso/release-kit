# Distribution Sources

External sources behind `SPEC-distribution.md`: what the binary carries, where it may write outside a target, and how it speaks to a machine. Each entry states what the source says and which rule or file it bears on. Decision records state what was chosen; this file states what the choice was checked against.

Verified against the listed sources on 2026-08-28 and re-checked on 2026-08-29. A source marked corroborating was reported by a parallel review and not independently fetched.

## Embedding assets in a Rust binary

`include_dir` exposes embedded contents as `&'static [u8]` and embeds unconditionally. Its metadata feature carries only basic data such as modification time and never permissions, so no file mode survives embedding. The documented compile cost is real: a 64 MB payload takes seconds and hundreds of megabytes of build memory.

`rust-embed` is the better-known alternative and embeds only in release builds, reading from the filesystem in debug unless `debug-embed` is set.

- <https://docs.rs/include_dir/latest/include_dir/>
- <https://crates.io/crates/rust-embed>
- <https://docs.rs/rust-embed/latest/rust_embed/trait.RustEmbed.html>

Bearing: `distribution:the-payload-roots-are-declared-once` and `src/payload_roots.rs`. Unconditional embedding is the stronger guarantee for this use, because a debug build reading from disk would let an uncommitted edit run without a rebuild. Losing the file mode is why a materialized script is invoked as `sh <path>` rather than executed.

## XDG Base Directory Specification

Data at `$XDG_DATA_HOME` (`~/.local/share`), configuration at `$XDG_CONFIG_HOME` (`~/.config`), state at `$XDG_STATE_HOME` (`~/.local/state`), and cache at `$XDG_CACHE_HOME` (`~/.cache`) for data the specification calls non-essential. State is what should persist between restarts but is not important or portable enough to be data.

The specification also says user-specific executable files may be stored in `$HOME/.local/bin`.

- <https://specifications.freedesktop.org/basedir/latest/>
- <https://wiki.debian.org/XDGBaseDirectorySpecification>

Bearing: the run journal's state root, and the materialization cache, which is regenerable from the binary at any moment and therefore cache by the specification's own definition. The `~/.local/bin` line permits user-specific executables there; it does not make every file containing shell one. These are invoked by one program and never by a person.

## GitHub CLI extensions, and when a separate install lifecycle is warranted

Extensions install to and run from `~/.local/share/gh/extensions/`. A script extension is an executable at the repository root named for the repository, and `--pin` selects a tag or a commit; without it a script extension clones the tip of the repository rather than the latest release, which is a standing versioning complaint.

- <https://docs.github.com/en/github-cli/github-cli/using-github-cli-extensions>
- <https://docs.github.com/en/github-cli/github-cli/creating-github-cli-extensions>
- <https://github.com/cli/cli/issues/9295>

Bearing: `forge-setup:a-script-is-executed-never-installed`. The lesson is the inverse of the naive one. The extension mechanism exists because those commands are authored, released, updated, and removed independently of the CLI. Release-kit's setup scripts are none of those: they ship with the binary, they change only when it changes, and they are useless outside the ordering the setup imposes. Taking on that lifecycle would buy nothing and cost the version-resolution problem the linked issue is about.

## krew, as the negative case for a plugin directory

krew installs plugins under `~/.krew`, with binaries in `~/.krew/bin` and content in `~/.krew/store`. It has no versioning: it installs only the latest version in its registry. The gap is visible enough that a third-party backend exists to add pinning on top.

- <https://github.com/JJJJJones/krew/blob/v0.2.1/docs/KREW_ARCHITECTURE.md>
- <https://github.com/soupglasses/mise-krew>

Bearing: the argument against a `~/.release-kit/` tree of installed scripts. Every version problem krew has is one that shipping assets inside the binary avoids for free.

## devcontainer features, and what a versioned executable payload costs

Every feature carries at minimum a `devcontainer-feature.json` and an `install.sh`, and the published artifact is the whole subdirectory, pushed to an OCI registry as `<registry>/<namespace>/<id>[:version]`. A feature is republished only when its declared version changes, with major and minor tags maintained per semver.

- <https://containers.dev/implementors/features-distribution/>
- <https://github.com/devcontainers/spec/blob/main/docs/specs/devcontainer-features.md>
- <https://github.com/devcontainers/features>

Bearing: shipping executable shell as a versioned immutable artifact is a proven pattern rather than an improvisation. The price of that pattern is a registry, publishing tooling, and a resolution algorithm; embedding buys the same immutability for the cost of one macro invocation.

## uv and rustup, on not overwriting what you did not install

uv refuses to overwrite executables it did not install, and separates persistent tool data from the shims it places on `PATH`. rustup exposes stable public proxies on `PATH` rather than every managed implementation artifact.

- <https://docs.astral.sh/uv/concepts/tools/>
- <https://docs.astral.sh/uv/reference/storage/>
- <https://rust-lang.github.io/rustup/concepts/>

Corroborating: reported by a parallel review and not independently fetched.

Bearing: `distribution:skill-uninstall-removes-only-what-it-wrote` and the digest record in `src/skills/record.rs`, which is the refusal rule in its own form. The proxy rule is the argument that one command is the public entry point rather than nine files on `PATH`.

## Agent skills directory conventions

The emerging cross-vendor location is `.agents/skills/`, discovered automatically by most current agents, while individual products keep their own roots. `SKILL.md` is the common denominator, and differences are directory conventions plus optional agent-specific frontmatter. Copilot additionally reads a project-scope `.github/skills/`. Agent Plugins 1.0, a vendor-neutral package format carrying skills and server configuration together, was published on 2026-08-06.

- <https://www.webfuse.com/agent-skills-cheat-sheet>
- <https://developers.redhat.com/articles/2026/07/27/standardize-project-context-agentsmd-and-agent-skills>
- <https://agentman.ai/blog/agent-skills-ecosystem-report-2026>

Corroborating: secondary sources only. The primary specification at agentskills.io was not fetched, so treat the dates and the product counts as unverified.

Bearing: the two roots the installer writes are the two the convention names. A project-scope root is excluded by `distribution:a-skill-has-one-owner`, because an agent resolves a skill by name across scopes. Agent Plugins 1.0 is a thing to watch rather than a thing to build for.

## Preview, force, and honest idempotence

`-n, --dry-run` should not run the command but describe the changes that would occur. `--force` is for a destructive action that usually needs confirmation but must remain scriptable. On idempotence, the CLI Spec's position is that what consumers need is not a promise that every command is idempotent, but an accurate statement of what re-running does and a safe move when the answer is do not.

- <https://clig.dev/>
- <https://github.com/cli-guidelines/cli-guidelines>
- <https://clispec.dev/>

Bearing: the preview-by-default inversion across `rk init`, `rk skill install`, and `rk setup`, which is stricter than the guideline, and the per-step idempotence table in the setup documentation, which is the accurate statement the CLI Spec asks for rather than a blanket claim.

## Machine-readable streams

Terraform's machine-readable UI emits one JSON object per line, opening with a version message, with documented rules that a consumer ignores unknown fields and unknown message types. BuildKit emits solve-status events the same way under `--progress=rawjson`.

- <https://developer.hashicorp.com/terraform/internals/machine-readable-ui>
- <https://docs.docker.com/reference/cli/docker/buildx/build/>

Bearing: `distribution:machine-output-declares-its-schema`. Two unrelated codebases converging on a schema-first line-delimited stream with forward-compatibility rules is what makes it a convention rather than one tool's habit.

## Exit codes as a category, not an instance

The GitHub CLI documents four exit codes: 0 success, 1 any failure, 2 cancelled, and 4 authentication required. One code carries branchable meaning and everything else is the message's job. Terraform takes the other available move: `plan -detailed-exitcode` returns 0 for an empty diff, 1 for an error, and 2 for a non-empty diff, turning observation into judgment behind one option rather than a separate verb.

- <https://cli.github.com/manual/gh_help_exit-codes>
- <https://developer.hashicorp.com/terraform/cli/commands/plan>

Bearing: the exit-code matrix in `src/error.rs` and the closed reason vocabulary in `src/output.rs` — the code is the category, the reason and message carry the instance. The judging-mode flag on `rk status --check` and `rk versions --check` follows the Terraform precedent, and `gh run view --exit-status` is the flag-not-verb form of the same idea.
