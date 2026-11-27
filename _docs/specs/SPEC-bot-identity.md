# Bot Identity Specification

<!--TOC-->

- [Purpose](#purpose)
- [Requirements](#requirements)
  - [`bot-identity:the-bot-identity-reaches-one-project` — The bot identity reaches one project](#bot-identitythe-bot-identity-reaches-one-project--the-bot-identity-reaches-one-project)
  - [`bot-identity:the-bot-identity-serves-the-release-alone` — The bot identity serves the release alone](#bot-identitythe-bot-identity-serves-the-release-alone--the-bot-identity-serves-the-release-alone)

<!--TOC-->

## Purpose

Rules governing the bot identity a setup creates on a forge: which form it takes, what it may reach, and how a run names it. A release automation acts through that identity, so its reach is the reach of every release it makes. How a run reaches the forge and what each step asserts is bound by `SPEC-forge-setup.md`, and the declaration `setup.bot.name` is bound by `SPEC-target-config.md`. Where the GitHub bot's private key lives and how a run finds it is bound by `SPEC-bot-key.md`. Who changes the release refs is bound by `SPEC-release-ref-rules.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The forge documentation these rules rest on is in `../reference/REFERENCE-forge-setup-sources.md`.

## Requirements

### `bot-identity:the-bot-identity-reaches-one-project` — The bot identity reaches one project

Where `release.automation` is selected, the setup MUST create a bot identity whose membership reaches the one project it serves and nothing else, MUST choose the identity form from what the forge and the tier allow at the time the step runs, and MUST refuse with the role and the reason where no form is available. On GitLab a self-managed or Dedicated instance and a paid GitLab.com namespace take a project access token, a GitLab.com Free group takes a group service account granted this project at Maintainer, and a Free personal namespace refuses before any write. The tier is read on every run and recorded nowhere. An existing account under the configured name that is not a member of this project, and a rename over a live identity, MUST refuse before any grant, naming the account.

#### Scenario: A Free group with no identity yet

- GIVEN a GitLab.com project whose namespace reads `plan: free` and `kind: group`, and no service account under the bot's name
- WHEN `rk setup step install-bot --apply` runs
- THEN it creates the service account with no email, grants it this project at Maintainer, mints its token with `scopes` as a JSON array, stores the value from stdin, and `rk setup check` reports the membership and the stored variable

Verify: `cargo nextest run -E 'binary(cli) and (test(install_bot_) or test(the_observer_reads_the_service_account_form) or test(bot_secrets_mints_on_the_service_account_where_the_variable_is_missing) or test(bot_secrets_rotates_a_stored_service_account_token_when_named) or test(bot_secrets_keeps_a_stored_project_token_when_named) or test(bot_secrets_refuses_a_named_rotation_it_cannot_classify))'`

### `bot-identity:the-bot-identity-serves-the-release-alone` — The bot identity serves the release alone

The bot identity MUST carry the release request, the release tags, and the release lines, and no other actor's work. A protection that the setup installs MUST name the bot as a bypass actor only where a release needs it, and MUST name no other automation. On GitHub, each release ruleset MUST name exactly the repository administrator role and the release App, which the resolved App ID names. `protections-check` MUST fail a bypass that names any other actor. Any other automation acts as an identity of its own, which these protections refuse on the trunk, the release tags, and the release branches.

#### Scenario: A second App with write access

- GIVEN a GitHub target with the release App, and a second App installed with write access
- WHEN `rk setup --apply` runs, and then `rk setup check`
- THEN the check passes, each release ruleset names the administrator role and the release App alone, and a bypass that adds the second App fails `protections-check`

Verify: `cargo nextest run -E 'test(the_release_bypass_names_the_admin_role_and_the_release_app) or test(protections_check_fails_on_a_wider_bypass)'`
