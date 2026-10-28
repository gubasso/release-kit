//! The resolved context one setup run works in: target, repository, forge,
//! the forge CLI binary, and the environment a step receives.
//!
//! The environment is constructed, not inherited: `env_clear` plus exactly
//! the declared variables, the forge CLI's own configuration and
//! authentication variables, and — only for the steps that need them — the
//! bot credentials. The parent's environment does not leak into a
//! privileged child, no secret is ever an argv value, and key material
//! reaches no environment at all: `rk` reads the key the operator named and
//! writes it to the step's standard input. [`super::secrets`] owns that
//! boundary.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;
use zeroize::Zeroizing;

use super::secrets;
use crate::detect::{self, Forge};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::profile::{CapabilityRequests, ProfileSnapshot, ReleaseMode};

// The trunk every setup asserts is the one permanent branch the target
// states in its own committed configuration, read through `Ctx::trunk`.
// A target that names none keeps the compiled default, so a landing
// predating the key behaves exactly as it did.

/// A policy boolean as the JSON word a forge body carries.
/// The compact JSON text of one answer, so the proof fields read alike
/// whatever type holds them. A value that cannot serialize reads as
/// empty, which no real answer is.
fn canonical<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

const fn bool_word(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// A policy list as the JSON array a forge body carries. Every element
/// passed the floor table, so this quotes rather than escapes.
fn json_list(values: &[String]) -> String {
    let inner: Vec<String> = values.iter().map(|value| format!("\"{value}\"")).collect();
    format!("[{}]", inner.join(", "))
}

/// The GitHub API objects named by the policy's stable bypass vocabulary.
///
/// GitHub's built-in repository-administrator role is actor 5. Keeping that
/// API encoding here lets configuration, diagnostics, and prose name intent
/// while the setup owns the forge representation in one place.
pub(super) fn github_bypass_actors(values: &[String]) -> serde_json::Value {
    serde_json::Value::Array(
        values
            .iter()
            .filter_map(|value| match value.as_str() {
                crate::config::LOCAL_GITHUB_BYPASS => Some(serde_json::json!({
                    "actor_id": 5,
                    "actor_type": "RepositoryRole",
                    "bypass_mode": "always",
                })),
                _ => None,
            })
            .collect(),
    )
}

/// One GitHub ruleset's `rules` array, as JSON.
///
/// Every rule comes from `protection.owned_trunk_rules`, which the floor
/// table judges per integration mode and the observer checks against, so
/// one key decides what a run installs and what the check expects. A rule
/// this convention parameterizes carries its parameters; the rest are
/// bare type entries.
///
/// `wanted` selects which of the owned rules this ruleset carries. The
/// trunk's rules live in two rulesets because a bypass actor is recorded
/// on a ruleset: the rules a bypass may excuse are kept apart from the
/// rules that must hold against everyone.
fn compose_rules(
    protection: &crate::config::Protection,
    wanted: &[&str],
    required_check: &str,
    title_check: &str,
) -> String {
    let rules: Vec<serde_json::Value> = protection
        .owned_trunk_rules
        .iter()
        .filter(|rule| wanted.iter().any(|kind| kind == &rule.as_str()))
        .map(|rule| match rule.as_str() {
            "pull_request" => serde_json::json!({
                "type": "pull_request",
                "parameters": {
                    "required_approving_review_count": protection.required_approving_review_count,
                    "dismiss_stale_reviews_on_push": protection.dismiss_stale_reviews_on_push,
                    "require_code_owner_review": protection.require_code_owner_review,
                    "require_last_push_approval": protection.require_last_push_approval,
                    "required_review_thread_resolution": false,
                    "require_extra_approval_for_unattributed_changes": false,
                    "allowed_merge_methods": protection.allowed_merge_methods,
                }
            }),
            "required_status_checks" => serde_json::json!({
                "type": "required_status_checks",
                "parameters": {
                    "do_not_enforce_on_create": true,
                    "strict_required_status_checks_policy":
                        protection.strict_required_status_checks,
                    "required_status_checks": [
                        { "context": required_check },
                        { "context": title_check },
                    ],
                }
            }),
            other => serde_json::json!({ "type": other }),
        })
        .collect();
    // Pretty rather than compact: the body a run sends is what an
    // operator reads back off the forge when a protection is in doubt.
    serde_json::to_string_pretty(&rules).unwrap_or_else(|_| "[]".to_owned())
}

/// One target's effective protection policy: what it stated, resolved
/// against the authority that carries an implementation onto its trunk.
///
/// A target that stated a policy gets its own values: the floor table
/// already judged them under the same mode, and an operator who narrowed
/// or widened something meant it. The one exception is the former compiled
/// local tuple: it remains readable so an upgrade can migrate it, but setup
/// must never reinstall its missing freshness rules. A target that stated nothing gets the
/// compiled defaults, which describe forge integration because that is
/// the shape this convention had before the axis existed — so under
/// local integration they are adjusted rather than installed.
///
/// Two adjustments, and both only where the target stated nothing. GitHub
/// names the repository-administrator role as the bypass authority while
/// retaining every rule, so a local integrator may push but the release App
/// may merge only the tested request. The GitLab push level moves off zero
/// to the narrowest level that still admits a push.
///
/// The merge values follow the recorded merge form whatever the target
/// stated, because the floors hold them to exactly that form's values:
/// GitHub allows the one method the form names, and GitLab squashes every
/// merge request under `squash` and none under `rebase`. Under `rebase`
/// the safety ruleset also carries the linear-history rule, so a merge
/// commit reaches the trunk from no actor.
///
/// The profile decides two requirements beside the policy. The tag
/// protection follows the release mode alone: a target that releases
/// nothing mints no tag to protect. The required status check and the
/// GitLab pipeline requirement follow the forge gate alone, as
/// [`crate::profile::catalog::forge_gate_lands`] answers it: where no gate
/// lands, no workflow reports a check and no pipeline runs, so the trunk
/// ruleset drops its required-check rule and the GitLab trunk drops the
/// pipeline requirement, while deletion and force-push hold in every case.
/// The floors still require the check rule in the stated key; the
/// relaxation lives here, in the one result every reader takes.
///
/// One resolution serves the body a step sends, the observer that reads
/// the answer back, and the prerequisites, so a canonical apply cannot
/// install one shape and then fault its own result for not being another.
///
/// SATISFIES target-config:the-effective-protection-follows-the-profile
fn effective_protection(
    stated: Option<&crate::config::Protection>,
    integration: crate::landing::Integration,
    form: crate::landing::MergeForm,
    mode: ReleaseMode,
) -> Effective {
    let authority = stated.map_or_else(
        || {
            if integration == crate::landing::Integration::Local {
                crate::config::local_protection()
            } else {
                crate::config::Protection::default()
            }
        },
        |stated| {
            if integration == crate::landing::Integration::Local
                && crate::config::legacy_local_protection(stated)
            {
                let mut migrated = stated.clone();
                let current = crate::config::local_protection();
                migrated.bypass_actors = current.bypass_actors;
                migrated.owned_trunk_rules = current.owned_trunk_rules;
                migrated.gitlab.push_access_level = current.gitlab.push_access_level;
                migrated
            } else {
                stated.clone()
            }
        },
    );
    let mut policy = crate::config::with_merge_form(&authority, form);
    if form == crate::landing::MergeForm::Rebase
        && !policy
            .owned_trunk_rules
            .iter()
            .any(|rule| rule == crate::config::LINEAR_HISTORY_RULE)
    {
        policy
            .owned_trunk_rules
            .push(crate::config::LINEAR_HISTORY_RULE.to_owned());
    }
    let forge_gate = crate::profile::catalog::forge_gate_lands(integration, mode);
    if !forge_gate {
        policy
            .owned_trunk_rules
            .retain(|rule| rule != REQUIRED_CHECK_RULE);
    }
    Effective {
        policy,
        tag_protection: mode != ReleaseMode::None,
        forge_gate,
    }
}

/// The ruleset rule that requires a status check, which the trunk carries
/// only where a forge gate reports one.
const REQUIRED_CHECK_RULE: &str = "required_status_checks";

/// The effective protection: the policy, and the two requirements the
/// profile decides beside it.
struct Effective {
    /// The policy every setter, observer, and check reads.
    policy: crate::config::Protection,
    /// Whether the release tags are protected: wherever the release mode
    /// is not `none`.
    tag_protection: bool,
    /// Whether a forge gate lands, so the trunk requires its check on
    /// GitHub and the whole pipeline on GitLab.
    forge_gate: bool,
}

/// The authority and the merge form a setup configures the forge for.
///
/// The record, not the resolution's compiled default: a setup configures a
/// forge to match what landed, and a target with no record has landed
/// nothing here. Forge integration and `squash` are what such a target
/// carries, the same compatibility answers `rk integrate` reads, and a
/// record predating the merge form reads as `squash` too.
fn recorded_workflow(
    record: Option<&crate::landing::manifest::Manifest>,
) -> (crate::landing::Integration, crate::landing::MergeForm) {
    record.map_or_else(
        || {
            (
                crate::landing::manifest::integration_forge(),
                crate::landing::manifest::merge_form_squash(),
            )
        },
        |held| (held.git.integration, held.git.merge_form),
    )
}

/// The context the landed forge gate reports under: the title check the
/// target names under `squash`, and the commit gate's job under `rebase`.
///
/// The commit gate's job name is a constant of its rendered file, as the
/// title job's is, and no key renames it: a renamed context would require
/// a check nothing reports.
fn gate_check(protection: &crate::config::Protection, form: crate::landing::MergeForm) -> String {
    match form {
        crate::landing::MergeForm::Squash => protection.title_check.clone(),
        crate::landing::MergeForm::Rebase => crate::config::COMMIT_GATE_CHECK.to_owned(),
    }
}

/// The variables that pass through from the operator's environment to a
/// step: the interpreter's search path, the forge CLI's configuration and
/// authentication, and nothing else.
const PASSTHROUGH: [&str; 11] = [
    "PATH",
    "HOME",
    "XDG_CONFIG_HOME",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_HOST",
    "GH_CONFIG_DIR",
    "GLAB_TOKEN",
    "GITLAB_TOKEN",
    "GITLAB_HOST",
    "GLAB_CONFIG_DIR",
];

/// The value-bearing bot variables, forwarded only to the steps that
/// consume them and recorded in the journal as handling, never as value.
/// The key is in no list here: it reaches its step as bytes on standard
/// input, and neither it nor its path is ever put in an environment.
/// [`secrets`] owns that.
pub use super::secrets::VALUE_VARS as SECRET_VARS;

/// One resolved run context.
#[derive(Debug, Clone)]
pub struct Ctx {
    /// The repository being set up.
    pub target: Utf8PathBuf,
    /// The project path on the forge, empty where the profile names none.
    pub repo: String,
    /// The forge adapter the run acts through, where the profile names a
    /// forge this release drives. A target with no forge, or one this
    /// release has no adapter for, carries none: its local steps still
    /// run and its forge steps report as not applicable.
    pub forge: Option<Forge>,
    /// The forge the profile declares, preserved whatever the adapter
    /// says, so an unknown name stays readable.
    pub declared_forge: Option<String>,
    /// What the project is, as the target configuration resolves it.
    pub profile: ProfileSnapshot,
    /// Which optional products the target requests.
    pub capabilities: CapabilityRequests,
    /// The remote host, where one was detected.
    pub host: Option<String>,
    /// The value of `--required-check`, where given.
    pub required_check: Option<String>,
    /// The committed `setup.required_workflow`, on GitHub alone. No flag
    /// answers it: the setup never writes it, it reads it to prove that
    /// the workflow the release gate waits on is the workflow that carries
    /// the check the gate judges.
    pub required_workflow: Option<String>,
    /// The resolved forge CLI binary.
    pub cli: PathBuf,
    /// The detected technology, where the version file names one.
    pub tech: Option<&'static str>,
    /// The one permanent branch this target states, or the compiled
    /// default where it states none.
    trunk: String,
    /// The release-line prefix this target states, or the compiled
    /// default where it states none.
    line_prefix: String,
    /// The long-lived branches a single trunk retires, as this target
    /// names them.
    retired_branches: Vec<String>,
    /// Whether a full apply runs the release-line protection, which a
    /// project that keeps no line does not want run at all.
    release_lines: bool,
    /// The steps this target declared it does not run, each against its
    /// stated reason. A run reports them and judges none of them.
    excluded_steps: std::collections::BTreeMap<String, String>,
    /// The bot App's public identifier where this target states one; the
    /// environment still wins over it, and no private credential is here.
    bot_app_id: Option<String>,
    /// The ruleset that protects the trunk, as this target names it.
    trunk_ruleset: String,
    /// The ruleset that keeps the trunk undeletable and unrewritable, as
    /// this target names it. It names no bypass actor.
    safety_ruleset: String,
    /// The ruleset that makes published tags immutable.
    tag_ruleset: String,
    /// The ruleset that protects the release lines.
    lines_ruleset: String,
    /// The context the landed forge gate reports under: the title job under
    /// `squash`, the commit gate's job under `rebase`.
    title_check: String,
    /// The effective policy this target runs under: what the target
    /// stated, resolved against the recorded integration mode. Every
    /// stated value passed the floor table at load, so a run passes this
    /// to a step without judging it again, and one resolution serves the
    /// body a step sends, the observer that reads the answer back, and
    /// every prerequisite — so a run cannot install one shape and then
    /// fault its own result for not being another.
    protection: crate::config::Protection,
    /// The destinations the landing record names, or `None` where nothing
    /// landed here. The package check claims them as release-kit's own.
    landed: Option<Vec<String>>,
    /// Which authority carries an implementation onto this target's trunk.
    /// A local-integration trunk takes the direct push that mode's
    /// integrations end in, so the protection a run installs is not the
    /// forge-integration one.
    integration: crate::landing::Integration,
    /// How the commits of a branch reach this target's trunk, as the record
    /// says: it decides the merge settings and the forge gate a run
    /// installs and expects.
    merge_form: crate::landing::MergeForm,
    /// Whether the release tags are protected, as the effective protection
    /// decides it from the release mode.
    pub(super) tag_protection: bool,
    /// Whether a forge gate lands, as the effective protection decides it:
    /// the trunk then requires the gate's check, and GitLab the pipeline.
    pub(super) forge_gate: bool,
}

impl Ctx {
    /// Resolve detection, overrides, and the forge CLI in one pass, before
    /// any step runs.
    ///
    /// # Errors
    ///
    /// Refuses when the target is missing, when no remote resolves and no
    /// override covers the gap, when the host is unrecognized, and when the
    /// forge CLI is not on `PATH`.
    pub fn resolve(
        target: &Utf8PathBuf,
        repo_flag: Option<&str>,
        forge_flag: Option<&str>,
        required_check: Option<&str>,
    ) -> Result<Self, RkError> {
        if !target.is_dir() {
            return Err(RkError::missing(
                Diagnostic::new(
                    Reason::TargetNotFound,
                    format!("target {target} is not a directory; nothing was run"),
                )
                .expected("an existing repository to set up"),
            ));
        }
        let forge_flag = forge_flag
            .map(|name| {
                detect::Forge::parse(name).ok_or_else(|| {
                    RkError::Usage(format!(
                        "unknown forge '{name}'; the forges are: github, gitlab"
                    ))
                })
            })
            .transpose()?;
        let detected = detect::detect(target.as_std_path());
        let config = crate::config::load(target.as_std_path())?;
        let record = crate::landing::manifest::load(target)?;
        // The setup reads the same target configuration a landing records,
        // so which steps apply follows the profile rather than a second
        // detection of its own.
        let resolved = crate::profile::Params::resolve(
            target,
            &crate::profile::Inputs {
                forge: forge_flag.map(Forge::as_str),
                repo: repo_flag,
                ..crate::profile::Inputs::default()
            },
            config.as_ref(),
            record.as_ref(),
            crate::profile::Purpose::Preview,
        )?;
        let declared_forge = resolved.forge().map(str::to_owned);
        let forge = declared_forge.as_deref().and_then(Forge::parse);
        let repo = resolved.repo().to_owned();
        let repo = if repo == crate::projection::REPO_PLACEHOLDER {
            String::new()
        } else {
            repo
        };
        // The forge CLI is not a prerequisite of building a context: it is
        // a prerequisite of the steps a run actually calls the forge for,
        // which `require_cli` resolves at that point.
        let cli = PathBuf::new();
        let answers = config
            .as_ref()
            .map_or_else(crate::config::Setup::default, |held| held.setup.clone());
        // The flag wins, and the committed answer fills the gap on GitHub
        // alone: GitLab names no individual check and refuses a supplied
        // one, so a shared configuration must not make that refusal fire.
        let required_check = required_check.map(str::to_owned).or_else(|| {
            Some(answers.required_check.clone())
                .filter(|name| !name.is_empty() && forge == Some(Forge::Github))
        });
        let required_workflow = Some(answers.required_workflow.clone())
            .filter(|name| !name.is_empty() && forge == Some(Forge::Github));
        let bot_app_id = Some(answers.bot.app_id.clone()).filter(|id| !id.is_empty());
        let trunk = resolved.trunk().to_owned();
        let (integration, merge_form) = recorded_workflow(record.as_ref());
        let landed = record.as_ref().map(|held| {
            held.files
                .iter()
                .map(|file| file.destination.clone())
                .collect()
        });
        let stated = config.as_ref().map(|held| &held.protection);
        let effective =
            effective_protection(stated, integration, merge_form, resolved.release_mode());
        let protection = effective.policy;
        Ok(Self {
            target: target.clone(),
            repo,
            forge,
            host: detected.host,
            required_check,
            required_workflow,
            cli,
            tech: resolved.driver().and_then(|driver| {
                ["rust", "python", "bash"]
                    .into_iter()
                    .find(|known| *known == driver)
            }),
            trunk_ruleset: protection.trunk_ruleset(&trunk),
            safety_ruleset: protection.safety_ruleset(&trunk),
            tag_ruleset: protection.tag_ruleset.clone(),
            lines_ruleset: protection.lines_ruleset.clone(),
            title_check: gate_check(&protection, merge_form),
            protection,
            tag_protection: effective.tag_protection,
            forge_gate: effective.forge_gate,
            integration,
            merge_form,
            landed,
            trunk,
            line_prefix: resolved.line_prefix().to_owned(),
            profile: resolved.profile().clone(),
            capabilities: resolved.capabilities().clone(),
            declared_forge,
            retired_branches: answers.retired_branches,
            release_lines: answers.release_lines,
            excluded_steps: answers.excluded_steps,
            bot_app_id,
        })
    }

    /// Resolve the forge CLI where this run will call the forge for one of
    /// `steps`, and refuse where it cannot be found.
    ///
    /// The prerequisite belongs to the call, not to the command. A step is
    /// only a caller when the target's configuration selects it, the
    /// target has not excluded it, and the step reaches the forge at this
    /// particular forge. A preview writes nothing, and asks anyway for the
    /// steps it would act on, because it names the command it would run
    /// and a CLI nothing could find is worth saying then.
    ///
    /// # Errors
    /// Propagates the refusal when the forge CLI cannot be resolved.
    ///
    /// SATISFIES forge-setup:applicability-follows-the-target-configuration
    pub fn require_cli(&mut self, steps: &[&crate::setup::steps::StepSpec]) -> Result<(), RkError> {
        let Some(forge) = self.forge else {
            return Ok(());
        };
        let calls = steps.iter().any(|step| {
            step.forge_cli.contains(&forge) && crate::setup::report::stance(self, step).acts()
        });
        if calls && self.cli.as_os_str().is_empty() {
            self.cli = resolve_cli(forge)?;
        }
        Ok(())
    }

    /// A context the integration tests build directly, for an observer
    /// exercised against recorded forge answers rather than a repository.
    /// The trunk and the prefix take their compiled defaults, because such
    /// a test reads no target configuration.
    #[doc(hidden)]
    #[must_use]
    pub fn for_tests(
        target: Utf8PathBuf,
        repo: String,
        forge: Forge,
        cli: PathBuf,
        tech: Option<&'static str>,
    ) -> Self {
        let defaults = crate::config::Protection::default();
        Self {
            // The forge-integration shape, which is what every observer
            // and request-body test here asserts; a local-mode case states it.
            integration: crate::landing::Integration::Forge,
            merge_form: crate::landing::MergeForm::Squash,
            tag_protection: true,
            forge_gate: true,
            landed: None,
            target,
            repo,
            forge: Some(forge),
            declared_forge: Some(forge.as_str().to_owned()),
            profile: ProfileSnapshot {
                technologies: tech.into_iter().map(str::to_owned).collect(),
                forge: Some(forge.as_str().to_owned()),
                release: crate::profile::ReleaseIntent {
                    mode: ReleaseMode::Automatic,
                    driver: tech.map(str::to_owned),
                    style: Some(crate::landing::Style::Trunk),
                    line_prefix: Some(crate::config::LINE_PREFIX_DEFAULT.to_owned()),
                },
            },
            capabilities: CapabilityRequests {
                nix_packaging: false,
                reporting_policy: true,
                scorecard: false,
                code_scanning: None,
                workflow_guards: Some(true),
            },
            host: None,
            required_check: None,
            required_workflow: None,
            cli,
            tech,
            trunk: crate::config::TRUNK_DEFAULT.to_owned(),
            line_prefix: crate::config::LINE_PREFIX_DEFAULT.to_owned(),
            retired_branches: crate::config::Setup::default().retired_branches,
            release_lines: false,
            excluded_steps: std::collections::BTreeMap::new(),
            bot_app_id: None,
            trunk_ruleset: format!("{}-protection", crate::config::TRUNK_DEFAULT),
            safety_ruleset: format!("{}-safety", crate::config::TRUNK_DEFAULT),
            tag_ruleset: defaults.tag_ruleset.clone(),
            lines_ruleset: defaults.lines_ruleset.clone(),
            title_check: defaults.title_check.clone(),
            protection: defaults,
        }
    }

    /// The destinations the landing record names, or `None` where nothing
    /// landed here.
    #[must_use]
    pub fn landed_destinations(&self) -> Option<&[String]> {
        self.landed.as_deref()
    }

    /// Whether the run has a forge adapter to act through.
    #[must_use]
    pub const fn has_adapter(&self) -> bool {
        self.forge.is_some()
    }

    /// The forge adapter, or the refusal a forge operation answers where
    /// the profile names no forge this release drives.
    ///
    /// # Errors
    ///
    /// A `prerequisite-unmet` refusal naming the declared forge and the
    /// ones this release drives.
    pub fn adapter(&self) -> Result<Forge, RkError> {
        self.forge.ok_or_else(|| {
            let named = self.declared_forge.as_deref();
            let message = named.map_or_else(
                || "the profile names no forge, and this operation acts on one".to_owned(),
                |name| {
                    format!(
                        "the profile names the forge {name}, which this release has no adapter for"
                    )
                },
            );
            RkError::refusal(
                Diagnostic::new(Reason::PrerequisiteUnmet, message)
                    .expected("a profile naming github or gitlab")
                    .action("set profile.forge in .release-kit/config.toml, or pass --forge <github|gitlab>")
                    .target_state("unchanged"),
            )
        })
    }

    /// Whether this target's release is one release-kit drives.
    #[must_use]
    pub const fn automatic_release(&self) -> bool {
        matches!(self.profile.release.mode, ReleaseMode::Automatic)
    }

    /// The release driver, where the profile names one.
    #[must_use]
    pub fn driver(&self) -> Option<&str> {
        self.profile.release.driver.as_deref()
    }

    /// The forge the profile declares, whatever the adapter says.
    #[must_use]
    pub fn declared_forge(&self) -> Option<&str> {
        self.declared_forge.as_deref()
    }

    /// Whether the target requested the landed reporting policy.
    #[must_use]
    pub const fn reporting_policy(&self) -> bool {
        self.capabilities.reporting_policy
    }

    /// The one permanent branch this run asserts.
    #[must_use]
    pub fn trunk(&self) -> &str {
        &self.trunk
    }

    /// The release-line prefix this run asserts.
    #[must_use]
    pub fn line_prefix(&self) -> &str {
        &self.line_prefix
    }

    /// The long-lived branches this run's single-trunk step retires.
    #[must_use]
    pub fn retired_branches(&self) -> &[String] {
        &self.retired_branches
    }

    /// Whether a full apply runs the release-line protection.
    #[must_use]
    pub const fn release_lines(&self) -> bool {
        self.release_lines
    }

    /// Why this target does not run the named step, where it declared an
    /// exclusion for it. The reason is what the report prints, so an
    /// excluded step is always visible with the answer behind it.
    #[must_use]
    pub fn excluded(&self, step: &str) -> Option<&str> {
        self.excluded_steps.get(step).map(String::as_str)
    }

    /// How many steps this target declared it does not run.
    #[must_use]
    pub fn excluded_count(&self) -> usize {
        self.excluded_steps.len()
    }

    /// The bot App's public identifier this target states, where it does.
    #[must_use]
    pub fn bot_app_id(&self) -> Option<&str> {
        self.bot_app_id.as_deref()
    }

    /// The ruleset that protects the trunk.
    #[must_use]
    pub fn trunk_ruleset(&self) -> &str {
        &self.trunk_ruleset
    }

    /// The ruleset that keeps the trunk undeletable and unrewritable.
    #[must_use]
    pub fn safety_ruleset(&self) -> &str {
        &self.safety_ruleset
    }

    /// The ruleset that makes published tags immutable.
    #[must_use]
    pub fn tag_ruleset(&self) -> &str {
        &self.tag_ruleset
    }

    /// The ruleset that protects the release lines.
    #[must_use]
    pub fn lines_ruleset(&self) -> &str {
        &self.lines_ruleset
    }

    /// The context the landed forge gate reports under: the title job under
    /// `squash`, the commit gate's job under `rebase`.
    #[must_use]
    pub fn title_check(&self) -> &str {
        &self.title_check
    }

    /// Which authority carries an implementation onto this trunk.
    #[must_use]
    pub const fn integration(&self) -> crate::landing::Integration {
        self.integration
    }

    /// How the commits of a branch reach this trunk, as the record says.
    #[must_use]
    pub const fn merge_form(&self) -> crate::landing::MergeForm {
        self.merge_form
    }

    /// Whether the release tags are protected: wherever the release mode
    /// is not `none`.
    #[must_use]
    pub const fn tag_protection(&self) -> bool {
        self.tag_protection
    }

    /// Whether a forge gate lands, so the trunk requires its check and
    /// GitLab requires the pipeline.
    #[must_use]
    pub const fn forge_gate(&self) -> bool {
        self.forge_gate
    }

    /// The safety ruleset's rule types, sorted, as the compact JSON array
    /// the protection check compares against the forge's answer.
    fn safety_rule_types(&self) -> String {
        let mut types: Vec<&str> = self
            .protection
            .owned_trunk_rules
            .iter()
            .map(String::as_str)
            .filter(|rule| crate::config::SAFETY_RULES.contains(rule))
            .collect();
        types.sort_unstable();
        types.dedup();
        serde_json::to_string(&types).unwrap_or_else(|_| "[]".to_owned())
    }

    /// The trunk ruleset's rules: the request and the check it carries,
    /// which the recorded bypass actors may be excused from.
    fn trunk_rules(&self) -> String {
        compose_rules(
            &self.protection,
            &crate::config::REQUEST_RULES,
            self.required_check.as_deref().unwrap_or_default(),
            &self.title_check,
        )
    }

    /// The safety ruleset's rules: what holds against every actor.
    fn safety_rules(&self) -> String {
        compose_rules(
            &self.protection,
            &crate::config::SAFETY_RULES,
            self.required_check.as_deref().unwrap_or_default(),
            &self.title_check,
        )
    }

    /// The setup answers a proof is judged against, by name: everything
    /// that decides which steps run and what each one asserts, and
    /// nothing about the host the run happened on. Each value is its
    /// canonical text, so two runs that resolve the same target produce
    /// the same map.
    ///
    /// SATISFIES setup-proof:the-proof-carries-no-secret-or-machine-coordinate
    #[must_use]
    pub fn proof_fields(&self) -> std::collections::BTreeMap<String, String> {
        let mut fields = std::collections::BTreeMap::new();
        let mut put = |key: &str, value: String| {
            fields.insert(key.to_owned(), value);
        };
        put("forge", self.declared_forge.clone().unwrap_or_default());
        put("host", self.host.clone().unwrap_or_default());
        put("repo", self.repo.clone());
        put("profile", canonical(&self.profile));
        put("capabilities", canonical(&self.capabilities));
        put("trunk", self.trunk.clone());
        put("line_prefix", self.line_prefix.clone());
        put("retired_branches", canonical(&self.retired_branches));
        put("release_lines", bool_word(self.release_lines).to_owned());
        put("integration", canonical(&self.integration));
        put(
            "required_check",
            self.required_check.clone().unwrap_or_default(),
        );
        put(
            "required_workflow",
            self.required_workflow.clone().unwrap_or_default(),
        );
        put("bot_app_id", self.bot_app_id.clone().unwrap_or_default());
        put("trunk_ruleset", self.trunk_ruleset.clone());
        put("safety_ruleset", self.safety_ruleset.clone());
        put("tag_ruleset", self.tag_ruleset.clone());
        put("lines_ruleset", self.lines_ruleset.clone());
        put("title_check", self.title_check.clone());
        put("protection", canonical(&self.protection));
        fields
    }

    /// The floored policy this target states, already judged at load.
    #[must_use]
    pub const fn protection(&self) -> &crate::config::Protection {
        &self.protection
    }

    /// Whether this run targets a GitLab instance that is not gitlab.com,
    /// where registry trusted publishing cannot reach.
    #[must_use]
    pub fn self_hosted_gitlab(&self) -> bool {
        self.forge == Some(Forge::Gitlab)
            && self
                .host
                .as_deref()
                .is_some_and(|host| host != "gitlab.com")
    }

    /// The constructed environment a step receives. Secrets enter only for
    /// the step that consumes them; the caller records their handling.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one pass builds the whole environment a step receives, and splitting it would separate a variable from the value it carries"
    )]
    pub fn child_env(&self, step: &str) -> Vec<(OsString, OsString)> {
        let mut env: Vec<(OsString, OsString)> = vec![
            (
                "RK_FORGE".into(),
                self.forge.map_or("", Forge::as_str).into(),
            ),
            ("RK_REPO".into(), self.repo.clone().into()),
            ("RK_TRUNK_BRANCH".into(), self.trunk.clone().into()),
            ("RK_LINE_PREFIX".into(), self.line_prefix.clone().into()),
            ("RK_TRUNK_RULESET".into(), self.trunk_ruleset.clone().into()),
            (
                "RK_SAFETY_RULESET".into(),
                self.safety_ruleset.clone().into(),
            ),
            ("RK_TAG_RULESET".into(), self.tag_ruleset.clone().into()),
            ("RK_LINES_RULESET".into(), self.lines_ruleset.clone().into()),
            // The context the landed forge gate reports under, which the
            // trunk ruleset requires: the title job under `squash` and the
            // commit gate's job under `rebase`.
            ("RK_TITLE_CHECK".into(), self.title_check.clone().into()),
            // The recorded merge form, so a step asserts the squash
            // settings only where a squash merge reads them.
            ("RK_MERGE_FORM".into(), self.merge_form.as_str().into()),
            // The two requirements the profile decides: the release tags
            // are protected wherever a release mints them, and the trunk
            // requires the gate's check, or GitLab's pipeline, wherever a
            // forge gate lands to report it.
            (
                "RK_TAG_PROTECTION".into(),
                bool_word(self.tag_protection).into(),
            ),
            ("RK_FORGE_GATE".into(), bool_word(self.forge_gate).into()),
            // The floored policy, already judged against the floor table
            // at load. A step receives values, never a judgment: one
            // owner decides what passes, and it is not a shell script.
            (
                "RK_TAG_PATTERN".into(),
                self.protection.tag_pattern.clone().into(),
            ),
            (
                "RK_REVIEW_COUNT".into(),
                self.protection
                    .required_approving_review_count
                    .to_string()
                    .into(),
            ),
            (
                "RK_DISMISS_STALE_REVIEWS".into(),
                bool_word(self.protection.dismiss_stale_reviews_on_push).into(),
            ),
            (
                "RK_CODE_OWNER_REVIEW".into(),
                bool_word(self.protection.require_code_owner_review).into(),
            ),
            (
                "RK_LAST_PUSH_APPROVAL".into(),
                bool_word(self.protection.require_last_push_approval).into(),
            ),
            (
                "RK_MERGE_METHODS".into(),
                json_list(&self.protection.allowed_merge_methods).into(),
            ),
            (
                "RK_STRICT_CHECKS".into(),
                bool_word(self.protection.strict_required_status_checks).into(),
            ),
            (
                "RK_BYPASS_ACTORS".into(),
                github_bypass_actors(&self.protection.bypass_actors)
                    .to_string()
                    .into(),
            ),
            (
                "RK_SQUASH_TITLE_SOURCE".into(),
                self.protection.github.squash_title_source.clone().into(),
            ),
            (
                "RK_SQUASH_BODY_SOURCE".into(),
                self.protection.github.squash_body_source.clone().into(),
            ),
            (
                "RK_GITLAB_MERGE_METHOD".into(),
                self.protection.gitlab.merge_method.clone().into(),
            ),
            (
                "RK_GITLAB_SQUASH_OPTION".into(),
                self.protection.gitlab.squash_option.clone().into(),
            ),
            (
                "RK_GITLAB_SQUASH_TEMPLATE".into(),
                self.protection.gitlab.squash_commit_template.clone().into(),
            ),
            (
                "RK_GITLAB_PUSH_LEVEL".into(),
                self.protection.gitlab.push_access_level.to_string().into(),
            ),
            // The two rulesets' rules, built from the one key the floor
            // table judges and the observer reads, so the body a run
            // sends cannot install a rule the check does not expect, or
            // omit one it does. The split is what a bypass costs: an
            // actor excused from the trunk ruleset is excused from every
            // rule in it, so the rules that must hold against everyone
            // live in the safety ruleset, which names nobody.
            ("RK_TRUNK_RULES".into(), self.trunk_rules().into()),
            ("RK_SAFETY_RULES".into(), self.safety_rules().into()),
            (
                "RK_SAFETY_RULE_TYPES".into(),
                self.safety_rule_types().into(),
            ),
            (
                "RK_GITLAB_MERGE_LEVEL".into(),
                self.protection.gitlab.merge_access_level.to_string().into(),
            ),
            ("GH_PAGER".into(), "".into()),
            ("GLAB_PAGER".into(), "".into()),
        ];
        if let Some(check) = &self.required_check
            && self.forge == Some(Forge::Github)
            && self.forge_gate
            && matches!(step, "protect-trunk" | "protections-check")
        {
            env.push(("RK_REQUIRED_CHECK".into(), check.clone().into()));
        }
        for name in PASSTHROUGH {
            if let Some(value) = std::env::var_os(name) {
                env.push((name.into(), value));
            }
        }
        // The forge CLI override substitutes the binary for the run's own
        // calls; a step resolves the CLI by name, so the override's
        // directory leads the child's search path.
        if let Some(dir) = self.cli_override_dir() {
            let mut paths: Vec<PathBuf> = vec![dir];
            if let Some(existing) = std::env::var_os("PATH") {
                paths.extend(std::env::split_paths(&existing));
            }
            if let Ok(joined) = std::env::join_paths(paths) {
                env.retain(|(name, _)| name != "PATH");
                env.push(("PATH".into(), joined));
            }
        }
        if step == "bot-secrets" {
            for name in SECRET_VARS {
                if let Some(value) = secrets::value_of(name) {
                    env.push((name.into(), value));
                }
            }
        }
        env
    }

    /// The directory of an explicitly overridden forge CLI, where one is set.
    fn cli_override_dir(&self) -> Option<PathBuf> {
        let overridden = std::env::var_os(match self.forge? {
            Forge::Github => "RK_GH_BIN",
            Forge::Gitlab => "RK_GLAB_BIN",
        })?;
        Path::new(&overridden).parent().map(Path::to_path_buf)
    }

    /// The secret bytes a run must keep out of its own output: the values
    /// the environment carries. Every buffer is scrubbed on drop; none is
    /// ever logged or echoed.
    ///
    /// Key material is not read here. The step that transmits a key adds
    /// the very bytes it sends, so the needle cannot describe one file
    /// while the child receives another.
    #[must_use]
    pub fn secret_values() -> Vec<Zeroizing<Vec<u8>>> {
        SECRET_VARS
            .iter()
            .filter_map(|name| secrets::value_of(name))
            .map(|value| Zeroizing::new(value.into_encoded_bytes()))
            .collect()
    }
}

/// Resolve the forge CLI once, at context time: the `RK_GH_BIN` and
/// `RK_GLAB_BIN` overrides first, then a `PATH` search.
///
/// Not found and not executable are distinct failures, in the shell
/// convention. `rk branches prune` shares it for the verify path.
///
/// # Errors
///
/// Refuses when the override or the search resolves no usable binary.
pub fn resolve_cli(forge: Forge) -> Result<PathBuf, RkError> {
    let override_var = match forge {
        Forge::Github => "RK_GH_BIN",
        Forge::Gitlab => "RK_GLAB_BIN",
    };
    if let Some(overridden) = std::env::var_os(override_var).filter(|v| !v.is_empty()) {
        let path = PathBuf::from(&overridden);
        if !path.is_file() {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    format!(
                        "{override_var} names {}, which does not exist",
                        path.display()
                    ),
                )
                .expected("the override to name the forge CLI binary"),
            ));
        }
        // The scripts invoke the CLI by its canonical name through the
        // child's search path, so an override under any other name would
        // split one lifecycle across two binaries: observed through the
        // override, applied through whatever the name resolves to.
        if path.file_name().is_none_or(|name| name != forge.cli()) {
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    format!(
                        "{override_var} must name a binary called {}, and {} is not one",
                        forge.cli(),
                        path.display()
                    ),
                )
                .expected(format!(
                    "an override whose file name is {}, so scripts and observations run one binary",
                    forge.cli()
                )),
            ));
        }
        return Ok(path);
    }
    let name = forge.cli();
    let found = std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    });
    found.ok_or_else(|| {
        RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!(
                    "{name} is not on PATH, and a step this run acts on calls it on {}",
                    forge.as_str()
                ),
            )
            .expected(format!("the {name} CLI installed and authenticated"))
            .action(format!("install {name}, then run {name} auth login")),
        )
    })
}

#[cfg(test)]
mod tests {
    /// Both rulesets' rules come from the one owned-rules key, so what a
    /// run installs, what the floor table judges, and what the check
    /// expects cannot disagree. The split is what a bypass costs: a
    /// recorded actor is excused from every rule in the ruleset it names,
    /// so the rules that hold against everyone are composed apart, and a
    /// target under either authority composes the same two halves.
    #[test]
    fn each_ruleset_composes_its_half_of_the_owned_rule_key() {
        let kinds = |text: &str| -> Vec<String> {
            let parsed: Vec<serde_json::Value> =
                serde_json::from_str(text).expect("the rules parse");
            parsed
                .iter()
                .filter_map(|rule| rule["type"].as_str())
                .map(str::to_owned)
                .collect()
        };
        let mut policy = crate::config::Protection::default();
        let request =
            super::compose_rules(&policy, &crate::config::REQUEST_RULES, "gate", "pr-title");
        assert_eq!(kinds(&request), ["pull_request", "required_status_checks"]);
        let safety =
            super::compose_rules(&policy, &crate::config::SAFETY_RULES, "gate", "pr-title");
        assert_eq!(kinds(&safety), ["deletion", "non_fast_forward"]);

        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&request).expect("the rules parse");
        let checks = parsed
            .iter()
            .find(|rule| rule["type"] == "required_status_checks")
            .expect("the check rule");
        assert_eq!(
            checks["parameters"]["required_status_checks"],
            serde_json::json!([{ "context": "gate" }, { "context": "pr-title" }])
        );

        // The recorded bypass changes who is excused, never which rules
        // each ruleset carries.
        policy.bypass_actors = vec![crate::config::LOCAL_GITHUB_BYPASS.into()];
        assert_eq!(
            kinds(&super::compose_rules(
                &policy,
                &crate::config::REQUEST_RULES,
                "gate",
                "pr-title"
            )),
            ["pull_request", "required_status_checks"]
        );
        assert_eq!(
            kinds(&super::compose_rules(
                &policy,
                &crate::config::SAFETY_RULES,
                "gate",
                "pr-title"
            )),
            ["deletion", "non_fast_forward"]
        );
    }

    /// One effective policy serves the body a step sends, the observer
    /// that reads the answer back, and every prerequisite.
    ///
    /// A target that stated nothing gets the compiled defaults, which
    /// describe forge integration because that is the shape this
    /// convention had before the axis existed — so under local
    /// integration they are adjusted: the GitHub administrator role gains
    /// the bypass that admits the direct push, and the GitLab level moves
    /// off the zero that would close the trunk to that push. A
    /// target that stated a policy keeps every value it stated, because
    /// the floor table already judged it under the same mode. The former
    /// compiled local tuple is migrated before setup can reinstall it.
    #[test]
    fn the_effective_policy_follows_the_recorded_authority() {
        use crate::landing::{Integration, MergeForm};
        let silent = super::effective_protection(
            None,
            Integration::Forge,
            MergeForm::Squash,
            crate::profile::ReleaseMode::Automatic,
        )
        .policy;
        assert!(
            silent
                .owned_trunk_rules
                .contains(&"pull_request".to_owned())
        );
        assert_eq!(silent.gitlab.push_access_level, 0);

        let silent = super::effective_protection(
            None,
            Integration::Local,
            MergeForm::Squash,
            crate::profile::ReleaseMode::Automatic,
        )
        .policy;
        assert_eq!(
            silent.owned_trunk_rules,
            crate::config::Protection::default().owned_trunk_rules,
            "local integration retains the release request's atomic check"
        );
        assert_eq!(
            silent.bypass_actors,
            [crate::config::LOCAL_GITHUB_BYPASS.to_owned()]
        );
        assert_eq!(
            silent.gitlab.push_access_level, 40,
            "zero would close the trunk to the push this mode ends in"
        );

        let mut stated = crate::config::Protection::default();
        stated.gitlab.push_access_level = 0;
        stated.owned_trunk_rules = vec!["deletion".into()];
        let held = super::effective_protection(
            Some(&stated),
            Integration::Local,
            MergeForm::Squash,
            crate::profile::ReleaseMode::Automatic,
        )
        .policy;
        assert_eq!(held.gitlab.push_access_level, 0, "a stated value wins");
        assert_eq!(held.owned_trunk_rules, ["deletion".to_owned()]);

        let legacy = crate::config::Protection {
            owned_trunk_rules: vec!["deletion".into(), "non_fast_forward".into()],
            gitlab: crate::config::Gitlab {
                push_access_level: 40,
                ..crate::config::Gitlab::default()
            },
            ..crate::config::Protection::default()
        };
        let migrated = super::effective_protection(
            Some(&legacy),
            Integration::Local,
            MergeForm::Squash,
            crate::profile::ReleaseMode::Automatic,
        )
        .policy;
        assert_eq!(
            migrated,
            crate::config::local_protection(),
            "setup cannot reinstall the legacy policy while upgrade remains able to read it"
        );
    }

    /// The merge values follow the recorded form under both authorities,
    /// whether the target stated a policy or not, and only the rebase form
    /// adds the linear-history rule, to the safety half.
    ///
    /// SATISFIES target-config:the-effective-protection-follows-the-profile
    #[test]
    fn merge_form_decides_the_effective_merge_values() {
        use crate::landing::{Integration, MergeForm};
        let kinds = |text: &str| -> Vec<String> {
            let parsed: Vec<serde_json::Value> =
                serde_json::from_str(text).expect("the rules parse");
            parsed
                .iter()
                .filter_map(|rule| rule["type"].as_str())
                .map(str::to_owned)
                .collect()
        };
        let stated = crate::config::Protection::default();
        for integration in [Integration::Forge, Integration::Local] {
            for held in [None, Some(&stated)] {
                let squash = super::effective_protection(
                    held,
                    integration,
                    MergeForm::Squash,
                    crate::profile::ReleaseMode::Automatic,
                )
                .policy;
                assert_eq!(squash.allowed_merge_methods, ["squash".to_owned()]);
                assert_eq!(squash.gitlab.squash_option, "always");
                assert!(
                    !squash
                        .owned_trunk_rules
                        .contains(&crate::config::LINEAR_HISTORY_RULE.to_owned())
                );
                assert_eq!(
                    kinds(&super::compose_rules(
                        &squash,
                        &crate::config::SAFETY_RULES,
                        "gate",
                        "pr-title"
                    )),
                    ["deletion", "non_fast_forward"]
                );

                let rebase = super::effective_protection(
                    held,
                    integration,
                    MergeForm::Rebase,
                    crate::profile::ReleaseMode::Automatic,
                )
                .policy;
                assert_eq!(rebase.allowed_merge_methods, ["rebase".to_owned()]);
                assert_eq!(rebase.gitlab.squash_option, "never");
                assert_eq!(rebase.gitlab.merge_method, "ff");
                assert_eq!(
                    kinds(&super::compose_rules(
                        &rebase,
                        &crate::config::SAFETY_RULES,
                        "gate",
                        "pr-commits"
                    )),
                    ["deletion", "non_fast_forward", "required_linear_history"]
                );
                assert_eq!(
                    kinds(&super::compose_rules(
                        &rebase,
                        &crate::config::REQUEST_RULES,
                        "gate",
                        "pr-commits"
                    )),
                    ["pull_request", "required_status_checks"],
                    "the linear-history rule never sits where a bypass excuses it"
                );
            }
        }
    }

    /// The required context is the gate the form lands.
    #[test]
    fn merge_form_names_the_required_gate() {
        use crate::landing::MergeForm;
        let policy = crate::config::Protection::default();
        assert_eq!(super::gate_check(&policy, MergeForm::Squash), "pr-title");
        assert_eq!(super::gate_check(&policy, MergeForm::Rebase), "pr-commits");
    }
    /// The effective protection decides the two requirements from the
    /// profile: the tag protection follows the release mode alone, and the
    /// required check, carried by the trunk ruleset's check rule, follows
    /// the forge gate alone. Deletion and force-push hold in every cell.
    ///
    /// SATISFIES target-config:the-effective-protection-follows-the-profile
    #[test]
    fn release_less_effective_protection_follows_the_profile() {
        use crate::landing::{Integration, MergeForm};
        use crate::profile::ReleaseMode;
        for integration in [Integration::Forge, Integration::Local] {
            for form in [MergeForm::Squash, MergeForm::Rebase] {
                for mode in [
                    ReleaseMode::Automatic,
                    ReleaseMode::External,
                    ReleaseMode::None,
                ] {
                    let effective = super::effective_protection(None, integration, form, mode);
                    let at = format!("{integration:?} {form:?} {mode:?}");
                    let gate = integration == Integration::Forge || mode == ReleaseMode::Automatic;
                    assert_eq!(effective.tag_protection, mode != ReleaseMode::None, "{at}");
                    assert_eq!(effective.forge_gate, gate, "{at}");
                    let rules = &effective.policy.owned_trunk_rules;
                    assert_eq!(
                        rules.iter().any(|rule| rule == "required_status_checks"),
                        gate,
                        "{at}: {rules:?}"
                    );
                    for kept in ["deletion", "non_fast_forward", "pull_request"] {
                        assert!(rules.iter().any(|rule| rule == kept), "{at}: {kept}");
                    }
                    assert_eq!(
                        effective.policy.allowed_merge_methods,
                        [form.as_str().to_owned()],
                        "{at}"
                    );
                }
            }
        }
    }
}
