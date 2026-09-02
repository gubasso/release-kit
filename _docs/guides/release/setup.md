# Setup

This repository's instance of the one-time bootstrap. The procedure — every step, its checks, its `Automated:` forms, and its hand forms — is the shipped setup runbook; this page carries only what is this repository's own.

## Coordinates

Export these once; the rendered runbook's commands read `$OWNER/$REPO` through `--repo`, and the remaining exports feed the bot steps. [README.md](./README.md) says what each one is.

```bash
export OWNER=<account or organization that owns the repository>
export REPO=<repository name>
export CRATE=<package name as published to the registry>
export APP=<the bot App's name>
export APP_ID=<the bot App's numeric id>
export KEY=<absolute path to the App's .pem, outside every repository>
```

## The procedure

```bash
rk guide setup --forge github --tech rust --repo "$OWNER/$REPO"
```

The bot-App walkthrough it routes to is `rk forge github`; the registry forms are in `rk binding rust`. The bot steps read the credentials as `RK_BOT_APP_ID="$APP_ID"` and `RK_BOT_PRIVATE_KEY_FILE="$KEY"`.

## What this repository settles

- The required check is `test`: the job id its own `ci.yml` reports, the one value the payload does not write on a GitHub target. Name a CI job otherwise and the required check, the ruleset, and the runbook's step 3 prerequisite all move with it.
- The commit scopes `rk init --scopes` rendered into the title check and the hook block are the ones `AGENTS.md` lists.
- The bot App exists, is installed on this repository, and its credentials are stored; rerunning the setup here verifies rather than creates, which is every step's rerun shape anyway.
