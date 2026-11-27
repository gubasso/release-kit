# Bot Key Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`bot-key:the-key-has-one-default-path` — The key has one default path](#bot-keythe-key-has-one-default-path--the-key-has-one-default-path)
  - [`bot-key:three-sources-name-the-key-in-a-fixed-order` — Three sources name the key in a fixed order](#bot-keythree-sources-name-the-key-in-a-fixed-order--three-sources-name-the-key-in-a-fixed-order)
  - [`bot-key:the-import-checks-and-writes-the-key-atomically` — The import checks and writes the key atomically](#bot-keythe-import-checks-and-writes-the-key-atomically--the-import-checks-and-writes-the-key-atomically)
  - [`bot-key:an-unreadable-key-is-an-unmet-requirement` — An unreadable key is an unmet requirement](#bot-keyan-unreadable-key-is-an-unmet-requirement--an-unreadable-key-is-an-unmet-requirement)

<!--TOC-->

## Purpose

Rules governing where `rk` finds the private key of the GitHub release bot App, how `rk setup import-key` puts a downloaded key there, and how a check reports a key file that the current process cannot read. How a run reads the key and hands its bytes to a step is bound by `SPEC-forge-setup.md`, under `forge-setup:key-material-never-reaches-the-environment`. What a check result means is bound by `SPEC-setup-proof.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification.

## Requirements

### `bot-key:the-key-has-one-default-path` — The key has one default path

The default key path MUST be `<root>/release-kit/bot/github/<app-id>.pem`, where `<app-id>` is the resolved numeric App ID. The configuration root MUST be `XDG_CONFIG_HOME` where it is an absolute path, and `$HOME/.config` otherwise. Where neither gives a root, `rk` MUST refuse and name both variables.

The App ID names the file because one key signs for exactly one App. A key at the path of one App ID cannot pair with another App, and one machine serves every project that shares the App.

#### Scenario: The configuration root is relative

- GIVEN `XDG_CONFIG_HOME=relative/config`, `HOME` set, and the App ID `314159`
- WHEN `rk setup import-key` imports a downloaded key
- THEN the key lands at `$HOME/.config/release-kit/bot/github/314159.pem`

Verify: `cargo nextest run -E 'test(the_default_key_path_follows_xdg_config_home) or test(the_default_path_names_the_app_id)'`

### `bot-key:three-sources-name-the-key-in-a-fixed-order` — Three sources name the key in a fixed order

`rk` MUST take the key path from the first of three sources that names one: `RK_BOT_PRIVATE_KEY_FILE`, then the entry for the App ID in the table `[bot.github.private_key_files]` of the user configuration file `<root>/release-kit/config.toml`, then the default path. The App ID MUST resolve as `RK_BOT_APP_ID` first, then the committed `setup.bot.app_id`, for every reader, the `bot-secrets` step included. Without a resolved App ID only the variable MUST apply, and the lookup MUST name the App ID as the missing fact. The user configuration file MUST refuse an unknown key, an App ID that is not digits, and a relative path, each by name. An absent file MUST be no error. Nothing at the default path MUST read as no key imported yet, with the import as its remedy. The committed `.release-kit/config.toml` MUST hold no key path.

#### Scenario: A target commits its App ID and holds an imported key

- GIVEN `setup.bot.app_id = "314159"` committed, a key at the default path, and no `RK_BOT_*` variable
- WHEN `rk setup step bot-secrets --apply` and `rk setup step install-bot --apply` run
- THEN both steps succeed, and the forge stores the App ID from the configuration and the key bytes from the default path

Verify: `cargo nextest run -E 'test(the_key_path_resolves_variable_then_user_config_then_default) or test(the_user_config_refuses_a_relative_path_and_a_non_numeric_app_id) or test(a_missing_default_key_is_absent_and_names_the_import) or test(bot_secrets_takes_the_app_id_from_the_committed_config)'`

### `bot-key:the-import-checks-and-writes-the-key-atomically` — The import checks and writes the key atomically

`rk setup import-key <downloaded.pem>` MUST resolve the App ID as `bot-key:three-sources-name-the-key-in-a-fixed-order` says and refuse without one. It MUST run every check of the key lookup on the download except the mode. It MUST create the key folder with mode 0700 where it is missing, and refuse a folder open beyond its owner with `chmod 700`. It MUST write the bytes to a temporary file with mode 0600 in that folder, then rename the file onto the default path. A key already at the default path MUST refuse unless `--replace` names the rotation.

Where the default path, or a folder on it below the configuration root, is a symbolic link, the import MUST refuse with or without `--replace`, and say that the key is managed elsewhere. Where the variable or the user configuration names another path, the import MUST refuse and name that source. The import MUST read the written key back through the lookup before it deletes the download. A failed check MUST leave the download in place and remove the temporary file. The import MUST print the written path and the next command, and no key byte.

#### Scenario: A link stands at the default path

- GIVEN the default path is a symbolic link to a key that another tool writes
- WHEN `rk setup import-key --replace` runs
- THEN it refuses with "managed elsewhere", and the link and the download stay as they were

Verify: `cargo nextest run -E 'test(import_key_moves_the_download_to_the_default_path_owner_only) or test(import_key_keeps_the_download_when_a_check_fails) or test(import_key_refuses_an_existing_key_without_replace) or test(import_key_refuses_a_symlinked_default_path_even_with_replace) or test(import_key_refuses_where_another_source_names_the_key)'`

### `bot-key:an-unreadable-key-is-an-unmet-requirement` — An unreadable key is an unmet requirement

While `rk setup check` runs on GitHub, a key path from any source that the current process cannot read MUST leave `install-bot` and `bot-secrets` `unreadable`, with a detail that names the requirement, the path, the reason, and two remedies: `rk setup import-key <file>` and `RK_BOT_PRIVATE_KEY_FILE`. The detail MUST name no person, tool, or kind of runtime. A check whose only gaps are `unreadable` steps MUST exit 0 with a summary line that names the unmet requirement. The `--json` events MUST carry `unreadable` as the step status.

#### Scenario: The process cannot read the default path

- GIVEN a fully set-up target, the App ID, and a key folder that the current process cannot read
- WHEN `rk setup check` runs
- THEN both key steps read `unreadable` with the remedy, every other step is observed, and the check exits 0

Verify: `cargo nextest run -E 'test(a_check_reports_an_unreadable_key_as_an_unmet_requirement) or test(the_unreadable_detail_names_only_the_requirement_and_its_remedy) or test(an_observation_keeps_its_distinctions)'`
