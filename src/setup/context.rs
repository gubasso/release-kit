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

// The trunk every setup asserts is the one permanent branch the target
// states in its own committed configuration, read through `Ctx::trunk`.
// A target that names none keeps the compiled default, so a landing
// predating the key behaves exactly as it did.

/// A policy boolean as the JSON word a forge body carries.
const fn bool_word(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// A policy list as the JSON array a forge body carries. Every element
/// passed the floor table, so this quotes rather than escapes.
fn json_list(values: &[String]) -> String {
    let inner: Vec<String> = values.iter().map(|value| format!("\"{value}\"")).collect();
    format!("[{}]", inner.join(", "))
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
    /// The project path on the forge.
    pub repo: String,
    /// The forge the run acts on.
    pub forge: Forge,
    /// The remote host, where one was detected.
    pub host: Option<String>,
    /// The value of `--required-check`, where given.
    pub required_check: Option<String>,
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
    /// The ruleset that makes published tags immutable.
    tag_ruleset: String,
    /// The ruleset that protects the release lines.
    lines_ruleset: String,
    /// The context the landed title job reports under.
    title_check: String,
    /// The floored policy this target states. Every value here passed the
    /// floor table at load, so a run can pass it to a step without
    /// judging it again.
    protection: crate::config::Protection,
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
        let Some(forge) = forge_flag.or(detected.forge) else {
            let diagnostic = detected.host.as_ref().map_or_else(
                || {
                    Diagnostic::new(
                        Reason::ForgeUndetected,
                        "no forge detected: the target has no origin remote",
                    )
                },
                |host| {
                    Diagnostic::new(
                        Reason::ForgeUndetected,
                        format!("no forge detected: the host {host} is not recognized"),
                    )
                },
            );
            let diagnostic = diagnostic
                .expected("a github.com or gitlab remote, or an override")
                .action("pass --forge <github|gitlab>, and --repo <path> if the remote is absent");
            // An unrecognized host is a refusal, never a default; a
            // missing remote is absent input, in the sysexits sense.
            return Err(if detected.host.is_some() {
                RkError::refusal(diagnostic)
            } else {
                RkError::missing(diagnostic)
            });
        };

        let Some(repo) = repo_flag.map(str::to_owned).or(detected.repo) else {
            return Err(RkError::missing(
                Diagnostic::new(
                    Reason::ForgeUndetected,
                    "no repository detected: the target has no origin remote",
                )
                .expected("an origin remote naming the project")
                .action("pass --repo <owner/name>"),
            ));
        };
        let cli = resolve_cli(forge)?;
        let config = crate::config::load(target.as_std_path())?;
        let answers = config
            .as_ref()
            .map_or_else(crate::config::Setup::default, |held| held.setup.clone());
        // The flag wins, and the committed answer fills the gap on GitHub
        // alone: GitLab names no individual check and refuses a supplied
        // one, so a shared configuration must not make that refusal fire.
        let required_check = required_check.map(str::to_owned).or_else(|| {
            Some(answers.required_check.clone())
                .filter(|name| !name.is_empty() && forge == Forge::Github)
        });
        let bot_app_id = Some(answers.bot.app_id.clone()).filter(|id| !id.is_empty());
        let trunk = crate::config::trunk_of(target.as_std_path())?;
        let protection = config
            .as_ref()
            .map_or_else(crate::config::Protection::default, |held| {
                held.protection.clone()
            });
        Ok(Self {
            target: target.clone(),
            repo,
            forge,
            host: detected.host,
            required_check,
            cli,
            tech: detect::tech_of(target.as_std_path()),
            trunk_ruleset: protection.trunk_ruleset(&trunk),
            tag_ruleset: protection.tag_ruleset.clone(),
            lines_ruleset: protection.lines_ruleset.clone(),
            title_check: protection.title_check.clone(),
            protection,
            trunk,
            line_prefix: crate::config::line_prefix_of(target.as_std_path())?,
            retired_branches: answers.retired_branches,
            release_lines: answers.release_lines,
            excluded_steps: answers.excluded_steps,
            bot_app_id,
        })
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
            target,
            repo,
            forge,
            host: None,
            required_check: None,
            cli,
            tech,
            trunk: crate::config::TRUNK_DEFAULT.to_owned(),
            line_prefix: crate::config::LINE_PREFIX_DEFAULT.to_owned(),
            retired_branches: crate::config::Setup::default().retired_branches,
            release_lines: false,
            excluded_steps: std::collections::BTreeMap::new(),
            bot_app_id: None,
            trunk_ruleset: format!("{}-protection", crate::config::TRUNK_DEFAULT),
            tag_ruleset: defaults.tag_ruleset.clone(),
            lines_ruleset: defaults.lines_ruleset.clone(),
            title_check: defaults.title_check.clone(),
            protection: defaults,
        }
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

    /// The context the landed title job reports under.
    #[must_use]
    pub fn title_check(&self) -> &str {
        &self.title_check
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
        self.forge == Forge::Gitlab
            && self
                .host
                .as_deref()
                .is_some_and(|host| host != "gitlab.com")
    }

    /// The constructed environment a step receives. Secrets enter only for
    /// the step that consumes them; the caller records their handling.
    #[must_use]
    pub fn child_env(&self, step: &str) -> Vec<(OsString, OsString)> {
        let mut env: Vec<(OsString, OsString)> = vec![
            ("RK_FORGE".into(), self.forge.as_str().into()),
            ("RK_REPO".into(), self.repo.clone().into()),
            ("RK_TRUNK_BRANCH".into(), self.trunk.clone().into()),
            ("RK_LINE_PREFIX".into(), self.line_prefix.clone().into()),
            ("RK_TRUNK_RULESET".into(), self.trunk_ruleset.clone().into()),
            ("RK_TAG_RULESET".into(), self.tag_ruleset.clone().into()),
            ("RK_LINES_RULESET".into(), self.lines_ruleset.clone().into()),
            ("RK_TITLE_CHECK".into(), self.title_check.clone().into()),
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
            (
                "RK_GITLAB_MERGE_LEVEL".into(),
                self.protection.gitlab.merge_access_level.to_string().into(),
            ),
            ("GH_PAGER".into(), "".into()),
            ("GLAB_PAGER".into(), "".into()),
        ];
        if let Some(check) = &self.required_check {
            if self.forge == Forge::Github && matches!(step, "protect-trunk" | "protections-check")
            {
                env.push(("RK_REQUIRED_CHECK".into(), check.clone().into()));
            }
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
        let overridden = std::env::var_os(match self.forge {
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
                    "{name} is not on PATH, and every {} step calls it",
                    forge.as_str()
                ),
            )
            .expected(format!("the {name} CLI installed and authenticated"))
            .action(format!("install {name}, then run {name} auth login")),
        )
    })
}
