# Dependencies Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`dependencies:add-edits-no-file-the-target-owns` — Add edits no file the target owns](#dependenciesadd-edits-no-file-the-target-owns--add-edits-no-file-the-target-owns)
  - [`dependencies:a-prod-dependency-lands-through-the-native-command` — A prod dependency lands through the native command](#dependenciesa-prod-dependency-lands-through-the-native-command--a-prod-dependency-lands-through-the-native-command)
  - [`dependencies:the-assessment-is-offline-and-exits-zero` — The assessment is offline and exits zero](#dependenciesthe-assessment-is-offline-and-exits-zero--the-assessment-is-offline-and-exits-zero)
  - [`dependencies:the-channel-follows-the-source-evidence` — The channel follows the source evidence](#dependenciesthe-channel-follows-the-source-evidence--the-channel-follows-the-source-evidence)
  - [`dependencies:an-unjudgeable-pair-is-manual-with-its-reason` — An unjudgeable pair is manual with its reason](#dependenciesan-unjudgeable-pair-is-manual-with-its-reason--an-unjudgeable-pair-is-manual-with-its-reason)
  - [`dependencies:the-version-comes-from-the-source-tree` — The version comes from the source tree](#dependenciesthe-version-comes-from-the-source-tree--the-version-comes-from-the-source-tree)
  - [`dependencies:a-fragment-names-no-project-of-its-own` — A fragment names no project of its own](#dependenciesa-fragment-names-no-project-of-its-own--a-fragment-names-no-project-of-its-own)
  - [`dependencies:the-source-is-a-local-checkout` — The source is a local checkout](#dependenciesthe-source-is-a-local-checkout--the-source-is-a-local-checkout)

<!--TOC-->

## Purpose

Rules governing how the binary takes another project as a dependency of a target through `rk depend`: what it reads from the source and the target, what it serves for each manager and channel pair, and what it writes. The boundary against `SPEC-packaging.md` is the dependency: that spec binds how a consumer pins release-kit itself through `rk devshell`, and this one binds every other project a target takes. The files `rk init` lands are bound by `SPEC-landing.md`. External sources are recorded in `../reference/REFERENCE-dependencies-sources.md`.

## Requirements

### `dependencies:add-edits-no-file-the-target-owns` — Add edits no file the target owns

When `rk depend add --apply` runs against a target that already carries the chosen manager's file, the binary MUST print the fragments with their anchors, leave the file byte-identical, and exit 73 naming the reason, because a lexical observation does not justify a write into another project's manager file.

#### Scenario: The target already has a flake

- GIVEN a target with its own `flake.nix`
- WHEN `rk depend add --kind dev --manager flake --apply` runs
- THEN the three fragments print, `flake.nix` is unchanged, and the run exits 73 with reason `destructive-refusal`

Verify: `cargo nextest run -E 'test(depend_add_apply_refuses_a_manager_file_the_target_owns)'`

### `dependencies:a-prod-dependency-lands-through-the-native-command` — A prod dependency lands through the native command

The binary MUST report the target technology's own command for a prod dependency and MUST NOT edit `Cargo.toml`, `pyproject.toml`, or `package.json`, because a manifest and its lock are written by the tool that resolves them.

#### Scenario: A rust library for a rust target

- GIVEN a rust target and a source that names a crate
- WHEN `rk depend add --kind prod` runs, with or without `--apply`
- THEN the report carries `cargo add <name>@<version>`, nothing is written, and `--apply` exits 64

Verify: `cargo nextest run -E 'test(depend_add_prod_prints_the_native_command_and_refuses_apply) or test(prod_returns_the_native_command_per_technology)'`

### `dependencies:the-assessment-is-offline-and-exits-zero` — The assessment is offline and exits zero

`rk depend assess` MUST read both trees offline, write nothing, and exit 0 on every verdict, because a classification that fails or fetches cannot be the first step of every plan.

#### Scenario: A source that declares nothing

- GIVEN an empty source directory and any target
- WHEN `rk depend assess` runs
- THEN the verdict is `source-unknown`, the exit code is 0, and no file changed in either tree

Verify: `cargo nextest run -E 'test(/^depend_assess_/)'`

### `dependencies:the-channel-follows-the-source-evidence` — The channel follows the source evidence

The binary MUST offer a channel only where the source carries its signal: a `Cargo.toml` package for crates, a flake serving a `packages` output for flake, a `pyproject.toml` project for pypi, a `package.json` package for npm, and, on a GitHub remote, a `dist-workspace.toml` naming GitHub as its CI and hosting or binstall metadata resolving to GitHub releases for github-release.

#### Scenario: A crate with release archives on another host

- GIVEN a rust source with `dist-workspace.toml` whose remote is not `github.com`, or whose binstall `pkg-url` names another host
- WHEN the channels are read
- THEN crates is offered and github-release is not

Verify: `cargo nextest run -E 'test(the_channels_follow_the_evidence) or test(dist_hosting_is_read_from_the_dist_table)'`

### `dependencies:an-unjudgeable-pair-is-manual-with-its-reason` — An unjudgeable pair is manual with its reason

Where a manager cannot take a channel without knowledge the binary does not have offline, the binary MUST report the pair as manual with a closed reason and MUST NOT guess an attribute, a plugin name, or a hash.

#### Scenario: An asdf target

- GIVEN a target with `.tool-versions` and a source with three channels
- WHEN the options are computed
- THEN every pair is manual with reason `asdf-plugin-unknown`, the line the plugin would take is printed, and no seed is offered

Verify: `cargo nextest run -E 'test(every_pair_in_the_matrix_is_classified_once) or test(asdf_is_always_manual_with_its_reason)'`

### `dependencies:the-version-comes-from-the-source-tree` — The version comes from the source tree

The binary MUST pin the version the source tree declares, in the tag shape the source's tags show, with `--pin` as the one override, because the registry is not consulted and an invented tag fails at the first fetch.

#### Scenario: A source that tags without a prefix

- GIVEN a source whose tags are `1.3.0` and `1.4.0` and whose manifest declares `1.4.0`
- WHEN the pin resolves with no `--pin`
- THEN the version is `1.4.0` and the tag is `1.4.0`

Verify: `cargo nextest run -E 'test(the_tag_follows_the_source_tag_style) or test(the_argument_overrides_the_tree)'`

### `dependencies:a-fragment-names-no-project-of-its-own` — A fragment names no project of its own

Every `blocks/depend-*.in` MUST carry only `RK_DEP_*` tokens where a name, owner, tag, or version goes, because the payload names no project other than release-kit.

#### Scenario: Every block renders

- GIVEN a full token set
- WHEN every depend block renders
- THEN no `RK_DEP_` token remains and the payload scan finds no foreign name

Verify: `cargo nextest run -E 'test(the_payload_names_no_other_project) or test(every_depend_block_renders_all_its_tokens)'`

### `dependencies:the-source-is-a-local-checkout` — The source is a local checkout

When `--source` is a URL, the binary MUST refuse with a usage error naming the clone as the operator's step, because a read verb fetches nothing.

#### Scenario: A GitHub URL as the source

- GIVEN `--source https://github.com/owner/repo`
- WHEN `rk depend assess` or `rk depend add` runs
- THEN the run exits 64 before either tree is read

Verify: `cargo nextest run -E 'test(depend_refuses_a_url_source)'`
