//! The observation: everything the planner reads off the target, the
//! host, and — where asked — the forge, gathered once and stamped in the
//! evidence ledger.
//!
//! This is the I/O half of planning. It reads and never writes, and it
//! answers with data the pure planner consumes: the record as bytes and
//! as a parsed document, the configuration, every destination's bytes,
//! the repository's facts, the pin, and the forge's trunk tip where the
//! read was opted into.

use std::collections::BTreeMap;
use std::process::Command;

use camino::Utf8Path;

use crate::config::Config;
use crate::diagnostic::{Diagnostic, Reason};
use crate::digest::Digest;
use crate::error::RkError;
use crate::landing::manifest::{self, Manifest, Style, Workflow};
use crate::landing::{self, Params};
use crate::release::ReleaseSource;

use super::PinState;
use super::classify::RepositoryFacts;
use super::evidence::{EvidenceKind, Ledger};

/// The landing record, as read.
#[derive(Debug)]
pub enum RecordRead {
    /// No record at the target.
    Absent,
    /// A record this engine read.
    Present {
        /// The parsed record.
        manifest: Box<Manifest>,
        /// Its bytes, as found.
        bytes: Vec<u8>,
    },
    /// A record this engine could not read.
    Invalid {
        /// Why.
        reason: String,
    },
}

/// The committed configuration, as read.
#[derive(Debug)]
pub enum ConfigRead {
    /// No configuration at the target.
    Absent,
    /// A configuration this engine read.
    Present {
        /// The parsed configuration.
        config: Box<Config>,
        /// Its bytes, as found.
        bytes: Vec<u8>,
    },
    /// A configuration this engine could not read.
    Invalid {
        /// Why.
        reason: String,
        /// Its bytes, as found.
        bytes: Vec<u8>,
    },
}

/// What the forge said.
#[derive(Debug)]
pub enum ForgeRead {
    /// The forge was not asked.
    NotObserved {
        /// Why.
        reason: String,
    },
    /// The forge was asked about the trunk.
    Observed {
        /// The trunk asked about.
        trunk: String,
        /// The trunk's tip at the remote, where it has one.
        remote_tip: Option<String>,
        /// The forge's version, where the forge reports one and the read
        /// asked for it.
        version: Option<String>,
    },
}

/// The generator the binding's committed artifact needs, as found on the
/// host.
#[derive(Debug, Clone)]
pub struct GeneratorRead {
    /// The tool, as `versions.toml` names it.
    pub name: String,
    /// The version the host reports, where the tool answers.
    pub host: Option<String>,
}

/// The evidence ids each section of the observation cites.
#[derive(Debug, Default)]
pub struct Refs {
    /// The repository facts.
    pub repository: Vec<String>,
    /// The record.
    pub record: Option<String>,
    /// The configuration.
    pub configuration: Option<String>,
    /// Each destination read, by path.
    pub destinations: BTreeMap<String, String>,
    /// The host.
    pub host: Vec<String>,
    /// The pin.
    pub pin: Option<String>,
    /// The forge.
    pub forge: Vec<String>,
}

/// Everything observed, as data.
#[derive(Debug)]
pub struct Observation {
    /// The target, as given.
    pub target: String,
    /// Whether the target is a git repository.
    pub git: bool,
    /// The technology the version file names.
    pub tech: Option<String>,
    /// The forge the origin remote maps to.
    pub forge_name: Option<String>,
    /// The project path from the origin remote.
    pub repo: Option<String>,
    /// The facts the verdict reads.
    pub facts: RepositoryFacts,
    /// The record.
    pub record: RecordRead,
    /// The configuration.
    pub config: ConfigRead,
    /// Every destination present, by path, as bytes: the whole file, or
    /// the marked block for the two block destinations.
    pub files: BTreeMap<String, Vec<u8>>,
    /// The hook file's defect, where it has one.
    pub hooks_defect: Option<String>,
    /// The pin the wired manager records.
    pub pin: Option<PinState>,
    /// The managers whose file is present and names no release-kit.
    pub unwired_managers: Vec<String>,
    /// The generator on the host, where the technology has one and the
    /// host was asked.
    pub generator: Option<GeneratorRead>,
    /// The forge.
    pub forge: ForgeRead,
    /// The ledger, stamped as each fact was read.
    pub ledger: Ledger,
    /// The ids the sections cite.
    pub refs: Refs,
}

/// The landing parameters, resolved or not, with what withheld the Nix
/// destinations where the target cannot take them.
#[derive(Debug)]
pub struct Resolution {
    /// The parameters, where they resolved.
    pub params: Option<Params>,
    /// Which layer answered each parameter.
    pub sources: BTreeMap<String, String>,
    /// Why they did not resolve, where they did not.
    pub unresolved: Option<String>,
    /// The Nix destinations withheld, with the one reason.
    pub nix_withheld: Option<(Vec<String>, String)>,
}

/// The explicit answers a request carries.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct Flags {
    /// Binding override.
    pub tech: Option<String>,
    /// Forge override.
    pub forge: Option<String>,
    /// Repository override.
    pub repo: Option<String>,
    /// Workflow override.
    pub workflow: Option<String>,
    /// Release style override.
    pub style: Option<String>,
    /// Nix capability override.
    pub nix: Option<bool>,
}

/// One request to observe a target.
pub struct Request<'a> {
    /// The target.
    pub target: &'a Utf8Path,
    /// The explicit answers.
    pub flags: &'a Flags,
    /// The decisions selected, by id.
    pub decisions: &'a BTreeMap<String, String>,
    /// Whether to read the forge.
    pub observe_forge: bool,
    /// The instant every evidence item is stamped with.
    pub clock: &'a str,
    /// The candidate source, for the reads that validate against it.
    pub source: &'a dyn ReleaseSource,
    /// Paths beyond the landing's destinations whose presence the plan
    /// reads: the ones the candidate's guidance names.
    pub extra_paths: &'a [String],
}

/// Read the target.
///
/// # Errors
///
/// Returns [`RkError::Missing`] for a target that is not a directory,
/// the assessment's own failures where git cannot answer for a
/// repository, and [`RkError::Io`] for a read that fails for a reason
/// other than absence. A record or a configuration that does not read
/// is an observation, not a failure.
pub fn observe(request: &Request<'_>) -> Result<Observation, RkError> {
    let target = request.target;
    if !target.is_dir() {
        return Err(RkError::missing(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("target {target} is not a directory"),
            )
            .expected("an existing repository to plan for"),
        ));
    }
    let clock = request.clock;
    let mut ledger = Ledger::new();
    let mut refs = Refs::default();
    let record = read_record(target, clock, &mut ledger, &mut refs)?;
    let config = read_config(target, clock, &mut ledger, &mut refs)?;
    let facts = crate::assess::gather_facts(target)?;
    refs.repository.push(ledger.observe(
        "repository",
        EvidenceKind::Repository,
        "rk",
        clock,
        None,
        "marker scan, payload destinations, git tag --list, git for-each-ref, version file",
    ));
    let files = read_destinations(
        target,
        &record,
        request.extra_paths,
        clock,
        &mut ledger,
        &mut refs,
    )?;
    let hooks_defect = landing::hooks_file_defect(request.source, target)?;
    refs.host.push(ledger.observe(
        "host",
        EvidenceKind::Host,
        "rk",
        clock,
        None,
        "the engine's own version",
    ));
    let (pin, unwired_managers) = read_pin(target, clock, &mut ledger, &mut refs);
    let forge = if request.observe_forge {
        let trunk = crate::config::trunk_of(target.as_std_path())?;
        let remote_tip = remote_tip(target, &trunk)?;
        refs.forge.push(ledger.observe(
            "forge:trunk",
            EvidenceKind::Forge,
            "git",
            clock,
            None,
            format!("git ls-remote --heads origin {trunk}"),
        ));
        ForgeRead::Observed {
            trunk,
            remote_tip,
            version: None,
        }
    } else {
        ForgeRead::NotObserved {
            reason: "the forge read was not requested; --observe forge opts in".into(),
        }
    };
    Ok(Observation {
        target: target.to_string(),
        git: facts.git,
        tech: facts.tech.map(str::to_owned),
        forge_name: facts.forge.map(str::to_owned),
        repo: facts.repo.clone(),
        facts: RepositoryFacts {
            release_markers: facts.release_markers,
            collisions: facts.collisions,
            tags: facts.tags,
            long_lived_branches: facts.long_lived_branches,
        },
        record,
        config,
        files,
        hooks_defect,
        pin,
        unwired_managers,
        generator: None,
        forge,
        ledger,
        refs,
    })
}

/// Read the host tools the resolved parameters make relevant.
///
/// Each read is stamped: the binding's generator, and the forge's version
/// where the forge was already asked and is one that reports a floor.
/// This runs after the resolution, because which tool matters depends on
/// it.
pub fn observe_host_tools(
    observation: &mut Observation,
    tech: Option<&str>,
    forge: Option<&str>,
    clock: &str,
) {
    if let Some((name, _)) = tech.and_then(super::compatibility::generator_for) {
        let host = generator_version(name);
        observation.refs.host.push(observation.ledger.observe(
            format!("host:{name}"),
            EvidenceKind::Host,
            name,
            clock,
            None,
            format!("{} --version", generator_bin(name)),
        ));
        observation.generator = Some(GeneratorRead {
            name: name.to_owned(),
            host,
        });
    }
    if let (Some("gitlab"), ForgeRead::Observed { version, .. }) = (forge, &mut observation.forge) {
        *version = gitlab_version();
        observation.refs.forge.push(observation.ledger.observe(
            "forge:version",
            EvidenceKind::Forge,
            "glab",
            clock,
            None,
            "glab api version",
        ));
    }
}

/// The binary a generator runs as, honoring the override that keeps the
/// tests hermetic: `RK_DIST_BIN` for cargo-dist.
fn generator_bin(name: &str) -> String {
    match name {
        "cargo-dist" => std::env::var("RK_DIST_BIN").unwrap_or_else(|_| "dist".to_owned()),
        other => other.to_owned(),
    }
}

/// The generator's version as the host reports it, or `None` where the
/// tool is absent or answers nothing readable.
fn generator_version(name: &str) -> Option<String> {
    let out = Command::new(generator_bin(name))
        .arg("--version")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .find(|word| crate::release::declared::version_key(word).is_some())
        .map(|word| word.trim_start_matches('v').to_owned())
}

/// The GitLab instance's version through the forge CLI's read-only
/// `GET /version`, or `None` where nothing readable comes back.
fn gitlab_version() -> Option<String> {
    let out = Command::new(crate::probes::forge_bin(crate::detect::Forge::Gitlab))
        .args(["api", "version"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    body["version"].as_str().map(str::to_owned)
}

/// The record, as bytes and as a document, stamped.
fn read_record(
    target: &Utf8Path,
    clock: &str,
    ledger: &mut Ledger,
    refs: &mut Refs,
) -> Result<RecordRead, RkError> {
    let bytes = read_optional(&target.join(manifest::MANIFEST_PATH))?;
    let record = match (&bytes, manifest::load(target)) {
        (None, _) => RecordRead::Absent,
        (Some(bytes), Ok(Some(manifest))) => RecordRead::Present {
            manifest: Box::new(manifest),
            bytes: bytes.clone(),
        },
        (Some(_), Ok(None)) => RecordRead::Invalid {
            reason: "the record vanished between two reads".into(),
        },
        (Some(_), Err(error)) => RecordRead::Invalid {
            reason: error.to_string(),
        },
    };
    refs.record = Some(ledger.observe(
        "record",
        EvidenceKind::Record,
        "rk",
        clock,
        bytes.as_deref().map(Digest::of),
        format!("read {}", manifest::MANIFEST_PATH),
    ));
    Ok(record)
}

/// The configuration, as bytes and as a document, stamped.
fn read_config(
    target: &Utf8Path,
    clock: &str,
    ledger: &mut Ledger,
    refs: &mut Refs,
) -> Result<ConfigRead, RkError> {
    let bytes = read_optional(&target.join(crate::config::CONFIG_PATH))?;
    let config = match (&bytes, crate::config::load(target.as_std_path())) {
        (None, _) => ConfigRead::Absent,
        (Some(bytes), Ok(Some(config))) => ConfigRead::Present {
            config: Box::new(config),
            bytes: bytes.clone(),
        },
        (Some(bytes), Ok(None)) => ConfigRead::Invalid {
            reason: "the configuration vanished between two reads".into(),
            bytes: bytes.clone(),
        },
        (Some(bytes), Err(error)) => ConfigRead::Invalid {
            reason: error.to_string(),
            bytes: bytes.clone(),
        },
    };
    refs.configuration = Some(ledger.observe(
        "configuration",
        EvidenceKind::Configuration,
        "rk",
        clock,
        bytes.as_deref().map(Digest::of),
        format!("read {}", crate::config::CONFIG_PATH),
    ));
    Ok(config)
}

/// Every destination the payload can land, and every one the record
/// names, as bytes, each stamped.
fn read_destinations(
    target: &Utf8Path,
    record: &RecordRead,
    extra_paths: &[String],
    clock: &str,
    ledger: &mut Ledger,
    refs: &mut Refs,
) -> Result<BTreeMap<String, Vec<u8>>, RkError> {
    let mut paths: Vec<String> = landing::destinations().map(str::to_owned).collect();
    if let RecordRead::Present { manifest, .. } = record {
        paths.extend(manifest.files.iter().map(|file| file.destination.clone()));
    }
    paths.extend(extra_paths.iter().cloned());
    paths.sort();
    paths.dedup();
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for path in paths {
        let bytes = landing::read_recorded(target, &path)?;
        let id = ledger.observe(
            format!("destination:{path}"),
            EvidenceKind::Destination,
            "rk",
            clock,
            bytes.as_deref().map(Digest::of),
            if landing::block_markers(&path).is_some() {
                "read the marked block"
            } else {
                "read the file"
            },
        );
        refs.destinations.insert(path.clone(), id);
        if let Some(bytes) = bytes {
            files.insert(path, bytes);
        }
    }
    Ok(files)
}

/// The pin the wired manager records, where exactly one names
/// release-kit, stamped, beside the managers whose file is present and
/// names none.
fn read_pin(
    target: &Utf8Path,
    clock: &str,
    ledger: &mut Ledger,
    refs: &mut Refs,
) -> (Option<PinState>, Vec<String>) {
    let Ok(observed) = crate::self_depend::observe(target) else {
        return (None, Vec::new());
    };
    let unwired: Vec<String> = observed
        .managers
        .iter()
        .filter(|entry| {
            entry.present == crate::self_depend::Presence::Present && entry.pin == "unpinned"
        })
        .map(|entry| entry.manager.as_str().to_owned())
        .collect();
    let pin = observed.wired.and_then(|manager| {
        let entry = observed.entry(manager)?;
        Some(PinState {
            manager: manager.as_str().to_owned(),
            file: entry.file.clone()?,
            version: entry.version.clone()?,
        })
    });
    if let Some(pin) = &pin {
        refs.pin = Some(ledger.observe(
            "pin",
            EvidenceKind::Pin,
            "rk self-depend",
            clock,
            files_digest(target, &pin.file),
            format!("read {}", pin.file),
        ));
    }
    (pin, unwired)
}

/// Resolve the landing parameters over the flags, the decisions, the
/// configuration, and the record, and say which layer answered each.
///
/// A decision the operator selected for the workflow mode or the release
/// style answers as a flag would. The resolution never refuses: an
/// unresolved identity is a reason the planner turns into a blocked
/// precondition.
///
/// # Errors
///
/// Returns [`RkError::Usage`] for a flag value outside its grammar, and
/// the Nix shape read's failures other than absence.
pub fn resolve(request: &Request<'_>, observation: &Observation) -> Result<Resolution, RkError> {
    let flags = request.flags;
    let workflow_flag = flags
        .workflow
        .clone()
        .or_else(|| request.decisions.get("workflow-mode").cloned());
    let style_flag = flags
        .style
        .clone()
        .or_else(|| request.decisions.get("release-style").cloned());
    let inputs = landing::Inputs {
        tech: flags.tech.as_deref(),
        forge: flags.forge.as_deref(),
        repo: flags.repo.as_deref(),
        workflow: workflow_flag.as_deref().map(Workflow::parse).transpose()?,
        style: style_flag.as_deref().map(Style::parse).transpose()?,
        nix: flags.nix,
    };
    let config: Option<&Config> = match &observation.config {
        ConfigRead::Present { config, .. } => Some(config),
        ConfigRead::Absent | ConfigRead::Invalid { .. } => None,
    };
    let record: Option<&Manifest> = match &observation.record {
        RecordRead::Present { manifest, .. } => Some(manifest),
        RecordRead::Absent | RecordRead::Invalid { .. } => None,
    };
    let sources = sources(
        &inputs,
        flags,
        request.decisions,
        config,
        record,
        observation,
    );
    let resolved = Params::resolve(
        request.source,
        request.target,
        &inputs,
        config,
        record,
        landing::Purpose::Preview,
    );
    let (params, unresolved) = match resolved {
        Ok(params) => (Some(params), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let nix_withheld = match &params {
        Some(params) if params.nix() => landing::nix_withholding(request.target, record)?
            .map(|(set, reason)| (set.iter().map(|path| (*path).to_owned()).collect(), reason)),
        _ => None,
    };
    Ok(Resolution {
        params,
        sources,
        unresolved,
        nix_withheld,
    })
}

/// Which layer answered each parameter: `flag`, `decision`,
/// `configuration`, `record`, `detected`, or `default`.
fn sources(
    inputs: &landing::Inputs<'_>,
    flags: &Flags,
    decisions: &BTreeMap<String, String>,
    config: Option<&Config>,
    record: Option<&Manifest>,
    observation: &Observation,
) -> BTreeMap<String, String> {
    let mut sources = BTreeMap::new();
    let layer = |flag: bool, configured: bool, recorded: bool, detected: bool| {
        if flag {
            "flag"
        } else if configured {
            "configuration"
        } else if recorded {
            "record"
        } else if detected {
            "detected"
        } else {
            "default"
        }
    };
    identity_sources(&mut sources, inputs, config, record, observation, layer);
    let decided = |id: &str, flag: bool, configured: bool, recorded: bool| {
        if flag {
            "flag"
        } else if decisions.contains_key(id) {
            "decision"
        } else if configured {
            "configuration"
        } else if recorded {
            "record"
        } else {
            "default"
        }
    };
    sources.insert(
        "workflow".to_owned(),
        decided(
            "workflow-mode",
            flags.workflow.is_some(),
            config.is_some_and(|c| c.landing.workflow.is_some()),
            record.is_some(),
        )
        .to_owned(),
    );
    sources.insert(
        "style".to_owned(),
        decided(
            "release-style",
            flags.style.is_some(),
            config.is_some_and(|c| c.landing.style.is_some()),
            record.is_some_and(|r| r.parameters.style.is_some()),
        )
        .to_owned(),
    );
    sources.insert(
        "nix".to_owned(),
        layer(
            inputs.nix.is_some(),
            config.is_some_and(|c| c.landing.nix.is_some()),
            record.is_some(),
            false,
        )
        .to_owned(),
    );
    for (key, configured) in [
        ("trunk", config.is_some_and(|c| c.project.trunk.is_some())),
        (
            "line_prefix",
            config.is_some_and(|c| c.setup.line_prefix.is_some()),
        ),
        (
            "security_contact",
            config.is_some_and(|c| c.security.contact.is_some()),
        ),
        (
            "security_response",
            config.is_some_and(|c| c.security.response.is_some()),
        ),
    ] {
        sources.insert(
            key.to_owned(),
            layer(false, configured, record.is_some(), false).to_owned(),
        );
    }
    sources
}

/// The three identity parameters: flag, configuration, record, or the
/// detection.
fn identity_sources(
    sources: &mut BTreeMap<String, String>,
    inputs: &landing::Inputs<'_>,
    config: Option<&Config>,
    record: Option<&Manifest>,
    observation: &Observation,
    layer: impl Fn(bool, bool, bool, bool) -> &'static str,
) {
    sources.insert(
        "tech".to_owned(),
        layer(
            inputs.tech.is_some(),
            config.is_some_and(|c| !c.project.tech.is_empty()),
            record.is_some(),
            observation.tech.is_some(),
        )
        .to_owned(),
    );
    sources.insert(
        "forge".to_owned(),
        layer(
            inputs.forge.is_some(),
            config.is_some_and(|c| !c.project.forge.is_empty()),
            record.is_some(),
            observation.forge_name.is_some(),
        )
        .to_owned(),
    );
    sources.insert(
        "repo".to_owned(),
        layer(
            inputs.repo.is_some(),
            config.is_some_and(|c| !c.project.repo.is_empty()),
            record.is_some(),
            observation.repo.is_some(),
        )
        .to_owned(),
    );
}

/// The bytes at `path`, or `None` where nothing is there.
fn read_optional(path: &Utf8Path) -> Result<Option<Vec<u8>>, RkError> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RkError::Io(error)),
    }
}

/// The digest of a file under the target, where it reads.
fn files_digest(target: &Utf8Path, rel: &str) -> Option<Digest> {
    std::fs::read(target.join(rel))
        .ok()
        .map(|bytes| Digest::of(&bytes))
}

/// The trunk's tip at `origin`, through one read-only git call.
///
/// A remote without the branch answers `None`; a remote that cannot be
/// reached is a failure, because a forge asked and not answering is not
/// an observation.
fn remote_tip(target: &Utf8Path, trunk: &str) -> Result<Option<String>, RkError> {
    let mut command = Command::new(crate::probes::git_bin());
    for var in crate::maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .arg("-C")
        .arg(target)
        .args(["ls-remote", "--heads", "origin", trunk])
        .output()
        .map_err(|error| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("git could not be spawned: {error}"),
                )
                .expected("git on PATH, or RK_GIT_BIN naming it"),
            )
        })?;
    if !out.status.success() {
        return Err(RkError::subprocess(
            Diagnostic::new(
                Reason::ForgeTemporary,
                format!(
                    "git ls-remote could not read origin: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            )
            .expected("a reachable origin remote, or a plan without --observe forge"),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| line.split_whitespace().next().map(str::to_owned)))
}
