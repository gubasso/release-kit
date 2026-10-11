//! The migration's record: `registry.yaml` in the workspace.
//!
//! Only the `rk migrate` verbs write it. Each source is one listed file
//! with its checksum, its status, and its history; each group gathers the
//! sources one predecessor mechanism spread across files and names the
//! destinations that now carry what the project kept; each snapshot is a
//! copy of what no Git holds. A status change appends a dated history
//! entry and never rewrites an earlier one, so the registry is also the
//! migration's log.

use camino::{Utf8Component, Utf8Path};
use serde::{Deserialize, Serialize};

use super::inventory::{Kind, Placement};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// The registry shape this binary writes.
pub const SCHEMA_VERSION: u32 = 1;

/// Where one source stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Listed, and no fate decided yet.
    Pending,
    /// Its fate is decided: a group, or a destination.
    Mapped,
    /// What it carried is written to its destinations.
    Written,
    /// Its destinations were read against it and hold what it carried.
    Verified,
    /// Nothing of it is kept, for a stated reason.
    Dropped,
}

impl Status {
    /// The kebab-case word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Mapped => "mapped",
            Self::Written => "written",
            Self::Verified => "verified",
            Self::Dropped => "dropped",
        }
    }

    /// Parse the word an operator types.
    ///
    /// # Errors
    ///
    /// A usage error naming the five words.
    pub fn parse(word: &str) -> Result<Self, RkError> {
        match word {
            "pending" => Ok(Self::Pending),
            "mapped" => Ok(Self::Mapped),
            "written" => Ok(Self::Written),
            "verified" => Ok(Self::Verified),
            "dropped" => Ok(Self::Dropped),
            other => Err(RkError::Usage(format!(
                "unknown status '{other}'; the statuses are: pending, mapped, written, verified, dropped"
            ))),
        }
    }

    /// Whether a source in this status lets the migration close.
    #[must_use]
    pub const fn finished(self) -> bool {
        matches!(self, Self::Verified | Self::Dropped)
    }
}

/// One dated status change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// When, UTC to the second.
    pub at: String,
    /// The status it moved to.
    pub status: Status,
    /// Why, on a drop alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// One listed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// A slug made from the path.
    pub id: String,
    /// The target-relative path it had.
    pub path: String,
    /// The sha256 of the bytes the archive took.
    pub sha256: String,
    /// The tool that owns it.
    pub tool: String,
    /// What it is to its tool.
    pub kind: Kind,
    /// Whether the archive moved it or it stayed in place.
    pub placement: Placement,
    /// Free topic labels; sources that share one are candidates for one
    /// group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The group it belongs to, once mapped into one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Where it stands now.
    pub status: Status,
    /// Every status it took, oldest first.
    pub history: Vec<HistoryEntry>,
}

/// The sources one mechanism spread across files, and where what the
/// project kept now lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// A lowercase slug.
    pub id: String,
    /// The member source ids.
    pub sources: Vec<String>,
    /// The target-relative paths that carry what the group kept.
    #[serde(default)]
    pub destinations: Vec<String>,
}

/// A copy of what no Git holds, taken before anything destructive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// What it saves, as a slug.
    pub id: String,
    /// `git-bundle`, `tag-list`, or a forge snapshot's kind.
    pub kind: String,
    /// The workspace-relative file.
    pub file: String,
    /// The sha256 of the file.
    pub sha256: String,
    /// When it was taken.
    pub taken_at: String,
    /// Always false: a snapshot has no second copy, and a rebuild
    /// recreates the archived files alone.
    pub second_copy: bool,
}

/// The whole registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    /// The registry shape.
    pub schema_version: u32,
    /// The migration's identifier.
    pub id: String,
    /// The commit the archive read every source from.
    pub base_commit: String,
    /// Every listed file.
    pub sources: Vec<Source>,
    /// Every group the map has made.
    #[serde(default)]
    pub groups: Vec<Group>,
    /// Every snapshot the workspace holds.
    #[serde(default)]
    pub snapshots: Vec<Snapshot>,
}

/// A source id: the path, lowercased, with every run of other characters
/// turned into one hyphen.
#[must_use]
pub fn source_id(path: &str) -> String {
    slug(path)
}

/// Give every source a distinct id. In byte order of the paths, each
/// source takes the slug of its path, or, where an earlier path took that
/// slug, the slug with the first free `-<n>` counter from 2.
pub fn assign_ids(sources: &mut [Source]) {
    let mut order: Vec<usize> = (0..sources.len()).collect();
    order.sort_by(|left, right| sources[*left].path.cmp(&sources[*right].path));
    let mut taken = std::collections::BTreeSet::new();
    for index in order {
        let base = slug(&sources[index].path);
        let mut id = base.clone();
        let mut counter = 2_u32;
        while taken.contains(&id) {
            id = format!("{base}-{counter}");
            counter += 1;
        }
        taken.insert(id.clone());
        sources[index].id = id;
    }
}

/// What the file a snapshot names holds against its recorded checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrity {
    /// The recorded bytes.
    Intact,
    /// No file.
    Missing,
    /// Other bytes.
    Altered,
}

/// Read the file `snapshot` names under `workspace` against its checksum.
///
/// # Errors
///
/// A read that fails for any reason but an absent file, so a snapshot
/// that may still be whole is never mistaken for a damaged one.
pub fn integrity(workspace: &Utf8Path, snapshot: &Snapshot) -> std::io::Result<Integrity> {
    match std::fs::read(workspace.join(&snapshot.file)) {
        Ok(bytes) if crate::digest::Digest::of(&bytes).to_string() == snapshot.sha256 => {
            Ok(Integrity::Intact)
        }
        Ok(_) => Ok(Integrity::Altered),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Integrity::Missing),
        Err(error) => Err(error),
    }
}

/// Refuse a destination that leaves the target: one judged under the
/// target at close, which an absolute path or a parent step would turn
/// into a judgment of another tree.
fn check_destinations(destinations: &[String]) -> Result<(), RkError> {
    let escaping = destinations.iter().find(|destination| {
        Utf8Path::new(destination.as_str())
            .components()
            .any(|part| !matches!(part, Utf8Component::Normal(_) | Utf8Component::CurDir))
    });
    escaping.map_or(Ok(()), |destination| {
        Err(RkError::Usage(format!(
            "the destination {destination} leaves the target; name a path relative to the target root"
        )))
    })
}

/// A lowercase slug of `text`.
fn slug(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_owned()
}

/// Whether `word` is a lowercase slug, the shape of a group and a tag.
#[must_use]
pub fn is_slug(word: &str) -> bool {
    !word.is_empty()
        && !word.starts_with('-')
        && !word.ends_with('-')
        && word
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Read the registry of the workspace at `workspace`.
///
/// # Errors
///
/// [`RkError::Io`] for an unreadable file, and a refusal for one that does
/// not parse at this binary's shape.
pub fn load(workspace: &Utf8Path) -> Result<Registry, RkError> {
    let path = workspace.join(super::REGISTRY_FILE);
    let text = std::fs::read_to_string(&path)?;
    let registry: Registry = yaml_serde::from_str(&text).map_err(|error| {
        RkError::refusal(
            Diagnostic::new(
                Reason::RecordInvalid,
                format!("{path} is not a migration registry: {error}"),
            )
            .expected("the registry only rk migrate writes")
            .target_state("unchanged"),
        )
    })?;
    if registry.schema_version != SCHEMA_VERSION {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::UnsupportedSchema,
                format!(
                    "{path} declares schema_version {}, and this binary knows only {SCHEMA_VERSION}",
                    registry.schema_version
                ),
            )
            .target_state("unchanged"),
        ));
    }
    Ok(registry)
}

/// Write the registry atomically.
///
/// # Errors
///
/// A serialization or write failure.
pub fn save(workspace: &Utf8Path, registry: &Registry) -> Result<(), RkError> {
    let text = yaml_serde::to_string(registry)
        .map_err(|error| anyhow::anyhow!("the registry does not serialize: {error}"))?;
    crate::atomic::write(
        workspace.join(super::REGISTRY_FILE).as_std_path(),
        text.as_bytes(),
    )?;
    Ok(())
}

/// What one `mark` names and asks.
#[derive(Debug, Default)]
pub struct Mark<'a> {
    /// A source id, a source path, or a group id.
    pub name: &'a str,
    /// The status to record.
    pub status: Option<Status>,
    /// The group a source joins.
    pub group: Option<&'a str>,
    /// Tags a source gains.
    pub tags: &'a [String],
    /// Destinations a group gains.
    pub destinations: &'a [String],
    /// Why, on a drop.
    pub reason: Option<&'a str>,
}

/// What a mark changed: the sources it moved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Marked {
    /// `source` or `group`.
    pub subject: &'static str,
    /// The name it resolved.
    pub id: String,
    /// The sources whose status it recorded.
    pub sources: Vec<String>,
}

impl Registry {
    /// Record one mark, appending a history entry to every source it moves.
    ///
    /// # Errors
    ///
    /// A usage error for a name that resolves to nothing, a flag that does
    /// not apply to what the name resolves to, a group or tag that is not
    /// a slug, and a drop with no reason.
    pub fn mark(&mut self, mark: &Mark<'_>, at: &str) -> Result<Marked, RkError> {
        let status = mark
            .status
            .ok_or_else(|| RkError::Usage("name the status to record".into()))?;
        if status == Status::Dropped && mark.reason.is_none_or(|reason| reason.trim().is_empty()) {
            return Err(RkError::Usage(
                "a drop records its reason: pass --reason \"<why>\" (migration:a-mark-appends-and-never-rewrites)".into(),
            ));
        }
        if status != Status::Dropped && mark.reason.is_some() {
            return Err(RkError::Usage("--reason belongs to a drop alone".into()));
        }
        check_destinations(mark.destinations)?;
        for word in mark
            .group
            .into_iter()
            .chain(mark.tags.iter().map(String::as_str))
        {
            if !is_slug(word) {
                return Err(RkError::Usage(format!(
                    "'{word}' is not a lowercase slug of letters, digits, and hyphens"
                )));
            }
        }
        let entry = HistoryEntry {
            at: at.to_owned(),
            status,
            reason: mark.reason.map(str::to_owned),
        };
        if let Some(index) = self
            .sources
            .iter()
            .position(|source| source.id == mark.name || source.path == mark.name)
        {
            if !mark.destinations.is_empty() {
                return Err(RkError::Usage(
                    "--destination belongs to a group; name the group, or file this source with --group".into(),
                ));
            }
            let id = self.sources[index].id.clone();
            if let Some(group) = mark.group {
                for other in &mut self.groups {
                    other.sources.retain(|member| *member != id);
                }
                match self.groups.iter_mut().find(|other| other.id == group) {
                    Some(existing) => existing.sources.push(id.clone()),
                    None => self.groups.push(Group {
                        id: group.to_owned(),
                        sources: vec![id.clone()],
                        destinations: Vec::new(),
                    }),
                }
                self.sources[index].group = Some(group.to_owned());
            }
            let source = &mut self.sources[index];
            for tag in mark.tags {
                if !source.tags.contains(tag) {
                    source.tags.push(tag.clone());
                }
            }
            source.status = status;
            source.history.push(entry);
            return Ok(Marked {
                subject: "source",
                id: id.clone(),
                sources: vec![id],
            });
        }
        if let Some(group) = self.groups.iter_mut().find(|group| group.id == mark.name) {
            if mark.group.is_some() || !mark.tags.is_empty() {
                return Err(RkError::Usage(
                    "--group and --tag belong to a source; a group gains destinations alone".into(),
                ));
            }
            for destination in mark.destinations {
                if !group.destinations.contains(destination) {
                    group.destinations.push(destination.clone());
                }
            }
            let members = group.sources.clone();
            let id = group.id.clone();
            for source in self
                .sources
                .iter_mut()
                .filter(|source| members.contains(&source.id))
            {
                source.status = status;
                source.history.push(entry.clone());
            }
            return Ok(Marked {
                subject: "group",
                id,
                sources: members,
            });
        }
        Err(RkError::Usage(format!(
            "'{}' names no source and no group in the migration {}; rk migrate status --target . lists them",
            mark.name, self.id
        )))
    }

    /// Why the migration cannot close yet, one reason per blocking fact,
    /// judged against the target's own files.
    #[must_use]
    pub fn unfinished(&self, target: &Utf8Path) -> Vec<String> {
        let mut reasons = Vec::new();
        for source in &self.sources {
            if !source.status.finished() {
                reasons.push(format!("{} is {}", source.path, source.status.as_str()));
                continue;
            }
            if source.status != Status::Verified {
                continue;
            }
            let Some(group) = source
                .group
                .as_deref()
                .and_then(|id| self.groups.iter().find(|group| group.id == id))
            else {
                reasons.push(format!(
                    "{} is verified and belongs to no group",
                    source.path
                ));
                continue;
            };
            if group.destinations.is_empty() {
                reasons.push(format!("the group {} names no destination", group.id));
            }
        }
        // A group whose every source was dropped or filed elsewhere
        // carries nothing, so its planned destinations bind nothing.
        let live = |group: &Group| {
            group.sources.iter().any(|member| {
                self.sources
                    .iter()
                    .any(|source| source.id == *member && source.status != Status::Dropped)
            })
        };
        // A destination counts only as a regular file whose resolved path
        // stays inside the target: a symlink out of it would judge another
        // tree, and a directory carries nothing a group wrote.
        let root = std::fs::canonicalize(target).ok();
        let held = |destination: &str| {
            std::fs::canonicalize(target.join(destination)).is_ok_and(|path| {
                path.is_file() && root.as_ref().is_some_and(|root| path.starts_with(root))
            })
        };
        for group in self.groups.iter().filter(|group| live(group)) {
            for destination in &group.destinations {
                if !held(destination) {
                    reasons.push(format!(
                        "the group {} names {destination}, which the target does not hold",
                        group.id
                    ));
                }
            }
        }
        reasons.sort();
        reasons.dedup();
        reasons
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Group, HistoryEntry, Mark, Registry, SCHEMA_VERSION, Source, Status, assign_ids, source_id,
    };
    use crate::migrate::inventory::{Kind, Placement};

    fn registry() -> Registry {
        let source = |path: &str| Source {
            id: source_id(path),
            path: path.to_owned(),
            sha256: "0".repeat(64),
            tool: "release-please".to_owned(),
            kind: Kind::ReleaseConfig,
            placement: Placement::Archived,
            tags: Vec::new(),
            group: None,
            status: Status::Pending,
            history: vec![HistoryEntry {
                at: "2026-10-09T14:00:00Z".to_owned(),
                status: Status::Pending,
                reason: None,
            }],
        };
        Registry {
            schema_version: SCHEMA_VERSION,
            id: "20261009T140000Z".to_owned(),
            base_commit: "4f1c2e0".to_owned(),
            sources: vec![
                source("release-please-config.json"),
                source(".release-please-manifest.json"),
            ],
            groups: Vec::<Group>::new(),
            snapshots: Vec::new(),
        }
    }

    #[test]
    fn a_source_id_is_a_slug_of_its_path() {
        assert_eq!(
            source_id(".github/workflows/release-please.yml"),
            "github-workflows-release-please-yml"
        );
    }

    /// SATISFIES migration:a-mark-appends-and-never-rewrites
    #[test]
    fn a_mark_appends_history_and_never_rewrites_an_earlier_entry() {
        let mut registry = registry();
        let before = registry.sources[0].history.clone();
        let tags = vec!["versioning".to_owned()];
        registry
            .mark(
                &Mark {
                    name: "release-please-config.json",
                    status: Some(Status::Mapped),
                    group: Some("release-please"),
                    tags: &tags,
                    ..Mark::default()
                },
                "2026-10-09T15:00:00Z",
            )
            .expect("the source files into a new group");
        let destinations = vec!["release-plz.toml".to_owned()];
        let marked = registry
            .mark(
                &Mark {
                    name: "release-please",
                    status: Some(Status::Written),
                    destinations: &destinations,
                    ..Mark::default()
                },
                "2026-10-09T16:00:00Z",
            )
            .expect("the group moves its members");
        assert_eq!(marked.sources, ["release-please-config-json"]);
        let source = &registry.sources[0];
        assert_eq!(&source.history[..before.len()], &before[..]);
        assert_eq!(
            source
                .history
                .iter()
                .map(|entry| entry.status)
                .collect::<Vec<_>>(),
            [Status::Pending, Status::Mapped, Status::Written]
        );
        assert_eq!(source.tags, ["versioning"]);
        assert_eq!(registry.groups[0].destinations, ["release-plz.toml"]);
    }

    #[test]
    fn a_drop_needs_its_reason_and_a_flag_must_fit_its_subject() {
        let mut registry = registry();
        let at = "2026-10-09T15:00:00Z";
        assert!(
            registry
                .mark(
                    &Mark {
                        name: "release-please-config.json",
                        status: Some(Status::Dropped),
                        ..Mark::default()
                    },
                    at
                )
                .is_err()
        );
        let destinations = vec!["x".to_owned()];
        assert!(
            registry
                .mark(
                    &Mark {
                        name: "release-please-config.json",
                        status: Some(Status::Mapped),
                        destinations: &destinations,
                        ..Mark::default()
                    },
                    at
                )
                .is_err()
        );
        assert!(
            registry
                .mark(
                    &Mark {
                        name: "nothing-here",
                        status: Some(Status::Mapped),
                        ..Mark::default()
                    },
                    at
                )
                .is_err()
        );
        let marked = registry
            .mark(
                &Mark {
                    name: "release-please-config-json",
                    status: Some(Status::Dropped),
                    reason: Some("release-plz replaces it"),
                    ..Mark::default()
                },
                at,
            )
            .expect("a drop with its reason records");
        assert_eq!(marked.subject, "source");
        assert_eq!(
            registry.sources[0]
                .history
                .last()
                .and_then(|entry| entry.reason.as_deref()),
            Some("release-plz replaces it")
        );
    }

    /// Two paths that slug alike take distinct ids, the later path in
    /// byte order taking the counter, whatever order they arrive in.
    #[test]
    fn colliding_source_paths_take_distinct_ids() {
        let mut registry = registry();
        for source in &mut registry.sources {
            source.path = source
                .path
                .replace("release-please-config.json", "goreleaser.yaml");
            source.path = source
                .path
                .replace(".release-please-manifest.json", ".goreleaser.yaml");
        }
        assign_ids(&mut registry.sources);
        let ids: Vec<(&str, &str)> = registry
            .sources
            .iter()
            .map(|source| (source.path.as_str(), source.id.as_str()))
            .collect();
        assert_eq!(
            ids,
            [
                ("goreleaser.yaml", "goreleaser-yaml-2"),
                (".goreleaser.yaml", "goreleaser-yaml"),
            ]
        );
    }

    /// A destination is judged under the target at close, so a path that
    /// leaves it refuses as a usage error and records nothing.
    #[test]
    fn a_destination_outside_the_target_refuses() {
        let at = "2026-10-09T15:00:00Z";
        for destination in ["/etc/release.toml", "../release.toml", "a/../../b"] {
            let mut registry = registry();
            registry
                .mark(
                    &Mark {
                        name: "release-please-config.json",
                        status: Some(Status::Mapped),
                        group: Some("release-please"),
                        ..Mark::default()
                    },
                    at,
                )
                .expect("the source files into a group");
            let before = registry.clone();
            let destinations = vec![destination.to_owned()];
            let error = registry
                .mark(
                    &Mark {
                        name: "release-please",
                        status: Some(Status::Written),
                        destinations: &destinations,
                        ..Mark::default()
                    },
                    at,
                )
                .expect_err("an escaping destination refuses");
            assert_eq!(error.exit_code(), 64, "{destination}");
            assert!(
                error.to_string().contains("leaves the target"),
                "{destination}: {error}"
            );
            assert_eq!(registry, before, "{destination}: nothing was recorded");
        }
    }

    /// A group whose sources were all dropped or filed elsewhere names
    /// destinations that bind nothing, so they hold no close back.
    #[test]
    fn a_group_with_no_live_source_does_not_block_close() {
        let target = tempfile::tempdir().expect("a scratch target exists");
        let target = camino::Utf8Path::from_path(target.path()).expect("UTF-8");
        let mut registry = registry();
        let at = "2026-10-09T15:00:00Z";
        let destinations = vec!["release-plz.toml".to_owned()];
        for (name, status) in [
            ("release-please-config.json", Status::Mapped),
            (".release-please-manifest.json", Status::Mapped),
        ] {
            registry
                .mark(
                    &Mark {
                        name,
                        status: Some(status),
                        group: Some("release-please"),
                        ..Mark::default()
                    },
                    at,
                )
                .expect("files into the group");
        }
        registry
            .mark(
                &Mark {
                    name: "release-please",
                    status: Some(Status::Written),
                    destinations: &destinations,
                    ..Mark::default()
                },
                at,
            )
            .expect("the group plans its destination");
        assert!(
            registry
                .unfinished(target)
                .iter()
                .any(|reason| reason.contains("release-plz.toml")),
            "a live group's absent destination blocks"
        );
        for name in [
            "release-please-config.json",
            ".release-please-manifest.json",
        ] {
            registry
                .mark(
                    &Mark {
                        name,
                        status: Some(Status::Dropped),
                        reason: Some("the convention replaces it"),
                        ..Mark::default()
                    },
                    at,
                )
                .expect("drops");
        }
        assert_eq!(registry.unfinished(target), Vec::<String>::new());
    }

    /// A destination that resolves out of the target through a symlink
    /// is not held, however its words read.
    #[test]
    fn a_destination_reached_through_a_symlink_out_of_the_target_does_not_count() {
        let dir = tempfile::tempdir().expect("a scratch target exists");
        let outside = tempfile::tempdir().expect("an outside directory exists");
        std::fs::write(outside.path().join("release.toml"), "").expect("writes");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("docs")).expect("links");
        let target = camino::Utf8Path::from_path(dir.path()).expect("UTF-8");
        let mut registry = registry();
        let at = "2026-10-09T15:00:00Z";
        registry
            .mark(
                &Mark {
                    name: "release-please-config.json",
                    status: Some(Status::Mapped),
                    group: Some("release-please"),
                    ..Mark::default()
                },
                at,
            )
            .expect("files into the group");
        let destinations = vec!["docs/release.toml".to_owned()];
        registry
            .mark(
                &Mark {
                    name: "release-please",
                    status: Some(Status::Written),
                    destinations: &destinations,
                    ..Mark::default()
                },
                at,
            )
            .expect("a lexically relative destination records");
        assert!(
            registry
                .unfinished(target)
                .iter()
                .any(|reason| reason.contains("docs/release.toml")),
            "{:?}",
            registry.unfinished(target)
        );
    }

    /// A destination that names a directory carries nothing a group
    /// wrote, so it is not held.
    #[test]
    fn a_directory_destination_does_not_count() {
        let dir = tempfile::tempdir().expect("a scratch target exists");
        std::fs::create_dir_all(dir.path().join(".github/workflows")).expect("mkdir");
        let target = camino::Utf8Path::from_path(dir.path()).expect("UTF-8");
        let mut registry = registry();
        let at = "2026-10-09T15:00:00Z";
        registry
            .mark(
                &Mark {
                    name: "release-please-config.json",
                    status: Some(Status::Mapped),
                    group: Some("release-please"),
                    ..Mark::default()
                },
                at,
            )
            .expect("files into the group");
        let destinations = vec![".github/workflows".to_owned()];
        registry
            .mark(
                &Mark {
                    name: "release-please",
                    status: Some(Status::Written),
                    destinations: &destinations,
                    ..Mark::default()
                },
                at,
            )
            .expect("the group records its destination");
        assert!(
            registry
                .unfinished(target)
                .iter()
                .any(|reason| reason.contains(".github/workflows")),
            "{:?}",
            registry.unfinished(target)
        );
    }

    #[test]
    fn the_registry_round_trips_through_yaml() {
        let registry = registry();
        let text = yaml_serde::to_string(&registry).expect("serializes");
        let back: Registry = yaml_serde::from_str(&text).expect("parses");
        assert_eq!(back, registry);
        assert!(text.contains("status: pending"), "{text}");
    }
}
