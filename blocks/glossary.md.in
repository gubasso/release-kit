<!-- BEGIN release-kit -->

## Workflow terms

A term below names the actions the operator authorizes by using it in a request. A request that carries no term authorizes the file changes alone.

| Term                    | What the operator authorizes by using it                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `implement-and-request` | The file changes, then the branch and its worktree, the commits, the push, and the pull or merge request. It stops before the merge.             |
| `implement-and-merge`   | Everything `implement-and-request` names, then the merge of that request, then pruning the branch and its worktree. It stops before the release. |
| `full-implement`        | Everything `implement-and-merge` names, then the release, operated through `rk method operate` to a published version.                           |

Write this project's own terms below the end marker. Release-kit owns the lines between the markers and rewrites them at every upgrade.

<!-- END release-kit -->
