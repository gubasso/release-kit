//! What the stage verbs and the landing writer share for acting on an
//! entry they hold rather than on a path they were told.
//!
//! A directory's identity, the kernel's link to an open directory, a hold
//! on a nested directory that follows no link on the way, a write that
//! lands inside a held directory, an unpredictable nonce for a quarantine
//! name, and the proof pause.
//!
//! The crate forbids `unsafe`, so no `openat`, `unlinkat`, or `renameat`
//! reaches the kernel through FFI. The equivalent goes through
//! `/proc/self/fd/<fd>/<name>`, which resolves against the directory the
//! descriptor holds, whatever the path that led to it has since become.

use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::Write as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Component, Path, PathBuf};

/// One entry's identity: the device and inode a check saw, which the
/// entry acted on must still carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// The device.
    pub dev: u64,
    /// The inode.
    pub ino: u64,
}

impl Identity {
    /// The identity a metadata record carries.
    pub fn of(metadata: &fs::Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
}

/// The kernel's link to an open directory.
pub fn proc_path(dir: &File) -> PathBuf {
    use std::os::fd::AsRawFd as _;
    PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()))
}

/// Open a directory without following a final symlink.
pub fn open_dir(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

/// Hold the directory `relative` names below the held `root`, creating
/// each absent component on the way and following no link at any of
/// them.
///
/// Each component opens through the kernel's link to the directory
/// already held, so a component swapped for a symlink after the caller's
/// validation refuses instead of redirecting what follows outside the
/// root. An empty `relative` is the root itself, reopened.
///
/// # Errors
///
/// A component that is a link or another non-directory entry, a
/// component that cannot be created, and any other open failure.
pub fn hold_dir(root: &File, relative: &Path) -> std::io::Result<File> {
    hold_dir_with(root, relative, true)
}

/// [`hold_dir`] creating nothing: an absent component is the caller's
/// `NotFound`.
///
/// # Errors
///
/// As [`hold_dir`], plus `NotFound` for an absent component.
pub fn hold_dir_existing(root: &File, relative: &Path) -> std::io::Result<File> {
    hold_dir_with(root, relative, false)
}

fn hold_dir_with(root: &File, relative: &Path, create: bool) -> std::io::Result<File> {
    // The root is duplicated rather than reopened: the kernel's link to
    // it is a link, and this module opens a link at no final component.
    let mut held = root.try_clone()?;
    for component in relative.components() {
        let name = match component {
            Component::Normal(name) => name,
            Component::CurDir => continue,
            other => {
                return Err(std::io::Error::other(format!(
                    "{} is not a plain component below the target",
                    other.as_os_str().to_string_lossy()
                )));
            }
        };
        let path = proc_path(&held).join(name);
        let next = match open_dir(&path) {
            Ok(dir) => dir,
            Err(error) if create && error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&path)?;
                open_dir(&path)?
            }
            // A link under the name fails the directory open, as `ELOOP`
            // or as `ENOTDIR` depending on the kernel; the entry itself
            // says which, and a link is named as one.
            Err(error) => {
                if fs::symlink_metadata(&path).is_ok_and(|entry| entry.file_type().is_symlink()) {
                    return Err(std::io::Error::other(format!(
                        "{} is a link, and a landing follows none",
                        name.to_string_lossy()
                    )));
                }
                return Err(error);
            }
        };
        held = next;
    }
    Ok(held)
}

/// Write `bytes` as `name` inside the held directory `dir`, through a
/// temporary sibling and a rename over the destination, following no
/// link at either name.
///
/// The temporary file is created exclusively, so nothing that already
/// stands under its name is truncated; on any failure it is removed and
/// the destination holds what it held.
///
/// # Errors
///
/// Any failure from creating, writing, syncing, or renaming.
pub fn write_file(dir: &File, name: &OsStr, bytes: &[u8]) -> std::io::Result<()> {
    let base = proc_path(dir);
    let mut temp = std::ffi::OsString::from(format!(".{}.rk-tmp.", std::process::id()));
    temp.push(name);
    let temp_path = base.join(&temp);
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&temp_path)
        .and_then(|mut file| file.write_all(bytes).and_then(|()| file.sync_all()))
        .and_then(|()| fs::rename(&temp_path, base.join(name)));
    if written.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    written
}

/// The bytes at `name` inside the held directory `dir`, or `None` where
/// nothing stands there; a link under the name is refused rather than
/// followed.
///
/// # Errors
///
/// Any read failure other than absence.
pub fn read_file(dir: &File, name: &OsStr) -> std::io::Result<Option<Vec<u8>>> {
    use std::io::Read as _;
    let path = proc_path(dir).join(name);
    let opened = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&path);
    match opened {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(Some(bytes))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// A nonce nobody else can predict, for a quarantine name: from the
/// kernel's random source, or, where that cannot be read, from the clock,
/// the process, and a counter.
pub fn nonce() -> u64 {
    use std::io::Read as _;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut bytes = [0u8; 8];
    if File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .is_ok()
    {
        return u64::from_ne_bytes(bytes);
    }
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    (since.as_secs() << 30)
        ^ u64::from(since.subsec_nanos())
        ^ (u64::from(std::process::id()) << 48)
        ^ COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A quarantine name under `prefix`: the entry is renamed here, inside a
/// directory the run holds open, before its identity is judged and it is
/// removed, so nothing can be swapped under it between the two.
pub fn quarantine_name(prefix: &str) -> std::ffi::OsString {
    std::ffi::OsString::from(format!("{prefix}{:016x}", nonce()))
}

/// Move the entry `name` inside the held directory `dir` to a fresh
/// quarantine name, and return that name with what stands there.
pub fn quarantine(
    dir: &File,
    name: &std::ffi::OsStr,
    prefix: &str,
) -> std::io::Result<(std::ffi::OsString, fs::Metadata)> {
    let base = proc_path(dir);
    let quarantined = quarantine_name(prefix);
    fs::rename(base.join(name), base.join(&quarantined))?;
    let metadata = fs::symlink_metadata(base.join(&quarantined))?;
    Ok((quarantined, metadata))
}

/// The proof's pause: where `var` names a directory, announce `tag`
/// there and wait for `proceed` under it. Bounded, so a forgotten
/// variable cannot hang a run forever.
pub fn pause(var: &str, tag: &str, proceed: &str) {
    let Some(dir) = std::env::var_os(var).filter(|value| !value.is_empty()) else {
        return;
    };
    let dir = PathBuf::from(dir);
    let _ = fs::write(dir.join(tag), b"");
    let proceed = dir.join(proceed);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !proceed.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::{hold_dir, open_dir, read_file, write_file};

    /// A nested hold creates what is absent, reopens what exists, and
    /// refuses a component that became a link, writing nothing through it.
    #[test]
    fn a_hold_creates_and_refuses_a_linked_component() {
        let root = tempfile::tempdir().expect("a scratch root exists");
        let outside = tempfile::tempdir().expect("a scratch outside exists");
        let held = open_dir(root.path()).expect("the root opens");
        let leaf = hold_dir(&held, std::path::Path::new("a/b")).expect("the nested hold creates");
        write_file(&leaf, std::ffi::OsStr::new("file.txt"), b"one").expect("the write lands");
        assert_eq!(
            std::fs::read(root.path().join("a/b/file.txt")).expect("reads"),
            b"one"
        );
        assert_eq!(
            read_file(&leaf, std::ffi::OsStr::new("file.txt")).expect("reads"),
            Some(b"one".to_vec())
        );
        assert_eq!(
            read_file(&leaf, std::ffi::OsStr::new("absent")).expect("reads"),
            None
        );

        std::fs::remove_dir_all(root.path().join("a")).expect("the tree removes");
        std::os::unix::fs::symlink(outside.path(), root.path().join("a")).expect("the link");
        let refused = hold_dir(&held, std::path::Path::new("a/b")).expect_err("a link refuses");
        assert!(refused.to_string().contains("link"), "{refused}");
        assert_eq!(
            std::fs::read_dir(outside.path()).expect("reads").count(),
            0,
            "nothing lands through the link"
        );
    }
}
