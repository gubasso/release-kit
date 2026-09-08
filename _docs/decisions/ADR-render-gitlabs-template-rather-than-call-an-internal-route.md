# Render GitLab's template rather than call an internal route

## Context and Problem Statement

GitLab does expose its own rendered branch name for an issue. The `can_create_branch` controller action answers with `suggested_branch_name`, the string the web UI's button offers. Calling it would remove every rendering question from this binary.

## Considered Options

- `Reproduce Issue#to_branch_name in rk` — chosen.
- `Call can_create_branch and take suggested_branch_name` — rejected for three reasons that compound. It is a Rails controller route, not an `/api/v4` endpoint, so `glab api` cannot reach it and rk would build its own authenticated call. It carries no compatibility contract, so an upgrade can move it with no notice. And its failure is silent: a route that changes shape still answers with something that parses, so the name is wrong rather than absent.
- `Ask the operator whenever a template is set` — rejected: it makes the ordinary case interactive and puts the guess back on the person this verb helps.

## Decision Outcome

`src/issue.rs` reproduces the algorithm the source states: the confidential case first, the three parameters prepared with Rails `parameterize`, the template substitution or the plain join, then the hundred-character cut that drops its trailing partial segment. An unresolved placeholder stays in the text, as `Gitlab::StringPlaceholderReplacer` leaves it, and the landed grammar refuses that name one step later.

Enforced by `issue-branch:a-customized-template-is-read-not-assumed` and `issue-branch:a-name-the-grammar-refuses-stops-before-any-write`.

## Consequences

- Good: every call goes through `glab api` and `/api/v4`, so the credentials and the compatibility promise are the ones the forge CLI already owns.
- Good: the rendering is unit-tested with no network, including the cases a live instance is slowest to reproduce.

- Bad: transliteration is the residual gap. Rails approximates against its table and rk against its own, over Latin-1 Supplement and Latin Extended-A, so a character one covers and the other does not renders differently.
- Bad: an upstream change to the algorithm is one rk follows by hand.

## Status

Implemented — `src/issue.rs`. The verb reports an approximated title in its own detail line, which is the honest bound on the gap.
