# Setup

This repository's instance of the one-time bootstrap. The procedure — every step, its checks, its `Automated:` forms, and its hand forms — is the shipped setup runbook; this page carries only what is this repository's own.

## Coordinates

Export these once; the rendered runbook's commands read `$OWNER/$REPO` through `--repo`, and `$APP` names the App in the walkthrough. [README.md](./README.md) says what each one is.

```bash
export OWNER=<account or organization that owns the repository>
export REPO=<repository name>
export CRATE=<package name as published to the registry>
export APP=<the bot App's name>
```

## The procedure

```bash
rk guide setup --forge github --technology rust --repo "$OWNER/$REPO"
```

The bot-App walkthrough it routes to is `rk forge github`; the registry forms are in `rk binding rust`. The bot steps read the App ID from the committed `setup.bot.app_id`, and the key from the default path, `${XDG_CONFIG_HOME:-$HOME/.config}/release-kit/bot/github/<app-id>.pem`, where `rk setup import-key` puts the downloaded `.pem`. The runbook's step 2b states the two overrides.

## What this repository settles

- The required check is `gate`: the job id its own `ci.yml` reports, needing `test`, `dist-plan`, and `dist-build`, and the one value the landing does not write on a GitHub target. Name a CI job otherwise and the required check, the ruleset, and the runbook's step 3 prerequisite all move with it.
- The commit scope is held by shape rather than by a list: the landed title check admits lowercase letters, digits, and `_ . / -`, and the landed `AGENTS.md` block tells the author to prefer a scope this repository's history already carries.
- The bot App exists, is installed on this repository, and its credentials are stored; rerunning the setup here verifies rather than creates, which is every step's rerun shape anyway.
