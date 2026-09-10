//! `rk adopt`: a pre-record target becomes a recorded one.
//!
//! Adoption verifies the payload before writing configuration and its record. The
//! candidate payload is rendered first, exactly as `rk init` would
//! produce it; every `rendered` destination must match it byte for byte,
//! and one mismatch refuses the whole adoption listing every mismatch in
//! one run. Blessing whatever is on disk would launder arbitrary drift
//! into release-kit ownership, so nothing here ever takes the disk as the
//! baseline — and no target file is ever changed: not a byte, not a mode,
//! not a sentinel. Configuration writes before the manifest, after every check
//! has passed.

use serde::Serialize;

use crate::cli::adopt::AdoptArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::manifest::{self, FileRecord, Manifest, Parameters, Style, Workflow};
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
    /// The working-copy mode the candidate was rendered under and the
    /// record carries.
    workflow: &'static str,
    style: &'static str,
    /// Whether the record carries the Nix capability.
    nix: bool,
    /// The Nix destinations excluded from the candidate, each with why;
    /// absent where nothing was withheld.
    #[serde(skip_serializing_if = "Option::is_none")]
    withheld: Option<Vec<landing::Withheld>>,
    config: crate::config::Plan,
    /// Every destination, with its verification result.
    files: Vec<FileEntry>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Verify the target against the rendered candidate and, on `--apply`,
/// write the config and record inside `.release-kit/`.
///
/// # Errors
///
/// Returns a refusal for a target already carrying a record, for any
/// `rendered` mismatch or missing expected file — listing every one in
/// one run — and [`RkError::Missing`] where detection resolves no
/// technology, forge, or repository and no flag covers the gap.
#[allow(clippy::too_many_lines)]
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
    let config = crate::config::load(args.target.as_std_path())?;
    let params = landing::Params::resolve(
        &args.target,
        &landing::Inputs {
            tech: args.tech.as_deref(),
            forge: args.forge.as_deref(),
            repo: args.repo.as_deref(),
            workflow: args.workflow.as_deref().map(Workflow::parse).transpose()?,
            style: args.style.as_deref().map(Style::parse).transpose()?,
            nix: args.nix.then_some(true),
        },
        config.as_ref(),
        None,
        landing::Purpose::Adopt,
    )?;
    let config =
        crate::config::Plan::new(args.target.as_std_path(), &params, config.as_ref(), None)?;
    let tech = params.tech().to_owned();
    let repo = params.repo().to_owned();
    let workflow = params.workflow();
    let style = params
        .style()
        .ok_or_else(|| RkError::Usage("landing style is unresolved".into()))?;
    let mut entries = landing::projection(&params)?;
    let withheld = landing::withhold_nix(&args.target, params.nix(), None, &mut entries)?;
    let (files, records) = verify(args, workflow, &entries)?;

    for file in &files {
        out.result_line(match file.action {
            "differs" => format!("differs {} (seeded, target-owned)", file.path),
            action => format!("{action} {}", file.path),
        });
    }
    for entry in &withheld {
        out.result_line(format!("withheld {}: {}", entry.path, entry.reason));
    }

    if args.apply {
        config.apply(args.target.as_std_path())?;
        manifest::write(
            &args.target,
            &Manifest {
                schema_version: manifest::SCHEMA_VERSION,
                rk_version: env!("CARGO_PKG_VERSION").to_owned(),
                payload_sha256: crate::commands::payload::report().payload_sha256,
                origin: "adopt".to_owned(),
                tech: tech.clone(),
                forge: params.forge().to_owned(),
                landed_at: manifest::now(),
                parameters: Parameters {
                    repo: repo.clone(),
                    workflow,
                    style: Some(style),
                    nix: params.nix(),
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
            "commit the config and the record".to_owned(),
            format!("rk status --target {} reports this landing", args.target),
        ]
    } else {
        vec![format!(
            "rk adopt --tech {tech} --forge {} --repo {repo} --workflow {} --style {}{} --target {} --apply writes the config and the record inside .release-kit/",
            params.forge().to_owned(),
            workflow.as_str(),
            style.as_str(),
            if params.nix() { " --nix" } else { "" },
            args.target
        )]
    };
    out.result_line(format!(
        "{} {}\n{}",
        config.action,
        crate::config::CONFIG_PATH,
        config.content
    ));
    out.next(&next);
    out.emit(&Report {
        schema: "rk.adopt/5",
        config,
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        tech,
        forge: params.forge().to_owned(),
        repo,
        workflow: workflow.as_str(),
        style: style.as_str(),
        nix: params.nix(),
        withheld: (!withheld.is_empty()).then_some(withheld),
        files,
        next,
    })
}

/// The verification pass: every destination checked against the rendered
/// candidate, every failure collected before the one refusal, so an
/// operator resolves everything and re-runs once.
fn verify(
    args: &AdoptArgs,
    workflow: Workflow,
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
        .expected(format!(
            "every rendered destination matching the {} candidate, byte for byte",
            workflow.as_str()
        ))
        .action(
            "align first: rk adopt without --apply lists every differing destination; bring each to the selected candidate's bytes — rk snippet and rk payload print them — then re-run, or select the other candidate with --workflow or --style",
        )
        .target_state("unchanged"),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{FileEntry, Report};

    /// The complete `rk.adopt/5` shape, held by snapshot.
    #[test]
    fn the_adopt_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.adopt/5",
            config: crate::config::Plan {
                action: "added",
                changes: vec![],
                content: "schema_version = 1\n".into(),
            },
            mode: "apply",
            target: "/tmp/t".into(),
            tech: "rust".into(),
            forge: "github".into(),
            repo: "acme/widget".into(),
            workflow: "branches",
            style: "trunk",
            nix: false,
            withheld: None,
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                kind: "seeded",
                action: "differs",
            }],
            next: vec!["commit the config and the record".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.adopt/5","mode":"apply","target":"/tmp/t","tech":"rust","forge":"github","repo":"acme/widget","workflow":"branches","style":"trunk","nix":false,"config":{"action":"added","changes":[],"content":"schema_version = 1\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"differs"}],"next":["commit the config and the record"]}"#
        );
    }
}
