//! The temp-plus-rename writer every landing write goes through, and
//! the staged transaction an apply commits several writes through.
//!
//! A plain `fs::write` interrupted mid-call leaves a truncated file that
//! is valid-looking YAML until a forge parses it. Writing beside the
//! destination and renaming over it makes each write land whole or not at
//! all; the rename stays in one directory, which is what keeps it atomic
//! on POSIX filesystems. A transaction stages every write first and
//! renames them in order afterwards, so an interruption leaves each
//! destination holding either its previous bytes or its new ones, and
//! the caller learns which renames landed.

use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

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

/// Several writes staged beside their destinations, renamed over them in
/// order on commit; a staged file that is never committed is removed
/// when the transaction drops.
#[derive(Debug, Default)]
pub struct Transaction {
    staged: Vec<(PathBuf, PathBuf)>,
}

/// A commit that stopped part way: what landed, and what did not.
#[derive(Debug)]
pub struct Interrupted {
    /// The destinations renamed over before the failure, in order.
    pub landed: Vec<PathBuf>,
    /// The destination whose rename failed.
    pub failed: PathBuf,
    /// The failure.
    pub error: std::io::Error,
}

impl Transaction {
    /// An empty transaction.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage `bytes` for `path`: written whole and synced beside the
    /// destination, which holds what it held until commit.
    ///
    /// # Errors
    ///
    /// Any I/O failure from creating or writing the staged file; the
    /// destination is untouched.
    pub fn stage(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
        if let Some(parent) = parent {
            fs::create_dir_all(parent)?;
        }
        let name = path
            .file_name()
            .ok_or_else(|| std::io::Error::other(format!("no file name in {}", path.display())))?;
        let mut tmp_name = std::ffi::OsString::from(format!(".{}", std::process::id()));
        tmp_name.push(format!(".rk-txn-{}.", self.staged.len()));
        tmp_name.push(name);
        let tmp = path.with_file_name(tmp_name);
        let written = fs::File::create(&tmp)
            .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()));
        if written.is_err() {
            let _ = fs::remove_file(&tmp);
            return written;
        }
        self.staged.push((tmp, path.to_path_buf()));
        Ok(())
    }

    /// How many writes are staged.
    #[must_use]
    pub fn len(&self) -> usize {
        self.staged.len()
    }

    /// Whether nothing is staged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.staged.is_empty()
    }

    /// Rename every staged file over its destination, in staging order.
    ///
    /// # Errors
    ///
    /// The first rename that fails stops the commit; the result names
    /// every destination renamed before it. The remaining staged files
    /// are removed, so no destination is ever half-written.
    pub fn commit(self) -> Result<Vec<PathBuf>, Interrupted> {
        self.commit_stopping_at(None)
    }

    /// [`Self::commit`], stopped on purpose before the rename over
    /// `stop`, as if that rename had failed. This is the one seam the
    /// interruption proof needs: a rename cannot be made to fail from
    /// outside without a read failing first, and the proof is about what
    /// the tree holds after a commit that stopped part way.
    ///
    /// # Errors
    ///
    /// As [`Self::commit`], plus the injected stop.
    pub fn commit_stopping_at(mut self, stop: Option<&Path>) -> Result<Vec<PathBuf>, Interrupted> {
        let mut landed = Vec::new();
        let staged = std::mem::take(&mut self.staged);
        let mut pending = staged.into_iter();
        for (tmp, dest) in pending.by_ref() {
            let renamed = if stop.is_some_and(|stop| dest.ends_with(stop)) {
                Err(std::io::Error::other(
                    "the commit was stopped here for the proof",
                ))
            } else {
                fs::rename(&tmp, &dest)
            };
            if let Err(error) = renamed {
                let _ = fs::remove_file(&tmp);
                for (rest, _) in pending {
                    let _ = fs::remove_file(rest);
                }
                return Err(Interrupted {
                    landed,
                    failed: dest,
                    error,
                });
            }
            landed.push(dest);
        }
        Ok(landed)
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        for (tmp, _) in self.staged.drain(..) {
            let _ = fs::remove_file(tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Transaction, write};

    #[test]
    fn a_transaction_lands_every_write_in_order_and_leaves_no_temp() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let one = dir.path().join("a/one.txt");
        let two = dir.path().join("two.txt");
        let mut txn = Transaction::new();
        txn.stage(&one, b"one").expect("stages");
        txn.stage(&two, b"two").expect("stages");
        assert!(
            !one.exists() && !two.exists(),
            "staging touches no destination"
        );
        let landed = txn.commit().expect("commits");
        assert_eq!(landed, vec![one.clone(), two.clone()]);
        assert_eq!(std::fs::read(&one).expect("reads"), b"one");
        assert_eq!(std::fs::read(&two).expect("reads"), b"two");
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("the dir reads")
            .map(|entry| entry.expect("an entry").file_name())
            .filter(|name| name != "a" && name != "two.txt")
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn a_dropped_transaction_removes_what_it_staged() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let dest = dir.path().join("kept.txt");
        std::fs::write(&dest, b"before").expect("writes");
        {
            let mut txn = Transaction::new();
            txn.stage(&dest, b"after").expect("stages");
        }
        assert_eq!(std::fs::read(&dest).expect("reads"), b"before");
        assert_eq!(std::fs::read_dir(dir.path()).expect("reads").count(), 1);
    }

    #[test]
    fn an_interrupted_commit_names_what_landed_and_leaves_the_rest_whole() {
        let dir = tempfile::tempdir().expect("a scratch dir exists");
        let first = dir.path().join("first.txt");
        let blocked = dir.path().join("blocked");
        std::fs::create_dir(&blocked).expect("the blocking dir creates");
        let third = dir.path().join("third.txt");
        std::fs::write(&third, b"before").expect("writes");
        let mut txn = Transaction::new();
        txn.stage(&first, b"first").expect("stages");
        txn.stage(&blocked, b"bytes")
            .expect("stages beside a directory");
        txn.stage(&third, b"after").expect("stages");
        let interrupted = txn.commit().expect_err("the directory blocks the rename");
        assert_eq!(interrupted.landed, vec![first.clone()]);
        assert_eq!(interrupted.failed, blocked);
        assert_eq!(std::fs::read(&first).expect("reads"), b"first");
        assert_eq!(std::fs::read(&third).expect("reads"), b"before");
        assert_eq!(std::fs::read_dir(dir.path()).expect("reads").count(), 3);
    }

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
