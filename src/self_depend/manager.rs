//! The manager axis: which tool manager a target obtains `rk` through.
//!
//! One enum owns the list, and one detection reads which manager files
//! a target carries. `rk depend` reads the same files to land another
//! project, so both verbs share this module and a fifth manager is one
//! variant here plus its rows in each matrix. The per-manager pin reader
//! is pure text in, values out: it names whether a file mentions
//! release-kit, the version it pins where the manager records one, and
//! nothing it cannot read offline.

use camino::Utf8Path;
use clap::ValueEnum;
use serde::Serialize;

use super::Presence;
use super::discover::version_order;
use super::pin;
use crate::error::RkError;

/// A tool manager a project declares its development tools through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
#[value(rename_all = "kebab-case")]
pub enum Manager {
    /// A Nix flake: an input pinned at a tag and its package in the devshell.
    Flake,
    /// mise: one `[tools]` entry in its configuration file.
    Mise,
    /// asdf: one line in `.tool-versions`.
    Asdf,
    /// devbox: one entry in the `packages` array of `devbox.json`.
    Devbox,
}

/// The mise configuration paths, in mise's precedence order, the
/// highest first: a file, or a `conf.d` directory whose `*.toml` entries
/// all load. A local override file is left out: it is not committed.
pub const MISE_FILES: [&str; 9] = [
    "mise.toml",
    ".mise.toml",
    "mise/config.toml",
    "mise/conf.d",
    ".mise/config.toml",
    ".mise/conf.d",
    ".config/mise.toml",
    ".config/mise/config.toml",
    ".config/mise/conf.d",
];

impl Manager {
    /// The wire form.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flake => "flake",
            Self::Mise => "mise",
            Self::Asdf => "asdf",
            Self::Devbox => "devbox",
        }
    }

    /// Every manager, in the order the reports list them.
    pub const ALL: [Self; 4] = [Self::Flake, Self::Mise, Self::Asdf, Self::Devbox];

    /// The default file a manager is seeded into when absent.
    #[must_use]
    pub const fn default_file(self) -> &'static str {
        match self {
            Self::Flake => "flake.nix",
            Self::Mise => "mise.toml",
            Self::Asdf => ".tool-versions",
            Self::Devbox => "devbox.json",
        }
    }

    /// The paths a manager declares itself at, in precedence order.
    #[must_use]
    pub const fn candidates(self) -> &'static [&'static str] {
        match self {
            Self::Flake => &["flake.nix"],
            Self::Mise => &MISE_FILES,
            Self::Asdf => &[".tool-versions"],
            Self::Devbox => &["devbox.json"],
        }
    }
}

/// One manager and the file that declares it.
#[derive(Debug, Clone, Serialize)]
pub struct ManagerFile {
    /// The manager.
    pub manager: Manager,
    /// The file, relative to the target.
    pub file: String,
    /// Its text.
    #[serde(skip)]
    pub text: String,
}

/// The manager files present, one per manager, in the closed order.
///
/// For mise, the first path in precedence wins, and inside a `conf.d`
/// directory the file that already names `dep_name` wins over the first
/// in name order.
///
/// # Errors
///
/// Returns [`RkError::Io`] where a present file does not read.
pub fn manager_files(dir: &Utf8Path, dep_name: Option<&str>) -> Result<Vec<ManagerFile>, RkError> {
    let mut out = Vec::new();
    for manager in Manager::ALL {
        for candidate in manager.candidates() {
            let Some(file) = first_config(dir, candidate, dep_name)? else {
                continue;
            };
            out.push(ManagerFile {
                manager,
                text: std::fs::read_to_string(dir.join(&file))?,
                file,
            });
            break;
        }
    }
    Ok(out)
}

/// The configuration file a candidate path resolves to: the file
/// itself, or, under a `conf.d` directory, the first `*.toml` in name
/// order that already names the dependency, else the first in name order.
fn first_config(
    dir: &Utf8Path,
    candidate: &str,
    dep_name: Option<&str>,
) -> Result<Option<String>, RkError> {
    let path = dir.join(candidate);
    if path.is_file() {
        return Ok(Some(candidate.to_owned()));
    }
    if !candidate.ends_with("conf.d") || !path.is_dir() {
        return Ok(None);
    }
    let mut names: Vec<String> = std::fs::read_dir(&path)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "toml"))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    names.sort();
    if let Some(name) = dep_name {
        for file in &names {
            if first_mention(&std::fs::read_to_string(path.join(file))?, name).is_some() {
                return Ok(Some(format!("{candidate}/{file}")));
            }
        }
    }
    Ok(names.first().map(|name| format!("{candidate}/{name}")))
}

/// The first line naming `name` as a word, 1-based.
#[must_use]
pub fn first_mention(text: &str, name: &str) -> Option<usize> {
    text.lines()
        .position(|line| mentions(line, name))
        .map(|index| index + 1)
}

/// Whether a line names `name` as a whole word.
fn mentions(line: &str, name: &str) -> bool {
    let boundary = |c: Option<char>| {
        c.is_none_or(|c| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')))
    };
    line.match_indices(name).any(|(index, _)| {
        boundary(line[..index].chars().next_back())
            && boundary(line[index + name.len()..].chars().next())
    })
}

/// The name every manager file is read for.
pub const DEP_NAME: &str = "release-kit";

/// What one manager's file says about the release-kit pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinRead {
    /// No line names release-kit.
    Absent,
    /// One line names it with no version: a real state, reported and
    /// never rewritten.
    Unpinned {
        /// The one-based line.
        line: usize,
    },
    /// Exactly one line pins it.
    One {
        /// The one-based line.
        line: usize,
        /// The version as the manager records it: a `v` tag for the
        /// flake and devbox, a bare version for mise and asdf.
        version: String,
    },
    /// More than one line names it; the count is what a refusal names.
    Many {
        /// How many lines.
        count: usize,
    },
}

impl PinRead {
    /// The closed `pin` vocabulary.
    #[must_use]
    pub const fn word(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Unpinned { .. } => "unpinned",
            Self::One { .. } => "pinned",
            Self::Many { .. } => "ambiguous",
        }
    }

    /// Whether any line names release-kit.
    #[must_use]
    pub const fn names(&self) -> bool {
        !matches!(self, Self::Absent)
    }

    /// The version, where exactly one line pins it.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::One { version, .. } => Some(version),
            _ => None,
        }
    }

    /// How many lines name release-kit, where any does.
    #[must_use]
    pub const fn lines(&self) -> Option<usize> {
        match self {
            Self::Absent => None,
            Self::Unpinned { .. } | Self::One { .. } => Some(1),
            Self::Many { count } => Some(*count),
        }
    }

    /// Fold a list of `(line, version)` mentions to one read.
    fn fold(mentions: &[(usize, Option<String>)]) -> Self {
        match mentions {
            [] => Self::Absent,
            [(line, None)] => Self::Unpinned { line: *line },
            [(line, Some(version))] => Self::One {
                line: *line,
                version: version.clone(),
            },
            many => Self::Many { count: many.len() },
        }
    }
}

/// Read one manager's text for the release-kit pin.
#[must_use]
pub fn read_pin(manager: Manager, text: &str) -> PinRead {
    match manager {
        Manager::Flake => match pin::scan(text) {
            pin::Scan::None => PinRead::Absent,
            pin::Scan::Unpinned(line) => PinRead::Unpinned { line },
            pin::Scan::One(pin) => PinRead::One {
                line: pin.line,
                version: pin.tag,
            },
            pin::Scan::Many(count) => PinRead::Many { count },
        },
        Manager::Mise => PinRead::fold(&mise_mentions(text)),
        Manager::Asdf => PinRead::fold(&asdf_mentions(text)),
        Manager::Devbox => PinRead::fold(&devbox_mentions(text)),
    }
}

/// The double quote, as a code point: the source scan that keeps whole
/// artifacts out of the sources reads a quote literal as a string start.
const QUOTE: char = '\u{22}';

/// Every double-quoted value in a text, in order.
fn quoted_values(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(QUOTE) {
        let body = &rest[start + 1..];
        let Some(end) = body.find(QUOTE) else {
            break;
        };
        out.push(&body[..end]);
        rest = &body[end + 1..];
    }
    out
}

/// The first double-quoted value in a text.
fn quoted(text: &str) -> Option<&str> {
    quoted_values(text).into_iter().next()
}

/// A mise `[tools]` entry whose key names release-kit, in any backend:
/// `"cargo:release-kit"`, `"ubi:gubasso/release-kit"`, or the bare name.
/// The version is the quoted value, or the `version` field of a table.
fn mise_mentions(text: &str) -> Vec<(usize, Option<String>)> {
    let mut out = Vec::new();
    let mut in_tools = false;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_tools = line == "[tools]";
            continue;
        }
        if !in_tools || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches(QUOTE).trim_matches('\'');
        let named = key == DEP_NAME
            || key.ends_with(&format!(":{DEP_NAME}"))
            || key.ends_with(&format!("/{DEP_NAME}"));
        if !named {
            continue;
        }
        let value = value.trim();
        let version = if value.starts_with('{') {
            value
                .split_once("version")
                .and_then(|(_, rest)| quoted(rest))
                .map(str::to_owned)
        } else {
            quoted(value).map(str::to_owned)
        };
        let version = version.filter(|v| !v.is_empty() && v != "latest");
        out.push((index + 1, version));
    }
    out
}

/// An asdf line whose first word is `release-kit`; the second word is
/// the version.
fn asdf_mentions(text: &str) -> Vec<(usize, Option<String>)> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let mut words = line.split('#').next().unwrap_or("").split_whitespace();
            (words.next() == Some(DEP_NAME)).then(|| {
                let version = words
                    .next()
                    .filter(|word| *word != "latest")
                    .map(str::to_owned);
                (index + 1, version)
            })
        })
        .collect()
}

/// A devbox package entry naming release-kit: a flake reference
/// `github:gubasso/release-kit/<tag>#default` or a name `release-kit@<version>`
/// in the `packages` array, or a `release-kit` key in the `packages` object
/// whose value is the version.
fn devbox_mentions(text: &str) -> Vec<(usize, Option<String>)> {
    let mut out = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let values = quoted_values(raw.trim());
        let Some(at) = values.iter().position(|value| mentions(value, DEP_NAME)) else {
            continue;
        };
        let value = values[at];
        let version = if let Some((_, rest)) = value.split_once(&format!("{DEP_NAME}/")) {
            rest.split('#').next().filter(|v| !v.is_empty())
        } else if let Some((_, rest)) = value.split_once(&format!("{DEP_NAME}@")) {
            (!rest.is_empty()).then_some(rest)
        } else if value == DEP_NAME {
            values.get(at + 1).copied()
        } else {
            None
        };
        let version = version.filter(|v| *v != "latest").map(str::to_owned);
        out.push((index + 1, version));
    }
    out
}

/// The offline freshness of a pinned version against this binary's own:
/// a fact the report carries, never a judgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Freshness {
    /// The pin is older than this binary.
    Behind,
    /// The pin names this binary's version.
    Current,
    /// The pin is newer than this binary.
    Ahead,
}

impl Freshness {
    /// Compare a pinned version with this binary's.
    #[must_use]
    pub fn of(version: &str) -> Self {
        match version_order(version, env!("CARGO_PKG_VERSION")) {
            std::cmp::Ordering::Less => Self::Behind,
            std::cmp::Ordering::Equal => Self::Current,
            std::cmp::Ordering::Greater => Self::Ahead,
        }
    }
}

/// One manager's entry in the status report: what its file says.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// The manager.
    pub manager: Manager,
    /// Whether the manager's file exists.
    pub present: Presence,
    /// The file, relative to the target, where present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// `pinned`, `unpinned`, `absent`, or `ambiguous`.
    pub pin: &'static str,
    /// The pinned version as the manager records it, where exactly one
    /// line pins it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// How many lines name release-kit, where any does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_lines: Option<usize>,
    /// The pin against this binary's version, where one is pinned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness: Option<Freshness>,
    /// Whether `flake.lock` exists; the flake manager alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock: Option<Presence>,
    /// The locked ref of the input, where the lock names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_ref: Option<String>,
    /// The locked commit of the input, where the lock names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_rev: Option<String>,
    /// What the file says, kept for the verbs that act on it.
    #[serde(skip)]
    pub read: PinRead,
    /// The file's text, kept for the fragments judged against it.
    #[serde(skip)]
    pub text: Option<String>,
}

impl Entry {
    /// An entry for a manager whose file is absent.
    #[must_use]
    pub const fn absent(manager: Manager) -> Self {
        Self {
            manager,
            present: Presence::Absent,
            file: None,
            pin: "absent",
            version: None,
            pin_lines: None,
            freshness: None,
            lock: None,
            locked_ref: None,
            locked_rev: None,
            read: PinRead::Absent,
            text: None,
        }
    }

    /// An entry read from a present file.
    #[must_use]
    pub fn read(file: &ManagerFile) -> Self {
        let read = read_pin(file.manager, &file.text);
        Self {
            manager: file.manager,
            present: Presence::Present,
            file: Some(file.file.clone()),
            pin: read.word(),
            version: read.version().map(str::to_owned),
            pin_lines: read.lines(),
            freshness: read.version().map(Freshness::of),
            lock: None,
            locked_ref: None,
            locked_rev: None,
            read,
            text: Some(file.text.clone()),
        }
    }
}

impl Manager {
    /// Whether the manager records the `v` tag rather than the bare
    /// version: the flake and devbox pin a flake reference at a tag, mise
    /// and asdf record the version a registry names.
    #[must_use]
    pub const fn records_tag(self) -> bool {
        matches!(self, Self::Flake | Self::Devbox)
    }

    /// The version as this manager records it, from a tag with or
    /// without its `v`.
    #[must_use]
    pub fn recorded(self, tag: &str) -> String {
        let bare = tag.strip_prefix('v').unwrap_or(tag);
        if self.records_tag() {
            format!("v{bare}")
        } else {
            bare.to_owned()
        }
    }
}

/// The same text with one line's pinned version replaced: the first
/// occurrence of `from` after the release-kit mention on that line. Only
/// those bytes change; every other byte of the file survives.
#[must_use]
pub fn rewrite_line(text: &str, line: usize, from: &str, to: &str) -> String {
    let mut out = String::with_capacity(text.len() + to.len());
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        if index + 1 != line {
            out.push_str(raw);
            continue;
        }
        let mention = raw.find(DEP_NAME).map_or(0, |at| at + DEP_NAME.len());
        match raw[mention..].find(from) {
            Some(at) => {
                let start = mention + at;
                out.push_str(&raw[..start]);
                out.push_str(to);
                out.push_str(&raw[start + from.len()..]);
            }
            None => out.push_str(raw),
        }
    }
    out
}

/// One entry per manager, in `ALL` order, absent ones included: an
/// absent manager is a fact, never a fault.
#[must_use]
pub fn entries(files: &[ManagerFile]) -> Vec<Entry> {
    Manager::ALL
        .into_iter()
        .map(|manager| {
            files
                .iter()
                .find(|file| file.manager == manager)
                .map_or_else(|| Entry::absent(manager), Entry::read)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Freshness, MISE_FILES, Manager, PinRead, first_mention, manager_files, read_pin};

    #[test]
    fn the_first_mise_file_in_precedence_wins() {
        let dir = tempfile::tempdir().expect("a scratch dir");
        let root = camino::Utf8Path::from_path(dir.path()).expect("utf-8");
        std::fs::create_dir_all(root.join(".config/mise")).expect("mkdir");
        std::fs::write(root.join(".config/mise/config.toml"), "[tools]\n").expect("writes");
        std::fs::write(root.join(".mise.toml"), "[tools]\nnode = '24'\n").expect("writes");
        std::fs::write(root.join("devbox.json"), "{}\n").expect("writes");
        let files = manager_files(root, None).expect("reads");
        let names: Vec<(Manager, &str)> =
            files.iter().map(|m| (m.manager, m.file.as_str())).collect();
        assert_eq!(
            names,
            [
                (Manager::Mise, ".mise.toml"),
                (Manager::Devbox, "devbox.json")
            ]
        );
        assert_eq!(MISE_FILES[0], "mise.toml");
        std::fs::remove_file(root.join(".mise.toml")).expect("removes");
        std::fs::remove_file(root.join(".config/mise/config.toml")).expect("removes");
        std::fs::create_dir_all(root.join(".mise/conf.d")).expect("mkdir");
        std::fs::write(root.join(".mise/conf.d/tools.toml"), "[tools]\n").expect("writes");
        std::fs::write(root.join(".mise/conf.d/env.toml"), "[env]\n").expect("writes");
        std::fs::write(root.join(".mise/conf.d/README"), "").expect("writes");
        let files = manager_files(root, None).expect("reads");
        assert_eq!(
            files[0].file, ".mise/conf.d/env.toml",
            "a conf.d directory is mise ownership, its first toml in name order"
        );
        std::fs::write(
            root.join(".mise/conf.d/tools.toml"),
            "[tools]\n\"cargo:sample-tool\" = \"1.0.0\"\n",
        )
        .expect("writes");
        let files = manager_files(root, Some("sample-tool")).expect("reads");
        assert_eq!(
            files[0].file, ".mise/conf.d/tools.toml",
            "the file that already names the dependency is the destination"
        );
    }

    #[test]
    fn a_mention_is_found_by_line() {
        let text = "[tools]\nnode = '24'\n\"cargo:sample-tool\" = \"1.4.0\"\n";
        assert_eq!(first_mention(text, "sample-tool"), Some(3));
        assert_eq!(
            first_mention(text, "sample"),
            None,
            "a prefix is not a name"
        );
        assert_eq!(first_mention("", "sample-tool"), None);
    }

    #[test]
    fn every_manager_has_its_files_and_the_enum_is_closed() {
        for manager in Manager::ALL {
            assert_eq!(manager.candidates()[0], manager.default_file());
        }
        assert_eq!(Manager::ALL.len(), 4);
    }

    #[test]
    fn the_mise_reader_names_the_tool_in_any_backend() {
        let text = "[env]\nFOO = \"release-kit\"\n[tools]\nnode = \"24\"\n\"cargo:release-kit\" = \"0.3.18\"\n";
        assert_eq!(
            read_pin(Manager::Mise, text),
            PinRead::One {
                line: 5,
                version: "0.3.18".to_owned()
            }
        );
        let table =
            "[tools]\n\"ubi:gubasso/release-kit\" = { version = \"0.3.17\", exe = \"rk\" }\n";
        assert_eq!(read_pin(Manager::Mise, table).version(), Some("0.3.17"));
        assert_eq!(
            read_pin(Manager::Mise, "[tools]\nrelease-kit = \"latest\"\n"),
            PinRead::Unpinned { line: 2 }
        );
        assert_eq!(
            read_pin(
                Manager::Mise,
                "[tools]\n\"cargo:release-kit\" = \"0.3.18\"\n\"ubi:gubasso/release-kit\" = \"0.3.18\"\n"
            ),
            PinRead::Many { count: 2 }
        );
        assert_eq!(
            read_pin(Manager::Mise, "[tools]\nnode = \"24\"\n"),
            PinRead::Absent
        );
        assert_eq!(
            read_pin(
                Manager::Mise,
                "[tools]\n\"cargo:release-kit-extra\" = \"1\"\n"
            ),
            PinRead::Absent,
            "a longer name is not this one"
        );
    }

    #[test]
    fn the_asdf_reader_takes_the_second_word() {
        assert_eq!(
            read_pin(Manager::Asdf, "nodejs 24.0.0\nrelease-kit 0.3.18 # rk\n"),
            PinRead::One {
                line: 2,
                version: "0.3.18".to_owned()
            }
        );
        assert_eq!(
            read_pin(Manager::Asdf, "release-kit\n"),
            PinRead::Unpinned { line: 1 }
        );
        assert_eq!(
            read_pin(Manager::Asdf, "# release-kit 1\n"),
            PinRead::Absent
        );
    }

    #[test]
    fn the_devbox_reader_takes_the_flake_tag_or_the_version_suffix() {
        let reference =
            "{\n  \"packages\": [\n    \"github:gubasso/release-kit/v0.3.18#default\"\n  ]\n}\n";
        assert_eq!(
            read_pin(Manager::Devbox, reference),
            PinRead::One {
                line: 3,
                version: "v0.3.18".to_owned()
            }
        );
        assert_eq!(
            read_pin(Manager::Devbox, "{\"packages\": [\"release-kit@0.3.18\"]}"),
            PinRead::One {
                line: 1,
                version: "0.3.18".to_owned()
            }
        );
        assert_eq!(
            read_pin(
                Manager::Devbox,
                "{\"packages\": {\"release-kit\": \"0.3.18\"}}"
            )
            .version(),
            Some("0.3.18")
        );
        assert_eq!(
            read_pin(Manager::Devbox, "{\"packages\": [\"release-kit@latest\"]}"),
            PinRead::Unpinned { line: 1 }
        );
        assert_eq!(
            read_pin(Manager::Devbox, "{\"packages\": [\"nodejs@24\"]}"),
            PinRead::Absent
        );
    }

    #[test]
    fn the_flake_reader_is_the_pin_matcher() {
        let flake = "{\n  url = \"github:gubasso/release-kit/v0.2.16\";\n}\n";
        assert_eq!(
            read_pin(Manager::Flake, flake),
            PinRead::One {
                line: 2,
                version: "v0.2.16".to_owned()
            }
        );
    }

    #[test]
    fn a_one_fact_line_is_rewritten_in_place() {
        let mise = "[tools]\nnode = \"24\"\n\"cargo:release-kit\" = \"0.2.15\" # 0.2.15 was fine\n";
        assert_eq!(
            super::rewrite_line(mise, 3, "0.2.15", "0.2.16"),
            "[tools]\nnode = \"24\"\n\"cargo:release-kit\" = \"0.2.16\" # 0.2.15 was fine\n",
            "only the first occurrence after the name moves"
        );
        let devbox =
            "{\r\n  \"packages\": [\"github:gubasso/release-kit/v0.2.15#default\"]\r\n}\r\n";
        assert_eq!(
            super::rewrite_line(devbox, 2, "v0.2.15", "v0.2.16"),
            devbox.replace("v0.2.15", "v0.2.16")
        );
        assert_eq!(
            super::rewrite_line(mise, 9, "0.2.15", "0.2.16"),
            mise,
            "a line the file lacks changes nothing"
        );
        assert_eq!(Manager::Mise.recorded("v0.2.16"), "0.2.16");
        assert_eq!(Manager::Devbox.recorded("0.2.16"), "v0.2.16");
        assert!(Manager::Flake.records_tag());
        assert!(!Manager::Asdf.records_tag());
    }

    #[test]
    fn freshness_compares_with_the_binary() {
        assert_eq!(Freshness::of("v0.0.1"), Freshness::Behind);
        assert_eq!(Freshness::of(env!("CARGO_PKG_VERSION")), Freshness::Current);
        assert_eq!(
            Freshness::of(&format!("v{}", env!("CARGO_PKG_VERSION"))),
            Freshness::Current
        );
        assert_eq!(Freshness::of("v999.0.0"), Freshness::Ahead);
    }
}
