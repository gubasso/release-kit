//! `rk release status`: where a landed target's release stands.
//!
//! The record answers the style, the integration authority, the checkout
//! mode, and the trunk, offline. The forge answers the release request,
//! the last release, and the runs, and a field the runtime cannot read
//! reads `unknown`: the verb never asks for a credential so that a field
//! can read. The registry answers whether a version is published. The
//! judgments live in [`crate::release_state`]; this handler reads, joins,
//! and renders.
//!
//! SATISFIES release-state:the-report-joins-the-record-and-the-forge

use std::time::{Duration, Instant};

use camino::Utf8Path;
use serde::Serialize;
use serde_json::Value;

use crate::cli::release::{ReleaseAction, ReleaseArgs};
use crate::commands::lines::{self, LineRow};
use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, CheckoutMode, Integration, Manifest, Style};
use crate::output::Output;
use crate::profile::ReleaseMode;
use crate::release_state::{
    self, Answer, Api, ForgeCli, LastRelease, Merged, Page, Pair, Request, Run, Symptom,
};

/// The report's shape version.
const SCHEMA: &str = "rk.release-status/1";

/// How long `--wait` sleeps between two reads, unless the environment
/// names another interval in milliseconds.
const WAIT_INTERVAL_MS: u64 = 15_000;

/// The variable that overrides the wait interval.
const WAIT_INTERVAL_VAR: &str = "RK_RELEASE_WAIT_INTERVAL_MS";

/// Whether the forge could be read, and why not.
#[derive(Debug, Serialize)]
struct ForgeState {
    /// The recorded forge.
    name: Option<String>,
    /// The recorded project path.
    repo: Option<String>,
    /// `yes`, `unknown`, or `not-applicable`.
    read: Answer,
    /// Why the forge was not read, where it was not.
    reason: Option<String>,
}

/// What `--wait` found on the commit.
#[derive(Debug, Serialize)]
struct Wait {
    /// The commit waited on.
    sha: String,
    /// The run that tags and publishes.
    publish: Option<Run>,
    /// The run that builds the artifacts, where the pair builds any.
    artifact: Option<Run>,
    /// The release tag that names the commit, once one does.
    tag: Option<String>,
    /// Whether the timeout ended the wait before both runs concluded.
    timed_out: bool,
}

/// The machine form of `rk release status`.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// Whether a landing record exists; every other field needs one.
    landed: bool,
    /// The recorded release style.
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<Style>,
    /// The recorded integration authority.
    #[serde(skip_serializing_if = "Option::is_none")]
    integration: Option<Integration>,
    /// The recorded checkout mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    checkout_mode: Option<CheckoutMode>,
    /// The recorded trunk.
    #[serde(skip_serializing_if = "Option::is_none")]
    trunk: Option<String>,
    /// Whether the forge could be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    forge: Option<ForgeState>,
    /// The open release request.
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<Request>,
    /// The newest release.
    #[serde(skip_serializing_if = "Option::is_none")]
    last_release: Option<LastRelease>,
    /// Every release line, as `rk lines list` reports it.
    #[serde(skip_serializing_if = "Option::is_none")]
    lines: Option<Vec<LineRow>>,
    /// Every symptom the state shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    symptoms: Option<Vec<Symptom>>,
    /// What `--wait` found, where it ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    wait: Option<Wait>,
}

/// Route one `rk release` invocation.
///
/// # Errors
///
/// Each verb's own failures; every failure is typed.
pub fn run(args: &ReleaseArgs) -> Result<(), RkError> {
    match &args.action {
        ReleaseAction::Status {
            target,
            wait,
            timeout,
            json,
        } => status(target, wait.as_deref(), *timeout, Output::new(*json)),
    }
}

/// Everything the forge answered, gathered once.
struct Observed {
    request: Request,
    last: LastRelease,
    symptoms: Vec<Symptom>,
    wait: Option<Wait>,
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass reads the record, the lines, and the forge, and renders the one report"
)]
fn status(target: &Utf8Path, wait: Option<&str>, timeout: u64, out: Output) -> Result<(), RkError> {
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory"),
            )
            .expected("an existing repository to report on"),
        ));
    }
    let Some(manifest) = manifest::load(target)? else {
        out.result_line(format!("no landing at {target}"));
        out.next(&[format!(
            "rk status --target {target} classifies the target; the rk-setup skill lands the convention"
        )]);
        return out.emit(&Report {
            schema: SCHEMA,
            landed: false,
            style: None,
            integration: None,
            checkout_mode: None,
            trunk: None,
            forge: None,
            request: None,
            last_release: None,
            lines: None,
            symptoms: None,
            wait: None,
        });
    };
    let lines = lines::rows(target)?;
    let pair = pair_of(target, &manifest);
    let (forge, observed) = match &pair {
        Err(reason) => (
            ForgeState {
                name: manifest.profile.forge.clone(),
                repo: (!manifest.parameters.repo.is_empty())
                    .then(|| manifest.parameters.repo.clone()),
                read: Answer::NotApplicable,
                reason: Some(reason.clone()),
            },
            Observed {
                request: Request::none(),
                last: LastRelease {
                    tag: local_newest_release(target),
                    published: Answer::NotApplicable,
                    artifacts_verified: Answer::NotApplicable,
                },
                symptoms: Vec::new(),
                wait: None,
            },
        ),
        Ok(pair) => match ForgeCli::resolve(pair.forge, target) {
            Err(error) => (
                ForgeState {
                    name: Some(pair.forge.as_str().to_owned()),
                    repo: Some(pair.repo.clone()),
                    read: Answer::Unknown,
                    reason: Some(error.to_string()),
                },
                Observed {
                    request: Request::unknown(),
                    last: LastRelease {
                        tag: local_newest_release(target),
                        published: Answer::Unknown,
                        artifacts_verified: if pair.artifacts {
                            Answer::Unknown
                        } else {
                            Answer::NotApplicable
                        },
                    },
                    symptoms: Vec::new(),
                    wait: None,
                },
            ),
            Ok(cli) => {
                let mut reader = Reader {
                    cli: &cli,
                    pair,
                    read: Answer::Yes,
                    failures: std::cell::RefCell::new(Vec::new()),
                };
                let observed = reader.observe(target, wait, timeout);
                (
                    ForgeState {
                        name: Some(pair.forge.as_str().to_owned()),
                        repo: Some(pair.repo.clone()),
                        read: reader.read,
                        reason: reader.reason(),
                    },
                    observed,
                )
            }
        },
    };
    if wait.is_some() && observed.wait.is_none() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::ObservationIncomplete,
                format!(
                    "--wait needs the forge's runs, and this runtime cannot read them: {}",
                    forge
                        .reason
                        .as_deref()
                        .unwrap_or("the forge did not answer")
                ),
            )
            .expected("a forge CLI that can read this project's runs")
            .action("rerun where the forge CLI is authenticated for this project")
            .target_state("unchanged"),
        ));
    }
    render(out, &manifest, &forge, &observed, &lines);
    let failure = observed.wait.as_ref().and_then(wait_failure);
    out.emit(&Report {
        schema: SCHEMA,
        landed: true,
        style: manifest.profile.release.style,
        integration: Some(manifest.git.integration),
        checkout_mode: Some(manifest.git.checkout_mode),
        trunk: Some(manifest.git.trunk),
        forge: Some(forge),
        request: Some(observed.request),
        last_release: Some(observed.last),
        lines: Some(lines),
        symptoms: Some(observed.symptoms),
        wait: observed.wait,
    })?;
    failure.map_or(Ok(()), Err)
}

/// The pair the forge reads need, or why the forge is not read at all.
fn pair_of(target: &Utf8Path, manifest: &Manifest) -> Result<Pair, String> {
    if manifest.profile.release.mode != ReleaseMode::Automatic {
        return Err("the record selects no automated release".to_owned());
    }
    let driver = manifest
        .profile
        .release
        .driver
        .clone()
        .ok_or_else(|| "the record names no release driver".to_owned())?;
    let forge = manifest
        .profile
        .forge
        .as_deref()
        .and_then(Forge::parse)
        .ok_or_else(|| "the record names no forge this binary reads".to_owned())?;
    if manifest.parameters.repo.is_empty() {
        return Err("the record names no forge repository".to_owned());
    }
    let artifacts = match (forge, driver.as_str()) {
        (Forge::Github, "rust") => manifest.file("dist-workspace.toml").is_some(),
        (Forge::Github | Forge::Gitlab, "bash") => true,
        _ => false,
    };
    Ok(Pair {
        package: release_state::package_name(target.as_std_path(), &driver),
        driver,
        forge,
        repo: manifest.parameters.repo.clone(),
        trunk: manifest.git.trunk.clone(),
        artifacts,
    })
}

/// The newest release tag the clone holds, read offline.
fn local_newest_release(target: &Utf8Path) -> Option<String> {
    let mut command = std::process::Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let output = command
        .arg("-C")
        .arg(target.as_std_path())
        .args(["tag", "--list", "v*"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    release_state::newest_release(text.lines().map(str::trim))
}

/// The run that failed a wait, as the error the verb exits with after it
/// rendered the report; a timeout refuses, because nothing was judged.
fn wait_failure(wait: &Wait) -> Option<RkError> {
    if wait.timed_out {
        return Some(RkError::refusal(
            Diagnostic::new(
                Reason::ObservationIncomplete,
                format!(
                    "the runs on {} did not conclude before the timeout",
                    wait.sha
                ),
            )
            .expected("the publish run, and the artifact run where the pair builds one, concluded")
            .action("rerun with a longer --timeout, or read the runs on the forge")
            .target_state("unchanged"),
        ));
    }
    let failed = [&wait.publish, &wait.artifact]
        .into_iter()
        .flatten()
        .find(|run| run.concluded() && !run.succeeded())?;
    Some(RkError::check_failed(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "run {} on {} concluded {}",
                failed.id,
                wait.sha,
                failed.conclusion.as_deref().unwrap_or("without success")
            ),
        )
        .expected("every run on the release commit concluded success")
        .action("rk method recovery names the path for the symptom this report shows"),
    ))
}

/// The forge reads for one pair, with what they could not read.
struct Reader<'a> {
    cli: &'a ForgeCli,
    pair: &'a Pair,
    read: Answer,
    failures: std::cell::RefCell<Vec<String>>,
}

impl Reader<'_> {
    fn reason(&self) -> Option<String> {
        self.failures.borrow().first().cloned()
    }

    /// One call; a failure is remembered so the report says why a field
    /// reads unknown.
    fn api(&self, path: &str) -> Option<Value> {
        match self.cli.api(path) {
            Api::Ok(body) => Some(body),
            Api::Missing => None,
            Api::Failed(reason) => {
                self.failures.borrow_mut().push(reason);
                None
            }
        }
    }

    /// One call that tells a definite not-found apart from a failure.
    fn api_answer(&self, path: &str) -> Api {
        let answer = self.cli.api(path);
        if let Api::Failed(reason) = &answer {
            self.failures.borrow_mut().push(reason.clone());
        }
        answer
    }

    fn project(&self) -> String {
        release_state::gitlab_project(&self.pair.repo)
    }

    fn observe(&mut self, target: &Utf8Path, wait: Option<&str>, timeout: u64) -> Observed {
        let request = self.open_request();
        let mut wait = wait.map(|sha| self.wait(sha, timeout));
        // Read after the wait: a trunk run tags its own commit while it
        // runs, so a list read before it cannot hold the tag it made.
        let tags = self.tags();
        if let Some(wait) = wait.as_mut().filter(|wait| wait.tag.is_none()) {
            wait.tag = tags.as_ref().and_then(|tags| {
                tags.iter()
                    .find(|(name, commit)| *commit == wait.sha && name.starts_with('v'))
                    .map(|(name, _)| name.clone())
            });
        }
        // An unread tag list is no empty one: without it, the absence
        // of a release is not observed either.
        let unread = tags.is_none();
        // A wait reports the release its commit made and no other: a
        // commit that made none never borrows an older release's state.
        let tag = wait.as_ref().map_or_else(
            || {
                tags.as_ref().map_or_else(
                    || local_newest_release(target),
                    |tags| {
                        release_state::newest_release(tags.iter().map(|(name, _)| name.as_str()))
                    },
                )
            },
            |wait| wait.tag.clone(),
        );
        // A tag the clone supplies because the forge refused its list is
        // no observed release: its fields stay unknown, as they do where
        // the forge CLI is missing.
        let from_clone = unread && wait.is_none();
        let page = if from_clone {
            None
        } else {
            tag.as_deref().and_then(|tag| self.page(tag))
        };
        let last = LastRelease {
            published: if from_clone {
                Answer::Unknown
            } else {
                tag.as_deref().map_or(
                    if unread {
                        Answer::Unknown
                    } else {
                        Answer::NotApplicable
                    },
                    |tag| self.published(tag, page.as_ref()),
                )
            },
            artifacts_verified: if !self.pair.artifacts {
                Answer::NotApplicable
            } else if from_clone {
                Answer::Unknown
            } else if tag.is_none() {
                if unread { Answer::Unknown } else { Answer::No }
            } else {
                page.as_ref().map_or(Answer::Unknown, Page::verified)
            },
            tag,
        };
        let merged = self.merged().map(|(number, version)| {
            let name = format!("v{version}");
            let tagged = tags.as_ref().map_or(Answer::Unknown, |tags| {
                Answer::of(tags.iter().any(|(tag, _)| *tag == name))
            });
            let published = if last.tag.as_deref() == Some(name.as_str()) {
                last.published
            } else if tagged == Answer::No && self.pair.registry().is_none() {
                Answer::No
            } else {
                self.published(&name, None)
            };
            Merged {
                number,
                version,
                tagged,
                published,
            }
        });
        let symptoms =
            release_state::symptoms(merged.as_ref(), &last, page.as_ref(), self.pair.artifacts);
        if !self.failures.borrow().is_empty() {
            self.read = Answer::Unknown;
        }
        Observed {
            request,
            last,
            symptoms,
            wait,
        }
    }

    /// Whether the version a tag names is published: the registry's
    /// answer where the pair has one, the release page's otherwise.
    fn published(&self, tag: &str, page: Option<&Page>) -> Answer {
        let version = tag.strip_prefix('v').unwrap_or(tag);
        match (self.pair.registry(), self.pair.package.as_deref()) {
            (Some(registry), Some(package)) => {
                release_state::registry_serves(registry, package, Some(version))
            }
            (Some(_), None) => Answer::Unknown,
            (None, _) => page
                .cloned()
                .or_else(|| self.page(tag))
                .map_or(Answer::Unknown, |page| Answer::of(page.exists)),
        }
    }

    fn open_request(&self) -> Request {
        let Some(prefix) = self.pair.request_prefix() else {
            return Request::none();
        };
        match self.pair.forge {
            Forge::Github => self.github_request(prefix),
            Forge::Gitlab => self.gitlab_request(prefix),
        }
    }

    fn github_request(&self, prefix: &str) -> Request {
        let repo = &self.pair.repo;
        let Some(body) = self.api(&format!(
            "repos/{repo}/pulls?state=open&base={}&per_page=100",
            self.pair.trunk
        )) else {
            return Request::unknown();
        };
        let Some(request) = body.as_array().and_then(|requests| {
            requests.iter().find(|request| {
                request["head"]["ref"]
                    .as_str()
                    .is_some_and(|head| head.starts_with(prefix))
            })
        }) else {
            return Request::none();
        };
        let sha = request["head"]["sha"].as_str().unwrap_or_default();
        let checks = self
            .api(&format!(
                "repos/{repo}/commits/{sha}/check-runs?per_page=100"
            ))
            .map_or(release_state::Checks::Unknown, |body| {
                release_state::github_checks(&body)
            });
        let behind_trunk = self
            .api(&format!("repos/{repo}/compare/{}...{sha}", self.pair.trunk))
            .and_then(|body| body["behind_by"].as_u64())
            .map_or(Answer::Unknown, |behind| Answer::of(behind > 0));
        Request {
            state: "open",
            number: request["number"].as_u64(),
            proposed_version: request["title"]
                .as_str()
                .and_then(release_state::proposed_version),
            armed: Answer::of(!request["auto_merge"].is_null()),
            checks,
            behind_trunk,
        }
    }

    fn gitlab_request(&self, prefix: &str) -> Request {
        let project = self.project();
        let Some(body) = self.api(&format!(
            "projects/{project}/merge_requests?state=opened&target_branch={}&per_page=100",
            self.pair.trunk
        )) else {
            return Request::unknown();
        };
        let Some(request) = body.as_array().and_then(|requests| {
            requests.iter().find(|request| {
                request["source_branch"]
                    .as_str()
                    .is_some_and(|head| head.starts_with(prefix))
            })
        }) else {
            return Request::none();
        };
        let iid = request["iid"].as_u64();
        let detail = iid.and_then(|iid| {
            self.api(&format!(
                "projects/{project}/merge_requests/{iid}?include_diverged_commits_count=true"
            ))
        });
        let armed = request["merge_when_pipeline_succeeds"]
            .as_bool()
            .or_else(|| request["auto_merge_enabled"].as_bool())
            .map_or(Answer::Unknown, Answer::of);
        Request {
            state: "open",
            number: iid,
            proposed_version: request["title"]
                .as_str()
                .and_then(release_state::proposed_version),
            armed,
            checks: detail
                .as_ref()
                .map_or(release_state::Checks::Unknown, |detail| {
                    release_state::gitlab_checks(detail["head_pipeline"]["status"].as_str())
                }),
            behind_trunk: detail
                .as_ref()
                .and_then(|detail| detail["diverged_commits_count"].as_u64())
                .map_or(Answer::Unknown, |behind| Answer::of(behind > 0)),
        }
    }

    /// The newest merged release request: its number and the version its
    /// title proposed.
    fn merged(&self) -> Option<(u64, String)> {
        let prefix = self.pair.request_prefix()?;
        let (path, head, merged_at, number) = match self.pair.forge {
            Forge::Github => (
                format!(
                    "repos/{}/pulls?state=closed&base={}&sort=updated&direction=desc&per_page=30",
                    self.pair.repo, self.pair.trunk
                ),
                "/head/ref",
                "/merged_at",
                "/number",
            ),
            Forge::Gitlab => (
                format!(
                    "projects/{}/merge_requests?state=merged&target_branch={}&order_by=updated_at&sort=desc&per_page=30",
                    self.project(),
                    self.pair.trunk
                ),
                "/source_branch",
                "/merged_at",
                "/iid",
            ),
        };
        let body = self.api(&path)?;
        body.as_array()?
            .iter()
            .filter(|request| {
                request
                    .pointer(head)
                    .and_then(Value::as_str)
                    .is_some_and(|head| head.starts_with(prefix))
            })
            .filter_map(|request| {
                let at = request.pointer(merged_at)?.as_str()?.to_owned();
                let version = release_state::proposed_version(request["title"].as_str()?)?;
                Some((at, request.pointer(number)?.as_u64()?, version))
            })
            .max_by(|left, right| left.0.cmp(&right.0))
            .map(|(_, number, version)| (number, version))
    }

    /// Every tag the forge holds, with the commit it names.
    fn tags(&self) -> Option<Vec<(String, String)>> {
        let (path, commit) = match self.pair.forge {
            Forge::Github => (
                format!("repos/{}/tags?per_page=100", self.pair.repo),
                "/commit/sha",
            ),
            Forge::Gitlab => (
                format!("projects/{}/repository/tags?per_page=100", self.project()),
                "/commit/id",
            ),
        };
        let body = self.api(&path)?;
        Some(
            body.as_array()?
                .iter()
                .filter_map(|tag| {
                    Some((
                        tag["name"].as_str()?.to_owned(),
                        tag.pointer(commit)?.as_str()?.to_owned(),
                    ))
                })
                .collect(),
        )
    }

    /// The release page for a tag, with each artifact's attestation.
    fn page(&self, tag: &str) -> Option<Page> {
        match self.pair.forge {
            Forge::Github => {
                let repo = &self.pair.repo;
                let body = match self.api_answer(&format!("repos/{repo}/releases/tags/{tag}")) {
                    Api::Ok(body) => body,
                    Api::Missing => return Some(Page::default()),
                    Api::Failed(_) => return None,
                };
                let mut page = Page {
                    exists: true,
                    ..Page::default()
                };
                for asset in body["assets"].as_array().into_iter().flatten() {
                    let Some(name) = asset["name"].as_str() else {
                        continue;
                    };
                    page.assets.push(name.to_owned());
                    let Some(digest) = asset["digest"].as_str() else {
                        page.attestation_unknown = true;
                        continue;
                    };
                    match self.api_answer(&format!("repos/{repo}/attestations/{digest}")) {
                        Api::Ok(body)
                            if body["attestations"]
                                .as_array()
                                .is_some_and(|list| !list.is_empty()) => {}
                        Api::Ok(_) | Api::Missing => page.unattested.push(name.to_owned()),
                        Api::Failed(_) => page.attestation_unknown = true,
                    }
                }
                Some(page)
            }
            Forge::Gitlab => {
                let body =
                    match self.api_answer(&format!("projects/{}/releases/{tag}", self.project())) {
                        Api::Ok(body) => body,
                        Api::Missing => return Some(Page::default()),
                        Api::Failed(_) => return None,
                    };
                let names: Vec<String> = body["assets"]["links"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|link| link["name"].as_str().map(str::to_owned))
                    .collect();
                let bundles: Vec<&String> = names
                    .iter()
                    .filter(|name| name.ends_with(".sigstore.json"))
                    .collect();
                let assets: Vec<String> = names
                    .iter()
                    .filter(|name| !name.ends_with(".sigstore.json"))
                    .cloned()
                    .collect();
                let unattested = assets
                    .iter()
                    .filter(|name| name.ends_with(".tar.gz"))
                    .filter(|name| {
                        !bundles
                            .iter()
                            .any(|bundle| **bundle == format!("{name}.sigstore.json"))
                    })
                    .cloned()
                    .collect();
                Some(Page {
                    exists: true,
                    assets,
                    unattested,
                    attestation_unknown: false,
                })
            }
        }
    }

    /// Read the runs on `sha` until the publish run, and the artifact run
    /// where the pair builds one, concluded or the timeout passed.
    fn wait(&self, sha: &str, timeout: u64) -> Wait {
        let interval = std::env::var(WAIT_INTERVAL_VAR)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(WAIT_INTERVAL_MS);
        let deadline = Instant::now() + Duration::from_secs(timeout);
        loop {
            let (publish, artifact) = self.runs(sha);
            let concluded = publish.as_ref().is_some_and(Run::concluded)
                && (!self.pair.artifacts || artifact.as_ref().is_some_and(Run::concluded));
            let failed = [&publish, &artifact]
                .into_iter()
                .flatten()
                .any(|run| run.concluded() && !run.succeeded());
            if concluded || failed || Instant::now() >= deadline {
                let tag = artifact
                    .as_ref()
                    .and_then(|run| run.head_branch.clone())
                    .filter(|branch| branch.starts_with('v'));
                return Wait {
                    sha: sha.to_owned(),
                    timed_out: !concluded && !failed,
                    publish,
                    artifact,
                    tag,
                };
            }
            std::thread::sleep(Duration::from_millis(interval));
        }
    }

    /// The publish run and the artifact run on `sha`.
    fn runs(&self, sha: &str) -> (Option<Run>, Option<Run>) {
        match self.pair.forge {
            Forge::Github => {
                let Some(body) = self.api(&format!(
                    "repos/{}/actions/runs?head_sha={sha}&per_page=100",
                    self.pair.repo
                )) else {
                    return (None, None);
                };
                let publish = self
                    .pair
                    .publish_workflow()
                    .and_then(|workflow| release_state::github_run_on(&body, workflow, sha));
                let artifact = self
                    .pair
                    .artifact_workflow()
                    .and_then(|workflow| release_state::github_run_on(&body, workflow, sha));
                (publish, artifact)
            }
            Forge::Gitlab => {
                let Some(body) = self.api(&format!(
                    "projects/{}/pipelines?sha={sha}&per_page=100",
                    self.project()
                )) else {
                    return (None, None);
                };
                let trunk = self.pair.trunk.clone();
                let publish =
                    release_state::gitlab_pipeline_on(&body, |reference| reference == trunk);
                let artifact = self
                    .pair
                    .artifacts
                    .then(|| {
                        release_state::gitlab_pipeline_on(&body, |reference| {
                            reference.starts_with('v')
                        })
                    })
                    .flatten();
                (publish, artifact)
            }
        }
    }
}

/// The human report, in the order the JSON states it.
fn render(
    out: Output,
    manifest: &Manifest,
    forge: &ForgeState,
    observed: &Observed,
    lines: &[LineRow],
) {
    out.result_line(format!(
        "release: {} style, {} integration, {} checkout, trunk {}",
        manifest
            .profile
            .release
            .style
            .map_or("no", |style| style.as_str()),
        manifest.git.integration.as_str(),
        manifest.git.checkout_mode.as_str(),
        manifest.git.trunk
    ));
    out.result_line(format!(
        "forge: {} {} ({}{})",
        forge.name.as_deref().unwrap_or("none"),
        forge.repo.as_deref().unwrap_or(""),
        forge.read.as_str(),
        forge
            .reason
            .as_deref()
            .map_or_else(String::new, |reason| format!(": {reason}"))
    ));
    let request = &observed.request;
    match request.state {
        "open" => out.result_line(format!(
            "request: #{} proposing {}, armed {}, checks {}, behind the trunk {}",
            request.number.unwrap_or_default(),
            request
                .proposed_version
                .as_deref()
                .unwrap_or("an unread version"),
            request.armed.as_str(),
            request.checks.as_str(),
            request.behind_trunk.as_str()
        )),
        state => out.result_line(format!("request: {state}")),
    }
    let last = &observed.last;
    out.result_line(format!(
        "last release: {}, published {}, artifacts verified {}",
        last.tag.as_deref().unwrap_or("none"),
        last.published.as_str(),
        last.artifacts_verified.as_str()
    ));
    for row in lines {
        out.result_line(format!(
            "line {}: newest release {}",
            row.branch,
            row.newest_release.as_deref().unwrap_or("none")
        ));
    }
    if let Some(wait) = &observed.wait {
        let describe = |run: &Option<Run>| {
            run.as_ref().map_or_else(
                || "not found".to_owned(),
                |run| {
                    format!(
                        "{} {}",
                        run.id,
                        run.conclusion.as_deref().unwrap_or(run.status.as_str())
                    )
                },
            )
        };
        out.result_line(format!(
            "wait {}: publish run {}, artifact run {}{}",
            wait.sha,
            describe(&wait.publish),
            describe(&wait.artifact),
            if wait.timed_out { ", timed out" } else { "" }
        ));
    }
    for symptom in &observed.symptoms {
        out.result_line(format!(
            "SYMPTOM {}: {} (rk method recovery: {})",
            symptom.code, symptom.evidence, symptom.recovery
        ));
    }
    let mut next = Vec::new();
    if !observed.symptoms.is_empty() {
        next.push("rk method recovery owns the path for each symptom".to_owned());
    }
    next.push("rk guide release orders the release".to_owned());
    out.next(&next);
}

#[cfg(test)]
mod tests {
    use super::{ForgeState, Report, Wait};
    use crate::commands::lines::LineRow;
    use crate::landing::manifest::{CheckoutMode, Integration, Style};
    use crate::release_state::{
        Answer, Checks, LastRelease, MERGED_NOTHING_PUBLISHED, Request, Run, Symptom,
    };

    /// The complete `rk.release-status/1` shape, held by snapshot in both
    /// the landed and the absent forms.
    #[test]
    fn the_release_status_schema_snapshot_holds() {
        let landed = Report {
            schema: super::SCHEMA,
            landed: true,
            style: Some(Style::Trunk),
            integration: Some(Integration::Local),
            checkout_mode: Some(CheckoutMode::LinkedWorktree),
            trunk: Some("master".into()),
            forge: Some(ForgeState {
                name: Some("github".into()),
                repo: Some("acme/widget".into()),
                read: Answer::Yes,
                reason: None,
            }),
            request: Some(Request {
                state: "open",
                number: Some(12),
                proposed_version: Some("1.2.3".into()),
                armed: Answer::Yes,
                checks: Checks::Passing,
                behind_trunk: Answer::No,
            }),
            last_release: Some(LastRelease {
                tag: Some("v1.2.2".into()),
                published: Answer::Yes,
                artifacts_verified: Answer::NotApplicable,
            }),
            lines: Some(vec![LineRow {
                line: "1.4".into(),
                branch: "release/1.4".into(),
                presence: "remote",
                newest_release: Some("v1.4.5".into()),
                newest_candidate: None,
                tag_covered: None,
                seat: None,
            }]),
            symptoms: Some(vec![Symptom {
                code: MERGED_NOTHING_PUBLISHED,
                evidence: "release request #11 proposing 1.2.2 merged, and no tag v1.2.2 exists"
                    .into(),
                recovery: "The release request merged and nothing published",
            }]),
            wait: Some(Wait {
                sha: "aaa".into(),
                publish: Some(Run {
                    id: 20,
                    status: "completed".into(),
                    conclusion: Some("success".into()),
                    head_branch: Some("master".into()),
                }),
                artifact: None,
                tag: Some("v1.2.2".into()),
                timed_out: false,
            }),
        };
        assert_eq!(
            serde_json::to_string(&landed).expect("a report serializes"),
            r#"{"schema":"rk.release-status/1","landed":true,"style":"trunk","integration":"local","checkout_mode":"linked-worktree","trunk":"master","forge":{"name":"github","repo":"acme/widget","read":"yes","reason":null},"request":{"state":"open","number":12,"proposed_version":"1.2.3","armed":"yes","checks":"passing","behind_trunk":"no"},"last_release":{"tag":"v1.2.2","published":"yes","artifacts_verified":"not-applicable"},"lines":[{"line":"1.4","branch":"release/1.4","presence":"remote","newest_release":"v1.4.5"}],"symptoms":[{"code":"request-merged-nothing-published","evidence":"release request #11 proposing 1.2.2 merged, and no tag v1.2.2 exists","recovery":"The release request merged and nothing published"}],"wait":{"sha":"aaa","publish":{"id":20,"status":"completed","conclusion":"success","head_branch":"master"},"artifact":null,"tag":"v1.2.2","timed_out":false}}"#
        );
        let absent = Report {
            landed: false,
            style: None,
            integration: None,
            checkout_mode: None,
            trunk: None,
            forge: None,
            request: None,
            last_release: None,
            lines: None,
            symptoms: None,
            wait: None,
            ..landed
        };
        assert_eq!(
            serde_json::to_string(&absent).expect("a report serializes"),
            r#"{"schema":"rk.release-status/1","landed":false}"#
        );
    }
}
