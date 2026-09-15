//! The setup step table, defined by what each step proves and by the
//! target configuration it applies to.
//!
//! Steps follow `method/02-setup.md`, with reporting intake after the
//! protection inventory. Each step carries a pure predicate over the
//! resolved configuration: a forge step applies only where an adapter
//! exists, the release half applies only to an automatic release whose
//! automation this release carries, and a local step applies wherever its
//! own prerequisites hold. A step that does not apply reports as such
//! with the profile value that decided it, and carries no operator
//! reason; an exclusion is the operator's own statement about a step that
//! does apply.
//!
//! SATISFIES forge-setup:applicability-follows-the-target-configuration

use crate::detect::Forge;

use super::context::Ctx;

/// What a step touches when it applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutates {
    /// The step only reads and reports.
    Nothing,
    /// The step writes forge configuration.
    Forge,
    /// The step writes local repository state in the target.
    Local,
}

/// Why a step does not apply to a target, or `None` where it does.
///
/// The predicate is pure over the resolved configuration: it reads no
/// forge, no disk, and no environment, so `rk setup --list`, a preview, a
/// check, and an apply all answer alike.
pub type Applies = fn(&Ctx) -> Option<String>;

/// Every step applies wherever the run stands: the local ones.
const fn always(_: &Ctx) -> Option<String> {
    None
}

/// A step that calls the forge applies only where the profile names one
/// this release drives.
fn needs_adapter(ctx: &Ctx) -> Option<String> {
    if ctx.has_adapter() {
        return None;
    }
    Some(ctx.declared_forge().map_or_else(
        || "the profile names no forge".to_owned(),
        |name| format!("the profile names the forge {name}, which this release has no adapter for"),
    ))
}

/// A step that serves the release automation applies only to an automatic
/// release at a forge this release drives.
fn needs_release(ctx: &Ctx) -> Option<String> {
    needs_adapter(ctx).or_else(|| {
        (!ctx.automatic_release()).then(|| {
            format!(
                "profile.release.mode is {}, so no release automation is selected",
                ctx.profile.release.mode.as_str()
            )
        })
    })
}

/// The packaging gate reads the release driver's own command, so it
/// applies where an automatic release names one.
fn needs_driver(ctx: &Ctx) -> Option<String> {
    if !ctx.automatic_release() {
        return Some(format!(
            "profile.release.mode is {}, so no package is published from here",
            ctx.profile.release.mode.as_str()
        ));
    }
    ctx.driver().is_none().then(|| {
        "profile.release.driver names no technology, so no packaging command is known".to_owned()
    })
}

/// The private reporting channel pairs with the landed policy, so it
/// applies where the target requested one at a forge this release drives.
fn needs_reporting_policy(ctx: &Ctx) -> Option<String> {
    needs_adapter(ctx).or_else(|| {
        (!ctx.reporting_policy()).then(|| "capabilities.reporting_policy is false".to_owned())
    })
}

/// One step of the setup, in canonical order.
pub struct StepSpec {
    /// The name, which is also the `rk setup step` argument and the script
    /// file name in every forge tree.
    pub name: &'static str,
    /// The `method/02-setup.md` section the step executes.
    pub chapter: &'static str,
    /// What the step touches under apply.
    pub mutates: Mutates,
    /// What the step proves, from the chapter.
    pub proves: &'static str,
    /// Whether the step deletes anything; a destructive step carries its own
    /// refusal beyond `--apply`.
    pub destructive: bool,
    /// Whether a full run skips the step: an optional step applies only
    /// where its condition holds, by name, through `rk setup step`.
    pub optional: bool,
    /// The forges at which the step reaches the forge through its CLI.
    ///
    /// Empty where the step is local work alone. A forge absent from the
    /// list is one this step answers without a call, which `forge-version`
    /// does on GitHub: a rolling service declares no version floor, so
    /// there is nothing to read. A run that reaches the CLI for a step
    /// that never calls it refuses an operator who has no reason to need
    /// one.
    pub forge_cli: &'static [Forge],
    /// Steps that must be observed satisfied before this one applies.
    pub prereqs: &'static [&'static str],
    /// Why this step does not apply to a target, or `None` where it does.
    pub applies: Applies,
}

impl std::fmt::Debug for StepSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StepSpec")
            .field("name", &self.name)
            .field("chapter", &self.chapter)
            .field("mutates", &self.mutates)
            .field("destructive", &self.destructive)
            .field("optional", &self.optional)
            .field("prereqs", &self.prereqs)
            .finish_non_exhaustive()
    }
}

/// The fifteen steps, with reporting intake last.
///
/// `package-check`, `branch-reminder`, and `forge-version` belong to no
/// forge tree — the first reads its command from the technology binding,
/// the second writes an embedded hook body into the target's own git
/// directory, and the third is one read-only API call the observer already
/// makes — which makes them the three steps outside the parity rule.
pub const STEPS: [StepSpec; 15] = [
    StepSpec {
        name: "package-check",
        chapter: "§0",
        mutates: Mutates::Nothing,
        proves: "the package is publishable with no credentials, and carries the reporting policy where the binding can list it",
        destructive: false,
        optional: false,
        forge_cli: &[],
        prereqs: &[],
        applies: needs_driver,
    },
    StepSpec {
        name: "default-branch",
        chapter: "§1",
        mutates: Mutates::Forge,
        proves: "the trunk is the default branch",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_adapter,
    },
    StepSpec {
        name: "single-trunk",
        chapter: "§1",
        mutates: Mutates::Forge,
        proves: "no long-lived branch besides the trunk remains",
        destructive: true,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &["default-branch"],
        applies: needs_adapter,
    },
    StepSpec {
        name: "merge-cleanup",
        chapter: "§1",
        mutates: Mutates::Forge,
        proves: "the forge deletes a branch when its merge lands",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &["default-branch"],
        applies: needs_adapter,
    },
    StepSpec {
        name: "branch-reminder",
        chapter: "§1",
        mutates: Mutates::Local,
        proves: "a pull reminds the operator when a merged branch lingers locally",
        destructive: false,
        optional: false,
        forge_cli: &[],
        prereqs: &[],
        applies: always,
    },
    StepSpec {
        name: "ci-permissions",
        chapter: "§2",
        mutates: Mutates::Forge,
        proves: "CI may write and open requests",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_release,
    },
    StepSpec {
        name: "install-bot",
        chapter: "§2",
        mutates: Mutates::Forge,
        proves: "the bot identity can act on this project",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_release,
    },
    StepSpec {
        name: "bot-secrets",
        chapter: "§2",
        mutates: Mutates::Forge,
        proves: "the bot credentials are stored on the project",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_release,
    },
    StepSpec {
        name: "forge-version",
        chapter: "§3",
        mutates: Mutates::Nothing,
        proves: "the forge meets the convention's minimum version",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Gitlab],
        prereqs: &[],
        applies: needs_adapter,
    },
    StepSpec {
        name: "protect-trunk",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "the trunk takes no direct push, merges only by squash with the request's title and body as the message, and requires the named check",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &["default-branch", "forge-version"],
        applies: needs_adapter,
    },
    StepSpec {
        name: "protect-tags",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "v* is protected as far as the forge allows",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_adapter,
    },
    StepSpec {
        name: "protect-release-lines",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "release/* cannot be force-pushed or deleted",
        destructive: false,
        optional: true,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_release,
    },
    StepSpec {
        name: "auto-merge",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "a request may merge itself once its required checks pass",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &["default-branch"],
        applies: needs_release,
    },
    StepSpec {
        name: "protections-check",
        chapter: "§3",
        mutates: Mutates::Nothing,
        proves: "exactly the owned protections, with those rules",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_adapter,
    },
    StepSpec {
        name: "private-vulnerability-reporting",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "a vulnerability report has a private intake path, with the forge's limits named",
        destructive: false,
        optional: false,
        forge_cli: &[Forge::Github, Forge::Gitlab],
        prereqs: &[],
        applies: needs_reporting_policy,
    },
];

/// Look one step up by name.
#[must_use]
pub fn spec(name: &str) -> Option<&'static StepSpec> {
    STEPS.iter().find(|step| step.name == name)
}

#[cfg(test)]
mod tests {
    use super::{Ctx, STEPS, spec};
    use crate::profile::ReleaseMode;

    /// One resolved configuration to judge the table against: the forge
    /// the profile declares, whether this release has an adapter for it,
    /// and the release mode.
    fn ctx(forge: Option<&str>, mode: ReleaseMode) -> Ctx {
        let mut ctx = Ctx::for_tests(
            camino::Utf8PathBuf::from("/tmp/target"),
            "acme/widget".to_owned(),
            crate::detect::Forge::Github,
            std::path::PathBuf::from("gh"),
            Some("rust"),
        );
        ctx.declared_forge = forge.map(str::to_owned);
        ctx.forge = forge.and_then(crate::detect::Forge::parse);
        ctx.profile.forge = ctx.declared_forge.clone();
        ctx.profile.release.mode = mode;
        if mode != ReleaseMode::Automatic {
            ctx.profile.release.driver = None;
            ctx.profile.release.style = None;
            ctx.profile.release.line_prefix = None;
        }
        ctx
    }

    /// The steps one configuration applies, in table order.
    fn applicable(forge: Option<&str>, mode: ReleaseMode) -> Vec<&'static str> {
        let ctx = ctx(forge, mode);
        STEPS
            .iter()
            .filter(|step| (step.applies)(&ctx).is_none())
            .map(|step| step.name)
            .collect()
    }

    /// SATISFIES forge-setup:applicability-follows-the-target-configuration
    /// The matrix across an absent forge, an unknown one, and each driven
    /// one, at each release mode.
    #[test]
    fn the_applicability_matrix_follows_the_profile() {
        // No forge: the local step alone.
        for mode in [
            ReleaseMode::Automatic,
            ReleaseMode::External,
            ReleaseMode::None,
        ] {
            let steps = applicable(None, mode);
            assert_eq!(
                steps,
                if mode == ReleaseMode::Automatic {
                    vec!["package-check", "branch-reminder"]
                } else {
                    vec!["branch-reminder"]
                },
                "no forge, {mode:?}"
            );
        }
        // A forge this release has no adapter for reads like no forge.
        assert_eq!(
            applicable(Some("codeberg"), ReleaseMode::None),
            ["branch-reminder"]
        );

        for forge in ["github", "gitlab"] {
            let automatic = applicable(Some(forge), ReleaseMode::Automatic);
            for step in STEPS.iter().map(|step| step.name) {
                assert!(automatic.contains(&step), "{forge} automatic: {step}");
            }
            // A release-less target keeps the trunk half and the tag
            // protection, and takes none of the release half.
            for mode in [ReleaseMode::External, ReleaseMode::None] {
                let steps = applicable(Some(forge), mode);
                for step in [
                    "default-branch",
                    "single-trunk",
                    "merge-cleanup",
                    "branch-reminder",
                    "forge-version",
                    "protect-trunk",
                    "protect-tags",
                    "protections-check",
                    "private-vulnerability-reporting",
                ] {
                    assert!(steps.contains(&step), "{forge} {mode:?}: {step}");
                }
                for step in [
                    "package-check",
                    "ci-permissions",
                    "install-bot",
                    "bot-secrets",
                    "auto-merge",
                    "protect-release-lines",
                ] {
                    assert!(!steps.contains(&step), "{forge} {mode:?}: {step} applies");
                }
            }
        }
    }

    /// The reporting channel pairs with the landed policy: a target that
    /// requests none is asked for none.
    #[test]
    fn the_reporting_channel_follows_the_requested_policy() {
        let mut ctx = ctx(Some("github"), ReleaseMode::None);
        assert!(
            (spec("private-vulnerability-reporting")
                .expect("the step exists")
                .applies)(&ctx)
            .is_none()
        );
        ctx.capabilities.reporting_policy = false;
        let reason = (spec("private-vulnerability-reporting")
            .expect("the step exists")
            .applies)(&ctx)
        .expect("an unrequested policy names itself");
        assert!(reason.contains("capabilities.reporting_policy"), "{reason}");
    }

    /// The packaging gate reads the release driver's own command, so an
    /// ambiguous observation that named none leaves it out.
    #[test]
    fn the_packaging_gate_needs_a_named_driver() {
        let mut ctx = ctx(Some("github"), ReleaseMode::Automatic);
        ctx.profile.release.driver = None;
        let reason = (spec("package-check").expect("the step exists").applies)(&ctx)
            .expect("a driverless release names itself");
        assert!(reason.contains("profile.release.driver"), "{reason}");
    }

    /// Every prerequisite names a step that exists and comes earlier, so the
    /// full run can never be refused by its own table.
    #[test]
    fn every_prereq_is_an_earlier_step() {
        for (idx, step) in STEPS.iter().enumerate() {
            for prereq in step.prereqs {
                let position = STEPS
                    .iter()
                    .position(|other| other.name == *prereq)
                    .unwrap_or(usize::MAX);
                assert!(
                    position < idx,
                    "{}: prereq {prereq} is not an earlier step",
                    step.name
                );
            }
        }
        let reporting = &STEPS[STEPS.len() - 1];
        assert_eq!(reporting.name, "private-vulnerability-reporting");
        assert_eq!(reporting.chapter, "§3");
        assert_eq!(reporting.mutates, super::Mutates::Forge);
        assert!(!reporting.optional && !reporting.destructive);
        assert!(reporting.prereqs.is_empty());
        assert_eq!(STEPS[STEPS.len() - 2].name, "protections-check");
        assert!(spec("package-check").is_some());
        assert!(spec("no-such-step").is_none());
    }
}
