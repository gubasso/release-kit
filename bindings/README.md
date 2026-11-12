# Bindings

One document per release driver: the four [diff-surface](../method/05-diff-surface.md) answers, the concrete commands, and the registry facts the spine cannot state. A binding is the `release.automation` capability at one driver and one forge, selected where the project profile's release is automatic and that driver is among its technologies. Each binding assumes the whole [method](../method/README.md); nothing here repeats it.

| Binding               | Version file     | Bot               | Registry                | Artifacts                   |
| --------------------- | ---------------- | ----------------- | ----------------------- | --------------------------- |
| [Rust](./rust.md)     | `Cargo.toml`     | release-plz       | crates.io over OIDC     | cargo-dist                  |
| [Python](./python.md) | `pyproject.toml` | rk with git-cliff | PyPI over OIDC, or none | the sdist and the wheel     |
| [Bash](./bash.md)     | `VERSION`        | rk with git-cliff | none                    | the build command's tarball |

The deterministic files each binding lands live under `snippets/<driver>/<forge>/` in this repository and reach a project through `rk init` once the profile's release driver selects them; `rk profile` reports the selection before anything lands. The tool versions they assume are pinned once, in the versions registry at the repository root, readable with `rk versions`.

## Generated files

A file that a tool writes follows that tool's own form. The target configures the tool only as the tool documents, never to satisfy a check of the target. Every check that rewrites or judges the form or the prose of a file excludes the generated files instead. A check that only reads a file, such as a syntax parser, a secret scan, or a link check, keeps them.

These files are generated in a landed target:

- `CHANGELOG.md`, which the release tool of the binding writes.
- Each entry of `.release-kit/manifest.json` whose `kind` is `rendered` and whose `placement` is not `region`. A file with a region, such as `AGENTS.md` or `.pre-commit-config.yaml`, is a file that a person edits, and the checks keep it.
- `.release-kit/manifest.json` and `.release-kit/setup-proof.json`.
- The artifact workflow `.github/workflows/release.yml` that cargo-dist writes, on a Rust target on GitHub.

Exclude each one in the configuration of the tool where the tool has one, because that configuration also governs a run outside pre-commit, such as a recipe, CI, or an editor:

- dprint: `excludes` in `dprint.json`.
- markdownlint-cli2: `ignores` in `.markdownlint-cli2.jsonc`. Its `fix: true` mode rewrites, so it counts as a rewriting check.
- typos: `extend-exclude` in `_typos.toml`, with `--force-exclude` on the hook so that the configuration also holds for the files that pre-commit passes.
- editorconfig-checker: `Exclude` in `.editorconfig-checker.json`.
- The pre-commit fixers `trailing-whitespace`, `end-of-file-fixer`, and `mixed-line-ending` have no configuration of their own, so each hook takes an `exclude:` of its own.

Never use the top-level `exclude:` of `.pre-commit-config.yaml`, because it also removes the generated files from the secret, large-file, and merge-conflict checks.

A workflow that the forge runs keeps its security linters, because the forge runs it with the project's secrets. actionlint and zizmor read the rendered workflows and the generated `release.yml`, and a finding that the project accepts goes into the configuration of the linter, never into the file. [When the generated workflow fails the project's own gates](./rust.md#when-the-generated-workflow-fails-the-projects-own-gates) carries the case of `release.yml`.
