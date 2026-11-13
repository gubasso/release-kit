# Bash binding

| Axis                | Answer                                                     |
| ------------------- | ---------------------------------------------------------- |
| Version file        | `VERSION`, one line                                        |
| Release-request bot | the rk engine, with git-cliff                              |
| Registry and auth   | none                                                       |
| Artifact builder    | `rk release build`, which packs the declared tracked files |

`rk init --technology bash --release-driver bash` lands `VERSION`, `cliff.toml`, and the release pipeline: `.github/workflows/release.yml` on GitHub, `.gitlab-ci.yml` on GitLab.

## The shape

There is no registry, so stages 4 and 5 of [the spine](../method/00-model.md) collapse into the tag and the release page: publishing is tagging, and the tarball attached to the release is the distribution. The one-pull-request shape is unchanged. The release request bumps `VERSION` and the changelog, and merging it is what makes the pipeline tag and attach.

A one-line `VERSION` file is the committed source of truth. No upstream tool keeps one release request for this file on both forges, so the `rk` binary runs the release itself, as `rk release request`, `rk release build`, and `rk release publish`. `_docs/decisions/ADR-run-the-release-through-the-rk-engine.md` records why. The engine asks git-cliff for the next version, which git-cliff computes from Conventional Commits, decides the number itself, and writes the changelog through git-cliff, which is what qualifies the pair under [the invariants](../method/01-invariants.md). The engine runs git-cliff at the version `versions.toml` pins, and refuses any other.

## The workflows

Each pipeline runs `rk` in every release job and holds no release logic of its own. `rk` reads the trunk, the style, the integration, and the line prefix from the landing record.

- `release-request` runs `rk release request` on every push to the trunk and to a release line. It keeps one request per base on the branch `rk-release/<base>`, refreshes it in place, closes it when nothing remains to release, and arms it under the trunk style on the trunk. On GitHub under local integration the rendered `release-gate` job merges it instead, and the engine leaves it unarmed. The job reads git trees and builds nothing, as [what opens a request](#what-opens-a-request) states, so it installs no build package.
- On GitHub, the `release` job installs the build packages, then runs `rk release build`, `actions/attest` over the artifacts the build reports, and `rk release publish`. The attest step runs only where the build reports `release` or `resume`.
- On GitLab, `build` installs the build packages and runs `rk release build`, `provenance` signs the runner's statement over the tarball, and `publish` runs `rk release publish` after both.
- The build releases a final version only at the commit that merges the engine's own request, and a candidate only at the commit that writes it. Every other push skips with success.
- `release-candidate` mints a candidate on a release line, on purpose: on GitHub a `workflow_dispatch` of `release.yml` with the inputs `line`, `expect`, and `promote`, and on GitLab a manual job on a pipeline of the line with the variables `LINE`, `EXPECT`, and `PROMOTE`. It runs `rk lines mint <line> --expect <sha> --apply`, which writes the candidate's version into `VERSION`, commits it, and pushes the line with the bot's token. That push releases the candidate, so the job itself publishes nothing and creates no tag. A candidate the line already carries with no tag yet reports `pending`, and the job writes and pushes nothing.
- On GitLab the job adds `--as-bot`, so `rk` reads the account the bot's token names and commits as it, with the email the account returns or its username's noreply address on the instance, as the release request does. Where the forge names no account, the job stops before it writes a file.
- On GitHub the candidate's page is a prerelease that is never marked latest. On GitLab the page carries no prerelease badge, so the first line of its description names the candidate, and the latest-release permalink follows the candidate until the next stable release.
- With `promote` set, the job runs `rk lines mint <line> --promote --expect <sha> --apply`, which writes the stable version of the newest candidate. That push releases nothing. The line's release request proposes the version with its changelog, and its merge releases it.
- Every GitLab release job holds a resource group keyed by its branch, so a release on one line never waits on another line's.
- Every job authenticates as the bot: on GitHub the release App's token reaches `rk` as `GH_TOKEN`, and on GitLab the job sets `GITLAB_TOKEN` from `RELEASE_BOT_TOKEN`. `rk` refuses the default CI token, because a push made with it starts no pipeline.
- Where the target records no development environment, each job installs `rk` through the shell installer of the release that landed the pipeline, and installs git-cliff at its pin. Where it records one, each job takes the recorded setup on GitHub or the recorded image and setup lines on GitLab, and runs `rk` through the recorded prefix. The environment must then carry `rk`, and git-cliff at its pin. Where the environment carries no `rk`, record `rk = "installer"` in the table: each job then installs `rk` at the landing version onto its PATH after the setup, and still runs it through the prefix. On GitLab, the installer runs the published GNU/Linux `rk`, so the recorded image must carry `ldd` and the glibc loader. A Debian or Ubuntu based image carries both. The bare `nixos/nix` image carries neither, so a target on it records an image that does.
- The GitLab release jobs run on a Debian image pinned by digest, because the `rk` archive targets glibc. Before they run `rk` they fetch glab, the forge client `rk` drives, and git-cliff, each at its pin and each checked against the SHA-256 `versions.toml` records. Every tool they fetch, `rk` included, goes into `rk-tools/bin` under the temporary directory, outside the checkout, and that directory joins `PATH` before the run prefix. Under a recorded image they still fetch glab, so that image must carry `sh`, `curl`, `tar` with `gzip`, `mkdir`, and `sha256sum`, and needs no `/usr/local`.

## What ships

`[profile.release.build]` in `.release-kit/config.toml` declares what the release ships, as Cargo's `include` and `exclude` fields declare a crate's package. Both keys are lists of gitignore-style patterns:

- `foo` matches a file or a directory of that name at any depth, and `/foo` matches it at the root alone.
- `foo/` matches a directory, `*` and `?` stay within one path segment, `**/` and `/**` cross segments, and `[]` matches a character class.
- `include` overrides `exclude`, and a `!` pattern inside `include` carves out an exception.
- With neither key, every tracked file ships.

```toml
[profile.release.build]
exclude = ["docs/"]
verify = "make check"
```

The verify command runs in the unpacked archive, so the declaration ships every file the command reads. Here the `Makefile` and the `tests/` that `make check` runs stay in the package, and only `docs/` stays out.

The set comes from the tree of the commit, so an untracked or ignored file never ships. `VERSION` always ships, as Cargo always packs `Cargo.toml`. The files `rk` lands, and `.release-kit/`, stay out of the archive unless an `include` pattern names one. A project that wants its landed `SECURITY.md` in the archive names it, as in `include = ["bin/", "lib/", "/SECURITY.md"]`. The declaration alone scopes the package. A `.gitattributes` line, `export-ignore` included, plays no part.

`rk release build` packs the set from the git objects of `HEAD`, never from the working tree. It writes `dist/<name>-<version>.tar.gz`, whose members sit under the one top directory `<name>-<version>/`, and `dist/<name>-<version>.tar.gz.sha256` beside it. `<name>` is the last segment of the project path on the forge. The archive is reproducible:

- Members are sorted by path, with no directory entry.
- Each mode comes from the git mode, `0644`, `0755`, or a symlink.
- Owner and group are 0, with empty names.
- Every time is the commit time that `SOURCE_DATE_EPOCH` carries.
- The gzip header carries no file name and a zero time.

A build of one commit therefore writes the same bytes on every host. The build refuses a package that holds nothing past `VERSION`, and names `profile.release.build.include`.

`rk release build --list` prints the files the package holds at `HEAD`, one path per line, as `cargo package --list` does. It writes nothing and reads no forge, and `--json` prints the same list in the build report's schema. Run it before the first release.

## The verify command

`profile.release.build.verify` names an optional command that proves the package builds and installs from its own files, such as `make check` or `make install DESTDIR="$PWD/stage"`. `rk release build` unpacks the archive into a temporary directory and runs the command there, with `SOURCE_DATE_EPOCH` set. A failure stops the build before any upload, tag, or release.

A generated file, such as a man page built from Markdown, stays out of the package. The package carries its source, and the consumer's install builds it, as a crate's build script does. A project that ships a generated file commits it, and the declaration names it.

`profile.release.build.packages` names the `apt` packages the release job installs before the verify command, `make` by default. The GitHub workflow runs on `ubuntu-latest`, and the GitLab jobs run on Debian. Under a recorded development environment on GitLab, the recorded image replaces Debian, and the environment carries the build's tools.

The key `profile.release.build.command` is retired. A configuration or a record that still carries it refuses before any write, and `rk release request`, `rk release build`, `rk upgrade --apply`, and `rk status --check` each name `include`, `exclude`, and `verify` as its replacement.

## A project that Nix packages

Keep the package derivation in `nix/package.nix`, and let `flake.nix` only call it. Name `nix/package.nix` in the declaration, and leave `flake.nix` and `flake.lock` out:

```toml
[profile.release.build]
include = ["bin/", "lib/", "/nix/package.nix", "/README.md"]
```

A lock update then opens no release request of its own. It releases only when a commit also changes a packed file. The trade-off is plain. A consumer who builds the flake from a release tag gets the lock of that tag. A lock update alone, a security fix in a dependency included, reaches that consumer only with the next release that changes a packed file.

## What opens a request

After a release, `rk release request` proposes a version only where a file the release ships changed since the release tag, or where the release ships a different set of files. This is the rule release-plz applies to a crate. The request reads the declared set at `HEAD` and at the tag, each from its own tree and its own `.release-kit/config.toml`, and builds nothing. The declaration plays the part of the `include` and `exclude` fields of `Cargo.toml`, and Cargo packs `Cargo.toml`, so a declaration change that adds or drops a file counts, even where that file itself is unchanged. A tag whose configuration names neither key packs every tracked file, so the commit that first narrows the set opens one request where its type releases. A declaration restated in another form that selects the same set counts for nothing. git-cliff then reads only the commits that touch the packed files, for the version, the changelog, and the release notes. Where the declaration changed the set, git-cliff also reads the commits on `.release-kit/config.toml`, so the commit that changed it appears in the changelog. The request adds only the new section to the changelog, and every released section keeps its bytes. A change that a later commit reverts counts for nothing.

The files `rk` lands never count. Each revision drops every destination its own `.release-kit/manifest.json` names, and `.release-kit/` itself, also where an `include` pattern ships one. So the commit that takes an `rk` release opens no request. Every other file the declaration ships counts, a CI file included. To keep a file from opening a request, leave it out of the declaration.

A packed change opens a request only with a releasing commit. At least one commit since the release tag that touched the packed files, or `.release-kit/config.toml` where its declaration changed the set, must be typed `feat`, `fix`, or `perf`, or mark a breaking change. Otherwise the request names the commits it read, opens nothing, and closes a stale request, and those commits stay in the changelog of the next request. A declaration change that ships a file changes what ships, so type it as one, for example `fix(packaging): ship uninstall.sh`. A version set by hand, or promoted from a candidate, is judged the same way since the last stable release tag, and keeps its declared number once a releasing commit qualifies. With no release tag, the whole history counts, and the declared version proposes only where a commit that touched the files packed at `HEAD` releases. `rk release eligible` answers the same question at a desk, writes nothing, and reads no forge.

A dependency update that changes what ships is `fix(deps)`, and one for development only is `chore(deps-dev)` or `build(deps-dev)`. A security update is always `fix(deps)`, and its body names the advisory ID.

Where the declaration selects no tracked file yet, the request opens nothing, closes a stale request, and reports "no packed file exists yet: `profile.release.build.include` matches no tracked file".

## The changelog

git-cliff writes `CHANGELOG.md` from the template in the seeded `cliff.toml`. Each request adds one section below the `header` that `cliff.toml` defines, and leaves the rest of the file as it stands. Where the file does not open with that header, git-cliff writes the header and the section above the whole file. git-cliff expects a template that the project writes, so the project owns that template and keeps it. The changelog that it renders is generated all the same: exclude it from every formatter and prose gate, as [generated files](./README.md#generated-files) says, together with the other files that the landing writes.

## Setup specifics

- No registry means no step 0 metadata gate, no bootstrap token, no trusted publisher, and no enforcement switch. Setup is the branch shape, the protections, and the landed files.
- On GitLab, a project declares its own jobs in `.gitlab/ci/project.yml`, which the rendered parent triggers as a child pipeline. [The forge document](../forges/gitlab.md) carries the rules that file follows.
- The landed `VERSION` is `0.0.0`, the unreleased baseline. With no tag in the repository, the first release request proposes the `initial_tag` that `cliff.toml` configures, so the baseline and the computed first version can never collide.
- A project whose verify command is make keeps the Makefile contract: it honours `PREFIX`, `DESTDIR`, `bindir`, `libdir`, and `datadir`. Every downstream packaging tool assumes that contract, so it is the installability gate this binding runs where others dry-run against a registry.
- `rk setup step package-check` judges the verify command before the first release: a make command needs a makefile, and `make -n` must resolve its target where make is on the host. Another command's first word must resolve where the host can tell. The step judges only a plain command. A command with quoting or shell syntax, such as `make check && make install`, reads ok with its resolution not proved, because the build runs it through `sh -c`. It never lists the package and names that gap. Run `rk release build --list` before the first release, and check that the list holds what the project means to ship, `SECURITY.md` among it where the declaration names it.
- On both forges the tag is created only by `rk release publish`, after the build and the provenance exist, so a failed build, provenance, or upload leaves no tag. On GitLab a failed release save can still leave the tag at the pipeline's own commit, and a rerun of `publish` creates the release on it. A tag at another commit stops the job and routes to the recovery section `The tag exists and the release does not`.
- An `install.sh` one-liner must verify the checksum before extracting. A curl-pipe installer that skips it is the one fair complaint against the pattern. The checksum sits on the same release page as the tarball, so it proves the download arrived intact and nothing about where it came from. The attestation is what answers the second question.

## Provenance

- There is no registry, so the release page is the entire distribution surface, and its assets are mutable: whatever can replace the tarball can replace the `.sha256` beside it. This binding needs the attestation more than the ones with a registry behind them, not less.
- The provenance stays in the pipeline and out of `rk`, because the signing identity is the CI job's own.
- Every action this binding's workflows execute is pinned by full commit SHA, per [the invariants](../method/01-invariants.md). The readable tag beside each pin is the discovery ref `versions.toml` classifies and `rk versions --check` resolves.
- On GitHub, the landed `release.yml` attests the artifacts with `actions/attest` in the same job that builds them, which is why the `release` job carries `id-token: write`, `attestations: write`, and `artifact-metadata: write` alongside `contents: read`. The attestation is minted before `rk release publish` runs, and that command creates the tag when it publishes the release. The order is the rule, not an implementation detail: the release page is public the moment it exists, and a page created first would point at an unattested tarball until the attest step ran, permanently if the run failed between the two. A consumer's minimum identity check is `gh attestation verify <tarball> --repo <owner>/<repo>`, which proves the repository alone. The release gate in `rk guide release` step 6 is the stronger form, binding the evidence to the release commit and the signing workflow. An `install.sh` served from the release page is itself an artifact and can be attested and verified the same way before it is run.
- On GitLab, the runner itself writes a SLSA v1 provenance statement for the build when `RUNNER_GENERATE_ARTIFACTS_METADATA` is set. The `provenance` job signs that statement keylessly with `cosign attest-blob --type slsaprovenance1`, isolated from the build, and `rk release publish` publishes the bundle beside the tarball. Signing the runner's statement rather than the bare tarball is what makes it a build-provenance attestation. The statement carries the source commit, the builder, and the build parameters, where a plain signature proves only that some identity signed a digest. No standing private key is managed: keyless signing mints an ephemeral key pair against the job's OIDC identity and discards it, so this stays inside the one signing scheme the channel offers.
- The GitLab half works on GitLab.com only. Sigstore's public instance trusts `gitlab.com` as an OIDC issuer and no self-managed `CI_SERVER_URL`, and the `id_tokens` configuration must live in the project being built and signed. On a self-managed instance the `provenance` job states that limitation and the release goes on without provenance rather than failing. The release page then honestly carries the tarball and checksum alone.
- A consumer on GitLab verifies the bundle with `cosign verify-blob-attestation --type slsaprovenance1 --bundle <tarball>.sigstore.json --certificate-oidc-issuer https://gitlab.com --certificate-identity "https://gitlab.com/<project-path>//.gitlab-ci.yml@refs/heads/<ref>" <tarball>`. The certificate identity embeds the CI configuration path and the ref the release was built from, so a release cut from a `release/*` line verifies against that ref, not against `master`. Obtain cosign the way the pipeline does: download the release binary at the `versions.toml` pin and verify it against its published digest before running it.

## Downstream packaging

- AUR takes two packages: the tag-pinned one and the `-git` variant tracking the default branch.
- OBS covers rpm and deb across openSUSE, Fedora, Debian, and Ubuntu from the same tag, one service file per package.

## Recovery specifics

- There is no registry to withdraw from. A defective release is fixed forward. The defective release page keeps its tag and gains a warning line pointing at the successor, and its tarball stays for inspection.
- A failed publish reruns the release job on GitHub, or `publish` on GitLab, on the same commit, before any tag exists. `rk release build` packs the identical tarball again, and `rk release publish` finishes a draft or a partial page instead of duplicating it.
- A failed build, provenance, or upload leaves no tag, because only `rk release publish` creates it. A rerun after a fix lands is therefore a fresh release request and not a retry: un-bump `VERSION` as [recovery](../method/04-recovery.md#the-release-request-merged-and-nothing-published) states.
