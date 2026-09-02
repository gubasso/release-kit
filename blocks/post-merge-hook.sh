#!/bin/sh
# release-kit branch reminder
# Installed by rk setup step branch-reminder; rerunning that step rewrites it.
# After a merge arrives, report the branches and worktrees the forge already
# merged and retired. Prints nothing when there are none, never blocks a pull.
if rk branches prune --help >/dev/null 2>&1; then
  rk branches prune --quiet || :
fi
if rk worktree prune --help >/dev/null 2>&1; then
  rk worktree prune --quiet || :
fi
exit 0
