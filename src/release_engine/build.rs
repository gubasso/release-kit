//! `rk release build`: decide whether this push releases, and build it.
//!
//! The commit decides first, offline: the landing, the baseline, and a
//! push that is no release trigger skip. The forge decides the rest: a
//! version released at another commit skips, a release or a tag at this
//! commit resumes, and a tag at another commit refuses. A release
//! builds through the adapter, compares the rebuilt files with what the
//! registry already serves, and writes its checksums, its notes, and its
//! report beside `dist/`, never into it.

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::release_state::Registry;

use super::adapter::{self, Version};
use super::forge::Client;
use super::guard::{self, Existing, Outcome, Parent, Push};
use super::{BUILD_REPORT, CHECKSUMS, Context, Done, NOTES, OUT_DIR, cliff};

/// The report's shape version.
pub const SCHEMA: &str = "rk.release-build/1";

/// One artifact the build produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Its path, relative to the checkout.
    pub path: String,
    /// Its SHA-256, as lowercase hex.
    pub sha256: String,
}

/// The machine form of `rk release build`, also written to
/// `.rk-release/build.json` for `rk release publish` and the jobs between.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildReport {
    /// The shape version of this document.
    pub schema: String,
    /// `release`, `resume`, or `skip`.
    pub state: String,
    /// Why the push skips, where it does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The version, in the driver's spelling.
    pub version: String,
    /// The tag the version releases as.
    pub tag: String,
    /// Whether the version is a release candidate.
    pub candidate: bool,
    /// The commit the build ran at.
    pub commit: String,
    /// Every artifact, empty under `skip`.
    pub artifacts: Vec<Artifact>,
    /// The checksum file, under a release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksums: Option<String>,
    /// The notes file, under a release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// Decide, build, and report.
///
/// # Errors
///
/// A refusal of a conflicting tag, an unread tag, or a rebuild that
/// differs from the registry; a build failure.
pub fn run(target: &Utf8Path) -> Result<Done<BuildReport>, RkError> {
    let context = Context::load(target)?;
    context.require_tools("build")?;
    cliff::require_pin(target)?;
    let head = context.head()?;
    let version = context.adapter.read_version(target)?;
    let tag = context.adapter.tag_of(version);
    let mut report = BuildReport {
        schema: SCHEMA.to_owned(),
        state: "skip".to_owned(),
        reason: None,
        version: version.spelled(context.driver()),
        tag: tag.clone(),
        candidate: version.is_candidate(),
        commit: head.clone(),
        artifacts: Vec::new(),
        checksums: None,
        notes: None,
    };
    let outcome = decide(&context, &head, version, &tag)?;
    match outcome {
        Outcome::Skip(reason) => {
            report.reason = Some(reason.clone());
            write_report(&context, &report)?;
            return Ok(Done {
                report,
                lines: vec![format!("skip: {reason}")],
                next: Vec::new(),
            });
        }
        Outcome::Refuse(reason) => {
            return Err(super::conflict(reason, super::RECOVERY_TAG));
        }
        Outcome::Release | Outcome::Resume => {}
    }
    outcome.as_str().clone_into(&mut report.state);
    let epoch = super::git_line(
        target,
        &["log", "-1", "--format=%ct", "HEAD"],
        "the commit time",
    )?;
    let built = context.adapter.build(target, &epoch)?;
    let mut artifacts = Vec::new();
    for path in &built {
        artifacts.push(Artifact {
            path: path.to_string(),
            sha256: super::file_sha256(&target.join(path))?,
        });
    }
    if context.registry == Some(Registry::Pypi)
        && let Some(package) = context.package.as_deref()
    {
        compare_with_registry(package, version, &artifacts)?;
    }
    let out = target.join(OUT_DIR);
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(RkError::Io)?;
    }
    std::fs::create_dir_all(&out).map_err(RkError::Io)?;
    let notes = Utf8PathBuf::from(format!("{OUT_DIR}/{NOTES}"));
    cliff::notes(target, &tag, &target.join(&notes))?;
    let checksums = Utf8PathBuf::from(format!("{OUT_DIR}/{CHECKSUMS}"));
    let mut sums = String::new();
    for artifact in &artifacts {
        sums.push_str(&artifact.sha256);
        sums.push_str("  ");
        sums.push_str(&super::file_name(Utf8Path::new(&artifact.path)));
        sums.push('\n');
    }
    std::fs::write(target.join(&checksums), sums).map_err(RkError::Io)?;
    report.artifacts = artifacts;
    report.checksums = Some(checksums.to_string());
    report.notes = Some(notes.to_string());
    write_report(&context, &report)?;
    let mut lines = vec![format!(
        "{}: {tag} at {head}{}",
        report.state,
        if report.candidate {
            ", a release candidate"
        } else {
            ""
        }
    )];
    lines.extend(
        report
            .artifacts
            .iter()
            .map(|artifact| format!("built {} sha256 {}", artifact.path, artifact.sha256)),
    );
    Ok(Done {
        report,
        lines,
        next: vec!["attest the artifacts, then rk release publish".to_owned()],
    })
}

/// The outcome of this push.
fn decide(context: &Context, head: &str, version: Version, tag: &str) -> Result<Outcome, RkError> {
    let push = Push {
        version,
        parent: parent_version(context)?,
        merges_request: true,
    };
    if let Some(reason) = guard::trigger(push) {
        return Ok(Outcome::Skip(reason));
    }
    let Some(base) = context
        .pushed_branch()
        .filter(|branch| context.is_base(branch))
    else {
        return Ok(Outcome::Skip(
            "the push is to neither the trunk nor a release line".to_owned(),
        ));
    };
    let client = Client::resolve(context.forge, &context.target, &context.repo)?;
    if !version.is_candidate() {
        let merges = client.merges_request(head, &base, &super::request_branch(&base))?;
        if let Some(reason) = guard::trigger(Push {
            merges_request: merges,
            ..push
        }) {
            return Ok(Outcome::Skip(reason));
        }
    }
    forge_outcome(&client, head, tag)
}

/// The outcome once the forge answered for the release and the tag of
/// `tag`; `rk release publish` asks it again before it writes.
///
/// # Errors
///
/// A refusal where the release list cannot be read.
pub fn forge_outcome(client: &Client, head: &str, tag: &str) -> Result<Outcome, RkError> {
    let existing = match client.release(tag)? {
        None => Existing::None,
        Some(release) if release.draft => Existing::Draft,
        Some(_) => Existing::Published,
    };
    let at = client.tag(tag);
    Ok(guard::judge(head, tag, existing, &at))
}

/// The version the parent commit carries: `None` where the parent holds
/// no version file, or where the commit has no parent.
fn parent_version(context: &Context) -> Result<Parent, RkError> {
    let (file, _) = adapter::version_file(context.driver()).unwrap_or(("VERSION", None));
    let spec = format!("HEAD^:{file}");
    let output = super::git(&context.target, &["show", &spec])?;
    if !output.status.success() {
        return Ok(Parent::Absent);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(Parent::Carries(
        adapter::version_in(context.driver(), &text)
            .as_deref()
            .and_then(Version::parse),
    ))
}

/// Refuse a rebuilt file whose name the registry already serves for this
/// version with other bytes, before any upload or forge write.
///
/// SATISFIES release-engine:every-step-converges
fn compare_with_registry(
    package: &str,
    version: Version,
    artifacts: &[Artifact],
) -> Result<(), RkError> {
    let spelled = version.spelled("python");
    let served = crate::release_state::registry_files(Registry::Pypi, package, &spelled)
        .map_err(|reason| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::ObservationIncomplete,
                    format!("PyPI did not answer for {package} {spelled}: {reason}"),
                )
                .expected("PyPI's JSON API answering 200 or 404 for the version")
                .action("rerun the job once PyPI answers")
                .target_state("no upload and no forge write ran"),
            )
        })?
        .unwrap_or_default();
    for artifact in artifacts {
        let name = super::file_name(Utf8Path::new(&artifact.path));
        if let Some((_, digest)) = served.iter().find(|(file, _)| *file == name)
            && *digest != artifact.sha256
        {
            return Err(super::conflict(
                format!(
                    "PyPI serves {name} for {package} {spelled} with sha256 {digest}, and this rebuild has {}; yank the version and fix forward",
                    artifact.sha256
                ),
                super::RECOVERY_DEFECTIVE,
            ));
        }
    }
    Ok(())
}

/// Write the report into the output directory, and on GitHub into
/// `$GITHUB_OUTPUT`, where the next steps read it.
fn write_report(context: &Context, report: &BuildReport) -> Result<(), RkError> {
    let out = context.target.join(OUT_DIR);
    std::fs::create_dir_all(&out).map_err(RkError::Io)?;
    let json = serde_json::to_string_pretty(report).map_err(|error| anyhow::anyhow!(error))?;
    std::fs::write(out.join(BUILD_REPORT), format!("{json}\n")).map_err(RkError::Io)?;
    if context.forge == Forge::Github
        && let Some(path) = std::env::var_os("GITHUB_OUTPUT").filter(|path| !path.is_empty())
    {
        use std::io::Write as _;
        let paths: Vec<&str> = report
            .artifacts
            .iter()
            .map(|artifact| artifact.path.as_str())
            .collect();
        let delimiter = format!("rk-release-{}", report.commit);
        let mut block = vec![
            format!("state={}", report.state),
            format!("version={}", report.version),
            format!("tag={}", report.tag),
            format!("candidate={}", report.candidate),
            format!("artifacts<<{delimiter}"),
        ];
        block.extend(paths.iter().map(|path| (*path).to_owned()));
        block.push(delimiter);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(RkError::Io)?;
        for line in block {
            writeln!(file, "{line}").map_err(RkError::Io)?;
        }
    }
    Ok(())
}
