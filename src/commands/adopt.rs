//! `rk adopt`: a pre-record target becomes a recorded one.
//!
//! Adoption is a verification pass that happens to end in one write. The
//! candidate payload is rendered first, exactly as `rk init` would
//! produce it; every `rendered` destination must match it byte for byte,
//! and one mismatch refuses the whole adoption listing every mismatch in
//! one run. Blessing whatever is on disk would launder arbitrary drift
//! into release-kit ownership, so nothing here ever takes the disk as the
//! baseline — and no target file is ever changed: not a byte, not a mode,
//! not a sentinel. The one write is the manifest, last, after every check
//! has passed.

use serde::Serialize;

use crate::cli::adopt::AdoptArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::manifest::{self, FileRecord, Manifest, Parameters, Workflow};
use crate::landing::{self, Kind};
use crate::output::Output;
use crate::registry;

/// One verified destination.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// The declared ownership kind.
    kind: &'static str,
    /// `matches`, `differs` for a seeded file, or `state`.
    action: &'static str,
}

/// The machine form of an adoption report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target directory.
    target: String,
    /// The technology whose payload was verified.
    tech: String,
    /// The forge whose payload was verified.
    forge: String,
    /// The parameter the candidate was rendered under.
    repo: String,
    /// Every destination, with its verification result.
    files: Vec<FileEntry>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Verify the target against the rendered candidate and, on `--apply`,
/// write the record and nothing else.
///
/// # Errors
///
/// Returns a refusal for a target already carrying a record, for any
/// `rendered` mismatch or missing expected file — listing every one in
/// one run — and [`RkError::Missing`] where detection resolves no
/// technology, forge, or repository and no flag covers the gap.
pub fn run(args: &AdoptArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    if !args.target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {} is not a directory", args.target),
            )
            .expected("an existing repository to adopt"),
        ));
    }
    if landing::manifest::load(&args.target)?.is_some() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "{} already carries {}; it needs no adoption",
                    args.target,
                    manifest::MANIFEST_PATH
                ),
            )
            .expected("a target without a landing record")
            .action(format!(
                "rk upgrade --target {} takes it to this binary's payload",
                args.target
            ))
            .target_state("unchanged"),
        ));
    }
    let resolved = landing::resolve(&args.target, args.forge.as_deref(), args.repo.as_deref())?;
    let repo = resolved.repo.ok_or_else(landing::repo_unresolved)?;
    let tech = resolved_tech(args)?;
    let scopes = required_scopes(args.scopes.as_deref())?;
    let entries = landing::projection(&tech, &resolved.forge, &repo, &scopes)?;
    let (files, records) = verify(args, &entries)?;

    for file in &files {
        out.result_line(match file.action {
            "differs" => format!("differs {} (seeded, target-owned)", file.path),
            action => format!("{action} {}", file.path),
        });
    }

    if args.apply {
        manifest::write(
            &args.target,
            &Manifest {
                schema_version: manifest::SCHEMA_VERSION,
                rk_version: env!("CARGO_PKG_VERSION").to_owned(),
                payload_sha256: crate::commands::payload::report().payload_sha256,
                origin: "adopt".to_owned(),
                tech: tech.clone(),
                forge: resolved.forge.clone(),
                landed_at: manifest::now(),
                parameters: Parameters {
                    repo: repo.clone(),
                    scopes,
                    workflow: Workflow::Branches,
                },
                files: records,
                pins: registry::pins_for(&tech)
                    .into_iter()
                    .map(|pin| (pin.name, pin.version))
                    .collect(),
            },
        )?;
        out.result_line(format!("wrote {}", manifest::MANIFEST_PATH));
    }

    let next = if args.apply {
        vec![
            "commit the record".to_owned(),
            format!("rk status --target {} reports this landing", args.target),
        ]
    } else {
        vec![format!(
            "rk adopt --target {} --apply writes the record and nothing else",
            args.target
        )]
    };
    out.next(&next);
    out.emit(&Report {
        schema: "rk.adopt/1",
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        tech,
        forge: resolved.forge,
        repo,
        files,
        next,
    })
}

/// The verification pass: every destination checked against the rendered
/// candidate, every failure collected before the one refusal, so an
/// operator resolves everything and re-runs once.
fn verify(
    args: &AdoptArgs,
    entries: &[landing::Entry],
) -> Result<(Vec<FileEntry>, Vec<FileRecord>), RkError> {
    let mut mismatches: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut files = Vec::new();
    let mut records = Vec::new();
    // An ill-formed hook file lists beside the mismatches rather than
    // refusing alone, so one run still names everything unadoptable.
    let mut defects: Vec<String> = Vec::new();
    if let Some(defect) = landing::hooks_file_defect(&args.target)? {
        defects.push(defect);
    }
    for entry in entries {
        let Some(bytes) = landing::read_destination(&args.target, entry)? else {
            // A block-placed artifact reads as absent from a file that
            // exists; the operator's remedy differs, so the label must.
            let label = if args.target.join(&entry.destination).exists() {
                format!("{} (carries no release-kit block)", entry.destination)
            } else {
                format!("{} (expected and missing)", entry.destination)
            };
            missing.push(label);
            continue;
        };
        let action = match entry.kind {
            Kind::Rendered | Kind::Seeded if bytes == entry.rendered => "matches",
            Kind::Rendered => {
                mismatches.push(entry.destination.clone());
                "differs"
            }
            Kind::Seeded => "differs",
            Kind::State => "state",
        };
        files.push(FileEntry {
            path: entry.destination.clone(),
            kind: entry.kind.as_str(),
            action,
        });
        records.push(FileRecord {
            destination: entry.destination.clone(),
            kind: entry.kind,
            sha256: Digest::of(&bytes),
            baseline_sha256: match entry.kind {
                Kind::State => None,
                Kind::Rendered | Kind::Seeded => Some(Digest::of(&entry.baseline)),
            },
        });
    }
    if mismatches.is_empty() && missing.is_empty() && defects.is_empty() {
        return Ok((files, records));
    }
    let listed: Vec<String> = mismatches
        .iter()
        .map(|path| format!("{path} (differs from the rendered candidate)"))
        .chain(missing.iter().cloned())
        .chain(defects.iter().cloned())
        .collect();
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "this target is not adoptable as-is, and no record was written: {}",
                listed.join(", ")
            ),
        )
        .expected("every rendered destination matching this payload's candidate, byte for byte")
        .action(
            "restore each file to the candidate's bytes — rk snippet prints them — or take the difference deliberately through a fresh landing and a reviewed diff",
        )
        .target_state("unchanged"),
    ))
}

/// The technology whose payload the target runs: the flag, or detection
/// from the version file.
fn resolved_tech(args: &AdoptArgs) -> Result<String, RkError> {
    args.tech.as_deref().map_or_else(
        || {
            crate::detect::tech_of(args.target.as_std_path())
                .map(str::to_owned)
                .ok_or_else(|| {
                    RkError::missing(
                        Diagnostic::new(
                            Reason::TargetNotFound,
                            "no technology detected: the target has no version file",
                        )
                        .expected("a Cargo.toml, pyproject.toml, or VERSION file")
                        .action("pass --tech <rust|python|bash>"),
                    )
                })
        },
        |tech| Ok(tech.to_owned()),
    )
}

/// The `--scopes` argument an adoption cannot proceed without: there is
/// no record to read the parameter from yet.
fn required_scopes(raw: Option<&str>) -> Result<Vec<String>, RkError> {
    landing::parse_scopes(raw.ok_or_else(|| {
        RkError::Usage(
            "an adoption renders the candidate under the scopes parameter; pass --scopes <list>, the Conventional Commit scopes this project accepts".into(),
        )
    })?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{FileEntry, Report};

    /// The complete `rk.adopt/1` shape, held by snapshot.
    #[test]
    fn the_adopt_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.adopt/1",
            mode: "apply",
            target: "/tmp/t".into(),
            tech: "rust".into(),
            forge: "github".into(),
            repo: "acme/widget".into(),
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                kind: "seeded",
                action: "differs",
            }],
            next: vec!["commit the record".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.adopt/1","mode":"apply","target":"/tmp/t","tech":"rust","forge":"github","repo":"acme/widget","files":[{"path":"release-plz.toml","kind":"seeded","action":"differs"}],"next":["commit the record"]}"#
        );
    }
}
