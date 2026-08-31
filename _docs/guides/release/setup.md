# Setup

The one-time bootstrap of this repository onto its own convention. Every step shows the command by hand first, then the `rk` step that runs the same thing, then what success prints. The reasoning lives in `rk method setup`; this page carries what a person types against `gubasso/release-kit`.

## Facts

| Fact              | Value                                               |
| ----------------- | --------------------------------------------------- |
| Project           | `gubasso/release-kit`                               |
| Crate             | `release-kit`                                       |
| Trunk             | `master`, the only permanent branch and the default |
| Required check    | `test`                                              |
| Version truth     | `Cargo.toml`                                        |
| Publish workflow  | `release-plz.yml`                                   |
| Artifact workflow | `release.yml`                                       |
| Bot secrets       | `RELEASE_BOT_APP_ID`, `RELEASE_BOT_APP_PRIVATE_KEY` |

## 1. Gate the package metadata

Before anything that needs a credential or cannot be undone: the common registry rejects surface here with no token.

```bash
cargo publish --dry-run --locked
# check: exits 0 after "warning: aborting upload due to dry run"
```

Automated: `rk setup step package-check`.

## 2. Make master the trunk and the default

The bot's release request targets the default branch with no configuration, so the trunk must be it. This repository already has `master`; a repository without one gets it created at the current default's tip first.

```bash
gh repo edit gubasso/release-kit --default-branch master
gh repo view gubasso/release-kit --json defaultBranchRef -q .defaultBranchRef.name
# check: prints "master"
```

Automated: `rk setup step default-branch --apply`.

## 3. Retire every other long-lived branch

Exactly one permanent branch remains. For each candidate — `main`, and the retired integration branch — delete it only when it is an ancestor of the trunk; anything else holds work and refuses.

```bash
for candidate in main develop; do
  gh api "repos/gubasso/release-kit/git/ref/heads/$candidate" >/dev/null 2>&1 || continue
  status="$(gh api "repos/gubasso/release-kit/compare/$candidate...master" -q .status)"
  case "$status" in
    ahead | identical) gh api -X DELETE "repos/gubasso/release-kit/git/refs/heads/$candidate" ;;
    *) echo "$candidate is not an ancestor of master ($status); merge or move its work first" ;;
  esac
done
# check: gh api repos/gubasso/release-kit/git/ref/heads/<candidate> prints 404 for each
```

Automated: `rk setup step single-trunk --apply`. It is the one destructive step, and its guard fails closed.

## 4. Let Actions write and open pull requests

So the workflow's bot half can push branches and open the release request.

```bash
gh api -X PUT repos/gubasso/release-kit/actions/permissions/workflow \
  -f default_workflow_permissions=write -F can_approve_pull_request_reviews=true
gh api repos/gubasso/release-kit/actions/permissions/workflow
# check: prints "default_workflow_permissions": "write" and "can_approve_pull_request_reviews": true
```

Automated: `rk setup step ci-permissions --apply`.

## 5. Create the bot App

No command: creating a GitHub App is a web flow. Follow the walkthrough in `rk forge github`; what this repository needs from it is the App id and its private key.

## 6. Grant the App this repository

The installation endpoints refuse gh's own OAuth token, so this step needs a classic personal access token — or one click at github.com/settings/installations.

Store the token once in the OS keyring, then let each run read it back: the value reaches `rk` as an environment assignment, never a process argument, per `forge-setup:a-secret-never-reaches-argv`. Run this step on the host, never in a container — the keyring lookup needs a session bus a container does not have.

```bash
secret-tool store --label='GitHub classic PAT (repo scope)' service github account gh-classic-pat
GH_TOKEN="$(secret-tool lookup service github account gh-classic-pat)" rk setup step install-bot --target . --apply
# check: the step reports the installation id covering gubasso/release-kit
```

## 7. Store the bot credentials

The values travel on stdin and land as repository secrets; rerunning with new values is the rotation path. The environment carries the key's path, never its contents, and `rk` refuses a `.pem` that is group-readable, is inside the repository, or is not PEM-encoded private-key material.

```bash
chmod 600 <path to the .pem>
export RK_BOT_APP_ID=<app id>
export RK_BOT_PRIVATE_KEY_FILE=<path to the .pem>
rk setup step bot-secrets --target . --apply
gh secret list --repo gubasso/release-kit
# check: lists RELEASE_BOT_APP_ID and RELEASE_BOT_APP_PRIVATE_KEY
```

## 8. Protect the trunk

One ruleset: no direct push, no force-push, a pull request carrying the passing `test` check as the only way in, and squash as the only merge method, so `master` stays linear. Nothing in the pipeline writes the branch, so it names no bypass actor.

```bash
gh api -X POST repos/gubasso/release-kit/rulesets --input - <<'JSON'
{
  "name": "master-protection",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": { "include": ["refs/heads/master"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": false,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": false,
        "require_extra_approval_for_unattributed_changes": false,
        "allowed_merge_methods": ["squash"]
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "do_not_enforce_on_create": true,
        "strict_required_status_checks_policy": false,
        "required_status_checks": [{ "context": "test" }]
      }
    }
  ]
}
JSON
# check: gh api repos/gubasso/release-kit/rulesets -q '.[].name' includes master-protection
```

Automated: `rk setup step protect-trunk --target . --apply --required-check test`.

## 9. Protect the release tags

`v*` can be created but never moved or deleted; the pattern already covers the rc tags a release line mints.

```bash
gh api -X POST repos/gubasso/release-kit/rulesets --input - <<'JSON'
{
  "name": "release-tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": {
    "ref_name": { "include": ["refs/tags/v*"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "update" }
  ]
}
JSON
# check: gh api repos/gubasso/release-kit/rulesets -q '.[].name' includes release-tags
```

Automated: `rk setup step protect-tags --target . --apply`.

## 10. Protect the release lines, only when one exists

Style B's one extra step, skipped by a full apply: while an older line is alive, `release/*` can be neither force-pushed nor deleted, and pushes stay allowed because a cherry-pick lands by push. This repository runs it only after its first backport line is cut.

```bash
rk setup step protect-release-lines --target . --apply
# check: gh api repos/gubasso/release-kit/rulesets -q '.[].name' includes release-lines
```

## 11. Prove the protections

Exactly the owned rulesets — two, or three with the optional line protection — each still carrying the shape a release merge needs.

```bash
rk setup check --target .
# check: every step reports satisfied, and protect-release-lines reports skipped while no line exists
```

## 12. Land the workflow files

`rk init` lands the payload — this repository adopted its own landing, so the files already sit here — and cargo-dist generates the artifact workflow from `dist-workspace.toml`. Never edit `release.yml` by hand: regenerate it.

```bash
rk status --check --target .
dist generate
# check: .github/workflows/release.yml exists and dist plan prints the artifact list
```

## 13. Publish the first version by hand

Trusted publishing attaches to an existing package, so the first version goes up with a token: scoped to publishing new crates, shortest expiry, created at crates.io/settings/tokens for this step.

```bash
cargo login
cargo publish --locked
# check: crates.io serves release-kit at its first version
```

## 14. Register the trusted publisher

No command: on the crate's Settings page at crates.io, add the trusted publisher as owner `gubasso`, repository `release-kit`, workflow filename `release-plz.yml`. The filename is the invariant: `release.yml` is never registered.

## 15. Revoke the bootstrap token

No command: revoke it in the crates.io token settings, so the package has exactly one publishing path.

## 16. Prove the automated path

Cut one release end to end through [release.md](./release.md). Its verify step passing — the registry serves the version, and the tag and `master` name the same commit — is the proof the next step depends on.

## 17. Require trusted publishing

No command: turn on "Require trusted publishing for all new versions" in the crate's settings, now that one OIDC release has proven the path. From here every token publish is rejected; the hand-publish escape in `rk method recovery` starts by turning this off.
