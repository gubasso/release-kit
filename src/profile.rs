//! The resolved target configuration.
//!
//! The project profile, the Git workflow, and the capability requests,
//! resolved by one precedence from an invocation's flags, the committed
//! configuration, a compatible record, the target's observation, and the
//! compiled defaults.
//!
//! Three representations live here and stay apart. The declared
//! configuration is `crate::config::Config`, exactly as authored. The
//! resolved configuration is [`Resolved`]: every effective value beside the
//! runtime [`Source`] that answered it, which `rk profile` reports and
//! nothing serializes. The wire form is [`Params`]: the same values with no
//! source, which the projection consumes, the configuration writes back,
//! and the record carries as [`ProfileSnapshot`], [`GitWorkflow`], and
//! [`CapabilityRequests`].
//!
//! SATISFIES project-profile:every-field-resolves-by-one-precedence
//! SATISFIES project-profile:a-record-is-source-free

pub mod catalog;

use std::collections::BTreeMap;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, CheckoutMode, Provider, Style};

/// The release intent's mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseMode {
    /// release-kit drives the release: a bot maintains the request, and
    /// the landed automation tags and publishes.
    Automatic,
    /// The target releases through a process release-kit does not drive.
    /// No automation lands, and no bot-operate chapter applies.
    External,
    /// Nothing releases.
    None,
}

impl ReleaseMode {
    /// The flag, wire, and report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::External => "external",
            Self::None => "none",
        }
    }

    /// Parse a `--release-mode` flag value.
    ///
    /// # Errors
    ///
    /// Returns [`RkError::Usage`] naming the three values.
    pub fn parse(raw: &str) -> Result<Self, RkError> {
        match raw {
            "automatic" => Ok(Self::Automatic),
            "external" => Ok(Self::External),
            "none" => Ok(Self::None),
            other => Err(RkError::Usage(format!(
                "unknown release mode '{other}'; the modes are: automatic, external, none"
            ))),
        }
    }
}

/// The release intent: the mode and, for an automatic release, its driver,
/// style, and line prefix.
///
/// SATISFIES project-profile:release-intent-has-three-modes
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseIntent {
    /// The mode.
    pub mode: ReleaseMode,
    /// The technology that states the version and takes the bot; present
    /// for an automatic release alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver: Option<String>,
    /// The release style; present for an automatic release alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<Style>,
    /// The release-line branch prefix; present for an automatic release
    /// alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_prefix: Option<String>,
}

/// What the project is, on the wire: values alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSnapshot {
    /// The technologies present, sorted, zero or many.
    pub technologies: Vec<String>,
    /// The forge, where the project has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forge: Option<String>,
    /// The release intent.
    pub release: ReleaseIntent,
}

/// The Git workflow parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitWorkflow {
    /// The one permanent branch.
    pub trunk: String,
    /// Where a topic branch opens.
    pub checkout_mode: CheckoutMode,
}

/// The optional products the target requested.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequests {
    /// The seeded package expression and the seed flake pair.
    #[serde(default)]
    pub nix_packaging: bool,
    /// The landed vulnerability reporting policy.
    #[serde(default)]
    pub reporting_policy: bool,
    /// The `OpenSSF` Scorecard workflow.
    #[serde(default)]
    pub scorecard: bool,
    /// The code scanning workflow, by provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_scanning: Option<Provider>,
}

/// Where a resolved value came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// An invocation flag.
    Flag,
    /// The committed configuration.
    Config,
    /// A compatible landing record.
    Record,
    /// The target's observation: its version files and its origin remote.
    Observation,
    /// A compiled default.
    Default,
}

impl Source {
    /// Where this source sits in the one precedence, lowest first.
    ///
    /// The comparison a resolution needs when one field's answer has to be
    /// weighed against another's: a value only contradicts a decision that
    /// its own tier or a lower one made.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Flag => 0,
            Self::Config => 1,
            Self::Record => 2,
            Self::Observation => 3,
            Self::Default => 4,
        }
    }

    /// The report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Config => "config",
            Self::Record => "record",
            Self::Observation => "observation",
            Self::Default => "default",
        }
    }
}

/// What the observation proposes for the release, where nothing else
/// answered it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "state")]
pub enum Proposal {
    /// No release-bearing technology: nothing to automate.
    None,
    /// Exactly one release-bearing technology drives the release.
    Automatic {
        /// The driver.
        driver: String,
    },
    /// More than one release-bearing technology, so a flag must name the
    /// driver before an apply.
    Ambiguous {
        /// The candidates, sorted.
        drivers: Vec<String>,
    },
}

/// The complete resolved input to a projection, on the wire.
///
/// The values the configuration writes back, the record carries, and the
/// projection renders from, with no precedence source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Params {
    profile: ProfileSnapshot,
    git: GitWorkflow,
    capabilities: CapabilityRequests,
    repo: String,
    security_contact: String,
    security_response: String,
}

/// Explicit invocation answers; absence falls through to configuration.
#[derive(Default)]
pub struct Inputs<'a> {
    /// The technologies, replacing the declared list whole; empty is
    /// unsupplied.
    pub technologies: &'a [String],
    /// Forge override.
    pub forge: Option<&'a str>,
    /// Repository override.
    pub repo: Option<&'a str>,
    /// Release mode override.
    pub release_mode: Option<ReleaseMode>,
    /// Release driver override.
    pub release_driver: Option<&'a str>,
    /// Release style override.
    pub style: Option<Style>,
    /// Trunk override.
    pub trunk: Option<&'a str>,
    /// Checkout mode override.
    pub checkout_mode: Option<CheckoutMode>,
    /// Nix packaging request override.
    pub nix: Option<bool>,
    /// Reporting policy request override.
    pub reporting_policy: Option<bool>,
    /// Scorecard request override.
    pub scorecard: Option<bool>,
    /// Code scanning override: `Some(None)` turns it off, and absence
    /// leaves the configuration and the record to answer.
    pub code_scanning: Option<Option<Provider>>,
}

/// Compatibility policy for a landing candidate.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A first landing.
    Init,
    /// A preview may leave the repository unresolved and reports an
    /// ambiguous release proposal rather than refusing it.
    Preview,
    /// An existing record supplies compatibility answers.
    Upgrade,
    /// A pre-record target requires an explicit release style.
    Adopt,
}

/// The resolved target configuration, with the source of every value.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The values.
    pub params: Params,
    /// The source of each value, keyed by its configuration path.
    pub sources: BTreeMap<&'static str, Source>,
    /// The category names the catalog does not know, preserved.
    pub unknown: Vec<String>,
    /// What the observation proposed for the release, where the mode was
    /// not answered above it.
    pub proposal: Option<Proposal>,
}

/// The canonical form of a category name, or why it is refused.
///
/// Lowercase, matching `[a-z0-9][a-z0-9-]*`. An unknown name is preserved, so the
/// shape is what keeps a record and a configuration readable.
///
/// SATISFIES project-profile:an-unknown-category-is-preserved
///
/// # Errors
/// The refusal text, naming the value and the shape.
pub fn canonical_category(raw: &str) -> Result<String, String> {
    let lowered = raw.trim().to_ascii_lowercase();
    let shaped = lowered
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && lowered
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if shaped {
        Ok(lowered)
    } else {
        Err(format!(
            "{raw:?} is not a category name; one is lowercase letters and digits with hyphens inside, as [a-z0-9][a-z0-9-]*"
        ))
    }
}

/// A list of category names canonicalized, refused on a duplicate, and
/// sorted.
///
/// # Errors
/// The refusal text, naming the key.
pub fn canonical_list(key: &str, raw: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(raw.len());
    for value in raw {
        let name = canonical_category(value).map_err(|reason| format!("{key}: {reason}"))?;
        if out.contains(&name) {
            return Err(format!("{key} names {name} twice; each technology once"));
        }
        out.push(name);
    }
    out.sort();
    Ok(out)
}

impl Params {
    /// Reconstruct every projection parameter from the record alone,
    /// including the compatibility defaults applied when it was loaded.
    #[must_use]
    pub fn from_record(record: &manifest::Manifest) -> Self {
        Self {
            profile: record.profile.clone(),
            git: record.git.clone(),
            capabilities: record.capabilities.clone(),
            repo: record.parameters.repo.clone(),
            security_contact: record.parameters.security_contact.clone(),
            security_response: record.parameters.security_response.clone(),
        }
    }

    /// Resolve flags, configuration, recorded compatibility inputs or
    /// observation, and finally the compiled defaults. Comparisons use
    /// `from_record` alone.
    ///
    /// # Errors
    /// Refuses unresolved identity, an invalid release state, or a style
    /// an existing target has not answered.
    pub fn resolve(
        target: &Utf8Path,
        flags: &Inputs<'_>,
        config: Option<&crate::config::Config>,
        record: Option<&manifest::Manifest>,
        purpose: Purpose,
    ) -> Result<Self, RkError> {
        resolve(target, flags, config, record, purpose).map(|resolved| resolved.params)
    }

    /// What the project is.
    #[must_use]
    pub const fn profile(&self) -> &ProfileSnapshot {
        &self.profile
    }

    /// The Git workflow parameters.
    #[must_use]
    pub const fn git(&self) -> &GitWorkflow {
        &self.git
    }

    /// The capability requests.
    #[must_use]
    pub const fn capabilities(&self) -> &CapabilityRequests {
        &self.capabilities
    }

    /// The technologies present, sorted.
    #[must_use]
    pub fn technologies(&self) -> &[String] {
        &self.profile.technologies
    }

    /// The forge, where the project has one.
    #[must_use]
    pub fn forge(&self) -> Option<&str> {
        self.profile.forge.as_deref()
    }

    /// The release mode.
    #[must_use]
    pub const fn release_mode(&self) -> ReleaseMode {
        self.profile.release.mode
    }

    /// The release driver, for an automatic release.
    #[must_use]
    pub fn driver(&self) -> Option<&str> {
        self.profile.release.driver.as_deref()
    }

    /// The release style, for an automatic release that answered it.
    #[must_use]
    pub const fn style(&self) -> Option<Style> {
        self.profile.release.style
    }

    /// Whether this landing requested the Nix capability.
    #[must_use]
    pub const fn nix_packaging(&self) -> bool {
        self.capabilities.nix_packaging
    }

    /// Whether this landing requested the reporting policy.
    #[must_use]
    pub const fn reporting_policy(&self) -> bool {
        self.capabilities.reporting_policy
    }

    /// Whether this landing requested the Scorecard capability.
    #[must_use]
    pub const fn scorecard(&self) -> bool {
        self.capabilities.scorecard
    }

    /// The code scanning provider this landing requested, if any.
    #[must_use]
    pub const fn code_scanning(&self) -> Option<Provider> {
        self.capabilities.code_scanning
    }

    /// The project path used by parameter-bearing files, empty where the
    /// target has no forge repository.
    #[must_use]
    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// Where a topic branch opens.
    #[must_use]
    pub const fn checkout_mode(&self) -> CheckoutMode {
        self.git.checkout_mode
    }

    /// The one permanent branch this landing writes into its artifacts.
    #[must_use]
    pub fn trunk(&self) -> &str {
        &self.git.trunk
    }

    /// The release-line prefix this landing writes into its artifacts,
    /// the compiled default where the release intent carries none.
    #[must_use]
    pub fn line_prefix(&self) -> &str {
        self.profile
            .release
            .line_prefix
            .as_deref()
            .unwrap_or(crate::config::LINE_PREFIX_DEFAULT)
    }

    /// The contact the landed policy names, empty for the forge's own
    /// authored wording.
    #[must_use]
    pub fn security_contact(&self) -> &str {
        &self.security_contact
    }

    /// The acknowledgment window the landed policy promises.
    #[must_use]
    pub fn security_response(&self) -> &str {
        &self.security_response
    }

    /// The canonical identity and Git workflow flags, as `rk init` and
    /// `rk adopt` take them: every resolved answer stated, so a follow-up
    /// command a preview prints applies the decision that was previewed.
    #[must_use]
    pub fn canonical_flags(&self) -> String {
        let mut out = String::new();
        for technology in &self.profile.technologies {
            out.push_str(" --technology ");
            out.push_str(technology);
        }
        if let Some(forge) = &self.profile.forge {
            out.push_str(" --forge ");
            out.push_str(forge);
        }
        // A preview stands in for an unresolved repository with the
        // placeholder, and a replayed apply takes the operator's own path
        // rather than that stand-in, so the flag stays out of the command.
        if !self.repo.is_empty() && self.repo != crate::projection::REPO_PLACEHOLDER {
            out.push_str(" --repo ");
            out.push_str(&self.repo);
        }
        out.push_str(" --release-mode ");
        out.push_str(self.profile.release.mode.as_str());
        if let Some(driver) = &self.profile.release.driver {
            out.push_str(" --release-driver ");
            out.push_str(driver);
        }
        if let Some(style) = self.profile.release.style {
            out.push_str(" --release-style ");
            out.push_str(style.as_str());
        }
        out.push_str(" --trunk ");
        out.push_str(&self.git.trunk);
        out.push_str(" --checkout-mode ");
        out.push_str(self.git.checkout_mode.as_str());
        out
    }

    /// Every opt-in capability's flag, as `rk init` and `rk adopt` take it.
    ///
    /// The resolved answers, not the flags the caller typed: a follow-up
    /// command a preview prints must apply the decision that was previewed,
    /// and the preview's decision is what resolution produced.
    #[must_use]
    pub fn capability_flags(&self) -> String {
        let mut out = String::new();
        if self.capabilities.nix_packaging {
            out.push_str(" --nix-packaging");
        }
        if self.capabilities.reporting_policy {
            out.push_str(" --reporting-policy");
        }
        if self.capabilities.scorecard {
            out.push_str(" --scorecard");
        }
        // The provider flag takes a value, so `off` is a statable answer and
        // is stated: a committed `capabilities.code_scanning` would
        // otherwise re-enable on replay exactly what this preview turned
        // off. The boolean flags above have no off form, so absence is
        // their only honest rendering and no committed value can
        // contradict it.
        out.push_str(" --code-scanning ");
        out.push_str(
            self.capabilities
                .code_scanning
                .map_or("off", Provider::as_str),
        );
        out
    }

    /// The same answers as `rk upgrade` takes them, every one stated.
    ///
    /// An upgrade can turn a capability off as well as on, so absence is no
    /// answer there and each value is rendered explicitly. That is what makes
    /// a printed follow-up command reproduce the previewed decision rather
    /// than re-resolve the configured one.
    #[must_use]
    pub fn capability_toggles(&self) -> String {
        let word = |on: bool| if on { "on" } else { "off" };
        format!(
            " --nix-packaging {} --reporting-policy {} --scorecard {} --code-scanning {}",
            word(self.capabilities.nix_packaging),
            word(self.capabilities.reporting_policy),
            word(self.capabilities.scorecard),
            self.capabilities
                .code_scanning
                .map_or("off", Provider::as_str)
        )
    }
}

#[cfg(test)]
impl Params {
    /// A parameter set for tests alone: an automatic rust release on
    /// GitHub. Production code reaches `Params` through `from_record` and
    /// `resolve` and through nothing else, and this constructor is
    /// compiled out of the shipped binary.
    pub(crate) fn for_test(repo: &str, style: Option<Style>) -> Self {
        Self {
            profile: ProfileSnapshot {
                technologies: vec!["rust".to_owned()],
                forge: Some("github".to_owned()),
                release: ReleaseIntent {
                    mode: ReleaseMode::Automatic,
                    driver: Some("rust".to_owned()),
                    style,
                    line_prefix: Some(crate::config::LINE_PREFIX_DEFAULT.to_owned()),
                },
            },
            git: GitWorkflow {
                trunk: crate::config::TRUNK_DEFAULT.to_owned(),
                checkout_mode: CheckoutMode::LinkedWorktree,
            },
            capabilities: CapabilityRequests {
                nix_packaging: false,
                reporting_policy: true,
                scorecard: false,
                code_scanning: None,
            },
            repo: repo.to_owned(),
            security_contact: String::new(),
            security_response: crate::config::RESPONSE_DEFAULT.to_owned(),
        }
    }

    /// The same set with the two security parameters answered.
    pub(crate) fn for_test_security(contact: &str, response: &str) -> Self {
        Self {
            security_contact: contact.to_owned(),
            security_response: response.to_owned(),
            ..Self::for_test("acme/widget", Some(Style::Trunk))
        }
    }

    /// A release-less set for tests: the technologies and the forge as
    /// given, no driver, no style.
    pub(crate) fn for_test_release_less(
        technologies: &[&str],
        forge: Option<&str>,
        mode: ReleaseMode,
    ) -> Self {
        let mut params = Self::for_test("acme/widget", None);
        params.profile.technologies = technologies.iter().map(|t| (*t).to_owned()).collect();
        params.profile.forge = forge.map(str::to_owned);
        params.profile.release = ReleaseIntent {
            mode,
            driver: None,
            style: None,
            line_prefix: None,
        };
        params.capabilities.reporting_policy = false;
        if forge.is_none() {
            params.repo = String::new();
        }
        params
    }

    /// The same set with the forge and the driver changed.
    pub(crate) fn set_pair_for_test(&mut self, driver: &str, forge: &str) {
        self.profile.technologies = vec![driver.to_owned()];
        self.profile.release.driver = Some(driver.to_owned());
        self.profile.forge = Some(forge.to_owned());
    }

    /// The same set with the checkout mode answered.
    pub(crate) const fn set_checkout_mode_for_test(&mut self, mode: CheckoutMode) {
        self.git.checkout_mode = mode;
    }

    /// The same set with the Nix opt-in answered.
    pub(crate) const fn set_nix_for_test(&mut self, nix: bool) {
        self.capabilities.nix_packaging = nix;
    }

    /// The same set with the Scorecard opt-in answered.
    pub(crate) const fn set_scorecard_for_test(&mut self, scorecard: bool) {
        self.capabilities.scorecard = scorecard;
    }

    /// The same set with the code scanning provider answered.
    pub(crate) const fn set_code_scanning_for_test(&mut self, provider: Option<Provider>) {
        self.capabilities.code_scanning = provider;
    }
}

/// The refusal for an invalid release state, naming the key and its
/// valid shape.
fn invalid_release(message: impl std::fmt::Display) -> RkError {
    RkError::Usage(format!(
        "{message}; an automatic release names a driver among profile.technologies and a style, and an external or none release names neither"
    ))
}

/// One field's answer and where it came from.
fn answered<T>(chain: [(Option<T>, Source); 5]) -> Option<(T, Source)> {
    chain
        .into_iter()
        .find_map(|(value, source)| value.map(|value| (value, source)))
}

/// Resolve every domain value by one precedence, keeping the source of
/// each.
///
/// # Errors
/// Refuses unresolved identity, an invalid release state, a duplicate or
/// malformed category name, and a style an existing target has not
/// answered.
#[allow(
    clippy::too_many_lines,
    reason = "the resolution is one precedence walk per field, and splitting it would hide that every field walks the same chain"
)]
pub fn resolve(
    target: &Utf8Path,
    flags: &Inputs<'_>,
    config: Option<&crate::config::Config>,
    record: Option<&manifest::Manifest>,
    purpose: Purpose,
) -> Result<Resolved, RkError> {
    let mut sources: BTreeMap<&'static str, Source> = BTreeMap::new();
    let observed = crate::detect::observe(target.as_std_path());
    let known_drivers = catalog::known_drivers();

    // Technologies: a supplied list replaces the declared one whole.
    let (technologies, source) = answered([
        (
            (!flags.technologies.is_empty()).then(|| flags.technologies.to_vec()),
            Source::Flag,
        ),
        (
            config.and_then(|c| c.profile.technologies.clone()),
            Source::Config,
        ),
        (
            record.map(|r| r.profile.technologies.clone()),
            Source::Record,
        ),
        (
            Some(
                observed
                    .technologies
                    .iter()
                    .map(|t| (*t).to_owned())
                    .collect(),
            ),
            Source::Observation,
        ),
        (None, Source::Default),
    ])
    .unwrap_or_else(|| (Vec::new(), Source::Default));
    let technologies =
        canonical_list("profile.technologies", &technologies).map_err(RkError::Usage)?;
    sources.insert("profile.technologies", source);

    // The forge: an explicit empty configuration value states no forge.
    let forge_flag = flags
        .forge
        .map(|name| canonical_category(name).map_err(RkError::Usage))
        .transpose()?;
    let (forge, source) = answered([
        (forge_flag.map(Some), Source::Flag),
        (
            config
                .and_then(|c| c.profile.forge.clone())
                .map(|value| if value.is_empty() { None } else { Some(value) }),
            Source::Config,
        ),
        (record.map(|r| r.profile.forge.clone()), Source::Record),
        (
            observed.forge.map(|forge| Some(forge.as_str().to_owned())),
            Source::Observation,
        ),
        (Some(None), Source::Default),
    ])
    .unwrap_or((None, Source::Default));
    let forge = forge
        .map(|name| canonical_category(&name).map_err(RkError::Usage))
        .transpose()?;
    sources.insert("profile.forge", source);

    // The repository identity, needed only where a forge is present.
    let (repo, source) = answered([
        (flags.repo.map(str::to_owned), Source::Flag),
        (
            config
                .map(|c| c.project.repo.clone())
                .filter(|value| !value.is_empty()),
            Source::Config,
        ),
        (
            record
                .map(|r| r.parameters.repo.clone())
                .filter(|value| !value.is_empty()),
            Source::Record,
        ),
        (observed.repo.clone(), Source::Observation),
        (None, Source::Default),
    ])
    .map_or((None, Source::Default), |(value, source)| {
        (Some(value), source)
    });
    sources.insert("project.repo", source);

    // The release mode: the observation proposes where nothing above
    // answers.
    let release_bearing: Vec<String> = technologies
        .iter()
        .filter(|name| known_drivers.contains(name))
        .cloned()
        .collect();
    let proposal = match release_bearing.as_slice() {
        [] => Proposal::None,
        [one] => Proposal::Automatic {
            driver: one.clone(),
        },
        many => Proposal::Ambiguous {
            drivers: many.to_vec(),
        },
    };
    // The proposal reads the version files alone. A missing forge is not
    // an answer about the release intent: it is a separate refusal the
    // automatic branch below raises, naming the remote it did not find and
    // the two ways out. Folding it in here would silently land a
    // release-less target for a crate whose author simply has no remote
    // yet, and the record would then claim a release intent nobody stated.
    let proposed_mode = match &proposal {
        Proposal::Automatic { .. } | Proposal::Ambiguous { .. } => ReleaseMode::Automatic,
        Proposal::None => ReleaseMode::None,
    };
    let (mode, source) = answered([
        (flags.release_mode, Source::Flag),
        (config.and_then(|c| c.profile.release.mode), Source::Config),
        (record.map(|r| r.profile.release.mode), Source::Record),
        (Some(proposed_mode), Source::Observation),
        (None, Source::Default),
    ])
    .unwrap_or((ReleaseMode::None, Source::Default));
    let mode_source = source;
    sources.insert("profile.release.mode", mode_source);
    let mode_answered_above = mode_source != Source::Observation;
    let proposal = (!mode_answered_above).then_some(proposal);

    // The driver, style, and line prefix belong to an automatic release
    // alone, and a value from a flag or the configuration under another
    // mode is a malformed intent rather than an ignored one.
    let driver_flag = flags
        .release_driver
        .map(|name| canonical_category(name).map_err(RkError::Usage))
        .transpose()?;
    let (driver, driver_source) = answered([
        (driver_flag, Source::Flag),
        (
            config.and_then(|c| c.profile.release.driver.clone()),
            Source::Config,
        ),
        (
            record.and_then(|r| r.profile.release.driver.clone()),
            Source::Record,
        ),
        (
            match &proposal {
                Some(Proposal::Automatic { driver }) if mode == ReleaseMode::Automatic => {
                    Some(driver.clone())
                }
                _ => None,
            },
            Source::Observation,
        ),
        (None, Source::Default),
    ])
    .map_or((None, Source::Default), |(value, source)| {
        (Some(value), source)
    });
    let (style, style_source) = answered([
        (flags.style, Source::Flag),
        (config.and_then(|c| c.profile.release.style), Source::Config),
        (record.and_then(|r| r.profile.release.style), Source::Record),
        (None, Source::Observation),
        (
            (mode == ReleaseMode::Automatic && matches!(purpose, Purpose::Init | Purpose::Preview))
                .then_some(Style::Trunk),
            Source::Default,
        ),
    ])
    .map_or((None, Source::Default), |(value, source)| {
        (Some(value), source)
    });
    let (line_prefix, prefix_source) = answered([
        (None, Source::Flag),
        (
            config.and_then(|c| c.profile.release.line_prefix.clone()),
            Source::Config,
        ),
        (
            record.and_then(|r| r.profile.release.line_prefix.clone()),
            Source::Record,
        ),
        (None, Source::Observation),
        (
            (mode == ReleaseMode::Automatic).then(|| crate::config::LINE_PREFIX_DEFAULT.to_owned()),
            Source::Default,
        ),
    ])
    .map_or((None, Source::Default), |(value, source)| {
        (Some(value), source)
    });

    // A reporting purpose never refuses what it can state: `rk profile`
    // and every preview report an intent the target cannot yet take, and
    // the capability catalog says why. A purpose that writes refuses,
    // because a record must not claim a release nothing can land.
    let reporting = purpose == Purpose::Preview;
    let release = match mode {
        ReleaseMode::Automatic => {
            if let Some(Proposal::Ambiguous { drivers }) = &proposal
                && driver.is_none()
                && !reporting
            {
                return Err(RkError::Usage(format!(
                    "the target carries more than one release-bearing technology, {}, and nothing names the driver; pass --release-driver <name>, or set profile.release.driver in {}",
                    drivers.join(" and "),
                    crate::config::CONFIG_PATH
                )));
            }
            if forge.is_none() && !reporting {
                let message = observed.host.map_or_else(
                    || "no forge detected: the target has no origin remote, and an automatic release needs one".to_owned(),
                    |host| format!("no forge detected: the host {host} is not recognized, and an automatic release needs one"),
                );
                return Err(RkError::refusal(
                    Diagnostic::new(Reason::ForgeUndetected, message)
                        .expected("a github.com or gitlab remote, or --forge")
                        .action("pass --forge <github|gitlab>, or --release-mode none for a project that releases nothing"),
                ));
            }
            if driver.is_none() && !reporting {
                return Err(invalid_release(format!(
                    "profile.release.mode is automatic and no driver is named; pass --release-driver <{}>",
                    known_drivers.join("|")
                )));
            }
            if let Some(driver) = &driver
                && !technologies.contains(driver)
                && !reporting
            {
                return Err(invalid_release(format!(
                    "profile.release.driver names {driver}, which profile.technologies does not carry ({})",
                    if technologies.is_empty() {
                        "empty".to_owned()
                    } else {
                        technologies.join(", ")
                    }
                )));
            }
            let style = match (style, purpose) {
                (None, Purpose::Upgrade | Purpose::Adopt) => {
                    return Err(RkError::Usage(
                        "the target carries no style parameter; set profile.release.style in .release-kit/config.toml or pass --release-style <trunk|lines>".into(),
                    ));
                }
                (None, _) => Style::Trunk,
                (Some(style), _) => style,
            };
            sources.insert("profile.release.driver", driver_source);
            sources.insert("profile.release.style", style_source);
            sources.insert("profile.release.line_prefix", prefix_source);
            ReleaseIntent {
                mode,
                driver,
                style: Some(style),
                line_prefix: Some(
                    line_prefix.unwrap_or_else(|| crate::config::LINE_PREFIX_DEFAULT.to_owned()),
                ),
            }
        }
        ReleaseMode::External | ReleaseMode::None => {
            // A stated value under a mode that has no room for it is a
            // malformed intent. A value the mode outranks is not: that is
            // ordinary precedence, and `--release-mode none` over a
            // configured automatic release is the one command that retires
            // it. So the refusal fires only where the subordinate value
            // speaks at or above the tier that chose the mode.
            for (key, present, value_source) in [
                ("profile.release.driver", driver.is_some(), driver_source),
                ("profile.release.style", style.is_some(), style_source),
                (
                    "profile.release.line_prefix",
                    line_prefix.is_some(),
                    prefix_source,
                ),
            ] {
                if present
                    && matches!(value_source, Source::Flag | Source::Config)
                    && value_source.rank() <= mode_source.rank()
                {
                    return Err(invalid_release(format!(
                        "{key} is set while profile.release.mode is {}",
                        mode.as_str()
                    )));
                }
            }
            ReleaseIntent {
                mode,
                driver: None,
                style: None,
                line_prefix: None,
            }
        }
    };

    // Every capability this binary ships for a forge renders the project
    // path, so the identity is required exactly where the forge has an
    // adapter. An unknown forge selects no such capability and needs none.
    let adapter_known = forge
        .as_deref()
        .is_some_and(|name| crate::detect::Forge::parse(name).is_some());
    let repo = match (forge.is_some(), adapter_known, repo) {
        // No forge at all: the identity has nowhere to point, so a lower
        // tier's remote or record must not survive into `[project]`.
        (false, _, _) => String::new(),
        (true, false, repo) => repo.unwrap_or_default(),
        (true, true, Some(repo)) => repo,
        (true, true, None) if purpose == Purpose::Preview => {
            crate::projection::REPO_PLACEHOLDER.to_owned()
        }
        (true, true, None) => return Err(crate::landing::repo_unresolved()),
    };

    // The Git workflow.
    let (trunk, source) = answered([
        (flags.trunk.map(str::to_owned), Source::Flag),
        (config.and_then(|c| c.git.trunk.clone()), Source::Config),
        (record.map(|r| r.git.trunk.clone()), Source::Record),
        (None, Source::Observation),
        (
            Some(crate::config::TRUNK_DEFAULT.to_owned()),
            Source::Default,
        ),
    ])
    .unwrap_or_else(|| (crate::config::TRUNK_DEFAULT.to_owned(), Source::Default));
    sources.insert("git.trunk", source);
    let (checkout_mode, source) = answered([
        (flags.checkout_mode, Source::Flag),
        (config.and_then(|c| c.git.checkout_mode), Source::Config),
        (record.map(|r| r.git.checkout_mode), Source::Record),
        (None, Source::Observation),
        (
            Some(if purpose == Purpose::Adopt {
                CheckoutMode::MainWorktree
            } else {
                CheckoutMode::LinkedWorktree
            }),
            Source::Default,
        ),
    ])
    .unwrap_or((CheckoutMode::LinkedWorktree, Source::Default));
    sources.insert("git.checkout_mode", source);

    // The capability requests.
    let (nix_packaging, source) = answered([
        (flags.nix, Source::Flag),
        (
            config.and_then(|c| c.capabilities.nix_packaging),
            Source::Config,
        ),
        (record.map(|r| r.capabilities.nix_packaging), Source::Record),
        (None, Source::Observation),
        (Some(false), Source::Default),
    ])
    .unwrap_or((false, Source::Default));
    sources.insert("capabilities.nix_packaging", source);
    let (reporting_policy, source) = answered([
        (flags.reporting_policy, Source::Flag),
        (
            config.and_then(|c| c.capabilities.reporting_policy),
            Source::Config,
        ),
        (
            record.map(|r| r.capabilities.reporting_policy),
            Source::Record,
        ),
        (None, Source::Observation),
        // A project that automates its release carries the policy by
        // default; a release-less profile asks for it explicitly.
        (
            Some(release.mode == ReleaseMode::Automatic),
            Source::Default,
        ),
    ])
    .unwrap_or((false, Source::Default));
    sources.insert("capabilities.reporting_policy", source);
    let (scorecard, source) = answered([
        (flags.scorecard, Source::Flag),
        (
            config.and_then(|c| c.capabilities.scorecard),
            Source::Config,
        ),
        (record.map(|r| r.capabilities.scorecard), Source::Record),
        (None, Source::Observation),
        (Some(false), Source::Default),
    ])
    .unwrap_or((false, Source::Default));
    sources.insert("capabilities.scorecard", source);
    let configured_scanning = config
        .and_then(|c| c.capabilities.code_scanning.as_deref())
        .map(Provider::parse)
        .transpose()?;
    let (code_scanning, source) = answered([
        (flags.code_scanning, Source::Flag),
        (configured_scanning, Source::Config),
        (record.map(|r| r.capabilities.code_scanning), Source::Record),
        (None, Source::Observation),
        (Some(None), Source::Default),
    ])
    .unwrap_or((None, Source::Default));
    sources.insert("capabilities.code_scanning", source);
    // A requested scanner this release cannot land at these dimensions is
    // an unavailable optional capability, not a malformed request. The
    // catalog reports it and the landing omits it, which is what
    // `project-profile:an-operation-refuses-only-what-it-requires` says
    // must happen: only the selected release automation blocks an apply.

    // The security policy's two answers.
    let (security_contact, source) = answered([
        (None, Source::Flag),
        (
            config.and_then(|c| c.security.contact.clone()),
            Source::Config,
        ),
        (
            record.map(|r| r.parameters.security_contact.clone()),
            Source::Record,
        ),
        (None, Source::Observation),
        (Some(String::new()), Source::Default),
    ])
    .unwrap_or((String::new(), Source::Default));
    let security_contact =
        crate::config::canonical_contact(&security_contact).map_err(crate::config::invalid)?;
    sources.insert("security.contact", source);
    let (security_response, source) = answered([
        (None, Source::Flag),
        (
            config.and_then(|c| c.security.response.clone()),
            Source::Config,
        ),
        (
            record.map(|r| r.parameters.security_response.clone()),
            Source::Record,
        ),
        (None, Source::Observation),
        (
            Some(crate::config::RESPONSE_DEFAULT.to_owned()),
            Source::Default,
        ),
    ])
    .unwrap_or_else(|| (crate::config::RESPONSE_DEFAULT.to_owned(), Source::Default));
    let security_response =
        crate::config::canonical_response(&security_response).map_err(crate::config::invalid)?;
    sources.insert("security.response", source);

    let mut unknown: Vec<String> = technologies
        .iter()
        .filter(|name| !known_drivers.contains(name))
        .map(|name| format!("technology {name}"))
        .collect();
    if let Some(name) = &forge
        && crate::detect::Forge::parse(name).is_none()
    {
        unknown.push(format!("forge {name}"));
    }

    Ok(Resolved {
        params: Params {
            profile: ProfileSnapshot {
                technologies,
                forge,
                release,
            },
            git: GitWorkflow {
                trunk,
                checkout_mode,
            },
            capabilities: CapabilityRequests {
                nix_packaging,
                reporting_policy,
                scorecard,
                code_scanning,
            },
            repo,
            security_contact,
            security_response,
        },
        sources,
        unknown,
        proposal,
    })
}
