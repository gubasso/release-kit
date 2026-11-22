# Task: provision

Bring `rk` to this host at the version the repository records, through the tool manager the project runs. The gate picks it where the pre-flight finds no `rk` or a record newer than the binary, and where the request asks to install or move `rk`.

## Pre-flight

1. Run `command -v nix mise devbox cargo` to learn which managers the host carries.
2. Select the version to install. Where the request names a release to move the pin to, that release governs. With no such release, read `rk_version` from `.release-kit/manifest.json`, which restores the binary of an existing landing. A target with no record and no named release takes the version the operator names.

## Execution

1. Ask the operator with `AskUserQuestion` which tool manager the project runs. The default is the manager whose file the repository already carries: `flake.nix`, `mise.toml`, or `devbox.json`. No `rk` verb installs or moves `rk`. State the cost of each option:
   - a Nix flake devshell, mise, or devbox: the manager file and its lock record the version for every contributor;
   - asdf: no asdf plugin for release-kit exists, so an asdf user installs the binary from the GitHub release, as a host install;
   - a native host install: no file records the version, so each host installs `rk` itself.
2. Read the Install section of the release-kit `README.md` for that manager. The crate's page, <https://crates.io/crates/release-kit>, carries that file. Take the commands from that section alone.
3. For a manager, placing the install lines of that section in the manager file is a gated edit you make. A host answer records nothing, because no file records a host install.
4. Run the install, or the move of the version the manager file pins, by that manager's own commands, as a gated step on the operator's host.
5. After the pin move, load the devshell and run the upgrade through the rk-setup skill of the rk in that devshell, and do every migration it names. Before `rk upgrade`, read it with `rk skill show rk-setup`. After it, the project's .claude/skills/rk-setup is that skill. Never use a host copy or an older skill. Then route to the `upgrade` task.

## Post-flight

1. In the dev environment, `rk --version` names the version the pre-flight selected. After a move, the `upgrade` task brings the record to it.
2. Where the `skill-shadow` probe of `rk doctor` fails, gate its `next` line for the operator.
3. Run the general pre-flight again, now with the binary.
