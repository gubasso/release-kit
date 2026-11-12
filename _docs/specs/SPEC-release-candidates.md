# Release Candidates Specification

## Purpose

Rules governing how `rk lines mint` prepares a release candidate on a release line and promotes it: which version it writes, which tip it acts on, and what it leaves to the pipeline that builds and publishes the candidate. A candidate is a prerelease version, `v<version>-rc.<n>`, minted on a line for a human to validate before the stable release. The line itself, its opening and its retirement, belongs to `SPEC-maintenance.md`. How a landed workflow obtains `rk` belongs to `SPEC-packaging.md`, and what a landing writes belongs to `SPEC-landing.md`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The decision behind these rules is `../decisions/ADR-publish-a-candidate-as-a-registry-prerelease.md`.

## Requirements

### `release-candidates:a-mint-creates-no-tag` — A mint creates no tag

`rk lines mint` MUST write the candidate's version into the target's version file, commit it, and push the line, and MUST NOT create a tag, because each pair's release half owns its tag, and a tag `rk` pushed first would make that half skip the publish.

#### Scenario: A candidate is minted

- GIVEN a line `release/1.4` whose remote holds `v1.4.2` and `v1.4.3-rc.2`
- WHEN `rk lines mint 1.4 --expect <tip> --apply` runs
- THEN the line carries one new commit with `1.4.3-rc.3` in its version file, and neither the clone nor the remote holds `v1.4.3-rc.3`

Verify: `cargo nextest run -E 'binary(cli) & test(a_mint_takes_the_next_rc_from_the_remote_tags)'`

### `release-candidates:a-promotion-writes-only-tested-code` — A promotion writes only tested code

`rk lines mint --promote` MUST write the stable version of the line's newest tagged candidate, and MUST refuse, naming each commit, while the line holds a commit after that candidate's tag that changes more than the package's own version, in its version file and in its lockfile entry with the references qualified by that version, because the stable release must be the code a human validated as the candidate, and the release request that follows proposes whatever the line holds.

#### Scenario: A fix landed after the candidate

- GIVEN `v1.4.3-rc.2` tagged at commit A, and a cherry-picked fix B on the line after A
- WHEN `rk lines mint 1.4 --promote --expect B --apply` runs
- THEN it refuses with `state-drift`, names B, writes nothing, and the operator mints `1.4.3-rc.3` instead

#### Scenario: The promotion runs again

- GIVEN a line promoted to `1.4.3` from `v1.4.3-rc.2`, with no other commit after the candidate
- WHEN `rk lines mint 1.4 --promote --expect <tip> --apply` runs again
- THEN it reports `pending` for `1.4.3` and writes nothing

Verify: `cargo nextest run -E 'binary(cli) & (test(promotion) | test(a_python_promote_commits_the_lock))'`

### `release-candidates:a-mint-acts-only-on-the-tip-its-caller-saw` — A mint acts only on the tip its caller saw

`rk lines mint --apply` MUST require `--expect`, and MUST refuse before any write when the line's remote tip, the checkout's `HEAD`, or the working tree differs from that tip, because two operators who dispatch at once would otherwise mint over a line neither of them validated.

#### Scenario: The line moved after the operator read it

- GIVEN an operator who read `release/1.4` at commit A, and a push that moved it to commit B
- WHEN `rk lines mint 1.4 --expect A --apply` runs
- THEN it refuses with `state-drift`, names B, and leaves the version file and the remote unchanged

Verify: `cargo nextest run -E 'binary(cli) & (test(a_mint_refuses_a_moved_tip_before_writing) | test(a_mint_refuses_a_wrong_checkout))'`

### `release-candidates:a-lost-race-names-the-winning-tip` — A lost race names the winning tip

If the remote rejects the mint's push because the line moved after the check, then `rk lines mint` MUST refuse with `remote-conflict` and name the tip that won, because the push is the lock, and the loser needs the new tip to decide whether to mint again.

#### Scenario: Another push lands between the check and the push

- GIVEN a mint that passed its check at commit A
- WHEN another push moves the line to commit B before the mint pushes
- THEN the mint refuses, names B, and the remote line stays at B

Verify: `cargo nextest run -E 'binary(cli) & test(a_lost_mint_race_names_the_winning_tip)'`

### `release-candidates:a-line-derives-its-candidate` — A line derives its candidate

`rk lines mint` MUST derive the candidate from the remote tags of the line alone: the stable base is the newest stable `v<major>.<minor>.<patch>` tag plus one patch, or `<major>.<minor>.0` while the line has released nothing, and the number is the highest candidate of that base plus one, because a line only ships its own patches and a clone holds only the tags it fetched. It MUST write the candidate through the driver adapter, so a python cut runs `uv version` and its commit carries `pyproject.toml` and `uv.lock`.

#### Scenario: A clone that never fetched the candidates

- GIVEN a remote with `v1.4.2`, `v1.4.3-rc.1`, and `v1.4.3-rc.2`, and a clone holding only `v1.4.2`
- WHEN `rk lines mint 1.4` previews
- THEN it reports `1.4.3-rc.3`

Verify: `cargo nextest run -E 'binary(cli) & (test(a_mint_takes_its_stable_base_as_settled) | test(a_mint_writes_each_technology_spelling) | test(a_python_cut_commits_the_lock))'`

### `release-candidates:a-pending-candidate-is-published-not-reminted` — A pending candidate is published, not reminted

When the version file already carries a candidate of the derived base whose tag does not exist, `rk lines mint` MUST write nothing, report the candidate as `pending`, and exit 0, because the release half has not published that number yet, and the caller's release step publishes it.

#### Scenario: The publisher failed after the mint pushed

- GIVEN a line whose version file carries `1.4.3-rc.1`, with no `v1.4.3-rc.1` tag
- WHEN `rk lines mint 1.4 --expect <tip> --apply` runs
- THEN it reports `pending` for `1.4.3-rc.1`, and the line gains no commit

Verify: `cargo nextest run -E 'binary(cli) & test(a_mint_takes_its_stable_base_as_settled)'`
