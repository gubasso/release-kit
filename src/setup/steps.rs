//! The ordered step table: one step per executable item in
//! `method/02-setup.md`, in the chapter's order, each defined by what it
//! proves rather than by how a forge achieves it.

/// What a step touches when it applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mutates {
    /// The step only reads and reports.
    Nothing,
    /// The step writes forge configuration.
    Forge,
}

/// One step of the setup, in canonical order.
#[derive(Debug)]
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
    /// Steps that must be observed satisfied before this one applies.
    pub prereqs: &'static [&'static str],
}

/// The eleven steps, in the chapter's order. `package-check` belongs to no
/// forge tree — it reads its command from the technology binding — which
/// makes it the one step outside the parity rule.
pub const STEPS: [StepSpec; 11] = [
    StepSpec {
        name: "package-check",
        chapter: "§0",
        mutates: Mutates::Nothing,
        proves: "the package is publishable with no credentials",
        destructive: false,
        optional: false,
        prereqs: &[],
    },
    StepSpec {
        name: "default-branch",
        chapter: "§1",
        mutates: Mutates::Forge,
        proves: "the trunk is the default branch",
        destructive: false,
        optional: false,
        prereqs: &[],
    },
    StepSpec {
        name: "single-trunk",
        chapter: "§1",
        mutates: Mutates::Forge,
        proves: "no long-lived branch besides the trunk remains",
        destructive: true,
        optional: false,
        prereqs: &["default-branch"],
    },
    StepSpec {
        name: "merge-cleanup",
        chapter: "§1",
        mutates: Mutates::Forge,
        proves: "the forge deletes a branch when its merge lands",
        destructive: false,
        optional: false,
        prereqs: &["default-branch"],
    },
    StepSpec {
        name: "ci-permissions",
        chapter: "§2",
        mutates: Mutates::Forge,
        proves: "CI may write and open requests",
        destructive: false,
        optional: false,
        prereqs: &[],
    },
    StepSpec {
        name: "install-bot",
        chapter: "§2",
        mutates: Mutates::Forge,
        proves: "the bot identity can act on this project",
        destructive: false,
        optional: false,
        prereqs: &[],
    },
    StepSpec {
        name: "bot-secrets",
        chapter: "§2",
        mutates: Mutates::Forge,
        proves: "the bot credentials are stored on the project",
        destructive: false,
        optional: false,
        prereqs: &[],
    },
    StepSpec {
        name: "protect-trunk",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "the trunk takes no direct push, merges only by squash, and requires the named check",
        destructive: false,
        optional: false,
        prereqs: &["default-branch"],
    },
    StepSpec {
        name: "protect-tags",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "v* is protected as far as the forge allows",
        destructive: false,
        optional: false,
        prereqs: &[],
    },
    StepSpec {
        name: "protect-release-lines",
        chapter: "§3",
        mutates: Mutates::Forge,
        proves: "release/* cannot be force-pushed or deleted",
        destructive: false,
        optional: true,
        prereqs: &[],
    },
    StepSpec {
        name: "protections-check",
        chapter: "§3",
        mutates: Mutates::Nothing,
        proves: "exactly the owned protections, with those rules",
        destructive: false,
        optional: false,
        prereqs: &[],
    },
];

/// Look one step up by name.
#[must_use]
pub fn spec(name: &str) -> Option<&'static StepSpec> {
    STEPS.iter().find(|step| step.name == name)
}

#[cfg(test)]
mod tests {
    use super::{STEPS, spec};

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
        assert!(spec("package-check").is_some());
        assert!(spec("no-such-step").is_none());
    }
}
