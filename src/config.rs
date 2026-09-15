//! The committed target configuration, parsed strictly and written from
//! authored text, typed by the domain that owns each answer.
//! Comparisons continue to use the landing record alone.
//!
//! SATISFIES project-profile:the-target-configuration-is-typed-by-domain

pub mod floors;
pub mod migrate;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::{CheckoutMode, Integration, Style};
use crate::profile::ReleaseMode;
use serde::Deserialize;

/// The committed input, relative to the target root.
pub const CONFIG_PATH: &str = ".release-kit/config.toml";
/// The configuration schema this binary writes. Schema 1 reads through
/// the one migration in [`migrate`].
pub const SCHEMA_VERSION: i64 = 2;

/// Per-target answers; an omitted table uses its compiled defaults.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Version of the authored configuration shape.
    pub schema_version: i64,
    /// Project identity.
    pub project: Project,
    /// What the project is.
    pub profile: Profile,
    /// How topic branches reach the trunk.
    pub git: Git,
    /// Which optional products the target requests.
    pub capabilities: Capabilities,
    /// Report-routing facts and the two policy answers.
    pub security: Security,
    /// Forge setup inputs and the setup declaration.
    pub setup: Setup,
    /// Names and floored policy.
    pub protection: Protection,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project: Project::default(),
            profile: Profile::default(),
            git: Git::default(),
            capabilities: Capabilities::default(),
            security: Security::default(),
            setup: Setup::default(),
            protection: Protection::default(),
        }
    }
}

/// The `project` table: identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Project {
    /// P: project path on the forge, nested groups included. Empty where
    /// the project has no forge repository.
    pub repo: String,
}

/// The `profile` table: what the project is.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Profile {
    /// P: every technology present, zero or many. Absent means detect.
    pub technologies: Option<Vec<String>>,
    /// P: the forge. Absent means detect from the remote; empty states
    /// that the project has no forge.
    pub forge: Option<String>,
    /// P: the release intent.
    pub release: Release,
}

/// The `profile.release` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Release {
    /// P: automatic, external, or none. Absent means the observation
    /// proposes one.
    pub mode: Option<ReleaseMode>,
    /// P: the technology that states the version and takes the bot;
    /// automatic alone.
    pub driver: Option<String>,
    /// P: trunk or lines; automatic alone.
    pub style: Option<Style>,
    /// P: release-line branch prefix; automatic alone.
    pub line_prefix: Option<String>,
}

/// The `git` table: the Git workflow parameters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Git {
    /// P: the one permanent branch, rendered into every landed artifact
    /// that names it. Absent means the landing has not answered it, so a
    /// record's own answer survives an upgrade that predates the key.
    pub trunk: Option<String>,
    /// P: linked-worktree or main-worktree.
    pub checkout_mode: Option<CheckoutMode>,
    /// P: local or forge, the authority that moves an implementation
    /// onto the trunk.
    pub integration: Option<Integration>,
}

/// The `capabilities` table: the optional products the target requests.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Capabilities {
    /// P: opt-in Nix capability.
    pub nix_packaging: Option<bool>,
    /// P: the landed vulnerability reporting policy.
    pub reporting_policy: Option<bool>,
    /// P: opt-in Scorecard capability.
    pub scorecard: Option<bool>,
    /// P: opt-in code scanning provider, as `codeql`, `semgrep`, or `off`.
    /// The value stays a string here so an absent key and an explicit `off`
    /// stay distinguishable; the resolution parses it.
    pub code_scanning: Option<String>,
}

/// The compiled trunk, used where neither a configuration nor a record answers.
pub const TRUNK_DEFAULT: &str = "master";

/// The compiled release-line prefix, used where nothing else answers.
pub const LINE_PREFIX_DEFAULT: &str = "release/";

/// The `security` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Security {
    /// N: project receiving vulnerability reports.
    pub advisories: String,
    /// P: contact when the forge channel is unavailable, rendered into the
    /// landed policy. Absent means the landing has not answered it, so a
    /// record's own answer survives an upgrade that predates the key; an
    /// explicit empty string resets the policy to the forge's own prose.
    pub contact: Option<String>,
    /// P: the acknowledgment window the landed policy promises. Absent
    /// means unanswered, exactly as `contact` does.
    pub response: Option<String>,
}

/// The compiled response stance, used where nothing else answers: the
/// policy promises no window at all.
pub const RESPONSE_DEFAULT: &str = "best-effort";

/// The canonical form of a security contact, or why it is refused.
///
/// One trimmed line. The value is rendered into `SECURITY.md` verbatim, so
/// a line feed, a carriage return, or any other ASCII control character
/// would break the sentence it lands in and is refused before any write.
/// Emptiness is not a refusal: it selects the forge's own authored prose.
///
/// # Errors
/// The refusal text, naming the key and what it accepts.
pub fn canonical_contact(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.chars().any(char::is_control) {
        return Err(format!(
            "security.contact carries a control character; it is one line naming an address, a URL, a person, or a team, and empty selects the forge's own wording, found {trimmed:?}"
        ));
    }
    Ok(trimmed.to_owned())
}

/// The canonical form of a response stance, or why it is refused.
///
/// Either `best-effort` or a plural-correct day count: `1 day`, `<n> days`,
/// `1 business day`, or `<n> business days`, with `n` a `u32` above one
/// written without a sign or a leading zero. The grammar is narrow because
/// the rendered sentence is a public promise, and only a value this
/// renderer can state exactly may reach it. An empty value reads as the
/// compiled default.
///
/// # Errors
/// The refusal text, naming the key and every accepted form.
pub fn canonical_response(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == RESPONSE_DEFAULT {
        return Ok(RESPONSE_DEFAULT.to_owned());
    }
    let refusal = || {
        format!(
            "security.response must be one of: best-effort, 1 day, <n> days, 1 business day, <n> business days, where n is a whole number above one; found {trimmed:?}"
        )
    };
    let (count, unit) = trimmed.split_once(' ').ok_or_else(refusal)?;
    let plural = match unit {
        "day" | "business day" => false,
        "days" | "business days" => true,
        _ => return Err(refusal()),
    };
    let number: u32 = count.parse().map_err(|_| refusal())?;
    // A canonical count round-trips, which refuses a sign and a leading
    // zero without a second pass over the text.
    if count != number.to_string() || (number > 1) != plural {
        return Err(refusal());
    }
    Ok(trimmed.to_owned())
}

/// The `setup` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Setup {
    /// N: the check the merge must pass.
    pub required_check: String,
    /// N: long-lived branches retired by the trunk.
    pub retired_branches: Vec<String>,
    /// N: run release-line protection in a full apply.
    pub release_lines: bool,
    /// N: the steps this target does not run, each against the reason a
    /// report prints. An exclusion narrows what the setup judges and
    /// weakens no floor: every value a step the target still runs reads is
    /// floored exactly as before.
    pub excluded_steps: BTreeMap<String, String>,
    /// Public bot identity.
    pub bot: Bot,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            required_check: String::new(),
            retired_branches: vec!["main".into(), "develop".into()],
            release_lines: false,
            excluded_steps: BTreeMap::new(),
            bot: Bot::default(),
        }
    }
}

/// The `setup.bot` table.
///
/// The App's public identifier and nothing else. The installation id is
/// not here: it is the forge's own state, one cheap call answers it, and a
/// cached copy that goes stale buys a refusal the operator must resolve by
/// hand. The private key and the token are never here at all.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bot {
    /// N: public App identifier; private credentials stay outside this file.
    pub app_id: String,
    /// Accepted and ignored. Version 0.3.13 wrote this key, so a target
    /// landed by it must still parse; nothing reads the value and no new
    /// configuration carries it. Removing it outright would refuse every
    /// such target, because this reader denies an unknown key by design.
    #[serde(default, skip_serializing)]
    pub installation_id: Option<i64>,
}

/// The `protection` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "these are independent policy switches in the committed TOML schema, not a state machine a smaller type could carry"
)]
pub struct Protection {
    /// N: trunk ruleset name. Absent derives `<trunk>-protection`, which
    /// is the name the setup script built before the key existed, so a
    /// target that states none keeps the ruleset it already has.
    pub trunk_ruleset: Option<String>,
    /// N: tag ruleset name.
    pub tag_ruleset: String,
    /// N: release-line ruleset name.
    pub lines_ruleset: String,
    /// N: title job context.
    pub title_check: String,
    /// F: invariant, covers every published version.
    pub tag_pattern: String,
    /// F: invariant, empty.
    pub bypass_actors: Vec<String>,
    /// F: invariant, exactly squash.
    pub allowed_merge_methods: Vec<String>,
    /// F: invariant, true.
    pub strict_required_status_checks: bool,
    /// F: invariant, the rules `git.integration` names. A landing writes
    /// this key with the authority, and a target that narrowed or
    /// widened it keeps what it stated.
    pub owned_trunk_rules: Vec<String>,
    /// F: floor zero; higher is stricter.
    pub required_approving_review_count: i64,
    /// F: floor false; true is stricter.
    pub dismiss_stale_reviews_on_push: bool,
    /// F: floor false; true is stricter.
    pub require_code_owner_review: bool,
    /// F: floor false; true is stricter.
    pub require_last_push_approval: bool,
    /// GitHub policy.
    pub github: Github,
    /// GitLab policy.
    pub gitlab: Gitlab,
}

impl Default for Protection {
    fn default() -> Self {
        Self {
            trunk_ruleset: None,
            tag_ruleset: "release-tags".into(),
            lines_ruleset: "release-lines".into(),
            title_check: "pr-title".into(),
            tag_pattern: "refs/tags/v*".into(),
            bypass_actors: Vec::new(),
            allowed_merge_methods: vec!["squash".into()],
            strict_required_status_checks: true,
            owned_trunk_rules: vec![
                "deletion".into(),
                "non_fast_forward".into(),
                "pull_request".into(),
                "required_status_checks".into(),
            ],
            required_approving_review_count: 0,
            dismiss_stale_reviews_on_push: false,
            require_code_owner_review: false,
            require_last_push_approval: false,
            github: Github::default(),
            gitlab: Gitlab::default(),
        }
    }
}

impl Protection {
    /// The trunk ruleset's name: the target's own answer, or the name the
    /// setup script derived before the key existed.
    #[must_use]
    pub fn trunk_ruleset(&self, trunk: &str) -> String {
        self.trunk_ruleset
            .clone()
            .unwrap_or_else(|| format!("{trunk}-protection"))
    }
}

/// The `protection.github` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Github {
    /// F: invariant, `PR_TITLE`.
    pub squash_title_source: String,
    /// F: invariant, `PR_BODY`.
    pub squash_body_source: String,
}

impl Default for Github {
    fn default() -> Self {
        Self {
            squash_title_source: "PR_TITLE".into(),
            squash_body_source: "PR_BODY".into(),
        }
    }
}

/// The `protection.gitlab` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Gitlab {
    /// F: invariant, linear history.
    pub merge_method: String,
    /// F: invariant, always squash.
    pub squash_option: String,
    /// F: invariant, references title and description.
    pub squash_commit_template: String,
    /// F: invariant, the level `git.integration` names: zero under forge
    /// integration, which takes no push at all, and the narrowest level
    /// that admits one under local integration, whose integrations end in
    /// that push.
    pub push_access_level: i64,
    /// F: floor thirty.
    pub merge_access_level: i64,
}

impl Default for Gitlab {
    fn default() -> Self {
        Self {
            merge_method: "ff".into(),
            squash_option: "always".into(),
            squash_commit_template: include_str!("../blocks/gitlab-squash-commit-template.in")
                .trim_end_matches('\n')
                .to_owned(),
            push_access_level: 0,
            merge_access_level: 40,
        }
    }
}

/// Read the optional file; content errors refuse instead of falling back.
///
/// # Errors
/// Returns a config-invalid refusal for invalid content, and preserves I/O errors.
pub fn load(target: &Path) -> Result<Option<Config>, RkError> {
    let text = match std::fs::read_to_string(target.join(CONFIG_PATH)) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    parse(&text).map(Some)
}

/// The text as this binary's schema: a schema 1 file migrated, any other
/// text as it is, so the strict reader judges the schema afterwards.
fn current_text(text: &str) -> Result<String, RkError> {
    let raw: toml::Value =
        toml::from_str(text).map_err(|error: toml::de::Error| invalid(error.to_string()))?;
    match raw.get("schema_version").and_then(toml::Value::as_integer) {
        Some(1) => migrate::to_schema_2(text),
        _ => Ok(text.to_owned()),
    }
}

fn parse(text: &str) -> Result<Config, RkError> {
    let text = current_text(text)?;
    let raw: toml::Value =
        toml::from_str(&text).map_err(|error: toml::de::Error| invalid(error.to_string()))?;
    if raw.get("schema_version").and_then(toml::Value::as_integer) != Some(SCHEMA_VERSION) {
        return Err(invalid(format!(
            "schema_version must be {SCHEMA_VERSION}, or 1 for a file this binary migrates"
        )));
    }
    // The release mode is read as a string first, so a value outside the
    // vocabulary refuses naming its own key rather than a span and a
    // variant list the reader must locate for itself.
    if let Some(mode) = raw
        .get("profile")
        .and_then(|profile| profile.get("release"))
        .and_then(|release| release.get("mode"))
    {
        let named = mode.as_str().ok_or_else(|| {
            invalid("profile.release.mode must be a string: automatic, external, or none")
        })?;
        crate::profile::ReleaseMode::parse(named)
            .map_err(|error| invalid(format!("profile.release.mode: {error}")))?;
    }
    let config: Config = toml::from_str(&text).map_err(|error: toml::de::Error| {
        let mut message = error.to_string();
        if let Some(rest) = error.message().strip_prefix("unknown field `") {
            let names: Vec<_> = rest.split('`').collect();
            if let Some(unknown) = names.first()
                && let Some(nearest) = names
                    .iter()
                    .skip(2)
                    .step_by(2)
                    .min_by_key(|name| distance(unknown, name))
            {
                let _ = write!(message, "; nearest known key: {nearest}");
            }
        }
        invalid(message)
    })?;
    if let Some(technologies) = &config.profile.technologies {
        crate::profile::canonical_list("profile.technologies", technologies).map_err(invalid)?;
    }
    if let Some(forge) = &config.profile.forge
        && !forge.is_empty()
    {
        crate::profile::canonical_category(forge)
            .map_err(|reason| invalid(format!("profile.forge: {reason}")))?;
    }
    if let Some(driver) = &config.profile.release.driver {
        crate::profile::canonical_category(driver)
            .map_err(|reason| invalid(format!("profile.release.driver: {reason}")))?;
    }
    if matches!(
        config.profile.release.mode,
        Some(ReleaseMode::External | ReleaseMode::None)
    ) {
        let mode = config
            .profile
            .release
            .mode
            .map_or("none", ReleaseMode::as_str);
        for (key, present) in [
            (
                "profile.release.driver",
                config.profile.release.driver.is_some(),
            ),
            (
                "profile.release.style",
                config.profile.release.style.is_some(),
            ),
            (
                "profile.release.line_prefix",
                config.profile.release.line_prefix.is_some(),
            ),
        ] {
            if present {
                return Err(invalid(format!(
                    "{key} is set while profile.release.mode is {mode}; an external or none release names no driver, style, or line prefix"
                )));
            }
        }
    }
    if let Some(contact) = &config.security.contact {
        canonical_contact(contact).map_err(invalid)?;
    }
    if let Some(response) = &config.security.response {
        canonical_response(response).map_err(invalid)?;
    }
    exclusions(&config.setup.excluded_steps)?;
    floors::check(&config)?;
    Ok(config)
}

/// Judge the declared exclusions: every id names a step this binary runs,
/// and every exclusion states why.
///
/// A reason is required because the exclusions are the audit trail. A
/// reader must be able to tell a chosen subset from an incomplete setup,
/// and a line that names a step and says nothing tells them neither.
fn exclusions(excluded: &BTreeMap<String, String>) -> Result<(), RkError> {
    for (name, reason) in excluded {
        if crate::setup::steps::spec(name).is_none() {
            let nearest = crate::setup::steps::STEPS
                .iter()
                .min_by_key(|step| distance(name, step.name))
                .map_or("", |step| step.name);
            return Err(invalid(format!(
                "setup.excluded_steps names {name}, which is no setup step; nearest known step: {nearest}"
            )));
        }
        if reason.trim().is_empty() {
            return Err(invalid(format!(
                "setup.excluded_steps names {name} with no reason; an excluded step is reported with why it is out of scope"
            )));
        }
    }
    Ok(())
}

pub(super) fn invalid(message: impl std::fmt::Display) -> RkError {
    RkError::refusal(
        Diagnostic::new(Reason::ConfigInvalid, format!("{CONFIG_PATH}: {message}"))
            .action(format!("edit {CONFIG_PATH} and retry"))
            .target_state("nothing was written"),
    )
}

fn distance(left: &str, right: &str) -> usize {
    let mut row: Vec<_> = (0..=right.chars().count()).collect();
    for (i, a) in left.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, b) in right.chars().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (previous + usize::from(a != b))
                .min(row[j] + 1)
                .min(old + 1);
            previous = old;
        }
    }
    row.last().copied().unwrap_or(0)
}

/// Write the authored template with TOML-escaped scalar substitutions.
///
/// # Errors
/// Returns invalid configuration or I/O failures before or during the atomic write.
pub fn write(target: &Path, config: &Config) -> Result<(), RkError> {
    let bytes = render(config)?;
    parse(&String::from_utf8_lossy(&bytes))?;
    crate::atomic::write(&target.join(CONFIG_PATH), &bytes)?;
    Ok(())
}

fn array(values: &[String]) -> toml_edit::Value {
    toml_edit::Value::Array(values.iter().collect())
}

/// The exclusions as the one-line inline table the template carries. A
/// landing writes an empty one; an operator who wants several may turn it
/// into a `[setup.excluded_steps]` table, which this reader parses the
/// same way.
fn inline(values: &BTreeMap<String, String>) -> toml_edit::Value {
    let mut table = toml_edit::InlineTable::new();
    for (key, value) in values {
        table.insert(key, value.clone().into());
    }
    toml_edit::Value::InlineTable(table)
}

/// The value of every landing parameter as the template renders it, or
/// `None` for a key the resolved answers omit: the repository where the
/// project has no forge, and the automatic-only release keys under
/// another mode.
#[allow(
    clippy::too_many_lines,
    reason = "the render list is one token per authored line of the config template, and splitting it would hide that correspondence"
)]
fn fields(config: &Config) -> Result<Vec<(&'static str, Option<toml_edit::Value>)>, RkError> {
    // The trunk names the ruleset the setup installs, so the written
    // configuration states the name a target actually gets rather than a
    // literal that would be wrong for any trunk but the default.
    let trunk = config
        .git
        .trunk
        .clone()
        .ok_or_else(|| invalid("git.trunk is unresolved"))?;
    let technologies = config
        .profile
        .technologies
        .clone()
        .ok_or_else(|| invalid("profile.technologies is unresolved"))?;
    let forge = config
        .profile
        .forge
        .clone()
        .ok_or_else(|| invalid("profile.forge is unresolved"))?;
    let mode = config
        .profile
        .release
        .mode
        .ok_or_else(|| invalid("profile.release.mode is unresolved"))?;
    let mut fields: Vec<(&'static str, Option<toml_edit::Value>)> = vec![
        (
            "RK_CONFIG_SCHEMA_VERSION",
            Some(config.schema_version.into()),
        ),
        (
            "RK_CONFIG_PROJECT_REPO",
            (!config.project.repo.is_empty()).then(|| config.project.repo.clone().into()),
        ),
        ("RK_CONFIG_PROFILE_TECHNOLOGIES", Some(array(&technologies))),
        ("RK_CONFIG_PROFILE_FORGE", Some(forge.into())),
        ("RK_CONFIG_PROFILE_RELEASE_MODE", Some(mode.as_str().into())),
        (
            "RK_CONFIG_PROFILE_RELEASE_DRIVER",
            config
                .profile
                .release
                .driver
                .clone()
                .map(toml_edit::Value::from),
        ),
        (
            "RK_CONFIG_PROFILE_RELEASE_STYLE",
            config
                .profile
                .release
                .style
                .map(|style| style.as_str().into()),
        ),
        (
            "RK_CONFIG_PROFILE_RELEASE_LINE_PREFIX",
            config
                .profile
                .release
                .line_prefix
                .clone()
                .map(toml_edit::Value::from),
        ),
        ("RK_CONFIG_GIT_TRUNK", Some(trunk.clone().into())),
        (
            "RK_CONFIG_GIT_CHECKOUT_MODE",
            Some(
                config
                    .git
                    .checkout_mode
                    .ok_or_else(|| invalid("git.checkout_mode is unresolved"))?
                    .as_str()
                    .into(),
            ),
        ),
        (
            "RK_CONFIG_GIT_INTEGRATION",
            Some(
                config
                    .git
                    .integration
                    .ok_or_else(|| invalid("git.integration is unresolved"))?
                    .as_str()
                    .into(),
            ),
        ),
        (
            "RK_CONFIG_CAPABILITIES_NIX_PACKAGING",
            Some(
                config
                    .capabilities
                    .nix_packaging
                    .ok_or_else(|| invalid("capabilities.nix_packaging is unresolved"))?
                    .into(),
            ),
        ),
        (
            "RK_CONFIG_CAPABILITIES_REPORTING_POLICY",
            Some(
                config
                    .capabilities
                    .reporting_policy
                    .ok_or_else(|| invalid("capabilities.reporting_policy is unresolved"))?
                    .into(),
            ),
        ),
        (
            "RK_CONFIG_CAPABILITIES_SCORECARD",
            Some(
                config
                    .capabilities
                    .scorecard
                    .ok_or_else(|| invalid("capabilities.scorecard is unresolved"))?
                    .into(),
            ),
        ),
        (
            "RK_CONFIG_CAPABILITIES_CODE_SCANNING",
            Some(
                config
                    .capabilities
                    .code_scanning
                    .clone()
                    .ok_or_else(|| invalid("capabilities.code_scanning is unresolved"))?
                    .into(),
            ),
        ),
        (
            "RK_CONFIG_SECURITY_ADVISORIES",
            Some(config.security.advisories.clone().into()),
        ),
        (
            "RK_CONFIG_SECURITY_CONTACT",
            Some(config.security.contact.clone().unwrap_or_default().into()),
        ),
        (
            "RK_CONFIG_SECURITY_RESPONSE",
            Some(
                config
                    .security
                    .response
                    .clone()
                    .unwrap_or_else(|| RESPONSE_DEFAULT.to_owned())
                    .into(),
            ),
        ),
        (
            "RK_CONFIG_SETUP_REQUIRED_CHECK",
            Some(config.setup.required_check.clone().into()),
        ),
        (
            "RK_CONFIG_SETUP_RETIRED_BRANCHES",
            Some(array(&config.setup.retired_branches)),
        ),
        (
            "RK_CONFIG_SETUP_RELEASE_LINES",
            Some(config.setup.release_lines.into()),
        ),
        (
            "RK_CONFIG_SETUP_EXCLUDED_STEPS",
            Some(inline(&config.setup.excluded_steps)),
        ),
        (
            "RK_CONFIG_SETUP_BOT_APP_ID",
            Some(config.setup.bot.app_id.clone().into()),
        ),
    ];
    fields.extend(
        protection_fields(&config.protection, trunk.as_str())
            .into_iter()
            .map(|(token, value)| (token, Some(value))),
    );
    Ok(fields)
}

fn render(config: &Config) -> Result<Vec<u8>, RkError> {
    let fields = fields(config)?;
    let template = crate::embedded::BLOCKS
        .get_file("target-config.toml.in")
        .and_then(include_dir::File::contents_utf8)
        .ok_or_else(|| invalid("the binary lacks its configuration template"))?;
    // Each authored line has one token. Substitute in the source line once,
    // so a user's string containing another token stays literal. A line
    // whose token has no value is dropped: the key is absent rather than
    // written empty, and a table every one of whose keys dropped goes with
    // its own header rather than standing empty.
    let lines: Vec<&str> = template.split_inclusive('\n').collect();
    let dropped: Vec<bool> = lines
        .iter()
        .map(|line| {
            matches!(
                fields.iter().find(|(token, _)| line.contains(token)),
                Some((_, None))
            )
        })
        .collect();
    let mut bytes = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at];
        if line.trim_start().starts_with('[') && line.trim_end().ends_with(']') {
            let mut end = at + 1;
            let mut keeps = false;
            while end < lines.len()
                && !(lines[end].trim_start().starts_with('[')
                    && lines[end].trim_end().ends_with(']'))
            {
                keeps |=
                    fields.iter().any(|(token, _)| lines[end].contains(token)) && !dropped[end];
                end += 1;
            }
            if !keeps {
                // The header, its lines, and the blank line that opened it.
                while bytes.last() == Some(&b'\n')
                    && bytes.len() >= 2
                    && bytes[bytes.len() - 2] == b'\n'
                {
                    bytes.pop();
                }
                at = end;
                continue;
            }
        }
        if !dropped[at] {
            match fields.iter().find(|(token, _)| line.contains(token)) {
                Some((token, Some(value))) => bytes.extend(crate::landing::substitute(
                    line.as_bytes(),
                    token.as_bytes(),
                    value.to_string().as_bytes(),
                )),
                Some((_, None)) => {}
                None => bytes.extend_from_slice(line.as_bytes()),
            }
        }
        at += 1;
    }
    Ok(bytes)
}

fn protection_fields(
    protection: &Protection,
    trunk: &str,
) -> Vec<(&'static str, toml_edit::Value)> {
    vec![
        (
            "RK_CONFIG_PROTECTION_TRUNK_RULESET",
            protection.trunk_ruleset(trunk).into(),
        ),
        (
            "RK_CONFIG_PROTECTION_TAG_RULESET",
            protection.tag_ruleset.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_LINES_RULESET",
            protection.lines_ruleset.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_TITLE_CHECK",
            protection.title_check.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_TAG_PATTERN",
            protection.tag_pattern.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_BYPASS_ACTORS",
            array(&protection.bypass_actors),
        ),
        (
            "RK_CONFIG_PROTECTION_ALLOWED_MERGE_METHODS",
            array(&protection.allowed_merge_methods),
        ),
        (
            "RK_CONFIG_PROTECTION_STRICT_REQUIRED_STATUS_CHECKS",
            protection.strict_required_status_checks.into(),
        ),
        (
            "RK_CONFIG_PROTECTION_OWNED_TRUNK_RULES",
            array(&protection.owned_trunk_rules),
        ),
        (
            "RK_CONFIG_PROTECTION_REQUIRED_APPROVING_REVIEW_COUNT",
            protection.required_approving_review_count.into(),
        ),
        (
            "RK_CONFIG_PROTECTION_DISMISS_STALE_REVIEWS_ON_PUSH",
            protection.dismiss_stale_reviews_on_push.into(),
        ),
        (
            "RK_CONFIG_PROTECTION_REQUIRE_CODE_OWNER_REVIEW",
            protection.require_code_owner_review.into(),
        ),
        (
            "RK_CONFIG_PROTECTION_REQUIRE_LAST_PUSH_APPROVAL",
            protection.require_last_push_approval.into(),
        ),
        (
            "RK_CONFIG_PROTECTION_GITHUB_SQUASH_TITLE_SOURCE",
            protection.github.squash_title_source.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_GITHUB_SQUASH_BODY_SOURCE",
            protection.github.squash_body_source.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_GITLAB_MERGE_METHOD",
            protection.gitlab.merge_method.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_GITLAB_SQUASH_OPTION",
            protection.gitlab.squash_option.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_GITLAB_SQUASH_COMMIT_TEMPLATE",
            protection.gitlab.squash_commit_template.clone().into(),
        ),
        (
            "RK_CONFIG_PROTECTION_GITLAB_PUSH_ACCESS_LEVEL",
            protection.gitlab.push_access_level.into(),
        ),
        (
            "RK_CONFIG_PROTECTION_GITLAB_MERGE_ACCESS_LEVEL",
            protection.gitlab.merge_access_level.into(),
        ),
    ]
}

/// The two floored keys a write-back carries beside `git.integration`.
///
/// They are derived companions of the authority rather than recorded
/// parameters: the record holds no baseline for them, so a target that
/// narrowed either within its floor would read as pending forever and
/// route an upgrade that changes nothing.
/// `target-config:a-flag-overrides-and-a-landing-writes-back` leaves
/// class F policy with its use-time readers, and excluding these from
/// what status judges keeps that true.
const DERIVED_POLICY_KEYS: [&str; 2] = [
    "protection.owned_trunk_rules",
    "protection.gitlab.push_access_level",
];

/// The keys a landing writes back: every class P answer.
const PARAMETER_KEYS: [&str; 19] = [
    "project.repo",
    "profile.technologies",
    "profile.forge",
    "profile.release.mode",
    "profile.release.driver",
    "profile.release.style",
    "profile.release.line_prefix",
    "git.trunk",
    "git.checkout_mode",
    "git.integration",
    // The two floored keys the integration authority decides. They are
    // written back with it, because changing the authority without them
    // leaves a configuration whose own floor table refuses it.
    "protection.owned_trunk_rules",
    "protection.gitlab.push_access_level",
    "capabilities.nix_packaging",
    "capabilities.reporting_policy",
    "capabilities.scorecard",
    "capabilities.code_scanning",
    "security.contact",
    "security.response",
    "schema_version",
];

/// Change one landing parameter while preserving comments and table ordering.
///
/// # Errors
/// Refuses an invalid key, invalid resulting content, or unreadable file; writes atomically.
pub fn rewrite_key(target: &Path, key: &str, value: toml_edit::Value) -> Result<(), RkError> {
    let path = target.join(CONFIG_PATH);
    let text = std::fs::read_to_string(&path)?;
    let next = rewrite_text(&text, key, Some(value))?;
    crate::atomic::write(&path, next.as_bytes())?;
    Ok(())
}

/// The text with `key` set to `value`, or removed where `value` is
/// `None`, every other byte kept. A schema 1 text migrates first.
fn rewrite_text(text: &str, key: &str, value: Option<toml_edit::Value>) -> Result<String, RkError> {
    rewrite_all(text, vec![(key, value)])
}

/// The text with every named key set or removed in one document, every
/// other byte kept. A schema 1 text migrates first.
///
/// One document rather than one per key, because the answers are one
/// decision: writing `profile.release.mode = "none"` before removing the
/// driver it retires would make an intermediate text the reader rejects,
/// and an operator would have no command that performs the transition.
fn rewrite_all(
    text: &str,
    values: Vec<(&str, Option<toml_edit::Value>)>,
) -> Result<String, RkError> {
    for (key, _) in &values {
        if !PARAMETER_KEYS.contains(key) {
            return Err(invalid(format!("{key} is not a landing parameter")));
        }
    }
    parse(text)?;
    let text = current_text(text)?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| invalid(error.to_string()))?;
    let mut changed = false;
    // Only a table this call emptied may be pruned, so the names come from
    // the removals rather than from the whole key list.
    let mut emptied: Vec<&str> = Vec::new();
    for (key, value) in values {
        let removing = value.is_none();
        let applied = apply_key(&mut document, key, value)?;
        changed |= applied;
        if applied
            && removing
            && let Some(parent) = key.split('.').next()
        {
            emptied.push(parent);
        }
    }
    changed |= prune_empty_tables(&mut document, &emptied);
    // A value this writer rewrites keeps the comment the file already
    // carried, so the template's own sentence about a key outlives the
    // answer it describes: a target moving to local integration would read
    // `# F: invariant, contains all four rules` beside two of them. The
    // refresh replaces the template's comments alone and leaves every one
    // the operator wrote.
    changed |= migrate::refresh_comments(&mut document);
    if !changed {
        return Ok(text);
    }
    let next = document.to_string();
    parse(&next)?;
    Ok(next)
}

/// Drop the named tables that are now empty, answering whether the
/// document changed.
///
/// `target-config:an-unanswered-key-is-absent-and-not-empty` asks the
/// writer to leave out a table every one of whose keys it omitted, and the
/// fresh render already does. This is the same rule on the update path,
/// which edits authored text instead: without it a target that drops its
/// forge keeps a bare `[project]` header, and the two writers disagree
/// about one resolved answer.
///
/// Only the named tables, because a table the operator authored empty is
/// theirs and this writer emptied nothing in it.
///
/// Every comment the removed header carried, standing above it or inline
/// beside it, moves to the next table, or to the end of the file where the
/// removed table was last. The header is this binary's; the comment may be
/// the operator's, and an upgrade that silently deleted one would be
/// losing authored text.
pub(crate) fn prune_empty_tables(document: &mut toml_edit::DocumentMut, names: &[&str]) -> bool {
    let order: Vec<String> = document
        .as_table()
        .iter()
        .map(|(key, _)| key.to_owned())
        .collect();
    let mut changed = false;
    for (index, name) in order.iter().enumerate() {
        if !names.contains(&name.as_str())
            || !document
                .get(name)
                .and_then(toml_edit::Item::as_table)
                .is_some_and(toml_edit::Table::is_empty)
        {
            continue;
        }
        let carried = document
            .get(name)
            .and_then(toml_edit::Item::as_table)
            .and_then(|table| carried_comment(table.decor()));
        document.remove(name);
        changed = true;
        let Some(carried) = carried else { continue };
        // The next header the file actually prints: an implicit table
        // emits no header of its own, so its decor would take the comment
        // out of the rendered text with it.
        let next = order
            .iter()
            .skip(index + 1)
            .find(|name| {
                document
                    .get(name)
                    .and_then(toml_edit::Item::as_table)
                    .is_some_and(|table| !table.is_implicit())
            })
            .cloned();
        if let Some(next) = next
            && let Some(table) = document
                .get_mut(&next)
                .and_then(toml_edit::Item::as_table_mut)
        {
            let existing = table
                .decor()
                .prefix()
                .and_then(toml_edit::RawString::as_str)
                .unwrap_or_default()
                .trim_start_matches('\n')
                .to_owned();
            table
                .decor_mut()
                .set_prefix(format!("\n{carried}{existing}"));
        } else {
            let mut trailing = document.trailing().as_str().unwrap_or_default().to_owned();
            if !trailing.is_empty() && !trailing.ends_with('\n') {
                trailing.push('\n');
            }
            trailing.push_str(&carried);
            document.set_trailing(trailing);
        }
    }
    changed
}

/// Every comment in a decor, one per line, or `None` where it carries
/// none.
///
/// An inline comment beside a header becomes a free-standing line, because
/// the header it sat beside is going and a comment needs a line of its own
/// to survive.
fn carried_comment(decor: &toml_edit::Decor) -> Option<String> {
    let mut lines = String::new();
    for raw in [decor.prefix(), decor.suffix()] {
        let Some(text) = raw.and_then(toml_edit::RawString::as_str) else {
            continue;
        };
        for line in text
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('#'))
            .filter(|line| !is_template_comment(line))
        {
            lines.push_str(line);
            lines.push('\n');
        }
    }
    (!lines.is_empty()).then_some(lines)
}

/// Whether the comment is the template's own rather than the operator's.
///
/// The template marks every comment it writes with the class of the key it
/// sits beside: `# P:` a landing parameter, `# N:` a free name, `# F:` an
/// invariant floor. A comment for a key that is going describes nothing
/// once the key is gone, so it goes too, while anything the operator wrote
/// is carried. `refresh_comment` owns the same three markers on the
/// migration path.
fn is_template_comment(line: &str) -> bool {
    let rest = line.trim_start_matches('#').trim_start();
    ["P:", "N:", "F:"]
        .iter()
        .any(|marker| rest.starts_with(marker))
}

/// Put carried comment lines where the rendered file will still show
/// them: above the first header it prints, or at its end where it prints
/// none.
///
/// The last resort for text whose own domain is not rendered. A comment
/// with no home is still the operator's, and the end of the file is where
/// it survives.
pub(crate) fn place_carried(document: &mut toml_edit::DocumentMut, carried: &str) {
    let first = document
        .as_table()
        .iter()
        .find(|(_, item)| item.as_table().is_some_and(|table| !table.is_implicit()))
        .map(|(name, _)| name.to_owned());
    if let Some(first) = first
        && let Some(table) = document
            .get_mut(&first)
            .and_then(toml_edit::Item::as_table_mut)
    {
        let existing = table
            .decor()
            .prefix()
            .and_then(toml_edit::RawString::as_str)
            .unwrap_or_default()
            .trim_start_matches('\n')
            .to_owned();
        table
            .decor_mut()
            .set_prefix(format!("\n{carried}{existing}"));
        return;
    }
    let mut trailing = document.trailing().as_str().unwrap_or_default().to_owned();
    if !trailing.is_empty() && !trailing.ends_with('\n') {
        trailing.push('\n');
    }
    trailing.push_str(carried);
    document.set_trailing(trailing);
}

/// The operator's comments standing above `key`, or `None` where it
/// carries none.
///
/// Read before a move, so the comment travels with the value it describes
/// rather than staying beside a key that is gone.
pub(crate) fn key_comments(table: &toml_edit::Table, key: &str) -> Option<String> {
    let (name, _) = table.get_key_value(key)?;
    carried_comment(name.leaf_decor())
}

/// Put `carried` above `key`, keeping whatever decor it already has.
pub(crate) fn set_key_comments(table: &mut toml_edit::Table, key: &str, carried: &str) {
    let Some(mut name) = table.key_mut(key) else {
        return;
    };
    let decor = name.leaf_decor_mut();
    let existing = decor
        .prefix()
        .and_then(toml_edit::RawString::as_str)
        .unwrap_or_default()
        .trim_start_matches('\n')
        .to_owned();
    decor.set_prefix(format!("\n{carried}{existing}"));
}

/// The nearest known name to `unknown`, for a refusal that helps.
#[must_use]
pub(crate) fn nearest_known<'a>(unknown: &str, known: &[&'a str]) -> Option<&'a str> {
    known
        .iter()
        .min_by_key(|name| distance(unknown, name))
        .copied()
}

/// The operator's comments standing on a table's own header, removed from
/// it.
pub(crate) fn take_header_comments(table: &mut toml_edit::Table) -> Option<String> {
    let carried = carried_comment(table.decor())?;
    table.decor_mut().set_prefix("\n");
    Some(carried)
}

/// Remove `key` from `table`, answering the operator's comments it
/// carried.
///
/// A comment the operator wrote above or beside a key outlives the answer
/// it described: `project-profile:a-schema-one-configuration-migrates-in-place`
/// asks for every free-standing comment to survive, and an upgrade that
/// retires a key is the same promise on the other writer. The comments
/// move to the table's own header, where they read as a note on the domain
/// the key belonged to, and travel further with that header if the table
/// itself empties.
pub(crate) fn take_comments(table: &mut toml_edit::Table, key: &str) -> bool {
    let carried = table.get_key_value(key).and_then(|(name, item)| {
        let mut lines = carried_comment(name.leaf_decor()).unwrap_or_default();
        if let Some(value) = item.as_value()
            && let Some(more) = carried_comment(value.decor())
        {
            lines.push_str(&more);
        }
        (!lines.is_empty()).then_some(lines)
    });
    let removed = table.remove(key).is_some();
    if let Some(carried) = carried {
        let existing = table
            .decor()
            .prefix()
            .and_then(toml_edit::RawString::as_str)
            .unwrap_or_default()
            .trim_start_matches('\n')
            .to_owned();
        table
            .decor_mut()
            .set_prefix(format!("\n{carried}{existing}"));
    }
    removed
}

/// Set or remove one key in an open document, answering whether the
/// document changed. No validation: the caller validates the whole.
fn apply_key(
    document: &mut toml_edit::DocumentMut,
    key: &str,
    value: Option<toml_edit::Value>,
) -> Result<bool, RkError> {
    let segments: Vec<&str> = key.split('.').collect();
    // Every key in `PARAMETER_KEYS` has at least one segment, so the split
    // answers; a key that did not would have refused above.
    let Some((last, parents)) = segments.split_last() else {
        return Err(invalid(format!("{key} names no key")));
    };
    let Some(mut value) = value else {
        let mut item = document.as_item_mut();
        for segment in parents {
            if item.get(segment).is_none() {
                return Ok(false);
            }
            item = &mut item[segment];
        }
        let removed = item
            .as_table_mut()
            .is_some_and(|table| take_comments(table, last));
        return Ok(removed);
    };
    let mut item = document.as_item_mut();
    for segment in parents {
        if item.get(segment).is_none() {
            let mut table = toml_edit::Table::new();
            table.set_implicit(true);
            item[segment] = toml_edit::Item::Table(table);
        }
        item = &mut item[segment];
    }
    if let Some(old) = item.get(last).and_then(toml_edit::Item::as_value) {
        if old.to_string().trim() == value.to_string().trim() {
            return Ok(false);
        }
        *value.decor_mut() = old.decor().clone();
    } else if let Some(comment) = migrate::template_comment(&segments) {
        // A key this writer is adding rather than changing carries no
        // decor of the operator's, so it takes the template's own
        // comment. Without this a parameter that arrives in a later
        // release lands bare in every existing target's configuration,
        // beside keys that all state their class and their meaning.
        value.decor_mut().set_suffix(comment);
    }
    item[last] = toml_edit::Item::Value(value);
    Ok(true)
}

/// Resolved landing input, including every key a preview would write.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Plan {
    /// Added or updated configuration.
    pub action: &'static str,
    /// Keys whose configured answers differ from the record.
    pub changes: Vec<String>,
    /// The exact authored TOML the apply writes.
    pub content: String,
}

impl Plan {
    /// Resolve the output without writing it; existing comments survive.
    ///
    /// # Errors
    /// Propagates unreadable or invalid configuration.
    pub fn new(
        target: &Path,
        params: &crate::landing::Params,
        existing: Option<&Config>,
        record: Option<&crate::landing::manifest::Manifest>,
    ) -> Result<Self, RkError> {
        let text = existing
            .map(|_| std::fs::read_to_string(target.join(CONFIG_PATH)))
            .transpose()?;
        Self::compose(text.as_deref(), params, existing, record)
    }

    /// Resolve the output from the existing text already read, so a
    /// planner that owns no filesystem can compose it from its
    /// observation; existing comments survive.
    ///
    /// # Errors
    /// Propagates invalid configuration.
    pub fn compose(
        text: Option<&str>,
        params: &crate::landing::Params,
        existing: Option<&Config>,
        record: Option<&crate::landing::manifest::Manifest>,
    ) -> Result<Self, RkError> {
        let mut resolved = existing.cloned().unwrap_or_default();
        resolved.schema_version = SCHEMA_VERSION;
        params.repo().clone_into(&mut resolved.project.repo);
        resolved.profile = Profile {
            technologies: Some(params.technologies().to_vec()),
            forge: Some(params.forge().unwrap_or_default().to_owned()),
            release: Release {
                mode: Some(params.release_mode()),
                driver: params.driver().map(str::to_owned),
                style: params.style(),
                line_prefix: params.profile().release.line_prefix.clone(),
            },
        };
        resolved.git = Git {
            trunk: Some(params.trunk().to_owned()),
            checkout_mode: Some(params.checkout_mode()),
            integration: Some(params.integration()),
        };
        resolved.protection = protection_for(
            &resolved.protection,
            params.integration(),
            existing.is_none(),
        );
        resolved.capabilities = Capabilities {
            nix_packaging: Some(params.nix_packaging()),
            reporting_policy: Some(params.reporting_policy()),
            scorecard: Some(params.scorecard()),
            code_scanning: Some(
                params
                    .code_scanning()
                    .map_or("off", crate::landing::Provider::as_str)
                    .to_owned(),
            ),
        };
        resolved.security.contact = Some(params.security_contact().to_owned());
        resolved.security.response = Some(params.security_response().to_owned());
        let content = if let Some(text) = text.filter(|_| existing.is_some()) {
            rewrite_all(text, parameter_values(&resolved))?
        } else {
            String::from_utf8(render(&resolved)?).map_err(|e| invalid(e.to_string()))?
        };
        parse(&content)?;
        Ok(Self {
            action: if existing.is_some() {
                "updated"
            } else {
                "added"
            },
            changes: record.map_or_else(Vec::new, |record| pending(&resolved, record)),
            content,
        })
    }

    /// Write the prepared configuration before the landing record.
    ///
    /// # Errors
    /// Propagates an atomic write failure.
    pub fn apply(&self, target: &Path) -> Result<(), RkError> {
        crate::atomic::write(&target.join(CONFIG_PATH), self.content.as_bytes())?;
        Ok(())
    }
}

/// Every class P key with its value, `None` for a key the answers omit.
fn parameter_values(config: &Config) -> Vec<(&'static str, Option<toml_edit::Value>)> {
    let mut values: Vec<(&'static str, Option<toml_edit::Value>)> = vec![(
        "project.repo",
        (!config.project.repo.is_empty()).then(|| config.project.repo.clone().into()),
    )];
    if let Some(list) = &config.profile.technologies {
        values.push(("profile.technologies", Some(array(list))));
    }
    if let Some(forge) = config.profile.forge.clone() {
        values.push(("profile.forge", Some(forge.into())));
    }
    if let Some(mode) = config.profile.release.mode {
        values.push(("profile.release.mode", Some(mode.as_str().into())));
        values.push((
            "profile.release.driver",
            config
                .profile
                .release
                .driver
                .clone()
                .map(toml_edit::Value::from),
        ));
        values.push((
            "profile.release.style",
            config
                .profile
                .release
                .style
                .map(|style| style.as_str().into()),
        ));
        values.push((
            "profile.release.line_prefix",
            config
                .profile
                .release
                .line_prefix
                .clone()
                .map(toml_edit::Value::from),
        ));
    }
    if let Some(trunk) = config.git.trunk.clone() {
        values.push(("git.trunk", Some(trunk.into())));
    }
    if let Some(mode) = config.git.checkout_mode {
        values.push(("git.checkout_mode", Some(mode.as_str().into())));
    }
    if let Some(mode) = config.git.integration {
        values.push(("git.integration", Some(mode.as_str().into())));
        // The pair that authority decides travels with it: a write-back
        // that moved the mode alone would leave a configuration the
        // floor table refuses on the next read.
        let mut rules = toml_edit::Array::new();
        for rule in &config.protection.owned_trunk_rules {
            rules.push(rule.as_str());
        }
        values.push((
            "protection.owned_trunk_rules",
            Some(toml_edit::Value::Array(rules)),
        ));
        values.push((
            "protection.gitlab.push_access_level",
            Some(config.protection.gitlab.push_access_level.into()),
        ));
    }
    if let Some(value) = config.capabilities.nix_packaging {
        values.push(("capabilities.nix_packaging", Some(value.into())));
    }
    if let Some(value) = config.capabilities.reporting_policy {
        values.push(("capabilities.reporting_policy", Some(value.into())));
    }
    if let Some(value) = config.capabilities.scorecard {
        values.push(("capabilities.scorecard", Some(value.into())));
    }
    if let Some(value) = config.capabilities.code_scanning.clone() {
        values.push(("capabilities.code_scanning", Some(value.into())));
    }
    // An empty contact is an answer, not an absence: it resets the landed
    // policy to the forge's own prose, so it projects like any other value.
    if let Some(value) = config.security.contact.clone() {
        values.push(("security.contact", Some(value.into())));
    }
    if let Some(value) = config.security.response.clone() {
        values.push(("security.response", Some(value.into())));
    }
    values
}

/// Only explicit class P answers can be pending; comparisons still use the record.
#[must_use]
pub fn pending(config: &Config, record: &crate::landing::manifest::Manifest) -> Vec<String> {
    let recorded = {
        let params = crate::landing::Params::from_record(record);
        Plan::compose(None, &params, None, None)
            .ok()
            .and_then(|plan| parse(&plan.content).ok())
    };
    let Some(recorded) = recorded else {
        return Vec::new();
    };
    let render = |value: &Option<toml_edit::Value>| {
        value.as_ref().map_or_else(
            || "<absent>".to_owned(),
            |value| value.to_string().trim().to_owned(),
        )
    };
    let baseline: Vec<(&str, String)> = parameter_values(&recorded)
        .iter()
        .filter(|(key, _)| !DERIVED_POLICY_KEYS.contains(key))
        .map(|(key, value)| (*key, render(value)))
        .collect();
    parameter_values(config)
        .into_iter()
        .filter(|(key, _)| !DERIVED_POLICY_KEYS.contains(key))
        .map(|(key, value)| (key, render(&value)))
        .filter(|(key, value)| {
            baseline
                .iter()
                .any(|(other, old)| key == other && value != old)
        })
        .map(|(key, _)| key.to_owned())
        .collect()
}

/// The compiled protection floors a locally integrated trunk carries.
///
/// The ordinary defaults describe forge integration, because that is the
/// shape this convention had before the authority became an axis. A
/// locally integrated trunk drops the two rules no forge can apply to a
/// push, and takes the narrowest GitLab level that still admits the push
/// its integrations end in.
#[must_use]
fn local_protection() -> Protection {
    /// The narrowest GitLab access level that still admits a push.
    const MAINTAINER: i64 = 40;
    let mut policy = Protection::default();
    policy
        .owned_trunk_rules
        .retain(|rule| rule != "pull_request" && rule != "required_status_checks");
    policy.gitlab.push_access_level = MAINTAINER;
    policy
}

/// The protection values a landing writes, for one integration authority.
///
/// Exactly two keys differ between the authorities, and they are the two
/// this looks at: the owned trunk rules, and the GitLab push access
/// level. A target whose pair matches one authority's compiled defaults
/// never stated them; it took them, so a landing that resolves the other
/// authority writes that authority's pair instead, and a fresh landing
/// writes its own. A target whose pair matches neither is one an operator
/// narrowed or widened, and it keeps every value it stated: the floor
/// table already judged it under the same mode, and an operator who
/// changed a protection meant it.
///
/// The pair alone, never the whole policy: every other key here is a name
/// or a review policy the authority does not decide, and a target that
/// renamed its ruleset would otherwise read as having stated the pair.
///
/// This is what makes the committed configuration the one source: the
/// floors judge these values, the setup installs them, and its check
/// reads them back, so a local-integration target is never handed a trunk
/// its own integrations cannot push.
#[must_use]
fn protection_for(held: &Protection, integration: Integration, fresh: bool) -> Protection {
    let forge = Protection::default();
    let local = local_protection();
    let pair = |policy: &Protection| {
        (
            policy.owned_trunk_rules.clone(),
            policy.gitlab.push_access_level,
        )
    };
    let taken = fresh || pair(held) == pair(&forge) || pair(held) == pair(&local);
    if !taken {
        return held.clone();
    }
    let mut next = held.clone();
    let source = match integration {
        Integration::Forge => forge,
        Integration::Local => local,
    };
    next.owned_trunk_rules = source.owned_trunk_rules;
    next.gitlab.push_access_level = source.gitlab.push_access_level;
    next
}

/// The trunk accessor for callers without a setup context.
///
/// # Errors
/// Propagates invalid configuration and I/O failures.
pub fn trunk_of(target: &Path) -> Result<String, RkError> {
    Ok(load(target)?
        .and_then(|config| config.git.trunk)
        .unwrap_or_else(|| TRUNK_DEFAULT.to_owned()))
}

/// The release-line prefix for callers without a setup context.
///
/// # Errors
/// Propagates invalid configuration and I/O failures.
pub fn line_prefix_of(target: &Path) -> Result<String, RkError> {
    Ok(load(target)?
        .and_then(|config| config.profile.release.line_prefix)
        .unwrap_or_else(|| LINE_PREFIX_DEFAULT.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{CONFIG_PATH, Config, load, parse, rewrite_key, trunk_of, write};
    use crate::landing::{CheckoutMode, Integration, Style};
    use crate::profile::ReleaseMode;

    /// The resolved defaults a landing writes for an automatic rust
    /// release on GitHub.
    fn resolved_defaults() -> Config {
        Config {
            project: super::Project {
                repo: "acme/widget".into(),
            },
            profile: super::Profile {
                technologies: Some(vec!["rust".into()]),
                forge: Some("github".into()),
                release: super::Release {
                    mode: Some(ReleaseMode::Automatic),
                    driver: Some("rust".into()),
                    style: Some(Style::Trunk),
                    line_prefix: Some(super::LINE_PREFIX_DEFAULT.into()),
                },
            },
            git: super::Git {
                trunk: Some(super::TRUNK_DEFAULT.into()),
                checkout_mode: Some(CheckoutMode::LinkedWorktree),
                integration: Some(Integration::Local),
            },
            capabilities: super::Capabilities {
                nix_packaging: Some(false),
                reporting_policy: Some(true),
                scorecard: Some(false),
                code_scanning: Some("off".to_owned()),
            },
            // Writing states both security answers, so a reader sees the
            // policy the target landed rather than an implied one.
            security: super::Security {
                contact: Some(String::new()),
                response: Some(super::RESPONSE_DEFAULT.into()),
                ..super::Security::default()
            },
            // Writing resolves the derived ruleset name, so the file states
            // the name the setup installs rather than leaving it implied.
            protection: super::Protection {
                trunk_ruleset: Some(format!("{}-protection", super::TRUNK_DEFAULT)),
                ..super::Protection::default()
            },
            ..Config::default()
        }
    }

    #[test]
    fn an_omitted_key_is_distinguishable_from_an_explicit_default() {
        let omitted = parse("schema_version = 2\n").expect("omitted answers parse");
        let explicit = parse(
            "schema_version = 2\n[git]\ncheckout_mode = 'linked-worktree'\n[profile.release]\nmode = 'automatic'\nstyle = 'trunk'\n[capabilities]\nnix_packaging = false\nreporting_policy = true\nscorecard = false\ncode_scanning = 'off'\n",
        )
        .expect("explicit defaults parse");
        assert_eq!(omitted.git, super::Git::default());
        assert_eq!(omitted.capabilities, super::Capabilities::default());
        assert_eq!(
            explicit.git.checkout_mode,
            Some(CheckoutMode::LinkedWorktree)
        );
        assert_eq!(explicit.profile.release.style, Some(Style::Trunk));
        assert_eq!(explicit.capabilities.nix_packaging, Some(false));
        assert_eq!(explicit.capabilities.reporting_policy, Some(true));
        assert_eq!(explicit.capabilities.scorecard, Some(false));
        assert_eq!(explicit.capabilities.code_scanning.as_deref(), Some("off"));
        assert_ne!(omitted, explicit);
        // The older spellings of the checkout mode still read.
        let older = parse("schema_version = 2\n[git]\ncheckout_mode = 'worktree'\n")
            .expect("the older spelling reads");
        assert_eq!(older.git.checkout_mode, Some(CheckoutMode::LinkedWorktree));
    }

    /// A configuration written by 0.3.13 carries `installation_id`, which
    /// this version reads and ignores. Refusing it would strand every
    /// target that release landed.
    #[test]
    fn a_config_from_the_release_that_wrote_installation_id_still_reads() {
        let dir = tempfile::tempdir().expect("a tempdir");
        std::fs::create_dir_all(dir.path().join(".release-kit")).expect("the directory exists");
        std::fs::write(
            dir.path().join(CONFIG_PATH),
            "schema_version = 1\n\n[setup.bot]\napp_id = \"123\"\ninstallation_id = 0\n",
        )
        .expect("the config writes");
        let held = load(dir.path())
            .expect("the config reads")
            .expect("it is present");
        assert_eq!(held.setup.bot.app_id, "123");
        assert_eq!(
            held.setup.bot.installation_id,
            Some(0),
            "the key parses; nothing reads it"
        );
    }

    /// SATISFIES project-profile:a-schema-one-configuration-migrates-in-place
    #[test]
    fn a_schema_1_config_migrates_into_its_domains() {
        let held = parse(
            "schema_version = 1\n[project]\nrepo = 'acme/widget'\nforge = 'gitlab'\ntech = 'bash'\ntrunk = 'main'\n[landing]\nworkflow = 'branches'\nstyle = 'lines'\nnix = false\n[setup]\nline_prefix = 'stable/'\n",
        )
        .expect("a schema 1 file reads");
        assert_eq!(held.schema_version, 2);
        assert_eq!(held.profile.technologies, Some(vec!["bash".to_owned()]));
        assert_eq!(held.profile.forge.as_deref(), Some("gitlab"));
        assert_eq!(held.profile.release.mode, Some(ReleaseMode::Automatic));
        assert_eq!(held.profile.release.driver.as_deref(), Some("bash"));
        assert_eq!(held.profile.release.style, Some(Style::Lines));
        assert_eq!(held.profile.release.line_prefix.as_deref(), Some("stable/"));
        assert_eq!(held.git.trunk.as_deref(), Some("main"));
        assert_eq!(held.git.checkout_mode, Some(CheckoutMode::MainWorktree));
        assert_eq!(held.capabilities.nix_packaging, Some(false));
        assert_eq!(held.capabilities.reporting_policy, Some(true));
    }

    /// SATISFIES project-profile:release-intent-has-three-modes
    #[test]
    fn every_invalid_release_state_names_its_key() {
        for (text, key) in [
            (
                "[profile.release]\nmode = 'none'\nstyle = 'trunk'\n",
                "profile.release.style",
            ),
            (
                "[profile.release]\nmode = 'external'\ndriver = 'rust'\n",
                "profile.release.driver",
            ),
            (
                "[profile.release]\nmode = 'none'\nline_prefix = 'release/'\n",
                "profile.release.line_prefix",
            ),
            (
                "[profile]\ntechnologies = ['rust', 'rust']\n",
                "profile.technologies",
            ),
            (
                "[profile]\ntechnologies = ['Rust!']\n",
                "profile.technologies",
            ),
            ("[profile]\nforge = 'Git Hub'\n", "profile.forge"),
            (
                "[profile.release]\nmode = 'manual'\n",
                "profile.release.mode",
            ),
        ] {
            let error = parse(&format!("schema_version = 2\n{text}"))
                .expect_err("an invalid release state refuses")
                .to_string();
            assert!(error.contains(key), "{key}: {error}");
        }
    }

    /// A parameter that arrives in a later release lands beside the keys
    /// that were already there, stating its class and its meaning the way
    /// they do, rather than bare.
    #[test]
    fn a_newly_written_key_takes_the_templates_comment() {
        let text = "schema_version = 2\n\n[git]\ntrunk = 'main' # mine\n";
        let next = super::rewrite_text(
            text,
            "git.integration",
            Some(toml_edit::Value::from("local")),
        )
        .expect("the key writes");
        assert!(
            next.contains("integration = \"local\" # P: local or forge"),
            "{next}"
        );
        // A key the operator already commented keeps their words.
        let next = super::rewrite_text(&next, "git.trunk", Some(toml_edit::Value::from("master")))
            .expect("the key writes");
        assert!(next.contains("trunk = \"master\" # mine"), "{next}");
    }

    #[test]
    fn the_landed_config_template_round_trips() {
        let dir = tempfile::tempdir().expect("a target exists");
        let mut config = Config::default();
        config.project.repo = "acme/nested/widget".into();
        config.profile.technologies = Some(vec!["bash".into(), "python".into()]);
        config.profile.forge = Some("gitlab".into());
        config.profile.release = super::Release {
            mode: Some(ReleaseMode::Automatic),
            driver: Some("bash".into()),
            style: Some(Style::Lines),
            line_prefix: Some("stable/".into()),
        };
        config.git.trunk = Some("main".into());
        config.git.checkout_mode = Some(CheckoutMode::MainWorktree);
        config.git.integration = Some(Integration::Forge);
        config.capabilities.nix_packaging = Some(true);
        config.capabilities.reporting_policy = Some(false);
        config.capabilities.scorecard = Some(true);
        config.capabilities.code_scanning = Some("semgrep".to_owned());
        // The escaping subject moved to the one unrestricted string in this
        // table: `contact` is now a class P value the reader holds to a
        // single control-free line, so it can carry neither.
        config.security.advisories =
            "A \"quoted\" project\nRK_CONFIG_SECURITY_RESPONSE\\end".into();
        config.security.contact = Some("security team, room 3 \"the vault\"".into());
        config.security.response = Some("14 business days".into());
        config.setup.required_check = "build / test".into();
        config.setup.retired_branches = vec!["develop".into(), "old\"branch".into()];
        config.setup.release_lines = true;
        config.setup.excluded_steps = [
            (
                "package-check".to_owned(),
                "nothing is published".to_owned(),
            ),
            (
                "protect-trunk".to_owned(),
                "this project merges \"locally\"".to_owned(),
            ),
        ]
        .into_iter()
        .collect();
        config.setup.bot.app_id = "123".into();
        config.protection.trunk_ruleset = Some("primary".into());
        config.protection.tag_ruleset = "versions".into();
        config.protection.lines_ruleset = "maintenance".into();
        config.protection.title_check = "intent".into();
        config.protection.tag_pattern = "refs/tags/*".into();
        config
            .protection
            .owned_trunk_rules
            .push("required_signatures".into());
        config.protection.required_approving_review_count = 2;
        config.protection.dismiss_stale_reviews_on_push = true;
        config.protection.require_code_owner_review = true;
        config.protection.require_last_push_approval = true;
        config.protection.gitlab.squash_commit_template =
            "%{title}\n\nContext: %{description}".into();
        config.protection.gitlab.merge_access_level = 40;
        // A release-less profile with no forge: the automatic-only keys
        // and the repository are absent from the written file.
        let mut release_less = resolved_defaults();
        release_less.project.repo = String::new();
        release_less.profile.technologies = Some(Vec::new());
        release_less.profile.forge = Some(String::new());
        release_less.profile.release = super::Release {
            mode: Some(ReleaseMode::None),
            driver: None,
            style: None,
            line_prefix: None,
        };
        release_less.capabilities.reporting_policy = Some(false);
        for expected in [resolved_defaults(), config, release_less] {
            write(dir.path(), &expected).expect("the template renders");
            assert_eq!(load(dir.path()).expect("the config reads"), Some(expected));
            let text =
                std::fs::read_to_string(dir.path().join(CONFIG_PATH)).expect("the text reads");
            assert!(text.contains("# P: every technology"));
            assert!(text.contains("# F: invariant"));
        }
        let text = std::fs::read_to_string(dir.path().join(CONFIG_PATH)).expect("the text reads");
        assert!(
            !text.contains("style ="),
            "a none release writes no style: {text}"
        );
        assert!(!text.contains("repo ="), "no forge writes no repo: {text}");
    }

    #[test]
    fn a_config_with_an_unknown_key_refuses_by_name() {
        for (table, typo, nearest) in [
            ("", "schemax_version", "schema_version"),
            ("git", "trunkx", "trunk"),
            ("profile.release", "stile", "style"),
            ("capabilities", "nix_packagingg", "nix_packaging"),
            ("security", "contactx", "contact"),
            ("setup", "required_checkx", "required_check"),
            ("setup.bot", "app_i", "app_id"),
            ("protection", "trunk_rulesett", "trunk_ruleset"),
            (
                "protection.github",
                "squash_body_sourcex",
                "squash_body_source",
            ),
            ("protection.gitlab", "squash_optionx", "squash_option"),
        ] {
            let header = if table.is_empty() {
                String::new()
            } else {
                format!("[{table}]\n")
            };
            let text = format!("schema_version = 2\n{header}{typo} = 'value'\n");
            let error = parse(&text).expect_err("unknown keys refuse").to_string();
            for expected in [CONFIG_PATH, typo, &format!("nearest known key: {nearest}")] {
                assert!(error.contains(expected), "{error}");
            }
        }
    }

    /// An exclusion removes a step from scope, so a typo in one would
    /// silently keep judging a step the target does not run, and a
    /// reasonless one would leave a report nobody can audit.
    #[test]
    fn an_exclusion_names_a_real_step_and_states_why() {
        for (text, expected) in [
            (
                "[setup.excluded_steps]\nprotect-trunkk = 'we merge locally'\n",
                vec!["protect-trunkk", "nearest known step: protect-trunk"],
            ),
            (
                "[setup.excluded_steps]\nprotect-trunk = '  '\n",
                vec!["protect-trunk", "no reason"],
            ),
        ] {
            let error = parse(&format!("schema_version = 2\n{text}"))
                .expect_err("the exclusion refuses")
                .to_string();
            for want in expected {
                assert!(error.contains(want), "{error}");
            }
        }
        let held = parse(
            "schema_version = 2\n[setup.excluded_steps]\nprotect-trunk = 'we merge locally'\n",
        )
        .expect("a named step with a reason parses");
        assert_eq!(
            held.setup
                .excluded_steps
                .get("protect-trunk")
                .map(String::as_str),
            Some("we merge locally")
        );
    }

    /// An exclusion narrows what the setup judges. It never weakens the
    /// method's policy, so the floors bind a target that runs a subset
    /// exactly as they bind one that runs every step.
    #[test]
    fn an_exclusion_does_not_lift_a_floor() {
        let error = parse(
            "schema_version = 2\n[setup.excluded_steps]\nprotect-trunk = 'we merge locally'\n\n[protection]\nallowed_merge_methods = ['squash', 'merge']\n",
        )
        .expect_err("the floor binds an excluded step's keys too")
        .to_string();
        assert!(
            error.contains("protection.allowed_merge_methods"),
            "{error}"
        );
    }

    #[test]
    fn a_config_at_an_unknown_schema_refuses() {
        for text in ["schema_version = 999", "schema_version = '2'", ""] {
            let error = parse(text)
                .expect_err("a schema must be declared and known")
                .to_string();
            assert!(
                error.contains(CONFIG_PATH) && error.contains("schema_version"),
                "{error}"
            );
        }
    }

    #[test]
    fn an_unparsable_config_refuses_naming_the_position() {
        let error = parse("schema_version = 2\n[project\n")
            .expect_err("bad TOML refuses")
            .to_string();
        for expected in [CONFIG_PATH, "line 2", "column"] {
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn an_absent_config_reads_as_none() {
        let dir = tempfile::tempdir().expect("a target exists");
        assert_eq!(load(dir.path()).expect("absence is compatible"), None);
        assert_eq!(trunk_of(dir.path()).expect("the default reads"), "master");
    }

    #[test]
    fn loading_checks_floors_and_trunk_of_propagates_invalid_content() {
        let dir = tempfile::tempdir().expect("a target exists");
        std::fs::create_dir(dir.path().join(".release-kit")).expect("the directory exists");
        std::fs::write(
            dir.path().join(CONFIG_PATH),
            "schema_version = 2\n[protection]\nstrict_required_status_checks = false\n",
        )
        .expect("a config exists");
        let error =
            trunk_of(dir.path()).expect_err("invalid policy refuses even through the accessor");
        assert_eq!(error.exit_code(), 73);
        assert!(
            error
                .to_string()
                .contains("protection.strict_required_status_checks")
        );
    }

    #[test]
    fn rewrite_key_preserves_comments() {
        let dir = tempfile::tempdir().expect("a target exists");
        std::fs::create_dir(dir.path().join(".release-kit")).expect("the directory exists");
        let original = "# Project answers\nschema_version = 2\n\n[security] # first table stays first\ncontact = 'team' # keep me\n\n[profile.release]\n# Our release choice\nmode = 'automatic'\nstyle  = 'trunk'  # keep this reason\n\n[git]\ncheckout_mode = 'main-worktree'\n";
        let path = dir.path().join(CONFIG_PATH);
        std::fs::write(&path, original).expect("a config exists");
        rewrite_key(dir.path(), "profile.release.style", "lines".into())
            .expect("the style writes back");
        let text = std::fs::read_to_string(&path).expect("the text reads");
        assert_eq!(text, original.replace("'trunk'", "\"lines\""));
        assert_eq!(
            load(dir.path())
                .expect("the config reads")
                .expect("present")
                .profile
                .release
                .style,
            Some(Style::Lines)
        );
        rewrite_key(dir.path(), "project.repo", "acme/widget".into())
            .expect("an omitted table can be added");
        assert_eq!(
            load(dir.path())
                .expect("reads")
                .expect("present")
                .project
                .repo,
            "acme/widget"
        );
        rewrite_key(
            dir.path(),
            "security.contact",
            "security@acme.example".into(),
        )
        .expect("the contact is a landing parameter");
        rewrite_key(dir.path(), "security.response", "14 days".into())
            .expect("the response is a landing parameter");
        let held = load(dir.path()).expect("reads").expect("present");
        assert_eq!(
            held.security.contact.as_deref(),
            Some("security@acme.example")
        );
        assert_eq!(held.security.response.as_deref(), Some("14 days"));
        let text = std::fs::read_to_string(&path).expect("the text reads");
        assert!(text.contains("# keep me"), "the comment survives: {text}");
        let before = std::fs::read(&path).expect("the bytes read");
        for (key, value) in [
            ("security.advisories", "acme/private"),
            ("security.response", "90d"),
            ("profile.release.style", "unknown"),
        ] {
            assert!(rewrite_key(dir.path(), key, value.into()).is_err());
            assert_eq!(std::fs::read(&path).expect("the bytes read"), before);
        }
    }

    /// The two security answers are one line and one narrow grammar,
    /// because both land verbatim in a public policy.
    #[test]
    fn the_security_answers_are_held_to_their_grammar() {
        for value in ["team@acme.example", "  https://acme.example/report  ", ""] {
            super::canonical_contact(value).expect("a control-free line is a contact");
        }
        for value in ["one\ntwo", "one\rtwo", "one\u{7}two"] {
            let refusal = super::canonical_contact(value).expect_err("a control character refuses");
            assert!(refusal.contains("security.contact"), "{refusal}");
        }
        assert_eq!(
            super::canonical_contact("  team@acme.example  "),
            Ok("team@acme.example".to_owned()),
            "surrounding whitespace is trimmed"
        );
        for value in [
            "best-effort",
            "1 day",
            "2 days",
            "14 days",
            "1 business day",
            "14 business days",
        ] {
            assert_eq!(super::canonical_response(value), Ok(value.to_owned()));
        }
        assert_eq!(
            super::canonical_response(""),
            Ok(super::RESPONSE_DEFAULT.to_owned()),
            "an empty answer reads as the compiled default"
        );
        for value in [
            "0 days",
            "1 days",
            "2 day",
            "+2 days",
            "02 days",
            "4294967296 days",
            "90d",
            "two days",
            "we answer quickly",
            "2 weeks",
        ] {
            let refusal =
                super::canonical_response(value).expect_err("an unstateable window refuses");
            assert!(refusal.contains("security.response"), "{value}: {refusal}");
            assert!(refusal.contains("business days"), "{value}: {refusal}");
        }
        let refusal = parse("schema_version = 2\n[security]\nresponse = '90d'\n")
            .expect_err("the reader refuses it too")
            .to_string();
        assert!(refusal.contains("security.response"), "{refusal}");
    }

    /// SATISFIES target-config:an-unanswered-key-is-absent-and-not-empty
    /// A header whose every key the writer omitted goes, and every comment
    /// it carried survives: standing above it, standing beside it, and in
    /// either position when the emptied table is the file's last.
    #[test]
    fn a_pruned_header_leaves_no_comment_behind() {
        let cases = [
            (
                "a middle table, comment above",
                "schema_version = 2\n\n# the operator's note\n[project]\nrepo = \"acme/widget\"\n\n[git]\ntrunk = \"main\"\n",
            ),
            (
                "a middle table, comment inline",
                "schema_version = 2\n\n[project] # the operator's note\nrepo = \"acme/widget\"\n\n[git]\ntrunk = \"main\"\n",
            ),
            (
                "the last table, comment above",
                "schema_version = 2\n\n[git]\ntrunk = \"main\"\n\n# the operator's note\n[project]\nrepo = \"acme/widget\"\n",
            ),
            (
                "the last table, comment inline",
                "schema_version = 2\n\n[git]\ntrunk = \"main\"\n\n[project] # the operator's note\nrepo = \"acme/widget\"\n",
            ),
        ];
        for (case, text) in cases {
            let next = super::rewrite_text(text, "project.repo", None).expect("the key removes");
            assert!(!next.contains("repo ="), "{case}: {next}");
            assert!(
                !next.contains("[project]"),
                "{case}: a table every one of whose keys dropped goes with them: {next}"
            );
            assert!(
                next.contains("# the operator's note"),
                "{case}: the comment the header carried survives: {next}"
            );
            parse(&next).unwrap_or_else(|error| panic!("{case}: {error}"));
        }
    }

    /// SATISFIES target-config:a-flag-overrides-and-a-landing-writes-back
    /// A key the writer retires takes the template's own comment with it
    /// and leaves the operator's behind, on the header of the domain the
    /// key belonged to. The template comment describes a key that is gone;
    /// the operator's comment is authored text this writer does not delete.
    #[test]
    fn a_retired_key_drops_the_template_comment_and_keeps_the_operators() {
        let text = concat!(
            "schema_version = 2\n\n[project]\n",
            "# the operator's note\n",
            "repo = \"acme/widget\" # P: project path on the forge\n\n",
            "[git]\ntrunk = \"main\"\n"
        );
        let next = super::rewrite_text(text, "project.repo", None).expect("the key removes");
        assert!(!next.contains("repo ="), "{next}");
        assert!(!next.contains("[project]"), "{next}");
        assert!(
            next.contains("# the operator's note"),
            "authored text survives: {next}"
        );
        assert!(
            !next.contains("# P:"),
            "the template's comment describes a key that is gone: {next}"
        );
        parse(&next).expect("the result parses");

        // A domain that keeps other keys keeps the note on its own header.
        let text = concat!(
            "schema_version = 2\n\n[profile]\ntechnologies = [\"rust\"]\n\n",
            "[profile.release]\nmode = \"automatic\"\ndriver = \"rust\"\n",
            "# why this project names its own prefix\n",
            "line_prefix = \"stable/\"\n"
        );
        let next = super::rewrite_text(text, "profile.release.line_prefix", None)
            .expect("the key removes");
        assert!(!next.contains("line_prefix ="), "{next}");
        assert!(next.contains("[profile.release]"), "{next}");
        assert!(
            next.contains("# why this project names its own prefix"),
            "authored text survives: {next}"
        );
    }
}
