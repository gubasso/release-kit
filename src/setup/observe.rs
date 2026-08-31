//! The observe-and-verify half of every step's lifecycle.
//!
//! One implementation per forge and step, called by preview never, by apply
//! before and after the mutation, and by `check` as its whole job — so the
//! three modes cannot drift apart, and the mutating half is unreachable from
//! here by construction: nothing in this module spawns anything but
//! read-only forge-CLI calls and the technology's own dry-run check.

use serde_json::Value;

use crate::detect::Forge;
use crate::error::RkError;
use crate::setup::context::{Ctx, TRUNK_BRANCH};
use crate::setup::process::{Exec, Outcome};

/// The executor observes run through: the command layer wraps echoing,
/// journaling, and redaction around the process adapter.
pub type Runner<'a> = dyn FnMut(&Exec) -> Result<Outcome, RkError> + 'a;

/// The long-lived branch names `single-trunk` retires when each is an
/// ancestor of the trunk: the common default and the retired second branch.
pub const TRUNK_CANDIDATES: [&str; 2] = ["main", "develop"];

/// What one observation found.
#[derive(Debug)]
pub enum StepState {
    /// The desired state holds; a limitation names what the forge enforces
    /// less strongly than the step's proof claims.
    Satisfied {
        /// What was found, one line.
        detail: String,
        /// The weaker guarantee, by name, where the forge enforces less.
        limitation: Option<String>,
    },
    /// The desired state does not hold.
    Unsatisfied {
        /// What was found instead.
        detail: String,
    },
    /// An optional step's condition does not hold: nothing is wrong, and
    /// nothing is proven — `check` reports it as skipped, while an explicit
    /// single-step apply still runs it.
    Inapplicable {
        /// Why the step does not apply here.
        detail: String,
    },
    /// The observation could not decide.
    Unknown {
        /// Why not.
        detail: String,
    },
}

impl StepState {
    /// Whether the desired state holds.
    #[must_use]
    pub const fn satisfied(&self) -> bool {
        matches!(self, Self::Satisfied { .. })
    }

    fn ok(detail: impl Into<String>) -> Self {
        Self::Satisfied {
            detail: detail.into(),
            limitation: None,
        }
    }

    fn ok_with_limitation(detail: impl Into<String>, limitation: impl Into<String>) -> Self {
        Self::Satisfied {
            detail: detail.into(),
            limitation: Some(limitation.into()),
        }
    }

    fn not(detail: impl Into<String>) -> Self {
        Self::Unsatisfied {
            detail: detail.into(),
        }
    }

    fn inapplicable(detail: impl Into<String>) -> Self {
        Self::Inapplicable {
            detail: detail.into(),
        }
    }

    fn unknown(detail: impl Into<String>) -> Self {
        Self::Unknown {
            detail: detail.into(),
        }
    }
}

/// One read-only forge API answer.
enum Api {
    /// The call succeeded and parsed.
    Ok(Value),
    /// The forge answered 404: the thing is not there.
    Missing,
    /// The call failed for another reason, with the CLI's own words.
    Failed(String),
}

/// Observe one step's desired state.
///
/// # Errors
///
/// Propagates executor failures; a forge answer that merely disagrees is a
/// [`StepState`], not an error.
pub fn observe(ctx: &Ctx, step: &str, run: &mut Runner) -> Result<StepState, RkError> {
    if step == "package-check" {
        return package_check(ctx, run);
    }
    match ctx.forge {
        Forge::Github => github(ctx, step, run),
        Forge::Gitlab => gitlab(ctx, step, run),
    }
}

/// §0: the technology's own no-credential packaging check; the one step that
/// reads its command from the binding rather than from a forge tree.
fn package_check(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let (program, args): (&str, &[&str]) = match ctx.tech {
        Some("rust") => ("cargo", &["publish", "--dry-run", "--allow-dirty"]),
        Some("python") => ("python3", &["-m", "build"]),
        Some("bash") => {
            return Ok(StepState::ok(
                "no registry for this technology; there is nothing to package",
            ));
        }
        Some(other) => {
            return Ok(StepState::unknown(format!(
                "no packaging check is defined for {other}"
            )));
        }
        None => {
            return Ok(StepState::unknown(
                "no version file names a technology; see rk binding --list",
            ));
        }
    };
    let exec = Exec {
        program: program.into(),
        args: args.iter().map(Into::into).collect(),
        env: ctx.child_env("package-check"),
        cwd: ctx.target.as_std_path().to_path_buf(),
        stdin: None,
    };
    let outcome = run(&exec)?;
    Ok(if outcome.success() {
        StepState::ok("the package builds and passes the registry's dry run")
    } else {
        StepState::not(format!(
            "the packaging check failed: {}",
            last_line(&outcome.stderr)
        ))
    })
}

/// The destructive step's own guard: whether deleting a candidate branch
/// can lose work.
///
/// `Satisfied` means every candidate is already gone or is an ancestor of
/// the trunk; `Unsatisfied` means the deletion must refuse.
///
/// # Errors
///
/// Propagates executor failures.
pub fn single_trunk_guard(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    for candidate in TRUNK_CANDIDATES {
        if candidate == TRUNK_BRANCH {
            continue;
        }
        let state = match ctx.forge {
            Forge::Github => github_candidate_guard(ctx, run, candidate)?,
            Forge::Gitlab => gitlab_candidate_guard(ctx, run, candidate)?,
        };
        if !state.satisfied() {
            return Ok(state);
        }
    }
    Ok(StepState::ok(
        "every candidate branch is absent, or an ancestor of the trunk",
    ))
}

/// One candidate branch's ancestry, on GitHub.
fn github_candidate_guard(
    ctx: &Ctx,
    run: &mut Runner,
    candidate: &str,
) -> Result<StepState, RkError> {
    match api_get(
        ctx,
        run,
        &format!("repos/{}/git/ref/heads/{candidate}", ctx.repo),
    )? {
        Api::Missing => return Ok(StepState::ok(format!("{candidate} is already gone"))),
        Api::Failed(err) => return Ok(StepState::unknown(err)),
        Api::Ok(_) => {}
    }
    match api_get(
        ctx,
        run,
        &format!("repos/{}/compare/{candidate}...{TRUNK_BRANCH}", ctx.repo),
    )? {
        Api::Ok(body) => {
            let status = body["status"].as_str().unwrap_or("");
            Ok(if matches!(status, "ahead" | "identical") {
                StepState::ok(format!("{candidate} is an ancestor of {TRUNK_BRANCH}"))
            } else {
                StepState::not(format!(
                    "{candidate} is not an ancestor of {TRUNK_BRANCH} ({status}); deleting it would lose work"
                ))
            })
        }
        Api::Missing => Ok(StepState::unknown("the comparison is not readable")),
        Api::Failed(err) => Ok(StepState::unknown(err)),
    }
}

/// One candidate branch's ancestry, on GitLab.
fn gitlab_candidate_guard(
    ctx: &Ctx,
    run: &mut Runner,
    candidate: &str,
) -> Result<StepState, RkError> {
    let project = ctx.repo.replace('/', "%2F");
    match api_get(
        ctx,
        run,
        &format!("projects/{project}/repository/branches/{candidate}"),
    )? {
        Api::Missing => return Ok(StepState::ok(format!("{candidate} is already gone"))),
        Api::Failed(err) => return Ok(StepState::unknown(err)),
        Api::Ok(_) => {}
    }
    match api_get(
        ctx,
        run,
        &format!("projects/{project}/repository/compare?from={TRUNK_BRANCH}&to={candidate}"),
    )? {
        Api::Ok(body) => {
            let ahead = body["commits"]
                .as_array()
                .is_some_and(|list| !list.is_empty());
            Ok(if ahead {
                StepState::not(format!(
                    "{candidate} carries commits {TRUNK_BRANCH} does not; deleting it would lose work"
                ))
            } else {
                StepState::ok(format!("{candidate} is an ancestor of {TRUNK_BRANCH}"))
            })
        }
        Api::Missing => Ok(StepState::unknown("the comparison is not readable")),
        Api::Failed(err) => Ok(StepState::unknown(err)),
    }
}

/// One captured, read-only forge API call.
fn api_get(ctx: &Ctx, run: &mut Runner, path: &str) -> Result<Api, RkError> {
    let exec = Exec {
        program: ctx.cli.clone().into_os_string(),
        args: vec!["api".into(), path.into()],
        env: ctx.child_env("observe"),
        cwd: ctx.target.as_std_path().to_path_buf(),
        stdin: None,
    };
    let outcome = run(&exec)?;
    if outcome.success() {
        return Ok(
            serde_json::from_slice::<Value>(&outcome.stdout).map_or_else(
                |_| Api::Failed("the forge answer did not parse as JSON".into()),
                Api::Ok,
            ),
        );
    }
    let stderr = String::from_utf8_lossy(&outcome.stderr).into_owned();
    if stderr.contains("404") {
        Ok(Api::Missing)
    } else {
        Ok(Api::Failed(last_line(&outcome.stderr)))
    }
}

/// The last non-empty line of a byte stream, for one-line detail fields.
fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .to_owned()
}

#[allow(clippy::too_many_lines)]
fn github(ctx: &Ctx, step: &str, run: &mut Runner) -> Result<StepState, RkError> {
    let repo = &ctx.repo;
    match step {
        "default-branch" => Ok(match api_get(ctx, run, &format!("repos/{repo}"))? {
            Api::Ok(body) => {
                let found = body["default_branch"].as_str().unwrap_or("");
                if found == TRUNK_BRANCH {
                    StepState::ok(format!("{TRUNK_BRANCH} is the default branch"))
                } else {
                    StepState::not(format!("the default branch is {found}"))
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {repo}")),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "single-trunk" => {
            for candidate in TRUNK_CANDIDATES {
                if candidate == TRUNK_BRANCH {
                    continue;
                }
                match api_get(ctx, run, &format!("repos/{repo}/git/ref/heads/{candidate}"))? {
                    Api::Missing => {}
                    Api::Ok(_) => {
                        return Ok(StepState::not(format!("a {candidate} branch still exists")));
                    }
                    Api::Failed(err) => return Ok(StepState::unknown(err)),
                }
            }
            Ok(StepState::ok(
                "no long-lived branch besides the trunk remains",
            ))
        }
        "ci-permissions" => Ok(
            match api_get(
                ctx,
                run,
                &format!("repos/{repo}/actions/permissions/workflow"),
            )? {
                Api::Ok(body) => {
                    let write = body["default_workflow_permissions"] == "write";
                    let approve = body["can_approve_pull_request_reviews"] == true;
                    if write && approve {
                        StepState::ok("CI may write and open requests")
                    } else {
                        StepState::not(format!(
                            "workflow permissions are {} with request approval {}",
                            body["default_workflow_permissions"],
                            body["can_approve_pull_request_reviews"]
                        ))
                    }
                }
                Api::Missing => StepState::not("no workflow permissions are readable"),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "install-bot" => github_install_bot(ctx, run),
        "bot-secrets" => Ok(
            match api_get(ctx, run, &format!("repos/{repo}/actions/secrets"))? {
                Api::Ok(body) => {
                    let names: Vec<&str> = body["secrets"]
                        .as_array()
                        .map(|list| {
                            list.iter()
                                .filter_map(|secret| secret["name"].as_str())
                                .collect()
                        })
                        .unwrap_or_default();
                    let wanted = ["RELEASE_BOT_APP_ID", "RELEASE_BOT_APP_PRIVATE_KEY"];
                    if wanted.iter().all(|name| names.contains(name)) {
                        StepState::ok("both bot secrets are stored")
                    } else {
                        StepState::not(format!("stored secrets: {}", names.join(", ")))
                    }
                }
                Api::Missing => StepState::not("no secrets are readable"),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "protect-trunk" => github_trunk_ruleset(ctx, run),
        "protect-tags" => github_ruleset(ctx, run, "release-tags", &["deletion", "update"]),
        "protect-release-lines" => {
            if github_ruleset_body(ctx, run, "release-lines")?.is_none() {
                return Ok(StepState::inapplicable(
                    "release/* is unprotected; optional — applied only where older lines exist",
                ));
            }
            github_ruleset(ctx, run, "release-lines", &["deletion", "non_fast_forward"])
        }
        "protections-check" => {
            let mut failures = Vec::new();
            for owned in ["protect-trunk", "protect-tags", "protect-release-lines"] {
                match github(ctx, owned, run)? {
                    StepState::Satisfied { .. } | StepState::Inapplicable { .. } => {}
                    StepState::Unsatisfied { detail } | StepState::Unknown { detail } => {
                        failures.push(format!("{owned}: {detail}"));
                    }
                }
            }
            if let Api::Ok(body) = api_get(ctx, run, &format!("repos/{repo}/rulesets"))? {
                let owned = [
                    format!("{TRUNK_BRANCH}-protection"),
                    "release-tags".to_owned(),
                    "release-lines".to_owned(),
                ];
                for ruleset in body.as_array().into_iter().flatten() {
                    let name = ruleset["name"].as_str().unwrap_or("");
                    if !owned.iter().any(|expected| expected == name) {
                        failures.push(format!("a ruleset no step owns: {name}"));
                    }
                }
            }
            Ok(if failures.is_empty() {
                StepState::ok("exactly the owned protections, with those rules")
            } else {
                StepState::not(failures.join("; "))
            })
        }
        _ => Ok(StepState::unknown(format!("no observation for {step}"))),
    }
}

fn github_install_bot(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let installations = match api_get(ctx, run, "user/installations")? {
        Api::Ok(body) => body,
        Api::Missing => return Ok(StepState::not("no app installation is reachable")),
        // The installation endpoints refuse the OAuth token `gh auth
        // login` normally mints — the grant is documented for classic
        // personal access tokens only — so this 403 names the wrong
        // token class, not missing authentication.
        Api::Failed(err) if err.contains("authorized to a GitHub App") => {
            return Ok(StepState::unknown(format!(
                "{err}; the forge refuses gh's own OAuth token here — authenticate gh with a classic personal access token carrying repo scope, or grant the project at github.com/settings/installations"
            )));
        }
        Api::Failed(err) => return Ok(StepState::unknown(err)),
    };
    let ids: Vec<i64> = installations["installations"]
        .as_array()
        .map(|list| list.iter().filter_map(|i| i["id"].as_i64()).collect())
        .unwrap_or_default();
    if ids.is_empty() {
        return Ok(StepState::not("no app installation is reachable"));
    }
    for id in ids {
        let path = format!("user/installations/{id}/repositories?per_page=100");
        if let Api::Ok(body) = api_get(ctx, run, &path)? {
            let granted = body["repositories"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|repository| repository["full_name"] == ctx.repo.as_str());
            if granted {
                return Ok(StepState::ok(format!(
                    "installation {id} covers {}",
                    ctx.repo
                )));
            }
        }
    }
    Ok(StepState::not(format!(
        "no reachable installation covers {}",
        ctx.repo
    )))
}

/// A plain ruleset: active, and carrying exactly the expected rule types —
/// not one fewer, and not one more, because an extra rule here is a rule the
/// setup cannot reproduce or explain and can block the very push the method
/// depends on.
fn github_ruleset(
    ctx: &Ctx,
    run: &mut Runner,
    name: &str,
    rules: &[&str],
) -> Result<StepState, RkError> {
    let Some(detail) = github_ruleset_body(ctx, run, name)? else {
        return Ok(StepState::not(format!("no ruleset named {name}")));
    };
    if detail["enforcement"] != "active" {
        return Ok(StepState::not(format!("{name} is not active")));
    }
    let mut held: Vec<&str> = detail["rules"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|rule| rule["type"].as_str())
                .collect()
        })
        .unwrap_or_default();
    held.sort_unstable();
    let mut expected: Vec<&str> = rules.to_vec();
    expected.sort_unstable();
    if held == expected {
        Ok(StepState::ok(format!(
            "{name} is active with exactly its rules"
        )))
    } else {
        Ok(StepState::not(format!(
            "{name} carries the rules [{}] where the setup owns [{}]",
            held.join(", "),
            expected.join(", ")
        )))
    }
}

/// The trunk ruleset, checked for the shape a release merge needs.
fn github_trunk_ruleset(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let name = format!("{TRUNK_BRANCH}-protection");
    let Some(detail) = github_ruleset_body(ctx, run, &name)? else {
        return Ok(StepState::not(format!("no ruleset named {name}")));
    };
    let rules = detail["rules"].as_array().cloned().unwrap_or_default();
    let has = |kind: &str| rules.iter().any(|rule| rule["type"] == kind);
    let mut faults = Vec::new();
    if detail["enforcement"] != "active" {
        faults.push(format!("{name} is not active"));
    }
    if !detail["bypass_actors"].as_array().is_none_or(Vec::is_empty) {
        faults.push("a bypass actor is named".to_owned());
    }
    let owned = [
        "deletion",
        "non_fast_forward",
        "pull_request",
        "required_status_checks",
    ];
    for required in owned {
        if !has(required) {
            faults.push(format!("the {required} rule is missing"));
        }
    }
    for rule in &rules {
        if let Some(kind) = rule["type"].as_str() {
            if !owned.contains(&kind) {
                // Named for the sharpest case: an unowned rule is one the
                // setup cannot reproduce or explain, and it can block the
                // very merge the method depends on.
                faults.push(format!("an unowned rule is present: {kind}"));
            }
        }
    }
    if let Some(request) = rules.iter().find(|rule| rule["type"] == "pull_request") {
        if request["parameters"]["allowed_merge_methods"] != serde_json::json!(["squash"]) {
            faults.push("the merge method is not exactly a squash merge".to_owned());
        }
    }
    if let Some(checks) = rules
        .iter()
        .find(|rule| rule["type"] == "required_status_checks")
    {
        let contexts: Vec<&str> = checks["parameters"]["required_status_checks"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|check| check["context"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        // Where the expected check is known, the context set must be exactly
        // it: an extra stale context does not fail a merge, it hangs one.
        if contexts.is_empty() {
            faults.push("no status check is required".to_owned());
        } else if let Some(expected) = &ctx.required_check {
            if contexts != [expected.as_str()] {
                faults.push(format!(
                    "the required checks are [{}] where the setup owns [{expected}]",
                    contexts.join(", ")
                ));
            }
        }
    }
    Ok(if faults.is_empty() {
        StepState::ok(format!("{name} holds the release-merge shape"))
    } else {
        StepState::not(faults.join("; "))
    })
}

/// A ruleset's detail body by name, or `None` when no ruleset carries it.
fn github_ruleset_body(ctx: &Ctx, run: &mut Runner, name: &str) -> Result<Option<Value>, RkError> {
    let list = match api_get(ctx, run, &format!("repos/{}/rulesets", ctx.repo))? {
        Api::Ok(body) => body,
        Api::Missing | Api::Failed(_) => return Ok(None),
    };
    let id = list
        .as_array()
        .into_iter()
        .flatten()
        .find(|ruleset| ruleset["name"] == name)
        .and_then(|ruleset| ruleset["id"].as_i64());
    let Some(id) = id else { return Ok(None) };
    match api_get(ctx, run, &format!("repos/{}/rulesets/{id}", ctx.repo))? {
        Api::Ok(body) => Ok(Some(body)),
        Api::Missing | Api::Failed(_) => Ok(None),
    }
}

/// The GitLab limitation `protect-tags` and `protections-check` report.
const GITLAB_TAG_LIMITATION: &str =
    "an Owner or Maintainer can still delete a protected tag through the UI or API";

#[allow(clippy::too_many_lines)]
fn gitlab(ctx: &Ctx, step: &str, run: &mut Runner) -> Result<StepState, RkError> {
    let project = ctx.repo.replace('/', "%2F");
    match step {
        "default-branch" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                let found = body["default_branch"].as_str().unwrap_or("");
                if found == TRUNK_BRANCH {
                    StepState::ok(format!("{TRUNK_BRANCH} is the default branch"))
                } else {
                    StepState::not(format!("the default branch is {found}"))
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "single-trunk" => {
            for candidate in TRUNK_CANDIDATES {
                if candidate == TRUNK_BRANCH {
                    continue;
                }
                match api_get(
                    ctx,
                    run,
                    &format!("projects/{project}/repository/branches/{candidate}"),
                )? {
                    Api::Missing => {}
                    Api::Ok(_) => {
                        return Ok(StepState::not(format!("a {candidate} branch still exists")));
                    }
                    Api::Failed(err) => return Ok(StepState::unknown(err)),
                }
            }
            Ok(StepState::ok(
                "no long-lived branch besides the trunk remains",
            ))
        }
        "ci-permissions" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                if body["jobs_enabled"] == true {
                    StepState::ok("pipelines are enabled")
                } else {
                    StepState::not("pipelines are disabled")
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "install-bot" => {
            // The listing paginates, exactly as the script's does: an
            // active token past the first page must not read as absent, or
            // verification would contradict the apply it verifies. Absence
            // is only reported once a short page proves the listing was
            // exhausted; a bound reached on a full page is an unknown.
            let mut active = false;
            let mut exhausted = false;
            for page in 1..=10u32 {
                let path = format!(
                    "projects/{project}/access_tokens?state=active&per_page=100&page={page}"
                );
                let list = match api_get(ctx, run, &path)? {
                    Api::Ok(body) => body.as_array().cloned().unwrap_or_default(),
                    Api::Missing => Vec::new(),
                    Api::Failed(err) => return Ok(StepState::unknown(err)),
                };
                active = active
                    || list.iter().any(|token| {
                        token["name"] == "release-bot"
                            && token["revoked"] == false
                            && token["active"] != false
                    });
                if list.len() < 100 {
                    exhausted = true;
                }
                if active || exhausted {
                    break;
                }
            }
            if !active {
                return Ok(if exhausted {
                    StepState::not("no active release-bot token exists")
                } else {
                    StepState::unknown(
                        "the token listing did not exhaust within ten pages; nothing was decided",
                    )
                });
            }
            // A token whose stored variable has gone missing is a stranded
            // identity — its value is unrecoverable — so the step is only
            // satisfied when both halves hold, and a rerun rotates.
            Ok(
                match api_get(
                    ctx,
                    run,
                    &format!("projects/{project}/variables/RELEASE_BOT_TOKEN"),
                )? {
                    Api::Ok(_) => StepState::ok(
                        "an active release-bot token exists and its variable is stored",
                    ),
                    Api::Missing => StepState::not(
                        "an active release-bot token exists with no stored variable; a rerun revokes and replaces it",
                    ),
                    Api::Failed(err) => StepState::unknown(err),
                },
            )
        }
        "bot-secrets" => Ok(
            match api_get(
                ctx,
                run,
                &format!("projects/{project}/variables/RELEASE_BOT_TOKEN"),
            )? {
                Api::Ok(_) => StepState::ok("RELEASE_BOT_TOKEN is stored"),
                Api::Missing => StepState::not("RELEASE_BOT_TOKEN is not stored"),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "protect-trunk" => {
            let protection = match api_get(
                ctx,
                run,
                &format!("projects/{project}/protected_branches/{TRUNK_BRANCH}"),
            )? {
                Api::Ok(body) => body,
                Api::Missing => {
                    return Ok(StepState::not(format!("{TRUNK_BRANCH} is not protected")));
                }
                Api::Failed(err) => return Ok(StepState::unknown(err)),
            };
            // Exactly one push grant, and it is the no-access entry: the
            // forge honors the most permissive grant, so a second entry
            // beside access level 0 is a branch that still takes a push.
            let grants = protection["push_access_levels"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let no_push = grants.len() == 1 && grants[0]["access_level"] == 0;
            let settings = match api_get(ctx, run, &format!("projects/{project}"))? {
                Api::Ok(body) => body,
                Api::Missing | Api::Failed(_) => Value::Null,
            };
            let mut faults = Vec::new();
            if !no_push {
                faults.push(format!(
                    "{TRUNK_BRANCH} still takes a direct push: the forge honors the most permissive of {} push grants",
                    grants.len()
                ));
            }
            if protection["allow_force_push"] != false {
                faults.push(format!("{TRUNK_BRANCH} allows force pushes"));
            }
            if settings["only_allow_merge_if_pipeline_succeeds"] != true {
                faults.push("the pipeline requirement is off".to_owned());
            }
            if settings["merge_method"] != "ff" {
                faults.push("the merge method is not fast-forward".to_owned());
            }
            if settings["squash_option"] != "always" {
                faults.push("merge requests do not always squash".to_owned());
            }
            Ok(if faults.is_empty() {
                StepState::ok(format!("{TRUNK_BRANCH} holds the release-merge shape"))
            } else {
                StepState::not(faults.join("; "))
            })
        }
        "protect-tags" => Ok(
            match api_get(ctx, run, &format!("projects/{project}/protected_tags/v%2A"))? {
                Api::Ok(_) => {
                    StepState::ok_with_limitation("v* is protected", GITLAB_TAG_LIMITATION)
                }
                Api::Missing => StepState::not("v* is not protected"),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "protect-release-lines" => Ok(
            match api_get(
                ctx,
                run,
                &format!("projects/{project}/protected_branches/release%2F%2A"),
            )? {
                Api::Ok(body) => {
                    if body["allow_force_push"] == false {
                        StepState::ok("release/* refuses force pushes and deletion by git clients")
                    } else {
                        StepState::not("release/* allows force pushes")
                    }
                }
                Api::Missing => StepState::inapplicable(
                    "release/* is unprotected; optional — applied only where older lines exist",
                ),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "protections-check" => {
            let mut failures = Vec::new();
            let mut limitation = None;
            for owned in ["protect-trunk", "protect-tags", "protect-release-lines"] {
                match gitlab(ctx, owned, run)? {
                    StepState::Satisfied {
                        limitation: found, ..
                    } => limitation = limitation.or(found),
                    StepState::Inapplicable { .. } => {}
                    StepState::Unsatisfied { detail } | StepState::Unknown { detail } => {
                        failures.push(format!("{owned}: {detail}"));
                    }
                }
            }
            Ok(if failures.is_empty() {
                StepState::Satisfied {
                    detail: "the protections hold, as far as this forge enforces them".into(),
                    limitation,
                }
            } else {
                StepState::not(failures.join("; "))
            })
        }
        _ => Ok(StepState::unknown(format!("no observation for {step}"))),
    }
}
