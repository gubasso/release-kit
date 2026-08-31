# Setup runbook

The steps of [setup](../method/02-setup.md) as commands, once per repository, in the chapter's order. Every step that can be a command is one; every action that cannot names why no command replaces it and how often it actually occurs. `rk guide setup` fills in the project path, forge, and technology where detection resolves them.

## 0. Gate the package metadata

Before anything that needs credentials or cannot be undone:

```bash
rk setup step package-check --target .
```

## 1. Make the trunk the sole long-lived branch

The full apply below covers this section and the next two in order.

```bash
rk setup --target .                                      # preview every step
rk setup --target . --apply --required-check <name>      # run them in order
rk setup check --target .                                # prove what was applied
```

On github:

`--required-check <name>` names the CI job the trunk's protection requires, and the protection refuses without it: a wrong or missing check name does not fail, it hangs the merge button with nothing saying why. `gh api repos/<repo>/commits/HEAD/check-runs` lists the names the project's own workflow reports.

On gitlab:

No check is named here: the forge requires the whole pipeline through one project setting, so `--required-check` is refused as a usage error rather than silently discarded.

## 2. Let automation act

`rk setup` grants CI its permissions, adds the project to the bot identity, and stores the bot credentials. One action stays manual, on one forge, once per account ever.

On github:

Creating the bot App itself needs a browser, because the manifest flow redirects through one; it happens once in an account's lifetime, never per project. `rk forge github` carries the walkthrough, which credentials to collect, and the warning about the private key that downloads exactly once. With the App created, export `RK_BOT_APP_ID` and `RK_BOT_PRIVATE_KEY_FILE` (the path to the `.pem`, kept at mode `600` outside the repository) and the steps are commands: `rk setup step install-bot --target .` adds this project to the installation, and `rk setup step bot-secrets --target . --apply` stores the credentials. `install-bot` needs `GH_TOKEN` holding a classic personal access token with `repo` scope for that one call — the installation endpoints refuse gh's own OAuth token — or one click at `github.com/settings/installations` grants the project by hand; `rk forge github` states the caveat.

On gitlab:

Nothing is manual: creating the project access token also creates its bot user, so the whole bootstrap is `rk setup step install-bot --target . --apply`, which stores the token as the masked CI variable in the same step. `rk forge gitlab` states the role and scopes the token carries and why.

## 3. Protect the trunk and the tags

The trunk protection, the tag protection, and — where a project keeps older lines — the release-line protection, plus the check that exactly those protections exist with those rules.

```bash
rk setup check --target .            # check: every step reports satisfied
```

Where the forge enforces less than a step claims, the check names the weaker guarantee rather than passing; tag protection on GitLab is the case this exists for, per `rk forge gitlab`.

## 4. Land the workflow files

```bash
rk init --tech <tech> --target .             # preview
rk init --tech <tech> --target . --apply     # write, then fill the reported sentinels
```

## 5. Publish the first version by hand

Trusted publishing attaches to an existing package, so the first version goes up with a bootstrap token. Creating the token is manual — registry web UI, no API, once per package: scope it to publishing new versions of exactly this package, shortest expiry. The publish itself is a command, because the registry CLI reads the token from the environment.

On rust:

```bash
cargo login                                  # paste the bootstrap token
cargo publish
cargo info <crate>                           # check: the registry serves the version
```

On python:

```bash
python -m build
python -m twine upload dist/*                # reads the token from the environment
```

On bash:

There is no registry; the first release is proven by the automated path in step 7, and nothing is published by hand.

## 6. Register the trusted publisher

Manual — registry web UI, no API, once per package. Register the owner, the repository, and the publish workflow's filename; the binding names which landed file that is, and the filename registered must be the one that stays true. Then revoke the bootstrap token from step 5 — registry web UI, once per package — so the package has exactly one publishing path.

## 7. Prove the automated path

Cut one release end to end:

```bash
rk guide release
```

Its verify step passing — the registry serves the new version, and the tag and the trunk name the same commit — is the proof the next step depends on.

## 8. Require trusted publishing

Manual — registry web UI, no API, once per package. Turn on the registry's enforcement, now that one release has proven the OIDC path. From here every token publish is rejected; the hand-publish escape in [recovery](../method/04-recovery.md) starts by turning this off.
