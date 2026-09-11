//! The observe-and-verify half of every step's lifecycle.
//!
//! One implementation per forge and step, called by preview never, by apply
//! before and after the mutation, and by `check` as its whole job — so the
//! three modes cannot drift apart, and the mutating half is unreachable from
//! here by construction: nothing spawned from this module mutates anything —
//! read-only forge-CLI calls, the technology's own dry-run check, and the
//! App-credential read [`super::app_jwt`] carries for `install-bot`.

use serde_json::Value;

use crate::detect::Forge;
use crate::error::RkError;
use crate::setup::app_jwt::{self, AppApi};
use crate::setup::context::Ctx;
use crate::setup::process::{Exec, Outcome};
use crate::setup::workflow_jobs;

/// The executor observes run through: the command layer wraps echoing,
/// journaling, and redaction around the process adapter.
pub type Runner<'a> = dyn FnMut(&Exec) -> Result<Outcome, RkError> + 'a;

// The long-lived branch names `single-trunk` retires when each is an
// ancestor of the trunk come from the target's own configuration, read
// through `Ctx::retired_branches`. The compiled default is the common
// default branch and the retired second branch, so a target that names
// none behaves exactly as it did.

// The landed title check's context is the job in `pr-title.yml` that
// holds the squash title to the commit convention. The target names it in
// `protection.title_check`, read through `Ctx::title_check`; the landed
// job keeps its own name as a payload constant, so a target that renames
// the key without renaming the job breaks its own trunk protection and
// this observer reports it.

const GITLAB_PRIVATE_REPORTING_LIMITATION: &str = "GitLab has no project-level private reporting switch; the reporter must enable confidentiality; this proves project feature access, not successful submission by every external reporter";

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
    /// Eligibility or an optional step's condition does not hold: nothing
    /// is proven, and `check` reports it as skipped.
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
    if step == "branch-reminder" {
        return Ok(branch_reminder_state(ctx));
    }
    if step == "forge-version" {
        return forge_version(ctx, run);
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

/// §1: the post-merge reminder hook, judged from the target's own files;
/// the one step whose observation asks no forge and spawns no CLI.
fn branch_reminder_state(ctx: &Ctx) -> StepState {
    use crate::setup::branch_reminder::{HookState, observe_hook};
    match observe_hook(&ctx.target) {
        HookState::Installed => {
            StepState::ok("the post-merge hook carries the release-kit reminder")
        }
        HookState::Absent => StepState::not("no post-merge hook is installed"),
        HookState::Foreign => {
            StepState::not("a post-merge hook exists without the release-kit marker")
        }
        HookState::Drifted => StepState::not("the reminder hook drifted from this binary's body"),
        HookState::Unreadable(detail) => StepState::unknown(detail),
    }
}

/// The GitLab version this convention needs, as major and minor.
///
/// `trigger: strategy: mirror` arrived in GitLab 18.2, and the merge-request
/// pipeline's `project-jobs` bridge rests on it: below the floor the child
/// pipeline's status never reaches the parent, so a failing project job
/// merges.
pub const GITLAB_VERSION_FLOOR: (u64, u64) = (18, 2);

/// The two suffixes that name an edition rather than a pre-release. Every
/// other suffix is a pre-release, and the step fails closed on one.
const GITLAB_EDITIONS: [&str; 2] = ["ee", "ce"];

/// The refusal an instance below the floor reads: the reading, the reason,
/// and the fix.
fn version_refusal(found: &str, prerelease: Option<&str>) -> String {
    let (major, minor) = GITLAB_VERSION_FLOOR;
    let mut said = vec![format!(
        "this GitLab instance reports {found}; the convention needs {major}.{minor} or newer"
    )];
    if let Some(suffix) = prerelease {
        said.push(format!(
            "the -{suffix} suffix is a pre-release, and nothing proves the feature shipped in it, so this step fails closed"
        ));
    }
    said.push(format!(
        "the merge-request pipeline triggers a child pipeline with `strategy: mirror`, which GitLab added in {major}.{minor}"
    ));
    said.push(
        "below it the child's status never reaches the parent pipeline, so a failing project job merges".to_owned(),
    );
    said.push(format!(
        "upgrade the instance to {major}.{minor} or newer, or host the project on gitlab.com"
    ));
    said.join("; ")
}

/// §3: the forge's own version against the convention's floor.
///
/// GitHub is a rolling service and is answered without a call. GitLab is one
/// read-only `GET /version`, and every failure to read is `Unknown`, which
/// blocks the `protect-trunk` prerequisite exactly as `Unsatisfied` does.
fn forge_version(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    if ctx.forge == Forge::Github {
        return Ok(StepState::ok(
            "github.com is a rolling service and declares no version floor",
        ));
    }
    let body = match api_get(ctx, run, "version")? {
        Api::Ok(body) => body,
        Api::Missing => {
            return Ok(StepState::unknown(
                "this instance answers no GET /version; the floor cannot be read. Check that glab is authenticated against it: glab auth login",
            ));
        }
        Api::Failed(err) => {
            return Ok(StepState::unknown(format!(
                "the version could not be read: {err}. Check that glab is authenticated against this instance: glab auth login"
            )));
        }
    };
    let Some(found) = body["version"].as_str() else {
        return Ok(StepState::unknown(
            "the forge answer carries no version field; the floor cannot be read. Check that glab is authenticated against this instance: glab auth login",
        ));
    };
    let (number, suffix) = found
        .split_once('-')
        .map_or((found, None), |(n, s)| (n, Some(s)));
    let mut parts = number.split('.');
    let parsed = parts
        .next()
        .and_then(|major| major.parse::<u64>().ok())
        .zip(parts.next().and_then(|minor| minor.parse::<u64>().ok()));
    let Some(pair) = parsed else {
        return Ok(StepState::unknown(format!(
            "the forge reports the version as '{found}', which names no major and minor pair; the floor cannot be read"
        )));
    };
    if let Some(suffix) = suffix.filter(|s| !GITLAB_EDITIONS.contains(s)) {
        return Ok(StepState::not(version_refusal(found, Some(suffix))));
    }
    if pair < GITLAB_VERSION_FLOOR {
        return Ok(StepState::not(version_refusal(found, None)));
    }
    let (major, minor) = GITLAB_VERSION_FLOOR;
    Ok(StepState::ok(format!(
        "this instance reports {found}, at or above the {major}.{minor} floor"
    )))
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
    let trunk = ctx.trunk();
    for candidate in ctx.retired_branches() {
        let candidate = candidate.as_str();
        if candidate == trunk {
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
    let trunk = ctx.trunk();
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
        &format!("repos/{}/compare/{candidate}...{trunk}", ctx.repo),
    )? {
        Api::Ok(body) => {
            let status = body["status"].as_str().unwrap_or("");
            Ok(if matches!(status, "ahead" | "identical") {
                StepState::ok(format!("{candidate} is an ancestor of {trunk}"))
            } else {
                StepState::not(format!(
                    "{candidate} is not an ancestor of {trunk} ({status}); deleting it would lose work"
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
    let trunk = ctx.trunk();
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
        &format!("projects/{project}/repository/compare?from={trunk}&to={candidate}"),
    )? {
        Api::Ok(body) => {
            let ahead = body["commits"]
                .as_array()
                .is_some_and(|list| !list.is_empty());
            Ok(if ahead {
                StepState::not(format!(
                    "{candidate} carries commits {trunk} does not; deleting it would lose work"
                ))
            } else {
                StepState::ok(format!("{candidate} is an ancestor of {trunk}"))
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
    let trunk = ctx.trunk();
    let repo = &ctx.repo;
    match step {
        "private-vulnerability-reporting" => {
            let visibility_path = format!("repos/{repo}");
            match api_get(ctx, run, &visibility_path)? {
                Api::Ok(body) => match body["private"].as_bool() {
                    Some(true) => {
                        return Ok(StepState::inapplicable(
                            "private vulnerability reporting is available for public repositories",
                        ));
                    }
                    Some(false) => {}
                    None => {
                        return Ok(StepState::unknown(format!(
                            "{visibility_path}: repository visibility is unreadable"
                        )));
                    }
                },
                Api::Missing => {
                    return Ok(StepState::unknown(format!(
                        "{visibility_path}: repository visibility is unreadable (404)"
                    )));
                }
                Api::Failed(err) => {
                    return Ok(StepState::unknown(format!("{visibility_path}: {err}")));
                }
            }
            let path = format!("repos/{repo}/private-vulnerability-reporting");
            Ok(match api_get(ctx, run, &path)? {
                Api::Ok(body) => match body["enabled"].as_bool() {
                    Some(true) => StepState::ok("private vulnerability reporting is enabled"),
                    Some(false) => StepState::not("private vulnerability reporting is disabled"),
                    None => StepState::unknown(format!("{path}: enabled is unreadable")),
                },
                Api::Missing => {
                    StepState::unknown(format!("{path}: reporting state is unreadable (404)"))
                }
                Api::Failed(err) => StepState::unknown(format!("{path}: {err}")),
            })
        }

        "default-branch" => Ok(match api_get(ctx, run, &format!("repos/{repo}"))? {
            Api::Ok(body) => {
                let found = body["default_branch"].as_str().unwrap_or("");
                if found == trunk {
                    StepState::ok(format!("{trunk} is the default branch"))
                } else {
                    StepState::not(format!("the default branch is {found}"))
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {repo}")),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "single-trunk" => {
            for candidate in ctx.retired_branches() {
                let candidate = candidate.as_str();
                if candidate == trunk {
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
        "merge-cleanup" => Ok(match api_get(ctx, run, &format!("repos/{repo}"))? {
            Api::Ok(body) => {
                if body["delete_branch_on_merge"].as_bool().unwrap_or(false) {
                    StepState::ok("a merged branch is deleted by the forge")
                } else {
                    StepState::not("a merged branch outlives its merge")
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {repo}")),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "auto-merge" => Ok(match api_get(ctx, run, &format!("repos/{repo}"))? {
            Api::Ok(body) => {
                if body["allow_auto_merge"].as_bool().unwrap_or(false) {
                    StepState::ok("a request may merge itself once its checks pass")
                } else {
                    StepState::not("a request cannot merge itself; the auto-merge switch is off")
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {repo}")),
            Api::Failed(err) => StepState::unknown(err),
        }),
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
                    } else if names.is_empty() {
                        StepState::not("no bot secrets are stored")
                    } else {
                        StepState::not(format!("stored secrets: {}", names.join(", ")))
                    }
                }
                Api::Missing => StepState::not("no secrets are readable"),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "protect-trunk" => github_trunk_ruleset(ctx, run),
        "protect-tags" => github_ruleset(
            ctx,
            run,
            ctx.tag_ruleset(),
            "tag",
            "refs/tags/v*",
            &["deletion", "update"],
        ),
        "protect-release-lines" => {
            match github_ruleset_body(ctx, run, ctx.lines_ruleset())? {
                RulesetLookup::Absent => {
                    return Ok(StepState::inapplicable(
                        "release/* is unprotected; optional — applied only where older lines exist",
                    ));
                }
                RulesetLookup::Unreadable(err) => return Ok(StepState::unknown(err)),
                RulesetLookup::Found(_) => {}
            }
            github_ruleset(
                ctx,
                run,
                ctx.lines_ruleset(),
                "branch",
                "refs/heads/release/*",
                &["deletion", "non_fast_forward"],
            )
        }
        "protections-check" => {
            // Confirmed drift and unreadable answers stay apart: a proven
            // mismatch is drift even beside an outage, and an outage with
            // nothing proven wrong stays unknown, never drift.
            let mut failures = Vec::new();
            let mut unknowns = Vec::new();
            // Every satisfied step's limitation survives the aggregate.
            let mut limitations: Vec<String> = Vec::new();
            for owned in ["protect-trunk", "protect-tags", "protect-release-lines"] {
                match github(ctx, owned, run)? {
                    StepState::Satisfied {
                        limitation: found, ..
                    } => limitations.extend(found),
                    StepState::Inapplicable { .. } => {}
                    StepState::Unsatisfied { detail } => {
                        failures.push(format!("{owned}: {detail}"));
                    }
                    StepState::Unknown { detail } => {
                        unknowns.push(format!("{owned}: {detail}"));
                    }
                }
            }
            match api_get(ctx, run, &format!("repos/{repo}/rulesets"))? {
                Api::Ok(body) => {
                    let owned = [
                        ctx.trunk_ruleset().to_owned(),
                        ctx.tag_ruleset().to_owned(),
                        ctx.lines_ruleset().to_owned(),
                    ];
                    for ruleset in body.as_array().into_iter().flatten() {
                        let name = ruleset["name"].as_str().unwrap_or("");
                        if !owned.iter().any(|expected| expected == name) {
                            failures.push(format!("a ruleset no step owns: {name}"));
                        }
                    }
                }
                Api::Missing | Api::Failed(_) => {
                    unknowns.push("the ruleset inventory is not readable".to_owned());
                }
            }
            Ok(if !failures.is_empty() {
                StepState::not(failures.join("; "))
            } else if !unknowns.is_empty() {
                StepState::unknown(unknowns.join("; "))
            } else {
                StepState::Satisfied {
                    detail: "exactly the owned protections, with those rules".into(),
                    limitation: if limitations.is_empty() {
                        None
                    } else {
                        Some(limitations.join("; "))
                    },
                }
            })
        }
        _ => Ok(StepState::unknown(format!("no observation for {step}"))),
    }
}

/// The installation, observed as the App itself.
///
/// The forge serves `repos/{owner}/{repo}/installation` to an App JWT and
/// to nothing a user can hold. The caller mints `jwt` — once per run, with
/// the token and the key bytes already registered as redaction needles —
/// which is why this lives outside the name dispatch above: an observation
/// entered without that token has no honest answer.
#[must_use]
pub fn github_install_bot(ctx: &Ctx, jwt: &str) -> StepState {
    match app_jwt::api_get(ctx, jwt, &format!("repos/{}/installation", ctx.repo)) {
        AppApi::Ok(body) => {
            let id = body["id"].as_i64().unwrap_or_default();
            StepState::ok(format!("installation {id} covers {}", ctx.repo))
        }
        AppApi::Missing => StepState::not(format!("the App is not installed on {}", ctx.repo)),
        AppApi::Refused(detail) | AppApi::Failed(detail) => StepState::unknown(detail),
    }
}

/// A plain ruleset: active, and carrying exactly the expected rule types —
/// not one fewer, and not one more, because an extra rule here is a rule the
/// setup cannot reproduce or explain and can block the very push the method
/// depends on.
fn github_ruleset(
    ctx: &Ctx,
    run: &mut Runner,
    name: &str,
    target: &str,
    include: &str,
    rules: &[&str],
) -> Result<StepState, RkError> {
    let detail = match github_ruleset_body(ctx, run, name)? {
        RulesetLookup::Found(detail) => detail,
        RulesetLookup::Absent => {
            return Ok(StepState::not(format!("no ruleset named {name}")));
        }
        RulesetLookup::Unreadable(err) => return Ok(StepState::unknown(err)),
    };
    if detail["enforcement"] != "active" {
        return Ok(StepState::not(format!("{name} is not active")));
    }
    // The name proves nothing: the ruleset must cover exactly the declared
    // refs, or the protection it reports exists somewhere else.
    if detail["target"] != target {
        return Ok(StepState::not(format!(
            "{name} does not target {target} refs"
        )));
    }
    if detail["conditions"]["ref_name"]["include"] != serde_json::json!([include]) {
        return Ok(StepState::not(format!(
            "{name} does not cover {include} alone"
        )));
    }
    if detail["conditions"]["ref_name"]["exclude"] != serde_json::json!([]) {
        return Ok(StepState::not(format!(
            "{name} excludes refs from its own coverage"
        )));
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

// The trunk ruleset is checked for the shape a release merge needs.
// The rule kinds the setup writes and can reproduce come from
// `protection.owned_trunk_rules`, floored to contain all four. The set
// also drives the missing-rule fault, so a kind this convention refuses
// must stay out of it: adding one would demand that rule on every target.
// The floor is what stops a target dropping one it needs.

/// A fault line for every rule on the trunk that the setup does not own.
///
/// The merge queue gets its own text, because this convention refuses one
/// deliberately and the operator needs the consequence and the remedy. Every
/// other unowned kind reads generically: an unowned rule is one the setup
/// cannot reproduce or explain, and it can block the very merge the method
/// depends on.
fn unowned_rule_faults(rules: &[Value], owned: &[String]) -> Vec<String> {
    rules
        .iter()
        .filter_map(|rule| rule["type"].as_str())
        .filter(|kind| !owned.iter().any(|name| name == kind))
        .map(|kind| {
            if kind == "merge_queue" {
                MERGE_QUEUE_FAULT.to_owned()
            } else {
                format!("an unowned rule is present: {kind}")
            }
        })
        .collect()
}

fn github_trunk_ruleset(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    let name = ctx.trunk_ruleset().to_owned();
    let detail = match github_ruleset_body(ctx, run, &name)? {
        RulesetLookup::Found(detail) => detail,
        RulesetLookup::Absent => {
            return Ok(StepState::not(format!("no ruleset named {name}")));
        }
        RulesetLookup::Unreadable(err) => return Ok(StepState::unknown(err)),
    };
    let rules = detail["rules"].as_array().cloned().unwrap_or_default();
    let has = |kind: &str| rules.iter().any(|rule| rule["type"] == kind);
    let mut faults = Vec::new();
    if detail["enforcement"] != "active" {
        faults.push(format!("{name} is not active"));
    }
    // The name proves nothing: a ruleset applies only where its conditions
    // say, so a right-named ruleset covering another ref would otherwise
    // read as a protected trunk.
    if detail["target"] != "branch" {
        faults.push(format!("{name} does not target branches"));
    }
    let expected_ref = serde_json::json!([format!("refs/heads/{trunk}")]);
    if detail["conditions"]["ref_name"]["include"] != expected_ref {
        faults.push(format!("{name} does not cover refs/heads/{trunk} alone"));
    }
    // A matching exclusion negates the include, so the owned shape is an
    // exclusion list that is exactly empty.
    if detail["conditions"]["ref_name"]["exclude"] != serde_json::json!([]) {
        faults.push(format!("{name} excludes refs from its own coverage"));
    }
    if !detail["bypass_actors"].as_array().is_none_or(Vec::is_empty) {
        faults.push("a bypass actor is named".to_owned());
    }
    for required in &ctx.protection().owned_trunk_rules {
        if !has(required) {
            faults.push(format!("the {required} rule is missing"));
        }
    }
    faults.extend(unowned_rule_faults(
        &rules,
        &ctx.protection().owned_trunk_rules,
    ));
    if let Some(request) = rules.iter().find(|rule| rule["type"] == "pull_request") {
        if request["parameters"]["allowed_merge_methods"]
            != serde_json::json!(ctx.protection().allowed_merge_methods)
        {
            faults.push("the merge method is not exactly a squash merge".to_owned());
        }
    }
    if let Some(checks) = rules
        .iter()
        .find(|rule| rule["type"] == "required_status_checks")
    {
        if checks["parameters"]["strict_required_status_checks_policy"]
            != ctx.protection().strict_required_status_checks
        {
            faults.push(STALE_MERGE_FAULT.to_owned());
        }
        let contexts: Vec<&str> = checks["parameters"]["required_status_checks"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|check| check["context"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        // Where the expected check is known, the context set must be exactly
        // it plus the title check: an extra stale context does not fail a
        // merge, it hangs one, and a missing title check lets an
        // unconventional squash title land on the trunk.
        if contexts.is_empty() {
            faults.push("no status check is required".to_owned());
        } else if let Some(expected) = &ctx.required_check {
            let mut held = contexts.clone();
            held.sort_unstable();
            let title_check = ctx.title_check();
            let mut owned_contexts = [expected.as_str(), title_check];
            owned_contexts.sort_unstable();
            if held != owned_contexts {
                faults.push(format!(
                    "the required checks are [{}] where the setup owns [{}]",
                    contexts.join(", "),
                    owned_contexts.join(", ")
                ));
            }
        } else if !contexts.contains(&ctx.title_check()) {
            faults.push(format!("the {} check is not required", ctx.title_check()));
        }
    }
    match squash_merge_sources(ctx, run)? {
        MergeSources::Owned => {}
        MergeSources::Faults(proven) => faults.extend(proven),
        // Proven drift wins over an outage: an unreadable settings read
        // downgrades the answer to unknown only when nothing above it was
        // proven wrong.
        MergeSources::Unreadable(err) => {
            if faults.is_empty() {
                return Ok(StepState::unknown(err));
            }
        }
    }
    if let Some(shape) = gate_faults(ctx) {
        faults.push(shape);
    }
    if !faults.is_empty() {
        return Ok(StepState::not(faults.join("; ")));
    }
    Ok(StepState::ok(format!(
        "{name} holds the release-merge shape"
    )))
}

/// The ways the named gate is shaped so that it cannot report a blocking
/// answer. A required check that never reports is a broken trunk
/// protection, not a weaker guarantee, so each of these is a fault rather
/// than a limitation. Read only where the check is named: without the flag
/// the observation knows no gate.
///
/// It judges the gate alone. Which other jobs a project means to block a
/// merge is intent, no file states it, and `forges/github.md` carries that
/// as a convention instead.
fn gate_faults(ctx: &Ctx) -> Option<String> {
    let check = ctx.required_check.as_deref()?;
    workflow_jobs::faults(
        &workflow_jobs::read_gate(&ctx.target, check, ctx.trunk()),
        check,
        ctx.trunk(),
    )
}

/// What the repository's squash message settings hold.
enum MergeSources {
    /// The request's title and body, as the setup owns.
    Owned,
    /// Proven other values, one fault line each.
    Faults(Vec<String>),
    /// The settings could not be read.
    Unreadable(String),
}

/// The squash message sources, repository settings beside the ruleset:
/// with the title source unset, a one-commit request offers that commit's
/// own subject as the trunk's message, which the bot then reads for the
/// version; with the message source on another value, the trunk's body is
/// not the request's description the content gates judged. One GET
/// answers for both, each faulted by name.
fn squash_merge_sources(ctx: &Ctx, run: &mut Runner) -> Result<MergeSources, RkError> {
    Ok(match api_get(ctx, run, &format!("repos/{}", ctx.repo))? {
        Api::Ok(body) => {
            let mut faults = Vec::new();
            let owned_title = ctx.protection().github.squash_title_source.as_str();
            let owned_body = ctx.protection().github.squash_body_source.as_str();
            if body["squash_merge_commit_title"] != owned_title {
                faults.push(format!(
                    "the squash title source is {} where the setup owns {owned_title}",
                    body["squash_merge_commit_title"]
                ));
            }
            if body["squash_merge_commit_message"] != owned_body {
                faults.push(format!(
                    "the squash message source is {} where the setup owns {owned_body}",
                    body["squash_merge_commit_message"]
                ));
            }
            if faults.is_empty() {
                MergeSources::Owned
            } else {
                MergeSources::Faults(faults)
            }
        }
        Api::Missing => MergeSources::Faults(vec![format!("the forge does not know {}", ctx.repo)]),
        Api::Failed(err) => MergeSources::Unreadable(err),
    })
}

/// One ruleset lookup by name: found, provably absent, or unreadable —
/// an unreadable inventory must never read as an absent ruleset.
enum RulesetLookup {
    /// The ruleset exists; its detail body.
    Found(Value),
    /// The inventory was read successfully and no ruleset carries the
    /// name.
    Absent,
    /// The inventory or the detail could not be read.
    Unreadable(String),
}

/// A ruleset's detail body by name.
fn github_ruleset_body(ctx: &Ctx, run: &mut Runner, name: &str) -> Result<RulesetLookup, RkError> {
    // A 404 on the collection is an unreachable inventory — a missing
    // repository or an unauthorized read — never an empty one: an empty
    // inventory answers 200 with an empty list.
    let list = match api_get(ctx, run, &format!("repos/{}/rulesets", ctx.repo))? {
        Api::Ok(body) => body,
        Api::Missing => {
            return Ok(RulesetLookup::Unreadable(
                "the ruleset inventory is not readable".into(),
            ));
        }
        Api::Failed(err) => return Ok(RulesetLookup::Unreadable(err)),
    };
    let id = list
        .as_array()
        .into_iter()
        .flatten()
        .find(|ruleset| ruleset["name"] == name)
        .and_then(|ruleset| ruleset["id"].as_i64());
    let Some(id) = id else {
        return Ok(RulesetLookup::Absent);
    };
    match api_get(ctx, run, &format!("repos/{}/rulesets/{id}", ctx.repo))? {
        Api::Ok(body) => Ok(RulesetLookup::Found(body)),
        // A listed id that answers 404 is not proof of absence either — the
        // forge also answers 404 for an unauthorized read — so a rerun
        // decides, rather than a false drift.
        Api::Missing => Ok(RulesetLookup::Unreadable(format!(
            "the {name} detail is not readable"
        ))),
        Api::Failed(err) => Ok(RulesetLookup::Unreadable(err)),
    }
}

/// The GitLab limitation the `auto-merge` step reports: the forge has no
/// project-level switch, so the observation reads the pipeline requirement
/// the trunk protection asserts.
const GITLAB_AUTO_MERGE_LIMITATION: &str = "the forge offers no project-level auto-merge switch: availability follows the pipeline requirement protect-trunk asserts, and turning that requirement off removes auto-merge with nothing here reporting it";

/// The GitLab limitation `protect-tags` and `protections-check` report.
const GITLAB_TAG_LIMITATION: &str =
    "an Owner or Maintainer can still delete a protected tag through the UI or API";

/// The fault a merge queue on the trunk reads as: what is enabled, what it
/// costs, and how to undo it. This convention refuses a queue rather than
/// owning one, so the operator needs the consequence rather than a rule
/// type's bare name.
const MERGE_QUEUE_FAULT: &str = "a merge queue is enabled on the trunk; this convention lands no workflow that triggers on merge_group, so the queue waits on a required check that never reports and drops the request when its CI timeout expires. rk setup step protect-trunk --apply rewrites the ruleset without it";

/// The freshness defect is independent of an absent required check.
const STALE_MERGE_FAULT: &str = "the trunk permits a merge from a branch that does not carry the trunk's tip; an armed release request can therefore ship a version computed against a trunk that moved. rk setup step protect-trunk --apply rewrites the ruleset with the freshness requirement";

/// The GitLab limitation `protect-trunk` and `protections-check` report:
/// the title gate rides the request's own pipeline on this forge.
const GITLAB_TITLE_LIMITATION: &str = "the title gate stops accident, not authority: a merge request runs its own CI configuration, and a title edit starts no new pipeline";

#[allow(clippy::too_many_lines)]
fn gitlab(ctx: &Ctx, step: &str, run: &mut Runner) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    let project = ctx.repo.replace('/', "%2F");
    match step {
        "private-vulnerability-reporting" => {
            let path = format!("projects/{project}");
            Ok(match api_get(ctx, run, &path)? {
                Api::Ok(body) => {
                    let access = body["issues_access_level"].as_str();
                    if !matches!(access, Some("enabled" | "private" | "disabled")) {
                        StepState::unknown("issue intake access is unreadable")
                    } else if body
                        .get("issues_enabled")
                        .is_some_and(|flag| !flag.is_boolean())
                    {
                        StepState::unknown("legacy issue intake flag is unreadable")
                    } else if body["issues_enabled"] == false || access == Some("disabled") {
                        StepState::not("issue intake is disabled; see setup guide step 3g")
                    } else if access == Some("private") {
                        StepState::not("issue intake is restricted; see setup guide step 3g")
                    } else {
                        StepState::ok_with_limitation(
                            "issue intake is enabled",
                            GITLAB_PRIVATE_REPORTING_LIMITATION,
                        )
                    }
                }
                Api::Missing => {
                    StepState::unknown(format!("{path}: issue intake is unreadable (404)"))
                }
                Api::Failed(err) => StepState::unknown(format!("{path}: {err}")),
            })
        }

        "default-branch" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                let found = body["default_branch"].as_str().unwrap_or("");
                if found == trunk {
                    StepState::ok(format!("{trunk} is the default branch"))
                } else {
                    StepState::not(format!("the default branch is {found}"))
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "single-trunk" => {
            for candidate in ctx.retired_branches() {
                let candidate = candidate.as_str();
                if candidate == trunk {
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
        "merge-cleanup" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                if body["remove_source_branch_after_merge"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    StepState::ok("a merged branch is deleted by the forge")
                } else {
                    StepState::not("a merged branch outlives its merge")
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "auto-merge" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                if body["only_allow_merge_if_pipeline_succeeds"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    StepState::ok_with_limitation(
                        "a request may merge itself once its pipeline passes",
                        GITLAB_AUTO_MERGE_LIMITATION,
                    )
                } else {
                    StepState::not(
                        "the pipeline requirement auto-merge rides on is off; protect-trunk asserts it",
                    )
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
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
                &format!("projects/{project}/protected_branches/{trunk}"),
            )? {
                Api::Ok(body) => body,
                Api::Missing => {
                    return Ok(StepState::not(format!("{trunk} is not protected")));
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
            let policy = ctx.protection();
            let no_push =
                grants.len() == 1 && grants[0]["access_level"] == policy.gitlab.push_access_level;
            // The merge grant is owned exactly too: a merge level of 0 keeps
            // every release request unmergeable while the push shape reads
            // clean, so both halves are checked.
            let merges = protection["merge_access_levels"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let can_merge =
                merges.len() == 1 && merges[0]["access_level"] == policy.gitlab.merge_access_level;
            let settings = match api_get(ctx, run, &format!("projects/{project}"))? {
                Api::Ok(body) => body,
                Api::Missing | Api::Failed(_) => Value::Null,
            };
            let mut faults = Vec::new();
            if !no_push {
                faults.push(format!(
                    "{trunk} still takes a direct push: the forge honors the most permissive of {} push grants",
                    grants.len()
                ));
            }
            if !can_merge {
                faults.push(format!(
                    "{trunk} merge grants are not exactly the one owned maintainer level"
                ));
            }
            if protection["allow_force_push"] != false {
                faults.push(format!("{trunk} allows force pushes"));
            }
            if settings["only_allow_merge_if_pipeline_succeeds"] != true {
                faults.push("the pipeline requirement is off".to_owned());
            }
            if settings["merge_method"] != policy.gitlab.merge_method.as_str() {
                faults.push("the merge method is not fast-forward".to_owned());
            }
            if settings["squash_option"] != policy.gitlab.squash_option.as_str() {
                faults.push("merge requests do not always squash".to_owned());
            }
            if settings["squash_commit_template"] != policy.gitlab.squash_commit_template.as_str() {
                faults.push("the squash template is not the merge request's title".to_owned());
            }
            Ok(if faults.is_empty() {
                StepState::ok_with_limitation(
                    format!("{trunk} holds the release-merge shape"),
                    GITLAB_TITLE_LIMITATION,
                )
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
                    let level_ok = |levels: &Value| {
                        levels
                            .as_array()
                            .is_some_and(|list| list.len() == 1 && list[0]["access_level"] == 40)
                    };
                    if body["allow_force_push"] != false {
                        StepState::not("release/* allows force pushes")
                    } else if !level_ok(&body["push_access_levels"])
                        || !level_ok(&body["merge_access_levels"])
                    {
                        // A push level of 0 blocks the documented
                        // cherry-pick-by-push path while force-push reads
                        // clean, so the grant shape is owned exactly.
                        StepState::not(
                            "release/* grants are not exactly the owned maintainer levels",
                        )
                    } else {
                        StepState::ok("release/* refuses force pushes and deletion by git clients")
                    }
                }
                Api::Missing => StepState::inapplicable(
                    "release/* is unprotected; optional — applied only where older lines exist",
                ),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "protections-check" => {
            // Same separation as the sibling forge: proven drift wins,
            // an outage with nothing proven wrong stays unknown.
            let mut failures = Vec::new();
            let mut unknowns = Vec::new();
            // Every satisfied step's limitation survives the aggregate: a
            // first limitation must not shadow a second.
            let mut limitations: Vec<String> = Vec::new();
            for owned in ["protect-trunk", "protect-tags", "protect-release-lines"] {
                match gitlab(ctx, owned, run)? {
                    StepState::Satisfied {
                        limitation: found, ..
                    } => limitations.extend(found),
                    StepState::Inapplicable { .. } => {}
                    StepState::Unsatisfied { detail } => {
                        failures.push(format!("{owned}: {detail}"));
                    }
                    StepState::Unknown { detail } => {
                        unknowns.push(format!("{owned}: {detail}"));
                    }
                }
            }
            Ok(if !failures.is_empty() {
                StepState::not(failures.join("; "))
            } else if !unknowns.is_empty() {
                StepState::unknown(unknowns.join("; "))
            } else {
                StepState::Satisfied {
                    detail: "the protections hold, as far as this forge enforces them".into(),
                    limitation: if limitations.is_empty() {
                        None
                    } else {
                        Some(limitations.join("; "))
                    },
                }
            })
        }
        _ => Ok(StepState::unknown(format!("no observation for {step}"))),
    }
}
