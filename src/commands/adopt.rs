//! `rk adopt`: a pre-record target becomes a recorded one.
//!
//! A front over the direct writer: this binary's projection is computed
//! exactly as `rk init` would land it; every `rendered` destination must
//! match it byte for byte, and one mismatch refuses the whole adoption
//! listing every mismatch and every missing expected file in one run.
//! On `--apply` the writer lands the configuration and then the receipt,
//! last, inside `.release-kit/` and nothing else. Blessing whatever is on
//! disk would launder arbitrary drift into release-kit ownership, so
//! nothing here ever takes the disk as the candidate, and no target file
//! is ever changed: not a byte, not a mode, not a sentinel.
//!
//! SATISFIES landing:an-adoption-writes-the-record-and-nothing-else
//! SATISFIES landing:a-missing-receipt-is-a-classification

use serde::Serialize;

use crate::cli::adopt::AdoptArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::held;
use crate::landing::apply::{self, Prepared};
use crate::landing::manifest::{self, CheckoutMode, Provider};
use crate::landing::{self, Kind, lock};
use crate::output::Output;
use crate::profile::{CapabilityRequests, GitWorkflow, ProfileSnapshot};
use crate::projection::Placement;
use crate::stage::CapabilityNote;

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
    /// What the project is, as the candidate was rendered under it.
    profile: ProfileSnapshot,
    /// How topic branches reach the trunk.
    git: GitWorkflow,
    /// Which optional products the record carries.
    capabilities: CapabilityRequests,
    /// The parameter the candidate was rendered under.
    repo: String,
    /// Every capability, in catalog order, with its status.
    selection: Vec<CapabilityNote>,
    /// Why the selected release automation cannot land in this release,
    /// absent where it can or none is selected.
    #[serde(skip_serializing_if = "Option::is_none")]
    release_unavailable: Option<String>,
    /// Why the provider's licence condition refuses this target, absent
    /// where no condition applies or the licence satisfies it. A preview
    /// reports it and exits 0; the apply refuses on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    licence_refusal: Option<String>,
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
/// write the config and receipt inside `.release-kit/`.
///
/// # Errors
///
/// Returns a refusal for a target already carrying a receipt, for any
/// `rendered` mismatch or missing expected file, listing every one in
/// one run, and [`RkError::Missing`] where detection resolves no
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
    // An apply takes the target before it reads anything of it, the
    // receipt and the configuration included, so the bytes verified are
    // the bytes the receipt digests. A preview holds nothing.
    let lock = args
        .apply
        .then(|| lock::acquire(&args.target))
        .transpose()?;
    // One directory descriptor, held from here through the receipt write:
    // every read and every write goes through it, so a root exchanged
    // under the pathname later receives nothing.
    // The proof's pause: the lock is held and the directory is not yet.
    held::pause(apply::PAUSE_VAR, "locked", "proceed-locked");
    let held = lock.as_ref().map_or_else(
        || apply::Held::open(&args.target),
        |lock| apply::Held::open_locked(&args.target, lock),
    )?;
    // The proof's pause: the target is held, and nothing has been read.
    held::pause(apply::PAUSE_VAR, "held", "proceed-held");
    if landing::manifest::load(held.base())?.is_some() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "{} already carries {}; it needs no adoption",
                    args.target,
                    manifest::MANIFEST_PATH
                ),
            )
            .expected("a target without a landing receipt")
            .action(format!(
                "rk upgrade --target {} takes it to this binary's projection",
                args.target
            ))
            .target_state("unchanged"),
        ));
    }
    let config = crate::config::load(held.base().as_std_path())?;
    let params = landing::Params::resolve(
        held.base(),
        &landing::Inputs {
            nix: args.nix_packaging.then_some(true),
            reporting_policy: args.reporting_policy.then_some(true),
            scorecard: args.scorecard.then_some(true),
            code_scanning: args
                .code_scanning
                .as_deref()
                .map(Provider::parse)
                .transpose()?,
            ..args.profile.inputs()?
        },
        config.as_ref(),
        None,
        landing::Purpose::Adopt,
    )?;
    let checkout_mode = params.checkout_mode();

    let mut prepared = apply::prepare(&held, None, &params, config.as_ref())?;
    let files = verify(&held, checkout_mode, &prepared)?;
    // Every destination verified, so what the decision pass read as an
    // unattributed whole file is a file the agent brought to the
    // projection: the adoption records it and writes nothing else.
    prepared.collisions.clear();

    out.result_line(format!(
        "profile: {}",
        crate::commands::profile::describe(
            params.profile(),
            params.git(),
            params.capabilities(),
            params.repo()
        )
    ));
    crate::commands::init::describe_selection(out, &prepared);
    for file in &files {
        out.result_line(match file.action {
            "differs" => format!("differs {} (seeded, target-owned)", file.path),
            action => format!("{action} {}", file.path),
        });
    }
    for entry in &prepared.projection.omissions {
        out.result_line(format!("withheld {}: {}", entry.destination, entry.reason));
    }

    if let Some(lock) = &lock {
        apply::land(&held, None, &prepared, apply::Origin::Adopt, lock)?;
        out.result_line(format!("wrote {}", manifest::MANIFEST_PATH));
    }
    drop(lock);
    report(out, args, &params, &prepared, files)
}

/// The report of a verified target, and of the receipt where one was
/// written.
fn report(
    out: Output,
    args: &AdoptArgs,
    params: &landing::Params,
    prepared: &Prepared,
    files: Vec<FileEntry>,
) -> Result<(), RkError> {
    let repo = params.repo().to_owned();
    let mut next = if args.apply {
        vec![
            "commit the config and the receipt".to_owned(),
            format!("rk status --target {} reports this landing", args.target),
        ]
    } else {
        vec![format!(
            "rk adopt{}{} --target {} --apply writes the config and the receipt inside .release-kit/",
            params.canonical_flags(),
            params.capability_flags(),
            args.target
        )]
    };
    if let Some(reason) = prepared.projection.release_unavailable() {
        next.insert(
            0,
            format!("the apply refuses until the release automation resolves: {reason}"),
        );
    }
    if let Some(reason) = prepared.projection.licence_refusal.as_deref() {
        next.insert(
            0,
            format!("the apply refuses until the licence condition is answered: {reason}"),
        );
    }
    out.result_line(format!(
        "{} {}\n{}",
        prepared.config.action,
        crate::config::CONFIG_PATH,
        prepared.config.content
    ));
    out.next(&next);
    out.emit(&Report {
        schema: "rk.adopt/10",
        config: prepared.config.clone(),
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        profile: params.profile().clone(),
        git: params.git().clone(),
        capabilities: params.capabilities().clone(),
        repo,
        selection: prepared
            .projection
            .capabilities
            .iter()
            .map(|selection| CapabilityNote::of(selection, &prepared.projection))
            .collect(),
        release_unavailable: prepared.projection.release_unavailable().map(str::to_owned),
        licence_refusal: prepared.projection.licence_refusal.clone(),
        withheld: {
            let withheld: Vec<landing::Withheld> = prepared
                .projection
                .omissions
                .iter()
                .map(|omission| landing::Withheld {
                    path: omission.destination.clone(),
                    reason: omission.reason.clone(),
                })
                .collect();
            (!withheld.is_empty()).then_some(withheld)
        },
        files,
        next,
    })
}

/// The verification pass: every candidate checked against the disk,
/// every failure collected before the one refusal, so an operator
/// resolves everything and re-runs once.
fn verify(
    held: &apply::Held,
    checkout_mode: CheckoutMode,
    prepared: &Prepared,
) -> Result<Vec<FileEntry>, RkError> {
    let target = held.display();
    let mut mismatches: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut files = Vec::new();
    // An ill-formed marked document lists beside the mismatches rather
    // than refusing alone, so one run still names everything unadoptable.
    let defects: Vec<String> = prepared
        .projection
        .collisions
        .iter()
        .map(|collision| collision.reason.clone())
        .collect();
    for candidate in &prepared.projection.candidates {
        let path = held.base().join(&candidate.destination);
        let regular = std::fs::symlink_metadata(path.as_std_path())
            .is_ok_and(|metadata| metadata.is_file())
            || !path.exists();
        if !regular {
            mismatches.push(format!("{} (is not a regular file)", candidate.destination));
            continue;
        }
        let current = landing::read_recorded(held.base(), &candidate.destination)?;
        let Some(current) = current else {
            // A block-placed artifact reads as absent from a file that
            // exists; the operator's remedy differs, so the label must.
            let label = if path.exists() {
                format!("{} (carries no release-kit block)", candidate.destination)
            } else {
                format!("{} (expected and missing)", candidate.destination)
            };
            missing.push(label);
            continue;
        };
        let expected: &[u8] = match candidate.placement {
            Placement::Whole => &candidate.bytes,
            Placement::Region { .. } => candidate.region.as_deref().unwrap_or(&candidate.bytes),
        };
        let action = match candidate.kind {
            Kind::Rendered | Kind::Seeded if current == expected => "matches",
            Kind::Rendered => {
                mismatches.push(format!(
                    "{} (differs from the rendered candidate)",
                    candidate.destination
                ));
                "differs"
            }
            Kind::Seeded => "differs",
            Kind::State => "state",
        };
        files.push(FileEntry {
            path: candidate.destination.clone(),
            kind: candidate.kind.as_str(),
            action,
        });
    }
    if mismatches.is_empty() && missing.is_empty() && defects.is_empty() {
        return Ok(files);
    }
    let listed: Vec<String> = mismatches
        .iter()
        .cloned()
        .chain(missing.iter().cloned())
        .chain(defects.iter().cloned())
        .collect();
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "this target is not adoptable as-is, and no receipt was written: {}",
                listed.join(", ")
            ),
        )
        .expected(format!(
            "every rendered destination matching the {} candidate, byte for byte",
            checkout_mode.as_str()
        ))
        .action(format!(
            "align first: rk stage --target {} stages the candidate for a byte comparison, and the rk-setup skill carries the migration that brings each destination to it; then re-run, or select the other candidate with --checkout-mode, --integration, or --release-style{}",
            target,
            // A policy the target wrote its own contact into is the one
            // mismatch a committed answer resolves rather than an edit:
            // naming the keys turns a dead end into the next step.
            if mismatches.iter().any(|path| path.starts_with("SECURITY.md ")) {
                ". SECURITY.md states two facts a target owns: set security.contact and security.response in .release-kit/config.toml to the wording this policy already carries, and the candidate matches"
            } else {
                ""
            }
        ))
        .target_state("unchanged"),
    ))
}

#[cfg(test)]
mod tests {
    use super::{FileEntry, Report};
    use crate::landing::{CheckoutMode, Integration};
    use crate::profile::{
        CapabilityRequests, GitWorkflow, ProfileSnapshot, ReleaseIntent, ReleaseMode,
    };

    /// The complete `rk.adopt/10` shape, held by snapshot.
    #[test]
    fn the_adopt_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.adopt/10",
            config: crate::config::Plan {
                action: "added",
                changes: vec![],
                content: "schema_version = 2\n".into(),
            },
            mode: "apply",
            target: "/tmp/t".into(),
            profile: ProfileSnapshot {
                technologies: vec!["rust".into()],
                forge: Some("github".into()),
                release: ReleaseIntent {
                    mode: ReleaseMode::Automatic,
                    driver: Some("rust".into()),
                    style: Some(crate::landing::Style::Trunk),
                    line_prefix: Some("release/".into()),
                },
            },
            git: GitWorkflow {
                trunk: "master".into(),
                checkout_mode: CheckoutMode::MainWorktree,
                integration: Integration::Local,
            },
            capabilities: CapabilityRequests {
                nix_packaging: false,
                reporting_policy: true,
                scorecard: false,
                code_scanning: Some(crate::landing::Provider::Semgrep),
            },
            repo: "acme/widget".into(),
            selection: vec![],
            release_unavailable: None,
            licence_refusal: None,
            withheld: None,
            files: vec![FileEntry {
                path: "release-plz.toml".into(),
                kind: "seeded",
                action: "differs",
            }],
            next: vec!["commit the config and the receipt".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.adopt/10","mode":"apply","target":"/tmp/t","profile":{"technologies":["rust"],"forge":"github","release":{"mode":"automatic","driver":"rust","style":"trunk","line_prefix":"release/"}},"git":{"trunk":"master","checkout_mode":"main-worktree","integration":"local"},"capabilities":{"nix_packaging":false,"reporting_policy":true,"scorecard":false,"code_scanning":"semgrep"},"repo":"acme/widget","selection":[],"config":{"action":"added","changes":[],"content":"schema_version = 2\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"differs"}],"next":["commit the config and the receipt"]}"#
        );
    }
}
