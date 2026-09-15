# 12 — Integration

Integration moves one implementation onto the trunk. A project records which authority performs that move, and the choice is independent of where the implementation is edited.

## The two axes

`git.checkout_mode` answers where an implementation is edited. `linked-worktree` gives every code-changing branch its own linked working tree and keeps the main checkout free of commits. `main-worktree` switches the original working tree to the branch. [Worktrees](./08-worktrees.md) owns that axis and its guards.

`git.integration` answers who moves the implementation onto the trunk. `local` has the checkout perform and record the integration. `forge` has the forge perform and record it through a pull request or a merge request.

The axes are orthogonal, and every pairing works. Parallel work still takes one linked working tree per writer whatever the integration authority, because Git refuses one branch in two working trees and release-kit never overrides that refusal.

`git.integration` is a recorded Git workflow parameter. The landing verbs resolve it from a flag, the committed configuration, a compatible record, and a compiled default in that order, write it to `.release-kit/manifest.json`, render it into the committed hook block and routing block, and report it through `rk status`. `rk status --check` judges it. Changing it is `rk upgrade --integration <mode> --apply`, whose committed diff reaches every clone through the trunk, never an ad hoc local toggle. The compiled default is `local`. A record written before this axis existed answers `forge`, because that is what such a target actually landed.

## The invariant split

An implementation reaches the trunk through a short-lived branch, as one validated squash commit, and the trunk stays continuously releasable. The recorded integration mode chooses the authority that performs that integration. A pull request is one integration mechanism and not the definition of trunk-based development. [The model](./00-model.md) carries the branch forms and [the invariants](./01-invariants.md) carries the two rules this chapter splits.

The release request always integrates at the forge. Its bot branch carries the computed version and the rewritten changelog, and merging it is the only event that authorizes the tag, the registry publish, the provenance, and the artifacts. Local implementation integration removes the forge from nothing in the release path.

The trunk therefore carries one ordinary commit per implementation in either mode. A later push may carry several of them at once, and the release bot reads each one's release intent separately, so a batch of local integrations produces the changelog a sequence of forge merges would have produced.

## The gate boundary

A project assigns every check to the earliest native `pre-commit` stage that can run it. The stage before an irreversible boundary carries every check that boundary needs, and the boundary moves with the integration mode.

| Stage        | Under `forge`         | Under `local`                         |
| ------------ | --------------------- | ------------------------------------- |
| `pre-commit` | cheap author checks   | cheap author checks                   |
| `pre-push`   | the full branch suite | the trunk re-validation, on the push  |
| `manual`     | an operator sweep     | the full suite, run by `rk integrate` |

One rule produces both columns: every check runs at the last moment before the work crosses a boundary it cannot be pulled back from. Under forge integration that boundary is the branch push, because the forge sees the branch from then on. Under local integration the branch never leaves the checkout, so the boundary is the integration itself, and `pre-push` fires on the trunk push instead.

`pre-commit` accepts a closed set of stage names and admits no custom one, so no pre-integrate stage exists. `manual` runs for nobody by default and runs when a command asks for it, which makes it the pre-integrate contract. `rk integrate` invokes it with one line:

```bash
pre-commit run --hook-stage manual --all-files
```

Release-kit reads no hook identifier, no revision, no language, and no test category. The project owns everything in `.pre-commit-config.yaml` outside the marked block, owns its stage assignment, and owns its judgment about which checks are too expensive for a desk and which belong to continuous integration alone. That judgment lives in the project's own agent instructions, and the `rk-setup` skill is what researches the current hooks for the project's technologies.

Continuous integration invokes the same stages the project declared, through the project's own devshell or task runner. Release-kit lands no CI workflow and judges no parity: a claim reverse-engineered from a forge workflow file produces refusals a project cannot act on, so the convention is prose and an agent holds it at authoring time. `rk status --check` judges the marked block it owns and nothing else.

## Local integration

Local integration is the default. A single-writer project integrates one implementation after another with no forge round trip, and pushes the trunk when it decides to.

Under this mode the branch is born and dies in the checkout. It reaches no forge, so it has no pull request, no remote tip, and no review artifact. That is the mode's whole saving and its whole cost.

`rk integrate <branch>` performs the transaction and is the only path that writes a local trunk commit. It resolves the branch and its seat, refuses a dirty seat and a branch off the grammar, refreshes the trunk, brings the branch onto that tip, runs the `manual` stage, re-observes the trunk, creates one squash commit, runs the trunk gate against the new tip, and records the evidence the prune verbs read. Every refusal leaves the trunk at the tip it started from.

The command never pushes. The trunk push is a separate operator action and takes the fast-forward form alone. Where another writer wins the push, the loser fetches, replays the implementation, runs the gates again, and retries. No local integration uses a force-push.

The local path cannot observe remote checks before the trunk push, because the push is what starts them. Whoever pushes watches the resulting run and the release request, and repairs a failure as a fresh implementation integrated the same way.

## Forge integration

Forge integration runs the same local gates, then pushes the branch and opens or updates the pull request or merge request. The forge holds the merge behind its required check and performs the squash. The request is the integration record and the place review happens.

A project with more than one writer and a review requirement uses forge integration. No local gate substitutes for a second person reading a diff, and a local default does not make that gap smaller.

A recorded default binds no single execution. `--forge` and `--local` select the other authority once, without changing the record. Work an issue already names defaults to forge integration, because the forge minted and owns that branch. An agent states which authority an execution uses and never replaces a request with a local merge in silence.

## Protection and enforcement

Under forge integration the trunk keeps every protection [setup](./02-setup.md) installs: no direct push, no force-push, a request carrying the named passing check, and squash as the only merge method.

Under local integration the forge keeps what still holds against a direct push: no deletion, no force-push, and a restriction naming who may write the trunk. It drops the request rule and the required-check rule, because no forge can require a check before the push that starts it. Every tag protection and every release-request protection is unchanged, in both modes.

The desk and the forge remain the two distances [setup](./02-setup.md) describes. What changes is how far the forge's refusal reaches. A hook is discipline and not a boundary: it dies to `--no-verify` in either mode. What catches a bypass is the integration transaction, the trunk gate, the post-push run, and the release request that refuses to go green on a broken trunk.

## The release boundary

Pushing the trunk starts ordinary continuous integration and refreshes the release request. A failing post-push check blocks that request, so nothing publishes. It erases no local integration, and the next integration repairs it.

The release request stays the publication gate in both modes, and [operate](./03-operate.md) owns the sequence from its merge onward.

## Where this connects

The branch forms are [the model](./00-model.md); the split this chapter performs is held in [the invariants](./01-invariants.md); the mode is chosen in [setup](./02-setup.md) and reaches a target through [landing](./13-landing.md); the seats it integrates from are [worktrees](./08-worktrees.md); the step-by-step form is [the integration runbook](../runbooks/integration.md), `rk guide integration`.
