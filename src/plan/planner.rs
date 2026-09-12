//! The pure planner: from an observation, a desired state, and two
//! bundles read through the seam, one plan.
//!
//! No filesystem, no network, and no clock inside: the observation was
//! gathered before, the bundles answer through [`ReleaseSource`], and the
//! instant is an input. The same inputs produce the same plan and the
//! same fingerprint, which is the first thing the tests hold.

use std::collections::BTreeMap;

use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::manifest::{self, Alignment, FileRecord, Manifest, Parameters};
use crate::landing::{self, Entry, Kind, Placement};
use crate::release::{PAYLOAD_SCHEMA, ReleaseManifest, ReleaseSource};

use super::classify::{self, Finding, RecordState as ClassifyRecord, Verdict};
use super::evidence::EvidenceKind;
use super::gather::{ConfigRead, ForgeRead, Observation, RecordRead, Resolution};
use super::operation::Operation;
use super::readiness::{self, Evaluation, Precondition, Requirement};
use super::{
    BaselineState, BundleIdentity, Choice, Compatibility, Configuration, ConfigurationState,
    Decision, DesiredState, Destination, ForgeState, Guidance, Host, Identity, Installation,
    ObservedState, PLAN_SCHEMA, Plan, Planned, Postcondition, RecordState, Release, Repository,
    ResolvedRelease, Verification, fingerprint,
};

/// The candidate bundle, as the planner receives it.
pub struct Candidate<'a> {
    /// The source the candidate's bytes are read through.
    pub source: &'a dyn ReleaseSource,
    /// The candidate's manifest, read once.
    pub manifest: ReleaseManifest,
    /// Where it was read from: `embedded`, `crates`, or `directory`.
    pub venue: &'a str,
    /// How it was verified.
    pub verification: Verification,
}

/// The recorded release's bundle, as the planner receives it.
pub enum Baseline<'a> {
    /// No record, so no baseline is needed.
    NotNeeded,
    /// The recorded payload is the one compiled into this engine.
    Embedded(&'a dyn ReleaseSource),
    /// The recorded release's bundle, read from the release cache.
    Cached {
        /// The recorded version.
        version: String,
        /// The source.
        source: &'a dyn ReleaseSource,
    },
    /// The recorded release's bundle could not be read.
    NotObserved {
        /// Why.
        reason: String,
    },
}

/// Everything the planner reads.
pub struct Inputs<'a> {
    /// The instant the plan is computed, RFC 3339.
    pub clock: &'a str,
    /// The engine computing it.
    pub engine_version: &'a str,
    /// The selector as given.
    pub selector: &'a str,
    /// The candidate bundle.
    pub candidate: Candidate<'a>,
    /// The recorded release's bundle.
    pub baseline: Baseline<'a>,
    /// What was observed at the target.
    pub observation: Observation,
    /// The landing parameters, resolved or not.
    pub resolution: Resolution,
    /// The decisions the operator selected, by id.
    pub selected: &'a BTreeMap<String, String>,
}

/// What the three-way comparison decided for one destination.
struct Compared<'a> {
    entry: &'a Entry,
    /// The digest the destination holds now, or absent.
    before: Option<Digest>,
    /// Whether the candidate's bytes are written.
    write: bool,
    /// Whether the target edited a file release-kit owns.
    conflict: bool,
    /// Whether a rendered file the record names is missing from the disk.
    missing: bool,
    /// The record entry after the apply.
    record: FileRecord,
}

/// Compute the plan.
///
/// # Errors
///
/// Returns the seam's own failures where a bundle cannot be read, and a
/// serialization failure for the planned record, which is a defect.
#[allow(
    clippy::too_many_lines,
    reason = "one plan is one linear derivation from observation to fingerprint, and cutting it would separate a section from the inputs it cites"
)]
pub fn plan(inputs: Inputs<'_>) -> Result<Planned, RkError> {
    let Inputs {
        clock,
        engine_version,
        selector,
        candidate,
        baseline,
        mut observation,
        resolution,
        selected,
    } = inputs;
    let mut blobs: BTreeMap<Digest, Vec<u8>> = BTreeMap::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut preconditions: Vec<Precondition> = Vec::new();
    let mut decisions: Vec<Decision> = Vec::new();

    // The candidate, cited by everything derived from it.
    let candidate_ref = observation.ledger.observe(
        "candidate-bundle",
        EvidenceKind::Bundle,
        candidate.venue,
        clock,
        Some(candidate.manifest.payload_sha256.clone()),
        format!("manifest through the {} source", candidate.venue),
    );
    let baseline_state = match &baseline {
        Baseline::NotNeeded => BaselineState::NotNeeded,
        Baseline::Embedded(_) => BaselineState::Embedded,
        Baseline::Cached { version, .. } => BaselineState::Cached {
            version: version.clone(),
        },
        Baseline::NotObserved { reason } => BaselineState::NotObserved {
            reason: reason.clone(),
        },
    };
    let baseline_source: Option<&dyn ReleaseSource> = match &baseline {
        Baseline::Embedded(source) | Baseline::Cached { source, .. } => Some(*source),
        Baseline::NotNeeded | Baseline::NotObserved { .. } => None,
    };
    let baseline_ref = baseline_source.map(|source| {
        let digest = source
            .manifest()
            .ok()
            .map(|manifest| manifest.payload_sha256);
        observation.ledger.observe(
            "baseline-bundle",
            EvidenceKind::Bundle,
            "recorded release",
            clock,
            digest,
            "manifest through the seam",
        )
    });

    // The verdict and the record state.
    let verdict = classify::verdict(&observation.facts);
    let record_state = match &observation.record {
        RecordRead::Absent => ClassifyRecord::Absent,
        RecordRead::Present { .. } => ClassifyRecord::Present,
        RecordRead::Invalid { .. } => ClassifyRecord::Invalid,
    };
    let recorded: Option<&Manifest> = match &observation.record {
        RecordRead::Present { manifest, .. } => Some(manifest),
        RecordRead::Absent | RecordRead::Invalid { .. } => None,
    };
    if recorded.is_none() {
        for marker in &observation.facts.release_markers {
            findings.push(Finding {
                code: "release-marker",
                detail: marker.clone(),
            });
        }
        for collision in &observation.facts.collisions {
            findings.push(Finding {
                code: "payload-collision",
                detail: collision.clone(),
            });
        }
        if observation.facts.tags > 0 {
            findings.push(Finding {
                code: "tag",
                detail: format!(
                    "{} tags with no mechanism behind them",
                    observation.facts.tags
                ),
            });
        }
        for branch in &observation.facts.long_lived_branches {
            findings.push(Finding {
                code: "long-lived-branch",
                detail: branch.clone(),
            });
        }
    }
    if let RecordRead::Invalid { reason } = &observation.record {
        findings.push(Finding {
            code: "record-invalid",
            detail: reason.clone(),
        });
    }

    // The projection under the resolved parameters, where they resolved.
    let mut entries: Vec<Entry> = Vec::new();
    if let Some(params) = &resolution.params {
        entries = landing::projection(candidate.source, params)?;
        if let Some((set, _)) = &resolution.nix_withheld {
            entries.retain(|entry| !set.iter().any(|path| path == &entry.destination));
        }
    }

    // The three-way comparison, in the one-pass shape the landing rules
    // bind: every conflict collected, nothing refused one at a time.
    let mut compared: Vec<Compared<'_>> = Vec::new();
    for entry in &entries {
        let disk = observation.files.get(&entry.destination).map(Vec::as_slice);
        let before = disk.map(Digest::of);
        let comparison = recorded.map_or_else(
            || compare_fresh(entry, disk),
            |record| compare_recorded(entry, record.file(&entry.destination), disk),
        );
        if comparison.write {
            blobs.insert(Digest::of(&entry.rendered), entry.rendered.clone());
            if let Some(bytes) = disk {
                blobs.insert(Digest::of(bytes), bytes.to_vec());
            }
        }
        if comparison.conflict {
            if let Some(bytes) = disk {
                blobs.insert(Digest::of(bytes), bytes.to_vec());
            }
            blobs.insert(Digest::of(&entry.rendered), entry.rendered.clone());
            if let (Some(source), Some(record)) = (baseline_source, recorded) {
                if let Some(baseline_bytes) = baseline_bytes(source, record, entry) {
                    blobs.insert(Digest::of(&baseline_bytes), baseline_bytes);
                }
            }
        }
        compared.push(Compared {
            entry,
            before,
            write: comparison.write,
            conflict: comparison.conflict,
            missing: comparison.missing,
            record: comparison.record,
        });
    }
    let owned_drift = compared.iter().any(|c| c.conflict) || observation.hooks_defect.is_some();
    for c in compared.iter().filter(|c| c.conflict) {
        findings.push(Finding {
            code: if c.missing {
                "owned-missing"
            } else {
                "owned-drift"
            },
            detail: c.entry.destination.clone(),
        });
    }
    if let Some(defect) = &observation.hooks_defect {
        findings.push(Finding {
            code: "owned-drift",
            detail: defect.clone(),
        });
    }
    let classification = classify::classify(record_state, verdict, owned_drift);

    // The operations, in apply order: files, then the pin, then the
    // record, last.
    let mut operations: Vec<Operation> = Vec::new();
    for c in compared.iter().filter(|c| c.write && !c.conflict) {
        let after = Digest::of(&c.entry.rendered);
        operations.push(match c.entry.placement {
            Placement::Whole => Operation::WriteFile {
                path: c.entry.destination.clone(),
                kind: c.entry.kind,
                before: c.before.clone(),
                after,
            },
            Placement::Block => Operation::SpliceBlock {
                path: c.entry.destination.clone(),
                marker: landing::block_markers(&c.entry.destination)
                    .map_or("", |(begin, _)| begin)
                    .to_owned(),
                before: c.before.clone(),
                after,
            },
        });
    }
    let candidate_version = candidate.manifest.release_kit_version.clone();
    if let Some(pin) = &observation.pin {
        if trim_v(&pin.version) != trim_v(&candidate_version) {
            operations.push(Operation::UpdatePin {
                manager: pin.manager.clone(),
                before: pin.version.clone(),
                after: candidate_version.clone(),
            });
        }
    }
    let planned_record = match (&resolution.params, record_state) {
        (Some(params), ClassifyRecord::Absent | ClassifyRecord::Present) if !owned_drift => {
            let record = planned_manifest(
                &candidate,
                params,
                recorded,
                clock,
                compared.iter().map(|c| &c.record),
            );
            let bytes = manifest::render(&record)?;
            let after = Digest::of(&bytes);
            let before = match &observation.record {
                RecordRead::Present { bytes, .. } => Some(Digest::of(bytes)),
                RecordRead::Absent | RecordRead::Invalid { .. } => None,
            };
            if before.as_ref() == Some(&after) {
                None
            } else {
                blobs.insert(after.clone(), bytes);
                if let RecordRead::Present { bytes, .. } = &observation.record {
                    blobs.insert(Digest::of(bytes), bytes.clone());
                }
                Some(Operation::WriteRecord { before, after })
            }
        }
        _ => None,
    };
    if let Some(operation) = planned_record {
        operations.push(operation);
    }

    // The preconditions, each with its requirement.
    let record_ref = observation.refs.record.clone();
    let config_ref = observation.refs.configuration.clone();
    let resolution_refs: Vec<String> = record_ref
        .iter()
        .chain(config_ref.iter())
        .chain(observation.refs.repository.iter())
        .cloned()
        .collect();
    preconditions.push(Precondition {
        id: "technology-resolved".into(),
        requirement: Requirement::Required,
        evaluation: resolution
            .unresolved
            .as_ref()
            .map_or(Evaluation::Satisfied, |reason| Evaluation::Unsatisfied {
                reason: reason.clone(),
            }),
        decision: None,
        evidence_refs: resolution_refs.clone(),
    });
    if let RecordRead::Invalid { reason } = &observation.record {
        preconditions.push(Precondition {
            id: "record-readable".into(),
            requirement: Requirement::Required,
            evaluation: Evaluation::Unsatisfied {
                reason: reason.clone(),
            },
            decision: None,
            evidence_refs: record_ref.iter().cloned().collect(),
        });
    } else if recorded.is_some() {
        preconditions.push(Precondition {
            id: "record-readable".into(),
            requirement: Requirement::Required,
            evaluation: Evaluation::Satisfied,
            decision: None,
            evidence_refs: record_ref.iter().cloned().collect(),
        });
    }
    if let ConfigRead::Invalid { reason, .. } = &observation.config {
        preconditions.push(Precondition {
            id: "configuration-readable".into(),
            requirement: Requirement::Required,
            evaluation: Evaluation::Unsatisfied {
                reason: reason.clone(),
            },
            decision: None,
            evidence_refs: config_ref.iter().cloned().collect(),
        });
    }
    if let Some(record) = recorded {
        let newer =
            manifest::alignment(&record.rk_version, engine_version) == Alignment::TargetNewer;
        if newer {
            findings.push(Finding {
                code: "record-newer",
                detail: record.rk_version.clone(),
            });
        }
        preconditions.push(Precondition {
            id: "engine-not-older-than-record".into(),
            requirement: Requirement::Required,
            evaluation: if newer {
                Evaluation::Unsatisfied {
                    reason: format!(
                        "the record came from rk {}, newer than this engine's {engine_version}; install release-kit {} or newer",
                        record.rk_version, record.rk_version
                    ),
                }
            } else {
                Evaluation::Satisfied
            },
            decision: None,
            evidence_refs: record_ref.iter().cloned().collect(),
        });
    }
    let bundle_schema = candidate.manifest.payload_schema;
    preconditions.push(Precondition {
        id: "bundle-schema-readable".into(),
        requirement: Requirement::Required,
        evaluation: if bundle_schema <= PAYLOAD_SCHEMA {
            Evaluation::Satisfied
        } else {
            Evaluation::Unsatisfied {
                reason: format!(
                    "the bundle declares payload schema {bundle_schema}, and this engine reads schema {PAYLOAD_SCHEMA} at most; install release-kit {candidate_version} or newer"
                ),
            }
        },
        decision: None,
        evidence_refs: vec![candidate_ref.clone()],
    });
    if let Some(hooks_ref) = observation
        .refs
        .destinations
        .get(landing::HOOKS_DESTINATION)
        .cloned()
    {
        preconditions.push(Precondition {
            id: "hooks-file-spliceable".into(),
            requirement: Requirement::Required,
            evaluation: observation
                .hooks_defect
                .as_ref()
                .map_or(Evaluation::Satisfied, |defect| Evaluation::Unsatisfied {
                    reason: defect.clone(),
                }),
            decision: None,
            evidence_refs: vec![hooks_ref],
        });
    }
    for c in compared.iter().filter(|c| c.conflict) {
        let path = &c.entry.destination;
        let mut refs: Vec<String> = observation
            .refs
            .destinations
            .get(path)
            .cloned()
            .into_iter()
            .collect();
        refs.extend(record_ref.iter().cloned());
        refs.extend(baseline_ref.iter().cloned());
        preconditions.push(Precondition {
            id: format!("owned-file-unedited:{path}"),
            requirement: Requirement::Required,
            evaluation: Evaluation::Unsatisfied {
                reason: if c.missing {
                    "the record names it and the disk does not hold it".to_owned()
                } else if recorded.is_some() {
                    "the target edited a file release-kit owns".to_owned()
                } else {
                    "the destination exists with different content".to_owned()
                },
            },
            decision: None,
            evidence_refs: refs,
        });
    }
    let file_operations = operations
        .iter()
        .any(|operation| operation.path().is_some());
    if recorded.is_some() {
        let (requirement, evaluation, decision) = match &baseline_state {
            BaselineState::NotObserved { reason } => {
                let answered = selected.get("partial-baseline").map(String::as_str);
                decisions.push(Decision {
                    id: "partial-baseline".into(),
                    question: "plan against the record's digests alone, with the recorded release's bytes unread?".into(),
                    choices: vec![
                        Choice {
                            answer: "accept".into(),
                            consequence: "the three-way comparison shows what diverged and cannot show the baseline it diverged from".into(),
                        },
                        Choice {
                            answer: "fetch".into(),
                            consequence: "re-plan with --fetch so the recorded release's bundle is read through the crates venue".into(),
                        },
                    ],
                    selected: answered.map(str::to_owned),
                });
                (
                    if file_operations {
                        Requirement::DecisionRequired
                    } else {
                        Requirement::Advisory
                    },
                    if answered == Some("accept") {
                        Evaluation::Satisfied
                    } else {
                        Evaluation::NotObserved {
                            reason: reason.clone(),
                        }
                    },
                    Some("partial-baseline".to_owned()),
                )
            }
            _ => (Requirement::Advisory, Evaluation::Satisfied, None),
        };
        preconditions.push(Precondition {
            id: "baseline-observed".into(),
            requirement,
            evaluation,
            decision,
            evidence_refs: baseline_ref.iter().cloned().collect(),
        });
    }
    if recorded.is_none() && record_state == ClassifyRecord::Absent {
        let source = resolution
            .sources
            .get("workflow")
            .map_or("default", String::as_str);
        let answered = (source != "default").then(|| {
            resolution.params.as_ref().map_or_else(
                || "worktree".to_owned(),
                |p| p.workflow().as_str().to_owned(),
            )
        });
        decisions.push(Decision {
            id: "workflow-mode".into(),
            question: "which working-copy mode does this project choose?".into(),
            choices: vec![
                Choice {
                    answer: "worktree".into(),
                    consequence: "every code-changing branch lives in a linked worktree and the main checkout commits nothing".into(),
                },
                Choice {
                    answer: "branches".into(),
                    consequence: "branches are worked in the main checkout, with worktrees optional beside it".into(),
                },
            ],
            selected: answered.clone(),
        });
        preconditions.push(Precondition {
            id: "workflow-mode-answered".into(),
            requirement: Requirement::DecisionRequired,
            evaluation: if answered.is_some() {
                Evaluation::Satisfied
            } else {
                Evaluation::NotObserved {
                    reason: "no flag, configuration, or decision names the mode".into(),
                }
            },
            decision: Some("workflow-mode".into()),
            evidence_refs: resolution_refs.clone(),
        });
    }
    if let Some(record) = recorded {
        if record.parameters.style.is_none() {
            let source = resolution
                .sources
                .get("style")
                .map_or("default", String::as_str);
            let answered = (source != "default").then(|| {
                resolution
                    .params
                    .as_ref()
                    .and_then(landing::Params::style)
                    .map_or_else(|| "trunk".to_owned(), |s| s.as_str().to_owned())
            });
            decisions.push(Decision {
                id: "release-style".into(),
                question: "the record predates the release style; which one does this project run?".into(),
                choices: vec![
                    Choice {
                        answer: "trunk".into(),
                        consequence: "the bot's release request is armed to merge itself".into(),
                    },
                    Choice {
                        answer: "lines".into(),
                        consequence: "every merge is a human's, and release lines carry the maintained versions".into(),
                    },
                ],
                selected: answered.clone(),
            });
            preconditions.push(Precondition {
                id: "release-style-answered".into(),
                requirement: Requirement::DecisionRequired,
                evaluation: if answered.is_some() {
                    Evaluation::Satisfied
                } else {
                    Evaluation::NotObserved {
                        reason: "neither the configuration nor a decision names the style".into(),
                    }
                },
                decision: Some("release-style".into()),
                evidence_refs: resolution_refs.clone(),
            });
        }
    }
    if recorded.is_none() && verdict == Verdict::NeedsDecision {
        let answered = selected.get("release-activity").cloned();
        decisions.push(Decision {
            id: "release-activity".into(),
            question: "tags or a second long-lived branch exist with no mechanism behind them; what are they?".into(),
            choices: vec![
                Choice {
                    answer: "history".into(),
                    consequence: "the activity is history the landing leaves in place, and the plan proceeds as a setup".into(),
                },
                Choice {
                    answer: "migrate".into(),
                    consequence: "another mechanism made them; follow the migration procedure and retire it first".into(),
                },
            ],
            selected: answered.clone(),
        });
        preconditions.push(Precondition {
            id: "release-activity-explained".into(),
            requirement: Requirement::DecisionRequired,
            evaluation: if answered.is_some() {
                Evaluation::Satisfied
            } else {
                Evaluation::NotObserved {
                    reason: "the operator has not said what the release activity is".into(),
                }
            },
            decision: Some("release-activity".into()),
            evidence_refs: observation.refs.repository.clone(),
        });
    }
    preconditions.push(Precondition {
        id: "forge-observed".into(),
        requirement: Requirement::Advisory,
        evaluation: match &observation.forge {
            ForgeRead::Observed { .. } => Evaluation::Satisfied,
            ForgeRead::NotObserved { reason } => Evaluation::NotObserved {
                reason: reason.clone(),
            },
        },
        decision: None,
        evidence_refs: observation.refs.forge.clone(),
    });
    preconditions.push(Precondition {
        id: "pin-wired".into(),
        requirement: Requirement::Advisory,
        evaluation: if observation.pin.is_some() {
            Evaluation::Satisfied
        } else {
            Evaluation::NotObserved {
                reason: "no manager file names release-kit".into(),
            }
        },
        decision: None,
        evidence_refs: observation.refs.pin.iter().cloned().collect(),
    });
    let readiness = readiness::derive(&preconditions);

    // The postconditions, one per operation kind that leaves a check.
    let mut postconditions: Vec<Postcondition> = Vec::new();
    for operation in &operations {
        match operation {
            Operation::WriteFile { path, after, .. }
            | Operation::SpliceBlock { path, after, .. } => {
                postconditions.push(Postcondition::DestinationHolds {
                    path: path.clone(),
                    sha256: after.clone(),
                });
            }
            Operation::WriteRecord { after, .. } => {
                postconditions.push(Postcondition::RecordReadsBack {
                    sha256: after.clone(),
                });
            }
            Operation::UpdatePin { manager, after, .. } => {
                postconditions.push(Postcondition::PinReads {
                    manager: manager.clone(),
                    version: after.clone(),
                });
            }
            Operation::RemoveOwnedFile { .. } => {}
        }
    }
    if !operations.is_empty() {
        postconditions.push(Postcondition::StatusCheckClean);
    }

    // The sections, assembled.
    let configuration = resolution.params.as_ref().map(|params| Configuration {
        tech: params.tech().to_owned(),
        forge: params.forge().to_owned(),
        repo: params.repo().to_owned(),
        workflow: params.workflow().as_str().to_owned(),
        style: params.style().map(|style| style.as_str().to_owned()),
        nix: params.nix(),
        trunk: params.trunk().to_owned(),
        line_prefix: params.line_prefix().to_owned(),
        security_contact: params.security_contact().to_owned(),
        security_response: params.security_response().to_owned(),
        sources: resolution.sources.clone(),
        evidence_refs: resolution_refs.clone(),
    });
    let record_view = match &observation.record {
        RecordRead::Absent => RecordState::Absent,
        RecordRead::Present { manifest, bytes } => RecordState::Present {
            rk_version: manifest.rk_version.clone(),
            payload_sha256: manifest.payload_sha256.clone(),
            schema_version: manifest.schema_version,
            origin: manifest.origin.clone(),
            sha256: Digest::of(bytes),
        },
        RecordRead::Invalid { reason } => RecordState::Invalid {
            reason: reason.clone(),
        },
    };
    let configuration_view = match &observation.config {
        ConfigRead::Absent => ConfigurationState {
            present: false,
            sha256: None,
            invalid: None,
            pending: Vec::new(),
        },
        ConfigRead::Present { config, bytes } => ConfigurationState {
            present: true,
            sha256: Some(Digest::of(bytes)),
            invalid: None,
            pending: recorded
                .map_or_else(Vec::new, |record| crate::config::pending(config, record)),
        },
        ConfigRead::Invalid { reason, bytes } => ConfigurationState {
            present: true,
            sha256: Some(Digest::of(bytes)),
            invalid: Some(reason.clone()),
            pending: Vec::new(),
        },
    };
    let mut destination_paths: Vec<String> = entries
        .iter()
        .map(|entry| entry.destination.clone())
        .chain(
            recorded
                .into_iter()
                .flat_map(|record| record.files.iter().map(|file| file.destination.clone())),
        )
        .collect();
    destination_paths.sort();
    destination_paths.dedup();
    let destinations: Vec<Destination> = destination_paths
        .into_iter()
        .map(|path| {
            let bytes = observation.files.get(&path);
            Destination {
                present: bytes.is_some(),
                sha256: bytes.map(|bytes| Digest::of(bytes)),
                recorded_kind: recorded
                    .and_then(|record| record.file(&path))
                    .map(|file| file.kind),
                path,
            }
        })
        .collect();
    let installation_refs: Vec<String> = record_ref
        .iter()
        .chain(config_ref.iter())
        .cloned()
        .chain(observation.refs.destinations.values().cloned())
        .collect();
    let forge_view = match &observation.forge {
        ForgeRead::NotObserved { reason } => ForgeState::NotObserved {
            reason: reason.clone(),
        },
        ForgeRead::Observed { trunk, remote_tip } => ForgeState::Observed {
            trunk: trunk.clone(),
            remote_tip: remote_tip.clone(),
            evidence_refs: observation.refs.forge.clone(),
        },
    };
    let mut plan = Plan {
        schema: PLAN_SCHEMA,
        identity: Identity {
            plan_id: String::new(),
            created_at: clock.to_owned(),
            engine_version: engine_version.to_owned(),
        },
        classification,
        findings,
        desired_state: DesiredState {
            selector: selector.to_owned(),
            release: ResolvedRelease {
                version: candidate_version.clone(),
                venue: candidate.venue.to_owned(),
                payload_sha256: candidate.manifest.payload_sha256.clone(),
                payload_schema: bundle_schema,
            },
            configuration,
            unresolved: resolution.unresolved.clone(),
        },
        observed_state: ObservedState {
            repository: Repository {
                target: observation.target.clone(),
                git: observation.git,
                tags: observation.facts.tags,
                long_lived_branches: observation.facts.long_lived_branches.clone(),
                release_markers: observation.facts.release_markers.clone(),
                collisions: observation.facts.collisions.clone(),
                tech: observation.tech.clone(),
                forge: observation.forge_name.clone(),
                repo: observation.repo.clone(),
                verdict,
                evidence_refs: observation.refs.repository.clone(),
            },
            installation: Installation {
                record: record_view,
                configuration: configuration_view,
                destinations,
                evidence_refs: installation_refs,
            },
            host: Host {
                engine_version: engine_version.to_owned(),
                pin: observation.pin.clone(),
                evidence_refs: observation
                    .refs
                    .host
                    .iter()
                    .chain(observation.refs.pin.iter())
                    .cloned()
                    .collect(),
            },
            forge: forge_view,
        },
        release: Release {
            candidate: BundleIdentity {
                version: candidate_version,
                payload_sha256: candidate.manifest.payload_sha256.clone(),
                payload_schema: bundle_schema,
                artifacts: candidate.manifest.artifacts.len(),
                evidence_refs: vec![candidate_ref],
            },
            verification: candidate.verification,
            baseline: baseline_state,
            compatibility: Compatibility {
                engine_schema: PAYLOAD_SCHEMA,
                bundle_schema,
                readable: bundle_schema <= PAYLOAD_SCHEMA,
            },
            guidance: Guidance {
                coverage: "not-shipped",
            },
        },
        operations,
        preconditions,
        decisions,
        postconditions,
        evidence: observation.ledger.into_items(),
        readiness,
        input_fingerprint: Digest::of(b""),
    };
    plan.input_fingerprint = fingerprint::compute(&plan);
    plan.identity.plan_id = fingerprint::plan_id(&plan.input_fingerprint, clock);
    Ok(Planned { plan, blobs })
}

/// The comparison's outcome for one destination.
struct Outcome {
    write: bool,
    conflict: bool,
    missing: bool,
    record: FileRecord,
}

/// A destination on a target with no record: it lands as `rk init`
/// lands it. A differing `rendered` destination is a conflict, a
/// differing `seeded` or `state` one is the target's and is kept.
fn compare_fresh(entry: &Entry, disk: Option<&[u8]>) -> Outcome {
    let (write, conflict, landed) = match disk {
        None => (true, false, entry.rendered.clone()),
        Some(bytes) if bytes == entry.rendered => (false, false, bytes.to_vec()),
        Some(bytes) if entry.kind != Kind::Rendered => (false, false, bytes.to_vec()),
        Some(_) => (false, true, entry.rendered.clone()),
    };
    Outcome {
        write,
        conflict,
        missing: false,
        record: FileRecord {
            destination: entry.destination.clone(),
            kind: entry.kind,
            sha256: Digest::of(&landed),
            baseline_sha256: baseline_digest(entry),
        },
    }
}

/// A destination on a recorded target: the three digests decide it, in
/// the shape `rk upgrade` has always decided by.
fn compare_recorded(entry: &Entry, recorded: Option<&FileRecord>, disk: Option<&[u8]>) -> Outcome {
    let Some(recorded) = recorded else {
        return compare_fresh(entry, disk);
    };
    let candidate_record = |sha256: Digest| FileRecord {
        destination: entry.destination.clone(),
        kind: entry.kind,
        sha256,
        baseline_sha256: baseline_digest(entry),
    };
    // A seeded file this payload reclassifies as rendered claims ownership
    // of a file the target may have tuned; only untouched bytes permit it.
    if recorded.kind == Kind::Seeded && entry.kind == Kind::Rendered {
        let untouched =
            disk.is_some_and(|bytes| Some(Digest::of(bytes)) == recorded.baseline_sha256);
        return Outcome {
            write: untouched,
            conflict: !untouched,
            missing: disk.is_none(),
            record: candidate_record(Digest::of(&entry.rendered)),
        };
    }
    match entry.kind {
        Kind::Rendered => match disk {
            Some(bytes) if Digest::of(bytes) == recorded.sha256 => Outcome {
                write: bytes != entry.rendered,
                conflict: false,
                missing: false,
                record: candidate_record(Digest::of(&entry.rendered)),
            },
            Some(bytes) if bytes == entry.rendered => Outcome {
                write: false,
                conflict: false,
                missing: false,
                record: candidate_record(Digest::of(&entry.rendered)),
            },
            other => Outcome {
                write: false,
                conflict: true,
                missing: other.is_none(),
                record: candidate_record(Digest::of(&entry.rendered)),
            },
        },
        Kind::Seeded => {
            // Never written; the record follows the target's bytes and
            // keeps the baseline it tunes away from.
            let baseline = if recorded.kind == Kind::Rendered {
                Some(recorded.sha256.clone())
            } else {
                recorded.baseline_sha256.clone()
            };
            let sha256 = disk.map_or_else(|| recorded.sha256.clone(), Digest::of);
            Outcome {
                write: false,
                conflict: false,
                missing: false,
                record: FileRecord {
                    destination: entry.destination.clone(),
                    kind: entry.kind,
                    sha256,
                    baseline_sha256: baseline,
                },
            }
        }
        Kind::State => Outcome {
            write: false,
            conflict: false,
            missing: false,
            record: FileRecord {
                destination: entry.destination.clone(),
                kind: entry.kind,
                sha256: recorded.sha256.clone(),
                baseline_sha256: None,
            },
        },
    }
}

/// The digest a fresh record keeps as a destination's baseline.
fn baseline_digest(entry: &Entry) -> Option<Digest> {
    match entry.kind {
        Kind::State => None,
        Kind::Rendered | Kind::Seeded => Some(Digest::of(&entry.baseline)),
    }
}

/// The recorded release's bytes for one destination, where the baseline
/// bundle carries the artifact the record's baseline digest names.
fn baseline_bytes(source: &dyn ReleaseSource, record: &Manifest, entry: &Entry) -> Option<Vec<u8>> {
    let digest = record.file(&entry.destination)?.baseline_sha256.as_ref()?;
    source.blob(digest).ok()
}

/// The record an apply writes: the candidate's identity, the resolved
/// parameters, every compared destination, and the candidate's pins,
/// with the first landing's instant and origin preserved where a record
/// exists.
fn planned_manifest<'a>(
    candidate: &Candidate<'_>,
    params: &landing::Params,
    recorded: Option<&Manifest>,
    clock: &str,
    files: impl Iterator<Item = &'a FileRecord>,
) -> Manifest {
    let pins = crate::release::read(candidate.source, &candidate.manifest, "versions.toml")
        .map(|bytes| crate::registry::pins_in(&String::from_utf8_lossy(&bytes)))
        .unwrap_or_default()
        .into_iter()
        .filter(|pin| pin.used_by.iter().any(|user| user == params.tech()))
        .map(|pin| (pin.name, pin.version))
        .collect();
    Manifest {
        schema_version: manifest::SCHEMA_VERSION,
        rk_version: candidate.manifest.release_kit_version.clone(),
        payload_sha256: candidate.manifest.payload_sha256.clone(),
        origin: recorded.map_or_else(|| "init".to_owned(), |record| record.origin.clone()),
        tech: params.tech().to_owned(),
        forge: params.forge().to_owned(),
        landed_at: recorded.map_or_else(|| clock.to_owned(), |record| record.landed_at.clone()),
        parameters: Parameters {
            repo: params.repo().to_owned(),
            workflow: params.workflow(),
            style: params.style(),
            nix: params.nix(),
            trunk: params.trunk().to_owned(),
            line_prefix: params.line_prefix().to_owned(),
            security_contact: params.security_contact().to_owned(),
            security_response: params.security_response().to_owned(),
        },
        files: files
            .map(|file| FileRecord {
                destination: file.destination.clone(),
                kind: file.kind,
                sha256: file.sha256.clone(),
                baseline_sha256: file.baseline_sha256.clone(),
            })
            .collect(),
        pins,
    }
}

/// A version with its leading `v` removed, so a manager's recorded form
/// compares against the crate's.
fn trim_v(version: &str) -> &str {
    version.strip_prefix('v').unwrap_or(version)
}
