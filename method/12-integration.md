# 12 — Integration

Integration moves one implementation onto the trunk. A project records which authority performs that move, and the choice is independent of where the implementation is edited.

## The two axes

`git.checkout_mode` answers where an implementation is edited. `linked-worktree` gives every code-changing branch its own linked working tree and keeps the main checkout free of commits. `main-worktree` switches the original working tree to the branch. [Worktrees](./08-worktrees.md) owns that axis and its guards.

`git.integration` answers who moves the implementation onto the trunk. `local` has the checkout perform and record the integration. `forge` has the forge perform and record it through a pull request or a merge request.

The axes are orthogonal, and every pairing works. Parallel work still takes one linked working tree per writer whatever the integration authority, because Git refuses one branch in two working trees and release-kit never overrides that refusal.

`git.integration` is a recorded Git workflow parameter. The landing verbs resolve it from a flag, the committed configuration, a compatible record, and a compiled default in that order, write it to `.release-kit/manifest.json`, render it into the committed hook block and routing block, and report it through `rk status`. `rk status --check` judges it. Changing it is `rk upgrade --integration <mode> --apply`, whose committed diff reaches every clone through the trunk, never an ad hoc local toggle. The compiled default is `local`. A record written before this axis existed answers `forge`, because that is what such a target actually landed.

`rk integrate` reads the record alone, never the committed configuration. The record is what landed: the hook block that admits or refuses its writes, the routing block an agent reads, and the forge protections the setup installed all render from it. An edited configuration is pending input to the next landing, so taking it at execution time would integrate locally in a target whose installed controls still say forge.

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

Release-kit reads no hook identifier, no revision, no language, and no test category. The project owns everything in `.pre-commit-config.yaml` outside the marked block, owns its stage assignment, and owns the second axis below. That judgment lives in the project's own agent instructions, and the `rk-setup` skill is what researches the current hooks for the project's technologies.

## The scope axis

The stage says when a check runs. Scope says where it can run at all, and it comes from the same boundary rule: a check runs at every boundary it can meaningfully guard, and the two values that are not `both` name the two ways a check cannot.

| Scope        | What it means                                          | How a project expresses it                              |
| ------------ | ------------------------------------------------------ | ------------------------------------------------------- |
| `both`       | the check runs at a desk and in continuous integration | a hook at its native stage, with CI invoking that stage |
| `local-only` | the check is meaningless in continuous integration     | a hook whose id the CI sweep names in `SKIP`            |
| `ci-only`    | the check is meaningless at a desk                     | no hook; it names the remote boundary it guards and why |

Most checks are `both`, and that is the value with a rule attached: such a check must have a local execution path and a remote execution path of equivalent coverage, not identical commands. A desk that runs a suite through a devshell and a runner that runs it through a task runner satisfy it; a check that exists on one side alone and claims `both` does not.

Moment has two forms, because a `ci-only` check has no honest hook stage. A check with a local path declares its earliest native stage. A `ci-only` check declares the remote boundary it guards instead, ordinarily the pull request, beside the reason it cannot run at a desk.

Under local integration the `manual` stage is the strongest gate a project has, so a project whose continuous integration does not invoke that stage has a gate on one side of the boundary only. That is the failure this axis exists to make visible.

Continuous integration invokes the same stages the project declared, through the project's own devshell or task runner. Release-kit lands no CI workflow and judges no parity: a claim reverse-engineered from a forge workflow file produces refusals a project cannot act on, so the convention is prose and an agent holds it at authoring time. `rk status --check` judges the marked block it owns and nothing else.

## Local integration

Local integration is the default. A single-writer project integrates one implementation after another with no forge round trip, and pushes the trunk when it decides to.

Under this mode the branch is born and dies in the checkout. It reaches no forge, so it has no pull request, no remote tip, and no review artifact. That is the mode's whole saving and its whole cost.

`rk integrate <branch>` performs the transaction and is the only path that writes a local trunk commit. It resolves the branch and its seat, refuses a dirty seat and a branch off the grammar, refreshes the trunk, brings the branch onto that tip, and runs the `manual` stage. Then it re-observes the branch, builds the squash commit as an object no ref names, stages the evidence the prune verbs read, and publishes the commit with one compare-and-swap carrying the trunk tip it observed before the gate ran. Nothing is undone, because nothing is published early: a trunk that moved under the gate refuses with no ref touched, and evidence naming a commit the trunk does not reach proves nothing, so a refusal at the last step leaves inert residue rather than a false proof.

Two things the command holds that a forge would have held for it. The trunk message passes the whole landed `commit-msg` judgment here, because the command that writes the commit fires no hook, so a message with agent attribution or a reference to an ignored path refuses at the desk rather than reaching a permanent history. And the branch is observed before the gate and again after it: the tree that is integrated and the tip the evidence certifies are the one object the gate judged, so a commit landing in the seat mid-transaction refuses rather than riding in ungated or leaving evidence for work the trunk does not carry.

The command never pushes, and a preview refreshes nothing: it takes no lock, runs no fetch, and moves no ref. The trunk push is a separate operator action and takes the fast-forward form alone. Where another writer wins the push, the loser fetches, replays the implementation, runs the gates again, and retries. No local integration uses a force-push.

The local path cannot observe remote checks before the trunk push, because the push is what starts them. Whoever pushes watches the resulting run and the release request, and repairs a failure as a fresh implementation integrated the same way.

## Forge integration

Forge integration runs the project's own `pre-push` stage by pushing the branch, then names the request command for the forge the remote resolves to. Opening the request is the operator's act, and no verb here authors a request body: `rk message --check --kind body` is what judges one. The forge holds the merge behind its required check and performs the squash. The request is the integration record and the place review happens.

A project with more than one writer and a review requirement uses forge integration. No local gate substitutes for a second person reading a diff, and a local default does not make that gap smaller.

A recorded default binds no single execution. `--forge` and `--local` select the other authority once, without changing the record. Work an issue already names defaults to forge integration, because the forge minted and owns that branch. An agent states which authority an execution uses and never replaces a request with a local merge in silence.

## Protection and enforcement

Under forge integration the trunk keeps every protection [setup](./02-setup.md) installs: no direct push, no force-push, a request carrying the named passing check, and squash as the only merge method.

Under local integration GitHub keeps the full trunk ruleset, including the request rule and strict required check. It grants exactly the repository-administrator role an always bypass for the deliberate direct push. The release App is a distinct integration actor and receives no bypass, so its release request still cannot merge after the trunk advances. GitLab carries the local push authority through `protection.gitlab.push_access_level`. `rk setup` installs the shape the recorded mode names and its check expects that same shape, so a target that records `local` is neither locked out of its own integration nor stripped of atomic release freshness. Every tag protection is unchanged in both modes.

The local-mode release workflow retries the merge but is not the atomic protection. It wakes when the named workflow completes and after its push-side request job succeeds, discovers candidates across every result page, proves the request's identity against the forge, and attempts the merge only once the same recorded check has concluded successfully for the exact head. Any other answer is a successful no-op that leaves the request open. The ruleset performs the final base-freshness judgment at the merge boundary.

The desk and the forge remain the two distances [setup](./02-setup.md) describes. What changes is how far the forge's refusal reaches. A hook is discipline and not a boundary: it dies to `--no-verify` in either mode. What catches a bypass is the integration transaction, the trunk gate, the post-push run, and the release request that refuses to go green on a broken trunk.

## The release boundary

Pushing the trunk starts ordinary continuous integration and refreshes the release request. A failing post-push check leaves that request open, so nothing publishes; the retry job logs the non-success answer and exits successfully because the refusal is policy, not a broken workflow. It erases no local integration, and the next integration repairs it.

The release request stays the publication gate in both modes, and [operate](./03-operate.md) owns the sequence from its merge onward.

## Where this connects

The branch forms are [the model](./00-model.md); the split this chapter performs is held in [the invariants](./01-invariants.md); the mode is chosen in [setup](./02-setup.md) and reaches a target through [landing](./13-landing.md); the seats it integrates from are [worktrees](./08-worktrees.md); the step-by-step form is [the integration runbook](../runbooks/integration.md), `rk guide integration`.
