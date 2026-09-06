# Dependencies Sources

External sources behind the rules of `SPEC-dependencies.md`: what each tool manager and each technology's own command promise, and which block or rule each promise bears on. Decision records state what was chosen; this file states what the choice was checked against.

Verified against the listed sources on 2026-09-06.

## mise, on its configuration files and the backends

mise reads, from the highest precedence, `mise.local.toml`, `mise.toml`, `mise/config.toml`, `mise/conf.d/*.toml`, `.mise/config.toml`, `.mise/conf.d/*.toml`, `.config/mise.toml`, `.config/mise/config.toml`, and `.config/mise/conf.d/*.toml`, and "Paths that start with `mise` can be dotfiles, e.g. `.mise.toml` or `.mise/config.toml`." A tool is pinned in `[tools]` as `name = 'version'` or as an inline table with a `version` key. The cargo backend takes `"cargo:crate" = "version"` and uses `cargo-binstall` where it can; the ubi backend takes `"ubi:owner/repo" = "version"` or an inline table with `version`, `exe`, `matching`, and `tag_regex`, its example `ubi:goreleaser/goreleaser@1.25.1` showing the version without a `v`; the pipx backend takes `"pipx:package" = "version"` and defaults to `uvx` where uv is installed; the npm backend takes `"npm:package" = "version"`. `mise upgrade` takes tool names and, with `--bump`, rewrites the version in `mise.toml`.

- <https://mise.jdx.dev/configuration.html>
- <https://mise.jdx.dev/dev-tools/backends/cargo.html>
- <https://mise.jdx.dev/dev-tools/backends/ubi.html>
- <https://mise.jdx.dev/dev-tools/backends/pipx.html>
- <https://mise.jdx.dev/dev-tools/backends/npm.html>
- <https://mise.jdx.dev/cli/upgrade.html>

Bearing: the mise file precedence in `src/depend/target.rs`, the four `blocks/depend-mise-*.toml.in` fragments and the mise seed, and the `mise upgrade --bump` freshness line; `dependencies:the-version-comes-from-the-source-tree`, whose ubi pin renders the bare version.

## asdf, on `.tool-versions`

The file carries one line per tool, the plugin name, a space, and the version, and "Whenever `.tool-versions` file is present in a directory, the tool versions it declares will be used in that directory and any subdirectories." The plugin name is the file's first word and is the operator's knowledge; the binary reads no plugin registry.

- <https://asdf-vm.com/manage/configuration.html>

Bearing: `blocks/depend-asdf-line.in` and `dependencies:an-unjudgeable-pair-is-manual-with-its-reason`, under which every asdf pair is manual with reason `asdf-plugin-unknown`.

## devbox, on `devbox.json` packages and flakes

Packages are "a list of package names (`<packages>@<version>`)" or an object keyed by name, and the documented examples install flakes from GitHub as `github:nixos/nixpkgs/21.05#hello` and `github:nix-community/fenix#stable.toolchain`: the flake reference, an optional ref after the repository, and the output after `#`. "We currently support installing Flakes from Github and local paths."

- <https://www.jetify.com/docs/devbox/configuration/>

Bearing: `blocks/depend-devbox-flake.json.in`, which renders the flake reference at the tag with `#default`, and the devbox seed; a source without a flake makes the devbox pairs manual.

## Nix, on flake references and updating one input

A flake reference takes `github:<owner>/<repo>(/<rev-or-ref>)?` and `gitlab:<owner>/<repo>(/<rev-or-ref>)?`, where "`<rev-or-ref>` specifies the name of a branch or tag (`ref`), or a commit hash (`rev`)." An input is declared as `inputs.<name>.url = "..."` and a transitive input is inherited with `inputs.<name>.inputs.nixpkgs.follows = "nixpkgs"`. `nix flake update` takes input names as positional arguments and updates only those.

- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake>
- <https://nix.dev/manual/nix/latest/command-ref/new-cli/nix3-flake-update>

Bearing: `blocks/depend-flake-input.nix.in` and the flake seed, `src/depend/fragments.rs` `flake_ref`, which renders a reference for `github.com` and `gitlab.com` alone, and the `nix flake update <input>` freshness line.

## cargo-dist and cargo-binstall, on where release archives live

cargo-dist's `ci` setting accepts `github`, "currently the only supported CI backend", with `default = []`; its `hosting` setting accepts `github` for GitHub Releases and `simple` for a static file server, and defaults to inferring from `ci`: "when running on GitHub CI, we'll default to using GitHub Releases for hosting/announcing." cargo-binstall's default `pkg-url` for a GitHub repository is `{ repo }/releases/download/{ version }/` or `{ repo }/releases/download/v{ version }/` with a filename template; "For all other situations, `binstall` does not provide a default `pkg-url` and you need to manually specify it."

- <https://axodotdev.github.io/cargo-dist/book/reference/config.html>
- <https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md>

Bearing: `dependencies:the-channel-follows-the-source-evidence`, under which `src/depend/source.rs` offers github-release only where `dist-workspace.toml` names `github` as its CI and does not send hosting elsewhere, or where a binstall table carries no `pkg-url` or one naming GitHub.

## cargo, uv, and npm, on adding a dependency

`cargo add` takes `crate@version` as "Fetch from a registry with a version constraint" and `--dev` for a development dependency. `uv add` takes PEP 508 requirements such as `ruff==0.5.0`, with `--dev` as the alias for `--group dev`. `npm install [<@scope>/]<name>@<version>` installs "the specified version of the package" and "will fail if the version has not been published to the registry"; `--save-dev` puts it in `devDependencies`.

- <https://doc.rust-lang.org/cargo/commands/cargo-add.html>
- <https://docs.astral.sh/uv/reference/cli/>
- <https://docs.npmjs.com/cli/v11/commands/npm-install>

Bearing: `dependencies:a-prod-dependency-lands-through-the-native-command`, whose commands are rendered in `src/depend/matrix.rs`, and the note that no registry is consulted, so an unpublished version fails at the command and not before.
