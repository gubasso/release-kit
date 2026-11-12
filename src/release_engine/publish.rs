//! `rk release publish`: put the built and attested release on the forge,
//! and create the tag last.
//!
//! It reads the build report, checks the tag again, because a rerun can
//! start later than its build, and converges with what the forge already
//! holds: it skips what exists with the same bytes, creates what is
//! missing, and refuses what exists with other bytes, except on a GitHub
//! draft, which nothing public reads and which it repairs.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

use super::adapter::Version;
use super::build::{BuildReport, forge_outcome};
use super::forge::{Asset, Client};
use super::guard::Outcome;
use super::{BUILD_REPORT, CHECKSUMS, Context, Done, OUT_DIR};

/// The report's shape version.
pub const SCHEMA: &str = "rk.release-publish/1";

/// The machine form of `rk release publish`.
#[derive(Debug, Default, Serialize)]
pub struct PublishReport {
    /// The shape version of this document.
    pub schema: &'static str,
    /// `published`, `resumed`, or `skip`.
    pub state: &'static str,
    /// Why nothing was published, where nothing was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The tag the release carries.
    pub tag: String,
    /// Whether the release is a candidate.
    pub candidate: bool,
    /// Each file uploaded where none was.
    pub uploaded: Vec<String>,
    /// Each draft asset replaced because its bytes differed.
    pub replaced: Vec<String>,
    /// Each file already present with the same bytes.
    pub kept: Vec<String>,
    /// Each link added to a GitLab release.
    pub linked: Vec<String>,
}

/// One local file the release carries, with its SHA-256.
struct Local {
    path: Utf8PathBuf,
    name: String,
    sha256: String,
}

/// Publish the release the build report describes.
///
/// # Errors
///
/// A refusal of a missing or foreign build report, of a conflicting tag,
/// or of an asset whose bytes differ from what shipped; a forge failure.
pub fn run(target: &Utf8Path) -> Result<Done<PublishReport>, RkError> {
    let context = Context::load(target)?;
    let build = read_build(target)?;
    let mut report = PublishReport {
        schema: SCHEMA,
        state: "skip",
        tag: build.tag.clone(),
        candidate: build.candidate,
        ..PublishReport::default()
    };
    if build.state == "skip" {
        report.reason.clone_from(&build.reason);
        return Ok(Done {
            report,
            lines: vec![format!(
                "skip: {}",
                build
                    .reason
                    .as_deref()
                    .unwrap_or("the build released nothing")
            )],
            next: Vec::new(),
        });
    }
    let head = context.head()?;
    if head != build.commit {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "the build report is of {}, and the checkout stands at {head}",
                    build.commit
                ),
            )
            .expected("the build report of this commit")
            .target_state("unchanged"),
        ));
    }
    let client = Client::resolve(context.forge, target, &context.repo)?;
    // SATISFIES release-engine:a-version-releases-once
    match forge_outcome(&client, &head, &build.tag)? {
        Outcome::Skip(reason) => {
            report.reason = Some(reason.clone());
            return Ok(Done {
                report,
                lines: vec![format!("skip: {reason}")],
                next: Vec::new(),
            });
        }
        Outcome::Refuse(reason) => return Err(super::conflict(reason, super::RECOVERY_TAG)),
        Outcome::Release | Outcome::Resume => {}
    }
    let locals = locals(target, &build)?;
    let notes = build
        .notes
        .as_deref()
        .and_then(|notes| std::fs::read_to_string(target.join(notes)).ok())
        .unwrap_or_default();
    let resumed = match context.forge {
        Forge::Github => github(&client, target, &head, &build, &notes, &locals, &mut report)?,
        Forge::Gitlab => gitlab(&client, target, &head, &build, &notes, &locals, &mut report)?,
    };
    report.state = if resumed { "resumed" } else { "published" };
    let mut lines = vec![format!(
        "{} {}{} at {head}",
        report.state,
        build.tag,
        if build.candidate {
            " as a prerelease"
        } else {
            ""
        }
    )];
    for (verb, names) in [
        ("uploaded", &report.uploaded),
        ("replaced", &report.replaced),
        ("kept", &report.kept),
        ("linked", &report.linked),
    ] {
        if !names.is_empty() {
            lines.push(format!("{verb} {}", names.join(", ")));
        }
    }
    Ok(Done {
        report,
        lines,
        next: vec!["rk release status --wait <sha> reports the release".to_owned()],
    })
}

/// The build report `rk release build` wrote for this checkout.
fn read_build(target: &Utf8Path) -> Result<BuildReport, RkError> {
    let report_path = target.join(OUT_DIR).join(BUILD_REPORT);
    std::fs::read_to_string(&report_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .filter(|report: &BuildReport| report.schema == super::build::SCHEMA)
        .ok_or_else(|| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!("no rk.release-build/1 report at {report_path}"),
                )
                .expected("the report rk release build writes, carried to this job")
                .action("run rk release build first, and carry .rk-release/ to this job")
                .target_state("unchanged"),
            )
        })
}

/// Every file the release carries: each artifact, unchanged since the
/// build, the checksum file, and each provenance bundle beside an
/// artifact.
fn locals(target: &Utf8Path, build: &BuildReport) -> Result<Vec<Local>, RkError> {
    let mut locals = Vec::new();
    for artifact in &build.artifacts {
        let path = Utf8PathBuf::from(&artifact.path);
        let sha256 = super::file_sha256(&target.join(&path))?;
        if sha256 != artifact.sha256 {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!("{} changed since the build reported it", artifact.path),
                )
                .expected("the artifacts the build reported, carried unchanged")
                .target_state("unchanged"),
            ));
        }
        locals.push(Local {
            name: super::file_name(&path),
            path,
            sha256,
        });
    }
    let mut extra: Vec<Utf8PathBuf> = vec![Utf8PathBuf::from(format!("{OUT_DIR}/{CHECKSUMS}"))];
    extra.extend(
        build
            .artifacts
            .iter()
            .map(|artifact| Utf8PathBuf::from(format!("{}{BUNDLE}", artifact.path))),
    );
    for path in extra {
        if target.join(&path).is_file() {
            locals.push(Local {
                name: super::file_name(&path),
                sha256: super::file_sha256(&target.join(&path))?,
                path,
            });
        }
    }
    Ok(locals)
}

/// The suffix of the provenance bundle the attest step writes beside an
/// artifact.
const BUNDLE: &str = ".sigstore.json";

/// Sort the local files against what the forge holds: the missing, the
/// differing, and the same.
///
/// Where `keep_bundles` holds, a held provenance bundle counts as the same
/// when the artifact beside it is held with this build's bytes: each
/// attestation run signs new bundle bytes for the same artifact, so a rerun
/// keeps the bundle that shipped. The publish verifies no signature.
fn sort<'a>(
    locals: &'a [Local],
    held: &[Asset],
    keep_bundles: bool,
) -> (Vec<&'a Local>, Vec<&'a Local>, Vec<&'a Local>) {
    let (mut missing, mut differing, mut same) = (Vec::new(), Vec::new(), Vec::new());
    for local in locals {
        match held.iter().rev().find(|asset| asset.name == local.name) {
            None => missing.push(local),
            Some(asset) if asset.sha256.as_deref() == Some(local.sha256.as_str()) => {
                same.push(local);
            }
            Some(_) => differing.push(local),
        }
    }
    if keep_bundles {
        // SATISFIES release-engine:every-step-converges
        let (kept, rest): (Vec<&Local>, Vec<&Local>) = differing.into_iter().partition(|local| {
            local
                .name
                .strip_suffix(BUNDLE)
                .is_some_and(|artifact| same.iter().any(|held: &&Local| held.name == artifact))
        });
        same.extend(kept);
        differing = rest;
    }
    (missing, differing, same)
}

fn names(locals: &[&Local]) -> Vec<String> {
    locals.iter().map(|local| local.name.clone()).collect()
}

fn paths(target: &Utf8Path, locals: &[&Local]) -> Vec<Utf8PathBuf> {
    locals
        .iter()
        .map(|local| target.join(&local.path))
        .collect()
}

/// Refuse a file the forge holds with other bytes.
fn refuse_differing(differing: &[&Local], tag: &str, place: &str) -> Result<(), RkError> {
    if differing.is_empty() {
        return Ok(());
    }
    Err(super::conflict(
        format!(
            "{place} of {tag} holds {} with other bytes than this build",
            names(differing).join(", ")
        ),
        super::RECOVERY_DEFECTIVE,
    ))
}

/// GitHub: a draft at the commit, every asset, then the publish, which
/// creates the tag. Answers whether a release already existed.
fn github(
    client: &Client,
    target: &Utf8Path,
    head: &str,
    build: &BuildReport,
    notes: &str,
    locals: &[Local],
    report: &mut PublishReport,
) -> Result<bool, RkError> {
    let tag = &build.tag;
    let existing = client.release(tag)?;
    // SATISFIES release-engine:the-tag-is-created-last
    let Some(release) = existing else {
        let id = client.create_draft(tag, head, notes, build.candidate)?;
        let all: Vec<&Local> = locals.iter().collect();
        client.upload(tag, &paths(target, &all), false)?;
        client.publish_draft(&id, head, build.candidate)?;
        report.uploaded = names(&all);
        return Ok(false);
    };
    let (missing, differing, same) = sort(locals, &release.assets, !release.draft);
    report.kept = names(&same);
    // SATISFIES release-engine:every-step-converges
    if release.draft {
        client.upload(tag, &paths(target, &missing), false)?;
        client.upload(tag, &paths(target, &differing), true)?;
        client.publish_draft(&release.id, head, build.candidate)?;
        report.uploaded = names(&missing);
        report.replaced = names(&differing);
        return Ok(true);
    }
    refuse_differing(&differing, tag, "the published release")?;
    client.upload(tag, &paths(target, &missing), false)?;
    report.uploaded = names(&missing);
    Ok(true)
}

/// GitLab: every package file first, then one release create with every
/// link, which creates the tag. Answers whether a release or a package
/// file already existed.
fn gitlab(
    client: &Client,
    target: &Utf8Path,
    head: &str,
    build: &BuildReport,
    notes: &str,
    locals: &[Local],
    report: &mut PublishReport,
) -> Result<bool, RkError> {
    let tag = &build.tag;
    let version = Version::from_tag(tag).map_or_else(
        || tag.trim_start_matches('v').to_owned(),
        |v| v.spelled("semver"),
    );
    let stored = client.package_files(&version)?;
    let (missing, differing, same) = sort(locals, &stored, true);
    refuse_differing(&differing, tag, "the package registry")?;
    // SATISFIES release-engine:every-step-converges
    for local in &missing {
        client.upload_package(&version, &target.join(&local.path))?;
    }
    report.uploaded = names(&missing);
    report.kept = names(&same);
    let links: Vec<(String, String)> = locals
        .iter()
        .map(|local| {
            (
                local.name.clone(),
                client.package_url(&version, &local.name),
            )
        })
        .collect();
    let existed = !same.is_empty();
    match client.release(tag)? {
        None => {
            let description = if build.candidate {
                // SATISFIES release-engine:a-candidate-is-a-prerelease
                let notice =
                    super::forge_text("blocks/release-candidate-notice.md.in", &[("RK_TAG", tag)])?;
                let mut text = notice;
                text.push('\n');
                text.push('\n');
                text.push_str(notes);
                text
            } else {
                notes.to_owned()
            };
            client.create_release(tag, head, &description, &links)?;
            report.linked = links.iter().map(|(name, _)| name.clone()).collect();
            Ok(existed)
        }
        Some(release) => {
            for (name, url) in &links {
                if !release.assets.iter().any(|asset| asset.name == *name) {
                    client.add_link(tag, name, url)?;
                    report.linked.push(name.clone());
                }
            }
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Asset, Local, sort};

    fn local(name: &str, sha256: &str) -> Local {
        Local {
            path: format!("dist/{name}").into(),
            name: name.to_owned(),
            sha256: sha256.to_owned(),
        }
    }

    fn held(name: &str, sha256: &str) -> Asset {
        Asset {
            name: name.to_owned(),
            sha256: Some(sha256.to_owned()),
        }
    }

    fn named(locals: &[&Local]) -> Vec<String> {
        locals.iter().map(|local| local.name.clone()).collect()
    }

    /// SATISFIES release-engine:every-step-converges
    #[test]
    fn a_held_bundle_is_kept_only_beside_a_matching_artifact() {
        let locals = [
            local("w.tar.gz", "aa"),
            local("w.tar.gz.sigstore.json", "b2"),
        ];
        let matching = [held("w.tar.gz", "aa"), held("w.tar.gz.sigstore.json", "b1")];
        let (missing, differing, same) = sort(&locals, &matching, true);
        assert!(missing.is_empty() && differing.is_empty());
        assert_eq!(named(&same), ["w.tar.gz", "w.tar.gz.sigstore.json"]);
        // On a draft the bundle differs like any file.
        let (_, differing, _) = sort(&locals, &matching, false);
        assert_eq!(named(&differing), ["w.tar.gz.sigstore.json"]);
        // Beside an artifact with other bytes, both refuse.
        let other = [held("w.tar.gz", "00"), held("w.tar.gz.sigstore.json", "b1")];
        let (_, differing, same) = sort(&locals, &other, true);
        assert!(same.is_empty());
        assert_eq!(named(&differing), ["w.tar.gz", "w.tar.gz.sigstore.json"]);
    }
}
