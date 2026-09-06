# Dependencies runbook

The steps of [dependencies](../method/11-dependencies.md) as commands, in the chapter's order: the chapter owns each step's why, this page owns its how. `<source>` is the dependency's local checkout; where the request names a URL, the clone is the operator's step before step 2, and the cloned path is the source. The commands are the operator's to run: an agent serves a runbook and states the command, and runs one only where the operator's request named that step.

## 1. Classify the kind

Dev or prod, from the request; where the request does not say, the operator answers before anything is read for a plan.

```bash
rk method dependencies
# check: the two kinds and who lands each; a tool is dev, a library the code imports is prod
```

## 2. Read the source

```bash
rk depend assess --source <source> --target . --json
# check: the source names a package, a version, and at least one channel; source-unknown means the checkout declares nothing this binary reads
```

### 2a. A source given as a URL

Gated: the operator runs the clone.

```bash
git clone --depth 1 --branch <tag> <url> <source>
# check: <source>/Cargo.toml, pyproject.toml, package.json, or flake.nix exists; then step 2 with the path
```

## 3. Read the target

The same assessment reads the target; nothing else runs.

```bash
rk depend assess --source <source> --target . --json
# check: the managers list names the target's tool manager files, and an already line names where the dependency is present
```

## 4. Choose the manager and the channel

One present manager chooses itself. Several, or none, take `--manager`; a channel other than the manager's first takes `--channel`.

```bash
rk depend assess --source <source> --target .
# check: each dev line reads fragment, native, or manual with its reason; the chosen pair reads fragment
```

## 5. Preview

```bash
rk depend add --source <source> --target . --kind dev --manager <manager>
# check: DRY RUN; each fragment prints with its file, placement, and anchor; nothing is written
rk depend add --source <source> --target . --kind prod
# check: the native command prints; nothing is written
```

## 6. Apply, or run the native command

### 6a. A manager file the target lacks

```bash
rk depend add --source <source> --target . --kind dev --manager <manager> --apply
# check: wrote <file>; the manager's lock step follows from the next lines
```

### 6b. A manager file the target owns

`--apply` refuses with exit 73 and the file byte-identical. Apply each fragment by hand at its anchor, in the order printed, then reread.

```bash
rk depend add --source <source> --target . --kind dev --manager <manager> --json
# check: every fragment reports present true
```

### 6c. A manual pair

The report names the reason: an asdf plugin, a nixpkgs attribute, a source hash. The operator supplies it and edits the file; the printed text is the line the file takes once it is known.

### 6d. A prod dependency

Gated: the operator runs the printed command in the target.

```bash
cargo add <name>@<version>
# check: Cargo.toml and Cargo.lock name the library; uv add and npm install are the python and node forms the report prints
```

## 7. Verify

On flake:

```bash
nix flake lock
direnv reload
<bin> --version
# check: the pinned version answers from the flake shell; commit flake.nix and flake.lock together
```

On mise:

```bash
mise install
mise which <bin>
# check: the path is under the mise installs directory at the pinned version
```

On devbox:

```bash
devbox install
devbox run -- <bin> --version
# check: the pinned version answers; commit devbox.json and devbox.lock together
```

On prod:

```bash
cargo tree -i <name>
# check: the library appears once at the pinned version; uv tree and npm ls are the python and node forms
```

### 7a. The divergent rerun

A pin that does not resolve is a version the channel does not serve: rerun step 5 with `--pin` at a published release, and the manager's own update verb moves it from then on.
