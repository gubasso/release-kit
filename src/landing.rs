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

pub mod invariants;
pub mod manifest;

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

pub use manifest::{Style, Workflow};

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
const KINDS: [(&str, Kind); 16] = [
    (".github/workflows/release-plz.yml", Kind::Rendered),
    (".github/workflows/release-please.yml", Kind::Rendered),
    (".github/workflows/release.yml", Kind::Rendered),
    (".github/workflows/pr-title.yml", Kind::Rendered),
    (".github/workflows/nix.yml", Kind::Rendered),
    (".gitlab-ci.yml", Kind::Rendered),
    (".gitlab/ci/mr-title.yml", Kind::Rendered),
    ("release-plz.toml", Kind::Seeded),
    ("dist-workspace.toml", Kind::Seeded),
    ("release-please-config.json", Kind::Seeded),
    ("cliff.toml", Kind::Seeded),
    ("nix/package.nix", Kind::Seeded),
    ("flake.nix", Kind::Seeded),
    (".release-please-manifest.json", Kind::State),
    ("VERSION", Kind::State),
    ("flake.lock", Kind::State),
];

/// The destinations of the opt-in Nix capability, present in a projection
/// only where the landing's `nix` parameter is on.
///
/// The parameter is recorded, so `status`, `upgrade`, and `adopt` can
/// reconstruct whether these files are supposed to exist: an absent file
/// under `nix = false` is not wanted, never drifted.
pub const NIX_DESTINATIONS: [&str; 4] = [
    "nix/package.nix",
    "flake.nix",
    "flake.lock",
    ".github/workflows/nix.yml",
];

/// The subset a target with a flake of its own keeps out: the seed pair,
/// and the workflow whose check would run against a flake release-kit did
/// not author.
///
/// The seeded package expression is not in it — it lands either way, as
/// the starting point the target integrates by hand.
pub const NIX_WITHHOLDABLE: [&str; 3] = ["flake.nix", "flake.lock", ".github/workflows/nix.yml"];

/// The declared kind of a destination, or `None` for a file the payload
/// does not classify.
#[must_use]
pub fn kind_of(destination: &str) -> Option<Kind> {
    if destination == AGENTS_DESTINATION || destination == HOOKS_DESTINATION {
        return Some(Kind::Rendered);
    }
    KINDS
        .iter()
        .find(|(name, _)| *name == destination)
        .map(|(_, kind)| *kind)
}

/// The mechanical substitution sites in `rendered` files.
///
/// Known values, substituted identically everywhere each appears. The
/// owner is derived from the landing's `repo` parameter and the two scope
/// forms from its `scopes` list, so the landed bytes stay a deterministic
/// function of payload plus parameters.
pub const OWNER_TOKEN: &[u8] = b"OWNER";

/// The scope list, comma-joined: hook arguments and prose.
pub const SCOPES_CSV_TOKEN: &[u8] = b"RK_SCOPES_CSV";

/// The scope list, pipe-joined: the title checks' regular expression.
pub const SCOPES_PIPE_TOKEN: &[u8] = b"RK_SCOPES_PIPE";

/// The recorded release style: `trunk` arms the bot's request in the
/// landed release workflow, `lines` leaves every request unarmed.
pub const STYLE_TOKEN: &[u8] = b"RK_STYLE";

/// Substitute the landing parameters into a `rendered` file's bytes.
///
/// The repository's owner — the project path's first segment — replaces
/// every `OWNER` occurrence, the scope list replaces the two scope
/// tokens, and the recorded style replaces the style token. An empty
/// scope list — or an unresolved style — leaves its tokens standing,
/// which only a preview renders under; an apply refuses before reaching
/// here.
#[must_use]
pub fn render(baseline: &[u8], repo: &str, scopes: &[String], style: Option<Style>) -> Vec<u8> {
    let owner = repo.split('/').next().unwrap_or(repo);
    let mut out = substitute(baseline, OWNER_TOKEN, owner.as_bytes());
    if let Some(style) = style {
        out = substitute(&out, STYLE_TOKEN, style.as_str().as_bytes());
    }
    if !scopes.is_empty() {
        out = substitute(&out, SCOPES_CSV_TOKEN, scopes.join(",").as_bytes());
        // The pipe form drops into an extended regular expression, where a
        // dot matches any character; among the characters `parse_scopes`
        // admits, the dot is the only special one, so `api.v1` escapes to
        // match itself alone.
        let pipe: Vec<String> = scopes.iter().map(|s| s.replace('.', "\\.")).collect();
        out = substitute(&out, SCOPES_PIPE_TOKEN, pipe.join("|").as_bytes());
    }
    out
}

/// Every `token` occurrence replaced with `value`.
fn substitute(baseline: &[u8], token: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(baseline.len());
    let mut rest = baseline;
    while let Some(at) = find(rest, token) {
        out.extend_from_slice(&rest[..at]);
        out.extend_from_slice(value);
        rest = &rest[at + token.len()..];
    }
    out.extend_from_slice(rest);
    out
}

/// The `--scopes` argument parsed into the recorded list.
///
/// Comma-separated, each scope non-empty and made of letters, digits, and
/// `_ . / -` — a set safe for the title checks' regular expression once
/// the renderer escapes the dot, the one special character among them.
///
/// # Errors
///
/// Returns [`RkError::Usage`] naming the offending scope, or the empty
/// list.
pub fn parse_scopes(raw: &str) -> Result<Vec<String>, RkError> {
    let scopes: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|scope| !scope.is_empty())
        .map(str::to_owned)
        .collect();
    if scopes.is_empty() {
        return Err(RkError::Usage(
            "--scopes names no scope; pass a comma-separated list, e.g. --scopes api,cli".into(),
        ));
    }
    for scope in &scopes {
        let clean = scope
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'));
        if !clean {
            return Err(RkError::Usage(format!(
                "the scope '{scope}' carries a character outside letters, digits, and _ . / -"
            )));
        }
    }
    Ok(scopes)
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

/// The destination the hook block splices into.
pub const HOOKS_DESTINATION: &str = ".pre-commit-config.yaml";

/// The hook block's opening marker, a YAML comment at column zero.
pub const HOOKS_BEGIN: &str = "# BEGIN release-kit";

/// The hook block's closing marker.
pub const HOOKS_END: &str = "# END release-kit";

/// The top-level key the fresh hook file carries and the skills verify on
/// an existing one: the commit-msg and pre-push hooks run only where their
/// hook types are installed.
pub const HOOK_TYPES_LINE: &str = "default_install_hook_types: [pre-commit, commit-msg, pre-push]";

/// The authored routing-block template, `blocks/agents-block.md.in`.
static AGENTS_BLOCK: &str = include_str!("../blocks/agents-block.md.in");

/// The routing block's mode line, worktree form.
static AGENTS_LINE_WORKTREE: &str = include_str!("../blocks/agents-line-worktree.md.in");

/// The routing block's mode line, branches form.
static AGENTS_LINE_BRANCHES: &str = include_str!("../blocks/agents-line-branches.md.in");

/// The authored hook-block template, `blocks/pre-commit-block.yaml.in`.
static PRE_COMMIT_BLOCK: &str = include_str!("../blocks/pre-commit-block.yaml.in");

/// The worktree mode's guard entry, `blocks/pre-commit-worktree-guard.yaml.in`.
static PRE_COMMIT_WORKTREE_GUARD: &str =
    include_str!("../blocks/pre-commit-worktree-guard.yaml.in");

/// An authored block without the one final newline the repository's
/// hooks enforce on every file under `blocks/`; a test in
/// `src/embedded.rs` holds each file to exactly one.
fn authored(text: &str) -> &str {
    text.strip_suffix('\n').unwrap_or(text)
}

/// The one branch grammar.
///
/// The extended regular expression the landed
/// `rk-branch-name` hook tests, and the same anchored language
/// `rk worktree add` validates before creating anything. One owner by
/// token — `concat!` cannot interpolate a const, so [`hooks_block`]
/// substitutes it for the template's `RK_BRANCH_GRAMMAR` token.
pub const BRANCH_GRAMMAR: &str = r"^((build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)/[A-Za-z0-9._/-]+|([0-9]+|[A-Z][A-Z0-9]+-[0-9]+)-[A-Za-z0-9._-]+|release[-/].+)$";

/// The routing block for one workflow mode: the whole of target-side
/// governance, authored as `blocks/agents-block.md.in` and never grown
/// into a method chapter.
///
/// Markers included, without a
/// trailing newline and with its scope token unrendered: the template
/// with the mode's one orientation line substituted, everything else —
/// the agent-boundary line included — byte-identical across modes.
#[must_use]
pub fn routing_block(workflow: Workflow) -> String {
    let line = match workflow {
        Workflow::Worktree => authored(AGENTS_LINE_WORKTREE),
        Workflow::Branches => authored(AGENTS_LINE_BRANCHES),
    };
    authored(AGENTS_BLOCK).replacen("RK_WORKFLOW_LINE", line, 1)
}

/// The hook block for one workflow mode, authored as
/// `blocks/pre-commit-block.yaml.in` with the worktree mode's guard entry
/// beside it in `blocks/pre-commit-worktree-guard.yaml.in`.
///
/// Markers included, without a
/// trailing newline and with its scope token unrendered. What is landed
/// is what runs: the worktree mode's block carries the location guard and
/// names the sweep-skip pair, and the branches mode's block carries no
/// guard entry at all — never an entry that reads local state to decide
/// whether to enforce. The one branch grammar substitutes here from
/// [`BRANCH_GRAMMAR`].
#[must_use]
pub fn hooks_block(workflow: Workflow) -> String {
    let (guard, skip) = match workflow {
        Workflow::Worktree => (
            format!("{}\n", authored(PRE_COMMIT_WORKTREE_GUARD)),
            "no-commit-to-branch,rk-worktree-location",
        ),
        Workflow::Branches => (String::new(), "no-commit-to-branch"),
    };
    authored(PRE_COMMIT_BLOCK)
        .replacen("RK_BRANCH_GRAMMAR", BRANCH_GRAMMAR, 1)
        .replacen("RK_SWEEP_SKIP", skip, 1)
        .replacen("RK_WORKTREE_GUARD", &guard, 1)
}

/// The markers of a block destination, or `None` for a whole-file one.
#[must_use]
pub fn block_markers(destination: &str) -> Option<(&'static str, &'static str)> {
    match destination {
        AGENTS_DESTINATION => Some((BLOCK_BEGIN, BLOCK_END)),
        HOOKS_DESTINATION => Some((HOOKS_BEGIN, HOOKS_END)),
        _ => None,
    }
}

/// The marked block inside a document, markers included, or `None` where
/// the text carries no complete block.
#[must_use]
pub fn extract_block<'a>(text: &'a str, begin: &str, end: &str) -> Option<&'a str> {
    let start = text.find(begin)?;
    let stop = text[start..].find(end)? + start + end.len();
    Some(&text[start..stop])
}

/// The whole `AGENTS.md` content after splicing the rendered block.
///
/// A fresh file where none exists, the block replaced in place where one
/// is marked, appended after the target's own content otherwise —
/// release-kit owns the lines inside the markers, not the document.
#[must_use]
pub fn splice_agents_block(existing: Option<&str>, block: &str) -> String {
    existing.map_or_else(
        || format!("{block}\n"),
        |text| {
            extract_block(text, BLOCK_BEGIN, BLOCK_END).map_or_else(
                || format!("{}\n\n{block}\n", text.trim_end()),
                |found| text.replacen(found, block, 1),
            )
        },
    )
}

/// The whole `.pre-commit-config.yaml` content after splicing the
/// rendered hook block.
///
/// A fresh file carries the hook-types key, the `repos:` key, and the
/// block; a marked file takes the block in place; an unmarked file takes
/// it directly under its `repos:` line, above the target's own hooks. An
/// unmarked file with no `repos:` line is refused by name — the block's
/// entries are list items and have nowhere honest to go.
///
/// # Errors
///
/// The reason the block has no place, for the caller's refusal to carry.
pub fn splice_hooks_block(existing: Option<&str>, block: &str) -> Result<String, String> {
    let Some(text) = existing else {
        return Ok(format!("{HOOK_TYPES_LINE}\n\nrepos:\n{block}\n"));
    };
    if let Some(defect) = hooks_marker_defect(text) {
        return Err(defect);
    }
    if let Some(found) = extract_block(text, HOOKS_BEGIN, HOOKS_END) {
        return Ok(text.replacen(found, block, 1));
    }
    let mut out = String::with_capacity(text.len() + block.len() + 1);
    let mut placed = false;
    for line in text.split_inclusive('\n') {
        out.push_str(line);
        if !placed && line.trim_end() == "repos:" {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(block);
            out.push('\n');
            placed = true;
        }
    }
    if placed {
        Ok(out)
    } else {
        Err(format!(
            "{HOOKS_DESTINATION} exists with no repos: line, so the hook block has nowhere to land"
        ))
    }
}

/// The one definition of an ill-formed hook file, shared by the splice
/// and every reader that judges one.
///
/// The hooks between the markers execute, so ownership must be
/// unambiguous: exactly one begin marker paired with exactly one end
/// marker after it, or none of either. A second begin is a second block
/// pre-commit would still run, and a marker without its pair — or an end
/// before its begin — is a block whose extent nothing can state.
#[must_use]
pub fn hooks_marker_defect(text: &str) -> Option<String> {
    let begins = text.matches(HOOKS_BEGIN).count();
    let ends = text.matches(HOOKS_END).count();
    if begins > 1 || ends > 1 {
        return Some(format!(
            "{HOOKS_DESTINATION} carries more than one release-kit marker pair; release-kit owns exactly one block"
        ));
    }
    match (text.find(HOOKS_BEGIN), text.find(HOOKS_END)) {
        (Some(begin), Some(end)) if end > begin => None,
        (None, None) => None,
        _ => Some(format!(
            "{HOOKS_DESTINATION} carries an unmatched or misordered release-kit marker, so the block's extent is ambiguous"
        )),
    }
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
    // The shared zone is not a technology: `_shared/<forge>` composes into
    // every pair and never names one.
    if tech.starts_with('_') || embedded::SNIPPETS.get_dir(tech).is_none() {
        let known: Vec<String> = embedded::SNIPPETS
            .dirs()
            .map(|dir| dir.path().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('_'))
            .collect();
        return Err(RkError::Usage(format!(
            "unknown tech '{tech}'; the bindings are: {}",
            known.join(", ")
        )));
    }
    let pair = format!("{tech}/{forge}");
    let pair_dir = embedded::SNIPPETS.get_dir(&pair).ok_or_else(|| {
        let known: Vec<String> = embedded::SNIPPETS
            .dirs()
            .filter(|dir| !dir.path().to_string_lossy().starts_with('_'))
            .flat_map(include_dir::Dir::dirs)
            .map(|dir| dir.path().to_string_lossy().replace('/', ", "))
            .collect();
        RkError::Usage(format!(
            "the pair ({tech}, {forge}) has no landable files; the supported pairs are: {}",
            known.join("; ")
        ))
    })?;
    // Payload paths carry their zone prefix; destinations do not. The
    // shared zone lands first, and a destination both zones ship is a
    // payload defect refused by name, never one zone silently winning.
    let mut files: Vec<(String, &'static [u8])> = Vec::new();
    let shared = format!("_shared/{forge}");
    if let Some(shared_dir) = embedded::SNIPPETS.get_dir(&shared) {
        for (path, contents) in embedded::walk(shared_dir) {
            let rel = path
                .strip_prefix(&format!("{shared}/"))
                .map_or(path.as_str(), |rel| rel)
                .to_owned();
            files.push((rel, contents));
        }
    }
    for (path, contents) in embedded::walk(pair_dir) {
        let rel = path
            .strip_prefix(&format!("{pair}/"))
            .map_or(path.as_str(), |rel| rel)
            .to_owned();
        if files.iter().any(|(existing, _)| *existing == rel) {
            return Err(anyhow::anyhow!(
                "the shared zone and the pair ({tech}, {forge}) both ship {rel}; the payload is defective"
            )
            .into());
        }
        files.push((rel, contents));
    }
    Ok(files)
}

/// The whole payload projection for one pair.
///
/// Under the `repo`, `scopes`, `workflow`,
/// `style`, and `nix` parameters: every snippet with its kind and
/// rendered bytes, plus the routing block and the hook block — each a
/// pure function of the recorded mode — sorted by destination. The Nix
/// destinations project only where `nix` is on; a pair that ships none of
/// them honestly projects the smaller product.
///
/// # Errors
///
/// Returns the [`pair_files`] errors, and [`RkError::Other`] for a
/// snippet destination the kind table does not classify, which is a
/// defect in this binary.
pub fn projection(
    tech: &str,
    forge: &str,
    repo: &str,
    scopes: &[String],
    workflow: Workflow,
    style: Option<Style>,
    nix: bool,
) -> Result<Vec<Entry>, RkError> {
    let mut entries = Vec::new();
    for (destination, baseline) in pair_files(tech, forge)? {
        if !nix && NIX_DESTINATIONS.contains(&destination.as_str()) {
            continue;
        }
        let kind = kind_of(&destination).ok_or_else(|| {
            anyhow::anyhow!("the payload does not classify {destination}; the kind table is stale")
        })?;
        let rendered = match kind {
            Kind::Rendered => render(baseline, repo, scopes, style),
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
    for (destination, template) in [
        (AGENTS_DESTINATION, routing_block(workflow)),
        (HOOKS_DESTINATION, hooks_block(workflow)),
    ] {
        entries.push(Entry {
            destination: destination.to_owned(),
            kind: Kind::Rendered,
            placement: Placement::Block,
            baseline: template.as_bytes().to_vec(),
            rendered: render(template.as_bytes(), repo, scopes, style),
        });
    }
    entries.sort_by(|a, b| a.destination.cmp(&b.destination));
    Ok(entries)
}

/// Why the whole Nix capability stays out of a landing, or `None` where
/// the target's crate shape supports the seed.
///
/// The seeded package expression reads the target's `Cargo.toml` through
/// `importTOML` and supports one crate with a `[package]` table; landing
/// it into a workspace root — or beside no `Cargo.toml` at all — seeds a
/// file that throws on its first evaluation, so the landing reports the
/// smaller product instead.
#[must_use]
pub fn nix_unsupported_shape(target: &Utf8Path) -> Option<String> {
    let Ok(text) = std::fs::read_to_string(target.join("Cargo.toml")) else {
        return Some(
            "the target has no readable Cargo.toml, which the seeded package expression reads; no Nix file lands".to_owned(),
        );
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Some(
            "the target's Cargo.toml does not parse, and the seeded package expression reads it; no Nix file lands".to_owned(),
        );
    };
    if table.contains_key("package") {
        None
    } else {
        Some(
            "the target's Cargo.toml has no [package] table; the seed supports a single crate, so no Nix file lands".to_owned(),
        )
    }
}

/// Why the flake half of the Nix capability stays out of this landing, or
/// `None` where the pair lands whole.
///
/// The pair is all-or-nothing: a target that already carries a
/// `flake.nix` or `flake.lock` of its own keeps its pair — a seed lock
/// beside a foreign flake describes the wrong input graph — and the
/// rendered workflow is withheld with it, because a green check that
/// never builds the landed expression proves nothing. A pair the record
/// names is release-kit's own landing and is never withheld.
///
/// # Errors
///
/// Any read failure other than the files being absent.
pub fn nix_withheld(
    target: &Utf8Path,
    recorded: Option<&manifest::Manifest>,
) -> std::io::Result<Option<String>> {
    if recorded.is_some_and(|record| record.file("flake.nix").is_some()) {
        return Ok(None);
    }
    let mut present = Vec::new();
    for name in ["flake.nix", "flake.lock"] {
        match std::fs::symlink_metadata(target.join(name).as_std_path()) {
            Ok(_) => present.push(name),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    if present.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "the target already carries {}; its flake pair stays its own, and the nix workflow is withheld with it",
        present.join(" and ")
    )))
}

/// One destination a landing withholds, with why.
#[derive(Debug, Serialize)]
pub struct Withheld {
    /// The destination that stays out.
    pub path: String,
    /// The reason, stated once per destination so a machine reader needs
    /// no join.
    pub reason: String,
}

/// Drop the Nix destinations this target cannot take from a projection,
/// naming each with its reason.
///
/// The one judgment every landing verb shares, so a preview, an apply, an
/// upgrade, and an adoption all withhold identically: an unsupported
/// crate shape withholds the whole capability, and a flake pair of the
/// target's own withholds the pair and the workflow while the seeded
/// package expression still lands.
///
/// # Errors
///
/// Any read failure from the pair check other than absence.
pub fn withhold_nix(
    target: &Utf8Path,
    nix: bool,
    recorded: Option<&manifest::Manifest>,
    entries: &mut Vec<Entry>,
) -> Result<Vec<Withheld>, RkError> {
    if !nix {
        return Ok(Vec::new());
    }
    let (set, reason): (&[&str], String) = if let Some(reason) = nix_unsupported_shape(target) {
        (&NIX_DESTINATIONS[..], reason)
    } else if let Some(reason) = nix_withheld(target, recorded)? {
        (&NIX_WITHHOLDABLE[..], reason)
    } else {
        return Ok(Vec::new());
    };
    let mut withheld = Vec::new();
    entries.retain(|entry| {
        if set.contains(&entry.destination.as_str()) {
            withheld.push(Withheld {
                path: entry.destination.clone(),
                reason: reason.clone(),
            });
            false
        } else {
            true
        }
    });
    Ok(withheld)
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

/// The bytes a recorded destination currently holds, by the placement
/// its name implies.
///
/// The marked block for `AGENTS.md` and `.pre-commit-config.yaml`, the
/// whole file otherwise. `None` means the file — or the block — is
/// absent.
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
    if let Some((begin, end)) = block_markers(destination) {
        let text = String::from_utf8_lossy(&bytes);
        Ok(extract_block(&text, begin, end).map(|block| block.as_bytes().to_vec()))
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
/// the block spliced into its document and the whole document rewritten
/// the same way.
///
/// # Errors
///
/// Any write failure; the destination then holds what it held. An
/// unspliceable hook file surfaces as an error here only as a backstop —
/// [`hooks_splice_refusal`] is the check a verb runs before any write.
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
            let block = String::from_utf8_lossy(&entry.rendered).into_owned();
            let spliced = if entry.destination == HOOKS_DESTINATION {
                splice_hooks_block(existing.as_deref(), &block).map_err(std::io::Error::other)?
            } else {
                splice_agents_block(existing.as_deref(), &block)
            };
            atomic::write(path.as_std_path(), spliced.as_bytes())
        }
    }
}

/// The hook file's defect, read from the target: `None` for a missing
/// file or one the block can land in.
///
/// The one judgment every verb shares, covering every splice refusal —
/// ill-formed markers, and an unmarked file offering the block no
/// `repos:` line. Status reports it as rendered drift, upgrade collects
/// it as a conflict in preview and apply alike so no landing dies
/// half-written, and adopt lists it with its mismatches.
///
/// # Errors
///
/// Any read failure other than the file being absent.
pub fn hooks_file_defect(target: &Utf8Path) -> std::io::Result<Option<String>> {
    let path = target.join(HOOKS_DESTINATION);
    match std::fs::read(&path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            Ok(splice_hooks_block(Some(&text), authored(PRE_COMMIT_BLOCK)).err())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// The refusal a landing verb answers before writing anything, where
/// the target's hook file offers the block no place.
///
/// Checked ahead of every write so the all-or-nothing property holds and
/// no landing dies half-written into `.pre-commit-config.yaml`.
///
/// # Errors
///
/// [`RkError::Refusal`] naming the file, and any read failure.
pub fn hooks_splice_refusal(target: &Utf8Path) -> Result<(), RkError> {
    hooks_file_defect(target)?.map_or(Ok(()), |reason| {
        Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!("{reason}, and nothing was written"),
            )
            .expected("a .pre-commit-config.yaml the block can land in, or none")
            .action(format!(
                "resolve it in {}, then re-run",
                target.join(HOOKS_DESTINATION)
            ))
            .target_state("unchanged"),
        ))
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        AGENTS_DESTINATION, BLOCK_BEGIN, BLOCK_END, BRANCH_GRAMMAR, HOOK_TYPES_LINE, HOOKS_BEGIN,
        HOOKS_DESTINATION, HOOKS_END, Kind, Style, Workflow, extract_block, hooks_block, kind_of,
        pair_files, parse_scopes, projection, render, routing_block, splice_agents_block,
        splice_hooks_block,
    };
    use crate::embedded;

    fn scopes(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    /// Every snippet destination has a declared kind: a new landable file
    /// without a classification fails here, not at a landing. The shared
    /// zone's files are enumerated the same way.
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
        assert_eq!(kind_of(HOOKS_DESTINATION), Some(Kind::Rendered));
        assert_eq!(kind_of("something-else.txt"), None);
    }

    /// Substitution is total and derives from the repo parameter's first
    /// segment, so a nested GitLab project path still yields its root
    /// namespace; the scope list renders in both joined forms.
    #[test]
    fn rendering_substitutes_every_owner_occurrence() {
        let baseline = b"if: repository_owner == 'OWNER'\n# OWNER again: OWNER\n";
        let rendered = render(baseline, "acme/sub/widget", &[], None);
        let text = String::from_utf8(rendered).expect("rendered bytes stay text");
        assert_eq!(text, "if: repository_owner == 'acme'\n# acme again: acme\n");

        let baseline = b"scopes 'RK_SCOPES_CSV' match (RK_SCOPES_PIPE)\n";
        let rendered = render(baseline, "acme/widget", &scopes(&["api", "cli"]), None);
        let text = String::from_utf8(rendered).expect("rendered bytes stay text");
        assert_eq!(text, "scopes 'api,cli' match (api|cli)\n");

        // A dot is the one admitted character that is special in the
        // regular expression: it escapes, so `api.v1` matches only itself.
        let rendered = render(baseline, "acme/widget", &scopes(&["api.v1"]), None);
        let text = String::from_utf8(rendered).expect("rendered bytes stay text");
        assert_eq!(text, "scopes 'api.v1' match (api\\.v1)\n");
    }

    /// The scope argument parses to the recorded list, refusing the empty
    /// list and any scope that would not drop into the title regex.
    #[test]
    fn scope_parsing_refuses_the_unusable() {
        assert_eq!(
            parse_scopes("api, cli,guides/release").expect("a clean list parses"),
            scopes(&["api", "cli", "guides/release"])
        );
        assert!(parse_scopes("").is_err());
        assert!(parse_scopes(" , ").is_err());
        assert!(parse_scopes("api|cli").is_err());
        assert!(parse_scopes("a b").is_err());
    }

    /// The shared zone composes into every pair, lands first, and is
    /// absent from the technology listing an unknown tech names.
    #[test]
    fn the_shared_zone_composes_into_the_pair() {
        let files = pair_files("rust", "github").expect("the pair lists");
        assert!(
            files
                .iter()
                .any(|(dest, _)| dest == ".github/workflows/pr-title.yml"),
            "the shared title check lands with the pair"
        );
        let files = pair_files("rust", "gitlab").expect("the pair lists");
        assert!(
            files
                .iter()
                .any(|(dest, _)| dest == ".gitlab/ci/mr-title.yml"),
            "the shared title job lands with the pair"
        );
        let err = pair_files("_shared", "github").expect_err("the shared zone is no tech");
        let listing = err.to_string();
        let bindings = listing
            .split("the bindings are:")
            .nth(1)
            .expect("the refusal lists the bindings");
        assert!(!bindings.contains("_shared"), "{listing}");
    }

    /// A rendered projection carries no unsubstituted token and no
    /// mechanical sentinel; the one judgment sentinel stays in its seeded
    /// file.
    #[test]
    fn a_projection_renders_owned_files_and_keeps_seeded_judgment() {
        let entries = projection(
            "rust",
            "github",
            "acme/widget",
            &scopes(&["api", "cli"]),
            Workflow::Branches,
            Some(Style::Trunk),
            false,
        )
        .expect("the pair projects");
        let workflow = entries
            .iter()
            .find(|entry| entry.destination.ends_with("release-plz.yml"))
            .expect("the workflow projects");
        assert_eq!(workflow.kind, Kind::Rendered);
        let text = String::from_utf8_lossy(&workflow.rendered);
        assert!(!text.contains("OWNER"), "an owner token survived rendering");
        assert!(text.contains("'acme'"));
        assert!(!text.contains("TODO(release-kit)"));
        let title = entries
            .iter()
            .find(|entry| entry.destination.ends_with("pr-title.yml"))
            .expect("the title check projects");
        let text = String::from_utf8_lossy(&title.rendered);
        assert!(text.contains("api|cli"), "{text}");
        assert!(
            !text.contains("RK_SCOPES"),
            "a scope token survived: {text}"
        );
        let seeded = entries
            .iter()
            .find(|entry| entry.destination == "release-plz.toml")
            .expect("the seeded file projects");
        assert_eq!(seeded.kind, Kind::Seeded);
        assert_eq!(seeded.rendered, seeded.baseline);
        assert!(String::from_utf8_lossy(&seeded.rendered).contains("TODO(release-kit)"));
        for block in [AGENTS_DESTINATION, HOOKS_DESTINATION] {
            let entry = entries
                .iter()
                .find(|entry| entry.destination == block)
                .expect("both blocks are part of the projection");
            let text = String::from_utf8_lossy(&entry.rendered);
            assert!(!text.contains("RK_SCOPES"), "{block} kept a token: {text}");
            assert!(text.contains("api,cli"), "{block} lost the scopes: {text}");
        }
    }

    /// The Nix destinations project only under the opt-in: off, none of
    /// them appears; on, the rust pairs carry them — the gitlab pair too,
    /// minus the workflow, which is a forge file the gitlab payload does
    /// not ship — and a pair without them projects the smaller product.
    #[test]
    fn the_nix_destinations_project_only_under_the_opt_in() {
        use super::NIX_DESTINATIONS;
        let paths = |nix: bool, forge: &str| -> Vec<String> {
            projection(
                "rust",
                forge,
                "acme/widget",
                &scopes(&["api"]),
                Workflow::Worktree,
                Some(Style::Trunk),
                nix,
            )
            .expect("the pair projects")
            .into_iter()
            .map(|entry| entry.destination)
            .collect()
        };
        let off = paths(false, "github");
        for destination in NIX_DESTINATIONS {
            assert!(!off.contains(&destination.to_owned()), "{destination}");
        }
        let on = paths(true, "github");
        for destination in ["nix/package.nix", "flake.nix", "flake.lock"] {
            assert!(on.contains(&destination.to_owned()), "{destination}");
        }
        let gitlab = paths(true, "gitlab");
        assert!(gitlab.contains(&"nix/package.nix".to_owned()));
        assert!(!gitlab.contains(&".github/workflows/nix.yml".to_owned()));
        let bash = projection(
            "bash",
            "github",
            "acme/widget",
            &scopes(&["api"]),
            Workflow::Worktree,
            Some(Style::Trunk),
            true,
        )
        .expect("an out-of-matrix pair projects the smaller product");
        assert!(
            bash.iter()
                .all(|entry| !NIX_DESTINATIONS.contains(&entry.destination.as_str()))
        );
    }

    /// The github and gitlab copies of the forge-independent Nix payload
    /// stay byte-identical: the loader composes exactly two layers and has
    /// no technology-wide zone, so the duplication is deliberate and this
    /// parity test is what keeps it honest.
    #[test]
    fn the_nix_seeds_are_identical_across_forge_pairs() {
        for name in ["nix/package.nix", "flake.nix", "flake.lock"] {
            let github = embedded::SNIPPETS
                .get_file(format!("rust/github/{name}"))
                .expect("the github copy ships")
                .contents();
            let gitlab = embedded::SNIPPETS
                .get_file(format!("rust/gitlab/{name}"))
                .expect("the gitlab copy ships")
                .contents();
            assert_eq!(github, gitlab, "{name} diverged between the pairs");
        }
    }

    /// The withhold judgment: a flake pair of the target's own withholds
    /// the pair and the workflow while the package expression lands, a
    /// crate shape the seed does not support withholds everything, and a
    /// clean single-crate target withholds nothing.
    #[test]
    fn the_nix_withhold_judgment_covers_the_three_shapes() {
        use super::{NIX_DESTINATIONS, withhold_nix};
        let dir = tempfile::tempdir().expect("a scratch target exists");
        let target = camino::Utf8Path::from_path(dir.path()).expect("utf-8 path");
        let entries = || {
            projection(
                "rust",
                "github",
                "acme/widget",
                &scopes(&["api"]),
                Workflow::Worktree,
                Some(Style::Trunk),
                true,
            )
            .expect("the pair projects")
        };

        // No Cargo.toml: the whole capability is withheld by name.
        let mut all = entries();
        let withheld = withhold_nix(target, true, None, &mut all).expect("the judgment runs");
        let paths: Vec<&str> = withheld.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                ".github/workflows/nix.yml",
                "flake.lock",
                "flake.nix",
                "nix/package.nix"
            ]
        );
        assert!(
            all.iter()
                .all(|entry| !NIX_DESTINATIONS.contains(&entry.destination.as_str()))
        );

        // A single crate with its own flake: the pair and the workflow are
        // withheld, and the package expression still lands.
        std::fs::write(
            target.join("Cargo.toml"),
            "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n",
        )
        .expect("the crate manifest writes");
        std::fs::write(target.join("flake.nix"), "{ }\n").expect("the flake writes");
        let mut all = entries();
        let withheld = withhold_nix(target, true, None, &mut all).expect("the judgment runs");
        let paths: Vec<&str> = withheld.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(
            paths,
            [".github/workflows/nix.yml", "flake.lock", "flake.nix"]
        );
        assert!(
            all.iter()
                .any(|entry| entry.destination == "nix/package.nix")
        );

        // A clean single crate: nothing is withheld.
        std::fs::remove_file(target.join("flake.nix")).expect("the flake removes");
        let mut all = entries();
        let withheld = withhold_nix(target, true, None, &mut all).expect("the judgment runs");
        assert!(withheld.is_empty());
        assert!(all.iter().any(|entry| entry.destination == "flake.nix"));

        // Off, the judgment does not even look.
        let mut all = entries();
        let withheld = withhold_nix(target, false, None, &mut all).expect("the judgment runs");
        assert!(withheld.is_empty());
    }

    #[test]
    fn the_block_splices_into_every_agents_shape() {
        let owned = routing_block(Workflow::Branches);
        let block = owned.as_str();
        let fresh = splice_agents_block(None, block);
        assert_eq!(fresh, format!("{block}\n"));
        assert_eq!(extract_block(&fresh, BLOCK_BEGIN, BLOCK_END), Some(block));

        let appended = splice_agents_block(Some("# My project\n\nOwn rules.\n"), block);
        assert!(appended.starts_with("# My project\n\nOwn rules.\n\n<!-- BEGIN release-kit -->"));
        assert_eq!(
            extract_block(&appended, BLOCK_BEGIN, BLOCK_END),
            Some(block)
        );

        let stale = appended.replace("Never author a tag", "Do author a tag");
        let refreshed = splice_agents_block(Some(&stale), block);
        assert_eq!(
            extract_block(&refreshed, BLOCK_BEGIN, BLOCK_END),
            Some(block)
        );
        assert!(refreshed.starts_with("# My project"));
        assert_eq!(
            refreshed.matches("BEGIN release-kit").count(),
            1,
            "a re-splice must replace, not accumulate"
        );
    }

    /// The hook block lands under `repos:` in every honest shape and
    /// refuses the one dishonest shape by name.
    #[test]
    fn the_hook_block_splices_under_repos() {
        let owned = hooks_block(Workflow::Branches);
        let block = owned.as_str();
        let fresh = splice_hooks_block(None, block).expect("a fresh file splices");
        assert!(fresh.starts_with(HOOK_TYPES_LINE));
        assert!(fresh.contains("\nrepos:\n# BEGIN release-kit\n"));
        assert_eq!(extract_block(&fresh, HOOKS_BEGIN, HOOKS_END), Some(block));

        let own =
            "repos:\n  - repo: https://example.com/own\n    rev: v1\n    hooks:\n      - id: own\n";
        let spliced = splice_hooks_block(Some(own), block).expect("an unmarked file splices");
        assert!(spliced.starts_with("repos:\n# BEGIN release-kit\n"));
        assert!(spliced.contains("- id: own"), "the target's hooks survive");
        assert!(
            !spliced.contains(HOOK_TYPES_LINE),
            "an existing file's top level is the skills' duty, not the splice's"
        );

        let stale = spliced.replace("--force-scope", "--no-scope");
        let refreshed = splice_hooks_block(Some(&stale), block).expect("a marked file re-splices");
        assert_eq!(
            extract_block(&refreshed, HOOKS_BEGIN, HOOKS_END),
            Some(block)
        );
        assert_eq!(refreshed.matches(HOOKS_BEGIN).count(), 1);

        let err = splice_hooks_block(Some("minimum_pre_commit_version: '3.2.0'\n"), block)
            .expect_err("no repos: line refuses");
        assert!(err.contains("repos:"), "{err}");

        // The hooks between the markers execute, so ownership is exactly
        // one well-formed block: a duplicate or an unmatched marker
        // refuses rather than leaving a stale block active.
        let doubled = format!("repos:\n{block}\n{block}\n");
        let err = splice_hooks_block(Some(&doubled), block).expect_err("a second block refuses");
        assert!(err.contains("one block"), "{err}");
        let unmatched = "repos:\n# BEGIN release-kit\n  - repo: local\n";
        let err =
            splice_hooks_block(Some(unmatched), block).expect_err("an unmatched marker refuses");
        assert!(err.contains("unmatched"), "{err}");
    }

    /// Both modes of both blocks: the guard entry and the skip pair exist
    /// exactly in the worktree mode, one orientation line differs in the
    /// routing block, the rest is byte-identical, no mode token survives
    /// substitution, and the rendered grammar is [`BRANCH_GRAMMAR`], the
    /// one owner.
    #[test]
    fn the_blocks_render_per_mode_and_carry_the_one_grammar() {
        let worktree_hooks = hooks_block(Workflow::Worktree);
        let branches_hooks = hooks_block(Workflow::Branches);
        assert!(worktree_hooks.contains("- id: rk-worktree-location"));
        assert!(
            worktree_hooks.contains("SKIP=no-commit-to-branch,rk-worktree-location"),
            "{worktree_hooks}"
        );
        assert!(!branches_hooks.contains("rk-worktree-location"));
        assert!(branches_hooks.contains("SKIP=no-commit-to-branch in"));
        for block in [&worktree_hooks, &branches_hooks] {
            assert!(block.contains(BRANCH_GRAMMAR), "the grammar has one owner");
            for token in ["RK_BRANCH_GRAMMAR", "RK_SWEEP_SKIP", "RK_WORKTREE_GUARD"] {
                assert!(!block.contains(token), "{token} survived: {block}");
            }
        }
        // A hook entry renders as a YAML plain scalar, where a colon
        // followed by a space ends the scalar and breaks the whole file
        // — the defect dogfood caught in the guard's refusal messages —
        // so no entry value may carry one.
        for block in [&worktree_hooks, &branches_hooks] {
            for line in block.lines() {
                if let Some(value) = line.trim_start().strip_prefix("entry: ") {
                    assert!(
                        !value.contains(": "),
                        "an entry value breaks the YAML plain scalar: {line}"
                    );
                }
            }
        }
        let guard_line = worktree_hooks
            .lines()
            .position(|line| line.contains("id: rk-worktree-location"))
            .expect("the guard entry exists");
        let name_line = worktree_hooks
            .lines()
            .position(|line| line.contains("id: rk-branch-name"))
            .expect("the name hook exists");
        assert!(
            guard_line > name_line,
            "the guard lands directly after rk-branch-name"
        );

        let worktree_routing = routing_block(Workflow::Worktree);
        let branches_routing = routing_block(Workflow::Branches);
        assert!(worktree_routing.contains("This project works in worktrees"));
        assert!(branches_routing.contains("Branches are worked in the main checkout"));
        for block in [&worktree_routing, &branches_routing] {
            assert!(block.contains("creating or removing a worktree"));
            assert!(block.contains("`rk worktree add <branch>`"));
            assert!(!block.contains("RK_WORKFLOW_LINE"), "{block}");
        }
        let differing: Vec<(&str, &str)> = worktree_routing
            .lines()
            .zip(branches_routing.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(
            differing.len(),
            1,
            "exactly one routing line differs per mode: {differing:?}"
        );
    }

    /// One definition of an ill-formed hook file, for every reader: the
    /// well-formed shapes pass and each ambiguous shape names a defect.
    #[test]
    fn the_hook_marker_defects_are_named() {
        use super::hooks_marker_defect;
        let owned = hooks_block(Workflow::Branches);
        let block = owned.as_str();
        assert_eq!(hooks_marker_defect(""), None);
        assert_eq!(hooks_marker_defect(&format!("repos:\n{block}\n")), None);
        for (case, text) in [
            (
                "a second begin",
                format!("repos:\n{block}\n# BEGIN release-kit\n"),
            ),
            (
                "a second end",
                format!("repos:\n{block}\n# END release-kit\n"),
            ),
            (
                "an unpaired begin",
                "repos:\n# BEGIN release-kit\n".to_owned(),
            ),
            ("an unpaired end", "repos:\n# END release-kit\n".to_owned()),
            (
                "an end before its begin",
                "repos:\n# END release-kit\n# BEGIN release-kit\n".to_owned(),
            ),
        ] {
            assert!(
                hooks_marker_defect(&text).is_some(),
                "{case} must be a defect"
            );
        }
    }
}
