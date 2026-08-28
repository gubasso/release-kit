# 05 — Diff surface

Only four axes vary between technologies. A binding is these four answers plus the runbook lines and files that wire them; everything else is the spine, unchanged.

## The four axes

| Axis                | The question it answers                                     |
| ------------------- | ----------------------------------------------------------- |
| Version file        | Which committed artifact states the version the tag mirrors |
| Release-request bot | Which tool maintains the bump-and-changelog pull request    |
| Registry and auth   | Where a publish goes and how the workflow authenticates     |
| Artifact builder    | Which tool builds and attaches installers and binaries      |

## The answers, per technology

| Axis                | Rust            | Python                               | JS and Node                          | Go                     | Bash                |
| ------------------- | --------------- | ------------------------------------ | ------------------------------------ | ---------------------- | ------------------- |
| Version file        | `Cargo.toml`    | `pyproject.toml` `[project] version` | `package.json` `version`             | the tag itself         | `VERSION`           |
| Release-request bot | release-plz     | release-please                       | Changesets                           | none                   | git-cliff           |
| Registry and auth   | crates.io, OIDC | PyPI, OIDC                           | npm, OIDC                            | none; the module proxy | none                |
| Artifact builder    | cargo-dist      | wheels and sdist are the artifacts   | the registry tarball is the artifact | GoReleaser             | `make dist` tarball |

Rust, Python, and Bash have full bindings under [bindings](../bindings/README.md). The JS and Go columns state the answers for when their bindings land; Go is the degenerate case, where the tag is the version file and pushing it is the publish.

## Writing a new binding

A binding document answers the four axes for its technology, then carries only what the spine cannot say:

- The concrete commands for each stage of [operate](./03-operate.md).
- The registry's specific rejects, limits, and token scopes, so [setup](./02-setup.md) step 0 and step 5 are executable.
- The facts that disqualify or configure tools: what the bot can and cannot bump, what the artifact builder generates and owns.
- The deterministic files, added under `snippets/<technology>/` with sentinel placeholders, and their tools pinned in the versions registry.

What a binding never carries: a restatement of the spine, the invariants, or another binding's facts. If a sentence holds for every technology, it belongs in a method chapter.
