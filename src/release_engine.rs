//! The release engine: `rk release request`, `rk release build`, and
//! `rk release publish`, for each driver that has an adapter, on GitHub
//! and GitLab.
//!
//! The engine reads every fact it acts on from the landing record: the
//! driver, the forge, the repository, the trunk, the style, the
//! integration, the line prefix, and the registry answer. It takes no flag
//! that restates one. The judgments are pure and live in [`guard`]; the
//! actions read, judge, and write through the driver [`adapter`], the
//! pinned [`cliff`], and the forge CLIs in [`forge`]. Each action returns
//! its report, and the handler in `src/commands/release.rs` renders it.

pub mod adapter;
pub mod build;
pub mod cliff;
pub mod forge;
pub mod guard;
pub mod publish;
pub mod request;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::detect::Forge;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::landing::manifest::{self, Integration, MergeForm, Style};
use crate::profile::{BuildAnswer, ReleaseMode};
use crate::release_state::{self, Registry};

use self::adapter::DriverAdapter;

/// The branch prefix of the engine's one release request per base: the
/// request for `<base>` lives on `rk-release/<base>`.
pub const REQUEST_PREFIX: &str = "rk-release/";

/// The directory the build writes its report, its notes, and its
/// checksums into, beside `dist/` and never inside it, because a registry
/// upload sends every file `dist/` holds.
pub const OUT_DIR: &str = ".rk-release";

/// The build report inside [`OUT_DIR`], which `rk release publish` reads.
pub const BUILD_REPORT: &str = "build.json";

/// The release notes inside [`OUT_DIR`].
pub const NOTES: &str = "notes.md";

/// The checksum file inside [`OUT_DIR`], one `<sha256>  <name>` per
/// artifact.
pub const CHECKSUMS: &str = "SHA256SUMS";

/// The recovery section for a tag at another commit, or one nobody could
/// read.
pub const RECOVERY_TAG: &str = "The tag exists and the release does not";

/// The recovery section for bytes that differ from what already shipped.
pub const RECOVERY_DEFECTIVE: &str = "A published version is defective";

/// What one action returns: its report, the human lines, and the next
/// steps, rendered by the handler.
#[derive(Debug)]
pub struct Done<R: Serialize> {
    /// The machine report.
    pub report: R,
    /// The human report, one line each.
    pub lines: Vec<String>,
    /// The next steps the human report names.
    pub next: Vec<String>,
}

/// Everything the record answers for one engine run.
pub struct Context {
    /// The checkout the action runs in.
    pub target: Utf8PathBuf,
    /// The forge.
    pub forge: Forge,
    /// The project path on the forge.
    pub repo: String,
    /// The trunk branch.
    pub trunk: String,
    /// The release style.
    pub style: Option<Style>,
    /// The integration authority.
    pub integration: Integration,
    /// The merge form an armed request merges with.
    pub merge_form: MergeForm,
    /// The release-line branch prefix.
    pub line_prefix: String,
    /// The registry the release publishes to, where it has one.
    pub registry: Option<Registry>,
    /// The package name the registry knows, where the driver names one.
    pub package: Option<String>,
    /// The driver adapter.
    pub adapter: Box<dyn DriverAdapter>,
}

impl Context {
    /// Read the record at `target`.
    ///
    /// # Errors
    ///
    /// A refusal where the target carries no landing, or a landing the
    /// engine does not release.
    pub fn load(target: &Utf8Path) -> Result<Self, RkError> {
        let refuse = |message: String, expected: &str| {
            RkError::refusal(
                Diagnostic::new(Reason::StateDrift, message)
                    .expected(expected.to_owned())
                    .target_state("unchanged"),
            )
        };
        if !target.is_dir() {
            return Err(RkError::missing(
                Diagnostic::new(
                    Reason::TargetNotFound,
                    format!("target {target} is not a directory"),
                )
                .expected("the checkout the release job runs in"),
            ));
        }
        let record = manifest::load(target)?.ok_or_else(|| {
            refuse(
                format!("no landing at {target}"),
                "a target release-kit landed",
            )
        })?;
        let release = &record.profile.release;
        if release.mode != ReleaseMode::Automatic {
            return Err(refuse(
                "the record selects no automated release".to_owned(),
                "profile.release.mode = \"automatic\"",
            ));
        }
        let driver = release.driver.clone().unwrap_or_default();
        let command = release
            .build
            .as_ref()
            .and_then(|build| build.command.clone())
            .unwrap_or_else(|| BuildAnswer::COMMAND.to_owned());
        let adapter = adapter::for_driver(&driver, &command).ok_or_else(|| {
            refuse(
                format!("the release driver {driver:?} does not release through the rk engine"),
                "a bash or python release driver; a rust release runs release-plz",
            )
        })?;
        let forge = record
            .profile
            .forge
            .as_deref()
            .and_then(Forge::parse)
            .ok_or_else(|| {
                refuse(
                    "the record names no forge this binary releases on".to_owned(),
                    "a github or gitlab forge",
                )
            })?;
        if record.parameters.repo.is_empty() {
            return Err(refuse(
                "the record names no forge repository".to_owned(),
                "the project path recorded at landing",
            ));
        }
        Ok(Self {
            package: adapter::package_name(target.as_std_path(), &driver),
            registry: release_state::registry_for(&driver, release.registry),
            target: target.to_owned(),
            forge,
            repo: record.parameters.repo.clone(),
            trunk: record.git.trunk.clone(),
            style: release.style,
            integration: record.git.integration,
            merge_form: record.git.merge_form,
            line_prefix: release
                .line_prefix
                .clone()
                .unwrap_or_else(|| crate::config::LINE_PREFIX_DEFAULT.to_owned()),
            adapter,
        })
    }

    /// The driver's name.
    #[must_use]
    pub fn driver(&self) -> &'static str {
        self.adapter.driver()
    }

    /// The branch the push landed on: the checked-out branch, or on a
    /// detached CI checkout the branch the forge's job names.
    #[must_use]
    pub fn pushed_branch(&self) -> Option<String> {
        if let Ok(output) = git(&self.target, &["symbolic-ref", "--short", "-q", "HEAD"])
            && output.status.success()
        {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !branch.is_empty() {
                return Some(branch);
            }
        }
        let from = |var: &str| std::env::var(var).ok().filter(|value| !value.is_empty());
        from("CI_COMMIT_BRANCH").or_else(|| {
            (std::env::var("GITHUB_REF_TYPE").as_deref() == Ok("branch"))
                .then(|| from("GITHUB_REF_NAME"))
                .flatten()
        })
    }

    /// Whether `branch` is a release base: the trunk, or a release line
    /// `<prefix><major>.<minor>`.
    #[must_use]
    pub fn is_base(&self, branch: &str) -> bool {
        branch == self.trunk || self.is_line(branch)
    }

    /// Whether `branch` is a release line.
    #[must_use]
    pub fn is_line(&self, branch: &str) -> bool {
        branch
            .strip_prefix(&self.line_prefix)
            .and_then(|line| line.split_once('.'))
            .is_some_and(|(major, minor)| {
                <[&str; 2]>::from((major, minor))
                    .iter()
                    .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
            })
    }

    /// The commit the checkout stands at.
    ///
    /// # Errors
    ///
    /// A refusal where git cannot resolve `HEAD`.
    pub fn head(&self) -> Result<String, RkError> {
        git_line(&self.target, &["rev-parse", "HEAD"], "a commit checked out")
    }
}

/// The request branch for a base.
#[must_use]
pub fn request_branch(base: &str) -> String {
    format!("{REQUEST_PREFIX}{base}")
}

/// The variables through which git takes a commit identity over its
/// configuration. A job or a hook may export them, and the release commit
/// must carry the identity the forge names for the bot's token instead.
const IDENTITY_VARS: [&str; 4] = [
    "GIT_AUTHOR_NAME",
    "GIT_AUTHOR_EMAIL",
    "GIT_COMMITTER_NAME",
    "GIT_COMMITTER_EMAIL",
];

/// Run one git command against `root`, with the hook and identity
/// variables scrubbed.
///
/// A run under a hook then acts on `root` alone, and a commit takes the
/// identity its command configures.
///
/// # Errors
///
/// A refusal where git does not run.
pub fn git(root: &Utf8Path, args: &[&str]) -> Result<std::process::Output, RkError> {
    let mut command = std::process::Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS
        .iter()
        .chain(&IDENTITY_VARS)
    {
        command.env_remove(var);
    }
    command
        .arg("-C")
        .arg(root.as_std_path())
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(args)
        .output()
        .map_err(|source| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("git did not run: {source}"),
                )
                .expected("git installed and on PATH"),
            )
        })
}

/// Run one git command that must succeed, with its failure typed.
///
/// # Errors
///
/// The command's failure, with git's last line.
pub fn git_ok(root: &Utf8Path, args: &[&str], expected: &str) -> Result<String, RkError> {
    let output = git(root, args)?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(RkError::subprocess(
        Diagnostic::new(Reason::SubprocessFailed, last_line(&output.stderr))
            .expected(expected.to_owned()),
    ))
}

/// The trimmed output of one git command that must succeed.
///
/// # Errors
///
/// The command's failure.
pub fn git_line(root: &Utf8Path, args: &[&str], expected: &str) -> Result<String, RkError> {
    git_ok(root, args, expected).map(|text| text.trim().to_owned())
}

/// The last non-empty line of a tool's output.
#[must_use]
pub fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no detail")
        .trim()
        .to_owned()
}

/// The SHA-256 of a file, as lowercase hex.
///
/// # Errors
///
/// The read's failure.
pub fn file_sha256(path: &Utf8Path) -> Result<String, RkError> {
    Ok(crate::digest::Digest::of(&std::fs::read(path).map_err(RkError::Io)?).to_string())
}

/// A conflict with what the forge or the registry already holds, named
/// with its recovery section.
#[must_use]
pub fn conflict(message: String, recovery: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(Reason::ReleaseConflict, message)
            .expected("a release whose tag and bytes agree with this commit's build")
            .action(format!("rk method recovery: {recovery}"))
            .target_state("no further forge write ran"),
    )
}

/// The file name of a path.
#[must_use]
pub fn file_name(path: &Utf8Path) -> String {
    path.file_name().unwrap_or(path.as_str()).to_owned()
}

/// An embedded forge text with its tokens replaced.
///
/// # Errors
///
/// A block this binary does not embed, a defect in the binary.
pub fn forge_text(block: &str, tokens: &[(&str, &str)]) -> Result<String, RkError> {
    let mut text = crate::projection::embedded_block(block)?
        .trim_end_matches('\n')
        .to_owned();
    for (token, value) in tokens {
        text = text.replace(token, value);
    }
    Ok(text)
}

/// The default CI token's variable, where the credential the forge CLI
/// would push with is that token alone.
///
/// A push made with the default token starts no pipeline, so the release
/// half would never run. `GH_TOKEN` and `GITLAB_TOKEN` carry the bot
/// credential; a job that sets only `GITHUB_TOKEN` or `CI_JOB_TOKEN`, or
/// sets the bot variable to the same value, pushes as the default token.
#[must_use]
pub fn default_ci_token(
    forge: Forge,
    var: impl Fn(&str) -> Option<String>,
) -> Option<&'static str> {
    let (bot, default) = match forge {
        Forge::Github => ("GH_TOKEN", "GITHUB_TOKEN"),
        Forge::Gitlab => ("GITLAB_TOKEN", "CI_JOB_TOKEN"),
    };
    let set = |name: &str| var(name).filter(|value| !value.is_empty());
    let default_value = set(default)?;
    match set(bot) {
        Some(value) if value != default_value => None,
        _ => Some(default),
    }
}

/// Push to `origin` with the bot credential the job sets.
///
/// SATISFIES release-engine:the-request-pushes-as-the-bot
///
/// An https remote is pushed without any credential its URL embeds, such
/// as a CI job token, through a credential helper that reads the bot
/// token's variable at push time, so rk never holds the token.
///
/// # Errors
///
/// The push's failure, with git's last line.
pub fn push(context: &Context, args: &[&str]) -> Result<(), RkError> {
    let url = git_line(
        &context.target,
        &["remote", "get-url", "origin"],
        "an origin remote",
    )?;
    let (user, var) = match context.forge {
        Forge::Github => ("x-access-token", "GH_TOKEN"),
        Forge::Gitlab => ("oauth2", "GITLAB_TOKEN"),
    };
    let mut command: Vec<String> = Vec::new();
    let mut remote = url.clone();
    if let Some(rest) = url.strip_prefix("https://")
        && std::env::var(var).is_ok_and(|value| !value.is_empty())
    {
        let host_path = rest
            .split_once('/')
            .map_or(rest, |(authority, _)| authority);
        let bare = host_path
            .rsplit_once('@')
            .map_or(host_path, |(_, host)| host);
        remote = format!(
            "https://{bare}{}",
            rest.find('/').map_or("", |at| &rest[at..])
        );
        command.extend([
            "-c".to_owned(),
            "credential.helper=".to_owned(),
            "-c".to_owned(),
            format!(
                "credential.helper=!f() {{ echo username={user}; echo \"password=${var}\"; }}; f"
            ),
        ]);
    }
    command.push("push".to_owned());
    command.push(remote);
    command.extend(args.iter().map(|arg| (*arg).to_owned()));
    let borrowed: Vec<&str> = command.iter().map(String::as_str).collect();
    git_ok(
        &context.target,
        &borrowed,
        "a push the remote accepts under the bot credential",
    )
    .map(drop)
}

#[cfg(test)]
mod tests {
    use super::build::{Artifact, BuildReport};
    use super::publish::PublishReport;
    use super::request::RequestReport;

    /// SATISFIES release-engine:the-output-declares-its-schema
    ///
    /// The three reports, held by snapshot: a renamed or dropped field
    /// fails here and becomes a schema-version bump.
    #[test]
    fn each_engine_report_holds_its_schema() {
        let request = RequestReport {
            schema: super::request::SCHEMA,
            state: "opened",
            reason: None,
            base: Some("master".into()),
            version: Some("0.1.0".into()),
            proposal: Some("0.1.1".into()),
            tag: Some("v0.1.1".into()),
            request: Some(7),
            armed: Some(true),
            files: vec!["VERSION".into(), "CHANGELOG.md".into()],
        };
        assert_eq!(
            serde_json::to_string(&request).expect("serializes"),
            r#"{"schema":"rk.release-request/1","state":"opened","base":"master","version":"0.1.0","proposal":"0.1.1","tag":"v0.1.1","request":7,"armed":true,"files":["VERSION","CHANGELOG.md"]}"#
        );
        let build = BuildReport {
            schema: super::build::SCHEMA.into(),
            state: "release".into(),
            reason: None,
            version: "1.4.3rc2".into(),
            tag: "v1.4.3-rc.2".into(),
            candidate: true,
            commit: "aaa".into(),
            artifacts: vec![Artifact {
                path: "dist/acme-1.4.3rc2.tar.gz".into(),
                sha256: "00".into(),
            }],
            checksums: Some(".rk-release/SHA256SUMS".into()),
            notes: Some(".rk-release/notes.md".into()),
        };
        assert_eq!(
            serde_json::to_string(&build).expect("serializes"),
            r#"{"schema":"rk.release-build/1","state":"release","version":"1.4.3rc2","tag":"v1.4.3-rc.2","candidate":true,"commit":"aaa","artifacts":[{"path":"dist/acme-1.4.3rc2.tar.gz","sha256":"00"}],"checksums":".rk-release/SHA256SUMS","notes":".rk-release/notes.md"}"#
        );
        let publish = PublishReport {
            schema: super::publish::SCHEMA,
            state: "resumed",
            reason: None,
            tag: "v0.1.0".into(),
            candidate: false,
            uploaded: vec!["widget-0.1.0.tar.gz.sha256".into()],
            replaced: Vec::new(),
            kept: vec!["widget-0.1.0.tar.gz".into()],
            linked: Vec::new(),
        };
        assert_eq!(
            serde_json::to_string(&publish).expect("serializes"),
            r#"{"schema":"rk.release-publish/1","state":"resumed","tag":"v0.1.0","candidate":false,"uploaded":["widget-0.1.0.tar.gz.sha256"],"replaced":[],"kept":["widget-0.1.0.tar.gz"],"linked":[]}"#
        );
    }

    #[test]
    fn only_the_default_token_alone_is_the_default_token() {
        use crate::detect::Forge;
        let vars = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
        assert_eq!(
            super::default_ci_token(Forge::Github, vars(&[("GITHUB_TOKEN", "j")])),
            Some("GITHUB_TOKEN")
        );
        assert_eq!(
            super::default_ci_token(
                Forge::Github,
                vars(&[("GITHUB_TOKEN", "j"), ("GH_TOKEN", "j")])
            ),
            Some("GITHUB_TOKEN")
        );
        assert_eq!(
            super::default_ci_token(
                Forge::Github,
                vars(&[("GITHUB_TOKEN", "j"), ("GH_TOKEN", "b")])
            ),
            None
        );
        assert_eq!(
            super::default_ci_token(Forge::Gitlab, vars(&[("CI_JOB_TOKEN", "j")])),
            Some("CI_JOB_TOKEN")
        );
        assert_eq!(
            super::default_ci_token(
                Forge::Gitlab,
                vars(&[("CI_JOB_TOKEN", "j"), ("GITLAB_TOKEN", "b")])
            ),
            None
        );
        assert_eq!(super::default_ci_token(Forge::Gitlab, vars(&[])), None);
    }
}
