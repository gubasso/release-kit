# 11 — Dependencies

How a project takes another project as a dependency, from what that project declares about its own distribution and what this project already uses to manage its tools. [Setup](./02-setup.md) decides once how a project obtains `rk`; this chapter owns every other dependency: a tool the developers run, or a library the code imports. The command form of this chapter is the dependencies runbook, `rk guide dependencies`; `rk depend assess` reads both sides, and `rk depend add` serves one landing.

## The two kinds

A dependency is one of two kinds, and the kind decides who lands it.

- dev: a tool on `PATH` in the development environment. The project's tool manager owns it: a flake input and its package in the devshell, a mise `[tools]` entry, an asdf `.tool-versions` line, or a devbox package. `rk depend add` serves the text for the manager's file.
- prod: a library the code imports. The technology's own manifest owns it, and the technology's own command writes it: `cargo add`, `uv add`, `npm install`. `rk depend add` prints that command and writes nothing, because a manifest and its lock are edited by the tool that resolves them.

The kind is the first question, asked before any file is read for a plan, because a tool pinned as a library or a library pinned as a tool is a landing in the wrong owner.

## What the source declares

The source is a local checkout, never a URL: a fetch is the operator's step, and a read verb implies none. Every channel is a file in that checkout or a fact of its remote and tags, read offline.

| Channel        | Evidence                                                                                                                                            | Pin                                    |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| crates         | `Cargo.toml` names a package                                                                                                                        | the crate version                      |
| flake          | `flake.nix` serves a `packages` output                                                                                                              | the flake reference at the release tag |
| pypi           | `pyproject.toml` names a project                                                                                                                    | the project version                    |
| npm            | `package.json` names a package                                                                                                                      | the package version                    |
| github-release | `dist-workspace.toml` naming GitHub as its CI and hosting, or binstall metadata whose package URL is the GitHub release default, on a GitHub remote | the release archive at the tag         |

No registry is asked whether the version is published: the assessment reads the tree. A source that declares no version is reported as `version-unknown`, and `--pin` names the release. The version is the one the source tree declares, and the tag takes the shape the source's own tags show, `v1.2.3` or `1.2.3`; `--pin` is the one override.

## What the target manages

Four managers are recognized by their files, in the order the reports list them: a flake by `flake.nix`, mise by its configuration file or `conf.d` directory in the precedence mise documents, asdf by `.tool-versions`, devbox by `devbox.json`. A flake input takes a Nix identifier derived from the package name, because a scoped or dotted package name is not one. A target with one manager needs no choice; a target with several, or none, is asked which one, because a second manager for one tool is two pins that drift apart. A target with no manager file is seeded one, and only then: `rk depend add --apply` writes a manager file where the target has none and never edits one the target owns. An owned file takes the printed fragments by hand, each at its named anchor, in the order printed.

## The matrix

A manager takes a channel as a fragment, or names why it cannot.

| Manager | Fragment channels                 | Manual, with the reason                                                                                      |
| ------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| flake   | flake                             | crates, pypi, npm: the nixpkgs attribute is unknown offline; github-release: a source hash needs the network |
| mise    | crates, github-release, pypi, npm | flake: mise has no flake backend                                                                             |
| asdf    | none                              | every channel: the plugin name is the operator's knowledge                                                   |
| devbox  | flake                             | crates, pypi, npm, github-release: the nixpkgs attribute is unknown offline                                  |

A manual pair is a report, never a guess: the text the manager would take is printed where one exists, the reason is named, and nothing is written. A flake reference exists for `github.com` and `gitlab.com`; a source on another host makes the flake pairs manual too.

For a prod dependency the matrix is the technology: a rust target takes a crates source through `cargo add`, a python target a pypi source through `uv add`, a node target an npm source through `npm install`. A source of another technology is a mismatch the report names, not a command.

## The sequence

1. Classify the kind. Dev or prod, from the request or the operator's answer.
2. Read the source. `rk depend assess --source <source>` reads the checkout for its channels, version, executables, and remote.
3. Read the target. The same assessment reads the target for its managers, its technology, and where the dependency is already named.
4. Choose the manager and the channel. One present manager chooses itself; several, or none, are the operator's choice. The channel defaults to the first the manager prefers among those the source offers.
5. Preview. `rk depend add` prints the fragments with their anchors, or the native command, and writes nothing.
6. Apply, or run the native command. `--apply` seeds an absent manager file; an owned file takes the fragments by hand; a prod dependency runs the printed command.
7. Verify. The tool answers on `PATH` from the manager's shell, or the manifest and its lock name the library.

## Freshness

`rk depend` has no mover. The manager that took the pin moves it: `nix flake update <input>` for a flake, `mise upgrade --bump` for mise, an edit and `asdf install` for asdf, `devbox update` for devbox, and the technology's own update verb for a library. The report names the verb beside the landing so a project knows what keeps the pin current before it commits the pin.

## Boundary tests

- A library is never a tool pin: a prod dependency lands through the technology's command, whatever manager the target carries.
- An owned manager file is never edited: the fragments are printed, and `--apply` refuses.
- A pair the binary cannot judge offline is manual with its reason, never an invented attribute or plugin.
- The version comes from the source tree, and the tag from the source's tags; the registry is not consulted.
- A URL is not a source: the clone is the operator's step, then the path is the source.
