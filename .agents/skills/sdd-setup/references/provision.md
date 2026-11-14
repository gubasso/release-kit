# Task: provision

Bring `sdd` to this host at the version the repository records, and record in the repository how every contributor obtains it. The gate picks it where the pre-flight finds no `sdd`, an instance newer than the binary, or a first landing or a migration with no pin and no recorded host decision.

## Pre-flight

1. Run `command -v nix mise asdf devbox direnv` to learn which managers and loaders the host carries.
2. Where `sdd` runs, run `sdd self-depend status --target . --json`. Hold `wired`, `envrc_check`, and `envrc_old_line`, and the state of each manager.

## Step A: how the project obtains sdd

1. Where no `sdd` runs yet, get one for this session first, as a gated step on the operator's host: enter the manager that already pins it (`nix develop`, `mise install`, `asdf install`, or `devbox shell`), or else run `cargo binstall spec-driven-docs`. Every later step runs it, and `sdd docs self-depend` names the managers and the venues each one serves.
2. Ask the operator for the method. The default is the manager the repository already carries. State the cost of each option:
   - a Nix flake devshell, mise, asdf, or devbox: the manager file and its lock record the version for every contributor;
   - a native host install: no file records the version, so each host installs `sdd` itself, and the declaration records only the minimum.
3. For a manager, read the preview of `sdd self-depend add --target . --manager <manager>`, which prints the fragment and writes nothing. Where it names a seed, gate the same command with `--apply`. Where it seeds nothing, the manager file exists, and placing the fragment it printed is a gated edit you make.
4. For a native host install, record the decision. With an instance, gate `sdd self-depend add --target . --host --apply`. With none, hold the host answer: the Stage step of `land` or `migrate` takes `--acquisition host`, and `sdd init --stage <root> --apply` records it.
5. Where the operator declines the offer before a first landing or a migration, record the decline as a host decision, the same way.
6. Make the binary run in this session, as a gated step on the operator's host: `nix develop`, `mise install`, `asdf install`, `devbox shell`, or `cargo binstall spec-driven-docs` for a host install.

## Step B: how the dev environment loads

1. Ask whether the environment loads through direnv, another loader, or none.
2. With direnv, the loader line the manager needs goes into `.envrc` as a gated edit.
3. Where `wired` names a manager, this step does not close until the operator has answered about the freshness wire. Where `wired` is `null`, there is no pin to keep fresh, and where `envrc_check` is `true`, the wire is landed: in either case say nothing and continue.
4. Where `envrc_old_line` is `true`, the `.envrc` still runs the retired line `sdd self-depend sync --apply || true`, which no longer moves the pin. Gate `sdd self-depend sync --apply` first, because a check line under an old pin only warns. Run it from the loaded shell or with `nix develop -c`, because under `direnv exec` it moves nothing and exits 1. Then gate `sdd self-depend clean --target . --apply`, which swaps the retired line for the check line and keeps every other line byte for byte. Re-observe with the status verb, and continue at the next step only where `envrc_check` is still `false`.
5. Where `envrc_check` is `false`, ask with `AskUserQuestion` whether the project lands the line `sdd self-depend check || true` in its `.envrc`. State what the line does: on each directory entry it warns on stderr when a newer release is out, it reads the network at most once every seven days, it writes no file but its cache, and it never moves the pin. Moving the pin stays the operator's act, `sdd self-depend sync --apply`, which leaves a diff for the operator to review and commit.
6. On yes, landing the line is a gated step and you place it, because `sdd self-depend add --target . --manager <wired>` prints the fragment, edits no file the project owns, and seeds `.envrc` only where the target has none. Where that read-only run names `.envrc` among its seeds, gate the same command with `--apply`. Where it seeds nothing, the target already carries an `.envrc`, and you append the line to it yourself. Then re-observe with the status verb until `envrc_check` reads `true`.
7. On no, the landing is complete without the wire. Record the answer in the report, so the question reads as asked and answered.

## Post-flight

1. In the dev environment, `sdd --version` names the version the repository records.
2. `sdd self-depend status --target . --json` names the wired manager or the host decision, and `envrc_check` matches the answer to the freshness wire. A held host answer still reads `unwired` here, because only the landing records it, and the `land` or `migrate` post-flight proves it.
3. Run the general pre-flight again, now with the binary.
