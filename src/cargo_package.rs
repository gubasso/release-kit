//! What one Cargo package ships, read from `cargo package --list`.
//!
//! release-plz attributes a commit to a package only where the commit
//! changes a file this listing prints, for the changelog and for the
//! decision to release alike. Two callers read the listing for that reason:
//! the `package-check` setup step, which faults a crate that ships
//! release-kit's own files, and `rk integrate`, which warns when a message
//! states release intent over a change the listing does not reach.
//!
//! The module spawns nothing. Each caller passes its own cargo call, so the
//! setup keeps its journaled executor and integrate keeps a plain process,
//! while the shape of the answer and every match rule live here once.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

/// The one landed file a crate must ship: the reporting policy.
///
/// A consumer must find it in the artifact they downloaded. A reporting
/// policy readable on the forge alone is a policy the consumer who has only
/// the package cannot follow.
pub const POLICY_DESTINATION: &str = "SECURITY.md";

/// The directory release-kit owns by name: the target configuration and
/// the landing record live under it and nothing else does.
const OWN_DIRECTORY: &str = ".release-kit";

/// What one cargo call answered.
#[derive(Debug)]
pub struct Answer {
    /// Whether the call succeeded.
    pub success: bool,
    /// Its standard output.
    pub stdout: Vec<u8>,
    /// Its standard error.
    pub stderr: Vec<u8>,
}

/// What reading the package's file list found.
#[derive(Debug)]
pub enum Probe {
    /// The sole default package rooted at the target, and what it ships.
    Listed(Listing),
    /// Another workspace shape, whose listing no command answers
    /// unambiguously: no listing ran.
    OtherShape,
    /// `cargo metadata` failed, with its standard error.
    MetadataFailed(Vec<u8>),
    /// `cargo package --list` failed, with its standard error.
    ListingFailed(Vec<u8>),
}

/// Read the file list of the sole default package rooted at
/// `root_manifest`.
///
/// `cargo package --list` prints one path per line for one package and
/// emits no stable delimiter when it selects several, and a nested package
/// cannot include a file above its own root. So the listing runs only for a
/// sole selected default member whose manifest is the target's own
/// `Cargo.toml`. `list_flags` go after `--allow-dirty`, for a caller that
/// must keep the listing from writing.
///
/// # Errors
///
/// Propagates the caller's own failure to run cargo at all.
pub fn probe<E>(
    root_manifest: &Path,
    list_flags: &[&str],
    mut cargo: impl FnMut(&[&str]) -> Result<Answer, E>,
) -> Result<Probe, E> {
    let metadata = cargo(&["metadata", "--no-deps", "--format-version", "1"])?;
    if !metadata.success {
        return Ok(Probe::MetadataFailed(metadata.stderr));
    }
    let Some(manifest) = sole_root_package(&metadata.stdout, root_manifest) else {
        return Ok(Probe::OtherShape);
    };
    let mut args = vec!["package", "--list", "--allow-dirty"];
    args.extend_from_slice(list_flags);
    args.extend_from_slice(&["--manifest-path", &manifest]);
    let listing = cargo(&args)?;
    Ok(if listing.success {
        Probe::Listed(Listing::parse(&listing.stdout))
    } else {
        Probe::ListingFailed(listing.stderr)
    })
}

/// The manifest path of the one selected default package rooted at the
/// target, or `None` for every other workspace shape.
#[must_use]
pub fn sole_root_package(metadata: &[u8], root_manifest: &Path) -> Option<String> {
    let document: Value = serde_json::from_slice(metadata).ok()?;
    let defaults: Vec<&str> = document
        .get("workspace_default_members")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let [only] = defaults.as_slice() else {
        return None;
    };
    let manifest = document
        .get("packages")?
        .as_array()?
        .iter()
        .find(|package| package.get("id").and_then(Value::as_str) == Some(*only))?
        .get("manifest_path")?
        .as_str()?;
    // Compare what each path resolves to, so a symlinked or
    // differently-spelled target directory still reads as the root.
    let same =
        std::fs::canonicalize(manifest).ok()? == std::fs::canonicalize(root_manifest).ok()?;
    same.then(|| manifest.to_owned())
}

/// The paths one package ships, one per listed line.
#[derive(Debug, Default)]
pub struct Listing(BTreeSet<String>);

impl Listing {
    /// Parse `cargo package --list` output: trimmed, non-empty lines.
    #[must_use]
    pub fn parse(stdout: &[u8]) -> Self {
        Self(
            String::from_utf8_lossy(stdout)
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect(),
        )
    }

    /// Every listed path, sorted.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    /// Whether the package ships exactly `path`. An exact line, never a
    /// substring: `docs/SECURITY.md` and `SECURITY.md.bak` are different
    /// files.
    #[must_use]
    pub fn carries(&self, path: &str) -> bool {
        self.0.contains(path)
    }

    /// Whether any of `changed` is a path the package ships: the test
    /// release-plz applies to attribute a commit to the package.
    pub fn touched_by<'a>(&self, changed: impl IntoIterator<Item = &'a str>) -> bool {
        changed.into_iter().any(|path| self.carries(path))
    }

    /// Which of `forbidden` the package ships, in order.
    #[must_use]
    pub fn shipped<'a>(&self, forbidden: &'a BTreeSet<String>) -> Vec<&'a str> {
        forbidden
            .iter()
            .filter(|path| self.carries(path))
            .map(String::as_str)
            .collect()
    }
}

/// Every path release-kit lands or owns in a target that a package must
/// not ship: the target configuration, the landing record, and every
/// destination the record names, less the reporting policy.
///
/// With no record, release-kit has landed nothing, so only its own two
/// files are claimed: every other destination the sources could land would
/// be the target's own file there, and claiming it would fault a file
/// release-kit never rewrites.
#[must_use]
pub fn release_kit_paths(landed: Option<&[String]>) -> BTreeSet<String> {
    let mut paths: BTreeSet<String> = [
        crate::config::CONFIG_PATH,
        crate::landing::manifest::MANIFEST_PATH,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    paths.extend(landed.into_iter().flatten().cloned());
    paths.remove(POLICY_DESTINATION);
    paths
}

/// The `[package].exclude` entries that stop a package shipping `shipped`.
///
/// One `/.release-kit` covers everything under release-kit's own
/// directory, and each other path is rooted as it is. An exact path is
/// never wrong, where a parent directory can hold the target's own files.
#[must_use]
pub fn exclude_entries(shipped: &[&str]) -> Vec<String> {
    let own = format!("{OWN_DIRECTORY}/");
    let mut entries = Vec::new();
    if shipped.iter().any(|path| path.starts_with(&own)) {
        entries.push(format!("/{OWN_DIRECTORY}"));
    }
    let mut rest: Vec<String> = shipped
        .iter()
        .filter(|path| !path.starts_with(&own))
        .map(|path| format!("/{path}"))
        .collect();
    rest.sort();
    rest.dedup();
    entries.extend(rest);
    entries
}

/// The files release-plz reads its configuration from, in the order it
/// tries them.
pub const RELEASE_PLZ_CONFIGS: [&str; 3] = [
    "release-plz.toml",
    ".release-plz.toml",
    ".config/release-plz.toml",
];

/// One workspace member release-plz releases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    /// The package name.
    pub name: String,
    /// The package's manifest, as cargo metadata states it.
    pub manifest_path: String,
    /// Whether the package has a binary target, whose build ships the
    /// lock file.
    pub binary: bool,
}

/// The workspace members release-plz 0.4.3 releases, in the order cargo
/// metadata lists them.
///
/// A member takes part where its release-plz `release` key is not `false`
/// and Cargo's `publish` field lets it publish somewhere, or where the
/// member is `git_only` and is not an example-only package that leaves
/// `publish` unset. release-plz's own `publish` key only stops
/// `cargo publish`, so it selects nothing. A `[[package]]` entry's key
/// overrides the `[workspace]` key of the same name.
#[must_use]
pub fn released_packages(metadata: &[u8], config: Option<&str>) -> Vec<Released> {
    let Ok(document) = serde_json::from_slice::<Value>(metadata) else {
        return Vec::new();
    };
    let table: toml::Table = config
        .and_then(|text| toml::from_str(text).ok())
        .unwrap_or_default();
    let workspace = table.get("workspace");
    let packages = table.get("package").and_then(toml::Value::as_array);
    let key = |name: &str, key: &str| -> Option<bool> {
        packages
            .and_then(|list| {
                list.iter()
                    .find(|entry| entry.get("name").and_then(toml::Value::as_str) == Some(name))
            })
            .and_then(|entry| entry.get(key))
            .or_else(|| workspace.and_then(|table| table.get(key)))
            .and_then(toml::Value::as_bool)
    };
    let members: Vec<&str> = document
        .get("workspace_members")
        .and_then(Value::as_array)
        .map(|ids| ids.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    document
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|package| {
            package
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| members.contains(&id))
        })
        .filter_map(|package| {
            let name = package.get("name")?.as_str()?;
            let kinds: Vec<Vec<&str>> = package
                .get("targets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|target| {
                    target
                        .get("kind")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect()
                })
                .collect();
            let example = kinds.iter().all(|kind| kind.as_slice() == ["example"]);
            let publish = package.get("publish").filter(|value| !value.is_null());
            let publishable = publish.map_or(!example, |registries| {
                registries.as_array().is_some_and(|list| !list.is_empty())
            });
            let git_only = key(name, "git_only").unwrap_or(false);
            let unpublished_example = publish.is_none() && example;
            let takes_part = publishable || (git_only && !unpublished_example);
            (key(name, "release") != Some(false) && takes_part).then(|| Released {
                name: name.to_owned(),
                manifest_path: package
                    .get("manifest_path")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                binary: kinds.iter().any(|kind| kind.contains(&"bin")),
            })
        })
        .collect()
}

/// The files `cargo package --list` prints that the tree does not hold
/// under that name.
///
/// They are the generated manifest copy, the VCS record, and the lock,
/// which a workspace keeps at its root and which a caller adds only for a
/// package with a binary target.
pub const LISTED_NOT_TRACKED: [&str; 3] = ["Cargo.toml.orig", ".cargo_vcs_info.json", "Cargo.lock"];

#[cfg(test)]
mod tests {
    use super::{
        Answer, Listing, Probe, exclude_entries, probe, release_kit_paths, released_packages,
    };

    /// cargo metadata for members `(name, publish, kinds)`.
    fn metadata(members: &[(&str, serde_json::Value, &[&str])]) -> Vec<u8> {
        let packages: Vec<serde_json::Value> = members
            .iter()
            .map(|(name, publish, kinds)| {
                serde_json::json!({
                    "id": format!("{name} 0.1.0"), "name": name, "publish": publish,
                    "manifest_path": format!("/w/{name}/Cargo.toml"),
                    "targets": kinds.iter().map(|kind| serde_json::json!({"kind": [kind]})).collect::<Vec<_>>(),
                })
            })
            .collect();
        let ids: Vec<String> = members
            .iter()
            .map(|(name, ..)| format!("{name} 0.1.0"))
            .collect();
        serde_json::json!({"packages": packages, "workspace_members": ids})
            .to_string()
            .into_bytes()
    }

    fn names(metadata: &[u8], config: Option<&str>) -> Vec<String> {
        released_packages(metadata, config)
            .into_iter()
            .map(|package| package.name)
            .collect()
    }

    /// SATISFIES release-engine:a-rust-target-releases-one-package
    ///
    /// release-plz 0.4.3's selection: Cargo's `publish` field, the
    /// `git_only` key, the `release` key, and the example-only rule.
    #[test]
    fn release_plz_selects_the_released_members() {
        let null = serde_json::Value::Null;
        let none = serde_json::json!([]);
        let helper = metadata(&[
            ("widget", null.clone(), &["bin"]),
            ("xtask", none.clone(), &["bin"]),
        ]);
        let registry = "[workspace]\npublish = true\n";
        assert_eq!(names(&helper, Some(registry)), vec!["widget"]);
        let git_only = "[workspace]\npublish = false\ngit_only = true\n";
        assert_eq!(names(&helper, Some(git_only)), vec!["widget", "xtask"]);
        let opted_out = format!("{git_only}[[package]]\nname = \"xtask\"\nrelease = false\n");
        assert_eq!(names(&helper, Some(&opted_out)), vec!["widget"]);
        let single = metadata(&[("exobrain", none, &["bin"])]);
        assert_eq!(names(&single, Some(git_only)), vec!["exobrain"]);
        assert!(names(&single, Some(registry)).is_empty());
        let example = metadata(&[("demo", null.clone(), &["example"])]);
        assert!(names(&example, Some(git_only)).is_empty());
        let off = "[workspace]\nrelease = false\n";
        assert!(names(&metadata(&[("widget", null, &["lib"])]), Some(off)).is_empty());
        let released = released_packages(&helper, Some(registry));
        assert!(released[0].binary);
    }

    #[test]
    fn a_listing_is_trimmed_lines_without_blanks() {
        let listing = Listing::parse(b"  Cargo.toml\n\nsrc/main.rs  \n");
        assert!(listing.carries("Cargo.toml"));
        assert!(listing.carries("src/main.rs"));
        assert!(!listing.carries(""));
    }

    #[test]
    fn a_shipped_path_is_an_exact_line() {
        let listing = Listing::parse(b"docs/GLOSSARY.md\nGLOSSARY.md.bak\nnix/package.nix\n");
        let forbidden = release_kit_paths(Some(&[
            "GLOSSARY.md".to_owned(),
            "nix/package.nix".to_owned(),
        ]));
        assert_eq!(listing.shipped(&forbidden), vec!["nix/package.nix"]);
    }

    #[test]
    fn a_change_touches_the_package_only_through_a_listed_path() {
        let listing = Listing::parse(b"Cargo.toml\nsrc/main.rs\n");
        assert!(listing.touched_by(["docs/a.md", "src/main.rs"]));
        assert!(!listing.touched_by(["docs/a.md", "justfile"]));
    }

    #[test]
    fn without_a_record_only_the_two_own_files_are_claimed() {
        let paths = release_kit_paths(None);
        assert_eq!(
            paths.into_iter().collect::<Vec<_>>(),
            vec![".release-kit/config.toml", ".release-kit/manifest.json"]
        );
    }

    #[test]
    fn the_reporting_policy_is_never_claimed() {
        let paths = release_kit_paths(Some(&["SECURITY.md".to_owned()]));
        assert!(!paths.contains("SECURITY.md"));
    }

    #[test]
    fn exclude_entries_collapse_the_own_directory_first() {
        let entries = exclude_entries(&[
            "GLOSSARY.md",
            ".release-kit/manifest.json",
            ".release-kit/config.toml",
            "nix/package.nix",
        ]);
        assert_eq!(
            entries,
            vec!["/.release-kit", "/GLOSSARY.md", "/nix/package.nix"]
        );
    }

    fn answer(success: bool, stdout: &str) -> Answer {
        Answer {
            success,
            stdout: stdout.as_bytes().to_vec(),
            stderr: b"boom".to_vec(),
        }
    }

    #[test]
    fn another_shape_runs_no_listing() {
        let mut calls = Vec::new();
        let found = probe::<()>(std::path::Path::new("/nowhere/Cargo.toml"), &[], |args| {
            calls.push(args.join(" "));
            Ok(answer(
                true,
                r#"{"workspace_default_members":[],"packages":[]}"#,
            ))
        })
        .expect("the probe runs");
        assert!(matches!(found, Probe::OtherShape));
        assert_eq!(calls, vec!["metadata --no-deps --format-version 1"]);
    }

    #[test]
    fn list_flags_sit_between_allow_dirty_and_the_manifest() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let manifest = dir.path().join("Cargo.toml");
        std::fs::write(&manifest, "").expect("the manifest writes");
        let metadata = serde_json::json!({
            "workspace_default_members": ["w 0.1.0"],
            "packages": [{"id": "w 0.1.0", "manifest_path": manifest}],
        })
        .to_string();
        let mut calls = Vec::new();
        let found = probe::<()>(&manifest, &["--locked", "--offline"], |args| {
            calls.push(args.join(" "));
            Ok(if args[0] == "metadata" {
                answer(true, &metadata)
            } else {
                answer(true, "Cargo.toml\n")
            })
        })
        .expect("the probe runs");
        assert!(matches!(found, Probe::Listed(listing) if listing.carries("Cargo.toml")));
        assert_eq!(
            calls[1],
            format!(
                "package --list --allow-dirty --locked --offline --manifest-path {}",
                manifest.display()
            )
        );
    }

    #[test]
    fn a_failed_listing_is_named_not_guessed() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let manifest = dir.path().join("Cargo.toml");
        std::fs::write(&manifest, "").expect("the manifest writes");
        let metadata = serde_json::json!({
            "workspace_default_members": ["w 0.1.0"],
            "packages": [{"id": "w 0.1.0", "manifest_path": manifest}],
        })
        .to_string();
        let found = probe::<()>(&manifest, &[], |args| {
            Ok(if args[0] == "metadata" {
                answer(true, &metadata)
            } else {
                answer(false, "")
            })
        })
        .expect("the probe runs");
        assert!(matches!(found, Probe::ListingFailed(stderr) if stderr == b"boom"));
    }
}
