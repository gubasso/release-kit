//! Where a landed target's release stands: the release request, the last
//! release, and the symptoms a stuck release shows.
//!
//! The judgments are pure and take forge answers as JSON values, so each
//! rule is testable without a forge. The reads spawn the forge CLI the
//! setup verbs resolve, `gh api` or `glab api`, and the registry reads
//! spawn `curl`. A read that fails for any reason other than a definite
//! not-found answers `unknown`, because a credential this runtime cannot
//! reach is an observation boundary and never a state of the release.
//!
//! SATISFIES release-state:a-forge-field-reads-unknown-without-credentials

use std::path::{Path, PathBuf};

use camino::Utf8Path;
use serde::Serialize;
use serde_json::Value;

use crate::detect::Forge;
use crate::landing::manifest::version_is_newer;

/// One answer a forge or a registry gives: a field the runtime could not
/// read is `unknown`, and a field the landing makes meaningless is
/// `not-applicable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Answer {
    /// The fact holds.
    Yes,
    /// The fact does not hold.
    No,
    /// The runtime could not read it.
    Unknown,
    /// The landing gives the fact no meaning, such as artifacts on a pair
    /// that builds none.
    NotApplicable,
}

impl Answer {
    /// The word the JSON and the human report use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::No => "no",
            Self::Unknown => "unknown",
            Self::NotApplicable => "not-applicable",
        }
    }

    /// `yes` for true, `no` for false.
    #[must_use]
    pub const fn of(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }
}

/// Where a request's checks stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Checks {
    /// A check is still running, or none has started.
    Pending,
    /// Every check concluded without a failure.
    Passing,
    /// A check concluded with a failure.
    Failing,
    /// The runtime could not read them.
    Unknown,
}

impl Checks {
    /// The word the JSON and the human report use.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Passing => "passing",
            Self::Failing => "failing",
            Self::Unknown => "unknown",
        }
    }
}

/// The open release request, as the forge reports it.
#[derive(Debug, Clone, Serialize)]
pub struct Request {
    /// `none`, `open`, or `unknown`.
    pub state: &'static str,
    /// The request's number on the forge.
    pub number: Option<u64>,
    /// The version its title proposes.
    pub proposed_version: Option<String>,
    /// Whether the request merges itself once its checks pass.
    pub armed: Answer,
    /// Where its checks stand.
    pub checks: Checks,
    /// Whether the trunk moved past the commit the request was cut from.
    pub behind_trunk: Answer,
}

impl Request {
    /// A request the runtime could not read.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            state: "unknown",
            number: None,
            proposed_version: None,
            armed: Answer::Unknown,
            checks: Checks::Unknown,
            behind_trunk: Answer::Unknown,
        }
    }

    /// No open release request.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            state: "none",
            number: None,
            proposed_version: None,
            armed: Answer::NotApplicable,
            checks: Checks::Unknown,
            behind_trunk: Answer::NotApplicable,
        }
    }
}

/// The newest release and whether it reached its consumers.
#[derive(Debug, Clone, Serialize)]
pub struct LastRelease {
    /// The newest release tag, absent while none exists.
    pub tag: Option<String>,
    /// Whether the registry, or the forge where the pair has no registry,
    /// serves the version.
    pub published: Answer,
    /// Whether the release page carries its artifacts, each attested.
    pub artifacts_verified: Answer,
}

/// One symptom a release shows, with the recovery section that owns it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Symptom {
    /// The stable code a reader branches on.
    pub code: &'static str,
    /// What was observed, one line.
    pub evidence: String,
    /// The `rk method recovery` section that owns the path.
    pub recovery: &'static str,
}

/// The code for a merged release request that published nothing.
pub const MERGED_NOTHING_PUBLISHED: &str = "request-merged-nothing-published";
/// The code for a published version whose release page has no artifact.
pub const PUBLISHED_NO_ARTIFACTS: &str = "published-no-artifacts";
/// The code for an artifact no attestation covers.
pub const ARTIFACT_WITHOUT_ATTESTATION: &str = "artifact-without-attestation";

/// The recovery section for a merged request that published nothing.
const RECOVERY_NOTHING_PUBLISHED: &str = "The release request merged and nothing published";
/// The recovery section for an artifact build that left nothing, or
/// nothing attested.
const RECOVERY_ARTIFACT_BUILD: &str = "The artifact build failed";

/// What the landed pair releases through, from the record.
#[derive(Debug, Clone)]
pub struct Pair {
    /// The release driver.
    pub driver: String,
    /// The forge.
    pub forge: Forge,
    /// The project path on the forge.
    pub repo: String,
    /// The trunk branch.
    pub trunk: String,
    /// Whether the landing builds artifacts the release page carries.
    pub artifacts: bool,
    /// The package name the registry knows, where the pair has one.
    pub package: Option<String>,
}

impl Pair {
    /// The head branch prefix the driver's release request carries.
    #[must_use]
    pub fn request_prefix(&self) -> Option<&'static str> {
        match self.driver.as_str() {
            "rust" => Some("release-plz-"),
            "python" => Some("release-please--"),
            "bash" => Some("chore/release-v"),
            _ => None,
        }
    }

    /// The workflow file that tags and publishes, on GitHub.
    #[must_use]
    pub fn publish_workflow(&self) -> Option<&'static str> {
        match (self.forge, self.driver.as_str()) {
            (Forge::Github, "rust") => Some("release-plz.yml"),
            (Forge::Github, "python") => Some("release-please.yml"),
            (Forge::Github, "bash") => Some("release.yml"),
            _ => None,
        }
    }

    /// The workflow file that builds the artifacts, on GitHub, where the
    /// landing builds any.
    #[must_use]
    pub fn artifact_workflow(&self) -> Option<&'static str> {
        if !self.artifacts {
            return None;
        }
        match (self.forge, self.driver.as_str()) {
            (Forge::Github, "rust" | "bash") => Some("release.yml"),
            _ => None,
        }
    }

    /// The registry the driver publishes to, where it has one.
    #[must_use]
    pub fn registry(&self) -> Option<Registry> {
        match self.driver.as_str() {
            "rust" => Some(Registry::CratesIo),
            "python" => Some(Registry::Pypi),
            _ => None,
        }
    }
}

/// A registry with a public read-back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registry {
    /// crates.io.
    CratesIo,
    /// The Python Package Index.
    Pypi,
}

impl Registry {
    /// The public URL that answers 200 for a published version and 404
    /// for an unpublished one; with no version, for the package itself.
    #[must_use]
    pub fn url(self, package: &str, version: Option<&str>) -> String {
        match (self, version) {
            (Self::CratesIo, Some(version)) => {
                format!("https://crates.io/api/v1/crates/{package}/{version}")
            }
            (Self::CratesIo, None) => format!("https://crates.io/api/v1/crates/{package}"),
            (Self::Pypi, Some(version)) => {
                format!("https://pypi.org/pypi/{package}/{version}/json")
            }
            (Self::Pypi, None) => format!("https://pypi.org/pypi/{package}/json"),
        }
    }
}

/// The version a release request's title proposes: its last
/// whitespace-separated token that reads as a version, with any leading
/// `v` dropped.
#[must_use]
pub fn proposed_version(title: &str) -> Option<String> {
    title
        .split_whitespace()
        .rev()
        .map(|token| token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.'))
        .map(|token| token.strip_prefix('v').unwrap_or(token))
        .find(|token| is_version(token))
        .map(str::to_owned)
}

/// Whether `text` reads as `<major>.<minor>.<patch>`, with an optional
/// pre-release suffix.
#[must_use]
pub fn is_version(text: &str) -> bool {
    let core = text.split_once('-').map_or(text, |(core, _)| core);
    let parts: Vec<&str> = core.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// The newest release among `tags`: a `v<major>.<minor>.<patch>` tag with
/// no pre-release suffix, by version precedence.
#[must_use]
pub fn newest_release<'a>(tags: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let mut newest: Option<&str> = None;
    for tag in tags {
        let Some(version) = tag.strip_prefix('v') else {
            continue;
        };
        if !is_version(version) || version.contains('-') {
            continue;
        }
        if newest.is_none_or(|current| version_is_newer(version, &current[1..])) {
            newest = Some(tag);
        }
    }
    newest.map(str::to_owned)
}

/// Where a list of GitHub check runs stands.
#[must_use]
pub fn github_checks(body: &Value) -> Checks {
    let Some(runs) = body["check_runs"].as_array() else {
        return Checks::Unknown;
    };
    if runs.is_empty() {
        return Checks::Pending;
    }
    let failed = runs.iter().any(|run| {
        matches!(
            run["conclusion"].as_str(),
            Some(
                "failure"
                    | "cancelled"
                    | "timed_out"
                    | "action_required"
                    | "startup_failure"
                    | "stale"
            )
        )
    });
    if failed {
        return Checks::Failing;
    }
    if runs
        .iter()
        .any(|run| run["status"].as_str() != Some("completed"))
    {
        return Checks::Pending;
    }
    Checks::Passing
}

/// Where a GitLab pipeline status stands.
#[must_use]
pub fn gitlab_checks(status: Option<&str>) -> Checks {
    match status {
        Some("success") => Checks::Passing,
        Some("failed" | "canceled" | "canceling") => Checks::Failing,
        Some(_) | None => Checks::Pending,
    }
}

/// Whether a run or a pipeline finished, and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Run {
    /// The forge's id for it.
    pub id: u64,
    /// `queued`, `in_progress`, or `completed` on GitHub; the pipeline
    /// status on GitLab.
    pub status: String,
    /// How it concluded, once it did.
    pub conclusion: Option<String>,
    /// The branch or tag it ran on.
    pub head_branch: Option<String>,
}

impl Run {
    /// Whether it finished.
    #[must_use]
    pub const fn concluded(&self) -> bool {
        self.conclusion.is_some()
    }

    /// Whether it finished with success.
    #[must_use]
    pub fn succeeded(&self) -> bool {
        self.conclusion.as_deref() == Some("success")
    }
}

/// The run of `workflow` on commit `sha`, selected by the commit and the
/// push event, never by recency: a workflow that also reports on requests
/// makes the newest run the wrong one.
#[must_use]
pub fn github_run_on(body: &Value, workflow: &str, sha: &str) -> Option<Run> {
    let suffix = format!("/{workflow}");
    body["workflow_runs"]
        .as_array()?
        .iter()
        .filter(|run| run["head_sha"].as_str() == Some(sha))
        .filter(|run| run["event"].as_str() == Some("push"))
        .filter(|run| {
            run["path"]
                .as_str()
                .is_some_and(|path| path.ends_with(&suffix) || path == workflow)
        })
        .map(|run| Run {
            id: run["id"].as_u64().unwrap_or_default(),
            status: run["status"].as_str().unwrap_or("unknown").to_owned(),
            conclusion: (run["status"].as_str() == Some("completed"))
                .then(|| run["conclusion"].as_str().map(str::to_owned))
                .flatten(),
            head_branch: run["head_branch"].as_str().map(str::to_owned),
        })
        .min_by_key(|run| run.id)
}

/// The pipeline on commit `sha` that ran on `reference`.
#[must_use]
pub fn gitlab_pipeline_on(body: &Value, reference: impl Fn(&str) -> bool) -> Option<Run> {
    body.as_array()?
        .iter()
        .filter(|pipeline| pipeline["ref"].as_str().is_some_and(&reference))
        .map(|pipeline| {
            let status = pipeline["status"].as_str().unwrap_or("unknown").to_owned();
            let conclusion = matches!(
                status.as_str(),
                "success" | "failed" | "canceled" | "skipped"
            )
            .then(|| status.clone());
            Run {
                id: pipeline["id"].as_u64().unwrap_or_default(),
                status,
                conclusion,
                head_branch: pipeline["ref"].as_str().map(str::to_owned),
            }
        })
        .min_by_key(|run| run.id)
}

/// What a release page carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    /// Whether the page exists.
    pub exists: bool,
    /// Every artifact name on it.
    pub assets: Vec<String>,
    /// The artifacts no attestation covers.
    pub unattested: Vec<String>,
    /// Whether any attestation answer could not be read.
    pub attestation_unknown: bool,
}

impl Page {
    /// Whether the page carries its artifacts, each attested.
    #[must_use]
    pub const fn verified(&self) -> Answer {
        if !self.exists || self.assets.is_empty() || !self.unattested.is_empty() {
            return Answer::No;
        }
        if self.attestation_unknown {
            return Answer::Unknown;
        }
        Answer::Yes
    }
}

/// The newest merged release request, and what followed it.
#[derive(Debug, Clone)]
pub struct Merged {
    /// Its number.
    pub number: u64,
    /// The version its title proposed.
    pub version: String,
    /// Whether the tag for that version exists at the forge.
    pub tagged: Answer,
    /// Whether the version is published.
    pub published: Answer,
}

/// Every symptom the observed state shows. Pure: the reads gather the
/// facts, and this rule alone decides what they mean.
///
/// SATISFIES release-state:a-symptom-names-its-recovery
///
/// A merged request whose version has no tag, or no published version,
/// published nothing. A published release whose pair builds artifacts and
/// whose page carries none lost its artifact build. An artifact no
/// attestation covers was uploaded outside the run that signs.
#[must_use]
pub fn symptoms(
    merged: Option<&Merged>,
    last: &LastRelease,
    page: Option<&Page>,
    artifacts: bool,
) -> Vec<Symptom> {
    let mut found = Vec::new();
    if let Some(merged) = merged {
        let missing = match (merged.tagged, merged.published) {
            (Answer::No, _) => Some(format!("no tag v{} exists", merged.version)),
            (_, Answer::No) => Some(format!("{} is not published", merged.version)),
            _ => None,
        };
        if let Some(missing) = missing {
            found.push(Symptom {
                code: MERGED_NOTHING_PUBLISHED,
                evidence: format!(
                    "release request #{} proposing {} merged, and {missing}",
                    merged.number, merged.version
                ),
                recovery: RECOVERY_NOTHING_PUBLISHED,
            });
        }
    }
    if let (true, Some(tag), Answer::Yes, Some(page)) =
        (artifacts, last.tag.as_deref(), last.published, page)
    {
        if page.assets.is_empty() {
            found.push(Symptom {
                code: PUBLISHED_NO_ARTIFACTS,
                evidence: format!("{tag} is published and its release page carries no artifact"),
                recovery: RECOVERY_ARTIFACT_BUILD,
            });
        } else if !page.unattested.is_empty() {
            found.push(Symptom {
                code: ARTIFACT_WITHOUT_ATTESTATION,
                evidence: format!(
                    "{tag} carries {} with no attestation",
                    page.unattested.join(", ")
                ),
                recovery: RECOVERY_ARTIFACT_BUILD,
            });
        }
    }
    found
}

/// One read-only forge API answer.
#[derive(Debug)]
pub enum Api {
    /// The call succeeded and parsed.
    Ok(Value),
    /// The forge answered 404.
    Missing,
    /// The call failed for another reason, with the CLI's own words.
    Failed(String),
}

/// The forge CLI, resolved once.
#[derive(Debug, Clone)]
pub struct ForgeCli {
    cli: PathBuf,
    cwd: PathBuf,
}

impl ForgeCli {
    /// A CLI already resolved, run from `cwd`.
    #[must_use]
    pub const fn new(cli: PathBuf, cwd: PathBuf) -> Self {
        Self { cli, cwd }
    }

    /// The CLI for `forge`, run from `target`.
    ///
    /// # Errors
    ///
    /// The resolution's refusal where no usable binary resolves.
    pub fn resolve(forge: Forge, target: &Utf8Path) -> Result<Self, crate::error::RkError> {
        Ok(Self {
            cli: crate::setup::context::resolve_cli(forge)?,
            cwd: target.as_std_path().to_path_buf(),
        })
    }

    /// One read-only `api` call.
    #[must_use]
    pub fn api(&self, path: &str) -> Api {
        let answered = std::process::Command::new(&self.cli)
            .args(["api", path])
            .current_dir(&self.cwd)
            .env("GH_PAGER", "")
            .env("GLAB_PAGER", "")
            .env("NO_COLOR", "1")
            .output();
        let output = match answered {
            Ok(output) => output,
            Err(source) => return Api::Failed(format!("the forge CLI did not run: {source}")),
        };
        if output.status.success() {
            return serde_json::from_slice::<Value>(&output.stdout).map_or_else(
                |_| Api::Failed("the forge answer did not parse as JSON".to_owned()),
                Api::Ok,
            );
        }
        // `gh` renders "HTTP 404", `glab` "404 Not Found"; a bare
        // substring would read an outage message mentioning 404 as one.
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("HTTP 404") || stderr.contains("404 Not Found") {
            return Api::Missing;
        }
        Api::Failed(
            stderr
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("no output")
                .to_owned(),
        )
    }
}

/// Whether the registry serves `package`, at `version` where one is
/// named: a 200 answers yes, a 404 answers no, and anything else is
/// unknown.
#[must_use]
pub fn registry_serves(registry: Registry, package: &str, version: Option<&str>) -> Answer {
    let curl = std::env::var_os("RK_CURL_BIN").unwrap_or_else(|| "curl".into());
    let answered = std::process::Command::new(curl)
        .args([
            "-sS",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "--max-time",
            "20",
        ])
        .args(["-A", concat!("release-kit/", env!("CARGO_PKG_VERSION"))])
        .arg(registry.url(package, version))
        .output();
    match answered {
        Ok(output) => match String::from_utf8_lossy(&output.stdout).trim() {
            "200" => Answer::Yes,
            "404" => Answer::No,
            _ => Answer::Unknown,
        },
        Err(_) => Answer::Unknown,
    }
}

/// The package name the registry knows, read from the target's own
/// version file: `[package].name` in `Cargo.toml`, `[project].name` in
/// `pyproject.toml`.
#[must_use]
pub fn package_name(target: &Path, driver: &str) -> Option<String> {
    let (file, table) = match driver {
        "rust" => ("Cargo.toml", "package"),
        "python" => ("pyproject.toml", "project"),
        _ => return None,
    };
    let text = std::fs::read_to_string(target.join(file)).ok()?;
    let value: toml::Table = toml::from_str(&text).ok()?;
    value.get(table)?.get("name")?.as_str().map(str::to_owned)
}

/// The GitLab project id a path names, URL-encoded once.
#[must_use]
pub fn gitlab_project(repo: &str) -> String {
    repo.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        Answer, Checks, LastRelease, Merged, Page, github_checks, github_run_on, gitlab_checks,
        newest_release, proposed_version, symptoms,
    };

    #[test]
    fn a_title_proposes_its_last_version_token() {
        assert_eq!(
            proposed_version("chore: release v0.8.7").as_deref(),
            Some("0.8.7")
        );
        assert_eq!(
            proposed_version("chore(main): release 1.2.3").as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            proposed_version("chore(release): v2.0.0-rc.1").as_deref(),
            Some("2.0.0-rc.1")
        );
        assert_eq!(proposed_version("chore: release"), None);
    }

    #[test]
    fn the_newest_release_skips_candidates_and_other_tags() {
        assert_eq!(
            newest_release(["v0.9.0", "v0.10.0", "v0.11.0-rc.1", "latest", "v1.2"]).as_deref(),
            Some("v0.10.0")
        );
        assert_eq!(newest_release(["nightly"]), None);
    }

    #[test]
    fn check_runs_judge_failing_before_pending() {
        assert_eq!(github_checks(&json!({"check_runs": []})), Checks::Pending);
        assert_eq!(
            github_checks(&json!({"check_runs": [
                {"status": "completed", "conclusion": "success"},
                {"status": "in_progress", "conclusion": null}
            ]})),
            Checks::Pending
        );
        assert_eq!(
            github_checks(&json!({"check_runs": [
                {"status": "completed", "conclusion": "failure"},
                {"status": "in_progress", "conclusion": null}
            ]})),
            Checks::Failing
        );
        assert_eq!(
            github_checks(&json!({"check_runs": [
                {"status": "completed", "conclusion": "success"},
                {"status": "completed", "conclusion": "skipped"}
            ]})),
            Checks::Passing
        );
        assert_eq!(github_checks(&json!({})), Checks::Unknown);
        assert_eq!(gitlab_checks(Some("running")), Checks::Pending);
        assert_eq!(gitlab_checks(Some("failed")), Checks::Failing);
        assert_eq!(gitlab_checks(Some("success")), Checks::Passing);
    }

    /// The run is picked by its commit and its push event: a newer run on
    /// a request, the same workflow, never answers for the commit.
    #[test]
    fn a_run_is_selected_by_its_commit_never_by_recency() {
        let body = json!({"workflow_runs": [
            {"id": 30, "head_sha": "bbb", "event": "pull_request", "status": "in_progress",
             "conclusion": null, "path": ".github/workflows/release-plz.yml", "head_branch": "feat/x"},
            {"id": 20, "head_sha": "aaa", "event": "push", "status": "completed",
             "conclusion": "success", "path": ".github/workflows/release-plz.yml", "head_branch": "master"},
            {"id": 21, "head_sha": "aaa", "event": "push", "status": "queued",
             "conclusion": null, "path": ".github/workflows/release.yml", "head_branch": "v1.0.0"}
        ]});
        let publish = github_run_on(&body, "release-plz.yml", "aaa").expect("the publish run");
        assert_eq!(publish.id, 20);
        assert!(publish.succeeded());
        let artifact = github_run_on(&body, "release.yml", "aaa").expect("the artifact run");
        assert_eq!(artifact.id, 21);
        assert!(!artifact.concluded());
        assert_eq!(github_run_on(&body, "release-plz.yml", "bbb"), None);
    }

    fn last(tag: &str, published: Answer) -> LastRelease {
        LastRelease {
            tag: Some(tag.to_owned()),
            published,
            artifacts_verified: Answer::Unknown,
        }
    }

    #[test]
    fn each_symptom_has_its_rule() {
        let merged = Merged {
            number: 12,
            version: "1.2.3".into(),
            tagged: Answer::No,
            published: Answer::Unknown,
        };
        let found = symptoms(Some(&merged), &last("v1.2.2", Answer::Yes), None, false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, super::MERGED_NOTHING_PUBLISHED);
        assert_eq!(
            found[0].recovery,
            "The release request merged and nothing published"
        );
        let unpublished = Merged {
            tagged: Answer::Yes,
            published: Answer::No,
            ..merged.clone()
        };
        assert_eq!(
            symptoms(Some(&unpublished), &last("v1.2.3", Answer::No), None, false)[0].code,
            super::MERGED_NOTHING_PUBLISHED
        );
        let shipped = Merged {
            tagged: Answer::Yes,
            published: Answer::Yes,
            ..merged
        };
        let empty = Page {
            exists: true,
            ..Page::default()
        };
        let found = symptoms(
            Some(&shipped),
            &last("v1.2.3", Answer::Yes),
            Some(&empty),
            true,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, super::PUBLISHED_NO_ARTIFACTS);
        assert_eq!(found[0].recovery, "The artifact build failed");
        let unattested = Page {
            exists: true,
            assets: vec!["widget.tar.gz".into()],
            unattested: vec!["widget.tar.gz".into()],
            attestation_unknown: false,
        };
        let found = symptoms(None, &last("v1.2.3", Answer::Yes), Some(&unattested), true);
        assert_eq!(found[0].code, super::ARTIFACT_WITHOUT_ATTESTATION);
        assert!(found[0].evidence.contains("widget.tar.gz"));
        assert!(
            symptoms(None, &last("v1.2.3", Answer::Yes), Some(&empty), false).is_empty(),
            "a pair that builds no artifact has no artifact symptom"
        );
    }

    #[test]
    fn the_answer_words_are_the_wire_form() {
        for (answer, word) in [
            (Answer::Yes, "yes"),
            (Answer::No, "no"),
            (Answer::Unknown, "unknown"),
            (Answer::NotApplicable, "not-applicable"),
        ] {
            assert_eq!(answer.as_str(), word);
            assert_eq!(
                serde_json::to_string(&answer).expect("serializes"),
                format!("\"{word}\"")
            );
        }
        assert_eq!(Answer::of(true), Answer::Yes);
    }
}
