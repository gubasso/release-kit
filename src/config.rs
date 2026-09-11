//! The committed target answers, parsed strictly and written from authored text.
//! Comparisons continue to use the landing record alone.

pub mod floors;

use std::fmt::Write as _;
use std::path::Path;

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::{Style, Workflow};
use serde::Deserialize;

/// The committed input, relative to the target root.
pub const CONFIG_PATH: &str = ".release-kit/config.toml";
/// The only supported configuration schema.
pub const SCHEMA_VERSION: i64 = 1;

/// Per-target answers; an omitted table uses its compiled defaults.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Config {
    /// Version of the authored configuration shape.
    pub schema_version: i64,
    /// Landing identity and trunk name.
    pub project: Project,
    /// Values resolved into the landing record.
    pub landing: Landing,
    /// Report-routing facts, currently not rendered into any payload.
    pub security: Security,
    /// Forge setup inputs.
    pub setup: Setup,
    /// Names and floored policy.
    pub protection: Protection,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            project: Project::default(),
            landing: Landing::default(),
            security: Security::default(),
            setup: Setup::default(),
            protection: Protection::default(),
        }
    }
}

/// The `project` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Project {
    /// P: project path on the forge, nested groups included.
    pub repo: String,
    /// P: github or gitlab; empty means detect.
    pub forge: String,
    /// P: payload binding; empty means detect.
    pub tech: String,
    /// P: the one permanent branch, rendered into every landed artifact
    /// that names it. Absent means the landing has not answered it, so a
    /// record's own answer survives an upgrade that predates the key.
    pub trunk: Option<String>,
}

/// The compiled trunk, used where neither a configuration nor a record answers.
pub const TRUNK_DEFAULT: &str = "master";

/// The compiled release-line prefix, used where nothing else answers.
pub const LINE_PREFIX_DEFAULT: &str = "release/";

/// The `landing` table.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Landing {
    /// P: worktree or branches.
    pub workflow: Option<Workflow>,
    /// P: trunk or lines.
    pub style: Option<Style>,
    /// P: opt-in Nix capability.
    pub nix: Option<bool>,
}

/// The `security` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Security {
    /// N: project receiving vulnerability reports.
    pub advisories: String,
    /// N: contact when the form is unavailable.
    pub contact: String,
    /// N: best-effort or a response window.
    pub response: String,
}

impl Default for Security {
    fn default() -> Self {
        Self {
            advisories: String::new(),
            contact: String::new(),
            response: "best-effort".into(),
        }
    }
}

/// The `setup` table.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Setup {
    /// N: the check the merge must pass.
    pub required_check: String,
    /// N: long-lived branches retired by the trunk.
    pub retired_branches: Vec<String>,
    /// P: release-line branch prefix, rendered into the release triggers
    /// and branch guards a landing writes. Absent means unanswered, so a
    /// record's own answer survives an upgrade that predates the key.
    pub line_prefix: Option<String>,
    /// N: run release-line protection in a full apply.
    pub release_lines: bool,
    /// Public bot identity.
    pub bot: Bot,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            required_check: String::new(),
            retired_branches: vec!["main".into(), "develop".into()],
            line_prefix: None,
            release_lines: false,
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
// These are independent policy switches in the committed TOML schema.
#[allow(clippy::struct_excessive_bools)]
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
    /// F: invariant, contains all four rules.
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
    /// F: invariant, zero.
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

fn parse(text: &str) -> Result<Config, RkError> {
    let raw: toml::Value =
        toml::from_str(text).map_err(|error: toml::de::Error| invalid(error.to_string()))?;
    if raw.get("schema_version").and_then(toml::Value::as_integer) != Some(SCHEMA_VERSION) {
        return Err(invalid(format!("schema_version must be {SCHEMA_VERSION}")));
    }
    let config: Config = toml::from_str(text).map_err(|error: toml::de::Error| {
        let mut message = error.to_string();
        if let Some(rest) = error.message().strip_prefix("unknown field `") {
            let names: Vec<_> = rest.split('`').collect();
            if let Some(unknown) = names.first() {
                if let Some(nearest) = names
                    .iter()
                    .skip(2)
                    .step_by(2)
                    .min_by_key(|name| distance(unknown, name))
                {
                    let _ = write!(message, "; nearest known key: {nearest}");
                }
            }
        }
        invalid(message)
    })?;
    if !config.project.forge.is_empty()
        && crate::detect::Forge::parse(&config.project.forge).is_none()
    {
        return Err(invalid("project.forge must be github or gitlab"));
    }
    if !config.project.tech.is_empty()
        && (config.project.tech.starts_with('_')
            || crate::embedded::SNIPPETS
                .get_dir(&config.project.tech)
                .is_none())
    {
        return Err(invalid(
            "project.tech must name a supported payload binding",
        ));
    }
    floors::check(&config)?;
    Ok(config)
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

#[allow(clippy::too_many_lines)]
fn render(config: &Config) -> Result<Vec<u8>, RkError> {
    // The trunk names the ruleset the setup installs, so the written
    // configuration states the name a target actually gets rather than a
    // literal that would be wrong for any trunk but the default.
    let trunk = config
        .project
        .trunk
        .clone()
        .ok_or_else(|| invalid("project.trunk is unresolved"))?;
    let mut fields: Vec<(&str, toml_edit::Value)> = vec![
        ("RK_CONFIG_SCHEMA_VERSION", config.schema_version.into()),
        ("RK_CONFIG_PROJECT_REPO", config.project.repo.clone().into()),
        (
            "RK_CONFIG_PROJECT_FORGE",
            config.project.forge.clone().into(),
        ),
        ("RK_CONFIG_PROJECT_TECH", config.project.tech.clone().into()),
        ("RK_CONFIG_PROJECT_TRUNK", trunk.clone().into()),
        (
            "RK_CONFIG_LANDING_WORKFLOW",
            config
                .landing
                .workflow
                .ok_or_else(|| invalid("landing.workflow is unresolved"))?
                .as_str()
                .into(),
        ),
        (
            "RK_CONFIG_LANDING_STYLE",
            config
                .landing
                .style
                .ok_or_else(|| invalid("landing.style is unresolved"))?
                .as_str()
                .into(),
        ),
        (
            "RK_CONFIG_LANDING_NIX",
            config
                .landing
                .nix
                .ok_or_else(|| invalid("landing.nix is unresolved"))?
                .into(),
        ),
        (
            "RK_CONFIG_SECURITY_ADVISORIES",
            config.security.advisories.clone().into(),
        ),
        (
            "RK_CONFIG_SECURITY_CONTACT",
            config.security.contact.clone().into(),
        ),
        (
            "RK_CONFIG_SECURITY_RESPONSE",
            config.security.response.clone().into(),
        ),
        (
            "RK_CONFIG_SETUP_REQUIRED_CHECK",
            config.setup.required_check.clone().into(),
        ),
        (
            "RK_CONFIG_SETUP_RETIRED_BRANCHES",
            array(&config.setup.retired_branches),
        ),
        (
            "RK_CONFIG_SETUP_LINE_PREFIX",
            config
                .setup
                .line_prefix
                .clone()
                .ok_or_else(|| invalid("setup.line_prefix is unresolved"))?
                .into(),
        ),
        (
            "RK_CONFIG_SETUP_RELEASE_LINES",
            config.setup.release_lines.into(),
        ),
        (
            "RK_CONFIG_SETUP_BOT_APP_ID",
            config.setup.bot.app_id.clone().into(),
        ),
    ];
    fields.extend(protection_fields(&config.protection, trunk.as_str()));
    let template = crate::embedded::BLOCKS
        .get_file("target-config.toml.in")
        .and_then(include_dir::File::contents_utf8)
        .ok_or_else(|| invalid("the binary lacks its configuration template"))?;
    // Each authored line has one token. Substitute in the source line once,
    // so a user's string containing another token stays literal.
    let mut bytes = Vec::new();
    for line in template.split_inclusive('\n') {
        if let Some((token, value)) = fields.iter().find(|(token, _)| line.contains(token)) {
            bytes.extend(crate::landing::substitute(
                line.as_bytes(),
                token.as_bytes(),
                value.to_string().as_bytes(),
            ));
        } else {
            bytes.extend_from_slice(line.as_bytes());
        }
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

/// Change one landing parameter while preserving comments and table ordering.
///
/// # Errors
/// Refuses an invalid key, invalid resulting content, or unreadable file; writes atomically.
pub fn rewrite_key(target: &Path, key: &str, value: toml_edit::Value) -> Result<(), RkError> {
    let path = target.join(CONFIG_PATH);
    let text = std::fs::read_to_string(&path)?;
    let next = rewrite_text(&text, key, value)?;
    crate::atomic::write(&path, next.as_bytes())?;
    Ok(())
}

fn rewrite_text(text: &str, key: &str, mut value: toml_edit::Value) -> Result<String, RkError> {
    if ![
        "project.repo",
        "project.forge",
        "project.tech",
        "project.trunk",
        "landing.workflow",
        "landing.style",
        "landing.nix",
        "setup.line_prefix",
    ]
    .contains(&key)
    {
        return Err(invalid(format!("{key} is not a landing parameter")));
    }
    parse(text)?;
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|error| invalid(error.to_string()))?;
    let mut item = document.as_item_mut();
    for segment in key.split('.') {
        item = &mut item[segment];
    }
    if let Some(old) = item.as_value() {
        if old
            .as_str()
            .zip(value.as_str())
            .is_some_and(|(old, new)| old == new)
            || old
                .as_bool()
                .zip(value.as_bool())
                .is_some_and(|(old, new)| old == new)
        {
            return Ok(text.to_owned());
        }
        *value.decor_mut() = old.decor().clone();
    }
    *item = toml_edit::Item::Value(value);
    let next = document.to_string();
    parse(&next)?;
    Ok(next)
}

/// Resolved landing input, including every key a preview would write.
#[derive(Debug, serde::Serialize)]
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
        let mut resolved = existing.cloned().unwrap_or_default();
        resolved.project.tech = params.tech().into();
        resolved.project.forge = params.forge().into();
        resolved.project.repo = params.repo().into();
        resolved.landing = Landing {
            workflow: Some(params.workflow()),
            style: params.style(),
            nix: Some(params.nix()),
        };
        resolved.project.trunk = Some(params.trunk().to_owned());
        resolved.setup.line_prefix = Some(params.line_prefix().to_owned());
        let content = if existing.is_some() {
            let mut text = std::fs::read_to_string(target.join(CONFIG_PATH))?;
            for (key, value) in parameter_values(&resolved) {
                text = rewrite_text(&text, key, value)?;
            }
            text
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

fn parameter_values(config: &Config) -> Vec<(&'static str, toml_edit::Value)> {
    let mut values = Vec::new();
    for (key, value) in [
        ("project.repo", &config.project.repo),
        ("project.forge", &config.project.forge),
        ("project.tech", &config.project.tech),
    ] {
        if !value.is_empty() {
            values.push((key, value.clone().into()));
        }
    }
    if let Some(value) = config.landing.workflow {
        values.push(("landing.workflow", value.as_str().into()));
    }
    if let Some(value) = config.landing.style {
        values.push(("landing.style", value.as_str().into()));
    }
    if let Some(value) = config.landing.nix {
        values.push(("landing.nix", value.into()));
    }
    if let Some(value) = config.project.trunk.clone() {
        values.push(("project.trunk", value.into()));
    }
    if let Some(value) = config.setup.line_prefix.clone() {
        values.push(("setup.line_prefix", value.into()));
    }
    values
}

/// Only explicit class P answers can be pending; comparisons still use the record.
#[must_use]
pub fn pending(config: &Config, record: &crate::landing::manifest::Manifest) -> Vec<String> {
    let mut recorded = Config::default();
    recorded.project.repo.clone_from(&record.parameters.repo);
    recorded.project.forge.clone_from(&record.forge);
    recorded.project.tech.clone_from(&record.tech);
    recorded.landing = Landing {
        workflow: Some(record.parameters.workflow),
        style: record.parameters.style,
        nix: Some(record.parameters.nix),
    };
    recorded.project.trunk = Some(record.parameters.trunk.clone());
    recorded.setup.line_prefix = Some(record.parameters.line_prefix.clone());
    let baseline = parameter_values(&recorded);
    parameter_values(config)
        .into_iter()
        .filter(|(key, value)| {
            !baseline
                .iter()
                .any(|(other, old)| key == other && value.to_string() == old.to_string())
        })
        .map(|(key, _)| key.to_owned())
        .collect()
}

/// The trunk accessor for callers without a setup context.
///
/// # Errors
/// Propagates invalid configuration and I/O failures.
pub fn trunk_of(target: &Path) -> Result<String, RkError> {
    Ok(load(target)?
        .and_then(|config| config.project.trunk)
        .unwrap_or_else(|| TRUNK_DEFAULT.to_owned()))
}

/// The release-line prefix for callers without a setup context.
///
/// # Errors
/// Propagates invalid configuration and I/O failures.
pub fn line_prefix_of(target: &Path) -> Result<String, RkError> {
    Ok(load(target)?
        .and_then(|config| config.setup.line_prefix)
        .unwrap_or_else(|| LINE_PREFIX_DEFAULT.to_owned()))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{CONFIG_PATH, Config, load, parse, rewrite_key, trunk_of, write};
    use crate::landing::{Style, Workflow};

    #[test]
    fn an_omitted_landing_key_is_distinguishable_from_an_explicit_default() {
        let omitted = parse("schema_version = 1\n").expect("omitted answers parse");
        let explicit = parse(
            "schema_version = 1\n[landing]\nworkflow = 'worktree'\nstyle = 'trunk'\nnix = false\n",
        )
        .expect("explicit defaults parse");
        assert_eq!(omitted.landing, super::Landing::default());
        assert_eq!(explicit.landing.workflow, Some(Workflow::Worktree));
        assert_eq!(explicit.landing.style, Some(Style::Trunk));
        assert_eq!(explicit.landing.nix, Some(false));
        assert_ne!(omitted, explicit);
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

    #[test]
    fn the_landed_config_template_round_trips() {
        let dir = tempfile::tempdir().expect("a target exists");
        let mut config = Config::default();
        config.project.repo = "acme/nested/widget".into();
        config.project.forge = "gitlab".into();
        config.project.tech = "bash".into();
        config.project.trunk = Some("main".into());
        config.landing.workflow = Some(Workflow::Branches);
        config.landing.style = Some(Style::Lines);
        config.landing.nix = Some(true);
        config.security.advisories = "acme/private".into();
        config.security.contact = "A \"quoted\" contact\nRK_CONFIG_SECURITY_RESPONSE\\end".into();
        config.security.response = "90d".into();
        config.setup.required_check = "build / test".into();
        config.setup.retired_branches = vec!["develop".into(), "old\"branch".into()];
        config.setup.line_prefix = Some("stable/".into());
        config.setup.release_lines = true;
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
        let defaults = Config {
            landing: super::Landing {
                workflow: Some(Workflow::Worktree),
                style: Some(Style::Trunk),
                nix: Some(false),
            },
            project: super::Project {
                trunk: Some(super::TRUNK_DEFAULT.into()),
                ..super::Project::default()
            },
            setup: super::Setup {
                line_prefix: Some(super::LINE_PREFIX_DEFAULT.into()),
                ..super::Setup::default()
            },
            // Writing resolves the derived ruleset name, so the file states
            // the name the setup installs rather than leaving it implied.
            protection: super::Protection {
                trunk_ruleset: Some(format!("{}-protection", super::TRUNK_DEFAULT)),
                ..super::Protection::default()
            },
            ..Config::default()
        };
        for expected in [defaults, config] {
            write(dir.path(), &expected).expect("the template renders");
            assert_eq!(load(dir.path()).expect("the config reads"), Some(expected));
            let text =
                std::fs::read_to_string(dir.path().join(CONFIG_PATH)).expect("the text reads");
            assert!(text.contains("# P: project path"));
            assert!(text.contains("# F: invariant"));
        }
    }

    #[test]
    fn a_config_with_an_unknown_key_refuses_by_name() {
        for (table, typo, nearest) in [
            ("", "schemax_version", "schema_version"),
            ("project", "trunkx", "trunk"),
            ("landing", "stile", "style"),
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
            let text = format!("schema_version = 1\n{header}{typo} = 'value'\n");
            let error = parse(&text).expect_err("unknown keys refuse").to_string();
            for expected in [CONFIG_PATH, typo, &format!("nearest known key: {nearest}")] {
                assert!(error.contains(expected), "{error}");
            }
        }
    }

    #[test]
    fn a_config_at_an_unknown_schema_refuses() {
        for text in ["schema_version = 999", "schema_version = '1'", ""] {
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
        let error = parse("schema_version = 1\n[project\n")
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
            "schema_version = 1\n[protection]\nstrict_required_status_checks = false\n",
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
        let original = "# Project answers\nschema_version = 1\n\n[security] # first table stays first\ncontact = 'team' # keep me\n\n[landing]\n# Our release choice\nstyle  = 'trunk'  # keep this reason\nworkflow = 'branches'\n";
        let path = dir.path().join(CONFIG_PATH);
        std::fs::write(&path, original).expect("a config exists");
        rewrite_key(dir.path(), "landing.style", "lines".into()).expect("the style writes back");
        let text = std::fs::read_to_string(&path).expect("the text reads");
        assert_eq!(text, original.replace("'trunk'", "\"lines\""));
        assert_eq!(
            load(dir.path())
                .expect("the config reads")
                .expect("present")
                .landing
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
        let before = std::fs::read(&path).expect("the bytes read");
        for (key, value) in [("security.contact", "other"), ("landing.style", "unknown")] {
            assert!(rewrite_key(dir.path(), key, value.into()).is_err());
            assert_eq!(std::fs::read(&path).expect("the bytes read"), before);
        }
    }
}
