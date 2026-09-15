//! `rk profile`: what a target resolves to, and what the catalog selects
//! for it.
//!
//! Read-only. The command resolves the target configuration the way every
//! landing verb does, keeps the source of each value, computes the one
//! projection, and reports every domain value with its source, the
//! unknown categories, the observation's proposal where it answered the
//! release mode, every capability with its status, the omissions, and the
//! conflicts. It writes nothing and judges nothing: judgment belongs to
//! `rk status --check` and `rk setup check`.
//!
//! SATISFIES project-profile:the-profile-command-writes-nothing

use std::collections::BTreeMap;

use serde::Serialize;

use crate::cli::profile::ProfileArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, Provider};
use crate::landing::{self, Params};
use crate::output::Output;
use crate::profile::{self, CapabilityRequests, GitWorkflow, ProfileSnapshot, Proposal, Source};
use crate::projection::{Projection, ProjectionInput, TargetEvidence};
use crate::stage::{CapabilityNote, Note};

/// The machine form of a profile report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// The target directory.
    target: String,
    /// What the project is.
    profile: ProfileSnapshot,
    /// How topic branches reach the trunk.
    git: GitWorkflow,
    /// Which optional products the target requests.
    capabilities: CapabilityRequests,
    /// The project path on the forge, empty where the project has none.
    repo: String,
    /// The two security answers.
    security: Security,
    /// The source of each value, keyed by its configuration path.
    sources: BTreeMap<&'static str, Source>,
    /// The category names the catalog does not know.
    unknown: Vec<String>,
    /// What the observation proposed for the release, where nothing above
    /// it answered the mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    proposal: Option<Proposal>,
    /// Every capability, in catalog order, with its status and its
    /// destinations.
    selection: Vec<CapabilityNote>,
    /// The destinations the target's own state withholds.
    omissions: Vec<Note>,
    /// The block destinations whose document offers the block no place.
    collisions: Vec<Note>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// The two security answers.
#[derive(Debug, Serialize)]
struct Security {
    /// The contact, empty for the forge's own wording.
    contact: String,
    /// The acknowledgment window.
    response: String,
}

/// One line stating the three domains and the identity, for every human
/// report that names a resolved target.
#[must_use]
pub fn describe(
    profile: &ProfileSnapshot,
    git: &GitWorkflow,
    capabilities: &CapabilityRequests,
    repo: &str,
) -> String {
    let technologies = if profile.technologies.is_empty() {
        "none".to_owned()
    } else {
        profile.technologies.join(", ")
    };
    let release = match profile.release.mode {
        profile::ReleaseMode::Automatic => format!(
            "automatic (driver {}, style {}, line prefix {})",
            profile.release.driver.as_deref().unwrap_or("unresolved"),
            profile
                .release
                .style
                .map_or("unresolved", crate::landing::Style::as_str),
            profile
                .release
                .line_prefix
                .as_deref()
                .unwrap_or(crate::config::LINE_PREFIX_DEFAULT)
        ),
        other => other.as_str().to_owned(),
    };
    let mut requested: Vec<&str> = Vec::new();
    if capabilities.nix_packaging {
        requested.push("nix_packaging");
    }
    if capabilities.reporting_policy {
        requested.push("reporting_policy");
    }
    if capabilities.scorecard {
        requested.push("scorecard");
    }
    let scanning = capabilities.code_scanning.map(Provider::as_str);
    format!(
        "technologies {technologies}; forge {}; repo {}; release {release}; trunk {}; checkout mode {}; integration {}; requests {}{}",
        profile.forge.as_deref().unwrap_or("none"),
        match repo {
            "" => "none",
            crate::projection::REPO_PLACEHOLDER => "unresolved",
            named => named,
        },
        git.trunk,
        git.checkout_mode.as_str(),
        git.integration.as_str(),
        if requested.is_empty() {
            "none".to_owned()
        } else {
            requested.join(", ")
        },
        scanning.map_or_else(String::new, |provider| format!(
            ", code_scanning {provider}"
        ))
    )
}

/// Report the target's resolution and selection.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory, and
/// the resolution's own refusals for a malformed release intent.
#[allow(
    clippy::too_many_lines,
    reason = "one pass reports every domain, source, capability, and omission, and splitting it would separate a value from the report line that states it"
)]
pub fn run(args: &ProfileArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    if !args.target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {} is not a directory", args.target),
            )
            .expected("an existing repository to resolve for"),
        ));
    }
    let config = crate::config::load(args.target.as_std_path())?;
    let record = manifest::load(&args.target)?;
    let resolved = profile::resolve(
        &args.target,
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
        record.as_ref(),
        landing::Purpose::Preview,
    )?;
    let params: &Params = &resolved.params;
    let evidence = TargetEvidence::gather(&args.target, record.as_ref())?;
    let projection = Projection::compute(&ProjectionInput {
        params: params.clone(),
        evidence,
    })?;

    out.result_line(format!(
        "profile: {}",
        describe(
            params.profile(),
            params.git(),
            params.capabilities(),
            params.repo()
        )
    ));
    for (key, source) in &resolved.sources {
        out.result_line(format!("source {key}: {}", source.as_str()));
    }
    for name in &resolved.unknown {
        out.result_line(format!("unknown {name}: preserved; no adapter drives it"));
    }
    if let Some(proposal) = &resolved.proposal {
        out.result_line(format!(
            "proposal: {}",
            match proposal {
                Proposal::None => "no release-bearing technology, so the release mode is none".to_owned(),
                Proposal::Automatic { driver } => format!("an automatic release driven by {driver}"),
                Proposal::Ambiguous { drivers } => format!(
                    "ambiguous: {} are release-bearing; an apply refuses until --release-driver names one",
                    drivers.join(" and ")
                ),
            }
        ));
    }
    let selection: Vec<CapabilityNote> = projection
        .capabilities
        .iter()
        .map(|selection| CapabilityNote::of(selection, &projection))
        .collect();
    for note in &selection {
        let mut line = format!("{} {}", note.status, note.id);
        if !note.destinations.is_empty() {
            line.push_str(": ");
            line.push_str(&note.destinations.join(", "));
        }
        if let Some(reason) = &note.reason {
            line.push_str(" (");
            line.push_str(reason);
            line.push(')');
        }
        out.result_line(line);
        if let Some(action) = &note.action {
            out.result_line(format!("  action: {action}"));
        }
    }
    let omissions: Vec<Note> = projection
        .omissions
        .iter()
        .map(|omission| Note {
            destination: omission.destination.clone(),
            reason: omission.reason.clone(),
            action: omission.action.clone(),
        })
        .collect();
    let collisions: Vec<Note> = projection
        .collisions
        .iter()
        .map(|collision| Note {
            destination: collision.destination.clone(),
            reason: collision.reason.clone(),
            action: None,
        })
        .collect();
    for note in &omissions {
        out.result_line(format!("withheld {}: {}", note.destination, note.reason));
    }
    for note in &collisions {
        out.result_line(format!("collision {}: {}", note.destination, note.reason));
    }
    // `rk upgrade` takes each capability as `on|off`, and `rk init` takes
    // the boolean ones as bare flags. The follow-up command must parse, so
    // it renders the verb's own spelling rather than one of them twice.
    let (verb, capabilities) = if record.is_some() {
        ("upgrade", params.capability_toggles())
    } else {
        ("init", params.capability_flags())
    };
    let mut next = vec![format!(
        "rk {verb}{}{capabilities} --target {} previews the landing under these answers",
        params.canonical_flags(),
        args.target
    )];
    if let Some(Proposal::Ambiguous { drivers }) = &resolved.proposal {
        next.insert(
            0,
            format!(
                "name the driver before an apply: --release-driver <{}>",
                drivers.join("|")
            ),
        );
    }
    if let Some(reason) = projection.release_unavailable() {
        next.insert(
            0,
            format!("an apply refuses until the release automation resolves: {reason}"),
        );
    }
    next.push(format!(
        "rk stage --target {} stages the complete candidate for a byte comparison",
        args.target
    ));
    out.next(&next);
    out.emit(&Report {
        schema: "rk.profile/1",
        target: args.target.to_string(),
        profile: params.profile().clone(),
        git: params.git().clone(),
        capabilities: params.capabilities().clone(),
        repo: params.repo().to_owned(),
        security: Security {
            contact: params.security_contact().to_owned(),
            response: params.security_response().to_owned(),
        },
        sources: resolved.sources,
        unknown: resolved.unknown,
        proposal: resolved.proposal,
        selection,
        omissions,
        collisions,
        next,
    })
}

#[cfg(test)]
mod tests {
    use super::{Report, Security};
    use crate::landing::{CheckoutMode, Integration};
    use crate::profile::{
        CapabilityRequests, GitWorkflow, ProfileSnapshot, Proposal, ReleaseIntent, ReleaseMode,
        Source,
    };
    use crate::stage::{CapabilityNote, Note};

    /// The complete `rk.profile/1` shape, held by snapshot.
    #[test]
    fn the_profile_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.profile/1",
            target: "/tmp/t".into(),
            profile: ProfileSnapshot {
                technologies: vec!["python".into(), "rust".into()],
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
                checkout_mode: CheckoutMode::LinkedWorktree,
                integration: Integration::Local,
            },
            capabilities: CapabilityRequests {
                nix_packaging: false,
                reporting_policy: true,
                scorecard: false,
                code_scanning: None,
            },
            repo: "acme/widget".into(),
            security: Security {
                contact: String::new(),
                response: "best-effort".into(),
            },
            sources: std::iter::once(("profile.technologies", Source::Observation)).collect(),
            unknown: vec![],
            proposal: Some(Proposal::Ambiguous {
                drivers: vec!["python".into(), "rust".into()],
            }),
            selection: vec![CapabilityNote {
                id: "git.guards".into(),
                status: "selected".into(),
                reason: None,
                action: None,
                destinations: vec!["AGENTS.md".into()],
            }],
            omissions: vec![Note {
                destination: ".gitlab-ci.yml".into(),
                reason: "the target owns it".into(),
                action: Some("add the include".into()),
            }],
            collisions: vec![],
            next: vec!["rk init --target /tmp/t previews the landing".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.profile/1","target":"/tmp/t","profile":{"technologies":["python","rust"],"forge":"github","release":{"mode":"automatic","driver":"rust","style":"trunk","line_prefix":"release/"}},"git":{"trunk":"master","checkout_mode":"linked-worktree","integration":"local"},"capabilities":{"nix_packaging":false,"reporting_policy":true,"scorecard":false},"repo":"acme/widget","security":{"contact":"","response":"best-effort"},"sources":{"profile.technologies":"observation"},"unknown":[],"proposal":{"state":"ambiguous","drivers":["python","rust"]},"selection":[{"id":"git.guards","status":"selected","destinations":["AGENTS.md"]}],"omissions":[{"destination":".gitlab-ci.yml","reason":"the target owns it","action":"add the include"}],"collisions":[],"next":["rk init --target /tmp/t previews the landing"]}"#
        );
    }
}
