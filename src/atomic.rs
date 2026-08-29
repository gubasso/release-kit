//! The temp-plus-rename writer every landing write goes through.
//!
//! A plain `fs::write` interrupted mid-call leaves a truncated file that
//! is valid-looking YAML until a forge parses it. Writing beside the
//! destination and renaming over it makes each write land whole or not at
//! all; the rename stays in one directory, which is what keeps it atomic
//! on POSIX filesystems.

use std::fs;
use std::io::Write as _;
use std::path::Path;

/// Write `bytes` at `path` through a same-directory temporary file and a
/// rename, creating the parent directories it needs.
///
/// # Errors
///
/// Any I/O failure from creating, writing, or renaming; on failure the
/// temporary file is removed and the destination holds what it held.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other(format!("no file name in {}", path.display())))?;
    let mut tmp_name = std::ffi::OsString::from(format!(".{}", std::process::id()));
    tmp_name.push(".rk-tmp.");
    tmp_name.push(name);
    let tmp = path.with_file_name(tmp_name);
    let written = fs::File::create(&tmp)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| fs::rename(&tmp, path));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::write;

    #[test]
    fn a_write_creates_parents_lands_whole_and_leaves_no_temp() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let path = dir.path().join("deep/nested/file.txt");
        write(&path, b"first").expect("the write lands");
        assert_eq!(std::fs::read(&path).expect("the file reads"), b"first");
        write(&path, b"second").expect("the overwrite lands");
        assert_eq!(std::fs::read(&path).expect("the file reads"), b"second");
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().expect("a parent"))
            .expect("the dir reads")
            .map(|entry| entry.expect("an entry").file_name())
            .filter(|name| name != "file.txt")
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn a_failed_write_leaves_the_destination_alone() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        // A directory where the file should land: the rename fails.
        let path = dir.path().join("blocked");
        std::fs::create_dir(&path).expect("the blocking dir creates");
        assert!(write(&path, b"bytes").is_err());
        assert!(path.is_dir(), "the destination must be untouched");
    }
}
