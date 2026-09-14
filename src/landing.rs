//! The target-side landing model on the release seam: parameter
//! resolution, the seam-based projection, and the target writes.
//!
//! Every landable file has a declared kind, `rendered` files release-kit
//! owns and may rewrite, `seeded` files the target tunes, `state` files
//! the release automation maintains, and a `rendered` file's bytes are a
//! deterministic function of the payload plus the landing parameters, so
//! a later command can compare what is on disk against what would be
//! written.
//!
//! The pure pieces of that model, the kind table, the token rendering,
//! the block templating, the splice and marker judgments, the pair
//! selection, and the Nix crate-shape judgment, have one implementation
//! in [`crate::projection`] and are re-exported here under their old
//! names. What stays in this file is the path that reads a release bundle
//! through the seam ([`projection`] over a [`ReleaseSource`]), which the
//! planner and `--to` still need until a later phase deletes it, and the
//! functions that read or write a target.

pub mod invariants;
pub mod manifest;

use camino::Utf8Path;

pub use crate::projection::{
    AGENTS_DESTINATION, BLOCK_BEGIN, BLOCK_DESTINATIONS, BLOCK_END, BRANCH_GRAMMAR,
    GLOSSARY_DESTINATION, HOOK_TYPES_LINE, HOOKS_BEGIN, HOOKS_DESTINATION, HOOKS_END, Kind,
    LINE_PREFIX_RE_TOKEN, LINE_PREFIX_TOKEN, NIX_DESTINATIONS, NIX_WITHHOLDABLE, OWNER_TOKEN,
    REPO_PLACEHOLDER, REPO_TOKEN, SCOPE_SHAPE, SCOPE_SHAPE_TOKEN, SECURITY_SPANS, STYLE_TOKEN,
    TRUNK_BRANCH_TOKEN, authored, block_markers, destinations, extract_block, hooks_marker_defect,
    kind_of, marker_defect, render, scope_is_shaped, splice_hooks_block, splice_marked_block,
    substitute,
};
pub use manifest::{Style, Workflow};
use serde::Serialize;

use crate::atomic;
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::projection::{self as pure, evidence};
use crate::release::{self, ReleaseManifest, ReleaseSource};

/// The complete input to a payload projection. Comparisons reconstruct it
/// from the landing record; landing verbs resolve their candidate inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Params {
    tech: String,
    forge: String,
    repo: String,
    workflow: Workflow,
    style: Option<Style>,
    nix: bool,
    trunk: String,
    line_prefix: String,
    security_contact: String,
    security_response: String,
}

/// Explicit invocation answers; absence falls through to configuration.
#[derive(Default)]
pub struct Inputs<'a> {
    /// Binding override.
    pub tech: Option<&'a str>,
    /// Forge override.
    pub forge: Option<&'a str>,
    /// Repository override.
    pub repo: Option<&'a str>,
    /// Workflow override.
    pub workflow: Option<Workflow>,
    /// Release style override.
    pub style: Option<Style>,
    /// Nix capability override.
    pub nix: Option<bool>,
}

/// Compatibility policy for a landing candidate.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    /// A first landing.
    Init,
    /// A preview may leave the repository unresolved.
    Preview,
    /// An existing record supplies compatibility answers.
    Upgrade,
    /// A pre-record target requires an explicit release style.
    Adopt,
}

impl Params {
    /// Reconstruct every projection parameter from the record alone,
    /// including the compatibility defaults applied when it was loaded.
    #[must_use]
    pub fn from_record(record: &manifest::Manifest) -> Self {
        Self {
            tech: record.tech.clone(),
            forge: record.forge.clone(),
            repo: record.parameters.repo.clone(),
            workflow: record.parameters.workflow,
            style: record.parameters.style,
            nix: record.parameters.nix,
            trunk: record.parameters.trunk.clone(),
            line_prefix: record.parameters.line_prefix.clone(),
            security_contact: record.parameters.security_contact.clone(),
            security_response: record.parameters.security_response.clone(),
        }
    }

    /// Resolve flags, configuration, recorded compatibility inputs or detection,
    /// and finally the compiled defaults. Comparisons use `from_record` alone.
    ///
    /// # Errors
    /// Refuses unresolved identity or a style an existing target has not answered.
    pub fn resolve(
        source: &dyn ReleaseSource,
        target: &Utf8Path,
        flags: &Inputs<'_>,
        config: Option<&crate::config::Config>,
        record: Option<&manifest::Manifest>,
        purpose: Purpose,
    ) -> Result<Self, RkError> {
        let answer = |flag: Option<&str>, configured: Option<&str>, recorded: Option<&str>| {
            flag.or_else(|| configured.filter(|value| !value.is_empty()))
                .or(recorded)
                .map(str::to_owned)
        };
        let forge = answer(
            flags.forge,
            config.map(|c| c.project.forge.as_str()),
            record.map(|r| r.forge.as_str()),
        );
        let repo = answer(
            flags.repo,
            config.map(|c| c.project.repo.as_str()),
            record.map(|r| r.parameters.repo.as_str()),
        );
        let resolved = resolve(target, forge.as_deref(), repo.as_deref())?;
        let tech = answer(
            flags.tech,
            config.map(|c| c.project.tech.as_str()),
            record.map(|r| r.tech.as_str()),
        )
        .or_else(|| crate::detect::tech_of(target.as_std_path()).map(str::to_owned))
        .ok_or_else(|| {
            RkError::missing(
                Diagnostic::new(
                    Reason::TargetNotFound,
                    "no technology detected: the target has no version file",
                )
                .action("pass --tech <rust|python|bash>"),
            )
        })?;
        pair_files(source, &tech, &resolved.forge)?;
        let workflow = flags
            .workflow
            .or_else(|| config.and_then(|c| c.landing.workflow))
            .or_else(|| record.map(|r| r.parameters.workflow))
            .unwrap_or(if purpose == Purpose::Adopt {
                Workflow::Branches
            } else {
                Workflow::Worktree
            });
        let style = flags
            .style
            .or_else(|| config.and_then(|c| c.landing.style))
            .or_else(|| record.and_then(|r| r.parameters.style));
        let style = match (style, purpose) {
            (None, Purpose::Upgrade | Purpose::Adopt) => return Err(RkError::Usage("the target carries no style parameter; set landing.style in .release-kit/config.toml or pass --style <trunk|lines>".into())),
            (value, _) => Some(value.unwrap_or(Style::Trunk)),
        };
        let repo = resolved
            .repo
            .or_else(|| (purpose == Purpose::Preview).then(|| REPO_PLACEHOLDER.to_owned()))
            .ok_or_else(repo_unresolved)?;
        let trunk = config
            .and_then(|c| c.project.trunk.clone())
            .or_else(|| record.map(|r| r.parameters.trunk.clone()))
            .unwrap_or_else(|| crate::config::TRUNK_DEFAULT.to_owned());
        let line_prefix = config
            .and_then(|c| c.setup.line_prefix.clone())
            .or_else(|| record.map(|r| r.parameters.line_prefix.clone()))
            .unwrap_or_else(|| crate::config::LINE_PREFIX_DEFAULT.to_owned());
        // An explicitly present key wins, including an empty contact,
        // which is how a target resets a recorded custom contact. An
        // omitted key falls through to the record, so an upgrade under an
        // older configuration keeps the policy the target already carries.
        let security_contact = config
            .and_then(|c| c.security.contact.clone())
            .or_else(|| record.map(|r| r.parameters.security_contact.clone()))
            .unwrap_or_default();
        let security_contact =
            crate::config::canonical_contact(&security_contact).map_err(crate::config::invalid)?;
        let security_response = config
            .and_then(|c| c.security.response.clone())
            .or_else(|| record.map(|r| r.parameters.security_response.clone()))
            .unwrap_or_else(|| crate::config::RESPONSE_DEFAULT.to_owned());
        let security_response = crate::config::canonical_response(&security_response)
            .map_err(crate::config::invalid)?;
        Ok(Self {
            tech,
            forge: resolved.forge,
            repo,
            workflow,
            style,
            nix: flags
                .nix
                .or_else(|| config.and_then(|c| c.landing.nix))
                .or_else(|| record.map(|r| r.parameters.nix))
                .unwrap_or(false),
            trunk,
            line_prefix,
            security_contact,
            security_response,
        })
    }

    /// The binding selected for this landing.
    #[must_use]
    pub fn tech(&self) -> &str {
        &self.tech
    }

    /// The forge selected for this landing.
    #[must_use]
    pub fn forge(&self) -> &str {
        &self.forge
    }

    /// Whether this landing opted into Nix.
    #[must_use]
    pub const fn nix(&self) -> bool {
        self.nix
    }

    /// The project path used by parameter-bearing blocks.
    #[must_use]
    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// The mode used by parameter-bearing blocks.
    #[must_use]
    pub const fn workflow(&self) -> Workflow {
        self.workflow
    }

    /// The release style used by parameter-bearing blocks.
    #[must_use]
    pub const fn style(&self) -> Option<Style> {
        self.style
    }

    /// The one permanent branch this landing writes into its artifacts.
    #[must_use]
    pub fn trunk(&self) -> &str {
        &self.trunk
    }

    /// The release-line prefix this landing writes into its artifacts.
    #[must_use]
    pub fn line_prefix(&self) -> &str {
        &self.line_prefix
    }

    /// The contact the landed policy names, empty for the forge's own
    /// authored wording.
    #[must_use]
    pub fn security_contact(&self) -> &str {
        &self.security_contact
    }

    /// The acknowledgment window the landed policy promises.
    #[must_use]
    pub fn security_response(&self) -> &str {
        &self.security_response
    }
}

#[cfg(test)]
impl Params {
    /// A parameter set for tests alone. Production code reaches `Params`
    /// through `from_record` and `resolve` and through nothing else, and
    /// this constructor is compiled out of the shipped binary.
    pub(crate) fn for_test(repo: &str, style: Option<Style>) -> Self {
        Self {
            tech: "rust".to_owned(),
            forge: "github".to_owned(),
            repo: repo.to_owned(),
            workflow: Workflow::Worktree,
            style,
            nix: false,
            trunk: crate::config::TRUNK_DEFAULT.to_owned(),
            line_prefix: crate::config::LINE_PREFIX_DEFAULT.to_owned(),
            security_contact: String::new(),
            security_response: crate::config::RESPONSE_DEFAULT.to_owned(),
        }
    }

    /// The same set with the two security parameters answered.
    pub(crate) fn for_test_security(contact: &str, response: &str) -> Self {
        Self {
            security_contact: contact.to_owned(),
            security_response: response.to_owned(),
            ..Self::for_test("acme/widget", Some(Style::Trunk))
        }
    }

    /// The same set with the Nix opt-in answered.
    pub(crate) fn set_nix_for_test(&mut self, nix: bool) {
        self.nix = nix;
    }
}

/// One authored block, read through the seam as text.
fn block(
    source: &dyn ReleaseSource,
    manifest: &ReleaseManifest,
    path: &str,
) -> Result<String, RkError> {
    let bytes = release::read(source, manifest, path)?;
    String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("{path}: a block is UTF-8").into())
}

/// The routing block for one workflow mode, read from the bundle `source`
/// carries and composed by [`pure::compose_routing`].
///
/// # Errors
///
/// Returns the source's failures for a bundle that does not carry the
/// block.
pub fn routing_block(source: &dyn ReleaseSource, workflow: Workflow) -> Result<String, RkError> {
    let manifest = source.manifest()?;
    let line = block(source, &manifest, pure::routing_line(workflow))?;
    let template = block(source, &manifest, pure::AGENTS_BLOCK)?;
    Ok(pure::compose_routing(&template, &line))
}

/// The glossary block, read from the bundle `source` carries.
///
/// # Errors
///
/// Returns the source's failures for a bundle that does not carry the
/// block.
pub fn glossary_block(source: &dyn ReleaseSource) -> Result<String, RkError> {
    let manifest = source.manifest()?;
    Ok(pure::compose_glossary(&block(
        source,
        &manifest,
        pure::GLOSSARY_BLOCK,
    )?))
}

/// The hook block for one workflow mode, read from the bundle `source`
/// carries and composed by [`pure::compose_hooks`].
///
/// # Errors
///
/// Returns the source's failures for a bundle that does not carry the
/// block.
pub fn hooks_block(source: &dyn ReleaseSource, workflow: Workflow) -> Result<String, RkError> {
    let manifest = source.manifest()?;
    let guard = match workflow {
        Workflow::Worktree => Some(block(source, &manifest, pure::PRE_COMMIT_WORKTREE_GUARD)?),
        Workflow::Branches => None,
    };
    let template = block(source, &manifest, pure::PRE_COMMIT_BLOCK)?;
    Ok(pure::compose_hooks(&template, guard.as_deref()))
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
/// `(destination, payload bytes)`, read from the bundle `source` carries
/// and selected by [`pure::select_pair`].
///
/// # Errors
///
/// Returns [`RkError::Usage`] naming the known bindings for an unknown
/// technology, and the supported pairs for a pair with no files.
pub fn pair_files(
    source: &dyn ReleaseSource,
    tech: &str,
    forge: &str,
) -> Result<Vec<(String, Vec<u8>)>, RkError> {
    let manifest = source.manifest()?;
    let files: Vec<(String, crate::digest::Digest)> = manifest
        .under("snippets")
        .map(|(rel, artifact)| (format!("snippets/{rel}"), artifact.sha256.clone()))
        .collect();
    let mut out = Vec::new();
    for selected in pure::select_pair(&files, tech, forge)? {
        out.push((selected.destination, source.blob(selected.payload)?));
    }
    Ok(out)
}

/// The whole payload projection for one pair, from the bundle `source`
/// carries.
///
/// Under the `repo`, `workflow`,
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
pub fn projection(source: &dyn ReleaseSource, params: &Params) -> Result<Vec<Entry>, RkError> {
    let mut entries = Vec::new();
    for (destination, baseline) in pair_files(source, &params.tech, &params.forge)? {
        if !params.nix && NIX_DESTINATIONS.contains(&destination.as_str()) {
            continue;
        }
        let kind = kind_of(&destination).ok_or_else(|| {
            anyhow::anyhow!("the payload does not classify {destination}; the kind table is stale")
        })?;
        let rendered = match kind {
            Kind::Rendered => render(&baseline, params),
            Kind::Seeded | Kind::State => baseline.clone(),
        };
        entries.push(Entry {
            destination,
            kind,
            placement: Placement::Whole,
            baseline,
            rendered,
        });
    }
    // A bundle from before the glossary shipped declares no template for
    // it, and an older release stays selectable: the destination joins the
    // projection only where the selected bundle carries it.
    let mut blocks = vec![(AGENTS_DESTINATION, routing_block(source, params.workflow)?)];
    if source.manifest()?.artifact(pure::GLOSSARY_BLOCK).is_some() {
        blocks.push((GLOSSARY_DESTINATION, glossary_block(source)?));
    }
    blocks.push((HOOKS_DESTINATION, hooks_block(source, params.workflow)?));
    for (destination, template) in blocks {
        entries.push(Entry {
            destination: destination.to_owned(),
            kind: Kind::Rendered,
            placement: Placement::Block,
            baseline: template.as_bytes().to_vec(),
            rendered: render(template.as_bytes(), params),
        });
    }
    entries.sort_by(|a, b| a.destination.cmp(&b.destination));
    Ok(entries)
}

/// Why the whole Nix capability stays out of a landing, or `None` where
/// the target's crate shape supports the seed: the crate shape read from
/// `target`, judged by [`pure::nix_unsupported_shape`].
#[must_use]
pub fn nix_unsupported_shape(target: &Utf8Path) -> Option<String> {
    pure::nix_unsupported_shape(&evidence::crate_shape(target))
}

/// Why the flake half of the Nix capability stays out of this landing, or
/// `None` where the pair lands whole.
///
/// The flake pair's presence is read from `target` and judged by
/// [`pure::flake_pair_withheld`]. A pair the record names is never
/// withheld, and its presence is then not even read.
///
/// # Errors
///
/// Any read failure other than the files being absent.
pub fn nix_withheld(
    target: &Utf8Path,
    recorded: Option<&manifest::Manifest>,
) -> std::io::Result<Option<String>> {
    if evidence::flake_recorded(recorded) {
        return Ok(None);
    }
    let (flake_nix, flake_lock) = evidence::flake_presence(target)?;
    Ok(pure::flake_pair_withheld(false, flake_nix, flake_lock))
}

/// One destination a landing withholds, with why.
#[derive(Debug, Clone, Serialize)]
pub struct Withheld {
    /// The destination that stays out.
    pub path: String,
    /// The reason, stated once per destination so a machine reader needs
    /// no join.
    pub reason: String,
}

/// The Nix destinations an opted-in landing withholds at this target, with
/// the one reason, or `None` where the capability lands whole.
///
/// The judgment [`withhold_nix`] applies, exposed as a value so a planner
/// can read it without an entry list: an unsupported crate shape names
/// the whole capability, and a flake pair of the target's own names the
/// pair.
///
/// # Errors
///
/// Any read failure from the pair check other than absence.
pub fn nix_withholding(
    target: &Utf8Path,
    recorded: Option<&manifest::Manifest>,
) -> Result<Option<(&'static [&'static str], String)>, RkError> {
    if let Some(reason) = nix_unsupported_shape(target) {
        return Ok(Some((&NIX_DESTINATIONS[..], reason)));
    }
    if let Some(reason) = nix_withheld(target, recorded)? {
        return Ok(Some((&NIX_WITHHOLDABLE[..], reason)));
    }
    Ok(None)
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
    let Some((set, reason)) = nix_withholding(target, recorded)? else {
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
                Ok(bytes) => Some(bytes),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                Err(e) => return Err(e),
            };
            // The block is release-kit's own text; the document is the
            // target's bytes and is never decoded.
            let block = String::from_utf8_lossy(&entry.rendered).into_owned();
            if entry.destination == HOOKS_DESTINATION {
                let text = existing.map(|bytes| String::from_utf8_lossy(&bytes).into_owned());
                let spliced =
                    splice_hooks_block(text.as_deref(), &block).map_err(std::io::Error::other)?;
                atomic::write(path.as_std_path(), spliced.as_bytes())
            } else {
                let spliced = splice_marked_block(existing.as_deref(), &block);
                atomic::write(path.as_std_path(), &spliced)
            }
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
pub fn hooks_file_defect(
    source: &dyn ReleaseSource,
    target: &Utf8Path,
) -> Result<Option<String>, RkError> {
    let path = target.join(HOOKS_DESTINATION);
    match std::fs::read(&path) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let manifest = source.manifest()?;
            let template = block(source, &manifest, pure::PRE_COMMIT_BLOCK)?;
            Ok(splice_hooks_block(Some(&text), authored(&template)).err())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
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
pub fn hooks_splice_refusal(source: &dyn ReleaseSource, target: &Utf8Path) -> Result<(), RkError> {
    hooks_file_defect(source, target)?.map_or(Ok(()), |reason| {
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
    use super::{
        AGENTS_DESTINATION, BLOCK_BEGIN, BLOCK_DESTINATIONS, BLOCK_END, BRANCH_GRAMMAR,
        GLOSSARY_DESTINATION, HOOK_TYPES_LINE, HOOKS_BEGIN, HOOKS_DESTINATION, HOOKS_END, Kind,
        SCOPE_SHAPE, Style, Workflow, extract_block, kind_of, render, splice_hooks_block,
        splice_marked_block,
    };
    use crate::embedded;
    use crate::release::EmbeddedReleaseSource;

    /// The embedded bundle, which every test here reads through the seam.
    const SOURCE: EmbeddedReleaseSource = EmbeddedReleaseSource;

    fn pair_files(
        tech: &str,
        forge: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, crate::error::RkError> {
        super::pair_files(&SOURCE, tech, forge)
    }

    fn projection(params: &super::Params) -> Result<Vec<super::Entry>, crate::error::RkError> {
        super::projection(&SOURCE, params)
    }

    fn routing_block(workflow: Workflow) -> String {
        super::routing_block(&SOURCE, workflow).expect("the embedded bundle carries the block")
    }

    fn hooks_block(workflow: Workflow) -> String {
        super::hooks_block(&SOURCE, workflow).expect("the embedded bundle carries the block")
    }

    fn glossary_block() -> String {
        super::glossary_block(&SOURCE).expect("the embedded bundle carries the block")
    }

    /// The splice returns the document's bytes; every assertion below
    /// reads them back as text, which every fixture here is.
    fn spliced(existing: Option<&str>, block: &str) -> String {
        String::from_utf8(splice_marked_block(existing.map(str::as_bytes), block))
            .expect("the fixtures are text")
    }

    #[test]
    fn private_reporting_path_tokens_are_reproducible() {
        for repo in [
            "acme/widget",
            "acme/group/widget",
            "acme/OWNER-RK_STYLE-RK_SCOPE_SHAPE",
        ] {
            assert_eq!(
                super::render(
                    b"RK_REPO RK_REPO OWNER RK_STYLE RK_SCOPE_SHAPE",
                    &super::Params::for_test(repo, Some(super::Style::Trunk))
                ),
                format!("{repo} {repo} acme trunk {}", super::SCOPE_SHAPE).as_bytes()
            );
        }
        assert_eq!(super::kind_of("SECURITY.md"), Some(super::Kind::Rendered));
    }

    /// Both forge policies carry exactly one ordered pair of every
    /// security marker. The span renderer treats anything else as a
    /// payload defect and leaves the bytes alone, so this test is what
    /// keeps a defect out of a release rather than out of one landing.
    #[test]
    fn each_forge_policy_carries_one_ordered_pair_of_every_span() {
        for forge in ["github", "gitlab"] {
            let bytes = embedded::SNIPPETS
                .get_file(format!("_shared/{forge}/SECURITY.md"))
                .expect("the policy ships")
                .contents();
            let text = String::from_utf8_lossy(bytes);
            for (begin, end) in super::SECURITY_SPANS {
                let begin = String::from_utf8_lossy(begin);
                let end = String::from_utf8_lossy(end);
                assert_eq!(text.matches(begin.as_ref()).count(), 1, "{forge} {begin}");
                assert_eq!(text.matches(end.as_ref()).count(), 1, "{forge} {end}");
                assert!(
                    text.find(begin.as_ref()) < text.find(end.as_ref()),
                    "{forge}: {begin} must precede {end}"
                );
            }
        }
    }

    /// The default answers reproduce each forge's authored policy exactly,
    /// markers removed and each forge's own wording kept; an answered one
    /// states it; and a contact spelling a token name lands literally,
    /// because the spans resolve after every substitution.
    #[test]
    fn the_security_spans_render_per_answer() {
        for forge in ["github", "gitlab"] {
            let bytes = embedded::SNIPPETS
                .get_file(format!("_shared/{forge}/SECURITY.md"))
                .expect("the policy ships")
                .contents();
            let authored = String::from_utf8_lossy(bytes);
            let stripped = {
                let mut text = authored.clone().into_owned();
                for (begin, end) in super::SECURITY_SPANS {
                    text = text.replace(&String::from_utf8_lossy(begin).into_owned(), "");
                    text = text.replace(&String::from_utf8_lossy(end).into_owned(), "");
                }
                text
            };
            let default = super::Params {
                forge: forge.to_owned(),
                ..super::Params::for_test_security("", crate::config::RESPONSE_DEFAULT)
            };
            let rendered = String::from_utf8(render(bytes, &default)).expect("text");
            assert_eq!(
                rendered,
                stripped.replace("RK_REPO", "acme/widget"),
                "{forge}: the default answers must reproduce the authored policy"
            );
            assert!(!rendered.contains("RK_SECURITY"), "{forge}: {rendered}");

            let answered = super::Params {
                forge: forge.to_owned(),
                ..super::Params::for_test_security("OWNER RK_REPO <team@acme.example>", "14 days")
            };
            let rendered = String::from_utf8(render(bytes, &answered)).expect("text");
            assert!(
                rendered.contains("OWNER RK_REPO <team@acme.example>"),
                "{forge}: a contact spelling a token name lands literally: {rendered}"
            );
            assert!(
                rendered.contains("Maintainers acknowledge a report within 14 days."),
                "{forge}: {rendered}"
            );
            assert!(
                rendered.contains("This policy commits to no disclosure deadline."),
                "{forge}: {rendered}"
            );
            assert!(
                !rendered.contains("best-effort basis"),
                "{forge}: a stated window replaces the best-effort sentence: {rendered}"
            );
            assert!(
                !rendered.contains("no response or disclosure deadline"),
                "{forge}: a stated window contradicts the response disclaimer: {rendered}"
            );
        }
    }

    /// A defective span leaves the bytes alone rather than producing a
    /// half-written sentence: the payload test above is what catches one.
    #[test]
    fn a_defective_span_renders_unchanged() {
        let (begin, end) = super::SECURITY_SPANS[0];
        let begin = String::from_utf8_lossy(begin).into_owned();
        let end = String::from_utf8_lossy(end).into_owned();
        let params = super::Params::for_test_security("team@acme.example", "1 day");
        for baseline in [
            format!("contact {begin}a maintainer\n"),
            format!("contact a maintainer{end}\n"),
            format!("contact {end}a maintainer{begin}\n"),
            "contact a maintainer\n".to_owned(),
        ] {
            assert_eq!(
                render(baseline.as_bytes(), &params),
                baseline.as_bytes(),
                "{baseline}"
            );
        }
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
        for block in BLOCK_DESTINATIONS {
            assert_eq!(kind_of(block), Some(Kind::Rendered), "{block}");
        }
        assert_eq!(kind_of("something-else.txt"), None);
    }

    /// Substitution is total and derives from the repo parameter's first
    /// segment, so a nested GitLab project path still yields its root
    /// namespace. The scope shape rests on no parameter, so it renders
    /// under every landing.
    #[test]
    fn rendering_substitutes_every_owner_occurrence() {
        let baseline = b"if: repository_owner == 'OWNER'\n# OWNER again: OWNER\n";
        let rendered = render(baseline, &super::Params::for_test("acme/sub/widget", None));
        let text = String::from_utf8(rendered).expect("rendered bytes stay text");
        assert_eq!(text, "if: repository_owner == 'acme'\n# acme again: acme\n");

        let baseline = b"match (RK_SCOPE_SHAPE)\n";
        let rendered = render(baseline, &super::Params::for_test("acme/widget", None));
        let text = String::from_utf8(rendered).expect("rendered bytes stay text");
        assert_eq!(text, format!("match ({SCOPE_SHAPE})\n"));
    }

    /// The one scope shape is a bracket expression an extended regular
    /// expression takes verbatim: lowercase, and with the `-` last, where
    /// it stands for itself rather than opening a range.
    #[test]
    fn the_scope_shape_drops_into_the_title_check() {
        assert_eq!(SCOPE_SHAPE, "[a-z0-9._/-]+");
        assert!(
            !SCOPE_SHAPE.contains('\''),
            "the title checks single-quote it"
        );
    }

    /// The predicate `rk message --check` calls and the pattern the title
    /// checks render admit exactly the same characters. The pattern is
    /// expanded here from its own text, so editing one owner without the
    /// other fails: the desk and the forge judge one language.
    #[test]
    fn the_scope_predicate_and_the_rendered_pattern_agree() {
        let body = SCOPE_SHAPE
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix("]+"))
            .expect("the shape is one bracket expression, repeated");
        let chars: Vec<char> = body.chars().collect();
        let mut admitted = std::collections::BTreeSet::new();
        let mut at = 0;
        while at < chars.len() {
            // A `-` with a neighbour on each side opens a range; last, it
            // stands for itself, which is why the shape ends with it.
            if at + 2 < chars.len() && chars[at + 1] == '-' {
                for c in chars[at]..=chars[at + 2] {
                    admitted.insert(c);
                }
                at += 3;
            } else {
                admitted.insert(chars[at]);
                at += 1;
            }
        }
        for byte in 0..=127u8 {
            let c = char::from(byte);
            assert_eq!(
                super::scope_is_shaped(&c.to_string()),
                admitted.contains(&c),
                "the predicate and {SCOPE_SHAPE} disagree on {c:?}"
            );
        }
        assert!(super::scope_is_shaped("guides/release"));
        assert!(!super::scope_is_shaped(""), "a scope is never empty");
        assert!(!super::scope_is_shaped("Specs Ugly"));
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

    /// A loaded record reaches the projection unchanged, including old
    /// records' absent style and the two workflow modes.
    #[test]
    fn params_from_a_record_round_trips() {
        use super::{Params, manifest};
        let dir = tempfile::tempdir().expect("a scratch target exists");
        let target = camino::Utf8Path::from_path(dir.path()).expect("utf-8 path");
        for tech in ["rust", "bash"] {
            for forge in ["github", "gitlab"] {
                for workflow in [Workflow::Branches, Workflow::Worktree] {
                    for style in [None, Some(Style::Trunk), Some(Style::Lines)] {
                        for nix in [false, true] {
                            let record = manifest::Manifest {
                                schema_version: manifest::SCHEMA_VERSION,
                                rk_version: "0.1.0".to_owned(),
                                payload_sha256: crate::digest::Digest::of(b""),
                                origin: "init".to_owned(),
                                tech: tech.to_owned(),
                                forge: forge.to_owned(),
                                landed_at: "2026-08-29T00:00:00Z".to_owned(),
                                parameters: manifest::Parameters {
                                    repo: "acme/team/widget".to_owned(),
                                    workflow,
                                    style,
                                    nix,
                                    trunk: crate::config::TRUNK_DEFAULT.to_owned(),
                                    line_prefix: crate::config::LINE_PREFIX_DEFAULT.to_owned(),
                                    security_contact: String::new(),
                                    security_response: crate::config::RESPONSE_DEFAULT.to_owned(),
                                },
                                files: Vec::new(),
                                pins: std::collections::BTreeMap::new(),
                            };
                            manifest::write(target, &record).expect("the record writes");
                            let loaded = manifest::load(target)
                                .expect("the record loads")
                                .expect("the record exists");
                            let params = Params::from_record(&loaded);
                            assert_eq!(params.tech, tech);
                            assert_eq!(params.forge, forge);
                            assert_eq!(params.repo(), "acme/team/widget");
                            assert_eq!(params.workflow(), workflow);
                            assert_eq!(params.style(), style);
                            assert_eq!(params.nix, nix);
                            let entries = projection(&params).expect("the record projects");
                            let mut expected: Vec<_> = pair_files(tech, forge)
                                .expect("the pair lists")
                                .into_iter()
                                .filter(|(path, _)| {
                                    nix || !super::NIX_DESTINATIONS.contains(&path.as_str())
                                })
                                .collect();
                            let routing = routing_block(workflow);
                            let hooks = hooks_block(workflow);
                            let glossary = glossary_block();
                            expected.push((AGENTS_DESTINATION.to_owned(), routing.into_bytes()));
                            expected.push((GLOSSARY_DESTINATION.to_owned(), glossary.into_bytes()));
                            expected.push((HOOKS_DESTINATION.to_owned(), hooks.into_bytes()));
                            expected.sort_by(|a, b| a.0.cmp(&b.0));
                            assert_eq!(entries.len(), expected.len());
                            for (entry, (destination, baseline)) in entries.iter().zip(expected) {
                                assert_eq!(entry.destination, destination);
                                assert_eq!(entry.baseline, baseline);
                                let rendered = match entry.kind {
                                    Kind::Rendered => super::render(
                                        &baseline,
                                        &super::Params::for_test("acme/team/widget", style),
                                    ),
                                    Kind::Seeded | Kind::State => baseline.clone(),
                                };
                                assert_eq!(entry.rendered, rendered, "{destination}");
                            }
                        }
                    }
                }
            }
        }
    }

    fn resolved_test_params(
        tech: &str,
        resolved: &super::Resolved,
        workflow: Workflow,
        style: Option<Style>,
        nix: bool,
    ) -> Result<super::Params, crate::error::RkError> {
        super::Params::resolve(
            &SOURCE,
            camino::Utf8Path::new("."),
            &super::Inputs {
                tech: Some(tech),
                forge: Some(&resolved.forge),
                repo: resolved.repo.as_deref(),
                workflow: Some(workflow),
                style,
                nix: Some(nix),
            },
            None,
            None,
            super::Purpose::Init,
        )
    }

    /// A rendered projection carries no unsubstituted token and no
    /// mechanical sentinel; the one judgment sentinel stays in its seeded
    /// file.
    #[test]
    fn a_projection_renders_owned_files_and_keeps_seeded_judgment() {
        let entries = projection(
            &resolved_test_params(
                "rust",
                &super::Resolved {
                    forge: "github".to_owned(),
                    repo: Some("acme/widget".to_owned()),
                },
                Workflow::Branches,
                Some(Style::Trunk),
                false,
            )
            .expect("the parameters resolve"),
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
        assert!(text.contains(SCOPE_SHAPE), "{text}");
        assert!(
            !text.contains("RK_SCOPE_SHAPE"),
            "a scope token survived: {text}"
        );
        let seeded = entries
            .iter()
            .find(|entry| entry.destination == "release-plz.toml")
            .expect("the seeded file projects");
        assert_eq!(seeded.kind, Kind::Seeded);
        assert_eq!(seeded.rendered, seeded.baseline);
        assert!(String::from_utf8_lossy(&seeded.rendered).contains("TODO(release-kit)"));
        for block in BLOCK_DESTINATIONS {
            let entry = entries
                .iter()
                .find(|entry| entry.destination == block)
                .expect("every block is part of the projection");
            let text = String::from_utf8_lossy(&entry.rendered);
            assert!(
                !text.contains("RK_SCOPE_SHAPE"),
                "{block} kept a token: {text}"
            );
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
                &resolved_test_params(
                    "rust",
                    &super::Resolved {
                        forge: forge.to_owned(),
                        repo: Some("acme/widget".to_owned()),
                    },
                    Workflow::Worktree,
                    Some(Style::Trunk),
                    nix,
                )
                .expect("the parameters resolve"),
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
        // The capability lands no workflow, so both forges land the same
        // set: a job proving the build holds a merge only inside the
        // workflow the required check needs, and that one is the
        // target's own.
        let gitlab = paths(true, "gitlab");
        assert!(gitlab.contains(&"nix/package.nix".to_owned()));
        assert!(
            !on.iter()
                .chain(gitlab.iter())
                .any(|destination| destination.contains("nix.yml"))
        );
        let bash = projection(
            &resolved_test_params(
                "bash",
                &super::Resolved {
                    forge: "github".to_owned(),
                    repo: Some("acme/widget".to_owned()),
                },
                Workflow::Worktree,
                Some(Style::Trunk),
                true,
            )
            .expect("the parameters resolve"),
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
                &resolved_test_params(
                    "rust",
                    &super::Resolved {
                        forge: "github".to_owned(),
                        repo: Some("acme/widget".to_owned()),
                    },
                    Workflow::Worktree,
                    Some(Style::Trunk),
                    true,
                )
                .expect("the parameters resolve"),
            )
            .expect("the pair projects")
        };

        // No Cargo.toml: the whole capability is withheld by name.
        let mut all = entries();
        let withheld = withhold_nix(target, true, None, &mut all).expect("the judgment runs");
        let paths: Vec<&str> = withheld.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(paths, ["flake.lock", "flake.nix", "nix/package.nix"]);
        assert!(
            all.iter()
                .all(|entry| !NIX_DESTINATIONS.contains(&entry.destination.as_str()))
        );

        // A single crate with its own flake: the seed pair is withheld,
        // and the package expression still lands.
        std::fs::write(
            target.join("Cargo.toml"),
            "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n",
        )
        .expect("the crate manifest writes");
        std::fs::write(target.join("Cargo.lock"), "version = 4\n").expect("the lock writes");
        std::fs::create_dir_all(target.join("src")).expect("the src dir exists");
        std::fs::write(target.join("src/main.rs"), "fn main() {}\n").expect("the main writes");
        std::fs::write(target.join("flake.nix"), "{ }\n").expect("the flake writes");
        let mut all = entries();
        let withheld = withhold_nix(target, true, None, &mut all).expect("the judgment runs");
        let paths: Vec<&str> = withheld.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(paths, ["flake.lock", "flake.nix"]);
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

    /// The glossary takes the same three shapes the routing block does,
    /// and the marker pair it shares with `AGENTS.md` is what makes one
    /// splice serve both.
    #[test]
    fn the_glossary_splices_into_every_shape() {
        let owned = glossary_block();
        let block = owned.as_str();

        let fresh = spliced(None, block);
        assert_eq!(fresh, format!("{block}\n"));
        assert_eq!(extract_block(&fresh, BLOCK_BEGIN, BLOCK_END), Some(block));

        let own = "# Glossary\n\n- `spike` — a throwaway branch.\n";
        let appended = spliced(Some(own), block);
        assert!(appended.starts_with(own));
        assert_eq!(
            extract_block(&appended, BLOCK_BEGIN, BLOCK_END),
            Some(block)
        );

        let stale = appended.replace("full-implement", "do-everything");
        let refreshed = spliced(Some(&stale), block);
        assert_eq!(
            extract_block(&refreshed, BLOCK_BEGIN, BLOCK_END),
            Some(block)
        );
        assert_eq!(
            refreshed.matches("BEGIN release-kit").count(),
            1,
            "a re-splice must replace, not accumulate"
        );
    }

    /// Every line the target wrote below the end marker survives a
    /// re-splice byte for byte: the block owns its marked lines and the
    /// document belongs to the target.
    #[test]
    fn the_glossary_leaves_the_targets_region_alone() {
        let owned = glossary_block();
        let block = owned.as_str();
        let below = "\n## Our own terms\n\n- `spike` — a throwaway branch, never merged.\n";
        let landed = format!("{block}\n{below}");

        let refreshed = spliced(Some(&landed), block);
        assert!(
            refreshed.ends_with(below),
            "the target's own region changed: {refreshed}"
        );
        assert_eq!(
            extract_block(&refreshed, BLOCK_BEGIN, BLOCK_END),
            Some(block)
        );
    }

    /// Appending keeps the document whole: trailing spaces, blank lines,
    /// and a missing final newline are the target's bytes, and a block
    /// that owns its marked lines alone rewrites none of them.
    #[test]
    fn an_append_rewrites_no_byte_the_target_wrote() {
        let owned = glossary_block();
        let block = owned.as_str();
        for own in [
            "# Glossary\n\n- `spike` — throwaway.   \n\n\n",
            "# Glossary\n\n- `spike` — throwaway.",
            "# Glossary\r\n\r\n- `spike` — throwaway.\r\n",
        ] {
            let appended = spliced(Some(own), block);
            assert!(
                appended.starts_with(own),
                "the target's bytes changed: {appended:?}"
            );
            assert_eq!(
                extract_block(&appended, BLOCK_BEGIN, BLOCK_END),
                Some(block),
                "{appended:?}"
            );
            let marker = appended.find(BLOCK_BEGIN).expect("the block landed");
            assert!(
                appended[..marker].ends_with('\n'),
                "the block must open its own line: {appended:?}"
            );
        }
    }

    /// A document the target wrote is bytes, not text. A splice that
    /// decoded it would replace an invalid sequence with U+FFFD and
    /// rewrite a byte outside the markers, which the rule forbids.
    #[test]
    fn a_splice_decodes_no_byte_the_target_wrote() {
        let owned = glossary_block();
        let block = owned.as_str();

        // Appending: the invalid byte sits in the target's own document.
        let own = b"# Glossary\n\ncaf\xe9\n";
        let appended = splice_marked_block(Some(own), block);
        assert!(
            appended.starts_with(own),
            "the target's bytes changed: {appended:?}"
        );
        assert!(!appended.contains(&0xEF), "a replacement character landed");

        // Replacing: the invalid byte sits below the end marker.
        let mut landed = Vec::new();
        landed.extend_from_slice(block.replace("full-implement", "do-everything").as_bytes());
        landed.extend_from_slice(b"\n\ncaf\xe9\n");
        let refreshed = splice_marked_block(Some(&landed), block);
        assert!(
            refreshed.ends_with(b"\n\ncaf\xe9\n"),
            "the target's region below the markers changed: {refreshed:?}"
        );
        assert!(refreshed.starts_with(block.as_bytes()), "{refreshed:?}");
    }

    /// The glossary carries no parameter, so the same bytes land in
    /// every target: no token survives it and no mode changes it.
    #[test]
    fn the_glossary_block_carries_no_parameter() {
        let block = glossary_block();
        assert!(block.starts_with(BLOCK_BEGIN), "{block}");
        assert!(block.ends_with(BLOCK_END), "{block}");
        assert!(!block.contains("RK_"), "a token survived: {block}");
        assert!(!block.contains("OWNER"), "an owner token survived: {block}");
        for term in [
            "implement-and-request",
            "implement-and-merge",
            "full-implement",
        ] {
            assert!(block.contains(term), "{term} is missing from {block}");
        }
        assert!(
            routing_block(Workflow::Worktree).contains(GLOSSARY_DESTINATION),
            "the routing block must name the destination it indexes"
        );
    }

    #[test]
    fn the_block_splices_into_every_agents_shape() {
        let owned = routing_block(Workflow::Branches);
        let block = owned.as_str();
        let fresh = spliced(None, block);
        assert_eq!(fresh, format!("{block}\n"));
        assert_eq!(extract_block(&fresh, BLOCK_BEGIN, BLOCK_END), Some(block));

        let appended = spliced(Some("# My project\n\nOwn rules.\n"), block);
        assert!(appended.starts_with("# My project\n\nOwn rules.\n\n<!-- BEGIN release-kit -->"));
        assert_eq!(
            extract_block(&appended, BLOCK_BEGIN, BLOCK_END),
            Some(block)
        );

        let stale = appended.replace("Never author a tag", "Do author a tag");
        let refreshed = spliced(Some(&stale), block);
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
            assert!(block.contains("Create or remove a worktree"));
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
