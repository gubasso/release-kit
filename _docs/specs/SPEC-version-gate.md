# Version Gate Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`version-gate:every-command-has-a-gate-class` — Every command has a gate class](#version-gateevery-command-has-a-gate-class--every-command-has-a-gate-class)
  - [`version-gate:a-gated-command-names-its-fix` — A gated command names its fix](#version-gatea-gated-command-names-its-fix--a-gated-command-names-its-fix)
  - [`version-gate:integrate-judges-the-branch-record` — Integrate judges the branch record](#version-gateintegrate-judges-the-branch-record--integrate-judges-the-branch-record)

<!--TOC-->

## Purpose

Rules governing the check that every `rk` command makes before its handler runs: the class of each action, and what a gated action does when the landing record of its target and the binary disagree. The record and the verbs that write it are bound by `SPEC-landing.md`, and the skills that follow `rk_version` are bound by `SPEC-distribution.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The decision is `../decisions/ADR-gate-every-command-on-the-landing-version.md`.

## Requirements

### `version-gate:every-command-has-a-gate-class` — Every command has a gate class

The binary MUST give every action of every command family exactly one class and one target source in one table beside `Commands`, and MUST read the record of a gated action's target before the handler runs, because an action without a class runs against a landing of another release unchecked.

| Class         | What it covers                                                                                                                                                                                                                                       |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `exempt`      | An action that needs no target or must never fail: help, version, `doctor`, `usage`, `license`, `completions`, `method`, `binding`, `snippet`, `forge`, `guide`, `versions`, every `skill` action, `runs`, every `self-depend` action, and `status`. |
| `pre-landing` | An action that runs on a target with no record by design: `assess`, `init`, `adopt`, `migrate`, `stage`, `profile`, and every `setup` action. On a target with a record, it follows `read-only` or `writing` by its own effect.                      |
| `upgrade`     | `upgrade` alone, which keeps its own checks: it refuses a missing record and a record newer than the binary, and it repairs a binary newer than the record.                                                                                          |
| `read-only`   | An action that reads a landed target and writes nothing, and the preview of a writing action.                                                                                                                                                        |
| `writing`     | An action that writes a landed target or its forge, and every `--apply`.                                                                                                                                                                             |

#### Scenario: A new action lands without a class

- GIVEN a new subcommand added to the clap tree with no row in the class table
- WHEN the test suite runs
- THEN the walk over the clap tree fails and names the action

Verify: `cargo nextest run -E 'test(every_action_has_a_gate_class) or test(an_exempt_action_runs_without_a_record) or test(the_first_setup_runs_before_any_record)'`

### `version-gate:a-gated-command-names-its-fix` — A gated command names its fix

When the record and the binary disagree, a `read-only` or `writing` action MUST act by the table below, and MUST name the command that fixes the mismatch, because a refusal that names no fix sends the reader to search.

| State of the target          | Writing action                    | Read-only action                                              |
| ---------------------------- | --------------------------------- | ------------------------------------------------------------- |
| No record                    | Refuses and names `rk init`       | Refuses and names `rk init`                                   |
| Binary newer than the record | Refuses and names `rk upgrade`    | Runs, with one warning line on stderr that names `rk upgrade` |
| Record newer than the binary | Refuses and names the environment | Refuses and names the environment                             |
| Versions match               | Runs                              | Runs                                                          |

A refusal carries the reason `StateDrift` and exits 73, and under `--json` it renders as the JSON error envelope. A warning never enters the JSON body, and the action keeps its own exit code. The refusal on a target with no record also names `rk skill show rk-setup` and `rk skill show rk-setup --file <path>`. A record newer than the binary names both versions and the rk of the project's own environment, for example through `direnv exec .`, and it names the pin as the fix, with the `rk self-depend sync --apply --tag` that moves it, only where `self-depend` reads a pin older than the record. The refusal of a writing action from a binary newer than the record, run inside the project's environment, also carries the route through the setup skill of that binary, under `packaging:the-unattended-caller-never-fails-the-shell`, because a pin move is how a project reaches that state. The gate never writes, never moves a pin, and never runs `init` or `upgrade` by itself. The record is read through the legacy converter, so an old schema asks for `rk upgrade`, and a corrupt record is an error.

#### Scenario: A pin move is committed before its upgrade

- GIVEN a landed project whose devshell pin moved to a newer rk while the record names the older one
- WHEN the commit-msg hook runs `rk message --check`, and then `rk integrate` runs
- THEN the hook warns once and passes, and `rk integrate` exits 73 naming `rk upgrade` with the target unchanged

Verify: `cargo nextest run -E 'test(a_writing_action_refuses_a_target_with_no_record) or test(an_upgrade_repairs_an_older_landing) or test(a_writing_action_refuses_when_the_binary_is_newer) or test(a_read_only_action_warns_when_the_binary_is_newer) or test(a_record_newer_than_the_binary_names_the_environment) or test(the_warning_never_enters_the_json_body)'`

### `version-gate:integrate-judges-the-branch-record` — Integrate judges the branch record

`rk integrate` MUST judge the `rk_version` of the record at the tip of its branch by the rows above, in place of the record in the target's working tree, because the integration makes the branch record the trunk's. It MUST read that field alone, so a record at a newer schema still names its release, and a branch with no record at its tip MUST fall back to the target's record. A refusal MUST name the branch and the worktree that has it checked out. Where the branch record is newer than the binary, the refusal MUST also name the route from inside the worktree with `--target .`, because a release before this rule judges the trunk record and that route works on every release.

#### Scenario: A pin-move branch integrates from the main checkout

- GIVEN a branch whose pin move and upgrade record a newer rk than the trunk
- WHEN the rk of the branch runs `rk integrate <branch>` against the main checkout, and then an older rk does
- THEN the first passes the gate, and the second exits 73 naming the branch, its worktree, and the route from inside it, with the target unchanged

Verify: `cargo nextest run -E 'test(integrate_judges_the_record_of_its_branch) or test(integrate_falls_back_to_the_target_record_without_a_branch_record)'`
