//! `rk assess`: classify a target before anything lands.
//!
//! Reporting only, like `rk doctor`: the evidence is gathered read-only,
//! the verdict is computed by the rule in `crate::assess`, and every
//! classification exits 0. What the verdict routes to is stated as the
//! `next` lines, so a skill reads the same answer an operator does. The
//! verb is a front over the planner: beside its own verdict it prints
//! the plan's classification and readiness, computed offline against
//! the embedded bundle.

use serde::Serialize;

use crate::assess::{self, Classification, Evidence};
use crate::cli::assess::AssessArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::output::Output;
use crate::plan::{Readiness, classify};

/// What the planner says about the same target: which procedure a plan
/// is, and whether it may be applied.
#[derive(Debug, Serialize)]
struct PlanSummary {
    /// The plan's classification.
    classification: classify::Classification,
    /// The plan's readiness.
    readiness: Readiness,
}

/// The machine form of an assessment: evidence first, one verdict from it.
#[derive(Debug, Serialize)]
struct Report<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// The assessed repository.
    target: String,
    /// The verdict the evidence produces.
    classification: Classification,
    /// The evidence, flattened beside the verdict.
    #[serde(flatten)]
    evidence: &'a Evidence,
    /// The plan's classification and readiness for this target.
    plan: PlanSummary,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Classify the target and report it.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory, and
/// the record's own failure taxonomy for a landing record that exists but
/// cannot be trusted; a verdict is never an error.
pub fn run(args: &AssessArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    if !args.target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {} is not a directory", args.target),
            )
            .expected("an existing repository to classify"),
        ));
    }
    let evidence = assess::gather(&args.target)?;
    let classification = assess::classify(&evidence);
    let planned = crate::commands::reconcile::compute(
        &crate::plan::PlanRequest {
            target: args.target.clone(),
            intent: crate::plan::Intent::Reconcile,
            selector: "embedded".into(),
            fetch: false,
            observe_forge: false,
            flags: crate::plan::gather::Flags::default(),
            decisions: std::collections::BTreeMap::new(),
        },
        &crate::landing::manifest::now(),
    )?;
    let plan = PlanSummary {
        classification: planned.plan.classification,
        readiness: planned.plan.readiness,
    };
    let next = next_lines(args, &evidence, classification);

    out.result_line(format!("classification: {}", classification.as_str()));
    out.result_line(format!(
        "plan: {}, {}",
        plan.classification.as_str(),
        plan.readiness.as_str()
    ));
    out.result_line(format!(
        "landing: {}",
        evidence.landing.rk_version.as_deref().map_or_else(
            || "none".to_owned(),
            |version| format!("recorded at release-kit {version}")
        )
    ));
    out.result_line(format!("tech: {}", evidence.tech.unwrap_or("undetected")));
    out.result_line(format!(
        "forge: {}",
        match (evidence.forge, evidence.repo.as_deref()) {
            (Some(forge), Some(repo)) => format!("{forge} ({repo})"),
            (Some(forge), None) => forge.to_owned(),
            (None, _) => "undetected".to_owned(),
        }
    ));
    out.result_line(format!(
        "release markers: {}",
        join_or_none(&evidence.release_markers)
    ));
    out.result_line(format!(
        "payload collisions: {}",
        join_or_none(&evidence.collisions)
    ));
    if evidence.git {
        out.result_line(format!("tags: {}", evidence.tags));
        out.result_line(format!(
            "long-lived branches: {}",
            join_or_none(&evidence.long_lived_branches)
        ));
    } else {
        out.result_line("git: not a repository");
    }
    out.next(&next);

    out.emit(&Report {
        schema: "rk.assess/2",
        target: args.target.to_string(),
        classification,
        evidence: &evidence,
        plan,
        next,
    })
}

/// What each verdict routes to. A recorded landing routes by its status
/// report whatever the corpus verdict says: the verdict describes every
/// healthy landing as brownfield, and that is not a migration.
fn next_lines(args: &AssessArgs, evidence: &Evidence, verdict: Classification) -> Vec<String> {
    let target = &args.target;
    if evidence.landing.recorded {
        return vec![
            format!(
                "rk status --target {target} reports this landing; a recorded target routes by its status, not by classification"
            ),
            format!("rk upgrade --target {target} takes it to this binary's payload"),
        ];
    }
    match verdict {
        Classification::Greenfield => vec![
            format!("rk init --tech <tech> --target {target} lands the workflow; nothing is here to migrate"),
        ],
        Classification::Brownfield => vec![
            "rk guide migration carries the migration procedure".to_owned(),
            format!("rk adopt --target {target} previews whether what is here matches one rendered candidate"),
            format!("rk setup check --target {target} reports what the forge already enforces"),
        ],
        Classification::NeedsDecision => vec![
            "the operator says what the release activity is before any plan claims to know; rk guide migration carries the procedure once it is a migration".to_owned(),
        ],
    }
}

fn join_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_owned()
    } else {
        items.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::{PlanSummary, Report};
    use crate::assess::{Classification, Evidence, Landing};
    use crate::plan::{Readiness, classify};

    /// The complete `rk.assess/2` shape, held by snapshot.
    #[test]
    fn the_assess_report_schema_snapshot_holds() {
        let evidence = Evidence {
            landing: Landing {
                recorded: false,
                rk_version: None,
            },
            tech: Some("rust"),
            forge: Some("github"),
            repo: Some("acme/widget".into()),
            release_markers: vec!["CHANGELOG.md".into()],
            collisions: vec!["release-plz.toml".into()],
            git: true,
            tags: 3,
            long_lived_branches: vec!["develop".into()],
        };
        let report = Report {
            schema: "rk.assess/2",
            target: "/tmp/t".into(),
            classification: Classification::Brownfield,
            evidence: &evidence,
            plan: PlanSummary {
                classification: classify::Classification::Migration,
                readiness: Readiness::Blocked,
            },
            next: vec!["rk guide migration carries the migration procedure".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.assess/2","target":"/tmp/t","classification":"brownfield","landing":{"recorded":false},"tech":"rust","forge":"github","repo":"acme/widget","release_markers":["CHANGELOG.md"],"collisions":["release-plz.toml"],"git":true,"tags":3,"long_lived_branches":["develop"],"plan":{"classification":"migration","readiness":"blocked"},"next":["rk guide migration carries the migration procedure"]}"#
        );
    }
}
