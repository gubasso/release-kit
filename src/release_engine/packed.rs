//! The files a release packs, and whether they changed since the last
//! release.
//!
//! The request opens only where a packed file changed or the packed set
//! did, the rule release-plz applies to a crate. The list is the union of
//! what the release packs at `HEAD` and at the last release tag, each read
//! through the driver adapter and each without the destinations its own
//! revision's landing record names. A bash release packs the tracked files its declaration
//! selects, with Cargo's `include` and `exclude` semantics, and this
//! module packs them into a reproducible archive from the git objects. A
//! python sdist builds in a temporary worktree outside the checkout, which
//! is removed afterwards, also on failure.
//!
//! SATISFIES release-engine:a-request-follows-the-packed-files
//! SATISFIES release-engine:the-bash-release-packs-its-declaration

use std::collections::BTreeSet;
use std::io::{BufRead as _, Read as _, Write as _};
use std::sync::atomic::{AtomicU64, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

use super::Context;

/// The key that names the tracked files a bash release ships.
pub const INCLUDE_KEY: &str = "profile.release.build.include";

/// The key that names the tracked files a bash release leaves out.
pub const EXCLUDE_KEY: &str = "profile.release.build.exclude";

/// The key that names the command run in the unpacked bash archive.
pub const VERIFY_KEY: &str = "profile.release.build.verify";

/// The bash version file, which every bash release ships, as Cargo always
/// packs `Cargo.toml`.
pub const VERSION_FILE: &str = "VERSION";

/// Why a bash request opens nothing where the declaration selects no file
/// yet.
pub const NOTHING_YET: &str =
    "no packed file exists yet: profile.release.build.include matches no tracked file";

/// The directory every landing record and configuration lives in.
const RECORD_DIR: &str = ".release-kit/";

/// The files a python sdist generates rather than copies from the tree.
const GENERATED: [&str; 2] = ["PKG-INFO", "pyproject.toml.orig"];

/// What the request compared: the last release tag, the packed list, and
/// whether the packed set or a packed file differs between the tag and
/// `HEAD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    /// The last release tag.
    pub tag: String,
    /// The files the release packs at `HEAD` or at the tag, sorted.
    pub paths: Vec<String>,
    /// Whether any of them differs between the tag and `HEAD`, or the
    /// two revisions pack different sets.
    pub changed: bool,
    /// The paths git-cliff reads the history of: the packed files, the
    /// driver's version file, and the configuration where a change of its
    /// declaration changed the packed set, so the commit that changed it
    /// counts.
    pub history: Vec<String>,
    /// The paths whose commits the releasing-commit condition reads: the
    /// packed files, and the configuration where a change of its
    /// declaration changed the packed set. The version file stays out,
    /// because the release commit alone changes it.
    pub inputs: Vec<String>,
}

/// What a bash release ships, as Cargo's `include` and `exclude` declare
/// a crate's package: gitignore-style patterns, where `include` overrides
/// `exclude`, and neither key ships every tracked file.
pub struct Declaration {
    include: Option<Gitignore>,
    exclude: Gitignore,
}

impl Declaration {
    /// One pattern list as the gitignore matcher Cargo builds for it.
    ///
    /// # Errors
    ///
    /// The refusal text, naming the key and the pattern the matcher
    /// refuses.
    pub fn parse(key: &str, patterns: &[String]) -> Result<Gitignore, String> {
        let mut builder = GitignoreBuilder::new("");
        for pattern in patterns {
            builder.add_line(None, pattern).map_err(|error| {
                format!("{key} holds {pattern:?}, which is no gitignore-style pattern: {error}")
            })?;
        }
        builder
            .build()
            .map_err(|error| format!("{key} does not build a matcher: {error}"))
    }

    /// The declaration of two pattern lists. An empty `include` is no
    /// `include`, as in Cargo.
    ///
    /// # Errors
    ///
    /// The refusal text of a pattern the matcher refuses.
    pub fn new(include: &[String], exclude: &[String]) -> Result<Self, String> {
        Ok(Self {
            include: if include.is_empty() {
                None
            } else {
                Some(Self::parse(INCLUDE_KEY, include)?)
            },
            exclude: Self::parse(EXCLUDE_KEY, exclude)?,
        })
    }

    /// The declaration a revision's `.release-kit/config.toml` records:
    /// neither key where the revision holds no configuration or the table
    /// names no pattern.
    ///
    /// # Errors
    ///
    /// A refusal where a recorded pattern does not parse.
    pub fn at(root: &Utf8Path, rev: &str) -> Result<Self, RkError> {
        let (include, exclude) = recorded(root, rev);
        Self::new(&include, &exclude).map_err(|message| {
            RkError::refusal(
                Diagnostic::new(Reason::ConfigInvalid, format!("at {rev}: {message}"))
                    .expected("gitignore-style patterns, as Cargo's include and exclude take")
                    .target_state("unchanged"),
            )
        })
    }

    /// Whether an `include` pattern names the file.
    fn names(&self, path: &str) -> bool {
        self.include
            .as_ref()
            .is_some_and(|include| include.matched_path_or_any_parents(path, false).is_ignore())
    }

    /// Whether the declaration ships the tracked file `path`, as
    /// `cargo package` decides: through `include` where it is set, and
    /// otherwise through `exclude`, each pattern judged on the file and
    /// on every directory above it.
    #[must_use]
    pub fn ships(&self, path: &str) -> bool {
        if self.include.is_some() {
            return self.names(path);
        }
        !self
            .exclude
            .matched_path_or_any_parents(path, false)
            .is_ignore()
    }
}

/// The `include` and `exclude` patterns a revision's
/// `.release-kit/config.toml` records: none where the revision holds no
/// configuration or the table names no pattern.
fn recorded(root: &Utf8Path, rev: &str) -> (Vec<String>, Vec<String>) {
    let spec = format!("{rev}:{RECORD_DIR}config.toml");
    let text = super::git(root, &["show", &spec])
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default();
    let table: toml::Table = toml::from_str(&text).unwrap_or_default();
    let build = table
        .get("profile")
        .and_then(|profile| profile.get("release"))
        .and_then(|release| release.get("build"));
    let list = |key: &str| -> Vec<String> {
        build
            .and_then(|build| build.get(key))
            .and_then(toml::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    (list("include"), list("exclude"))
}

/// The files a bash release ships from the tracked files of a revision.
///
/// They are those the declaration selects, and `VERSION` always. A
/// destination rk's record names, and `.release-kit/`, ship only where an
/// `include` pattern names them.
#[must_use]
pub fn declared(
    tracked: &BTreeSet<String>,
    declaration: &Declaration,
    destinations: &BTreeSet<String>,
) -> BTreeSet<String> {
    tracked
        .iter()
        .filter(|path| {
            if path.as_str() == VERSION_FILE {
                return true;
            }
            let rk_owned = path.starts_with(RECORD_DIR) || destinations.contains(*path);
            if rk_owned {
                declaration.names(path)
            } else {
                declaration.ships(path)
            }
        })
        .cloned()
        .collect()
}

/// Whether a bash package holds nothing past the version file it always
/// ships.
#[must_use]
pub fn is_empty_package(files: &BTreeSet<String>) -> bool {
    files.iter().all(|path| path == VERSION_FILE)
}

/// The refusal of a bash release whose declaration ships no file yet.
#[must_use]
pub fn empty_package(rev: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!(
                "the package at {rev} holds no file past {VERSION_FILE}: {INCLUDE_KEY} and {EXCLUDE_KEY} select no tracked file"
            ),
        )
        .expected("a declaration that selects the tracked files the release ships")
        .action(format!(
            "widen {INCLUDE_KEY} or {EXCLUDE_KEY} in .release-kit/config.toml, run rk upgrade --apply, and read rk release build --list"
        ))
        .target_state("no artifact, upload, tag, or release"),
    )
}

/// The members of an archive below the one top directory every member
/// shares, where they share one; otherwise the members unchanged.
#[must_use]
pub fn strip_top(members: &[String]) -> Vec<String> {
    let clean: Vec<&str> = members
        .iter()
        .map(|member| member.trim_start_matches("./"))
        .filter(|member| !member.is_empty())
        .collect();
    let top = clean.first().and_then(|member| member.split_once('/'));
    let shared = top.is_some_and(|(top, _)| {
        clean.iter().all(|member| {
            member
                .split_once('/')
                .is_some_and(|(first, _)| first == top)
        })
    });
    clean
        .iter()
        .map(|member| {
            if shared {
                member.split_once('/').map_or(*member, |(_, rest)| rest)
            } else {
                member
            }
        })
        .filter(|member| !member.is_empty() && !member.ends_with('/'))
        .map(str::to_owned)
        .collect()
}

/// The members without the files a python sdist generates at its root.
#[must_use]
pub fn drop_generated(members: Vec<String>) -> Vec<String> {
    members
        .into_iter()
        .filter(|member| !GENERATED.contains(&member.as_str()))
        .collect()
}

/// The members that are tracked files of the revision.
#[must_use]
pub fn keep_tracked(members: Vec<String>, tracked: &BTreeSet<String>) -> BTreeSet<String> {
    members
        .into_iter()
        .filter(|member| tracked.contains(member))
        .collect()
}

/// The list without the files rk owns at that revision: every destination
/// its landing record names, and the record's own directory.
#[must_use]
pub fn subtract(list: BTreeSet<String>, destinations: &BTreeSet<String>) -> BTreeSet<String> {
    list.into_iter()
        .filter(|path| !path.starts_with(RECORD_DIR) && !destinations.contains(path))
        .collect()
}

/// A path as a git-cliff `--include-path` pattern that matches it alone:
/// each glob metacharacter stands inside brackets.
#[must_use]
pub fn cliff_pattern(path: &str) -> String {
    let mut pattern = String::with_capacity(path.len());
    for c in path.chars() {
        if matches!(c, '*' | '?' | '[' | ']') {
            pattern.push('[');
            pattern.push(c);
            pattern.push(']');
        } else {
            pattern.push(c);
        }
    }
    pattern
}

/// The file members of a gzip tarball, as the archive names them.
///
/// # Errors
///
/// A read of an archive that is no gzip tarball.
pub fn members(archive: &Utf8Path) -> Result<Vec<String>, RkError> {
    let unreadable = |source: std::io::Error| {
        RkError::check_failed(
            Diagnostic::new(
                Reason::PostconditionFailed,
                format!("{archive} is not a gzip tarball rk can read: {source}"),
            )
            .expected("a .tar.gz the build wrote")
            .target_state("unchanged"),
        )
    };
    let file = std::fs::File::open(archive).map_err(RkError::Io)?;
    let mut tarball = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut found = Vec::new();
    for entry in tarball.entries().map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_symlink() || kind.is_hard_link()) {
            continue;
        }
        let path = entry.path().map_err(unreadable)?;
        found.push(path.to_string_lossy().into_owned());
    }
    Ok(found)
}

/// One tracked file of a revision's tree: its git mode, its blob, and
/// its path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The git mode: `100644`, `100755`, or `120000` for a symlink.
    pub mode: String,
    /// The blob's object name.
    pub object: String,
    /// The path, relative to the root.
    pub path: String,
}

/// The tracked files of a revision's tree, sorted by path. A submodule is
/// a commit and no file of this tree, so it is left out.
///
/// # Errors
///
/// git's failure.
pub fn entries(root: &Utf8Path, rev: &str) -> Result<Vec<Entry>, RkError> {
    let listed = super::git_ok(
        root,
        &["ls-tree", "-r", "-z", "--full-tree", rev],
        "the revision's tree",
    )?;
    let mut found: Vec<Entry> = listed
        .split('\0')
        .filter_map(|line| {
            let (meta, path) = line.split_once('\t')?;
            let mut parts = meta.split(' ');
            let (mode, kind, object) = (parts.next()?, parts.next()?, parts.next()?);
            (kind == "blob").then(|| Entry {
                mode: mode.to_owned(),
                object: object.to_owned(),
                path: path.to_owned(),
            })
        })
        .collect();
    found.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(found)
}

/// The tracked files of a revision.
///
/// # Errors
///
/// git's failure.
pub fn tracked(root: &Utf8Path, rev: &str) -> Result<BTreeSet<String>, RkError> {
    Ok(entries(root, rev)?
        .into_iter()
        .map(|entry| entry.path)
        .collect())
}

/// The files a bash release ships at a revision: the tracked files the
/// declaration of that revision selects, with `VERSION`.
///
/// # Errors
///
/// git's failure, or a recorded pattern that does not parse.
pub fn bash_set(root: &Utf8Path, rev: &str) -> Result<BTreeSet<String>, RkError> {
    Ok(declared(
        &tracked(root, rev)?,
        &Declaration::at(root, rev)?,
        &destinations(root, rev),
    ))
}

/// The bytes of every blob, in order, through one `git cat-file --batch`.
fn blobs(root: &Utf8Path, objects: &[&str]) -> Result<Vec<Vec<u8>>, RkError> {
    let failed = |message: String| {
        RkError::subprocess(
            Diagnostic::new(Reason::SubprocessFailed, message)
                .expected("git cat-file to read every packed blob"),
        )
    };
    let mut command = std::process::Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let mut child = command
        .arg("-C")
        .arg(root.as_std_path())
        .args(["cat-file", "--batch"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|source| failed(format!("git did not run: {source}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| failed("git cat-file took no input".to_owned()))?;
    let mut input = String::new();
    for object in objects {
        input.push_str(object);
        input.push('\n');
    }
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let mut reader = std::io::BufReader::new(
        child
            .stdout
            .take()
            .ok_or_else(|| failed("git cat-file gave no output".to_owned()))?,
    );
    let mut out = Vec::with_capacity(objects.len());
    for object in objects {
        let mut header = String::new();
        reader.read_line(&mut header).map_err(RkError::Io)?;
        let size = header
            .split(' ')
            .nth(2)
            .and_then(|size| size.trim().parse::<usize>().ok())
            .ok_or_else(|| {
                failed(format!(
                    "git cat-file did not read {object}: {}",
                    header.trim()
                ))
            })?;
        let mut bytes = vec![0; size];
        reader.read_exact(&mut bytes).map_err(RkError::Io)?;
        let mut newline = [0; 1];
        reader.read_exact(&mut newline).map_err(RkError::Io)?;
        out.push(bytes);
    }
    writer
        .join()
        .map_err(|_| failed("git cat-file's input stopped".to_owned()))?
        .map_err(RkError::Io)?;
    child.wait().map_err(RkError::Io)?;
    Ok(out)
}

/// Pack `files` of the revision `rev` into the gzip tarball `archive`,
/// under the one top directory `top`, from the git objects alone.
///
/// The archive is reproducible: members sorted by path, no directory
/// entry, each mode from its git mode, owner and group 0 with empty
/// names, every time `epoch`, and a gzip header with no name and a zero
/// time, so a build of one commit writes the same bytes on every host.
///
/// # Errors
///
/// git's failure, or a write that fails.
pub fn pack(
    root: &Utf8Path,
    rev: &str,
    files: &BTreeSet<String>,
    top: &str,
    epoch: u64,
    archive: &Utf8Path,
) -> Result<(), RkError> {
    let shipped: Vec<Entry> = entries(root, rev)?
        .into_iter()
        .filter(|entry| files.contains(&entry.path))
        .collect();
    let objects: Vec<&str> = shipped.iter().map(|entry| entry.object.as_str()).collect();
    let contents = blobs(root, &objects)?;
    let file = std::fs::File::create(archive).map_err(RkError::Io)?;
    let gzip = flate2::GzBuilder::new()
        .mtime(0)
        .write(file, flate2::Compression::default());
    let mut tarball = tar::Builder::new(gzip);
    for (entry, bytes) in shipped.iter().zip(contents) {
        let path = format!("{top}/{}", entry.path);
        let mut header = tar::Header::new_gnu();
        header.set_uid(0);
        header.set_gid(0);
        header.set_username("").map_err(RkError::Io)?;
        header.set_groupname("").map_err(RkError::Io)?;
        header.set_mtime(epoch);
        if entry.mode == "120000" {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_mode(0o777);
            header.set_size(0);
            let target = String::from_utf8_lossy(&bytes).into_owned();
            tarball
                .append_link(&mut header, &path, &target)
                .map_err(RkError::Io)?;
        } else {
            header.set_entry_type(tar::EntryType::Regular);
            header.set_mode(if entry.mode == "100755" { 0o755 } else { 0o644 });
            header.set_size(bytes.len() as u64);
            tarball
                .append_data(&mut header, &path, bytes.as_slice())
                .map_err(RkError::Io)?;
        }
    }
    tarball
        .into_inner()
        .map_err(RkError::Io)?
        .finish()
        .map_err(RkError::Io)?;
    Ok(())
}

/// Unpack the gzip tarball `archive` into `dir`.
///
/// # Errors
///
/// A read or a write that fails.
pub fn unpack(archive: &Utf8Path, dir: &Utf8Path) -> Result<(), RkError> {
    let file = std::fs::File::open(archive).map_err(RkError::Io)?;
    tar::Archive::new(flate2::read::GzDecoder::new(file))
        .unpack(dir)
        .map_err(RkError::Io)
}

/// The destinations the landing record names at a revision: none where
/// the revision holds no record.
#[must_use]
pub fn destinations(root: &Utf8Path, rev: &str) -> BTreeSet<String> {
    let spec = format!("{rev}:{RECORD_DIR}manifest.json");
    let Ok(output) = super::git(root, &["show", &spec]) else {
        return BTreeSet::new();
    };
    if !output.status.success() {
        return BTreeSet::new();
    }
    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .ok()
        .and_then(|record| record.get("files").cloned())
        .and_then(|files| files.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|file| file.get("destination")?.as_str().map(str::to_owned))
        .collect()
}

/// The commit time of a revision, for `SOURCE_DATE_EPOCH`.
///
/// # Errors
///
/// git's failure.
pub fn epoch(root: &Utf8Path, rev: &str) -> Result<String, RkError> {
    super::git_line(
        root,
        &["log", "-1", "--format=%ct", rev],
        "the revision's commit time",
    )
}

/// The refusal where a python sdist holds no file git tracks.
#[must_use]
pub fn mirrors_nothing(what: &str, rev: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!(
                "{what} at {rev} holds no file git tracks there, so the request cannot tell which files the release packs"
            ),
        )
        .expected("a package whose members, below one top directory, are tracked files")
        .action("configure the build backend to pack the project's tracked files, then read the sdist with tar -tzf")
        .target_state("unchanged"),
    )
}

/// A fresh directory under the system's temporary directory, removed
/// when it drops, also on failure.
pub struct TempDir {
    dir: Utf8PathBuf,
}

impl TempDir {
    /// Create one, named for `purpose`.
    ///
    /// # Errors
    ///
    /// A temporary directory that is not UTF-8, or that cannot be created.
    pub fn new(purpose: &str) -> Result<Self, RkError> {
        static COUNT: AtomicU64 = AtomicU64::new(0);
        let base = Utf8PathBuf::from_path_buf(std::env::temp_dir()).map_err(|path| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::PrerequisiteUnmet,
                    format!("the temporary directory {} is not UTF-8", path.display()),
                )
                .expected("a UTF-8 temporary directory")
                .target_state("unchanged"),
            )
        })?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        let dir = base.join(format!(
            "rk-{purpose}-{}-{nanos}-{}",
            std::process::id(),
            COUNT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).map_err(RkError::Io)?;
        Ok(Self { dir })
    }

    /// The directory.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.dir
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A temporary worktree of one revision, outside the checkout, removed
/// when it drops, also on failure.
pub struct Scratch {
    root: Utf8PathBuf,
    dir: TempDir,
}

impl Scratch {
    /// Check out `rev` into a fresh directory under the system's temporary
    /// directory, with no hook run.
    ///
    /// # Errors
    ///
    /// A directory that cannot be created, or git's failure.
    pub fn new(root: &Utf8Path, rev: &str) -> Result<Self, RkError> {
        let scratch = Self {
            root: root.to_owned(),
            dir: TempDir::new("packed")?,
        };
        let tree = scratch.tree();
        super::git_ok(
            root,
            &[
                "-c",
                "core.hooksPath=/dev/null",
                "worktree",
                "add",
                "--detach",
                "-q",
                tree.as_str(),
                rev,
            ],
            "a temporary worktree of the revision",
        )?;
        Ok(scratch)
    }

    /// The worktree.
    #[must_use]
    pub fn tree(&self) -> Utf8PathBuf {
        self.dir.path().join("tree")
    }

    /// A directory beside the worktree, outside it, for build output.
    #[must_use]
    pub fn out(&self) -> Utf8PathBuf {
        self.dir.path().join("out")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let tree = self.tree();
        let _ = super::git(
            &self.root,
            &["worktree", "remove", "--force", tree.as_str()],
        );
        let _ = std::fs::remove_dir_all(&tree);
        let _ = super::git(&self.root, &["worktree", "prune"]);
    }
}

/// The packed files of one revision, without rk's own.
fn side(context: &Context, rev: &str) -> Result<BTreeSet<String>, RkError> {
    let listed = context.adapter.packed(&context.target, rev)?;
    Ok(subtract(listed, &destinations(&context.target, rev)))
}

/// Compare the packed files of `HEAD` with those of the release `tag`.
///
/// # Errors
///
/// A refusal where the tag is not in the checkout or the release packs no
/// file, and a build or git failure.
pub fn compare(context: &Context, tag: &str) -> Result<Comparison, RkError> {
    let root = &context.target;
    let at = format!("refs/tags/{tag}^{{commit}}");
    let held = super::git(root, &["rev-parse", "--verify", "-q", &at])?;
    if !held.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::ObservationIncomplete,
                format!("the release tag {tag} is not in the checkout"),
            )
            .expected("a checkout that holds every release tag, as fetch-depth 0 fetches them")
            .target_state("unchanged"),
        ));
    }
    let at_head = side(context, "HEAD")?;
    let released = side(context, tag)?;
    // The set counts as a set, as Cargo counts the include and exclude
    // fields of Cargo.toml, which every crate packs: a file that joins or
    // leaves the package changes it, unchanged as its bytes may be.
    let moved = at_head != released;
    let declared = moved && recorded(root, "HEAD") != recorded(root, tag);
    let mut list = at_head;
    list.extend(released);
    // A declaration that selects nothing yet is valid: the request names
    // it and opens nothing, and git diff with no path would read the
    // whole tree.
    if list.is_empty() {
        return Ok(Comparison {
            tag: tag.to_owned(),
            paths: Vec::new(),
            changed: false,
            history: Vec::new(),
            inputs: Vec::new(),
        });
    }
    let paths: Vec<String> = list.into_iter().collect();
    let changed = moved || differs(root, tag, &paths)?;
    let mut history = paths.clone();
    let mut inputs = paths.clone();
    // git-cliff 2.14.2 filters the commits by path before it reads the
    // release tags, so a tag on a commit outside the paths no longer bounds
    // the history, and the bump answers the released version. Every release
    // commit changes the version file, so that file keeps the last release
    // in view; cliff.toml skips the release commit itself.
    if let Some((file, _)) = super::adapter::version_file(context.adapter.driver())
        && !history.iter().any(|path| path == file)
    {
        history.push(file.to_owned());
    }
    if declared {
        history.push(format!("{RECORD_DIR}config.toml"));
        inputs.push(format!("{RECORD_DIR}config.toml"));
    }
    Ok(Comparison {
        tag: tag.to_owned(),
        paths,
        changed,
        history,
        inputs,
    })
}

/// Whether any of `paths` differs between `tag` and `HEAD`, as
/// `git diff --quiet` answers.
///
/// # Errors
///
/// git's failure to compare.
pub fn differs(root: &Utf8Path, tag: &str, paths: &[String]) -> Result<bool, RkError> {
    let mut args = vec![
        "--literal-pathspecs",
        "diff",
        "--quiet",
        "--no-ext-diff",
        "--no-textconv",
        tag,
        "HEAD",
        "--",
    ];
    args.extend(paths.iter().map(String::as_str));
    let output = super::git(root, &args)?;
    match output.status.code() {
        Some(0) => Ok(false),
        Some(1) => Ok(true),
        _ => Err(RkError::subprocess(
            Diagnostic::new(Reason::SubprocessFailed, super::last_line(&output.stderr))
                .expected("git to compare the packed files"),
        )),
    }
}

/// The files the release packs at `HEAD`, without rk's own, sorted: the
/// list a first release, with no release tag to compare with, reads the
/// commits of.
///
/// # Errors
///
/// A build or git failure.
pub fn at_head(context: &Context) -> Result<Vec<String>, RkError> {
    Ok(side(context, "HEAD")?.into_iter().collect())
}

/// The newest stable release tag below `HEAD`, other than `exclude`,
/// where one exists: `v` and a digit, with no prerelease part.
///
/// A candidate's tag never bounds a judgment, so the releasing commits
/// behind a promoted candidate still count since the last stable release.
///
/// # Errors
///
/// git's failure to run.
pub fn stable_tag_below(root: &Utf8Path, exclude: Option<&str>) -> Result<Option<String>, RkError> {
    let mut args = vec![
        "describe",
        "--tags",
        "--abbrev=0",
        "--match",
        "v[0-9]*",
        "--exclude",
        "*-*",
    ];
    if let Some(tag) = exclude {
        args.extend(["--exclude", tag]);
    }
    args.push("HEAD");
    let output = super::git(root, &args)?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|found| !found.is_empty()))
}

/// The newest release tag below `HEAD` other than `tag`, where one exists.
///
/// # Errors
///
/// git's failure to run.
pub fn previous_tag(root: &Utf8Path, tag: &str) -> Result<Option<String>, RkError> {
    let output = super::git(
        root,
        &[
            "describe",
            "--tags",
            "--abbrev=0",
            "--match",
            "v[0-9]*",
            "--exclude",
            tag,
            "HEAD",
        ],
    )?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|found| !found.is_empty()))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{cliff_pattern, drop_generated, keep_tracked, strip_top, subtract};

    fn owned(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|path| (*path).to_owned()).collect()
    }

    fn set(paths: &[&str]) -> BTreeSet<String> {
        owned(paths).into_iter().collect()
    }

    /// SATISFIES release-engine:a-request-follows-the-packed-files
    ///
    /// The list arithmetic: the shared top directory strips, the generated
    /// sdist files drop, only tracked files stay, the two revisions unite,
    /// and rk's own destinations and record directory leave each side.
    #[test]
    fn the_packed_list_arithmetic_holds() {
        assert_eq!(
            strip_top(&owned(&[
                "acme-0.1.0/",
                "acme-0.1.0/src/a.py",
                "acme-0.1.0/PKG-INFO"
            ])),
            owned(&["src/a.py", "PKG-INFO"])
        );
        assert_eq!(
            strip_top(&owned(&["./widget/README.md", "./widget/bin/tool"])),
            owned(&["README.md", "bin/tool"])
        );
        assert_eq!(
            strip_top(&owned(&["README.md", "src/tool.sh"])),
            owned(&["README.md", "src/tool.sh"]),
            "members with no shared top directory stay whole"
        );
        assert_eq!(strip_top(&owned(&["a/x", "b/y"])), owned(&["a/x", "b/y"]));
        assert_eq!(
            drop_generated(owned(&[
                "PKG-INFO",
                "pyproject.toml.orig",
                "pyproject.toml",
                "src/PKG-INFO"
            ])),
            owned(&["pyproject.toml", "src/PKG-INFO"])
        );
        let tracked = set(&["pyproject.toml", "src/a.py", "Makefile"]);
        assert_eq!(
            keep_tracked(
                owned(&["pyproject.toml", "src/a.py", "build/out.bin"]),
                &tracked
            ),
            set(&["pyproject.toml", "src/a.py"])
        );
        let head = subtract(
            set(&[
                ".gitlab-ci.yml",
                ".release-kit/config.toml",
                "src/a.py",
                "cliff.toml",
            ]),
            &set(&[".gitlab-ci.yml", "cliff.toml"]),
        );
        let at_tag = subtract(
            set(&[".gitlab-ci.yml", "old.py", "cliff.toml"]),
            &set(&["cliff.toml"]),
        );
        let mut union = head;
        union.extend(at_tag);
        assert_eq!(
            union,
            set(&[".gitlab-ci.yml", "old.py", "src/a.py"]),
            "a destination the tag's record does not name counts from the tag's side"
        );
        assert_eq!(cliff_pattern("src/a[1]*.py"), "src/a[[]1[]][*].py");
        assert_eq!(cliff_pattern("src/tool.sh"), "src/tool.sh");
    }

    /// The set a declaration selects from a tracked tree.
    fn selected(
        include: &[&str],
        exclude: &[&str],
        tracked: &[&str],
        recorded: &[&str],
    ) -> BTreeSet<String> {
        let declaration =
            super::Declaration::new(&owned(include), &owned(exclude)).expect("the patterns parse");
        super::declared(&set(tracked), &declaration, &set(recorded))
    }

    /// SATISFIES release-engine:the-bash-release-packs-its-declaration
    ///
    /// Cargo's `include` and `exclude` semantics: a name matches at any
    /// depth, a leading `/` anchors it at the root, a trailing `/` matches a
    /// directory, `**` crosses directories, `!` inside `include` carves out
    /// an exception, `include` overrides `exclude`, and with neither key
    /// every tracked file ships. Only tracked files ship, `VERSION` always
    /// ships, and rk's destinations and `.release-kit/` ship only where an
    /// `include` pattern names them.
    #[test]
    fn the_declaration_follows_cargo_semantics() {
        let tree = [
            "README.md",
            "VERSION",
            "Makefile",
            "docs/README.md",
            "docs/guide.md",
            "src/tool.sh",
            "src/lib/util.sh",
            "src/lib/test_util.sh",
            "tests/check.sh",
            "nested/tests/data.txt",
            "SECURITY.md",
            ".gitlab-ci.yml",
            ".release-kit/config.toml",
            ".release-kit/manifest.json",
        ];
        let recorded = [".gitlab-ci.yml", "SECURITY.md", "VERSION"];
        let all = |paths: &[&str]| set(paths);
        // Neither key: every tracked file, without rk's own, with VERSION.
        assert_eq!(
            selected(&[], &[], &tree, &recorded),
            all(&[
                "README.md",
                "VERSION",
                "Makefile",
                "docs/README.md",
                "docs/guide.md",
                "src/tool.sh",
                "src/lib/util.sh",
                "src/lib/test_util.sh",
                "tests/check.sh",
                "nested/tests/data.txt",
            ])
        );
        // A bare name matches at any depth; a leading slash only at the root.
        let any_depth = selected(&[], &["README.md"], &tree, &recorded);
        assert!(!any_depth.contains("README.md") && !any_depth.contains("docs/README.md"));
        let anchored = selected(&[], &["/README.md"], &tree, &recorded);
        assert!(!anchored.contains("README.md") && anchored.contains("docs/README.md"));
        // A trailing slash matches a directory, at any depth unless anchored.
        let dirs = selected(&[], &["tests/"], &tree, &recorded);
        assert!(!dirs.contains("tests/check.sh") && !dirs.contains("nested/tests/data.txt"));
        let root_dir = selected(&[], &["/tests/"], &tree, &recorded);
        assert!(!root_dir.contains("tests/check.sh") && root_dir.contains("nested/tests/data.txt"));
        // `**` crosses directories, and `*` stays within one.
        let deep = selected(&["src/**/*.sh"], &[], &tree, &recorded);
        assert_eq!(
            deep,
            all(&[
                "VERSION",
                "src/tool.sh",
                "src/lib/util.sh",
                "src/lib/test_util.sh"
            ])
        );
        let shallow = selected(&["/src/*.sh"], &[], &tree, &recorded);
        assert_eq!(shallow, all(&["VERSION", "src/tool.sh"]));
        // `!` inside include carves out an exception.
        assert_eq!(
            selected(&["src/", "!test_*.sh"], &[], &tree, &recorded),
            all(&["VERSION", "src/tool.sh", "src/lib/util.sh"])
        );
        // Include overrides exclude.
        assert_eq!(
            selected(&["src/"], &["src/"], &tree, &recorded),
            all(&[
                "VERSION",
                "src/tool.sh",
                "src/lib/util.sh",
                "src/lib/test_util.sh"
            ])
        );
        // Untracked files never ship: the set comes from the tree alone.
        assert!(!selected(&["*.txt"], &[], &tree, &recorded).contains("notes.txt"));
        // VERSION ships even where every pattern leaves it out.
        assert!(selected(&[], &["VERSION"], &tree, &recorded).contains("VERSION"));
        assert!(selected(&["src/"], &[], &tree, &recorded).contains("VERSION"));
        // rk's own files ship only where include names them.
        assert!(!selected(&[], &[], &tree, &recorded).contains("SECURITY.md"));
        assert!(!selected(&["*.md"], &[], &tree, &recorded).contains(".release-kit/config.toml"));
        let named = selected(&["/SECURITY.md", "src/"], &[], &tree, &recorded);
        assert!(named.contains("SECURITY.md") && !named.contains(".gitlab-ci.yml"));
        assert!(
            selected(&[".release-kit/"], &[], &tree, &recorded)
                .contains(".release-kit/config.toml")
        );
        // A pattern that matches nothing selects VERSION alone.
        assert_eq!(
            selected(&["/lib/"], &[], &tree, &recorded),
            all(&["VERSION"])
        );
    }
}
