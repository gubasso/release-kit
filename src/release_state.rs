//! Where a landed target's release stands: the release request, the last
//! release, and the symptoms a stuck release shows.
//!
//! The judgments are pure and take forge answers as JSON values, so each
//! rule is testable without a forge. The reads spawn the forge CLI the
//! setup verbs resolve, `gh api` or `glab api`, and the registry reads go
//! through `crate::http`, in-process. A read that fails for any reason other than a definite
//! not-found answers `unknown`, because a credential this runtime cannot
//! reach is an observation boundary and never a state of the release.
//!
//! SATISFIES release-state:a-forge-field-reads-unknown-without-credentials

use std::path::PathBuf;

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

/// The code for a release tag with no complete release page.
pub const TAG_WITHOUT_RELEASE: &str = "tag-without-release";

/// The recovery section for a tag whose release page is missing or
/// incomplete.
const RECOVERY_TAG_WITHOUT_RELEASE: &str = "The tag exists and the release does not";

/// The code for a release tag the clone holds at another commit than the
/// forge.
pub const LOCAL_TAG_DIFFERS: &str = "local-tag-differs";
/// The code for a release tag the clone holds and the forge does not.
pub const LOCAL_TAG_NOT_ON_FORGE: &str = "local-tag-not-on-forge";

/// The recovery section for a clone whose release tag the forge no longer
/// holds at that commit.
const RECOVERY_STALE_LOCAL_TAG: &str = "A clone holds a stale release tag";

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
    /// The prefix every release-line branch carries.
    pub line_prefix: String,
    /// Whether the landing builds artifacts the release page carries.
    pub artifacts: bool,
    /// The package name the registry knows, where the pair has one.
    pub package: Option<String>,
    /// The recorded `profile.release.registry` answer, absent where the
    /// target never answered it.
    pub registry_answer: Option<crate::profile::RegistryAnswer>,
}

impl Pair {
    /// The head branch prefix the driver's release request carries.
    #[must_use]
    pub fn request_prefix(&self) -> Option<&'static str> {
        match self.driver.as_str() {
            "rust" => Some("release-plz-"),
            // The engine keeps one request per base on `rk-release/<base>`.
            "python" | "bash" => Some(crate::release_engine::REQUEST_PREFIX),
            _ => None,
        }
    }

    /// The workflow file that tags and publishes, on GitHub.
    #[must_use]
    pub fn publish_workflow(&self) -> Option<&'static str> {
        match (self.forge, self.driver.as_str()) {
            (Forge::Github, "rust") => Some("release-plz.yml"),
            (Forge::Github, "python" | "bash") => Some("release.yml"),
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
            (Forge::Github, "rust" | "bash" | "python") => Some("release.yml"),
            _ => None,
        }
    }

    /// Whether `branch` is a release base: the trunk, or a release line.
    #[must_use]
    pub fn is_base(&self, branch: &str) -> bool {
        branch == self.trunk || crate::release_engine::is_line(&self.line_prefix, branch)
    }

    /// The registry the driver publishes to, where it has one.
    #[must_use]
    pub fn registry(&self) -> Option<Registry> {
        registry_for(&self.driver, self.registry_answer)
    }
}

/// The registry a release publishes to, from its driver and its answer.
///
/// The one owner of the mapping from a driver and the recorded
/// `profile.release.registry` answer, so the release status and the setup
/// milestones read one registry. A rust release reads crates.io and a
/// python release reads `PyPI`, each unless it answered `none`; every
/// other driver publishes to no registry.
#[must_use]
pub fn registry_for(
    driver: &str,
    answer: Option<crate::profile::RegistryAnswer>,
) -> Option<Registry> {
    use crate::profile::RegistryAnswer;
    match answer.or_else(|| RegistryAnswer::default_for(driver))? {
        RegistryAnswer::CratesIo if driver == "rust" => Some(Registry::CratesIo),
        RegistryAnswer::Pypi if driver == "python" => Some(Registry::Pypi),
        _ => None,
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
    gitlab_pipeline_where(body, |pipeline| {
        pipeline["ref"].as_str().is_some_and(&reference)
    })
}

/// The branch-push pipeline on commit `sha` whose branch `base` accepts.
///
/// It is selected by the commit and the push source, never by recency. A
/// merge-request, schedule, or child pipeline on the same commit runs no
/// release, and neither does the pipeline of the tag the release pushed,
/// whose ref `base` never accepts.
#[must_use]
pub fn gitlab_branch_push_on(body: &Value, sha: &str, base: impl Fn(&str) -> bool) -> Option<Run> {
    gitlab_pipeline_where(body, |pipeline| {
        pipeline["sha"].as_str() == Some(sha)
            && pipeline["source"].as_str() == Some("push")
            && pipeline["ref"].as_str().is_some_and(&base)
    })
}

/// The oldest pipeline `keep` accepts.
fn gitlab_pipeline_where(body: &Value, keep: impl Fn(&Value) -> bool) -> Option<Run> {
    body.as_array()?
        .iter()
        .filter(|pipeline| keep(pipeline))
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

/// The suffix of the provenance bundle a release page carries beside a
/// distribution it signs.
pub const BUNDLE_SUFFIX: &str = ".sigstore.json";

/// Whether a file on a release page is support material and no artifact:
/// the engine's checksum file, or a provenance bundle. Neither counts
/// toward a page's artifacts, and no attestation covers either.
#[must_use]
pub fn is_support(name: &str) -> bool {
    name == crate::release_engine::CHECKSUMS || name.ends_with(BUNDLE_SUFFIX)
}

/// What a release page carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Page {
    /// Whether the page exists.
    pub exists: bool,
    /// Every artifact name on it, support files left out.
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
/// published nothing. Where the tag is absent and the registry holds the
/// version, the evidence says so, because that closes the un-bump. A published release whose pair builds artifacts and
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
            // The registry answers here, because an untagged version on a
            // pair with no registry reads unpublished: the version is out,
            // so the un-bump is closed and the rerun at the same commit
            // stays the path.
            (Answer::No, Answer::Yes) => Some(format!(
                "no tag v{0} exists, and the registry already holds {0}, so the version cannot return to its previous value",
                merged.version
            )),
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

/// SATISFIES release-state:a-symptom-names-its-recovery
///
/// Each release tag newer than the newest complete release of its own
/// line, with no complete page itself. The trunk and each release line
/// keep their own cutoff: a `v<line>.*` tag belongs to that line, and
/// every other release tag to the trunk. Each group is walked newest
/// first and stops at its first complete release, so a tag older than a
/// complete release of its line drops out, and an accepted gap needs no
/// record. `complete` answers whether a tag's page is complete; an
/// unknown answer stops the walk of its line, because the unread page
/// could be the complete release that retires every older gap.
pub fn orphan_tags(
    tags: &[&str],
    lines: &[&str],
    mut complete: impl FnMut(&str) -> Answer,
) -> Vec<Symptom> {
    let line_of = |version: &str| {
        lines
            .iter()
            .find(|line| version.starts_with(&format!("{line}.")))
            .copied()
    };
    let mut groups: std::collections::BTreeMap<Option<&str>, Vec<&str>> =
        std::collections::BTreeMap::new();
    for tag in tags {
        let Some(version) = tag.strip_prefix('v') else {
            continue;
        };
        if !is_version(version) || version.contains('-') {
            continue;
        }
        groups.entry(line_of(version)).or_default().push(tag);
    }
    let mut found = Vec::new();
    for (line, mut group) in groups {
        group.sort_by(|left, right| {
            if version_is_newer(&left[1..], &right[1..]) {
                std::cmp::Ordering::Less
            } else if version_is_newer(&right[1..], &left[1..]) {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
        let place = line.map_or_else(|| "the trunk".to_owned(), |line| format!("the line {line}"));
        for tag in group {
            // A complete page is the cutoff, and an unread page could be
            // one, so either stops the walk of the line.
            if complete(tag) != Answer::No {
                break;
            }
            found.push(Symptom {
                code: TAG_WITHOUT_RELEASE,
                evidence: format!(
                    "{tag} exists on {place} with no complete release page, and no complete release of that line is newer"
                ),
                recovery: RECOVERY_TAG_WITHOUT_RELEASE,
            });
        }
    }
    found
}

/// SATISFIES release-state:a-symptom-names-its-recovery
///
/// Each release tag the clone holds that the forge holds at another
/// commit, or does not hold at all, in tag order. `local` and `forge` pair
/// each tag with the commit it names; a local tag that is no
/// `v<version>`, a candidate included, is skipped. A tag only the forge
/// holds is no symptom: an ordinary fetch brings it. The forge holds the
/// true release tags, because a release tag is derived and never
/// authored, so the evidence ends with the fetch that refreshes the
/// clone: the single-tag form where the commits differ, and the prune
/// form where the forge lacks the tag, because an explicit refspec cannot
/// remove a tag the remote does not hold.
#[must_use]
pub fn local_tag_drift(local: &[(&str, &str)], forge: &[(&str, &str)]) -> Vec<Symptom> {
    let mut local: Vec<(&str, &str)> = local
        .iter()
        .filter(|(tag, _)| tag.strip_prefix('v').is_some_and(is_version))
        .copied()
        .collect();
    local.sort_unstable();
    local
        .into_iter()
        .filter_map(|(tag, commit)| {
            match forge.iter().find(|(name, _)| *name == tag) {
                Some((_, held)) if *held == commit => None,
                Some((_, held)) => Some(Symptom {
                    code: LOCAL_TAG_DIFFERS,
                    evidence: format!(
                        "the clone holds {tag} at {commit} and the forge holds it at {held}; the forge holds the true release tags, so refresh the clone: git fetch origin '+refs/tags/{tag}:refs/tags/{tag}'"
                    ),
                    recovery: RECOVERY_STALE_LOCAL_TAG,
                }),
                None => Some(Symptom {
                    code: LOCAL_TAG_NOT_ON_FORGE,
                    evidence: format!(
                        "the clone holds {tag} at {commit} and the forge holds no such tag; the forge holds the true release tags, so refresh the clone: git fetch --prune origin '+refs/tags/v*:refs/tags/v*'"
                    ),
                    recovery: RECOVERY_STALE_LOCAL_TAG,
                }),
            }
        })
        .collect()
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
/// named: a 200 answers yes, a 404 answers no, and any other answer is
/// unknown.
///
/// # Errors
///
/// The reason where the registry gave no answer within twenty seconds.
pub fn registry_serves(
    registry: Registry,
    package: &str,
    version: Option<&str>,
) -> Result<Answer, String> {
    let url = registry.url(package, version);
    let answer = crate::http::send(&crate::http::Request::get(
        &url,
        std::time::Duration::from_secs(20),
    ))
    .map_err(|reason| format!("{url} gave no answer: {reason}"))?;
    Ok(match answer.status {
        200 => Answer::Yes,
        404 => Answer::No,
        _ => Answer::Unknown,
    })
}

/// Every file the registry serves for `package` at `version`, with its
/// SHA-256.
///
/// `Ok(None)` where the registry answers 404 for the version, and the
/// reason where it answers anything else. It reads through the client
/// [`registry_serves`] reads through.
///
/// # Errors
///
/// The reason where the registry gave no definite answer.
pub fn registry_files(
    registry: Registry,
    package: &str,
    version: &str,
) -> Result<Option<Vec<(String, String)>>, String> {
    let answer = crate::http::send(&crate::http::Request::get(
        &registry.url(package, Some(version)),
        std::time::Duration::from_secs(20),
    ))?;
    match answer.status {
        404 => Ok(None),
        200 => {
            let value: Value = serde_json::from_slice(&answer.body)
                .map_err(|_| "the answer is not JSON".to_owned())?;
            Ok(Some(
                value["urls"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|file| {
                        Some((
                            file["filename"].as_str()?.to_owned(),
                            file["digests"]["sha256"].as_str()?.to_owned(),
                        ))
                    })
                    .collect(),
            ))
        }
        other => Err(format!("the registry answered \"{other}\"")),
    }
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
        local_tag_drift, newest_release, proposed_version, symptoms,
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

    /// With no tag, a version the registry already serves closes the
    /// un-bump, so the evidence says the registry holds it.
    #[test]
    fn an_untagged_version_the_registry_holds_says_so() {
        let merged = Merged {
            number: 12,
            version: "1.2.3".into(),
            tagged: Answer::No,
            published: Answer::Yes,
        };
        let found = symptoms(Some(&merged), &last("v1.2.2", Answer::Yes), None, false);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, super::MERGED_NOTHING_PUBLISHED);
        assert!(
            found[0]
                .evidence
                .contains("the registry already holds 1.2.3"),
            "{}",
            found[0].evidence
        );
        assert_eq!(
            found[0].recovery,
            "The release request merged and nothing published"
        );
        let unpublished = Merged {
            published: Answer::No,
            ..merged
        };
        let found = symptoms(
            Some(&unpublished),
            &last("v1.2.2", Answer::Yes),
            None,
            false,
        );
        assert_eq!(found[0].code, super::MERGED_NOTHING_PUBLISHED);
        assert!(
            !found[0].evidence.contains("registry"),
            "{}",
            found[0].evidence
        );
    }

    #[test]
    fn local_tag_drift_reports_a_moved_and_a_local_only_tag() {
        let found = local_tag_drift(
            &[("v0.3.0", "ccc"), ("v0.1.0", "aaa"), ("v0.2.0", "bbb")],
            &[("v0.1.0", "fff"), ("v0.3.0", "ccc"), ("v0.4.0", "ddd")],
        );
        let codes: Vec<&str> = found.iter().map(|symptom| symptom.code).collect();
        assert_eq!(
            codes,
            [super::LOCAL_TAG_DIFFERS, super::LOCAL_TAG_NOT_ON_FORGE],
            "the moved tag and the local-only tag, in tag order: {found:?}"
        );
        assert!(found[0].evidence.contains("v0.1.0 at aaa"));
        assert!(found[0].evidence.contains("holds it at fff"));
        assert!(found[1].evidence.contains("v0.2.0 at bbb"));
        assert!(
            found
                .iter()
                .all(|symptom| symptom.recovery == "A clone holds a stale release tag")
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
