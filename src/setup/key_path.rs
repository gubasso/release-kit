//! Where the release bot's private key file is.
//!
//! One App key signs for one App, so the key's default place is named by
//! the App ID: `<root>/release-kit/bot/github/<app-id>.pem`, under the
//! user configuration root. Two sources override it, first match wins:
//! `RK_BOT_PRIVATE_KEY_FILE`, for one run against another file, and an
//! entry for the App ID in the user configuration file
//! `<root>/release-kit/config.toml`. The committed target configuration
//! never holds a key path: a path on one machine is a coordinate no other
//! machine shares.
//!
//! SATISFIES bot-key:the-key-has-one-default-path
//! SATISFIES bot-key:three-sources-name-the-key-in-a-fixed-order

use std::collections::BTreeMap;
use std::ffi::OsString;

use camino::Utf8PathBuf;
use serde::Deserialize;

use super::secrets::{self, PRIVATE_KEY_FILE};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// Which source named the key path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `RK_BOT_PRIVATE_KEY_FILE`.
    Variable,
    /// The entry for the App ID in the user configuration file.
    UserConfig,
    /// The default path for the App ID.
    Default,
}

impl Source {
    /// The word a refusal and a journal use for the source.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Variable => "variable",
            Self::UserConfig => "user-configuration",
            Self::Default => "default",
        }
    }

    /// The source, named for a person.
    #[must_use]
    pub const fn named(self) -> &'static str {
        match self {
            Self::Variable => PRIVATE_KEY_FILE,
            Self::UserConfig => "the rk user configuration file",
            Self::Default => "the default key path",
        }
    }
}

/// A key path and the source that named it.
#[derive(Debug, Clone)]
pub struct KeyPath {
    /// The path as its source gave it, before any resolution.
    pub raw: OsString,
    /// Which source named it.
    pub source: Source,
}

/// What the resolution found.
#[derive(Debug)]
pub enum Resolution {
    /// A source named a path.
    Named(KeyPath),
    /// No variable is set and no App ID is resolved, so neither the user
    /// configuration nor the default path can name a file.
    NoAppId,
}

/// The user configuration root: `XDG_CONFIG_HOME` where it is an absolute
/// path, else `$HOME/.config`. A relative `XDG_CONFIG_HOME` is ignored,
/// as the XDG Base Directory specification says.
///
/// # Errors
///
/// Refuses where neither variable gives a root.
pub fn config_root() -> Result<Utf8PathBuf, RkError> {
    let xdg = secrets::value_of("XDG_CONFIG_HOME")
        .and_then(|value| Utf8PathBuf::from_path_buf(value.into()).ok())
        .filter(|path| path.is_absolute());
    if let Some(root) = xdg {
        return Ok(root);
    }
    let home = secrets::value_of("HOME")
        .and_then(|value| Utf8PathBuf::from_path_buf(value.into()).ok())
        .filter(|path| !path.as_str().is_empty());
    home.map(|home| home.join(".config")).ok_or_else(|| {
        RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                "neither XDG_CONFIG_HOME nor HOME gives a configuration root",
            )
            .expected("an absolute XDG_CONFIG_HOME, or HOME")
            .action("set HOME, or XDG_CONFIG_HOME to an absolute path, and run it again")
            .step("bot-secrets"),
        )
    })
}

/// The folder that holds the default key files.
#[must_use]
pub fn key_dir(root: &Utf8PathBuf) -> Utf8PathBuf {
    root.join("release-kit/bot/github")
}

/// The default key path of one App.
#[must_use]
pub fn default_path(root: &Utf8PathBuf, app_id: &str) -> Utf8PathBuf {
    key_dir(root).join(format!("{app_id}.pem"))
}

/// The rk user configuration file.
#[must_use]
pub fn user_config_path(root: &Utf8PathBuf) -> Utf8PathBuf {
    root.join("release-kit/config.toml")
}

/// The user configuration file's schema: one table of key paths by App ID.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserConfig {
    #[serde(default)]
    bot: Bot,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bot {
    #[serde(default)]
    github: Github,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Github {
    #[serde(default)]
    private_key_files: BTreeMap<String, String>,
}

/// The user configuration's key path for `app_id`, absent where the file
/// or the entry is absent.
///
/// # Errors
///
/// Refuses a file that does not parse, an unknown key, an App ID that is
/// not digits, and a path that is not absolute, each by name.
pub fn user_config_entry(root: &Utf8PathBuf, app_id: &str) -> Result<Option<Utf8PathBuf>, RkError> {
    let path = user_config_path(root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(refuse(&path, &format!("it cannot be read: {err}")));
        }
    };
    let parsed: UserConfig = toml::from_str(&text)
        .map_err(|err| refuse(&path, &format!("it does not parse: {}", err.message())))?;
    let mut found = None;
    for (id, file) in parsed.bot.github.private_key_files {
        if id.is_empty() || !id.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(refuse(
                &path,
                &format!("the App ID '{id}' in [bot.github.private_key_files] is not digits"),
            ));
        }
        let file = Utf8PathBuf::from(file);
        if !file.is_absolute() {
            return Err(refuse(
                &path,
                &format!(
                    "the key path '{file}' for App ID {id} in [bot.github.private_key_files] is not absolute"
                ),
            ));
        }
        if id == app_id {
            found = Some(file);
        }
    }
    Ok(found)
}

/// The key path, by the fixed order: the variable, then the user
/// configuration entry for the App ID, then the default path.
///
/// # Errors
///
/// Refuses a user configuration file that is wrong, and a missing
/// configuration root where a source needs one.
pub fn resolve(app_id: Option<&str>) -> Result<Resolution, RkError> {
    if let Some(raw) = secrets::value_of(PRIVATE_KEY_FILE) {
        return Ok(Resolution::Named(KeyPath {
            raw,
            source: Source::Variable,
        }));
    }
    let Some(app_id) = app_id else {
        return Ok(Resolution::NoAppId);
    };
    let root = config_root()?;
    if let Some(file) = user_config_entry(&root, app_id)? {
        return Ok(Resolution::Named(KeyPath {
            raw: file.into_string().into(),
            source: Source::UserConfig,
        }));
    }
    Ok(Resolution::Named(KeyPath {
        raw: default_path(&root, app_id).into_string().into(),
        source: Source::Default,
    }))
}

/// One refusal of the user configuration file.
fn refuse(path: &Utf8PathBuf, problem: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(Reason::PrerequisiteUnmet, format!("{path}: {problem}"))
            .expected(
                "one table [bot.github.private_key_files] of \"<app-id>\" = \"<absolute path>\"",
            )
            .action(format!("correct or remove {path}"))
            .step("bot-secrets"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(text: &str) -> Result<Option<Utf8PathBuf>, RkError> {
        let dir = tempfile::tempdir().expect("a scratch root");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8");
        std::fs::create_dir_all(root.join("release-kit")).expect("the folder");
        std::fs::write(user_config_path(&root), text).expect("the file writes");
        user_config_entry(&root, "314159")
    }

    #[test]
    fn the_default_path_names_the_app_id() {
        let root = Utf8PathBuf::from("/cfg");
        assert_eq!(
            default_path(&root, "314159"),
            "/cfg/release-kit/bot/github/314159.pem"
        );
        assert_eq!(user_config_path(&root), "/cfg/release-kit/config.toml");
    }

    #[test]
    fn the_user_config_reads_the_entry_for_the_app_id() {
        let found = entry("[bot.github.private_key_files]\n\"314159\" = \"/keys/a.pem\"\n\"27\" = \"/keys/b.pem\"\n")
            .expect("parses");
        assert_eq!(found.as_deref(), Some(camino::Utf8Path::new("/keys/a.pem")));
        let none =
            entry("[bot.github.private_key_files]\n\"27\" = \"/keys/b.pem\"\n").expect("parses");
        assert!(none.is_none());
    }

    #[test]
    fn the_user_config_refuses_a_relative_path_and_a_non_numeric_app_id() {
        let relative = entry("[bot.github.private_key_files]\n\"314159\" = \"keys/a.pem\"\n")
            .expect_err("a relative path refuses");
        assert!(relative.to_string().contains("not absolute"), "{relative}");
        let named = entry("[bot.github.private_key_files]\n\"my-app\" = \"/keys/a.pem\"\n")
            .expect_err("a name refuses");
        assert!(named.to_string().contains("'my-app'"), "{named}");
        let unknown = entry("[bot.github]\nkey = \"/keys/a.pem\"\n").expect_err("unknown refuses");
        assert!(unknown.to_string().contains("key"), "{unknown}");
    }

    #[test]
    fn an_absent_user_config_is_no_error() {
        let dir = tempfile::tempdir().expect("a scratch root");
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("utf-8");
        assert!(
            user_config_entry(&root, "314159")
                .expect("absent")
                .is_none()
        );
    }
}
