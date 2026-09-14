//! What the stage verbs share for acting on an entry they hold rather than
//! on a path they were told: a directory's identity, the kernel's link to
//! an open directory, an unpredictable nonce for a quarantine name, and
//! the proof pause.
//!
//! The crate forbids `unsafe`, so no `openat`, `unlinkat`, or `renameat`
//! reaches the kernel through FFI. The equivalent goes through
//! `/proc/self/fd/<fd>/<name>`, which resolves against the directory the
//! descriptor holds, whatever the path that led to it has since become.

use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};

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
