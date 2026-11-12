# Tool Requirements Specification

## Purpose

Rules governing the tools the `rk` binary runs: which tools a verb checks, when it checks them, and what it says when one cannot serve it. A tool here is a dependency of the `rk` application, which the host supplies, and `rk` keeps that list short: it sends its own web requests and signs its own App token, so neither curl nor OpenSSL is on it. A project's own hook manager is not one: `rk integrate` asks git to run the clone's `pre-merge-commit` hook under `git:the-pre-merge-commit-hook-is-the-pre-integrate-contract`, and git runs whatever the project installed. The reports of `rk doctor` and `rk self-depend status` read a tool without stopping on it. `rk release status` stops on a missing tool like any verb, and reads `unknown` only for a credential the runtime lacks or a forge or registry that does not answer, under `release-state:a-forge-field-reads-unknown-without-credentials`. No adopting project adopts this spec: a project cannot violate a rule about how `rk` behaves and cannot run the verification. The sources behind the floors are `../reference/REFERENCE-tool-requirements-sources.md`.

## Requirements

### `tool-requirements:a-verb-checks-its-tools-first` — A verb checks its tools first

A verb MUST check every tool that `rk` runs for it before its first read of a forge and its first write, and MUST stop with one refusal that names each missing tool and each tool below the floor this binary calls, with the target unchanged, because a verb that fails halfway on a missing tool leaves work no report accounts for. The verb, its flags, and the target's record MUST decide which tools it checks, and the presence of a tool on the host MUST NOT decide it. The check MUST judge the program the verb spawns, from the directory the verb spawns it in, so a relative override or `PATH` entry resolves the same way for both. The floors MUST be the releases that added the calls this binary makes: git 2.36.0 for `git hook run`, and gh 2.57.0 for `gh auth status --active`.

#### Scenario: A host lacks git

- GIVEN a landed target and a host on which git does not run
- WHEN any verb that runs git acts on the target
- THEN it exits with the prerequisite code, names git, and the target's refs, index, and files are unchanged

#### Scenario: Two tools cannot serve one verb

- GIVEN a target whose record releases a rust crate, and a host with neither git nor cargo
- WHEN `rk integrate <branch> --apply` runs
- THEN one refusal names both tools, and the trunk stands where it stood

#### Scenario: A tool is below its floor

- GIVEN a git that reports 2.35.0
- WHEN `rk integrate <branch> --apply` runs
- THEN it refuses before the gate, naming the version found and the 2.36.0 it calls

#### Scenario: The record decides a tool

- GIVEN a python target and a bash target, on a host without uv
- WHEN `rk release request` runs on each
- THEN the python target's request refuses naming uv, and the bash target's request names no uv

Verify: `cargo nextest run -E 'binary(cli) & (test(each_git_verb_checks_git_before_it_acts) | test(a_verb_names_every_missing_tool_at_once) | test(a_git_below_its_floor_stops_the_verb) | test(a_forge_cli_below_its_floor_stops_the_verb) | test(a_condition_comes_from_the_record) | test(rk_runs_with_no_curl_or_openssl_on_path) | test(the_gh_auth_probe_runs_active_alone))'`

### `tool-requirements:rk-sends-its-own-requests` — rk sends its own requests

`rk` MUST send every HTTP request in-process, through one client that links no system OpenSSL and needs no system library, and MUST sign the GitHub App token in-process, so no child process holds the token or the key. The client MUST trust the platform's certificate store, plus the first bundle file that `SSL_CERT_FILE`, `NIX_SSL_CERT_FILE`, or `CURL_CA_BUNDLE` names, plus each directory that `SSL_CERT_DIR` names. It MUST choose the proxy for each request and each redirect from `https_proxy` or `HTTPS_PROXY`, `http_proxy` or `HTTP_PROXY`, and `all_proxy` or `ALL_PROXY`, the lowercase name first. It MUST skip the proxy where `no_proxy`, else `NO_PROXY`, names the host, a domain above it, its address, or an address range that holds it. Each call MUST keep the redirect policy and the timeout of the curl call it replaced, and MUST name `rk` in its user agent. `RK_HTTP_ORIGIN`, the one test seam, MUST accept only an `http://` origin on `127.0.0.1`, `::1`, or `localhost`, MUST refuse any other value by name, and MUST send every request and every redirect to that origin, through no proxy.

#### Scenario: A container trusts its own CA bundle

- GIVEN a server whose certificate a CA outside the platform store signs, and `SSL_CERT_FILE` naming a bundle that holds that CA
- WHEN `rk` sends a request to it
- THEN the request is answered, and without the variable it fails the TLS check

Verify: `cargo nextest run -E 'test(/http::tests::/) | test(the_app_jwt_verifies_with_the_public_key) | test(rk_runs_with_no_curl_or_openssl_on_path) | test(the_app_token_reaches_no_child_process)'`

### `tool-requirements:the-doctor-reports-the-hook-runner` — The doctor reports the hook runner

In a repository, `rk doctor` MUST report whether git finds a `pre-merge-commit` hook, in the hooks directory git names or, on a git that runs them, in git's configuration, and for each hook manager whose header an installed `pre-commit`, `commit-msg`, `pre-push`, or `pre-merge-commit` hook carries, whether that manager runs, naming the hooks that call it; a hook no known manager wrote MUST read as the project's own. Each MUST be a soft probe, because the project chooses its hook manager and the host supplies it, and no probe MUST change what a verb runs.

#### Scenario: A clone whose hooks prek wrote

- GIVEN a clone with prek's `pre-commit` and `pre-merge-commit` hooks, and prek on `PATH`
- WHEN `rk doctor` runs in it
- THEN it reports the gate hook installed and prek running, naming both hooks, and exits 0

Verify: `cargo nextest run -E 'test(the_doctor_reports_a_missing_pre_merge_commit_hook) | test(the_doctor_reports_prek_for_prek_hooks) | test(the_doctor_reports_pre_commit_for_pre_commit_hooks) | test(the_doctor_reports_a_custom_hook_as_the_projects_own) | test(the_doctor_reports_a_missing_runner_and_exits_zero) | test(the_doctor_reads_a_configured_hook_and_the_hooks_path)'`

### `tool-requirements:a-nested-hook-configuration-is-reported` — A nested hook configuration is reported

`rk status` MUST report each tracked `.pre-commit-config.yaml` below the target's root whose every directory prek's walk enters, naming `.prekignore` as the repair, because prek runs such a file as a project of its own. A directory is left out where the first rule that names it ignores it, from the `.prekignore` files of the root and the directories above it, then their `.ignore` files, then git's ignore rules with the global excludes. A hidden directory is left out unless a rule keeps it, and a Cookiecutter template directory always. The report MUST be a warning, so `rk status --check` does not fail on it, because the file is the project's.

#### Scenario: A test fixture carries a hook configuration

- GIVEN a landed target with a tracked `fixtures/x/.pre-commit-config.yaml` and no `.prekignore`
- WHEN `rk status` runs
- THEN it warns about that path and names `.prekignore`, and `rk status --check` does not fail on it

Verify: `cargo nextest run -E 'test(status_reports_a_nested_hook_configuration) | test(status_omits_a_nested_configuration_that_prekignore_excludes) | test(status_check_passes_with_a_nested_configuration) | test(the_nested_search_walks_directories_as_prek_does) | test(a_relative_target_reads_its_own_prekignore) | test(the_nested_search_skips_templates_and_hidden_directories_as_prek_does)'`
