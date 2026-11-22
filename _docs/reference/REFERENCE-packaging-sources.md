# Packaging Sources

External sources behind the consumer rules of `SPEC-packaging.md`, the install commands of the `## Install` section of the root `README.md`, and the artifact-reach half of `SPEC-forge-setup.md`: what Cargo, the shell installer of a release, and each binding's packaging tool promise, the release bot's package boundary, and which rule each promise bears on.

Verified against the listed sources on 2026-09-04, the packaging-tool sections on 2026-09-11, the exact-version install section on 2026-09-14, the release-plz section on 2026-09-27, the shell installer section on 2026-11-12, and the install route sections on 2026-11-22.

## Cargo, on installing one exact version into one root

`cargo install --version` accepts a version requirement such as `~1.2`, and a value without a requirement operator must be in the form `MAJOR.MINOR.PATCH` and installs exactly that version, which is not treated as a caret requirement. `--root` names the installation root and takes precedence over `CARGO_INSTALL_ROOT`, the `install.root` config value, `CARGO_HOME`, and `$HOME/.cargo`, and every executable lands in that root's `bin`. `--locked` forces Cargo to use the `Cargo.lock` the package ships, which the default ignores. An install whose version, source, binaries, features, profile, and target are unchanged is a no-op without `--force`.

- <https://doc.rust-lang.org/cargo/commands/cargo-install.html>

Bearing: `packaging:the-operator-selects-the-installed-version`. The manager's own surface already installs one exact release into a root of the operator's choosing, so acquisition needs no verb in `rk`, and a landing that fetched a release would duplicate what this command does with a checked lockfile.

## The shell installer, on installing the release it was built for

Each release of this repository carries `release-kit-installer.sh`, which cargo-dist generates for that release. The script names its own version in `APP_VERSION`, downloads its archive from the `releases/download/v<version>` path of that same release, and holds the sha256 of each archive it may download, so a script fetched from one release installs exactly that release. Where `RELEASE_KIT_UNMANAGED_INSTALL` names a directory, the script installs into it and changes no `PATH` setup and no updater.

- <https://github.com/gubasso/release-kit/releases/download/v0.9.10/release-kit-installer.sh>

Bearing: `packaging:a-landed-job-takes-rk-by-one-route`. A landed job that fetches the installer from the release of the version it pins gets that exact binary, with no version argument and no resolution.

## Nix, on a flake input at a release tag

The flake reference `github:<owner>/<repo>/<rev-or-ref>` selects a tag by name: "`<rev-or-ref>` specifies the name of a branch or tag (`ref`), or a commit hash (`rev`)." `inputs.<name>.follows` takes a `/`-separated path of input names, and the manual shows the empty value in its example of a circular input, `inputs.b.inputs.a.follows = "";`, with the comment "Tell the 'b' flake not to fetch 'a' again, to ensure its 'a' is this 'a'." `nix flake update [option...] inputs...` updates only the named inputs in `flake.lock`, as its example `nix flake update nixpkgs` shows.

- <https://nix.dev/manual/nix/2.28/command-ref/new-cli/nix3-flake>
- <https://nix.dev/manual/nix/2.28/command-ref/new-cli/nix3-flake-update>

Bearing: the Nix flake route of the README. The empty follows on the `release-kit` input inside the release-kit input keeps the previous release that the flake pins for its own development out of `flake.lock`. A pin move edits the tag and updates the one input.

## mise, on the cargo backend

The cargo backend installs a crate at a pinned version: "pin one with `mise use cargo:eza@0.23.5`." It "downloads a prebuilt binary when cargo-binstall can find one and builds the crate with Cargo otherwise."

- <https://mise.jdx.dev/dev-tools/backends/cargo.html>

Bearing: the mise route of the README, `mise use cargo:release-kit@X.Y.Z`.

## devbox, on a flake reference as a package

A remote flake is a package name: "Use `github:<org>/<repo>/<ref>#<output>` as the package name to install from a Github repo." The command is `devbox add <pkg>...`, from `internal/boxcli/add.go` at the same commit. The guide page that the example links, `https://www.jetify.com/docs/devbox/guides/using-flakes/`, returned HTTP 404 on 2026-11-22.

- <https://github.com/jetify-com/devbox/blob/58c59a28c257e66c3020b2910e2270af66001d06/examples/flakes/README.md>
- <https://github.com/jetify-com/devbox/blob/58c59a28c257e66c3020b2910e2270af66001d06/internal/boxcli/add.go>

Bearing: the devbox route of the README, `devbox add github:gubasso/release-kit/vX.Y.Z#default`.

## asdf, on the plugin index

The plugin index holds no entry named `rk` or `release-kit`, and no other asdf plugin for release-kit exists.

- <https://github.com/asdf-vm/asdf-plugins/tree/1fc7a2c7cbf185f4147e84f1a58e1f5513dc8041/plugins>

Bearing: the asdf entry of the README. asdf records no version of rk, so an asdf user takes the GitHub release, which is a host install.

## The GitHub release, on its archive and its installer

The release `v0.10.3` carries `release-kit-x86_64-unknown-linux-gnu.tar.xz` and `release-kit-installer.sh`, and `dist-workspace.toml` builds that one target. Its notes give the installer command `curl --proto '=https' --tlsv1.2 -LsSf https://github.com/gubasso/release-kit/releases/download/v0.10.3/release-kit-installer.sh | sh`.

- <https://github.com/gubasso/release-kit/releases/tag/v0.10.3>

Bearing: the GitHub release route of the README, for asdf users and any host with no manager. The crate `release-kit` on <https://crates.io/crates/release-kit> serves the host install, `cargo install release-kit`, under the Cargo section above.

## Cargo, on package selection and the file listing

`cargo metadata` reports `workspace_default_members`, the package ids a bare command operates on, beside a `packages` array whose entries carry `id` and `manifest_path`; `--no-deps` limits the output to workspace members and `--format-version 1` fixes the shape. `cargo package --list` prints the files that would be included in the generated `.crate`, one per line; `--manifest-path` selects the manifest, and `--allow-dirty` permits an uncommitted working tree. Which files land is governed by `[package].include` and `[package].exclude`, which the manifest reference documents together with the ignore rules Cargo applies, and that reference names `cargo package --list` as the way to see the result. A package root bounds the files a package can carry, so a nested workspace member cannot include a file above it.

- <https://doc.rust-lang.org/cargo/commands/cargo-metadata.html>
- <https://doc.rust-lang.org/cargo/commands/cargo-package.html>
- <https://doc.rust-lang.org/cargo/reference/manifest.html#the-exclude-and-include-fields>

Bearing: `forge-setup:a-package-check-states-policy-reach` — the metadata call decides whether exactly one default package is rooted at the target, which is the one shape whose listing is unambiguous, and the listing then answers policy inclusion by exact path.

## release-plz, on which commits count toward a package

For each package, release-plz walks the commits since its last release and keeps a commit only when the files it changes intersect the files `cargo package --list` prints for that package at that commit. A kept commit is listed in the package's changelog and is what makes a release necessary. No configuration option admits a commit that changes only files outside the package, and the FAQ gives the same answer for a release request that opens too often. `release_commits` narrows the kept commits further by a message pattern; with it unset, every kept commit asks for a release, whatever its type. A dependency change in `Cargo.toml` or `Cargo.lock` asks for a release too. At the pinned 0.4.3, release-plz ignores a change to a dev-dependency requirement that a package takes from the workspace `Cargo.toml` through `workspace = true`, unless it updates the `Cargo.lock` dependencies of a package with binaries (pull request 3290). It collects the package history across merged branches, and keeps a change that survives an equal package snapshot through another merge lineage (pull requests 3078 and 3079).

- <https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/crates/release_plz_core/src/command/update/updater.rs#L950-L970>
- <https://release-plz.dev/docs/faq>
- <https://github.com/release-plz/release-plz/blob/release-plz-v0.4.3/website/docs/why.md#conventional-commits-arent-required->
- <https://github.com/release-plz/release-plz/pull/3290>
- <https://github.com/release-plz/release-plz/pull/3078>
- <https://github.com/release-plz/release-plz/pull/3079>
- <https://release-plz.dev/docs/config#the-release_commits-field>

Bearing: `forge-setup:a-package-check-states-policy-reach` — a packaged file a landing rewrites makes that landing a release, so the check faults each packaged release-kit path — and `git:a-local-integration-warns-of-an-uncounted-release` — a release-intent message on a change the listing does not reach is invisible to the bot.

## Python packaging, on the sdist and the wheel

A PEP 517 project names its own build backend in `pyproject.toml`, and the backend decides what each distribution contains; `python -m build` produces an sdist and then a wheel from that sdist by default. The packaging guide states that the sdist and the wheel are different distribution formats with different contents, and each backend documents its own inclusion configuration, so there is no one command that lists both across backends.

- <https://peps.python.org/pep-0517/>
- <https://build.pypa.io/en/stable/>
- <https://packaging.python.org/en/latest/discussions/package-formats/>

Bearing: `forge-setup:a-package-check-states-policy-reach` — the binding has no deterministic listing command here, so the check keeps its packaging result and names the inclusion unproved.

## Git, on archive and export-ignore

`git archive` creates an archive of the named tree, and paths carrying the `export-ignore` attribute are not added to it.

- <https://git-scm.com/docs/git-archive>
- <https://git-scm.com/docs/gitattributes#_creating_an_archive>

Bearing: none since release-kit 0.9.15. The Bash binding packs its archive from its own `include` and `exclude` declaration, and `export-ignore` plays no part, as `REFERENCE-release-engine-sources.md` records.
