//! `rk setup import-key`: move a downloaded bot App private key to its
//! default path, owner-only.
//!
//! The verb changes nothing on a forge, and `install-bot` already observes
//! its result. It runs every check of the key lookup on the download but
//! the mode, writes the bytes to a temporary file with mode 0600 in the key
//! folder, renames that file onto the default path, reads the written key
//! back through the lookup, and only then deletes the download. It prints
//! no key byte.
//!
//! SATISFIES bot-key:the-import-checks-and-writes-the-key-atomically

use std::io::Write as _;

use camino::{Utf8Path, Utf8PathBuf};

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::output::Output;
use crate::setup::app_jwt;
use crate::setup::key_path::{self, Source};
use crate::setup::secrets::{self, KeyLookup, PRIVATE_KEY_FILE, Resolved};

/// Import `file` to the default key path of the App that `target`
/// resolves.
///
/// # Errors
///
/// Refuses without an App ID, where another source names the key, where
/// the default path or a folder on it is a symbolic link, where the key
/// folder is open beyond its owner, where a key already stands there
/// without `--replace`, and for every wrong download.
pub fn run(out: Output, file: &Utf8Path, target: &Utf8Path, replace: bool) -> Result<(), RkError> {
    secrets::refuse_legacy_key()?;
    if !target.is_dir() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory; nothing was imported"),
            )
            .expected("the repository whose configuration names the App ID"),
        ));
    }
    let configured = crate::config::load(target.as_std_path())?
        .map(|config| config.setup.bot.app_id)
        .filter(|id| !id.is_empty());
    let Some(app_id) = app_jwt::app_id(configured.as_deref())? else {
        return Err(refuse(
            secrets::NO_APP_ID.to_owned(),
            "set the App ID, then import again",
        ));
    };
    let root = key_path::config_root()?;
    let dest = key_path::default_path(&root, &app_id);
    refuse_another_source(&root, &app_id, &dest)?;
    refuse_a_link(&root, &dest)?;

    let download = read_download(file, target, &dest)?;
    let dir = key_path::key_dir(&root);
    prepare_dir(&dir)?;
    if std::fs::symlink_metadata(&dest).is_ok() && !replace {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::DestructiveRefusal,
                format!("a bot private key already stands at {dest}"),
            )
            .expected("no key at the default path, or --replace to rotate it")
            .action("rerun with --replace to rotate the key; the download stays where it is"),
        ));
    }

    write_atomically(&dir, &dest, &app_id, &download.bytes)?;
    match secrets::look_up_key_file(target, Some(&app_id))? {
        KeyLookup::Found(key)
            if std::fs::canonicalize(&dest).is_ok_and(|held| held == key.path.as_std_path()) => {}
        _ => {
            return Err(refuse(
                format!("the key written to {dest} does not read back as the bot key"),
                "check the folder and run the import again; the download stays where it is",
            ));
        }
    }
    std::fs::remove_file(&download.path).map_err(|err| {
        refuse(
            format!(
                "the key is at {dest}, and the download {} cannot be deleted: {err}",
                download.path
            ),
            "delete the download by hand",
        )
    })?;
    out.result_line(format!("imported the bot private key to {dest}"));
    out.result_line(format!("deleted the download {}", download.path));
    out.next(&[
        format!("rk setup step bot-secrets --target {target} --apply"),
        format!("rk setup step install-bot --target {target} --apply"),
    ]);
    Ok(())
}

/// The download, read and judged.
struct Download {
    path: Utf8PathBuf,
    bytes: zeroize::Zeroizing<Vec<u8>>,
}

/// Read the download through every check of the lookup but the mode: a
/// fresh download is often group-readable, and the copy is owner-only.
fn read_download(file: &Utf8Path, target: &Utf8Path, dest: &Utf8Path) -> Result<Download, RkError> {
    let path = match secrets::resolve_path(&file.as_os_str().to_owned(), target)? {
        Resolved::Path(path) => path,
        Resolved::Missing(path) => {
            return Err(refuse(
                format!("{path} does not exist"),
                "name the downloaded .pem",
            ));
        }
        Resolved::Unreachable { path, reason } => {
            return Err(refuse(
                format!("{path} is unreadable: {reason}"),
                "name a readable .pem",
            ));
        }
    };
    if std::fs::canonicalize(dest).is_ok_and(|held| held == path.as_std_path()) {
        return Err(refuse(
            format!("{path} is already the default key path"),
            "name the downloaded .pem, not the imported key",
        ));
    }
    match secrets::read_key(path, Source::Default, false)? {
        KeyLookup::Found(key) => Ok(Download {
            path: key.path,
            bytes: key.bytes,
        }),
        KeyLookup::Unavailable(unavailable) => Err(unavailable.refusal()),
        KeyLookup::Absent(remedy) => Err(refuse(remedy, "name the downloaded .pem")),
    }
}

/// Refuse where the variable or the user configuration names another
/// path: the key the import writes would not be the key rk reads.
fn refuse_another_source(root: &Utf8PathBuf, app_id: &str, dest: &Utf8Path) -> Result<(), RkError> {
    if let Some(named) = secrets::value_of(PRIVATE_KEY_FILE)
        && Utf8Path::new(&named.to_string_lossy()) != dest
    {
        return Err(refuse(
            format!(
                "{PRIVATE_KEY_FILE} names {}, so rk reads that file and not {dest}",
                named.to_string_lossy()
            ),
            &format!("unset {PRIVATE_KEY_FILE}, then import again"),
        ));
    }
    if let Some(named) = key_path::user_config_entry(root, app_id)?
        && named != dest
    {
        return Err(refuse(
            format!(
                "{} names {named} for App ID {app_id}, so rk reads that file and not {dest}",
                key_path::user_config_path(root)
            ),
            "remove the entry for this App ID, then import again",
        ));
    }
    Ok(())
}

/// Refuse where the default path, or a folder on it below the
/// configuration root, is a symbolic link: the key is managed elsewhere,
/// and a file put in the link's place would be overwritten with the old
/// key when the place that manages it next writes the path.
fn refuse_a_link(root: &Utf8PathBuf, dest: &Utf8Path) -> Result<(), RkError> {
    let mut checked = Vec::new();
    let mut walk = root.join("release-kit");
    checked.push(walk.clone());
    for part in ["bot", "github"] {
        walk = walk.join(part);
        checked.push(walk.clone());
    }
    checked.push(dest.to_owned());
    for path in checked {
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            let linked = std::fs::read_link(&path).map_or_else(
                |_| "an unreadable target".to_owned(),
                |to| to.display().to_string(),
            );
            let at = if path == dest {
                format!("the bot private key at {path}")
            } else {
                format!("the bot private key folder {path}")
            };
            return Err(RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    format!("{at} is a symbolic link to {linked}, so it is managed elsewhere: rotate it where it is managed, then run `rk setup step bot-secrets --apply`"),
                )
                .expected("a key path that rk manages, with no symbolic link on it")
                .action("rotate the key where it is managed; the link and the download stay as they are"),
            ));
        }
    }
    Ok(())
}

/// Create the key folder owner-only where it is missing, and refuse one
/// that exists open beyond its owner.
fn prepare_dir(dir: &Utf8Path) -> Result<(), RkError> {
    if let Ok(meta) = std::fs::metadata(dir) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = meta.permissions().mode();
            if mode & 0o077 != 0 {
                return Err(refuse(
                    format!("{dir} is open to group or other ({:04o})", mode & 0o7777),
                    &format!("chmod 700 {dir}"),
                ));
            }
        }
        #[cfg(not(unix))]
        let _ = meta;
        return Ok(());
    }
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(dir).map_err(|err| {
        refuse(
            format!("{dir} cannot be created: {err}"),
            "check the configuration root",
        )
    })
}

/// Write the key to a temporary file with mode 0600 in the key folder,
/// judge it, and rename it onto the default path. A failure removes the
/// temporary file and leaves the default path as it stood.
fn write_atomically(
    dir: &Utf8Path,
    dest: &Utf8Path,
    app_id: &str,
    bytes: &[u8],
) -> Result<(), RkError> {
    let temp = dir.join(format!(".{app_id}.pem.{}.tmp", std::process::id()));
    let written = (|| -> Result<(), RkError> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut handle = options.open(&temp).map_err(|err| {
            refuse(
                format!("{temp} cannot be created: {err}"),
                "check the key folder",
            )
        })?;
        handle
            .write_all(bytes)
            .and_then(|()| handle.sync_all())
            .map_err(|err| {
                refuse(
                    format!("{temp} cannot be written: {err}"),
                    "check the key folder",
                )
            })?;
        match secrets::read_key(temp.clone(), Source::Default, true)? {
            KeyLookup::Found(_) => {}
            KeyLookup::Unavailable(unavailable) => return Err(unavailable.refusal()),
            KeyLookup::Absent(remedy) => return Err(refuse(remedy, "check the key folder")),
        }
        std::fs::rename(&temp, dest).map_err(|err| {
            refuse(
                format!("{temp} cannot be renamed onto {dest}: {err}"),
                "check the key folder",
            )
        })
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// One refusal, in this verb's shape.
fn refuse(message: String, action: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(Reason::PrerequisiteUnmet, message)
            .expected("a downloaded PEM private key outside the repository, and the App ID")
            .action(action.to_owned()),
    )
}
