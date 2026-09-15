//! `rk upgrade`: a landed target takes this binary's projection.
//!
//! A front over the direct writer: the receipt is required, the target's
//! evidence is gathered once, this binary's projection is computed from
//! its embedded sources, and every destination is decided by its recorded
//! kind alone. A recorded generated file is replaced from the candidate
//! whatever its bytes are, because the operator and the agent authorized
//! the migration and Git holds the recovery; a recorded seeded or state
//! file is preserved with its current digest entering the receipt; a
//! recorded marked region is replaced alone; a recorded destination the
//! projection no longer produces is released to the target. A whole-file
//! destination present on disk and absent from the receipt refuses
//! before any write, every collision collected in one pass. Preview by
//! default; `--apply` lands under one target lock with the receipt last.
//!
//! SATISFIES landing:ownership-is-elementary
//! SATISFIES landing:a-dropped-file-stays
//! SATISFIES landing:a-target-is-never-downgraded

use std::fmt::Write as _;

use serde::Serialize;

use crate::cli::upgrade::UpgradeArgs;
use crate::diagnostic::{Diagnostic, Reason};
use crate::embedded;
use crate::error::RkError;
use crate::held;
use crate::landing::apply::{self, Action, Collision, Prepared};
use crate::landing::manifest::{self, Alignment, Manifest, Provider};
use crate::landing::{self, lock};
use crate::output::Output;
use crate::profile::{CapabilityRequests, GitWorkflow, ProfileSnapshot};
use crate::stage::CapabilityNote;

/// One destination and what the upgrade decided for it.
#[derive(Debug, Serialize)]
struct FileEntry {
    /// The destination, relative to the target.
    path: String,
    /// The kind this projection declares for it, or the recorded kind of
    /// a released destination.
    kind: &'static str,
    /// `created`, `replaced`, `matched`, `preserved`, `drift`, `released`,
    /// or `collision` in a preview.
    action: &'static str,
}

/// The machine form of an upgrade report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The target directory.
    target: String,
    /// The version the receipt came from.
    from_version: String,
    /// This binary's version.
    to_version: &'static str,
    /// What the project is, as the rewritten receipt carries it.
    profile: ProfileSnapshot,
    /// How topic branches reach the trunk.
    git: GitWorkflow,
    /// Which optional products the rewritten receipt carries.
    capabilities: CapabilityRequests,
    /// Every capability, in catalog order, with its status.
    selection: Vec<CapabilityNote>,
    /// Why the selected release automation cannot land in this release,
    /// absent where it can or none is selected. A preview reports it and
    /// exits 0; the apply refuses on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    release_unavailable: Option<String>,
    /// Why the provider's licence condition refuses this target, absent
    /// where no condition applies or the licence satisfies it. A preview
    /// reports it and exits 0; the apply refuses on it.
    #[serde(skip_serializing_if = "Option::is_none")]
    licence_refusal: Option<String>,
    /// The Nix destinations this target could not take, each with why;
    /// absent where nothing was withheld.
    #[serde(skip_serializing_if = "Option::is_none")]
    withheld: Option<Vec<landing::Withheld>>,
    /// The destinations the landing refuses as they stand; absent where
    /// there is none. A preview lists them and exits 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    collisions: Option<Vec<Collision>>,
    config: crate::config::Plan,
    /// Every destination, with its action.
    files: Vec<FileEntry>,
    /// What plausibly follows.
    next: Vec<String>,
}

/// Upgrade the landed target to this binary's projection.
///
/// # Errors
///
/// Returns a refusal for a missing receipt, an unknown receipt schema, a
/// receipt from a newer binary, and, on apply, any collected collision;
/// and [`RkError::Io`] on filesystem failure.
pub fn run(args: &UpgradeArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    // An apply takes the target before it reads anything of it, the
    // receipt and the configuration included. A preview holds nothing.
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
    let recorded = load_upgradable(&held)?;
    let existing = crate::config::load(held.base().as_std_path())?;
    let params = resolve_params(args, &held, &recorded, existing.as_ref())?;

    let (prepared, landed) = if let Some(lock) = lock {
        let prepared = apply::prepare(&held, Some(&recorded), &params, existing.as_ref())?;
        let landed = apply::land(
            &held,
            Some(&recorded),
            &prepared,
            apply::Origin::Upgrade,
            &lock,
        )?;
        drop(lock);
        (prepared, Some(landed))
    } else {
        (
            apply::prepare(&held, Some(&recorded), &params, existing.as_ref())?,
            None,
        )
    };

    let sentinels = report_decisions(out, &held, &prepared, landed.is_some())?;
    if landed.is_some() {
        out.result_line(format!("rewrote {}", manifest::MANIFEST_PATH));
        for sentinel in &sentinels {
            out.result_line(format!("fill this sentinel: {sentinel}"));
        }
    }

    let next = next_lines(args, &params, prepared.collisions.is_empty());
    out.next(&next);
    out.emit(&Report {
        schema: "rk.upgrade/10",
        config: prepared.config.clone(),
        mode: if args.apply { "apply" } else { "preview" },
        target: args.target.to_string(),
        from_version: recorded.rk_version,
        to_version: env!("CARGO_PKG_VERSION"),
        profile: params.profile().clone(),
        git: params.git().clone(),
        capabilities: params.capabilities().clone(),
        selection: prepared
            .projection
            .capabilities
            .iter()
            .map(|selection| CapabilityNote::of(selection, &prepared.projection))
            .collect(),
        release_unavailable: prepared.projection.release_unavailable().map(str::to_owned),
        licence_refusal: prepared.projection.licence_refusal.clone(),
        withheld: withheld_of(&prepared),
        collisions: (!prepared.collisions.is_empty()).then(|| prepared.collisions.clone()),
        files: prepared
            .decisions
            .iter()
            .map(|decision| FileEntry {
                path: decision.destination.clone(),
                kind: decision.kind.as_str(),
                action: decision.action.as_str(),
            })
            .chain(prepared.collisions.iter().map(|collision| FileEntry {
                path: collision.path.clone(),
                kind: landing::kind_of(&collision.path).map_or("unknown", landing::Kind::as_str),
                action: "collision",
            }))
            .collect(),
        next,
    })
}

/// The withheld list a report carries.
fn withheld_of(prepared: &Prepared) -> Option<Vec<landing::Withheld>> {
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
}

/// Every human line one upgrade prints about its own decisions, and the
/// sentinels a landed run found while it read them back.
///
/// `landed` says whether the run wrote: only then is a created or replaced
/// destination read back for its unfilled sentinels, because a preview has
/// nothing on disk to read.
///
/// # Errors
///
/// A read failure at a destination the run just wrote.
fn report_decisions(
    out: Output,
    held: &apply::Held,
    prepared: &Prepared,
    landed: bool,
) -> Result<Vec<String>, RkError> {
    out.result_line(format!(
        "profile: {}",
        crate::commands::profile::describe(
            prepared.params.profile(),
            prepared.params.git(),
            prepared.params.capabilities(),
            prepared.params.repo()
        )
    ));
    crate::commands::init::describe_selection(out, prepared);
    for key in &prepared.config.changes {
        out.result_line(format!("configuration changes {key}"));
    }
    out.result_line(format!(
        "{} {}",
        prepared.config.action,
        crate::config::CONFIG_PATH
    ));
    let mut sentinels: Vec<String> = Vec::new();
    for decision in &prepared.decisions {
        if landed && matches!(decision.action, Action::Created | Action::Replaced) {
            let bytes =
                landing::read_recorded(held.base(), &decision.destination)?.unwrap_or_default();
            collect_sentinels(&decision.destination, &bytes, &mut sentinels);
        }
        out.result_line(crate::commands::init::describe(decision));
    }
    for collision in &prepared.collisions {
        out.result_line(format!(
            "collision {}: {}",
            collision.path, collision.reason
        ));
    }
    for entry in &prepared.projection.omissions {
        out.result_line(format!("withheld {}: {}", entry.destination, entry.reason));
        if let Some(action) = &entry.action {
            out.result_line(format!("  action: {action}"));
        }
    }
    if let Some(reason) = prepared.projection.licence_refusal.as_deref() {
        out.result_line(format!("licence refusal: {reason}"));
    }
    if let Some(reason) = prepared.projection.release_unavailable() {
        out.result_line(format!("release automation unavailable: {reason}"));
    }
    Ok(sentinels)
}

/// One capability flag's answer: `on`, `off`, or unanswered.
///
/// Every opt-in capability reads its flag the same way, so the refusal
/// names the flag and the two values from one place.
fn toggle(flag: &str, value: Option<&str>) -> Result<Option<bool>, RkError> {
    match value {
        None => Ok(None),
        Some("on") => Ok(Some(true)),
        Some("off") => Ok(Some(false)),
        Some(other) => Err(RkError::Usage(format!(
            "unknown --{flag} value '{other}'; the values are: on, off"
        ))),
    }
}

fn resolve_params(
    args: &UpgradeArgs,
    held: &apply::Held,
    recorded: &Manifest,
    existing: Option<&crate::config::Config>,
) -> Result<landing::Params, RkError> {
    let nix = toggle("nix-packaging", args.nix_packaging.as_deref())?;
    let reporting_policy = toggle("reporting-policy", args.reporting_policy.as_deref())?;
    let scorecard = toggle("scorecard", args.scorecard.as_deref())?;
    let code_scanning = args
        .code_scanning
        .as_deref()
        .map(Provider::parse)
        .transpose()?;
    landing::Params::resolve(
        held.base(),
        &landing::Inputs {
            nix,
            reporting_policy,
            scorecard,
            code_scanning,
            ..args.profile.inputs()?
        },
        existing,
        Some(recorded),
        landing::Purpose::Upgrade,
    )
}

/// The `Next:` lines for each outcome. A behavior-defining flag the
/// preview was run with rides into the follow-up command, so following
/// it applies the decision that was previewed, never a different one.
fn next_lines(args: &UpgradeArgs, params: &landing::Params, clean: bool) -> Vec<String> {
    // A flag the preview was run with rides into the follow-up, as the
    // resolved answer it produced; a flag it was not run with stays out,
    // so the configuration and the record keep answering it.
    let profile = &args.profile;
    let mut identity_flags = String::new();
    for technology in params.technologies() {
        if !profile.technology.is_empty() {
            let _ = write!(identity_flags, " --technology {technology}");
        }
    }
    for (given, flag, value) in [
        (
            profile.forge.is_some(),
            "forge",
            params.forge().map(str::to_owned),
        ),
        (
            profile.repo.is_some(),
            "repo",
            Some(params.repo().to_owned()),
        ),
        (
            profile.release_mode.is_some(),
            "release-mode",
            Some(params.release_mode().as_str().to_owned()),
        ),
        (
            profile.release_driver.is_some(),
            "release-driver",
            params.driver().map(str::to_owned),
        ),
        (
            profile.release_style.is_some(),
            "release-style",
            params.style().map(|style| style.as_str().to_owned()),
        ),
        (
            profile.trunk.is_some(),
            "trunk",
            Some(params.trunk().to_owned()),
        ),
        (
            profile.checkout_mode.is_some(),
            "checkout-mode",
            Some(params.checkout_mode().as_str().to_owned()),
        ),
    ] {
        if given && let Some(value) = value {
            let _ = write!(identity_flags, " --{flag} {value}");
        }
    }
    let workflow_flag = String::new();
    let style_flag = String::new();
    let capabilities = params.capability_toggles();
    if args.apply {
        vec![
            "commit the upgraded files, the receipt included".to_owned(),
            format!("rk status --target {} reports the result", args.target),
        ]
    } else if clean {
        vec![
            format!(
                "rk upgrade{identity_flags}{workflow_flag}{style_flag}{capabilities} --target {} --apply writes",
                args.target
            ),
            format!(
                "rk stage --target {} stages the complete candidate for a byte comparison",
                args.target
            ),
        ]
    } else {
        vec![
            format!(
                "resolve each collision above through the rk-setup skill; rk upgrade{identity_flags}{workflow_flag}{style_flag}{capabilities} --target {} --apply refuses until then",
                args.target
            ),
            format!(
                "rk stage --target {} stages the complete candidate for a byte comparison",
                args.target
            ),
        ]
    }
}

/// The receipt an upgrade may act on: present, at a known schema, and not
/// from a newer binary than this one.
fn load_upgradable(held: &apply::Held) -> Result<Manifest, RkError> {
    let target = held.display();
    let Some(recorded) = manifest::load(held.base())? else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "no {} at {target}: an upgrade needs the receipt of the landing it moves, and nothing was written",
                    manifest::MANIFEST_PATH
                ),
            )
            .expected("a recorded landing")
            .action(format!(
                "rk stage --target {target} stages this binary's candidate for a byte comparison; the rk-setup skill carries the best-effort migration, ending in rk adopt for a target brought to the candidate or rk init for a fresh one"
            ))
            .target_state("unchanged"),
        ));
    };
    if manifest::alignment(&recorded.rk_version, env!("CARGO_PKG_VERSION"))
        == Alignment::TargetNewer
    {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "this landing came from rk {}, newer than this binary's {}; downgrading a target is not an upgrade",
                    recorded.rk_version,
                    env!("CARGO_PKG_VERSION")
                ),
            )
            .expected("a binary at or above the recorded rk_version")
            .action(format!("install release-kit {} or newer", recorded.rk_version))
            .target_state("unchanged"),
        ));
    }
    Ok(recorded)
}

/// The judgment sentinels a newly written file carries.
fn collect_sentinels(destination: &str, bytes: &[u8], found: &mut Vec<String>) {
    let text = String::from_utf8_lossy(bytes);
    for (idx, line) in text.lines().enumerate() {
        if line.contains(embedded::SENTINEL) {
            found.push(format!("{destination}:{}: {}", idx + 1, line.trim()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FileEntry, Report};
    use crate::landing::CheckoutMode;
    use crate::profile::{
        CapabilityRequests, GitWorkflow, ProfileSnapshot, ReleaseIntent, ReleaseMode,
    };

    /// The complete `rk.upgrade/10` shape, held by snapshot.
    #[test]
    fn the_upgrade_report_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.upgrade/10",
            config: crate::config::Plan {
                action: "added",
                changes: vec![],
                content: "schema_version = 2\n".into(),
            },
            mode: "preview",
            target: "/tmp/t".into(),
            from_version: "0.1.0".into(),
            to_version: "0.2.0",
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
            },
            capabilities: CapabilityRequests {
                nix_packaging: false,
                reporting_policy: true,
                scorecard: false,
                code_scanning: None,
            },
            selection: vec![],
            release_unavailable: None,
            licence_refusal: None,
            withheld: None,
            collisions: None,
            files: vec![
                FileEntry {
                    path: "release-plz.toml".into(),
                    kind: "seeded",
                    action: "drift",
                },
                FileEntry {
                    path: "legacy.yml".into(),
                    kind: "rendered",
                    action: "released",
                },
            ],
            next: vec!["rk upgrade --target /tmp/t --apply writes".into()],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.upgrade/10","mode":"preview","target":"/tmp/t","from_version":"0.1.0","to_version":"0.2.0","profile":{"technologies":["rust"],"forge":"github","release":{"mode":"automatic","driver":"rust","style":"trunk","line_prefix":"release/"}},"git":{"trunk":"master","checkout_mode":"main-worktree"},"capabilities":{"nix_packaging":false,"reporting_policy":true,"scorecard":false},"selection":[],"config":{"action":"added","changes":[],"content":"schema_version = 2\n"},"files":[{"path":"release-plz.toml","kind":"seeded","action":"drift"},{"path":"legacy.yml","kind":"rendered","action":"released"}],"next":["rk upgrade --target /tmp/t --apply writes"]}"#
        );
    }
}
