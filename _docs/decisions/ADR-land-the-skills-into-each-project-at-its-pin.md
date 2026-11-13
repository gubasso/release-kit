# Land the skills into each project at its pin

## Context and Problem Statement

rk installed `rk-setup` and `rk-release` once under the home, while each project pins its own rk. On 2026-11-13 the projects of one host pinned rk from 0.9.1 to 0.9.15, and one host copy of the skills served all of them. `rk doctor` in one project failed `skill-sources` on 26 files, and that stop blocked a batch of adoptions. Nothing owned the host copy.

## Considered Options

- `Project scope in the landing` — chosen.
- `User scope, as before` — rejected: one copy cannot match several pins.
- `One home-manager copy at a host pin` — rejected: it moves the copy to a host pin, and the projects still pin other releases.

## Decision Outcome

Chosen option: `project scope in the landing`. `rk init` and `rk upgrade` land every file of every skill package under `.claude/skills/<name>/` and `.agents/skills/<name>/`, and the project commits them. The files are plain copies of the package and carry no version stamp. The version lives once, as `rk_version` in `.release-kit/manifest.json`, which also lists each skill file with its digest. A skill folder that the record does not attribute is never replaced. No rk verb writes a skill under the home.

The earlier objection, two entries under one name, goes away with the home copy. Claude Code resolves "personal over project", so a home copy left behind wins, and `rk doctor` names it. `prisma skills sync` and `gh skill install` also default to project scope.

Enforced by `distribution:a-skill-has-one-owner`, `distribution:a-landing-carries-the-skills`, `distribution:an-unattributed-skill-folder-is-never-replaced`, `distribution:a-skill-is-readable-before-a-landing`, and `distribution:the-doctor-names-a-shadowing-user-copy`.

## Consequences

- Good: each project reads the skills of the rk that it pins, and a pin move brings them through `rk upgrade`.
- Good: the skills reach a fresh clone, a CI job, and a cloud session.
- Bad: every project commits 38 more files, 19 package files under each of two roots.
- Bad: the host copies of earlier releases must go once, by `rk skill uninstall --apply`, or they shadow the project copy in Claude Code.

## Status

Accepted
