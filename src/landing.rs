//! The target-side landing model: file kinds, parameter rendering, and
//! the routing block.
//!
//! Every landable file has a declared kind — `rendered` files release-kit
//! owns and may rewrite, `seeded` files the target tunes, `state` files
//! the release automation maintains — and a `rendered` file's bytes are a
//! deterministic function of the payload plus the landing parameters, so
//! a later command can compare what is on disk against what would be
//! written. The kinds are declared here, beside the payload, never
//! inferred at runtime; a test holds the table closed over every snippet.

pub mod manifest;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::{atomic, embedded};

/// Who owns a landed file's bytes after landing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// release-kit owns it: a newer payload re-renders it, and a target
    /// edit is a conflict.
    Rendered,
    /// The target owns it: a starting point the project tunes, reported
    /// and never rewritten.
    Seeded,
    /// The release automation owns it: never written after the first
    /// landing, never compared.
    State,
}

impl Kind {
    /// The wire and report form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rendered => "rendered",
            Self::Seeded => "seeded",
            Self::State => "state",
        }
    }
}

/// The declared classification: every landable destination and its kind.
/// The workflow and pipeline files carry the release automation and the
/// OIDC permission, so release-kit owns them; the tool configurations are
/// per-project judgment; the two state files are rewritten by the release
/// automation itself.
const KINDS: [(&str, Kind); 10] = [
    (".github/workflows/release-plz.yml", Kind::Rendered),
    (".github/workflows/release-please.yml", Kind::Rendered),
    (".github/workflows/release.yml", Kind::Rendered),
    (".gitlab-ci.yml", Kind::Rendered),
    ("release-plz.toml", Kind::Seeded),
    ("dist-workspace.toml", Kind::Seeded),
    ("release-please-config.json", Kind::Seeded),
    ("cliff.toml", Kind::Seeded),
    (".release-please-manifest.json", Kind::State),
    ("VERSION", Kind::State),
];

/// The declared kind of a destination, or `None` for a file the payload
/// does not classify.
#[must_use]
pub fn kind_of(destination: &str) -> Option<Kind> {
    if destination == AGENTS_DESTINATION {
        return Some(Kind::Rendered);
    }
    KINDS
        .iter()
        .find(|(name, _)| *name == destination)
        .map(|(_, kind)| *kind)
}

/// The mechanical substitution site in `rendered` files.
///
/// One known value, substituted identically everywhere it appears. The
/// owner is derived from the landing's `repo` parameter, so the landed
/// bytes stay a deterministic function of payload plus parameters.
pub const OWNER_TOKEN: &[u8] = b"OWNER";

/// Substitute the landing parameters into a `rendered` file's bytes: the
/// repository's owner — the project path's first segment — replaces every
/// `OWNER` occurrence.
#[must_use]
pub fn render(baseline: &[u8], repo: &str) -> Vec<u8> {
    let owner = repo.split('/').next().unwrap_or(repo).as_bytes();
    let mut out = Vec::with_capacity(baseline.len());
    let mut rest = baseline;
    while let Some(at) = find(rest, OWNER_TOKEN) {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(owner);
        rest = &rest[at + OWNER_TOKEN.len()..];
    }
    out.extend_from_slice(rest);
    out
}

/// First occurrence of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The destination the routing block splices into.
pub const AGENTS_DESTINATION: &str = "AGENTS.md";

/// The block's opening marker.
pub const BLOCK_BEGIN: &str = "<!-- BEGIN release-kit -->";

/// The block's closing marker.
pub const BLOCK_END: &str = "<!-- END release-kit -->";

/// The routing block: the whole of target-side governance. Four lines of
/// operational discovery — the files are owned, a convention governs
/// them, and where the convention lives — spliced into the target's
/// `AGENTS.md` and never grown into a method chapter.
const ROUTING_BLOCK: &str = "<!-- BEGIN release-kit -->

## Releases

- This repository runs the release-kit convention; `rk method invariants` states what must stay true.
- Never author a tag, and never hand-edit a generated artifact workflow.
- Run `rk status` before changing anything under `.github/workflows/` or `.gitlab-ci.yml`, or any file `.release-kit/manifest.json` names.
- The full method is `rk method --list`; the recovery paths are `rk method recovery`.

<!-- END release-kit -->";

/// The routing block, markers included, without a trailing newline.
#[must_use]
pub const fn routing_block() -> &'static str {
    ROUTING_BLOCK
}

/// The marked block inside a target's `AGENTS.md`, markers included, or
/// `None` where the file carries no complete block.
#[must_use]
pub fn extract_block(text: &str) -> Option<&str> {
    let start = text.find(BLOCK_BEGIN)?;
    let end = text[start..].find(BLOCK_END)? + start + BLOCK_END.len();
    Some(&text[start..end])
}

/// The whole `AGENTS.md` content after splicing the block.
///
/// A fresh file where none exists, the block replaced in place where one
/// is marked, appended after the target's own content otherwise —
/// release-kit owns the lines inside the markers, not the document.
#[must_use]
pub fn splice_block(existing: Option<&str>) -> String {
    existing.map_or_else(
        || format!("{ROUTING_BLOCK}\n"),
        |text| {
            extract_block(text).map_or_else(
                || format!("{}\n\n{ROUTING_BLOCK}\n", text.trim_end()),
                |found| text.replacen(found, ROUTING_BLOCK, 1),
            )
        },
    )
}

/// How a projected artifact occupies its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// The artifact is the whole file.
    Whole,
    /// The artifact is the marked block inside the target's `AGENTS.md`.
    Block,
}

/// One artifact of the payload projection: what would land at one
/// destination, with the payload bytes it was rendered from.
#[derive(Debug)]
pub struct Entry {
    /// The destination, relative to the target root.
    pub destination: String,
    /// The declared kind.
    pub kind: Kind,
    /// Whole file, or the marked block.
    pub placement: Placement,
    /// The payload bytes before substitution — what `baseline_sha256`
    /// digests.
    pub baseline: Vec<u8>,
    /// The bytes a landing writes: substituted for `rendered` files,
    /// identical to the baseline otherwise.
    pub rendered: Vec<u8>,
}

/// The landable files of one `(technology, forge)` pair, as
/// `(destination, payload bytes)`.
///
/// # Errors
///
/// Returns [`RkError::Usage`] naming the known bindings for an unknown
/// technology, and the supported pairs for a pair with no files.
pub fn pair_files(tech: &str, forge: &str) -> Result<Vec<(String, &'static [u8])>, RkError> {
    embedded::SNIPPETS.get_dir(tech).ok_or_else(|| {
        let known: Vec<String> = embedded::SNIPPETS
            .dirs()
            .map(|dir| dir.path().to_string_lossy().into_owned())
            .collect();
        RkError::Usage(format!(
            "unknown tech '{tech}'; the bindings are: {}",
            known.join(", ")
        ))
    })?;
    let pair = format!("{tech}/{forge}");
    let pair_dir = embedded::SNIPPETS.get_dir(&pair).ok_or_else(|| {
        let known: Vec<String> = embedded::SNIPPETS
            .dirs()
            .flat_map(include_dir::Dir::dirs)
            .map(|dir| dir.path().to_string_lossy().replace('/', ", "))
            .collect();
        RkError::Usage(format!(
            "the pair ({tech}, {forge}) has no landable files; the supported pairs are: {}",
            known.join("; ")
        ))
    })?;
    // Payload paths carry the `<tech>/<forge>/` prefix; destinations do not.
    Ok(embedded::walk(pair_dir)
        .into_iter()
        .map(|(path, contents)| {
            let rel = path
                .strip_prefix(&format!("{pair}/"))
                .map_or(path.as_str(), |rel| rel)
                .to_owned();
            (rel, contents)
        })
        .collect())
}

/// The whole payload projection for one pair under one `repo` parameter:
/// every snippet with its kind and rendered bytes, plus the routing
/// block, sorted by destination.
///
/// # Errors
///
/// Returns the [`pair_files`] errors, and [`RkError::Other`] for a
/// snippet destination the kind table does not classify, which is a
/// defect in this binary.
pub fn projection(tech: &str, forge: &str, repo: &str) -> Result<Vec<Entry>, RkError> {
    let mut entries = Vec::new();
    for (destination, baseline) in pair_files(tech, forge)? {
        let kind = kind_of(&destination).ok_or_else(|| {
            anyhow::anyhow!("the payload does not classify {destination}; the kind table is stale")
        })?;
        let rendered = match kind {
            Kind::Rendered => render(baseline, repo),
            Kind::Seeded | Kind::State => baseline.to_vec(),
        };
        entries.push(Entry {
            destination,
            kind,
            placement: Placement::Whole,
            baseline: baseline.to_vec(),
            rendered,
        });
    }
    entries.push(Entry {
        destination: AGENTS_DESTINATION.to_owned(),
        kind: Kind::Rendered,
        placement: Placement::Block,
        baseline: ROUTING_BLOCK.as_bytes().to_vec(),
        rendered: ROUTING_BLOCK.as_bytes().to_vec(),
    });
    entries.sort_by(|a, b| a.destination.cmp(&b.destination));
    Ok(entries)
}

/// The bytes an entry's destination currently holds: the whole file, or
/// the marked block extracted from the target's `AGENTS.md`. `None` means
/// the file — or the block — is absent.
///
/// # Errors
///
/// Any read failure other than the file being absent.
pub fn read_destination(target: &Utf8Path, entry: &Entry) -> std::io::Result<Option<Vec<u8>>> {
    read_recorded(target, &entry.destination)
}

/// The bytes a recorded destination currently holds, by the placement its
/// name implies: the marked block for `AGENTS.md`, the whole file
/// otherwise. `None` means the file — or the block — is absent.
///
/// # Errors
///
/// Any read failure other than the file being absent.
pub fn read_recorded(target: &Utf8Path, destination: &str) -> std::io::Result<Option<Vec<u8>>> {
    let path = target.join(destination);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if destination == AGENTS_DESTINATION {
        let text = String::from_utf8_lossy(&bytes);
        Ok(extract_block(&text).map(|block| block.as_bytes().to_vec()))
    } else {
        Ok(Some(bytes))
    }
}

/// What one detection pass resolved for a target-side verb, with the
/// override flags applied.
#[derive(Debug)]
pub struct Resolved {
    /// The forge whose payload applies.
    pub forge: String,
    /// The project path, where a flag or the remote names one.
    pub repo: Option<String>,
}

/// Resolve forge and repository in one pass: the flags override, the
/// `origin` remote answers otherwise.
///
/// An unrecognized host refuses rather than defaulting — landing one
/// forge's files into the other forge's project is a half-configured
/// repository that looks done.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for an unknown `--forge` value, and a
/// refusal naming the override when no forge resolves.
pub fn resolve(
    target: &Utf8Path,
    forge_flag: Option<&str>,
    repo_flag: Option<&str>,
) -> Result<Resolved, RkError> {
    let forge_flag = forge_flag
        .map(|name| {
            crate::detect::Forge::parse(name).ok_or_else(|| {
                RkError::Usage(format!(
                    "unknown forge '{name}'; the forges are: github, gitlab"
                ))
            })
        })
        .transpose()?;
    let detected = crate::detect::detect(target.as_std_path());
    let forge = forge_flag
        .or(detected.forge)
        .map(|forge| forge.as_str().to_owned())
        .ok_or_else(|| {
            let message = detected.host.map_or_else(
                || "no forge detected: the target has no origin remote".to_owned(),
                |host| format!("no forge detected: the host {host} is not recognized"),
            );
            RkError::refusal(
                Diagnostic::new(Reason::ForgeUndetected, message)
                    .expected("a github.com or gitlab remote, or --forge")
                    .action("pass --forge <github|gitlab>"),
            )
        })?;
    Ok(Resolved {
        forge,
        repo: repo_flag.map(str::to_owned).or(detected.repo),
    })
}

/// The refusal a verb answers when it needs the `repo` parameter and
/// neither a flag nor the remote supplies one.
#[must_use]
pub fn repo_unresolved() -> RkError {
    RkError::missing(
        Diagnostic::new(
            Reason::ForgeUndetected,
            "no repository detected: the target has no origin remote",
        )
        .expected("an origin remote naming the project")
        .action("pass --repo <path>"),
    )
}

/// Land one entry: the whole file through the temp-plus-rename writer, or
/// the block spliced into `AGENTS.md` and the whole document rewritten the
/// same way.
///
/// # Errors
///
/// Any write failure; the destination then holds what it held.
pub fn write_destination(target: &Utf8Path, entry: &Entry) -> std::io::Result<()> {
    let path = target.join(&entry.destination);
    match entry.placement {
        Placement::Whole => atomic::write(path.as_std_path(), &entry.rendered),
        Placement::Block => {
            let existing = match std::fs::read(&path) {
                Ok(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            let spliced = splice_block(existing.as_deref());
            atomic::write(path.as_std_path(), spliced.as_bytes())
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        AGENTS_DESTINATION, Kind, extract_block, kind_of, projection, render, routing_block,
        splice_block,
    };
    use crate::embedded;

    /// Every snippet destination has a declared kind: a new landable file
    /// without a classification fails here, not at a landing.
    #[test]
    fn the_kind_table_closes_over_every_snippet() {
        for tech_dir in embedded::SNIPPETS.dirs() {
            for pair_dir in tech_dir.dirs() {
                let prefix = format!("{}/", pair_dir.path().to_string_lossy());
                for (path, _) in embedded::walk(pair_dir) {
                    let destination = path.strip_prefix(&prefix).unwrap_or(&path);
                    assert!(
                        kind_of(destination).is_some(),
                        "{destination}: no declared kind"
                    );
                }
            }
        }
        assert_eq!(kind_of(AGENTS_DESTINATION), Some(Kind::Rendered));
        assert_eq!(kind_of("something-else.txt"), None);
    }

    /// Substitution is total and derives from the repo parameter's first
    /// segment, so a nested GitLab project path still yields its root
    /// namespace.
    #[test]
    fn rendering_substitutes_every_owner_occurrence() {
        let baseline = b"if: repository_owner == 'OWNER'\n# OWNER again: OWNER\n";
        let rendered = render(baseline, "acme/sub/widget");
        let text = String::from_utf8(rendered).expect("rendered bytes stay text");
        assert_eq!(text, "if: repository_owner == 'acme'\n# acme again: acme\n");
    }

    /// A rendered projection carries no unsubstituted token and no
    /// mechanical sentinel; the one judgment sentinel stays in its seeded
    /// file.
    #[test]
    fn a_projection_renders_owned_files_and_keeps_seeded_judgment() {
        let entries = projection("rust", "github", "acme/widget").expect("the pair projects");
        let workflow = entries
            .iter()
            .find(|entry| entry.destination.ends_with("release-plz.yml"))
            .expect("the workflow projects");
        assert_eq!(workflow.kind, Kind::Rendered);
        let text = String::from_utf8_lossy(&workflow.rendered);
        assert!(!text.contains("OWNER"), "an owner token survived rendering");
        assert!(text.contains("'acme'"));
        assert!(!text.contains("TODO(release-kit)"));
        let seeded = entries
            .iter()
            .find(|entry| entry.destination == "release-plz.toml")
            .expect("the seeded file projects");
        assert_eq!(seeded.kind, Kind::Seeded);
        assert_eq!(seeded.rendered, seeded.baseline);
        assert!(String::from_utf8_lossy(&seeded.rendered).contains("TODO(release-kit)"));
        assert!(
            entries
                .iter()
                .any(|entry| entry.destination == AGENTS_DESTINATION),
            "the routing block is part of the projection"
        );
    }

    #[test]
    fn the_block_splices_into_every_agents_shape() {
        let fresh = splice_block(None);
        assert_eq!(fresh, format!("{}\n", routing_block()));
        assert_eq!(extract_block(&fresh), Some(routing_block()));

        let appended = splice_block(Some("# My project\n\nOwn rules.\n"));
        assert!(appended.starts_with("# My project\n\nOwn rules.\n\n<!-- BEGIN release-kit -->"));
        assert_eq!(extract_block(&appended), Some(routing_block()));

        let stale = appended.replace("Never author a tag", "Do author a tag");
        let refreshed = splice_block(Some(&stale));
        assert_eq!(extract_block(&refreshed), Some(routing_block()));
        assert!(refreshed.starts_with("# My project"));
        assert_eq!(
            refreshed.matches("BEGIN release-kit").count(),
            1,
            "a re-splice must replace, not accumulate"
        );
    }
}
