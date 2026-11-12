# Bash binding

| Axis                | Answer                                              |
| ------------------- | --------------------------------------------------- |
| Version file        | `VERSION`, one line                                 |
| Release-request bot | the rk engine, with git-cliff                       |
| Registry and auth   | none                                                |
| Artifact builder    | the build command's tarball, `make dist` by default |

`rk init --technology bash --release-driver bash` lands `VERSION`, `cliff.toml`, and the release pipeline: `.github/workflows/release.yml` on GitHub, `.gitlab-ci.yml` on GitLab.

## The shape

There is no registry, so stages 4 and 5 of [the spine](../method/00-model.md) collapse into the tag and the release page: publishing is tagging, and the tarball attached to the release is the distribution. The one-pull-request shape is unchanged. The release request bumps `VERSION` and the changelog, and merging it is what makes the pipeline tag and attach.

A one-line `VERSION` file is the committed source of truth. No upstream tool keeps one release request for this file on both forges, so the `rk` binary runs the release itself, as `rk release request`, `rk release build`, and `rk release publish`. `_docs/decisions/ADR-run-the-release-through-the-rk-engine.md` records why. The engine asks git-cliff for the next version, which git-cliff computes from Conventional Commits, decides the number itself, and writes the changelog through git-cliff, which is what qualifies the pair under [the invariants](../method/01-invariants.md). The engine runs git-cliff at the version `versions.toml` pins, and refuses any other.

## The workflows

Each pipeline runs `rk` in every release job and holds no release logic of its own. `rk` reads the trunk, the style, the integration, and the line prefix from the landing record.

- `release-request` runs `rk release request` on every push to the trunk and to a release line. It keeps one request per base on the branch `rk-release/<base>`, refreshes it in place, closes it when nothing remains to release, and arms it under the trunk style on the trunk. On GitHub under local integration the rendered `release-gate` job merges it instead, and the engine leaves it unarmed.
- On GitHub, the `release` job runs `rk release build`, `actions/attest` over the artifacts the build reports, and `rk release publish`. The attest step runs only where the build reports `release` or `resume`.
- On GitLab, `build` runs `rk release build`, `provenance` signs the runner's statement over the tarball, and `publish` runs `rk release publish` after both.
- The build releases a final version only at the commit that merges the engine's own request, and a candidate only at the commit that writes it. Every other push skips with success.
- Every job authenticates as the bot: on GitHub the release App's token reaches `rk` as `GH_TOKEN`, and on GitLab the job sets `GITLAB_TOKEN` from `RELEASE_BOT_TOKEN`. `rk` refuses the default CI token, because a push made with it starts no pipeline.
- Where the target records no development environment, each job installs `rk` through the shell installer of the release that landed the pipeline, and installs git-cliff at its pin. Where it records one, each job takes the recorded setup on GitHub or the recorded image and setup lines on GitLab, and runs `rk` through the recorded prefix. The environment must then carry `rk`, and git-cliff at its pin.
- The GitLab release jobs run on a Debian image pinned by digest, because the `rk` archive targets glibc. Before they run `rk` they fetch glab, the forge client `rk` drives, and git-cliff, each at its pin and each checked against the SHA-256 `versions.toml` records. Every tool they fetch, `rk` included, goes into `rk-tools/bin` under the temporary directory, outside the checkout, and that directory joins `PATH` before the run prefix. Under a recorded image they still fetch glab, so that image must carry `sh`, `curl`, `tar` with `gzip`, `mkdir`, and `sha256sum`, and needs no `/usr/local`.

## The changelog

git-cliff writes `CHANGELOG.md` from the template in the seeded `cliff.toml`. git-cliff expects a template that the project writes, so the project owns that template and keeps it. The changelog that it renders is generated all the same: exclude it from every formatter and prose gate, as [generated files](./README.md#generated-files) says, together with the other files that the landing writes.

## Setup specifics

- No registry means no step 0 metadata gate, no bootstrap token, no trusted publisher, and no enforcement switch. Setup is the branch shape, the protections, and the landed files.
- On GitLab, a project declares its own jobs in `.gitlab/ci/project.yml`, which the rendered parent triggers as a child pipeline. [The forge document](../forges/gitlab.md) carries the rules that file follows.
- The landed `VERSION` is `0.0.0`, the unreleased baseline. With no tag in the repository, the first release request proposes the `initial_tag` that `cliff.toml` configures, so the baseline and the computed first version can never collide.
- The build command is `profile.release.build.command`, `make dist` where nothing answers it, and `rk release build` runs it from the record. The packages the release image installs before the build are `profile.release.build.packages`, `make` by default. They are `apt` names on both forges: the GitHub workflow runs on `ubuntu-latest`, and the GitLab jobs run on Debian. Under a recorded development environment on GitLab the recorded image replaces Debian, and the environment carries the build's tools.
- The command's output contract is `dist/<name>.tar.gz` and `dist/<name>.tar.gz.sha256`, the tarball a pure function of the tree. The build refuses a run that leaves either file out, so the contract does not move with the command.
- A project whose command is make keeps the Makefile contract: it honours `PREFIX`, `DESTDIR`, `bindir`, `libdir`, and `datadir`. Every downstream packaging tool assumes that contract, so it is the installability gate this binding runs where others dry-run against a registry.
- `rk setup step package-check` judges the command before the first release: a make command needs a makefile, and `make -n` must resolve its target where make is on the host. Another command's first word must resolve where the host can tell. It never inspects the tarball and names that gap: `git archive` honours the `export-ignore` attribute, so a tracked file can be absent. Run the command and `tar -tzf` the result before the first release, and check that `SECURITY.md` is in it.
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
- A failed publish reruns the release job on GitHub, or `publish` on GitLab, on the same commit, before any tag exists. The build command reproduces the identical tarball, and `rk release publish` finishes a draft or a partial page instead of duplicating it.
- A failed build, provenance, or upload leaves no tag, because only `rk release publish` creates it. A rerun after a fix lands is therefore a fresh release request and not a retry: un-bump `VERSION` as [recovery](../method/04-recovery.md#the-release-request-merged-and-nothing-published) states.
