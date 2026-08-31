//! The resolved context one setup run works in: target, repository, forge,
//! the forge CLI binary, and the environment a step receives.
//!
//! The environment is constructed, not inherited: `env_clear` plus exactly
//! the declared variables, the forge CLI's own configuration and
//! authentication variables, and — only for the steps that need them — the
//! bot credentials. The parent's environment does not leak into a
//! privileged child, and no secret is ever an argv value.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use camino::Utf8PathBuf;

use crate::detect::{self, Forge};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// The trunk every setup asserts: the one permanent branch; named so a
/// later option can change it.
pub const TRUNK_BRANCH: &str = "master";

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

/// The secret-bearing variables, forwarded only to the step that consumes
/// them and recorded in the journal as handling, never as value.
pub const SECRET_VARS: [&str; 3] = ["RK_BOT_APP_ID", "RK_BOT_PRIVATE_KEY", "RK_BOT_TOKEN"];

/// One resolved run context.
#[derive(Debug)]
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
        Ok(Self {
            target: target.clone(),
            repo,
            forge,
            host: detected.host,
            required_check: required_check.map(str::to_owned),
            cli,
            tech: detect::tech_of(target.as_std_path()),
        })
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
            ("RK_TRUNK_BRANCH".into(), TRUNK_BRANCH.into()),
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
        if matches!(step, "install-bot" | "bot-secrets") {
            for name in SECRET_VARS {
                if let Some(value) = std::env::var_os(name) {
                    env.push((name.into(), value));
                }
            }
            if let Some(value) = std::env::var_os("RK_BOT_INSTALLATION") {
                env.push(("RK_BOT_INSTALLATION".into(), value));
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

    /// The secret values present in the operator's environment, for
    /// redaction; never logged, never echoed.
    #[must_use]
    pub fn secret_values() -> Vec<Vec<u8>> {
        SECRET_VARS
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .filter(|value| !value.is_empty())
            .map(String::into_bytes)
            .collect()
    }
}

/// Resolve the forge CLI once, at context time: the `RK_GH_BIN` and
/// `RK_GLAB_BIN` overrides first, then a `PATH` search. Not found and not
/// executable are distinct failures, in the shell convention.
fn resolve_cli(forge: Forge) -> Result<PathBuf, RkError> {
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
