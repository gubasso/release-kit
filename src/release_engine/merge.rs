//! `rk release merge`: the operator's order to merge the trunk's release
//! request.
//!
//! The verb previews by default and writes nothing: it reads the one open
//! release request against the trunk, the trunk tip, the request's head,
//! and the required check for that head. Under `--apply` it dispatches the
//! landed merge job with exactly those values and a fresh correlation, and
//! waits for that one run. The job runs as the release bot, proves every
//! value again, and merges with the head pinned; rk merges nothing itself.
//!
//! A release line keeps the merge by a person, so the verb reads the trunk
//! alone.

use std::time::{Duration, Instant};

use camino::Utf8Path;
use serde::Serialize;
use serde_json::{Value, json};

use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest;
use crate::profile::ReleaseMode;
use crate::release_state::{Api, gitlab_project, proposed_version};

use super::Done;
use super::forge::Client;

/// The report's schema.
pub const SCHEMA: &str = "rk.release-merge/1";

/// How long the wait sleeps between two reads of the run, by default.
const WAIT_INTERVAL_MS: u64 = 15_000;

/// The variable that overrides the interval, in milliseconds.
const WAIT_INTERVAL_VAR: &str = "RK_RELEASE_WAIT_INTERVAL_MS";

/// The prefix of the merge run's name; the correlation follows it.
pub const RUN_NAME_PREFIX: &str = "rk release merge ";

/// The report: what the verb read, and under `--apply` what the run did.
#[derive(Debug, Serialize)]
pub struct Report {
    /// The schema this document follows.
    pub schema: &'static str,
    /// `preview`, or under `--apply` `merged`.
    pub state: &'static str,
    /// The release request, by number.
    pub request: u64,
    /// The version the request proposes, where its title names one.
    pub version: Option<String>,
    /// The trunk.
    pub trunk: String,
    /// The trunk tip the merge expects.
    pub base: String,
    /// The request head the merge expects.
    pub head: String,
    /// The required check, on GitHub; GitLab judges the whole pipeline.
    pub check: Option<String>,
    /// What the check or the pipeline reads for the head.
    pub check_state: String,
    /// The correlation the dispatch carried, under `--apply`.
    pub correlation: Option<String>,
    /// The run or pipeline that merged, under `--apply`.
    pub run: Option<String>,
    /// The commit the merge wrote on the trunk, where the forge names it.
    pub merge_commit: Option<String>,
}

/// What the verb read before it acts.
struct Candidate {
    number: u64,
    version: Option<String>,
    head: String,
    base: String,
    check: Option<String>,
    check_state: String,
    green: bool,
}

/// The record's facts the verb needs.
struct Record {
    forge: Forge,
    repo: String,
    trunk: String,
    shape: &'static str,
    workflow: &'static str,
    required_check: String,
}

/// Preview the trunk's release request, or under `apply` dispatch its
/// merge and wait up to `timeout` seconds for the run.
///
/// # Errors
///
/// A refusal where the target releases through no landed automation, where
/// zero or several release requests stand open against the trunk, where
/// the check is not green under `apply`, where the forge refuses the
/// dispatch, or where the run fails or does not conclude in time.
pub fn run(target: &Utf8Path, apply: bool, timeout: u64) -> Result<Done<Report>, RkError> {
    let record = record(target)?;
    let client = Client::resolve(record.forge, target, &record.repo)?;
    let candidate = candidate(&client, &record)?;
    let mut lines = vec![
        format!(
            "release request {} proposing {} from {} into {}",
            crate::release_state::request_name(record.forge, candidate.number),
            candidate.version.as_deref().unwrap_or("an unread version"),
            candidate.head,
            record.trunk
        ),
        format!("{} at {}", record.trunk, candidate.base),
        candidate.check.as_ref().map_or_else(
            || {
                format!(
                    "the request's pipeline at the head reads {}",
                    candidate.check_state
                )
            },
            |name| {
                format!(
                    "the check {name} at the head reads {}",
                    candidate.check_state
                )
            },
        ),
    ];
    let mut report = Report {
        schema: SCHEMA,
        state: "preview",
        request: candidate.number,
        version: candidate.version.clone(),
        trunk: record.trunk.clone(),
        base: candidate.base.clone(),
        head: candidate.head.clone(),
        check: candidate.check.clone(),
        check_state: candidate.check_state.clone(),
        correlation: None,
        run: None,
        merge_commit: None,
    };
    if !apply {
        return Ok(Done {
            report,
            lines,
            next: vec![format!("rk release merge --target {target} --apply")],
        });
    }
    if !candidate.green {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "the head of {} reads {}, so no merge is dispatched",
                    crate::release_state::request_name(record.forge, candidate.number),
                    candidate.check_state
                ),
            )
            .expected("a passing check for the request's head")
            .action("wait for the check, then run rk release merge --apply again")
            .target_state("nothing was dispatched"),
        ));
    }
    let correlation = correlation();
    let run = match record.forge {
        Forge::Github => dispatch_github(&client, &record, &candidate, &correlation)?,
        Forge::Gitlab => dispatch_gitlab(&client, &record, &candidate)?,
    };
    lines.push(format!("dispatched the merge as {run}"));
    let failed = wait(&client, &record, &correlation, &run, timeout)?;
    let name = crate::release_state::request_name(record.forge, candidate.number);
    let merge_commit = outcome(&client, &record, candidate.number, &run, failed, &mut lines)?;
    lines.push(format!("merged {name} at {}", candidate.head));
    report.state = "merged";
    report.correlation = Some(correlation);
    report.run = Some(run);
    report.merge_commit.clone_from(&merge_commit);
    let next = merge_commit.map_or_else(
        || vec![format!("rk release status --target {target}")],
        |sha| vec![format!("rk release status --target {target} --wait {sha}")],
    );
    Ok(Done {
        report,
        lines,
        next,
    })
}

/// What the run did, read from the request itself: the commit the merge
/// wrote, with a line where the run failed after its merge, or a refusal
/// where nothing merged or the outcome is unread.
fn outcome(
    client: &Client,
    record: &Record,
    number: u64,
    run: &str,
    failed: Option<String>,
    lines: &mut Vec<String>,
) -> Result<Option<String>, RkError> {
    // A run can fail after its merge, in a cleanup such as the branch
    // deletion, so the request's own state decides what happened, never
    // the run's conclusion alone.
    let name = crate::release_state::request_name(record.forge, number);
    Ok(match (merged(client, record, number), failed) {
        (Some((true, sha)), None) => sha,
        (Some((true, sha)), Some(conclusion)) => {
            lines.push(format!(
                "the merge run {run} concluded {conclusion} after {name} merged; its log names the step that failed"
            ));
            sha
        }
        (Some((false, _)), Some(conclusion)) => {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!("the merge run {run} concluded {conclusion}, and {name} did not merge"),
                )
                .expected("a merge run that proves the request and merges it")
                .action("read the merge job's log on the forge: it names the proof that refused, and rk release merge previews again")
                .target_state("the release request stays open"),
            ));
        }
        (answer, conclusion) => {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::ObservationIncomplete,
                    format!(
                        "the merge run {run} concluded {}, and whether {name} merged {}",
                        conclusion.as_deref().unwrap_or("success"),
                        if answer.is_some() {
                            "reads no"
                        } else {
                            "is unread"
                        }
                    ),
                )
                .action("run rk release status: it reads the request and the last release")
                .target_state("the merge outcome is unknown"),
            ));
        }
    })
}

/// The record's facts, refusing a target that lands no release automation.
fn record(target: &Utf8Path) -> Result<Record, RkError> {
    let refuse = |message: String, expected: &str| {
        RkError::refusal(
            Diagnostic::new(Reason::StateDrift, message)
                .expected(expected.to_owned())
                .target_state("nothing was dispatched"),
        )
    };
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory"),
            )
            .expected("the checkout of a landed target"),
        ));
    }
    // The configuration loads first, so a retired key refuses here with
    // the diagnostic every verb reports.
    crate::config::load(target.as_std_path())?;
    let manifest = manifest::load(target)?.ok_or_else(|| {
        refuse(
            format!("no landing at {target}"),
            "a target release-kit landed",
        )
    })?;
    let release = &manifest.profile.release;
    if release.mode != ReleaseMode::Automatic {
        return Err(refuse(
            "the record selects no automated release, so no merge job is landed".to_owned(),
            "profile.release.mode = \"automatic\"",
        ));
    }
    let driver = release.driver.as_deref();
    let shape = crate::projection::release_driver_shape(driver).ok_or_else(|| {
        refuse(
            format!("the release driver {driver:?} lands no merge job"),
            "a rust, bash, or python release driver",
        )
    })?;
    let forge = manifest
        .profile
        .forge
        .as_deref()
        .and_then(Forge::parse)
        .ok_or_else(|| {
            refuse(
                "the record names no forge this binary merges on".to_owned(),
                "a github or gitlab forge",
            )
        })?;
    if manifest.parameters.repo.is_empty() {
        return Err(refuse(
            "the record names no forge repository".to_owned(),
            "the project path recorded at landing",
        ));
    }
    Ok(Record {
        forge,
        repo: manifest.parameters.repo.clone(),
        trunk: manifest.git.trunk.clone(),
        shape,
        workflow: if driver == Some("rust") {
            "release-plz.yml"
        } else {
            "release.yml"
        },
        required_check: manifest.parameters.required_check.clone(),
    })
}

/// The one open release request against the trunk, with what the merge
/// will expect of it.
fn candidate(client: &Client, record: &Record) -> Result<Candidate, RkError> {
    let (path, branch, head_of) = match record.forge {
        Forge::Github => (
            format!(
                "repos/{}/pulls?state=open&base={}&per_page=100",
                record.repo,
                query(&record.trunk)
            ),
            "/head/ref",
            "/head/sha",
        ),
        Forge::Gitlab => (
            format!(
                "projects/{}/merge_requests?state=opened&target_branch={}&per_page=100",
                gitlab_project(&record.repo),
                query(&record.trunk)
            ),
            "/source_branch",
            "/sha",
        ),
    };
    let open = client.read_all(&path)?;
    let found: Vec<&Value> = open
        .iter()
        .filter(|request| {
            request
                .pointer(branch)
                .and_then(Value::as_str)
                .is_some_and(|name| name.starts_with(record.shape))
        })
        .collect();
    let number_of = |request: &Value| {
        request
            .get(if record.forge == Forge::Github {
                "number"
            } else {
                "iid"
            })
            .and_then(Value::as_u64)
    };
    let [request] = found.as_slice() else {
        let names: Vec<String> = found
            .iter()
            .filter_map(|request| number_of(request))
            .map(|n| crate::release_state::request_name(record.forge, n))
            .collect();
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                if names.is_empty() {
                    format!("no release request stands open against {}", record.trunk)
                } else {
                    format!(
                        "{} release requests stand open against {}: {}",
                        names.len(),
                        record.trunk,
                        names.join(", ")
                    )
                },
            )
            .expected(format!(
                "exactly one open release request against {}, on a {}* branch",
                record.trunk, record.shape
            ))
            .action("read rk release status; the bot opens the request on the next releasing push")
            .target_state("nothing was dispatched"),
        ));
    };
    let unread = |what: &str| {
        RkError::refusal(
            Diagnostic::new(
                Reason::ObservationIncomplete,
                format!("the forge answered no {what}"),
            )
            .action("run rk release merge again once the forge answers")
            .target_state("nothing was dispatched"),
        )
    };
    let number = number_of(request).ok_or_else(|| unread("request number"))?;
    let head = request
        .pointer(head_of)
        .and_then(Value::as_str)
        .ok_or_else(|| unread("request head"))?
        .to_owned();
    let version = request["title"].as_str().and_then(proposed_version);
    let base = trunk_tip(client, record)?.ok_or_else(|| unread("trunk tip"))?;
    let (check, check_state, green) = check(client, record, number, &head)?;
    Ok(Candidate {
        number,
        version,
        head,
        base,
        check,
        check_state,
        green,
    })
}

/// The trunk tip the merge will expect, where the forge names it.
fn trunk_tip(client: &Client, record: &Record) -> Result<Option<String>, RkError> {
    Ok(match record.forge {
        Forge::Github => client.read(&format!(
            "repos/{}/branches/{}",
            record.repo,
            query(&record.trunk)
        ))?,
        Forge::Gitlab => client.read(&format!(
            "projects/{}/repository/branches/{}",
            gitlab_project(&record.repo),
            query(&record.trunk)
        ))?,
    }
    .and_then(|branch| {
        branch
            .pointer(if record.forge == Forge::Github {
                "/commit/sha"
            } else {
                "/commit/id"
            })
            .and_then(Value::as_str)
            .map(str::to_owned)
    }))
}

/// The required check, or on GitLab the head pipeline, for `head`: its
/// name, what it reads, and whether it passed.
fn check(
    client: &Client,
    record: &Record,
    number: u64,
    head: &str,
) -> Result<(Option<String>, String, bool), RkError> {
    Ok(match record.forge {
        Forge::Github => {
            if record.required_check.is_empty() {
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::ConfigInvalid,
                        "the record names no required check, so the merge job has nothing to judge"
                            .to_owned(),
                    )
                    .expected("setup.required_check in .release-kit/config.toml")
                    .action("run rk upgrade --target . --apply, which records the default check")
                    .target_state("nothing was dispatched"),
                ));
            }
            let runs = client.read(&format!(
                "repos/{}/commits/{head}/check-runs?check_name={}&filter=latest",
                record.repo,
                query(&record.required_check)
            ))?;
            let state = runs.as_ref().map_or_else(
                || "no check runs".to_owned(),
                |body| match body["check_runs"].as_array().map(Vec::as_slice) {
                    Some([run]) => format!(
                        "{} {}",
                        run["status"].as_str().unwrap_or("unknown"),
                        run["conclusion"].as_str().unwrap_or("none")
                    ),
                    Some(runs) => format!("{} results", runs.len()),
                    None => "an unread answer".to_owned(),
                },
            );
            let green = state == "completed success";
            (Some(record.required_check.clone()), state, green)
        }
        Forge::Gitlab => {
            let detail = client
                .read(&format!(
                    "projects/{}/merge_requests/{number}",
                    gitlab_project(&record.repo)
                ))?
                .unwrap_or(Value::Null);
            let pipeline = &detail["head_pipeline"];
            let state = if pipeline["sha"].as_str() == Some(head) {
                pipeline["status"].as_str().unwrap_or("unknown").to_owned()
            } else {
                "no pipeline for this head".to_owned()
            };
            let green = state == "success";
            (None, state, green)
        }
    })
}

/// Dispatch the GitHub merge job from the trunk's own definition, which is
/// the trusted one, and name the run the wait follows.
fn dispatch_github(
    client: &Client,
    record: &Record,
    candidate: &Candidate,
    correlation: &str,
) -> Result<String, RkError> {
    let body = json!({
        "ref": record.trunk,
        "inputs": {
            "action": "merge",
            "request": candidate.number.to_string(),
            "base": candidate.base,
            "head": candidate.head,
            "correlation": correlation,
        },
    });
    client.write(
        "POST",
        &format!(
            "repos/{}/actions/workflows/{}/dispatches",
            record.repo, record.workflow
        ),
        Some(&body),
        "the merge dispatch",
    )?;
    Ok(format!("{RUN_NAME_PREFIX}{correlation}"))
}

/// Create the GitLab merge pipeline on the trunk, and name it by its id,
/// which is the correlation the wait follows.
fn dispatch_gitlab(
    client: &Client,
    record: &Record,
    candidate: &Candidate,
) -> Result<String, RkError> {
    let body = json!({
        "ref": record.trunk,
        "variables": [
            {"key": "RK_RELEASE_ACTION", "value": "merge"},
            {"key": "RK_MERGE_REQUEST", "value": candidate.number.to_string()},
            {"key": "RK_MERGE_BASE", "value": candidate.base},
            {"key": "RK_MERGE_HEAD", "value": candidate.head},
        ],
    });
    let answer = client.write(
        "POST",
        &format!("projects/{}/pipeline", gitlab_project(&record.repo)),
        Some(&body),
        "the merge pipeline",
    )?;
    answer["id"]
        .as_u64()
        .map(|id| format!("pipeline {id}"))
        .ok_or_else(|| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::ObservationIncomplete,
                    "the forge created the merge pipeline and named no id".to_owned(),
                )
                .action("read the project's pipelines on the forge; the merge job reports there")
                .target_state("a merge pipeline may be running"),
            )
        })
}

/// Wait for the one correlated run to conclude: `None` where it succeeded,
/// or the conclusion it failed with.
fn wait(
    client: &Client,
    record: &Record,
    correlation: &str,
    run: &str,
    timeout: u64,
) -> Result<Option<String>, RkError> {
    let deadline = Instant::now() + Duration::from_secs(timeout);
    let interval = Duration::from_millis(
        std::env::var(WAIT_INTERVAL_VAR)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(WAIT_INTERVAL_MS),
    );
    loop {
        let outcome = match record.forge {
            Forge::Github => github_outcome(client, record, correlation),
            Forge::Gitlab => gitlab_outcome(client, record, run),
        };
        match outcome {
            Some(Ok(())) => return Ok(None),
            Some(Err(conclusion)) => return Ok(Some(conclusion)),
            None if Instant::now() >= deadline => {
                return Err(RkError::refusal(
                    Diagnostic::new(
                        Reason::StateDrift,
                        format!("the merge run {run} did not conclude within {timeout} seconds"),
                    )
                    .action("read the merge run on the forge, then rk release status")
                    .target_state("the merge run may still be running"),
                ));
            }
            None => std::thread::sleep(interval),
        }
    }
}

/// The correlated GitHub run's outcome, or `None` while it is not found or
/// not finished. Another run of the same workflow never ends the wait.
fn github_outcome(
    client: &Client,
    record: &Record,
    correlation: &str,
) -> Option<Result<(), String>> {
    let Api::Ok(body) = client.api(
        "GET",
        &format!(
            "repos/{}/actions/workflows/{}/runs?event=workflow_dispatch&branch={}&per_page=50",
            record.repo,
            record.workflow,
            query(&record.trunk)
        ),
        None,
    ) else {
        return None;
    };
    let name = format!("{RUN_NAME_PREFIX}{correlation}");
    let run = body["workflow_runs"]
        .as_array()?
        .iter()
        .find(|run| run["display_title"].as_str() == Some(name.as_str()))?;
    if run["status"].as_str() != Some("completed") {
        return None;
    }
    match run["conclusion"].as_str() {
        Some("success") => Some(Ok(())),
        other => Some(Err(other.unwrap_or("with no conclusion").to_owned())),
    }
}

/// The GitLab pipeline's outcome, or `None` while it runs.
fn gitlab_outcome(client: &Client, record: &Record, run: &str) -> Option<Result<(), String>> {
    let id = run.strip_prefix("pipeline ")?;
    let Api::Ok(body) = client.api(
        "GET",
        &format!("projects/{}/pipelines/{id}", gitlab_project(&record.repo)),
        None,
    ) else {
        return None;
    };
    match body["status"].as_str() {
        Some("success") => Some(Ok(())),
        Some(status @ ("failed" | "canceled" | "skipped")) => Some(Err(status.to_owned())),
        _ => None,
    }
}

/// Whether the request merged, and the commit the merge wrote on the
/// trunk where the forge names it; `None` where the request is unread.
///
/// A GitLab fast-forward merge names neither a merge nor a squash commit:
/// the trunk then stands at the request's own head.
fn merged(client: &Client, record: &Record, number: u64) -> Option<(bool, Option<String>)> {
    let path = match record.forge {
        Forge::Github => format!("repos/{}/pulls/{number}", record.repo),
        Forge::Gitlab => format!(
            "projects/{}/merge_requests/{number}",
            gitlab_project(&record.repo)
        ),
    };
    let Api::Ok(body) = client.api("GET", &path, None) else {
        return None;
    };
    let merged = match record.forge {
        Forge::Github => body["merged"].as_bool()?,
        Forge::Gitlab => body["state"].as_str()? == "merged",
    };
    let named = |key: &str| {
        body[key]
            .as_str()
            .filter(|sha| !sha.is_empty())
            .map(str::to_owned)
    };
    let sha = named("merge_commit_sha").or_else(|| named("squash_commit_sha"));
    let sha = match record.forge {
        Forge::Gitlab => sha.or_else(|| named("sha")),
        Forge::Github => sha,
    };
    Some((merged, sha.filter(|_| merged)))
}

/// A fresh correlation: the time and the process, so two dispatches in
/// one second still differ.
fn correlation() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    format!("{nanos:x}-{:x}", std::process::id())
}

/// One query value, percent-encoded so a check or branch name with spaces
/// or reserved characters stays one value.
fn query(value: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::query;

    #[test]
    fn a_query_value_keeps_its_reserved_characters_as_data() {
        assert_eq!(query("gate"), "gate");
        assert_eq!(query("build (matrix, 1)"), "build%20%28matrix%2C%201%29");
        assert_eq!(query("release/1.4"), "release%2F1.4");
    }

    #[test]
    fn the_release_merge_schema_snapshot_holds() {
        let report = super::Report {
            schema: super::SCHEMA,
            state: "preview",
            request: 7,
            version: Some("0.2.0".to_owned()),
            trunk: "master".to_owned(),
            base: "b".repeat(40),
            head: "a".repeat(40),
            check: Some("gate".to_owned()),
            check_state: "completed success".to_owned(),
            correlation: None,
            run: None,
            merge_commit: None,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("the report serializes"),
            format!(
                r#"{{"schema":"rk.release-merge/1","state":"preview","request":7,"version":"0.2.0","trunk":"master","base":"{}","head":"{}","check":"gate","check_state":"completed success","correlation":null,"run":null,"merge_commit":null}}"#,
                "b".repeat(40),
                "a".repeat(40)
            )
        );
    }
}
