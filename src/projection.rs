//! The one pure target-specific projection over this binary's embedded
//! sources.
//!
//! [`Projection::compute`] takes a [`ProjectionInput`], values alone, and
//! answers the complete candidate artifact tree this installed binary
//! would land in one target: every destination with its ownership
//! [`Kind`], its [`Placement`], the complete proposed bytes, the rendered
//! region where the destination is a marked region, and the embedded
//! source paths it was rendered from. Staging and production landing
//! share it byte for byte, so what an agent studies in a stage is what a
//! landing writes.
//!
//! This module reads snippets and blocks through `src/embedded.rs`
//! directly and reads nothing else: no target file, no environment, no
//! Git state, no clock, no registry, no network. Whatever a projection
//! needs from the target arrives as [`TargetEvidence`], gathered before
//! construction by [`evidence::gather`], which lives in its own file so a
//! source scan can hold this one to the pure boundary.
//!
//! Every pure piece of the landing model has one implementation here: the
//! kind table, the token substitution, the block templating, the splice
//! and marker judgments, and the Nix crate-shape judgment. Which embedded
//! source belongs to which capability, and whether a capability is
//! available at the target's dimensions, is the catalog's answer in
//! `crate::profile::catalog`, and this module renders what the catalog
//! selects. `src/landing.rs` re-exports the pieces under their old names.
//!
//! One input type, one compute function, one candidate shape: staging and
//! every landing verb consume this one projection.

pub mod evidence;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::embedded;
use crate::error::RkError;
pub use crate::landing::manifest::Provider;
use crate::landing::{CheckoutMode, Params};
use crate::profile::ReleaseMode;
use crate::profile::catalog::{self, Availability, Selection, Status};

/// The complete input to one projection: the resolved landing parameters
/// and the typed evidence read from the target beforehand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionInput {
    /// The resolved landing parameters.
    pub params: Params,
    /// What the target already holds, as values.
    pub evidence: TargetEvidence,
}

/// What a projection needs to know about the target, gathered before the
/// projection runs and carried as values.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "each field is one independent fact read off the target, not a state a smaller type could carry"
)]
pub struct TargetEvidence {
    /// The complete existing document at each block destination that
    /// exists on disk, keyed by destination. An absent key is an absent
    /// file.
    pub documents: BTreeMap<String, Vec<u8>>,
    /// The crate facts the Nix seed relies on.
    pub crate_shape: CrateShape,
    /// Whether `flake.nix` is present at the target, a link included.
    pub flake_nix_present: bool,
    /// Whether `flake.lock` is present at the target, a link included.
    pub flake_lock_present: bool,
    /// Whether the receipt already records `flake.nix`: a pair release-kit
    /// landed is its own and is never withheld.
    pub flake_recorded: bool,
    /// Whether `.gitlab-ci.yml` is present at the target, a link included.
    pub root_pipeline_present: bool,
    /// Whether the receipt already records `.gitlab-ci.yml`: a root
    /// pipeline release-kit landed is its own and is never withheld.
    pub root_pipeline_recorded: bool,
}

impl TargetEvidence {
    /// The existing document at one block destination.
    #[must_use]
    pub fn document(&self, destination: &str) -> Option<&[u8]> {
        self.documents.get(destination).map(Vec::as_slice)
    }
}

/// The structural facts of the target's crate that the seeded Nix
/// package expression and the seed flake's smoke check rely on.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CrateShape {
    /// The text of the target's `Cargo.toml`, or `None` where none reads.
    pub cargo_toml: Option<String>,
    /// Whether `Cargo.lock` is a file at the target.
    pub cargo_lock: bool,
    /// Whether `src/main.rs` is a file at the target.
    pub main_rs: bool,
}

/// The complete candidate artifact tree for one target.
///
/// A value, never a record: it carries no operation, no decision, and no
/// apply state, and it is not serializable. A consumer renders it again
/// from scratch rather than reading a saved copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    /// Every capability the catalog answered for this target, in catalog
    /// order: selected, withheld, or omitted with its reason.
    pub capabilities: Vec<Selection>,
    /// Every candidate, sorted by destination.
    pub candidates: Vec<Candidate>,
    /// The destinations the target's own state withholds, each with its
    /// one reason and, where one exists, the operator's one edit. A
    /// destination no selected capability ships is absent, never
    /// omitted.
    pub omissions: Vec<Omission>,
    /// The block destinations whose existing document offers the block no
    /// place, a target-side defect staging explains and landing refuses.
    pub collisions: Vec<Collision>,
    /// Why the recorded code scanning provider's licence condition refuses
    /// this target, or `None` where no condition applies or the licence
    /// satisfies it. A landing verb refuses on it and writes nothing; `rk
    /// status` reports it as a warning and still exits 0, because the
    /// target is not broken and the operator owns the licensing decision.
    pub licence_refusal: Option<String>,
    /// Why a recorded capability cannot run at this target at all, which is a
    /// receipt nothing can honour rather than a decision anyone made. Empty on
    /// every target whose parameters came through resolution, because
    /// resolution refuses these before they are recorded.
    pub record_defects: Vec<String>,
}

/// One proposed destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The capability that lands it.
    pub capability: &'static str,
    /// The destination, relative to the target root.
    pub destination: String,
    /// Who owns the bytes after landing.
    pub kind: Kind,
    /// The whole file, or the one marked region.
    pub placement: Placement,
    /// The complete proposed destination bytes. For a region this is the
    /// complete spliced document, computed from the existing document in
    /// the evidence, so a staged view equals what production writes.
    pub bytes: Vec<u8>,
    /// The rendered block alone for a region destination: what the
    /// receipt digests. `None` for a whole file.
    pub region: Option<Vec<u8>>,
    /// The embedded source paths the candidate was rendered from, each
    /// carrying its distribution root as the first segment.
    pub sources: Vec<String>,
}

/// How a candidate occupies its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// The candidate is the whole file.
    Whole,
    /// The candidate is the one marked region between these markers; the
    /// bytes outside them belong to the target.
    Region {
        /// The opening marker.
        begin: &'static str,
        /// The closing marker.
        end: &'static str,
    },
}

/// One destination withheld from this target, with why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Omission {
    /// The destination that stays out.
    pub destination: String,
    /// The reason, stated once per destination.
    pub reason: String,
    /// The one edit the operator makes to activate what release-kit could
    /// not, where the omission is a withheld activation rather than a
    /// shape the target cannot take.
    pub action: Option<String>,
}

/// One block destination the target's document cannot take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    /// The destination whose document offers the block no place.
    pub destination: String,
    /// The reason, for staging to explain and landing to refuse with.
    pub reason: String,
}

impl Projection {
    /// The complete candidate tree for `input`, from this binary's
    /// embedded sources alone.
    ///
    /// # Errors
    ///
    /// A source defect in this binary: an unknown technology or an
    /// unsupported pair as [`RkError::Usage`], and as [`RkError::Other`] a
    /// destination two sources ship, a snippet the kind table does not
    /// classify, or a block this binary does not embed.
    pub fn compute(input: &ProjectionInput) -> Result<Self, RkError> {
        Self::compute_over(&embedded_snippets(), input)
    }

    /// [`Self::compute`] over an explicit snippet list, whose paths carry
    /// the `snippets/` root; the embedded tree in production, an injected
    /// one under test.
    #[allow(
        clippy::too_many_lines,
        reason = "one pass selects, renders, splices, and withholds; splitting it would separate a withheld destination from the selection that offered it"
    )]
    fn compute_over(files: &[(String, &[u8])], input: &ProjectionInput) -> Result<Self, RkError> {
        let params = &input.params;
        let evidence = &input.evidence;
        let availability = Availability::over(
            files
                .iter()
                .filter_map(|(path, _)| path.strip_prefix("snippets/").map(str::to_owned))
                .collect(),
        );
        let mut capabilities = catalog::select(params, &availability);
        let mut candidates: Vec<Candidate> = Vec::new();
        for selection in &capabilities {
            if !selection.lands() {
                continue;
            }
            for source in &selection.sources {
                let destination = source
                    .splitn(3, '/')
                    .nth(2)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{source}: a snippet path has a zone, a forge, and a destination"
                        )
                    })?
                    .to_owned();
                let path = format!("snippets/{source}");
                let bytes = files
                    .iter()
                    .find(|(candidate, _)| *candidate == path)
                    .map(|(_, bytes)| *bytes)
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{path}: the catalog selected a source the tree does not carry"
                        )
                    })?;
                if let Some(existing) = candidates
                    .iter()
                    .find(|candidate| candidate.destination == destination)
                {
                    return Err(anyhow::anyhow!(
                        "{} and {} both ship {destination}: {} and {path}; the embedded sources are defective",
                        existing.capability,
                        selection.id,
                        existing.sources.join(", ")
                    )
                    .into());
                }
                let kind = kind_of(&destination).ok_or_else(|| {
                    anyhow::anyhow!(
                        "the embedded sources do not classify {destination}; the kind table is stale"
                    )
                })?;
                let rendered = match kind {
                    Kind::Rendered => render(bytes, params),
                    Kind::Seeded | Kind::State => bytes.to_vec(),
                };
                candidates.push(Candidate {
                    capability: selection.id,
                    destination,
                    kind,
                    placement: Placement::Whole,
                    bytes: rendered,
                    region: None,
                    sources: vec![path],
                });
            }
        }
        let mut collisions = Vec::new();
        for destination in BLOCK_DESTINATIONS {
            let (template, sources) = block_template(destination, params.checkout_mode())?;
            if let Some(whole) = candidates
                .iter()
                .find(|candidate| candidate.destination == destination)
            {
                return Err(anyhow::anyhow!(
                    "{destination} is both a whole file from {} and a marked region from {}; the embedded sources are defective",
                    whole.sources.join(", "),
                    sources.join(", ")
                )
                .into());
            }
            let region = render(template.as_bytes(), params);
            let (begin, end) = block_markers(destination).ok_or_else(|| {
                anyhow::anyhow!("{destination} is a block destination with no markers")
            })?;
            match propose_document(destination, evidence.document(destination), &region) {
                Ok(bytes) => candidates.push(Candidate {
                    capability: catalog::GUARDS,
                    destination: destination.to_owned(),
                    kind: Kind::Rendered,
                    placement: Placement::Region { begin, end },
                    bytes,
                    region: Some(region),
                    sources,
                }),
                Err(reason) => collisions.push(Collision {
                    destination: destination.to_owned(),
                    reason,
                }),
            }
        }
        let mut omissions = Vec::new();
        if let Some((set, reason)) = nix_withholding(params.nix_packaging(), evidence)
            && candidates
                .iter()
                .any(|candidate| candidate.capability == catalog::PACKAGING_NIX)
        {
            candidates.retain(|candidate| {
                if set.contains(&candidate.destination.as_str()) {
                    omissions.push(Omission {
                        destination: candidate.destination.clone(),
                        reason: reason.clone(),
                        action: None,
                    });
                    false
                } else {
                    true
                }
            });
            withhold(&mut capabilities, catalog::PACKAGING_NIX, &reason, None);
        }
        // The release-less GitLab root pipeline: where the target owns a
        // root pipeline the receipt does not attribute, the fragment still
        // lands as the gate's prerequisite, and the activation is the
        // operator's one include line rather than a refusal of the whole
        // landing.
        let title_root = candidates.iter().position(|candidate| {
            candidate.capability == catalog::TITLE_CHECK
                && candidate.destination == GITLAB_ROOT_PIPELINE
        });
        if let Some(index) = title_root
            && evidence.root_pipeline_present
            && !evidence.root_pipeline_recorded
        {
            let candidate = candidates.remove(index);
            let reason = format!(
                "the target owns {GITLAB_ROOT_PIPELINE}, so the release-less root pipeline that would activate the title gate stays out; the fragment lands as the gate's prerequisite"
            );
            let action = format!(
                "add the include to the target's own {GITLAB_ROOT_PIPELINE}: include:\n  - local: {GITLAB_TITLE_FRAGMENT}"
            );
            omissions.push(Omission {
                destination: candidate.destination,
                reason: reason.clone(),
                action: Some(action.clone()),
            });
            withhold(
                &mut capabilities,
                catalog::TITLE_CHECK,
                &reason,
                Some(action),
            );
        }
        candidates.sort_by(|a, b| a.destination.cmp(&b.destination));
        omissions.sort_by(|a, b| a.destination.cmp(&b.destination));
        let licence_refusal = code_scanning_licence_refusal(
            params.code_scanning(),
            params.driver(),
            &evidence.crate_shape,
        );
        let mut record_defects: Vec<String> =
            code_scanning_incompatibility(params.code_scanning(), params.driver(), params.forge())
                .into_iter()
                .collect();
        // A recorded automatic release whose automation this release cannot
        // land is a receipt nothing can honour: named here, so a
        // record-only reader sees it.
        if params.release_mode() == ReleaseMode::Automatic
            && let Some(selection) = capabilities
                .iter()
                .find(|selection| selection.id == catalog::RELEASE_AUTOMATION)
            && matches!(selection.status, Status::Unavailable | Status::Unknown)
            && let Some(reason) = &selection.reason
        {
            record_defects.push(reason.clone());
        }
        Ok(Self {
            capabilities,
            candidates,
            omissions,
            collisions,
            licence_refusal,
            record_defects,
        })
    }

    /// Why the selected release automation cannot land, where the profile
    /// asked for one this release does not carry at its dimensions. An
    /// apply refuses on it before any write; a preview reports it.
    #[must_use]
    pub fn release_unavailable(&self) -> Option<&str> {
        self.capabilities
            .iter()
            .find(|selection| {
                selection.id == catalog::RELEASE_AUTOMATION
                    && matches!(selection.status, Status::Unavailable | Status::Unknown)
            })
            .and_then(|selection| selection.reason.as_deref())
    }

    /// The one selection for a capability.
    #[must_use]
    pub fn capability(&self, id: &str) -> Option<&Selection> {
        self.capabilities
            .iter()
            .find(|selection| selection.id == id)
    }
}

/// Mark one capability withheld with its reason and the operator's edit.
fn withhold(capabilities: &mut [Selection], id: &str, reason: &str, action: Option<String>) {
    if let Some(selection) = capabilities.iter_mut().find(|selection| selection.id == id) {
        selection.status = Status::Withheld;
        selection.reason = Some(reason.to_owned());
        selection.action = action;
    }
}

/// The GitLab root pipeline, the one destination a release-less landing
/// withholds where the target owns it.
pub const GITLAB_ROOT_PIPELINE: &str = ".gitlab-ci.yml";

/// The GitLab title fragment the root pipeline includes.
pub const GITLAB_TITLE_FRAGMENT: &str = ".gitlab/ci/mr-title.yml";

/// Every snippet this binary embeds, as `(path, bytes)` with the path
/// carrying the `snippets/` root, sorted by path.
fn embedded_snippets() -> Vec<(String, &'static [u8])> {
    embedded::walk(&embedded::SNIPPETS)
        .into_iter()
        .map(|(path, bytes)| (format!("snippets/{path}"), bytes))
        .collect()
}

/// Every `(driver, forge)` pair at which the release automation is
/// available, in path order.
#[must_use]
pub fn supported_pairs() -> Vec<(String, String)> {
    Availability::embedded()
        .automation_tuples()
        .into_iter()
        .filter_map(|entry| {
            entry
                .split_once(", ")
                .map(|(driver, forge)| (driver.to_owned(), forge.to_owned()))
        })
        .collect()
}

/// Who owns a landed file's bytes after landing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// release-kit owns it: a newer binary re-renders it, and a target
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
const KINDS: [(&str, Kind); 20] = [
    (".github/workflows/release-plz.yml", Kind::Rendered),
    (".github/workflows/release-please.yml", Kind::Rendered),
    (".github/workflows/release.yml", Kind::Rendered),
    (".github/workflows/pr-title.yml", Kind::Rendered),
    (".github/workflows/scorecard.yml", Kind::Rendered),
    (".github/workflows/code-scanning-codeql.yml", Kind::Rendered),
    (
        ".github/workflows/code-scanning-semgrep.yml",
        Kind::Rendered,
    ),
    (".gitlab/ci/code-scanning-semgrep.yml", Kind::Rendered),
    (".gitlab-ci.yml", Kind::Rendered),
    ("SECURITY.md", Kind::Rendered),
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
///
/// The capability lands no workflow, on either forge, and each forge's
/// reason is its own. On GitHub a job gates the merge only inside the
/// workflow the required check needs, and that workflow is the target's
/// own. On GitLab the merge check is the whole pipeline, and a target's
/// jobs live in the child pipeline the rendered parent triggers, which the
/// target owns. The bindings serve the job for both.
pub const NIX_DESTINATIONS: [&str; 3] = ["nix/package.nix", "flake.nix", "flake.lock"];

/// The subset a target with a flake of its own keeps out: the seed pair,
/// whose files would sit beside a flake release-kit did not author.
///
/// The seeded package expression is not in it: it lands either way, as
/// the starting point the target integrates by hand.
pub const NIX_WITHHOLDABLE: [&str; 2] = ["flake.nix", "flake.lock"];

/// The destinations of the opt-in Scorecard capability, present in a
/// projection only where the landing's `scorecard` parameter is on.
///
/// The parameter is recorded for the reason [`NIX_DESTINATIONS`] states:
/// an absent file under `scorecard = false` is not wanted, never drifted.
/// The capability is GitHub's alone, so the shared GitLab zone ships no
/// counterpart and a GitLab landing projects nothing for it, whatever the
/// parameter says. A Scorecard run needs no forge setting on a public
/// repository, so the capability adds no setup step and cannot conflict
/// with `forge-setup:every-supported-forge-runs-every-step`.
///
/// The file is `rendered`: its bytes carry the recorded trunk and nothing a
/// target is expected to tune, so release-kit owns them and an edit is
/// drift.
pub const SCORECARD_DESTINATIONS: [&str; 1] = [".github/workflows/scorecard.yml"];

/// Every destination the opt-in code scanning capability can land, against
/// the provider that lands it.
///
/// One source owns one destination, so the provider is in the name: a
/// landing writes exactly the entry its recorded provider names and the
/// forge ships. Switching provider retires one destination and adds
/// another, which `landing:a-dropped-file-stays` already answers.
///
/// `codeql` is GitHub's own analyzer and has no GitLab entry, so a GitLab
/// landing that names it lands nothing; the parameter resolution refuses
/// that pair by name before it gets here, as it refuses a binding outside
/// [`CODE_SCANNING_TECHS`].
pub const CODE_SCANNING_DESTINATIONS: [(&str, Provider); 3] = [
    (
        ".github/workflows/code-scanning-codeql.yml",
        Provider::CodeQl,
    ),
    (
        ".github/workflows/code-scanning-semgrep.yml",
        Provider::Semgrep,
    ),
    (".gitlab/ci/code-scanning-semgrep.yml", Provider::Semgrep),
];

/// The bindings that ship a code scanning workflow.
///
/// A scanner reads one language: the `CodeQL` arm fixes `languages: rust` and
/// both Semgrep arms name the `p/rust` ruleset, so the sources live in the
/// rust pairs rather than in a forge's technology-independent shared zone.
/// A landing for any other binding refuses the capability by name instead of
/// recording a provider and writing nothing.
pub const CODE_SCANNING_TECHS: [&str; 1] = ["rust"];

/// Why the named code scanning provider cannot run at this
/// `(technology, forge)`, or `None` where it can or none is named.
///
/// One owner for the question, because two callers ask it and they must not
/// disagree. Parameter resolution turns a reason into a refusal, so `init`,
/// `upgrade`, `adopt`, and `stage` never record an answer they cannot honour.
/// The projection reports the same reason as a record defect, because
/// `Params::from_record` cannot fail and a hand-edited or foreign receipt
/// reaches `rk status` through it: without this, a receipt naming a provider
/// whose pair ships nothing would project nothing, name nothing, and read as
/// clean.
#[must_use]
pub fn code_scanning_incompatibility(
    provider: Option<Provider>,
    driver: Option<&str>,
    forge: Option<&str>,
) -> Option<String> {
    let named = provider?;
    let Some(driver) = driver else {
        return Some(format!(
            "a scanner reads the release driver's language, and the profile names no automatic release driver; the bindings that carry one are: {}",
            CODE_SCANNING_TECHS.join(", ")
        ));
    };
    if !CODE_SCANNING_TECHS.contains(&driver) {
        return Some(format!(
            "the {driver} binding ships no code scanning workflow; a scanner reads one language, and the bindings that carry one are: {}",
            CODE_SCANNING_TECHS.join(", ")
        ));
    }
    let Some(forge) = forge else {
        return Some(
            "a code scanning workflow runs at a forge, and the profile names none".to_owned(),
        );
    };
    if named == Provider::CodeQl && forge != "github" {
        return Some(format!(
            "codeql is GitHub's own analyzer and the {forge} pair ships no workflow for it; pass --code-scanning semgrep"
        ));
    }
    None
}

/// The SPDX identifiers this convention recognizes as OSI-approved, sorted.
///
/// Lowercase, because the comparison is case-insensitive: SPDX asks for that
/// and cargo accepts any casing.
///
/// A closed list rather than a parse of the OSI register: the register moves
/// and this binary reads no network, so an identifier absent here is
/// unrecognized rather than rejected, and the refusal says so. Every entry
/// is an OSI-approved licence that appears on published Rust crates.
const OSI_APPROVED: [&str; 18] = [
    "0bsd",
    "agpl-3.0",
    "agpl-3.0-only",
    "agpl-3.0-or-later",
    "apache-2.0",
    "bsd-2-clause",
    "bsd-3-clause",
    "bsl-1.0",
    "epl-2.0",
    "gpl-2.0",
    "gpl-2.0-only",
    "gpl-2.0-or-later",
    "gpl-3.0",
    "gpl-3.0-only",
    "gpl-3.0-or-later",
    "isc",
    "mit",
    "mpl-2.0",
];

/// Every SPDX exception identifier, lowercase.
///
/// SPDX requires the right operand of `WITH` to be a `<license-exception-id>`,
/// so an operand outside this list makes the expression malformed and the
/// licence unread. The whole register rather than a subset, because a subset
/// refuses a legitimate crate: `GPL-2.0-only WITH GCC-exception-2.0` names a
/// real exception, and a reader carrying only the newer `GCC-exception-3.1`
/// would refuse it.
///
/// An exception grants permission rather than withdrawing it, so none of these
/// changes whether the left operand is OSI-approved. It is validated because a
/// reader that cannot parse the expression has not read the licence.
///
/// Taken from the SPDX license-list-data exception register, 86 identifiers, on
/// 2026-09-15. A later addition upstream is a patch here, and the refusal names
/// the operand it did not recognize.
const SPDX_EXCEPTIONS: [&str; 86] = [
    "389-exception",
    "asterisk-exception",
    "asterisk-linking-protocols-exception",
    "autoconf-exception-2.0",
    "autoconf-exception-3.0",
    "autoconf-exception-generic",
    "autoconf-exception-generic-3.0",
    "autoconf-exception-macro",
    "bison-exception-1.24",
    "bison-exception-2.2",
    "bootloader-exception",
    "cgal-linking-exception",
    "classpath-exception-2.0",
    "classpath-exception-2.0-short",
    "clisp-exception-2.0",
    "cryptsetup-openssl-exception",
    "digia-qt-lgpl-exception-1.1",
    "digirule-foss-exception",
    "ecos-exception-2.0",
    "erlang-otp-linking-exception",
    "fawkes-runtime-exception",
    "fltk-exception",
    "fmt-exception",
    "font-exception-2.0",
    "freertos-exception-2.0",
    "gcc-exception-2.0",
    "gcc-exception-2.0-note",
    "gcc-exception-3.1",
    "gmsh-exception",
    "gnat-exception",
    "gnome-examples-exception",
    "gnu-compiler-exception",
    "gnu-javamail-exception",
    "google-patent-webm",
    "gpl-3.0-389-ds-base-exception",
    "gpl-3.0-interface-exception",
    "gpl-3.0-linking-exception",
    "gpl-3.0-linking-source-exception",
    "gpl-cc-1.0",
    "gstreamer-exception-2005",
    "gstreamer-exception-2008",
    "harbour-exception",
    "i2p-gpl-java-exception",
    "independent-modules-exception",
    "kicad-libraries-exception",
    "kvirc-openssl-exception",
    "lgpl-3.0-linking-exception",
    "libpri-openh323-exception",
    "libtool-exception",
    "linux-syscall-note",
    "llgpl",
    "llvm-exception",
    "lzma-exception",
    "mif-exception",
    "mxml-exception",
    "nokia-qt-exception-1.1",
    "ocaml-lgpl-linking-exception",
    "occt-exception-1.0",
    "openjdk-assembly-exception-1.0",
    "openvpn-openssl-exception",
    "pcre2-exception",
    "polyparse-exception",
    "ps-or-pdf-font-exception-20170817",
    "qpl-1.0-inria-2004-exception",
    "qt-gpl-exception-1.0",
    "qt-lgpl-exception-1.1",
    "qwt-exception-1.0",
    "romic-exception",
    "rrdtool-floss-exception-2.0",
    "rsync-linking-exception",
    "sane-exception",
    "shl-2.0",
    "shl-2.1",
    "simple-library-usage-exception",
    "spelling-provider-lgpl-exception",
    "sqlitestudio-openssl-exception",
    "stunnel-exception",
    "swi-exception",
    "swift-exception",
    "texinfo-exception",
    "u-boot-exception-2.0",
    "ubdl-exception",
    "universal-foss-exception-1.0",
    "vsftpd-openssl-exception",
    "wxwindows-exception-3.1",
    "x11vnc-openssl-exception",
];

/// Whether `identifier` is in `list`, matched the way SPDX asks for it.
///
/// Without regard to case: SPDX states that an identifier "should be matched
/// in a case-insensitive manner", and cargo accepts `license = "mit"` without
/// complaint, so a real crate can carry any casing and a case-sensitive
/// comparison would refuse a licence it recognizes.
fn listed(list: &[&str], identifier: &str) -> bool {
    let lowered = identifier.to_ascii_lowercase();
    list.contains(&lowered.as_str())
}

/// One token of an SPDX licence expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token<'a> {
    /// An opening parenthesis.
    Open,
    /// A closing parenthesis.
    Close,
    /// The conjunction: the codebase is offered under both terms at once.
    And,
    /// The disjunction: the reader chooses one term.
    Or,
    /// The exception operator, whose right side names an exception rather
    /// than a licence.
    With,
    /// A licence identifier, a licence reference, or an exception
    /// identifier. Which one it is depends on its position.
    Identifier(&'a str),
}

/// The tokens of one SPDX expression, or `None` where a character can begin
/// no token.
///
/// The identifier alphabet is SPDX's own plus `:`, which a
/// `DocumentRef-...:LicenseRef-...` reference carries. A reference
/// tokenizes and then simply matches no approved identifier, which is the
/// honest answer: it names a licence whose text lives outside the register.
fn tokenize(expression: &str) -> Option<Vec<Token<'_>>> {
    let mut tokens = Vec::new();
    let bytes = expression.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if byte.is_ascii_whitespace() {
            at += 1;
            continue;
        }
        if byte == b'(' {
            tokens.push(Token::Open);
            at += 1;
            continue;
        }
        if byte == b')' {
            tokens.push(Token::Close);
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() {
            let byte = bytes[at];
            if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+' | b':') {
                at += 1;
            } else {
                break;
            }
        }
        if at == start {
            return None;
        }
        let word = &expression[start..at];
        tokens.push(match word {
            "AND" => Token::And,
            "OR" => Token::Or,
            "WITH" => Token::With,
            other => Token::Identifier(other),
        });
    }
    Some(tokens)
}

/// A recursive-descent reader over one tokenized SPDX expression.
///
/// It answers two questions in one pass, and both must hold: whether the
/// expression is well formed, and whether every licence identifier in it is
/// OSI-approved. A malformed expression answers `None` rather than falling
/// back on the identifiers it happened to contain, because an expression
/// nobody can parse states no terms at all.
struct Spdx<'a> {
    tokens: &'a [Token<'a>],
    at: usize,
}

impl<'a> Spdx<'a> {
    /// The token at the cursor, without consuming it.
    fn peek(&self) -> Option<Token<'a>> {
        self.tokens.get(self.at).copied()
    }

    /// The token at the cursor, consumed.
    fn bump(&mut self) -> Option<Token<'a>> {
        let token = self.peek()?;
        self.at += 1;
        Some(token)
    }

    /// One expression: operands joined by `AND` and `OR`, each operand a
    /// parenthesized expression or a simple licence.
    ///
    /// Both operators require every operand to be approved. A disjunction
    /// offers the reader a choice, so one unapproved term is a term the
    /// reader may take.
    fn expression(&mut self) -> Option<bool> {
        let mut approved = self.operand()?;
        while matches!(self.peek(), Some(Token::And | Token::Or)) {
            self.bump();
            approved = self.operand()? && approved;
        }
        Some(approved)
    }

    /// One operand: a parenthesized expression, or a licence identifier
    /// optionally carrying `WITH` and an exception identifier.
    ///
    /// A `+` suffix reads as the bare identifier, which is what the
    /// deprecated `GPL-3.0+` form means. The operand after `WITH` must be an
    /// exception identifier this release recognizes, and it changes no
    /// verdict: an exception grants permission rather than withdrawing it, so
    /// which licence the codebase is offered under is answered by the left
    /// operand alone.
    fn operand(&mut self) -> Option<bool> {
        match self.bump()? {
            Token::Open => {
                let inner = self.expression()?;
                (self.bump()? == Token::Close).then_some(inner)
            }
            Token::Identifier(name) => {
                let identifier = name.strip_suffix('+').unwrap_or(name);
                let approved = listed(&OSI_APPROVED, identifier);
                if self.peek() == Some(Token::With) {
                    self.bump();
                    // SPDX requires an exception identifier here, so an
                    // operand this release does not recognize leaves the
                    // expression malformed and the licence unread.
                    match self.bump()? {
                        Token::Identifier(exception) if listed(&SPDX_EXCEPTIONS, exception) => {}
                        _ => return None,
                    }
                }
                Some(approved)
            }
            Token::And | Token::Or | Token::With | Token::Close => None,
        }
    }
}

/// Whether one SPDX expression is well formed and every licence in it is
/// OSI-approved.
///
/// Both conditions, and the pair is the point. A malformed expression is
/// refused rather than read for the identifiers it contains, because
/// `MIT OR` names one licence and no complete offer, and accepting it would
/// land a workflow whose provider's terms nothing established.
#[must_use]
pub fn licence_is_osi_approved(expression: &str) -> bool {
    let Some(tokens) = tokenize(expression) else {
        return false;
    };
    let mut reader = Spdx {
        tokens: &tokens,
        at: 0,
    };
    let Some(approved) = reader.expression() else {
        return false;
    };
    approved && reader.at == tokens.len()
}

/// Why the code scanning capability's licence condition refuses this
/// target, or `None` where no condition applies or the licence satisfies
/// it.
///
/// Only `codeql` carries a condition: its terms cover an open-source
/// codebase, and a run over anything else needs a paid seat, so a landing
/// that guessed would write a licence violation. Semgrep CE carries none,
/// and is the fallback the refusal names.
///
/// The licence is read where the binding declares it. For `rust` that is
/// the `license` field of `Cargo.toml`. A manifest that names a
/// `license-file` instead states no identifier this judgment can read, so
/// the pair refuses rather than guessing at the file's contents.
#[must_use]
pub fn code_scanning_licence_refusal(
    provider: Option<Provider>,
    driver: Option<&str>,
    shape: &CrateShape,
) -> Option<String> {
    if provider != Some(Provider::CodeQl) {
        return None;
    }
    if driver != Some("rust") {
        return Some(format!(
            "the {} binding declares no licence field this release reads, and codeql's terms cover an open-source codebase alone; land --code-scanning semgrep, which carries no licence condition",
            driver.unwrap_or("release-less")
        ));
    }
    let fallback = "land --code-scanning semgrep, which carries no licence condition";
    let Some(text) = shape.cargo_toml.as_deref() else {
        return Some(format!(
            "the target has no readable Cargo.toml, so the licence codeql's terms depend on cannot be read; {fallback}"
        ));
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Some(format!(
            "the target's Cargo.toml does not parse, so the licence codeql's terms depend on cannot be read; {fallback}"
        ));
    };
    let licence = table
        .get("package")
        .and_then(toml::Value::as_table)
        .and_then(|package| package.get("license"))
        .and_then(toml::Value::as_str);
    match licence {
        None => Some(format!(
            "the target's Cargo.toml declares no license field, and codeql's terms cover an open-source codebase alone; declare one, or {fallback}"
        )),
        Some(expression) if !licence_is_osi_approved(expression) => Some(format!(
            "the target's license, {expression}, is not one this release recognizes as OSI-approved, and codeql's terms cover an open-source codebase alone; {fallback}"
        )),
        Some(_) => None,
    }
}

/// The declared kind of a destination, or `None` for a file the sources
/// does not classify.
#[must_use]
pub fn kind_of(destination: &str) -> Option<Kind> {
    if BLOCK_DESTINATIONS.contains(&destination) {
        return Some(Kind::Rendered);
    }
    KINDS
        .iter()
        .find(|(name, _)| *name == destination)
        .map(|(_, kind)| *kind)
}

/// Every destination the embedded sources can land, in declaration order.
///
/// The whole files and the three block destinations. The classification
/// reads it to ask whether a destination is already present at a target.
pub fn destinations() -> impl Iterator<Item = &'static str> {
    KINDS
        .iter()
        .map(|(name, _)| *name)
        .chain(BLOCK_DESTINATIONS)
}

/// The mechanical substitution sites in `rendered` files.
///
/// Known values, substituted identically everywhere each appears. The
/// owner is derived from the landing's `repo` parameter and the scope
/// shape from [`SCOPE_SHAPE`], so the landed bytes stay a deterministic
/// function of the embedded sources plus parameters.
pub const OWNER_TOKEN: &[u8] = b"OWNER";

/// The repository a preview stands in for where nothing answered.
///
/// It is a placeholder, never a project path: a plan that would render
/// it into a target is blocked, and only a preview may carry it.
pub const REPO_PLACEHOLDER: &str = "OWNER";

/// The full recorded project path, including nested namespaces.
pub const REPO_TOKEN: &[u8] = b"RK_REPO";

/// The one scope shape: the title checks' regular expression.
pub const SCOPE_SHAPE_TOKEN: &[u8] = b"RK_SCOPE_SHAPE";

/// The recorded release style: `trunk` arms the bot's request in the
/// landed release workflow, `lines` leaves every request unarmed.
pub const STYLE_TOKEN: &[u8] = b"RK_STYLE";

/// The one permanent branch. A landed release trigger, ref guard, and
/// branch guard each name it, so a target whose trunk is not `master`
/// needs its own answer in its own bytes.
pub const TRUNK_BRANCH_TOKEN: &[u8] = b"RK_TRUNK_BRANCH";

/// The release-line branch prefix, naming the lines a release trigger
/// accepts beside the trunk.
pub const LINE_PREFIX_TOKEN: &[u8] = b"RK_LINE_PREFIX";

/// The same prefix, escaped for a slash-delimited regular expression.
///
/// A GitLab rule names a line that way, and a raw `release/` would close
/// the delimiter and break the pipeline, so the two forms are two tokens.
/// This one substitutes first: the plain token is its own prefix.
pub const LINE_PREFIX_RE_TOKEN: &[u8] = b"RK_LINE_PREFIX_RE";

/// The three replaceable spans of a landed security policy, each as its
/// ordered begin and end marker.
///
/// A span is not a token. Each forge's policy carries its own authored
/// prose inside the markers, so a landing that answers neither security
/// parameter strips the markers and reproduces the file the forge's
/// snippet states, byte for byte and in that forge's own words. A landing
/// that answers one replaces the interior of the spans that fact belongs
/// to. The markers are HTML comments because the snippet is Markdown a
/// reader may open before it is ever rendered.
pub const SECURITY_SPANS: [(&[u8], &[u8]); 3] = [
    (
        b"<!--RK_SECURITY_CONTACT_BEGIN-->",
        b"<!--RK_SECURITY_CONTACT_END-->",
    ),
    (
        b"<!--RK_SECURITY_RESPONSE_BEGIN-->",
        b"<!--RK_SECURITY_RESPONSE_END-->",
    ),
    (
        b"<!--RK_SECURITY_DEADLINE_BEGIN-->",
        b"<!--RK_SECURITY_DEADLINE_END-->",
    ),
];

/// The sentence a policy with an acknowledgment window states in place of
/// the forge's best-effort wording.
fn acknowledgment(response: &str) -> String {
    format!("Maintainers acknowledge a report within {response}.")
}

/// What a policy with an acknowledgment window says about deadlines: the
/// authored sentence disclaims a response deadline, which a stated window
/// contradicts, so only the disclosure half survives.
const DISCLOSURE_ONLY: &[u8] = b"This policy commits to no disclosure deadline.";

/// The replacement for each span under one parameter set, or `None` where
/// the forge's authored interior stands.
fn security_replacements(params: &Params) -> [Option<Vec<u8>>; 3] {
    let contact = (!params.security_contact().is_empty())
        .then(|| params.security_contact().as_bytes().to_vec());
    let promised = params.security_response() != crate::config::RESPONSE_DEFAULT;
    [
        contact,
        promised.then(|| acknowledgment(params.security_response()).into_bytes()),
        promised.then(|| DISCLOSURE_ONLY.to_vec()),
    ]
}

/// One marked span replaced, or the markers alone removed.
///
/// Exactly one ordered begin and end pair is a span; anything else is a
/// source defect a test holds, so this leaves such bytes untouched rather
/// than growing a runtime failure mode into every rendered file.
fn replace_span(baseline: &[u8], begin: &[u8], end: &[u8], value: Option<&[u8]>) -> Vec<u8> {
    let ordered = find(baseline, begin)
        .zip(find(baseline, end))
        .filter(|(start, stop)| stop > start);
    let Some((start, stop)) = ordered else {
        return baseline.to_vec();
    };
    let mut out = Vec::with_capacity(baseline.len());
    out.extend_from_slice(&baseline[..start]);
    out.extend_from_slice(value.unwrap_or_else(|| &baseline[start + begin.len()..stop]));
    out.extend_from_slice(&baseline[stop + end.len()..]);
    out
}

/// Substitute the landing parameters into a `rendered` file's bytes.
///
/// The repository's owner, the project path's first segment, replaces
/// every `OWNER` occurrence; the full path replaces `RK_REPO` last. The
/// one scope shape replaces the scope token, and the recorded style
/// replaces the style token. The scope shape rests on no parameter, so it
/// substitutes always. An unresolved style leaves its token standing,
/// which only a preview renders under: an apply refuses before reaching
/// here.
///
/// The trunk and the line prefix substitute from the same parameters, so
/// a target that renames either carries the new name in every artifact
/// that names it rather than in the binary's behavior alone.
///
/// The security policy's marked spans resolve last, after every token, so
/// a contact that happens to spell a token name lands literally rather
/// than being read as one more substitution site.
#[must_use]
pub fn render(baseline: &[u8], params: &Params) -> Vec<u8> {
    let repo = params.repo();
    let owner = repo.split('/').next().unwrap_or(repo);
    let mut out = substitute(baseline, OWNER_TOKEN, owner.as_bytes());
    if let Some(style) = params.style() {
        out = substitute(&out, STYLE_TOKEN, style.as_str().as_bytes());
    }
    out = substitute(&out, SCOPE_SHAPE_TOKEN, SCOPE_SHAPE.as_bytes());
    out = substitute(&out, TRUNK_BRANCH_TOKEN, params.trunk().as_bytes());
    let escaped = params.line_prefix().replace('/', "\\/");
    out = substitute(&out, LINE_PREFIX_RE_TOKEN, escaped.as_bytes());
    out = substitute(&out, LINE_PREFIX_TOKEN, params.line_prefix().as_bytes());
    out = substitute(&out, REPO_TOKEN, repo.as_bytes());
    for ((begin, end), value) in SECURITY_SPANS.iter().zip(security_replacements(params)) {
        out = replace_span(&out, begin, end, value.as_deref());
    }
    out
}

/// Every `token` occurrence replaced with `value`.
#[must_use]
pub fn substitute(baseline: &[u8], token: &[u8], value: &[u8]) -> Vec<u8> {
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

/// The destination the glossary block splices into.
///
/// The document is the target's own vocabulary, so the block shares
/// `AGENTS.md`'s marker pair and owns nothing outside it.
pub const GLOSSARY_DESTINATION: &str = "GLOSSARY.md";

/// The destination the hook block splices into.
pub const HOOKS_DESTINATION: &str = ".pre-commit-config.yaml";

/// Every block destination, in the order a landing writes them.
///
/// A block destination owns the lines between its markers and nothing
/// else, so every verb that asks whether a destination is block-placed
/// reads this one list.
pub const BLOCK_DESTINATIONS: [&str; 3] =
    [AGENTS_DESTINATION, GLOSSARY_DESTINATION, HOOKS_DESTINATION];

/// The hook block's opening marker, a YAML comment at column zero.
pub const HOOKS_BEGIN: &str = "# BEGIN release-kit";

/// The hook block's closing marker.
pub const HOOKS_END: &str = "# END release-kit";

/// The top-level key the fresh hook file carries and the skills verify on
/// an existing one: the commit-msg and pre-push hooks run only where their
/// hook types are installed.
pub const HOOK_TYPES_LINE: &str = "default_install_hook_types: [pre-commit, commit-msg, pre-push]";

/// The authored routing-block template.
pub const AGENTS_BLOCK: &str = "blocks/agents-block.md.in";

/// The authored glossary template.
pub const GLOSSARY_BLOCK: &str = "blocks/glossary.md.in";

/// The routing block's mode line, worktree form.
pub const AGENTS_LINE_WORKTREE: &str = "blocks/agents-line-worktree.md.in";

/// The routing block's mode line, branches form.
pub const AGENTS_LINE_BRANCHES: &str = "blocks/agents-line-branches.md.in";

/// The authored hook-block template.
pub const PRE_COMMIT_BLOCK: &str = "blocks/pre-commit-block.yaml.in";

/// The worktree mode's guard entry.
pub const PRE_COMMIT_WORKTREE_GUARD: &str = "blocks/pre-commit-worktree-guard.yaml.in";

/// The routing block's mode line for one checkout mode.
#[must_use]
pub const fn routing_line(mode: CheckoutMode) -> &'static str {
    match mode {
        CheckoutMode::LinkedWorktree => AGENTS_LINE_WORKTREE,
        CheckoutMode::MainWorktree => AGENTS_LINE_BRANCHES,
    }
}

/// One authored block this binary embeds, as text, by its embedded path.
///
/// # Errors
///
/// [`RkError::Other`] for a block this binary does not embed or one that
/// is not UTF-8, both defects in the binary.
pub fn embedded_block(path: &str) -> Result<&'static str, RkError> {
    let name = path.strip_prefix("blocks/").unwrap_or(path);
    let file = embedded::BLOCKS
        .get_file(name)
        .ok_or_else(|| anyhow::anyhow!("{path}: this binary embeds no such block"))?;
    std::str::from_utf8(file.contents())
        .map_err(|_| anyhow::anyhow!("{path}: a block is UTF-8").into())
}

/// An authored block without the one final newline the repository's
/// hooks enforce on every file under `blocks/`; a test in
/// `src/embedded.rs` holds each file to exactly one.
#[must_use]
pub fn authored(text: &str) -> &str {
    text.strip_suffix('\n').unwrap_or(text)
}

/// The one branch grammar.
///
/// The extended regular expression the landed `rk-branch-name` hook
/// tests, and the same anchored language `rk worktree add` validates
/// before creating anything. One owner by token: `concat!` cannot
/// interpolate a const, so [`compose_hooks`] substitutes it for the
/// template's `RK_BRANCH_GRAMMAR` token.
pub const BRANCH_GRAMMAR: &str = r"^((build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)/[A-Za-z0-9._/-]+|([0-9]+|[A-Z][A-Z0-9]+-[0-9]+)-[A-Za-z0-9._-]+|release[-/].+)$";

/// The one commit scope shape.
///
/// A bracket expression, lowercase, admitting the digits and `_ . / -`
/// beside the letters, so `area/subarea` reads as one scope. It holds the
/// shape of a scope and never its vocabulary: the word itself is the
/// author's, guided by the routing block and by the repository's own
/// history. One owner by token: the title checks take it as
/// `RK_SCOPE_SHAPE` through [`render`], and `rk message --check` reads it
/// directly, so the desk and the forge judge one language.
pub const SCOPE_SHAPE: &str = "[a-z0-9._/-]+";

/// Whether one scope matches [`SCOPE_SHAPE`].
///
/// The predicate and the pattern are one owner, so the desk's judgment
/// cannot drift from the forge's: `rk message --check` calls this, the
/// title checks render the pattern, and a test holds the two equal over
/// every ASCII character.
#[must_use]
pub fn scope_is_shaped(scope: &str) -> bool {
    !scope.is_empty()
        && scope.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '/' | '-')
        })
}

/// The routing block from its authored template and the mode's one
/// orientation line.
///
/// Markers included, without a trailing newline and with its scope token
/// unrendered. Everything but the substituted line, the agent-boundary
/// line included, is byte-identical across modes.
#[must_use]
pub fn compose_routing(template: &str, line: &str) -> String {
    authored(template).replacen("RK_WORKFLOW_LINE", authored(line), 1)
}

/// The glossary block from its authored template: markers included and
/// without a trailing newline. It carries no token and no mode, so the
/// same bytes land in every target.
#[must_use]
pub fn compose_glossary(template: &str) -> String {
    authored(template).to_owned()
}

/// The hook block from its authored template, with the worktree mode's
/// guard entry where `guard` carries one.
///
/// `Some` is the worktree mode: the block carries the location guard and
/// names the sweep-skip pair. `None` is the branches mode: no guard entry
/// at all, never an entry that reads local state to decide whether to
/// enforce. The one branch grammar substitutes from [`BRANCH_GRAMMAR`].
/// Markers included, without a trailing newline and with its scope token
/// unrendered.
#[must_use]
pub fn compose_hooks(template: &str, guard: Option<&str>) -> String {
    let (guard, skip) = guard.map_or_else(
        || (String::new(), "no-commit-to-branch"),
        |entry| {
            (
                format!("{}\n", authored(entry)),
                "no-commit-to-branch,rk-worktree-location",
            )
        },
    );
    authored(template)
        .replacen("RK_BRANCH_GRAMMAR", BRANCH_GRAMMAR, 1)
        .replacen("RK_SWEEP_SKIP", skip, 1)
        .replacen("RK_WORKTREE_GUARD", &guard, 1)
}

/// The routing block for one checkout mode, from this binary's embedded
/// templates.
///
/// # Errors
///
/// A block this binary does not embed, a defect in the binary.
pub fn routing_block(mode: CheckoutMode) -> Result<String, RkError> {
    Ok(compose_routing(
        embedded_block(AGENTS_BLOCK)?,
        embedded_block(routing_line(mode))?,
    ))
}

/// The glossary block from this binary's embedded template.
///
/// # Errors
///
/// A block this binary does not embed, a defect in the binary.
pub fn glossary_block() -> Result<String, RkError> {
    Ok(compose_glossary(embedded_block(GLOSSARY_BLOCK)?))
}

/// The hook block for one checkout mode, from this binary's embedded
/// templates.
///
/// # Errors
///
/// A block this binary does not embed, a defect in the binary.
pub fn hooks_block(mode: CheckoutMode) -> Result<String, RkError> {
    let guard = match mode {
        CheckoutMode::LinkedWorktree => Some(embedded_block(PRE_COMMIT_WORKTREE_GUARD)?),
        CheckoutMode::MainWorktree => None,
    };
    Ok(compose_hooks(embedded_block(PRE_COMMIT_BLOCK)?, guard))
}

/// The unrendered block for one block destination under one checkout mode,
/// with the embedded source paths it was composed from.
fn block_template(destination: &str, mode: CheckoutMode) -> Result<(String, Vec<String>), RkError> {
    match destination {
        AGENTS_DESTINATION => Ok((
            routing_block(mode)?,
            vec![AGENTS_BLOCK.to_owned(), routing_line(mode).to_owned()],
        )),
        GLOSSARY_DESTINATION => Ok((glossary_block()?, vec![GLOSSARY_BLOCK.to_owned()])),
        HOOKS_DESTINATION => {
            let mut sources = vec![PRE_COMMIT_BLOCK.to_owned()];
            if mode == CheckoutMode::LinkedWorktree {
                sources.push(PRE_COMMIT_WORKTREE_GUARD.to_owned());
            }
            Ok((hooks_block(mode)?, sources))
        }
        other => Err(anyhow::anyhow!("{other} is not a block destination").into()),
    }
}

/// The markers of a block destination, or `None` for a whole-file one.
#[must_use]
pub fn block_markers(destination: &str) -> Option<(&'static str, &'static str)> {
    match destination {
        AGENTS_DESTINATION | GLOSSARY_DESTINATION => Some((BLOCK_BEGIN, BLOCK_END)),
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

/// The whole document's bytes after splicing a marked block into it.
///
/// A fresh file where none exists, the block replaced in place where one
/// is marked, appended after the target's own content otherwise:
/// release-kit owns the lines inside the markers, not the document. Both
/// markdown destinations take this shape, `AGENTS.md` and the glossary.
#[must_use]
pub fn splice_marked_block(existing: Option<&[u8]>, block: &str) -> Vec<u8> {
    let block = block.as_bytes();
    let Some(text) = existing else {
        return [block, b"\n"].concat();
    };
    // Bytes, never text: the document belongs to the target and a decode
    // that replaces one invalid sequence rewrites a byte outside the
    // markers, which is the one thing a block destination never does.
    if let Some(start) = find(text, BLOCK_BEGIN.as_bytes())
        && let Some(offset) = find(&text[start..], BLOCK_END.as_bytes())
    {
        let stop = start + offset + BLOCK_END.len();
        return [&text[..start], block, &text[stop..]].concat();
    }
    // Appending keeps every byte the target wrote, trailing blank lines
    // and an absent final newline included. The only addition is the
    // separator that opens the block's own line.
    let mut out = Vec::with_capacity(text.len() + block.len() + 3);
    out.extend_from_slice(text);
    if !text.ends_with(b"\n") {
        out.push(b'\n');
    }
    out.push(b'\n');
    out.extend_from_slice(block);
    out.push(b'\n');
    out
}

/// The whole `.pre-commit-config.yaml` content after splicing the
/// rendered hook block.
///
/// A fresh file carries the hook-types key, the `repos:` key, and the
/// block; a marked file takes the block in place; an unmarked file takes
/// it directly under its `repos:` line, above the target's own hooks. An
/// unmarked file with no `repos:` line is refused by name: the block's
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

/// The one definition of an ill-formed block document, shared by every
/// splice and every reader that judges one: `None` for a whole-file
/// destination or a well-formed document.
///
/// Ownership must be unambiguous: exactly one begin marker paired with
/// exactly one end marker after it, or none of either. A second begin is
/// a second block, which for the hook file pre-commit would still run,
/// and a marker without its pair, or an end before its begin, is a block
/// whose extent nothing can state.
#[must_use]
pub fn marker_defect(destination: &str, text: &str) -> Option<String> {
    let (begin, end) = block_markers(destination)?;
    let begins = text.matches(begin).count();
    let ends = text.matches(end).count();
    if begins > 1 || ends > 1 {
        return Some(format!(
            "{destination} carries more than one release-kit marker pair; release-kit owns exactly one block"
        ));
    }
    match (text.find(begin), text.find(end)) {
        (Some(begin), Some(end)) if end > begin => None,
        (None, None) => None,
        _ => Some(format!(
            "{destination} carries an unmatched or misordered release-kit marker, so the block's extent is ambiguous"
        )),
    }
}

/// [`marker_defect`] for the hook file, the destination whose entries
/// execute.
#[must_use]
pub fn hooks_marker_defect(text: &str) -> Option<String> {
    marker_defect(HOOKS_DESTINATION, text)
}

/// The complete proposed document for one block destination: the
/// rendered `region` spliced into the `existing` document the evidence
/// carries, or the reason the document offers it no place.
fn propose_document(
    destination: &str,
    existing: Option<&[u8]>,
    region: &[u8],
) -> Result<Vec<u8>, String> {
    // The block is release-kit's own text. The document is the target's
    // bytes: the markdown splice works on them directly, and the marker
    // judgment decodes a copy only to count ASCII markers, which a
    // replacement character neither creates nor hides.
    let block = String::from_utf8_lossy(region).into_owned();
    if let Some(text) = existing
        && let Some(defect) = marker_defect(destination, &String::from_utf8_lossy(text))
    {
        return Err(defect);
    }
    if destination == HOOKS_DESTINATION {
        // The hook splice is line-based text, so a document that is not
        // UTF-8 has no honest place for the block: a lossy decode would
        // rewrite a byte outside the markers, which a region never does.
        let text = match existing {
            None => None,
            Some(bytes) => Some(std::str::from_utf8(bytes).map_err(|_| {
                format!(
                    "{destination} is not UTF-8, so the hook block has nowhere to land without rewriting the target's bytes"
                )
            })?),
        };
        return splice_hooks_block(text, &block).map(String::into_bytes);
    }
    Ok(splice_marked_block(existing, &block))
}

/// Why the whole Nix capability stays out of a landing, or `None` where
/// the target's crate shape supports the seed.
///
/// The gate holds every structural prerequisite the seed relies on, not
/// only evaluation: the package expression reads `Cargo.toml` through
/// `importTOML` and throws without `../Cargo.lock`, and the seed flake's
/// smoke check runs the crate's binary, which only an implicit
/// `src/main.rs` or an explicit `[[bin]]` entry produces. A shape
/// missing any of these would land files that fail on their first
/// evaluation or first check, so the landing reports the smaller product
/// with the missing piece named instead.
#[must_use]
pub fn nix_unsupported_shape(shape: &CrateShape) -> Option<String> {
    let Some(text) = shape.cargo_toml.as_deref() else {
        return Some(
            "the target has no readable Cargo.toml, which the seeded package expression reads; no Nix file lands".to_owned(),
        );
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Some(
            "the target's Cargo.toml does not parse, and the seeded package expression reads it; no Nix file lands".to_owned(),
        );
    };
    if !table.contains_key("package") {
        return Some(
            "the target's Cargo.toml has no [package] table; the seed supports a single crate, so no Nix file lands".to_owned(),
        );
    }
    if !shape.cargo_lock {
        return Some(
            "the target has no Cargo.lock, which the seeded package expression builds from; commit one, then opt in".to_owned(),
        );
    }
    let implicit_bin = shape.main_rs
        && table
            .get("package")
            .and_then(toml::Value::as_table)
            .and_then(|package| package.get("autobins"))
            .and_then(toml::Value::as_bool)
            != Some(false);
    let explicit_bins = table.get("bin").and_then(toml::Value::as_array);
    if explicit_bins.is_none() && !implicit_bin {
        return Some(
            "the target declares no binary — no effective src/main.rs and no [[bin]] entry — and the seed flake's smoke check runs one; no Nix file lands".to_owned(),
        );
    }
    // The seed's mainProgram is the first [[bin]] entry; one whose
    // required-features a default build does not enable produces no
    // executable, so the smoke check would fail on a green landing. A
    // requirement the default feature set covers builds normally and
    // passes.
    if let Some(bins) = explicit_bins {
        let required = bins
            .first()
            .and_then(toml::Value::as_table)
            .and_then(|bin| bin.get("required-features"))
            .and_then(toml::Value::as_array);
        if let Some(required) = required {
            let enabled = default_features(&table);
            let missing = required
                .iter()
                .filter_map(toml::Value::as_str)
                .any(|feature| !enabled.contains(feature));
            if missing {
                return Some(
                    "the target's first [[bin]] entry requires features a default build does not enable; no Nix file lands".to_owned(),
                );
            }
        }
    }
    None
}

/// Whether any feature's list carries a `dep:name` edge, which is what
/// suppresses the optional dependency's implicit same-named feature.
fn dep_edge_suppresses(features: &toml::Table, name: &str) -> bool {
    let edge = format!("dep:{name}");
    features.values().any(|list| {
        list.as_array().is_some_and(|entries| {
            entries
                .iter()
                .filter_map(toml::Value::as_str)
                .any(|entry| entry == edge)
        })
    })
}

/// Whether `name` is declared an optional dependency, in any of the
/// dependency tables a binary's build reads.
fn is_optional_dependency(table: &toml::Table, name: &str) -> bool {
    ["dependencies", "build-dependencies"]
        .iter()
        .any(|section| {
            table
                .get(*section)
                .and_then(toml::Value::as_table)
                .and_then(|dependencies| dependencies.get(name))
                .and_then(toml::Value::as_table)
                .and_then(|dependency| dependency.get("optional"))
                .and_then(toml::Value::as_bool)
                == Some(true)
        })
}

/// The features a default build enables: the `default` feature resolved
/// through the `[features]` table's own enables, an approximation of
/// cargo's default resolution for the documented supported shapes, erring
/// toward withholding where the semantics run deeper. Dependency forms,
/// `dep:name` and weak `name?/feature`, are not feature names here and are
/// skipped; the closure is bounded by the table's size.
fn default_features(table: &toml::Table) -> std::collections::BTreeSet<String> {
    let Some(features) = table.get("features").and_then(toml::Value::as_table) else {
        return std::collections::BTreeSet::new();
    };
    let mut enabled = std::collections::BTreeSet::new();
    let mut queue = vec!["default".to_owned()];
    while let Some(name) = queue.pop() {
        if !enabled.insert(name.clone()) {
            continue;
        }
        if let Some(implies) = features.get(&name).and_then(toml::Value::as_array) {
            for implied in implies.iter().filter_map(toml::Value::as_str) {
                if implied.starts_with("dep:") || implied.contains("?/") {
                    // `dep:name` enables the dependency without a feature
                    // of this crate; a weak `name?/feature` edge enables
                    // nothing by itself.
                    continue;
                }
                if let Some((package, _)) = implied.split_once('/') {
                    // A strong `name/feature` edge activates this crate's
                    // same-named feature only for an optional dependency,
                    // and only where that feature exists: declared
                    // explicitly, or implicit and not suppressed by a
                    // `dep:` edge anywhere in the table. A non-optional
                    // dependency's edge enables a feature of the
                    // dependency and nothing of this crate.
                    let feature_exists =
                        features.contains_key(package) || !dep_edge_suppresses(features, package);
                    if is_optional_dependency(table, package) && feature_exists {
                        queue.push(package.to_owned());
                    }
                } else {
                    queue.push(implied.to_owned());
                }
            }
        }
    }
    enabled
}

/// Why the flake half of the Nix capability stays out of this landing, or
/// `None` where the pair lands whole.
///
/// The pair is all-or-nothing: a target that already carries a
/// `flake.nix` or `flake.lock` of its own keeps its pair, because a seed
/// lock beside a foreign flake describes the wrong input graph. A pair
/// the record names is release-kit's own landing and is never withheld.
#[must_use]
pub fn flake_pair_withheld(
    flake_recorded: bool,
    flake_nix_present: bool,
    flake_lock_present: bool,
) -> Option<String> {
    if flake_recorded {
        return None;
    }
    let present: Vec<&str> = [
        ("flake.nix", flake_nix_present),
        ("flake.lock", flake_lock_present),
    ]
    .into_iter()
    .filter_map(|(name, present)| present.then_some(name))
    .collect();
    if present.is_empty() {
        return None;
    }
    Some(format!(
        "the target already carries {}; its flake pair stays its own",
        present.join(" and ")
    ))
}

/// The Nix destinations an opted-in landing withholds at this target, with
/// the one reason, or `None` where the capability lands whole or `nix` is
/// off.
///
/// An unsupported crate shape names the whole capability, and a flake
/// pair of the target's own names the pair while the seeded package
/// expression still lands. Every landing verb shares this one judgment, so
/// a stage, an apply, an upgrade, and an adoption all withhold
/// identically.
#[must_use]
pub fn nix_withholding(
    nix: bool,
    evidence: &TargetEvidence,
) -> Option<(&'static [&'static str], String)> {
    if !nix {
        return None;
    }
    if let Some(reason) = nix_unsupported_shape(&evidence.crate_shape) {
        return Some((&NIX_DESTINATIONS[..], reason));
    }
    flake_pair_withheld(
        evidence.flake_recorded,
        evidence.flake_nix_present,
        evidence.flake_lock_present,
    )
    .map(|reason| (&NIX_WITHHOLDABLE[..], reason))
}

#[cfg(test)]
mod tests {
    use super::{
        AGENTS_DESTINATION, BLOCK_BEGIN, BLOCK_DESTINATIONS, BLOCK_END, Candidate, Collision,
        CrateShape, GLOSSARY_DESTINATION, HOOK_TYPES_LINE, HOOKS_BEGIN, HOOKS_DESTINATION,
        HOOKS_END, Placement, Projection, ProjectionInput, TargetEvidence, extract_block,
    };
    use crate::landing::{Params, Style};

    /// A supported single-crate shape, so nothing is withheld.
    fn supported_shape() -> CrateShape {
        CrateShape {
            cargo_toml: Some("[package]\nname = \"widget\"\nversion = \"0.1.0\"\n".to_owned()),
            cargo_lock: true,
            main_rs: true,
        }
    }

    fn input(evidence: TargetEvidence) -> ProjectionInput {
        let mut params = Params::for_test("acme/widget", Some(Style::Trunk));
        params.set_nix_for_test(true);
        ProjectionInput { params, evidence }
    }

    fn compute(evidence: TargetEvidence) -> Projection {
        Projection::compute(&input(evidence)).expect("the embedded pair projects")
    }

    fn candidate<'a>(projection: &'a Projection, destination: &str) -> &'a Candidate {
        projection
            .candidates
            .iter()
            .find(|candidate| candidate.destination == destination)
            .expect("the destination projects")
    }

    /// The bytes of a document outside its one marked region.
    fn outside(bytes: &[u8], begin: &str, end: &str) -> (Vec<u8>, Vec<u8>) {
        let text = String::from_utf8_lossy(bytes);
        let start = text.find(begin).expect("the begin marker is present");
        let stop = text[start..].find(end).expect("the end marker is present") + start + end.len();
        (bytes[..start].to_vec(), bytes[stop..].to_vec())
    }

    #[test]
    fn equal_projection_inputs_yield_byte_identical_projections() {
        let mut documents = std::collections::BTreeMap::new();
        documents.insert(
            AGENTS_DESTINATION.to_owned(),
            b"# Widget\n\nOwn rules.\n".to_vec(),
        );
        let evidence = TargetEvidence {
            documents,
            crate_shape: supported_shape(),
            ..TargetEvidence::default()
        };
        let first = input(evidence.clone());
        let second = input(evidence);
        assert_eq!(first, second, "the inputs are values and compare equal");
        let a = Projection::compute(&first).expect("the pair projects");
        let b = Projection::compute(&second).expect("the pair projects");
        assert_eq!(a.candidates.len(), b.candidates.len());
        for (x, y) in a.candidates.iter().zip(&b.candidates) {
            assert_eq!(x.destination, y.destination);
            assert_eq!(x.kind, y.kind);
            assert_eq!(x.placement, y.placement);
            assert_eq!(x.bytes, y.bytes, "{}", x.destination);
            assert_eq!(x.region, y.region, "{}", x.destination);
            assert_eq!(x.sources, y.sources, "{}", x.destination);
        }
        assert_eq!(a, b);
        let destinations: Vec<&str> = a
            .candidates
            .iter()
            .map(|candidate| candidate.destination.as_str())
            .collect();
        let mut sorted = destinations.clone();
        sorted.sort_unstable();
        assert_eq!(destinations, sorted, "candidates sort by destination");
        assert!(a.omissions.is_empty(), "{:?}", a.omissions);
        assert!(a.collisions.is_empty(), "{:?}", a.collisions);
    }

    /// The Scorecard destination is classified, gated by its parameter
    /// alone, and rendered: the pure projection answers the capability
    /// with no target read and no forge call.
    #[test]
    fn the_scorecard_destination_projects_only_under_the_opt_in() {
        use super::{Kind, SCORECARD_DESTINATIONS, kind_of};
        let destination = SCORECARD_DESTINATIONS[0];
        assert_eq!(kind_of(destination), Some(Kind::Rendered));

        let project = |scorecard: bool| {
            let mut params = Params::for_test("acme/widget", Some(Style::Trunk));
            params.set_scorecard_for_test(scorecard);
            Projection::compute(&ProjectionInput {
                params,
                evidence: TargetEvidence::default(),
            })
            .expect("the embedded pair projects")
        };

        let off = project(false);
        assert!(
            !off.candidates
                .iter()
                .any(|candidate| candidate.destination == destination),
            "off by default, and an absent candidate is no omission"
        );
        assert!(off.omissions.is_empty(), "{:?}", off.omissions);

        let on = project(true);
        let candidate = candidate(&on, destination);
        assert_eq!(candidate.kind, Kind::Rendered);
        assert_eq!(candidate.placement, Placement::Whole);
        let text = String::from_utf8_lossy(&candidate.bytes);
        assert!(!text.contains("RK_"), "a token survived: {text}");
    }

    /// The licence judgment over the expression forms a crate manifest
    /// uses: every operand must be recognized, a disjunction of approved
    /// terms passes, and one unapproved operand anywhere fails.
    #[test]
    fn the_licence_judgment_reads_every_operand() {
        use super::licence_is_osi_approved as approved;
        for expression in [
            "MIT",
            "Apache-2.0",
            "MIT OR Apache-2.0",
            "MIT AND Apache-2.0",
            "(MIT OR Apache-2.0) AND ISC",
            "Apache-2.0 WITH LLVM-exception OR MIT",
            "GPL-3.0+",
        ] {
            assert!(approved(expression), "{expression} is OSI-approved");
        }
        for expression in [
            "",
            "   ",
            "LicenseRef-proprietary",
            "MIT AND LicenseRef-proprietary",
            "SEE LICENSE IN COPYING",
            "CC-BY-4.0",
            "DocumentRef-spdx:LicenseRef-proprietary",
        ] {
            assert!(!approved(expression), "{expression} is not recognized");
        }
        // A malformed expression is refused rather than read for the
        // identifiers it happens to carry. Each of these names at least one
        // approved licence and states no complete offer, and accepting any
        // of them would land a workflow whose terms nothing established.
        for expression in [
            "MIT OR",
            "OR MIT",
            "MIT AND",
            "MIT WITH",
            "WITH LLVM-exception",
            "(MIT",
            "MIT)",
            "MIT Apache-2.0",
            "()",
            "(MIT OR Apache-2.0",
            "MIT OR (Apache-2.0",
            "MIT OR ()",
            "AND",
            "(",
            ")",
            "MIT WITH AND ISC",
            "MIT OR OR ISC",
            "MIT @ Apache-2.0",
        ] {
            assert!(!approved(expression), "{expression} is malformed");
        }
        // Well-formed nesting and a nested exception still read.
        for expression in [
            "MIT AND (Apache-2.0 OR ISC)",
            "((MIT))",
            "Apache-2.0 WITH LLVM-exception AND ISC",
            "(Apache-2.0 WITH LLVM-exception)",
        ] {
            assert!(approved(expression), "{expression} is well formed");
        }
        // SPDX states an identifier "should be matched in a case-insensitive
        // manner", and cargo accepts `license = "mit"` without complaint, so a
        // real crate can carry any casing and none of these may read as
        // unrecognized.
        for expression in [
            "mit",
            "MiT",
            "apache-2.0 OR mit",
            "APACHE-2.0 WITH llvm-exception",
        ] {
            assert!(
                approved(expression),
                "{expression} names an approved licence"
            );
        }
        // The operators are the other half of the same sentence: SPDX matches
        // them case-sensitively, so a lowercase one is not an operator and the
        // expression it appears in is malformed.
        for expression in [
            "MIT or Apache-2.0",
            "MIT and Apache-2.0",
            "MIT with LLVM-exception",
        ] {
            assert!(!approved(expression), "{expression} carries no operator");
        }
        // The operand after WITH must be an exception identifier. One this
        // release does not recognize leaves the expression malformed, so the
        // licence goes unread rather than being taken from the left operand.
        for expression in [
            "MIT WITH definitely-not-an-spdx-exception",
            "MIT WITH MIT",
            "MIT WITH Apache-2.0",
        ] {
            assert!(!approved(expression), "{expression} names no exception");
        }
        // The register is carried whole, not sampled. A subset refuses a real
        // crate: this one names an exception older than the version a sampled
        // list would have kept.
        for expression in [
            "GPL-2.0-only WITH GCC-exception-2.0",
            "GPL-2.0-or-later WITH Classpath-exception-2.0",
            "Apache-2.0 WITH Swift-exception",
            "GPL-3.0-only WITH Autoconf-exception-generic",
        ] {
            assert!(approved(expression), "{expression} names a real exception");
        }
    }

    /// The compatibility question has one owner, and the projection reports
    /// its answer as a record defect: a receipt reaches `rk status` through
    /// `from_record`, which cannot fail, so a provider whose pair ships
    /// nothing must be named rather than read as clean.
    #[test]
    fn a_recorded_provider_its_pair_cannot_run_is_a_record_defect() {
        use super::{Provider, code_scanning_incompatibility};

        assert!(code_scanning_incompatibility(None, Some("bash"), Some("gitlab")).is_none());
        assert!(
            code_scanning_incompatibility(Some(Provider::Semgrep), Some("rust"), Some("gitlab"))
                .is_none()
        );
        assert!(
            code_scanning_incompatibility(Some(Provider::CodeQl), Some("rust"), Some("github"))
                .is_none()
        );

        let bash =
            code_scanning_incompatibility(Some(Provider::Semgrep), Some("bash"), Some("github"))
                .expect("a binding with no scanner is named");
        assert!(bash.contains("bash binding"), "{bash}");
        let gitlab =
            code_scanning_incompatibility(Some(Provider::CodeQl), Some("rust"), Some("gitlab"))
                .expect("codeql on gitlab is named");
        assert!(gitlab.contains("codeql"), "{gitlab}");
        let release_less =
            code_scanning_incompatibility(Some(Provider::Semgrep), None, Some("github"))
                .expect("no driver is named");
        assert!(
            release_less.contains("no automatic release driver"),
            "{release_less}"
        );

        // The projection carries the same reason, so a record-only reader sees
        // it without going through resolution.
        let mut params = Params::for_test("acme/widget", Some(Style::Trunk));
        params.set_code_scanning_for_test(Some(Provider::Semgrep));
        let clean = Projection::compute(&ProjectionInput {
            params: params.clone(),
            evidence: TargetEvidence::default(),
        })
        .expect("the pair projects");
        assert!(
            clean.record_defects.is_empty(),
            "{:?}",
            clean.record_defects
        );
    }

    /// The code scanning capability: the provider gates its own destination,
    /// semgrep carries no licence condition, and codeql's condition reads the
    /// crate's declared licence without touching the filesystem.
    #[test]
    fn the_code_scanning_destinations_follow_the_recorded_provider() {
        use super::{
            CODE_SCANNING_DESTINATIONS, Kind, Provider, code_scanning_licence_refusal, kind_of,
        };
        for (destination, _) in CODE_SCANNING_DESTINATIONS {
            assert_eq!(kind_of(destination), Some(Kind::Rendered), "{destination}");
        }

        let project = |provider: Option<Provider>| {
            let mut params = Params::for_test("acme/widget", Some(Style::Trunk));
            params.set_code_scanning_for_test(provider);
            Projection::compute(&ProjectionInput {
                params,
                evidence: TargetEvidence {
                    crate_shape: CrateShape {
                        cargo_toml: Some(
                            "[package]\nname = \"widget\"\nlicense = \"MIT\"\n".to_owned(),
                        ),
                        ..CrateShape::default()
                    },
                    ..TargetEvidence::default()
                },
            })
            .expect("the embedded pair projects")
        };
        let landed = |projection: &Projection, destination: &str| {
            projection
                .candidates
                .iter()
                .any(|candidate| candidate.destination == destination)
        };

        let off = project(None);
        for (destination, _) in CODE_SCANNING_DESTINATIONS {
            assert!(!landed(&off, destination), "{destination}");
        }
        assert!(off.licence_refusal.is_none());

        let codeql = project(Some(Provider::CodeQl));
        assert!(landed(
            &codeql,
            ".github/workflows/code-scanning-codeql.yml"
        ));
        assert!(!landed(
            &codeql,
            ".github/workflows/code-scanning-semgrep.yml"
        ));
        assert!(codeql.licence_refusal.is_none(), "MIT satisfies the terms");

        let semgrep = project(Some(Provider::Semgrep));
        assert!(landed(
            &semgrep,
            ".github/workflows/code-scanning-semgrep.yml"
        ));
        assert!(!landed(
            &semgrep,
            ".github/workflows/code-scanning-codeql.yml"
        ));

        // Semgrep carries no condition, whatever the licence says.
        let proprietary = CrateShape {
            cargo_toml: Some("[package]\nlicense = \"LicenseRef-proprietary\"\n".to_owned()),
            ..CrateShape::default()
        };
        assert!(
            code_scanning_licence_refusal(Some(Provider::Semgrep), Some("rust"), &proprietary)
                .is_none()
        );
        let refusal =
            code_scanning_licence_refusal(Some(Provider::CodeQl), Some("rust"), &proprietary)
                .expect("codeql refuses a licence its terms do not cover");
        assert!(refusal.contains("LicenseRef-proprietary"), "{refusal}");
        assert!(refusal.contains("semgrep"), "{refusal}");
        // A binding whose licence field this release does not read refuses
        // rather than assuming the terms are met.
        let bash =
            code_scanning_licence_refusal(Some(Provider::CodeQl), Some("bash"), &proprietary)
                .expect("an unread binding refuses");
        assert!(bash.contains("bash binding"), "{bash}");
    }

    /// The pure boundary, held by a source scan over this file's
    /// production code: everything above the first `#[cfg(test)]`, with
    /// comment lines skipped. The evidence gathering that reads a target
    /// lives in `src/projection/evidence.rs`, which this scan does not
    /// cover on purpose.
    #[test]
    fn the_projection_performs_no_filesystem_git_environment_clock_registry_or_network_read() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/projection.rs");
        let text = std::fs::read_to_string(&path).expect("the source reads");
        let production = text.split("#[cfg(test)]").next().unwrap_or("");
        let needles = [
            "std::fs",
            "std::env",
            "std::process",
            "std::time",
            "SystemTime",
            "Instant",
            "std::net",
            "Command::new",
            "registry::",
            "curl",
            "reqwest",
            "blob(",
        ];
        let mut hits = Vec::new();
        for (index, line) in production.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for needle in needles {
                if line.contains(needle) {
                    hits.push(format!("src/projection.rs:{}: {needle}", index + 1));
                }
            }
        }
        assert!(
            hits.is_empty(),
            "the projection reads beyond its inputs: {hits:?}"
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one test walks the three marked destinations and the three unmarked shapes"
    )]
    fn marked_region_projection_preserves_every_target_byte_outside_the_markers() {
        let agents_before = "# Widget\n\nOperator prose above.\n\n";
        let agents_after = "\n\n## Our rules\n\nOperator prose below.   \n";
        let glossary_before = "# Glossary\n\n- `spike` is a throwaway branch.\n\n";
        let glossary_after = "\n\n## More terms\n\n- `own` is ours.";
        let hooks_before = "default_install_hook_types: [pre-commit]\n\nrepos:\n";
        let hooks_after =
            "\n  - repo: https://example.com/own\n    rev: v1\n    hooks:\n      - id: own\n";
        let stale = |begin: &str, end: &str| format!("{begin}\nstale block\n{end}");
        let mut documents = std::collections::BTreeMap::new();
        documents.insert(
            AGENTS_DESTINATION.to_owned(),
            format!(
                "{agents_before}{}{agents_after}",
                stale(BLOCK_BEGIN, BLOCK_END)
            )
            .into_bytes(),
        );
        documents.insert(
            GLOSSARY_DESTINATION.to_owned(),
            format!(
                "{glossary_before}{}{glossary_after}",
                stale(BLOCK_BEGIN, BLOCK_END)
            )
            .into_bytes(),
        );
        documents.insert(
            HOOKS_DESTINATION.to_owned(),
            format!(
                "{hooks_before}{}{hooks_after}",
                stale(HOOKS_BEGIN, HOOKS_END)
            )
            .into_bytes(),
        );
        let projection = compute(TargetEvidence {
            documents: documents.clone(),
            crate_shape: supported_shape(),
            ..TargetEvidence::default()
        });
        assert!(
            projection.collisions.is_empty(),
            "{:?}",
            projection.collisions
        );
        for (destination, before, after) in [
            (AGENTS_DESTINATION, agents_before, agents_after),
            (GLOSSARY_DESTINATION, glossary_before, glossary_after),
            (HOOKS_DESTINATION, hooks_before, hooks_after),
        ] {
            let candidate = candidate(&projection, destination);
            let Placement::Region { begin, end } = candidate.placement else {
                panic!("{destination} is a region");
            };
            let region = candidate
                .region
                .as_deref()
                .expect("a region carries its block");
            let (head, tail) = outside(&candidate.bytes, begin, end);
            assert_eq!(
                head,
                before.as_bytes(),
                "{destination}: bytes before the markers"
            );
            assert_eq!(
                tail,
                after.as_bytes(),
                "{destination}: bytes after the markers"
            );
            let inside = &candidate.bytes[head.len()..candidate.bytes.len() - tail.len()];
            assert_eq!(
                inside, region,
                "{destination}: the region is the rendered block"
            );
            let (existing_head, existing_tail) = outside(&documents[destination], begin, end);
            assert_eq!(head, existing_head);
            assert_eq!(tail, existing_tail);
        }

        // Unmarked documents: the hook block lands under the owning key,
        // the routing block appends, and an absent file yields a fresh
        // document.
        let own_hooks =
            "repos:\n  - repo: https://example.com/own\n    rev: v1\n    hooks:\n      - id: own\n";
        let own_agents = "# Widget\n\nOwn rules.";
        let mut documents = std::collections::BTreeMap::new();
        documents.insert(HOOKS_DESTINATION.to_owned(), own_hooks.as_bytes().to_vec());
        documents.insert(
            AGENTS_DESTINATION.to_owned(),
            own_agents.as_bytes().to_vec(),
        );
        let projection = compute(TargetEvidence {
            documents,
            crate_shape: supported_shape(),
            ..TargetEvidence::default()
        });
        assert!(
            projection.collisions.is_empty(),
            "{:?}",
            projection.collisions
        );
        let hooks = candidate(&projection, HOOKS_DESTINATION);
        let hooks_text = String::from_utf8_lossy(&hooks.bytes);
        let region = String::from_utf8_lossy(hooks.region.as_deref().expect("a region"));
        assert!(
            hooks_text.starts_with(&format!(
                "repos:\n{region}\n  - repo: https://example.com/own"
            )),
            "{hooks_text}"
        );
        assert!(!hooks_text.contains(HOOK_TYPES_LINE));
        let agents = candidate(&projection, AGENTS_DESTINATION);
        assert!(agents.bytes.starts_with(own_agents.as_bytes()));
        assert_eq!(
            extract_block(
                &String::from_utf8_lossy(&agents.bytes),
                BLOCK_BEGIN,
                BLOCK_END
            )
            .map(str::as_bytes),
            agents.region.as_deref()
        );
        let glossary = candidate(&projection, GLOSSARY_DESTINATION);
        let region = glossary.region.as_deref().expect("a region");
        assert_eq!(
            glossary.bytes,
            [region, b"\n"].concat(),
            "an absent file is fresh"
        );
    }

    /// A hook document that is not UTF-8 offers the block no place: the
    /// line-based splice would have to decode it, and a lossy decode
    /// rewrites a byte outside the markers. A valid document still
    /// splices, and the markdown destinations, spliced as bytes, take an
    /// invalid byte outside their markers unchanged.
    #[test]
    fn a_hook_document_that_is_not_utf8_collides_instead_of_being_rewritten() {
        let mut documents = std::collections::BTreeMap::new();
        let mut invalid = b"repos:\n# own \xff above\n".to_vec();
        invalid.extend_from_slice(format!("{HOOKS_BEGIN}\nstale\n{HOOKS_END}\n").as_bytes());
        invalid.extend_from_slice(b"  - repo: local \xff below\n");
        documents.insert(HOOKS_DESTINATION.to_owned(), invalid);
        let mut agents = b"# Widget r\xe9sum\xe9\n\n".to_vec();
        agents.extend_from_slice(format!("{BLOCK_BEGIN}\nstale\n{BLOCK_END}\n\n").as_bytes());
        agents.extend_from_slice(b"r\xe9sum\xe9\n");
        documents.insert(AGENTS_DESTINATION.to_owned(), agents.clone());
        let projection = compute(TargetEvidence {
            documents,
            crate_shape: supported_shape(),
            ..TargetEvidence::default()
        });
        let collided: Vec<&str> = projection
            .collisions
            .iter()
            .map(|c| c.destination.as_str())
            .collect();
        assert_eq!(collided, [HOOKS_DESTINATION]);
        assert!(
            projection.collisions[0].reason.contains("not UTF-8"),
            "{}",
            projection.collisions[0].reason
        );
        assert!(
            !projection
                .candidates
                .iter()
                .any(|c| c.destination == HOOKS_DESTINATION),
            "a colliding destination projects no candidate"
        );
        let agents = candidate(&projection, AGENTS_DESTINATION);
        assert!(
            agents.bytes.starts_with(b"# Widget r\xe9sum\xe9\n\n"),
            "{:?}",
            agents.bytes
        );
        assert!(
            agents.bytes.ends_with(b"\n\nr\xe9sum\xe9\n"),
            "{:?}",
            agents.bytes
        );
        assert!(
            !agents.bytes.contains(&0xEF),
            "a replacement character landed"
        );

        let mut documents = std::collections::BTreeMap::new();
        let valid =
            format!("repos:\n# own above\n{HOOKS_BEGIN}\nstale\n{HOOKS_END}\n  - repo: local\n");
        documents.insert(HOOKS_DESTINATION.to_owned(), valid.into_bytes());
        let projection = compute(TargetEvidence {
            documents,
            crate_shape: supported_shape(),
            ..TargetEvidence::default()
        });
        assert!(
            projection.collisions.is_empty(),
            "{:?}",
            projection.collisions
        );
        let hooks = candidate(&projection, HOOKS_DESTINATION);
        let text = String::from_utf8(hooks.bytes.clone()).expect("a valid document stays text");
        assert!(text.starts_with("repos:\n# own above\n"), "{text}");
        assert!(text.ends_with("\n  - repo: local\n"), "{text}");
        assert!(!text.contains("stale"), "the region is replaced: {text}");
    }

    /// A snippet that ships a block destination as a whole file is a
    /// source defect named by both sides: the snippet's source path and
    /// the block's template paths.
    #[test]
    fn a_whole_file_colliding_with_a_marked_region_names_both_source_paths() {
        let files: Vec<(String, &[u8])> = vec![
            ("snippets/_shared/github/SECURITY.md".to_owned(), b"policy"),
            ("snippets/rust/github/AGENTS.md".to_owned(), b"whole"),
        ];
        let err = Projection::compute_over(&files, &input(TargetEvidence::default()))
            .expect_err("a whole file at a block destination refuses");
        let text = err.to_string();
        assert!(text.contains("snippets/rust/github/AGENTS.md"), "{text}");
        assert!(text.contains(super::AGENTS_BLOCK), "{text}");
        assert!(text.contains(super::AGENTS_LINE_WORKTREE), "{text}");
        assert!(text.contains("embedded sources are defective"), "{text}");
    }

    #[test]
    fn duplicate_whole_file_destinations_and_overlapping_marked_regions_refuse_with_the_conflicting_source_names()
     {
        // Two capabilities shipping one destination is a source defect
        // named by both sides.
        let files: Vec<(String, &[u8])> = vec![
            ("snippets/_shared/github/SECURITY.md".to_owned(), b"shared"),
            ("snippets/rust/github/SECURITY.md".to_owned(), b"pair"),
            ("snippets/rust/github/release-plz.toml".to_owned(), b"seed"),
        ];
        let err = Projection::compute_over(&files, &input(TargetEvidence::default()))
            .expect_err("a doubled destination refuses");
        let text = err.to_string();
        assert!(
            text.contains("snippets/_shared/github/SECURITY.md"),
            "{text}"
        );
        assert!(text.contains("snippets/rust/github/SECURITY.md"), "{text}");
        assert!(text.contains("embedded sources are defective"), "{text}");

        let clean: Vec<(String, &[u8])> = vec![
            ("snippets/_shared/github/SECURITY.md".to_owned(), b"shared"),
            ("snippets/rust/github/release-plz.toml".to_owned(), b"seed"),
        ];
        let projection = Projection::compute_over(&clean, &input(TargetEvidence::default()))
            .expect("a clean list projects");
        let whole: Vec<&str> = projection
            .candidates
            .iter()
            .filter(|c| c.placement == Placement::Whole)
            .map(|c| c.destination.as_str())
            .collect();
        assert_eq!(whole, ["SECURITY.md", "release-plz.toml"]);
        assert_eq!(
            projection
                .candidates
                .iter()
                .find(|c| c.destination == "SECURITY.md")
                .map(|c| c.sources.clone()),
            Some(vec!["snippets/_shared/github/SECURITY.md".to_owned()])
        );

        let doubled = format!("{BLOCK_BEGIN}\na\n{BLOCK_END}\n{BLOCK_BEGIN}\nb\n{BLOCK_END}\n");
        let unmatched = format!("repos:\n{HOOKS_BEGIN}\n  - repo: local\n");
        let misordered = format!("# G\n{BLOCK_END}\n{BLOCK_BEGIN}\n");
        let mut documents = std::collections::BTreeMap::new();
        documents.insert(AGENTS_DESTINATION.to_owned(), doubled.into_bytes());
        documents.insert(HOOKS_DESTINATION.to_owned(), unmatched.into_bytes());
        documents.insert(GLOSSARY_DESTINATION.to_owned(), misordered.into_bytes());
        let projection = compute(TargetEvidence {
            documents,
            crate_shape: supported_shape(),
            ..TargetEvidence::default()
        });
        let mut collided: Vec<&str> = projection
            .collisions
            .iter()
            .map(|Collision { destination, .. }| destination.as_str())
            .collect();
        collided.sort_unstable();
        let mut expected = BLOCK_DESTINATIONS.to_vec();
        expected.sort_unstable();
        assert_eq!(collided, expected);
        for collision in &projection.collisions {
            assert!(
                collision.reason.contains(&collision.destination),
                "{collision:?}"
            );
            assert!(
                !projection
                    .candidates
                    .iter()
                    .any(|candidate| candidate.destination == collision.destination),
                "{} collided and still projects",
                collision.destination
            );
        }
        let agents = projection
            .collisions
            .iter()
            .find(|c| c.destination == AGENTS_DESTINATION)
            .expect("the doubled document collides");
        assert!(agents.reason.contains("more than one"), "{}", agents.reason);
        let hooks = projection
            .collisions
            .iter()
            .find(|c| c.destination == HOOKS_DESTINATION)
            .expect("the unmatched document collides");
        assert!(hooks.reason.contains("unmatched"), "{}", hooks.reason);
    }
}
