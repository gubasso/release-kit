//! `rk doctor`: is this host ready?
//!
//! Runs the whole probe catalog from `crate::probes` and reports by
//! class. The catalog is shared: a mutating command guards the subset it
//! depends on with the same probes, so what the doctor says and what a
//! command refuses on cannot drift apart.

use serde::Serialize;

use crate::cli::doctor::DoctorArgs;
use crate::error::RkError;
use crate::output::Output;
use crate::probes::{self, ProbeClass, ProbeResult, ProbeStatus};

/// The machine form of the doctor report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// Every probe's answer, in catalog order.
    probes: Vec<ProbeResult>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Run the catalog and report it.
///
/// # Errors
///
/// Returns [`RkError::Other`] only when the report cannot serialize; probe
/// failures are results, not errors, and the exit code stays 0.
pub fn run(args: &DoctorArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    let probes = probes::run_all();
    let next = vec!["rk usage lists every verb and flag in one call".to_owned()];

    for class in [ProbeClass::Hard, ProbeClass::Soft] {
        out.result_line(match class {
            ProbeClass::Hard => "hard",
            ProbeClass::Soft => "soft",
        });
        for probe in probes.iter().filter(|probe| probe.class == class) {
            let status = match probe.status {
                ProbeStatus::Ok => "ok    ",
                ProbeStatus::Failed => "failed",
            };
            out.result_line(format!("  {status}  {}: {}", probe.id, probe.message));
            if let Some(remediation) = &probe.remediation {
                out.result_line(format!("          next: {remediation}"));
            }
        }
    }
    out.next(&next);

    out.emit(&Report {
        schema: "rk.doctor/1",
        probes,
        next,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use crate::probes::{ProbeClass, ProbeResult, ProbeStatus};

    /// The `rk.doctor/1` probe shape, held by snapshot.
    #[test]
    fn the_probe_schema_snapshot_holds() {
        let ok = ProbeResult {
            id: "sh",
            class: ProbeClass::Hard,
            status: ProbeStatus::Ok,
            message: "sh runs".into(),
            remediation: None,
        };
        assert_eq!(
            serde_json::to_string(&ok).expect("a probe serializes"),
            r#"{"id":"sh","class":"hard","status":"ok","message":"sh runs"}"#
        );
        let failed = ProbeResult {
            id: "gh-auth",
            class: ProbeClass::Soft,
            status: ProbeStatus::Failed,
            message: "gh is not authenticated".into(),
            remediation: Some("run gh auth login".into()),
        };
        assert_eq!(
            serde_json::to_string(&failed).expect("a probe serializes"),
            r#"{"id":"gh-auth","class":"soft","status":"failed","message":"gh is not authenticated","remediation":"run gh auth login"}"#
        );
    }
}
