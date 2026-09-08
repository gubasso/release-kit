# Issue branch sources

The upstream documentation and source behind `_docs/specs/SPEC-issue-branch.md`, `rk issue start`, `src/issue.rs`, and the issue rows in the forge documents. Each entry records what was verified and when.

## GitHub, on creating a branch linked to an issue

Verified 2026-09-08 against `https://docs.github.com/en/graphql/reference/input-objects#createlinkedbranchinput`. `CreateLinkedBranchInput` declares `issueId` and `oid` as required, and `name` as `String` rather than `String!`, described as "The name of the new branch. Defaults to issue number and title." `oid` is `GitObjectID!`, "The commit SHA to base the new branch on". Bearing: an omitted name is a request for the forge's own name, which is why the mint passes no `--name`, and the base commit needs no flag because the CLI resolves it from the default branch.

## GitHub, on reading the branches an issue links

Verified 2026-09-08 against `https://docs.github.com/en/graphql/reference/objects#issue` and `#linkedbranch`. `Issue.linkedBranches` is `LinkedBranchConnection!`, "Branches linked to this issue", and `LinkedBranch.ref` is "The branch's ref". Bearing: the read is the authority for the minted name, and the same read makes a second run idempotent instead of minting twice.

## GitHub, on the linked-branch connection

Verified 2026-09-08 against `https://docs.github.com/en/graphql/reference/objects#issue` and `https://docs.github.com/en/graphql/guides/using-pagination-in-the-graphql-api`. `Issue.linkedBranches` is a connection: it takes `first` and answers `pageInfo { hasNextPage }` beside its nodes, and reading past the first page needs cursor traversal. Bearing: the read asks for 100 and for `hasNextPage`. An issue linking more than that is reported as a state this verb will not guess at, rather than silently truncated, because a truncated answer is what would make a mint look authorized.

## GitHub CLI, on the host it calls

Verified 2026-09-08 against `https://cli.github.com/manual/gh_api`. `gh api --hostname` defaults to `github.com`, and `gh issue develop --repo` accepts `[HOST/]OWNER/REPO`. Bearing: this verb passes neither, so every GitHub call it makes goes to `github.com`. A clone whose origin is another host is refused by name before any call, rather than acted on at the wrong host.

## GitHub CLI, on `gh issue develop`

Verified 2026-09-08 against `https://cli.github.com/manual/gh_issue_develop` and the cli/cli release history. The command was introduced in gh 2.19.0 through cli/cli pull request 6254, which describes it as remotely generating a branch linked to the issue. A later release added `--worktree`. Bearing: 2.19.0 is the GitHub floor in `Forge::cli_floor`, because below it the command does not exist. The `--worktree` flag is not used, because rk owns the path derivation and GitLab has no equivalent.

## GitHub CLI, on `gh api` and GraphQL variables

Verified 2026-09-08 against `https://cli.github.com/manual/gh_api`. Every parameter other than `query` and `operationName` is read as a GraphQL variable. `-f` keeps a value a string, while `-F` reads a value typed, so an integer reaches the API as a JSON number. Bearing: `number` is declared `Int!`, so the read passes it with `-F`; `-f` would send the string form and the query would fail.

## GitLab, on the branch name template

Verified 2026-09-08 against `https://docs.gitlab.com/ee/user/project/repository/branches/` and `https://docs.gitlab.com/ee/api/projects.html`. A project carries a branch name template as the setting `issue_branch_template`, "Template for branch names created from issues", read and written through the Projects API and set in the project's repository settings. The documented default composes the issue id and the issue title, and the title is modified to use only characters acceptable in Git branch names. The supported variables are `%{id}`, `%{title}`, and `%{branch_creator}`. GitLab applies the template in the web UI. Bearing: the template is read from the project rather than assumed, and the API path is read the template, render it, then create the branch.

## GitLab, on rendering the name

Verified 2026-09-08 against `Issue#to_branch_name` in `app/models/issue.rb` of `https://gitlab.com/gitlab-org/gitlab`. A confidential issue yields `<iid>-confidential-issue` and no template applies. Otherwise the three parameters are prepared first: the id parameterized with the case preserved, the title parameterized, and the branch creator's username parameterized with the case preserved. With no template the present values of id and title are joined with a hyphen. With a template the placeholders are substituted. A name longer than 100 characters is cut to 100 and then loses its trailing partial segment, which the source writes as `sub(/-[^-]*\Z/, '')`. Bearing: `gitlab_branch_name` in `src/issue.rs` reproduces this order exactly, including the confidential case and the truncation.

## GitLab, on an unresolved placeholder

Verified 2026-09-08 against `Gitlab::StringPlaceholderReplacer` in `lib/gitlab/string_placeholder_replacer.rb` of `https://gitlab.com/gitlab-org/gitlab`. Where the replacement for a placeholder is nil, the placeholder is unchanged in the output text. Bearing: an unknown placeholder such as `%{author}` survives into the name rather than raising. rk keeps it, and the grammar check one step later refuses the name and names the template as the cause. Refusing earlier would diverge from GitLab invisibly.

## Rails, on `String#parameterize`

Verified 2026-09-08 against `https://api.rubyonrails.org/classes/String.html#method-i-parameterize` and `ActiveSupport::Inflector.parameterize`. The method transliterates to an ASCII approximation, replaces every run of characters outside `[A-Za-z0-9_-]` with the separator, squeezes repeated separators into one, drops a leading and a trailing separator, and downcases unless the case is preserved. The documented example renders `"^très|Jolie-- "` as `tres-jolie`. `ActiveSupport::Inflector.transliterate` replaces a character it has no approximation for with `?`, which the run replacement then turns into the separator. Bearing: `parameterize` in `src/issue.rs` follows this order, and a character outside its table takes the same `?` path, so the two agree wherever the tables agree. The `approximated` flag reports the one place they can differ.

## GitLab, on which branch names link to an issue

Verified 2026-09-08 against `Issue#related_branches` in `app/models/issue.rb` of `https://gitlab.com/gitlab-org/gitlab` and `https://docs.gitlab.com/user/project/repository/branches/`. GitLab associates a branch with an issue when the branch name begins with the issue's own iid followed by a hyphen. No other part of the name takes part in the match. Bearing: a project template such as `feat/%{id}-%{title}` renders a name the landed grammar admits and GitLab links to nothing, so `rk issue start` refuses it rather than create a branch and report a link that does not exist. The same prefix is what makes a second run find a branch minted under an older title.

## GitLab, on searching branches by prefix

Verified 2026-09-08 against `https://docs.gitlab.com/api/branches/`. `GET /projects/:id/repository/branches` takes `search`, and a term beginning with `^` matches from the start of the branch name. The answer is an array, and a list endpoint answers one page of twenty by default. `glab api --paginate`, verified 2026-09-08 against `glab api --help` on glab 1.114.0, requests every page and emits the arrays as one JSON array under its default `json` output. Bearing: every name this verb can use carries the issue's link prefix, so this one read is a superset of a read of the exact rendered name and replaces it. It finds the branch the current rendering would produce, it finds one an earlier title or template produced instead, and it finds every other branch the issue owns. An empty array is the only proof of absence, and the read asks for every page: a second page left unread would read the same way.

## GitHub CLI, on the base flag

Verified 2026-09-08 against `gh issue develop --help` on gh 2.99.0 and `https://cli.github.com/manual/gh_issue_develop`. `--base` is "Name of the remote branch you want to make your new branch from". GitLab's Branches API `ref` accepts a branch name or a commit SHA. Bearing: the two forges do not accept the same set of values, so `rk issue start --base` documents the remote branch as the form both take and names the SHA as GitLab's alone.

## GitLab CLI, on the host it calls

Verified 2026-09-08 against `glab api --help` on glab 1.114.0 and `https://docs.gitlab.com/cli/api/`. Where the working directory is a Git directory, `glab api` uses the GitLab authenticated host there; otherwise it uses gitlab.com. `--hostname` overrides both. `--hostname` names the GitLab instance, and `--ssh-hostname` is a separate setting for an instance whose SSH endpoint differs from it. Bearing: this verb passes `--hostname` for a host an issue URL named, because a URL's host is the instance and a clone with no remote would otherwise be acted on at gitlab.com. It passes nothing for a host read from a git remote, because a remote names a transport endpoint an instance may serve under another name, and the CLI's own resolution from the working directory already maps it.

## GitLab, on creating a branch

Verified 2026-09-08 against `https://docs.gitlab.com/ee/api/branches.html`. `POST /projects/:id/repository/branches` takes `branch`, the name of the branch, and `ref`, the branch name or commit SHA to create the branch from, and answers `201 Created`. It resolves no template. Bearing: the create call is why the reads exist. The absence a mint acts on comes from the prefix search above, not from a `404` on the exact name.

## GitLab CLI, on `glab mr create --related-issue`

Verified 2026-09-08 against the `glab` source at `https://gitlab.com/gitlab-org/cli`. With `--create-source-branch` the command composes the branch name in the client, from the issue iid and a lowercased title with non-alphanumeric characters replaced. It applies no project template, transliterates nothing, squeezes no separator runs, and truncates nothing. It also opens a merge request before the first commit exists. Bearing: the command stays in the forge document's table for hand work, and rk does not use it. The two facts are the reason `src/issue.rs` renders the name itself.

## GitLab, on the rendered name the web UI shows

Verified 2026-09-08 against the `can_create_branch` project controller action in `https://gitlab.com/gitlab-org/gitlab`, which answers with `suggested_branch_name`. It is a Rails controller route rather than an `/api/v4` endpoint. `glab api` reaches `/api/v4` alone, and the route carries no compatibility contract. Bearing: rk does not call it, and `_docs/decisions/ADR-render-gitlabs-template-rather-than-call-an-internal-route.md` records the cost that choice accepts.

## Live proof

Not run. The proof transcript this record is meant to carry — a real GitHub issue minted and adopted, and a GitLab project's template rendered across an ASCII title, a punctuation run, an accented title, a title over 100 characters, and a confidential issue — has not been executed against a live instance. Every rendering claim above rests on the source and the documentation alone. The transliteration table is the one place a live run can still disagree, and `rk issue start` reports that case in its own detail line.
