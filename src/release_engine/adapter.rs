//! The driver adapters: the one owner of where each driver keeps its
//! version, how the version is spelled, how it is written, how the
//! release artifacts build, and which files the release packs.
//!
//! The readers here serve every driver, release-plz's rust included,
//! because `rk lines mint` and the release status read a rust version
//! too. The [`DriverAdapter`] implementations serve the drivers the
//! engine releases, bash and python, and each writes the version through
//! the native tool of its ecosystem.
//!
//! SATISFIES release-engine:the-adapter-owns-the-version-file

use std::collections::BTreeSet;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// A release version: `<major>.<minor>.<patch>`, and the candidate number
/// when it is a release candidate. Any other prerelease or build segment
/// is not a version release-kit writes or reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// The major position.
    pub major: u64,
    /// The minor position.
    pub minor: u64,
    /// The patch position.
    pub patch: u64,
    /// The candidate number, for a release candidate.
    pub rc: Option<u64>,
}

impl Version {
    /// The unreleased baseline a bash target starts at.
    pub const BASELINE: Self = Self {
        major: 0,
        minor: 0,
        patch: 0,
        rc: None,
    };

    /// Parse the semver spelling `1.4.3` or `1.4.3-rc.2`, or the PEP 440
    /// spelling `1.4.3rc2`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let (stable, rc) = if let Some((stable, n)) = text.split_once("-rc.") {
            (stable, Some(n))
        } else if let Some((stable, n)) = text.split_once("rc") {
            (stable, Some(n))
        } else {
            (text, None)
        };
        let number = |part: &str| -> Option<u64> {
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse().ok())
                .flatten()
        };
        let mut parts = stable.split('.');
        let version = Self {
            major: number(parts.next()?)?,
            minor: number(parts.next()?)?,
            patch: number(parts.next()?)?,
            rc: match rc {
                Some(n) => Some(number(n)?),
                None => None,
            },
        };
        parts.next().is_none().then_some(version)
    }

    /// The version a release tag names: `v1.4.3` or `v1.4.3-rc.2`.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        let version = tag.strip_prefix('v')?;
        let parsed = Self::parse(version)?;
        // A tag carries the semver spelling alone.
        (parsed.rc.is_none() || version.contains("-rc.")).then_some(parsed)
    }

    /// The version as the driver's manifest spells it: PEP 440 for
    /// python, semver for every other driver.
    ///
    /// SATISFIES release-engine:the-python-version-maps-to-the-tag
    #[must_use]
    pub fn spelled(self, driver: &str) -> String {
        let stable = format!("{}.{}.{}", self.major, self.minor, self.patch);
        match (self.rc, driver) {
            (None, _) => stable,
            (Some(n), "python") => format!("{stable}rc{n}"),
            (Some(n), _) => format!("{stable}-rc.{n}"),
        }
    }

    /// The tag the release creates for this version, the same on every
    /// driver: the semver spelling behind a `v`.
    #[must_use]
    pub fn tag(self) -> String {
        format!("v{}", self.spelled("semver"))
    }

    /// Whether the version is a release candidate.
    #[must_use]
    pub const fn is_candidate(self) -> bool {
        self.rc.is_some()
    }
}

/// The file that carries a driver's version, with the table that holds
/// its `version` key, or `None` for the `VERSION` file whose whole content
/// is the version.
#[must_use]
pub fn version_file(driver: &str) -> Option<(&'static str, Option<&'static str>)> {
    match driver {
        "rust" => Some(("Cargo.toml", Some("package"))),
        "python" => Some(("pyproject.toml", Some("project"))),
        "bash" => Some(("VERSION", None)),
        _ => None,
    }
}

/// The version a version file's text carries, in the technology's own
/// spelling: `[package].version` in `Cargo.toml`, `[project].version` in
/// `pyproject.toml`, the trimmed `VERSION` file.
#[must_use]
pub fn version_in(driver: &str, text: &str) -> Option<String> {
    let (_, table) = version_file(driver)?;
    let Some(table) = table else {
        let version = text.trim();
        return (!version.is_empty()).then(|| version.to_owned());
    };
    let value: toml::Table = toml::from_str(text).ok()?;
    value
        .get(table)?
        .get("version")?
        .as_str()
        .map(str::to_owned)
}

/// The version the target's own version file carries, in the
/// technology's own spelling.
#[must_use]
pub fn manifest_version(target: &Path, driver: &str) -> Option<String> {
    let (file, _) = version_file(driver)?;
    let text = std::fs::read_to_string(target.join(file)).ok()?;
    version_in(driver, &text)
}

/// The package name the registry knows, read from the driver's version
/// file: `[package].name` in `Cargo.toml`, `[project].name` in
/// `pyproject.toml`, and none for a `VERSION` file.
#[must_use]
pub fn package_name(target: &Path, driver: &str) -> Option<String> {
    let (file, table) = version_file(driver)?;
    let table = table?;
    let text = std::fs::read_to_string(target.join(file)).ok()?;
    let value: toml::Table = toml::from_str(&text).ok()?;
    value.get(table)?.get("name")?.as_str().map(str::to_owned)
}

/// What the engine asks of a driver: its version, read and written
/// through the native tool, the tag a version takes, and its build.
///
/// The registry upload stays in CI, where the publishing identity is the
/// job, so the trait has no publish method.
pub trait DriverAdapter {
    /// The driver's name, as the record states it.
    fn driver(&self) -> &'static str;

    /// The version the target carries now.
    ///
    /// # Errors
    ///
    /// A refusal where the version cannot be read or is not one release-kit
    /// releases.
    fn read_version(&self, root: &Utf8Path) -> Result<Version, RkError>;

    /// Write `version` and return every file the write changed, relative
    /// to `root`.
    ///
    /// # Errors
    ///
    /// The native tool's failure.
    fn write_version(&self, root: &Utf8Path, version: Version)
    -> Result<Vec<Utf8PathBuf>, RkError>;

    /// The tag a version releases as.
    fn tag_of(&self, version: Version) -> String {
        version.tag()
    }

    /// Build the release artifacts into `dist/` and return them, relative
    /// to `root`. `epoch` is the commit time, set as `SOURCE_DATE_EPOCH`.
    ///
    /// # Errors
    ///
    /// The build's failure, or a build that left no artifact.
    fn build(&self, root: &Utf8Path, epoch: &str) -> Result<Vec<Utf8PathBuf>, RkError>;

    /// The files the release packs at the revision `rev` of the checkout
    /// at `root`, each a tracked file of that revision, relative to the
    /// root. A python build runs in a temporary worktree outside the
    /// checkout, and a bash list reads the revision's tree alone.
    ///
    /// SATISFIES release-engine:a-request-follows-the-packed-files
    ///
    /// # Errors
    ///
    /// The build's failure, or a package that holds no tracked file.
    fn packed(&self, root: &Utf8Path, rev: &str) -> Result<BTreeSet<String>, RkError>;

    /// Why the request opens nothing where neither revision packs a file
    /// past rk's own.
    fn nothing_packed(&self) -> String;
}

/// The adapter for a driver the engine releases. A bash release takes the
/// name its archive carries, `<name>-<version>.tar.gz`, and the verify
/// command the record names.
#[must_use]
pub fn for_driver(
    driver: &str,
    name: &str,
    verify: Option<&str>,
) -> Option<Box<dyn DriverAdapter>> {
    match driver {
        "bash" => Some(Box::new(Bash {
            name: name.to_owned(),
            verify: verify.map(str::to_owned),
        })),
        "python" => Some(Box::new(Python)),
        _ => None,
    }
}

/// A version the file carries that release-kit does not release.
fn unreadable(file: &str, found: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::StateDrift,
            format!("{file} carries {found:?}, which is not a version release-kit releases"),
        )
        .expected("a <major>.<minor>.<patch> version, or a release candidate of one")
        .target_state("unchanged"),
    )
}

/// Run a tool and keep its output, a spawn failure and a failed run both
/// typed with the tool's own words.
fn run_tool(
    mut command: std::process::Command,
    what: &str,
) -> Result<std::process::Output, RkError> {
    let output = command.output().map_err(|source| {
        RkError::refusal(
            Diagnostic::new(
                Reason::PrerequisiteUnmet,
                format!("{what} did not run: {source}"),
            )
            .expected(format!("{what} on PATH, at the version the job installs"))
            .target_state("unchanged"),
        )
    })?;
    if output.status.success() {
        return Ok(output);
    }
    let detail = String::from_utf8_lossy(&output.stderr)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .trim()
        .to_owned();
    Err(RkError::subprocess(
        Diagnostic::new(Reason::SubprocessFailed, format!("{what} failed: {detail}"))
            .expected(format!("{what} to succeed")),
    ))
}

/// The files of `dir` whose name ends with one of `suffixes`, relative to
/// `root` and sorted.
fn listed(root: &Utf8Path, dir: &str, suffixes: &[&str]) -> Result<Vec<Utf8PathBuf>, RkError> {
    let mut found = Vec::new();
    let path = root.join(dir);
    if !path.is_dir() {
        return Ok(found);
    }
    for entry in std::fs::read_dir(&path).map_err(RkError::Io)? {
        let entry = entry.map_err(RkError::Io)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_file() && suffixes.iter().any(|suffix| name.ends_with(suffix)) {
            found.push(Utf8PathBuf::from(format!("{dir}/{name}")));
        }
    }
    found.sort();
    Ok(found)
}

/// Clear `dist/`, so it holds only what this build writes.
fn clear_dist(root: &Utf8Path) -> Result<(), RkError> {
    let dist = root.join("dist");
    if dist.exists() {
        std::fs::remove_dir_all(&dist).map_err(RkError::Io)?;
    }
    Ok(())
}

/// A build that produced nothing the contract names.
fn empty_build(contract: &str) -> RkError {
    RkError::check_failed(
        Diagnostic::new(
            Reason::PostconditionFailed,
            format!("the build left no {contract} in dist/"),
        )
        .expected(format!("a build that writes {contract} into dist/"))
        .action("rk method recovery: The artifact build failed")
        .target_state("no forge write ran"),
    )
}

/// The bash driver: the `VERSION` file, and the archive `rk` packs from
/// the declared tracked files of `HEAD`, `dist/<name>-<version>.tar.gz`
/// beside its `.sha256`, proven by the recorded verify command.
///
/// SATISFIES release-engine:the-bash-release-packs-its-declaration
#[derive(Debug, Clone)]
pub struct Bash {
    /// The name the archive and its top directory carry: the last segment
    /// of the project path on the forge.
    pub name: String,
    /// The `[profile.release.build].verify` command the target records.
    pub verify: Option<String>,
}

impl Bash {
    /// Run the verify command in the unpacked archive, in a temporary
    /// directory outside the checkout, with `SOURCE_DATE_EPOCH` set.
    fn verify(&self, archive: &Utf8Path, top: &str, epoch: &str) -> Result<(), RkError> {
        let Some(verify) = &self.verify else {
            return Ok(());
        };
        let scratch = super::packed::TempDir::new("verify")?;
        super::packed::unpack(archive, scratch.path())?;
        let output = std::process::Command::new(crate::probes::sh_bin())
            .args(["-c", verify])
            .current_dir(scratch.path().join(top).as_std_path())
            .env("SOURCE_DATE_EPOCH", epoch)
            .output()
            .map_err(|source| {
                RkError::refusal(
                    Diagnostic::new(
                        Reason::PrerequisiteUnmet,
                        format!("the verify command {verify:?} did not run: {source}"),
                    )
                    .expected("sh on PATH")
                    .target_state("no upload, tag, or release ran"),
                )
            })?;
        if output.status.success() {
            return Ok(());
        }
        let detail = super::last_line(if output.stderr.is_empty() {
            &output.stdout
        } else {
            &output.stderr
        });
        Err(RkError::check_failed(
            Diagnostic::new(
                Reason::PostconditionFailed,
                format!(
                    "the verify command {verify:?}, {}, failed in the unpacked {top}.tar.gz: {detail}",
                    super::packed::VERIFY_KEY
                ),
            )
            .expected("a package that builds and installs from its own files")
            .action(format!(
                "ship the files the command needs through {} and {}, or correct {}; rk release build --list prints what the package holds",
                super::packed::INCLUDE_KEY,
                super::packed::EXCLUDE_KEY,
                super::packed::VERIFY_KEY
            ))
            .target_state("no upload, tag, or release ran"),
        ))
    }
}

impl DriverAdapter for Bash {
    fn driver(&self) -> &'static str {
        "bash"
    }

    fn read_version(&self, root: &Utf8Path) -> Result<Version, RkError> {
        let found = manifest_version(root.as_std_path(), "bash").ok_or_else(|| {
            RkError::refusal(
                Diagnostic::new(Reason::StateDrift, "the target carries no VERSION file")
                    .expected("a VERSION file holding the committed version")
                    .target_state("unchanged"),
            )
        })?;
        Version::parse(&found).ok_or_else(|| unreadable("VERSION", &found))
    }

    fn write_version(
        &self,
        root: &Utf8Path,
        version: Version,
    ) -> Result<Vec<Utf8PathBuf>, RkError> {
        std::fs::write(
            root.join("VERSION"),
            format!("{}\n", version.spelled("bash")),
        )
        .map_err(RkError::Io)?;
        Ok(vec![Utf8PathBuf::from("VERSION")])
    }

    /// Pack the declared set of `HEAD` from its git objects, never from
    /// the working tree, then verify it before anything publishes.
    fn build(&self, root: &Utf8Path, epoch: &str) -> Result<Vec<Utf8PathBuf>, RkError> {
        clear_dist(root)?;
        let files = self.packed(root, "HEAD")?;
        if super::packed::is_empty_package(&files) {
            return Err(super::packed::empty_package("HEAD"));
        }
        let at_head = super::git_ok(root, &["show", "HEAD:VERSION"], "VERSION at HEAD")?;
        let version = Version::parse(at_head.trim())
            .ok_or_else(|| unreadable("VERSION", at_head.trim()))?
            .spelled("bash");
        let top = format!("{}-{version}", self.name);
        let tarball = Utf8PathBuf::from(format!("dist/{top}.tar.gz"));
        let checksum = Utf8PathBuf::from(format!("dist/{top}.tar.gz.sha256"));
        std::fs::create_dir_all(root.join("dist")).map_err(RkError::Io)?;
        let time = epoch.trim().parse::<u64>().map_err(|_| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    format!("the commit time {epoch:?} is not a number of seconds"),
                )
                .expected("git log --format=%ct")
                .target_state("unchanged"),
            )
        })?;
        super::packed::pack(root, "HEAD", &files, &top, time, &root.join(&tarball))?;
        let digest = super::file_sha256(&root.join(&tarball))?;
        std::fs::write(root.join(&checksum), format!("{digest}  {top}.tar.gz\n"))
            .map_err(RkError::Io)?;
        if let Err(error) = self.verify(&root.join(&tarball), &top, epoch) {
            clear_dist(root)?;
            return Err(error);
        }
        Ok(vec![tarball, checksum])
    }

    /// The declared set of the revision, read from its own tree and its
    /// own configuration, with no build.
    fn packed(&self, root: &Utf8Path, rev: &str) -> Result<BTreeSet<String>, RkError> {
        super::packed::bash_set(root, rev)
    }

    fn nothing_packed(&self) -> String {
        super::packed::NOTHING_YET.to_owned()
    }
}

/// The python driver: a uv project with a static version, written and
/// relocked through `uv version`, and built through `uv build`.
#[derive(Debug, Clone, Copy)]
pub struct Python;

impl DriverAdapter for Python {
    fn driver(&self) -> &'static str {
        "python"
    }

    fn read_version(&self, root: &Utf8Path) -> Result<Version, RkError> {
        let mut command = std::process::Command::new(crate::probes::uv_bin());
        command
            .args(["version", "--short"])
            .current_dir(root.as_std_path());
        let output = run_tool(command, "uv version --short")?;
        let found = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Version::parse(&found).ok_or_else(|| unreadable("pyproject.toml", &found))
    }

    fn write_version(
        &self,
        root: &Utf8Path,
        version: Version,
    ) -> Result<Vec<Utf8PathBuf>, RkError> {
        let spelled = version.spelled("python");
        let mut command = std::process::Command::new(crate::probes::uv_bin());
        // `--no-sync` relocks and leaves the environment alone.
        command
            .args(["version", &spelled, "--no-sync"])
            .current_dir(root.as_std_path());
        run_tool(command, &format!("uv version {spelled} --no-sync"))?;
        let mut written = vec![Utf8PathBuf::from("pyproject.toml")];
        if root.join("uv.lock").is_file() {
            written.push(Utf8PathBuf::from("uv.lock"));
        }
        Ok(written)
    }

    fn build(&self, root: &Utf8Path, epoch: &str) -> Result<Vec<Utf8PathBuf>, RkError> {
        clear_dist(root)?;
        let mut command = std::process::Command::new(crate::probes::uv_bin());
        command
            .arg("build")
            .current_dir(root.as_std_path())
            .env("SOURCE_DATE_EPOCH", epoch);
        run_tool(command, "uv build")?;
        let artifacts = listed(root, "dist", &[".tar.gz", ".whl"])?;
        if artifacts.is_empty() {
            return Err(empty_build("sdist or wheel"));
        }
        Ok(artifacts)
    }

    /// The sdist's members are source paths and hold every file the wheel
    /// builds from, so the sdist alone is the list, with every backend.
    fn packed(&self, root: &Utf8Path, rev: &str) -> Result<BTreeSet<String>, RkError> {
        let tracked = super::packed::tracked(root, rev)?;
        let scratch = super::packed::Scratch::new(root, rev)?;
        let out = scratch.out();
        let mut command = std::process::Command::new(crate::probes::uv_bin());
        command
            .args(["build", "--sdist", "--out-dir", out.as_str()])
            .current_dir(scratch.tree().as_std_path())
            .env("SOURCE_DATE_EPOCH", super::packed::epoch(root, rev)?);
        run_tool(command, "uv build --sdist")?;
        let mut members = Vec::new();
        for entry in std::fs::read_dir(&out).map_err(RkError::Io)? {
            let path = entry.map_err(RkError::Io)?.path();
            let Some(path) = Utf8Path::from_path(&path) else {
                continue;
            };
            if path.as_str().ends_with(".tar.gz") {
                members.extend(super::packed::drop_generated(super::packed::strip_top(
                    &super::packed::members(path)?,
                )));
            }
        }
        let kept = super::packed::keep_tracked(members, &tracked);
        if kept.is_empty() {
            return Err(super::packed::mirrors_nothing("the sdist uv builds", rev));
        }
        Ok(kept)
    }

    fn nothing_packed(&self) -> String {
        "no packed file exists yet: the sdist holds no tracked file past rk's own".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{DriverAdapter as _, Version, version_in};

    fn version(text: &str) -> Version {
        Version::parse(text).expect("a version release-kit reads")
    }

    #[test]
    fn a_version_reads_and_spells_each_technology() {
        let rc = version("1.4.3-rc.2");
        assert_eq!(rc, version("1.4.3rc2"));
        assert_eq!(rc.spelled("rust"), "1.4.3-rc.2");
        assert_eq!(rc.spelled("bash"), "1.4.3-rc.2");
        assert_eq!(rc.spelled("python"), "1.4.3rc2");
        assert_eq!(rc.tag(), "v1.4.3-rc.2");
        assert_eq!(version("1.4.3").spelled("python"), "1.4.3");
        for other in ["1.4", "1.4.3.1", "1.4.3-beta.1", "1.4.3-rc.", "v1.4.3", ""] {
            assert_eq!(Version::parse(other), None, "{other}");
        }
    }

    #[test]
    fn a_tag_reads_back_to_its_version() {
        assert_eq!(Version::from_tag("v1.4.3"), Some(version("1.4.3")));
        assert_eq!(Version::from_tag("v1.4.3-rc.2"), Some(version("1.4.3rc2")));
        assert_eq!(Version::from_tag("v1.4.3rc2"), None);
        assert_eq!(Version::from_tag("1.4.3"), None);
    }

    /// SATISFIES release-engine:the-python-version-maps-to-the-tag
    #[test]
    fn the_python_version_maps_to_the_tag_and_back() {
        for (pep440, tag) in [("1.4.3", "v1.4.3"), ("1.4.3rc2", "v1.4.3-rc.2")] {
            let parsed = version(pep440);
            assert_eq!(parsed.tag(), tag);
            assert_eq!(super::Python.tag_of(parsed), tag);
            let back = Version::from_tag(tag).expect("a release tag");
            assert_eq!(back.spelled("python"), pep440);
        }
    }

    #[test]
    fn a_version_file_text_reads_per_driver() {
        assert_eq!(version_in("bash", "0.2.0\n").as_deref(), Some("0.2.0"));
        assert_eq!(
            version_in(
                "python",
                "[project]\nname = \"acme\"\nversion = \"1.4.3rc2\"\n"
            )
            .as_deref(),
            Some("1.4.3rc2")
        );
        assert_eq!(version_in("bash", "  \n"), None);
        assert_eq!(version_in("go", "1.0.0"), None);
    }
}
