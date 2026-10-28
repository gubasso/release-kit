//! Reading a stage back as a landing's answers: the receipt's
//! parameters, never its bytes.
//!
//! A production verb named `--stage <root>` reads `stage.json`, refuses a
//! receipt another binary wrote or one taken for another target, takes
//! every answer the stage resolved as its own, and renders again from
//! scratch. Once the answers resolve, they must equal the stage's, so the
//! landing cannot differ from the stage a comparison reads against.
//!
//! SATISFIES staging:a-landing-reads-the-stage-parameters

use std::path::Path;

use camino::Utf8Path;

use super::{Parameters, RECEIPT_NAME, Receipt};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::Params;
use crate::profile::Inputs;

/// Read the receipt at `root` and hold it to this binary and this target.
///
/// # Errors
///
/// [`RkError::Missing`] where no receipt reads at `root`, and a
/// `state-drift` refusal for a receipt from another version or another
/// target.
pub fn load(root: &Utf8Path, canonical_target: &Path) -> Result<Receipt, RkError> {
    let path = root.join(RECEIPT_NAME);
    let receipt: Receipt = std::fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| {
            RkError::missing(
                Diagnostic::new(
                    Reason::TargetNotFound,
                    format!("{path} is no stage receipt this binary reads"),
                )
                .expected("the root of a stage `rk stage` wrote")
                .action("rk stage --target <target> writes one"),
            )
        })?;
    if receipt.rk_version != env!("CARGO_PKG_VERSION") {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "the stage at {root} was written by rk {}, and this is rk {}; nothing was written",
                    receipt.rk_version,
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .expected("a stage this binary wrote")
            .action("stage again with this binary, then land from that stage")
            .target_state("unchanged"),
        ));
    }
    let target = canonical_target.display().to_string();
    if receipt.target != target {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "the stage at {root} was taken for {}, not {target}; nothing was written",
                    receipt.target
                ),
            )
            .expected("a stage taken for this target")
            .action(format!("rk stage --target {target} writes one"))
            .target_state("unchanged"),
        ));
    }
    Ok(receipt)
}

/// Every answer the stage resolved, as invocation answers.
#[must_use]
pub fn inputs(parameters: &Parameters) -> Inputs<'_> {
    let release = &parameters.profile.release;
    Inputs {
        technologies: &parameters.profile.technologies,
        forge: parameters.profile.forge.as_deref(),
        repo: (!parameters.repo.is_empty()).then_some(parameters.repo.as_str()),
        release_mode: Some(release.mode),
        release_driver: release.driver.as_deref(),
        style: release.style,
        trunk: Some(parameters.git.trunk.as_str()),
        checkout_mode: Some(parameters.git.checkout_mode),
        integration: Some(parameters.git.integration),
        merge_form: Some(parameters.git.merge_form),
        required_check: (!parameters.required_check.is_empty())
            .then_some(parameters.required_check.as_str()),
        required_workflow: (!parameters.required_workflow.is_empty())
            .then_some(parameters.required_workflow.as_str()),
        nix: Some(parameters.capabilities.nix_packaging),
        reporting_policy: Some(parameters.capabilities.reporting_policy),
        scorecard: Some(parameters.capabilities.scorecard),
        workflow_guards: parameters.capabilities.workflow_guards,
        code_scanning: Some(parameters.capabilities.code_scanning),
    }
}

/// Refuse where the answers a landing resolved differ from the stage's:
/// an answer no flag carries, such as the security contact, moved in the
/// configuration after the stage was written.
///
/// # Errors
///
/// A `state-drift` refusal naming the restage.
pub fn confirm(params: &Params, receipt: &Receipt, root: &Utf8Path) -> Result<(), RkError> {
    let resolved = serde_json::to_value(Parameters::from(params)).map_err(anyhow::Error::from)?;
    let staged = serde_json::to_value(&receipt.parameters).map_err(anyhow::Error::from)?;
    if resolved == staged {
        return Ok(());
    }
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!(
                "the target's answers moved since the stage at {root} was written, and nothing was written"
            ),
        )
        .expected("the answers the stage resolved")
        .action("stage again, compare, then land from the new stage")
        .target_state("unchanged"),
    ))
}

/// The refusal for an answer given beside `--stage`, which carries every
/// answer itself.
#[must_use]
pub fn answered_twice() -> RkError {
    RkError::Usage(
        "--stage carries every answer the stage resolved; pass the answers to rk stage instead"
            .to_owned(),
    )
}
