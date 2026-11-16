---
upstream: the release-kit issue tracker, where the case will be filed
affects: the trunk ruleset that the protect-trunk step of rk setup writes on GitHub under integration = "local"
state: investigating
filing: deferred
workaround: none. The operator ordered on 2026-11-16 to record the case and fix it later. Each trunk push of a local integration goes through the administrator bypass, and GitHub reports it as a bypassed rule violation.
---

# The trunk ruleset under local integration makes every trunk push a rule violation

## Symptom

A project records `integration = "local"` in `.release-kit/config.toml`. Its operator pushes a local integration to the trunk with `git push origin master`. The push succeeds, and GitHub prints this in the push output:

```text
remote: Bypassed rule violations for refs/heads/master:
remote:
remote: - Changes must be made through a pull request.
remote:
remote: - 2 of 2 required status checks are expected.
```

The same project passes `rk setup check`. The `protect-trunk` step reports `ok protect-trunk — master-protection holds the release-merge shape beside master-safety`, and all eight steps pass. So the setup reports the shape as correct, and the forge reports each push as a violation of that shape.

The case shows in vitals, claude-session, and exobrain-tech, which all record `integration = "local"`. It also shows on every trunk push of release-kit itself.

## How it works

1. `setup/github/protect-trunk` writes two rulesets on the trunk. The safety ruleset, `master-safety`, holds deletion and force-push protection and names no bypass actor. The trunk ruleset, `master-protection`, holds the `pull_request` rule. Where a forge gate reports a check, it also holds `required_status_checks`.
2. Under local integration, the trunk ruleset names one bypass actor: `RepositoryRole` 5, the repository administrator, with `bypass_mode` `always`. The script comment gives the reason: the deliberate trunk push of the operator goes through, and the release App, which holds no repository role, stays under the rules.
3. `rk integrate <branch> --apply` writes the squash onto the local trunk after the `pre-merge-commit` hook passes. The checks of the project run there, and the pre-push hooks run again at the push.
4. `git push origin master` sends that squash as a direct push. GitHub judges it against `master-protection`. The push did not come through a pull request, and no status check reported on the commit before the push. So the push breaks both rules.
5. The pusher holds the administrator role, so the bypass lets the push through. GitHub records the bypass and prints "Bypassed rule violations" on each push.
6. `rk setup check` judges the ruleset against the release-merge shape, which is the shape for a forge merge. That shape holds, so the check passes. The check does not judge whether the shape fits the recorded integration mode.

The cause: under local integration, the trunk ruleset keeps the rules of a forge merge, and the normal trunk push gets through them only by the administrator bypass. A triager first reads the message as a push that skipped the checks. That reading is wrong, because the local hooks ran every check before the push. The forge only sees no pull request and no reported status.

The smallest fix is not yet chosen. These questions stay open for the later fix:

- Must rk shape the trunk ruleset differently under local integration? One option is a ruleset with no `pull_request` rule. Another is a bypass that GitHub does not report as a violation.
- What does the release-merge shape still need under local integration? The release request of the release App still merges through a pull request, so the rules that govern that merge must stay.

## Signal

The text `Bypassed rule violations for refs/heads/master` in the output of a trunk push, in a project that records `integration = "local"`, while `rk setup check` reports `ok protect-trunk`.

## Workaround

None. The tree carries no change for this case. Each trunk push of a local integration goes through the administrator bypass until the later fix.
