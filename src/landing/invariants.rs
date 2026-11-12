//! The invariants a seeded file still carries.
//!
//! A `seeded` file is the target's to tune — nothing here rewrites one —
//! but the narrow part the invariants own is judged: a target may choose
//! its platforms, its installers, and its install path; it may not choose
//! to ship unattested. The judgment reads the effective configuration,
//! never the text: a commented key, a `false` value, or an unpaired phase
//! must fail, and whitespace or key order must not matter. The table is
//! keyed by `(technology, forge, destination)` — the kind table is
//! destination-keyed, and a second pair sharing a destination would
//! otherwise silently inherit the wrong rule.
//!
//! A second, pair-keyed table judges target-wide relationships that need
//! files read from the target's disk: generated files the seeds ship no
//! copy of, and target-owned files landed configuration requires. No digest
//! records a generated file, so nothing else sees it drift away from the
//! configuration it was generated from. That judgment reads the generated
//! text, because the generator is not available to re-run and the text is
//! what the forge executes. It reads the grammar the generator writes and
//! reports what it cannot resolve; a workflow hand-authored in some further
//! YAML presentation is beyond a text reader, and the generator's own check
//! stays the whole-file proof.

use camino::Utf8Path;
use serde::Serialize;

use crate::embedded;
use crate::profile::RegistryAnswer;

/// One invariant a landed file's effective configuration violates: a
/// stable code, the destination, why, and exactly what to write — the
/// operator is told the remediation, never just what was not found.
#[derive(Debug, Clone, Serialize)]
pub struct InvariantFailure {
    /// The stable machine code of the failed rule.
    pub code: &'static str,
    /// The landed destination the failure is about.
    pub destination: String,
    /// Why the configuration violates the invariant.
    pub reason: String,
    /// Exactly what to write to satisfy the rule.
    pub remediation: &'static str,
}

impl InvariantFailure {
    fn new(
        code: &'static str,
        destination: &str,
        reason: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self {
            code,
            destination: destination.to_owned(),
            reason: reason.into(),
            remediation,
        }
    }
}

/// Judge one landed file against the rules its `(tech, forge,
/// destination)` key owns.
///
/// A destination no rule owns fails nothing. `registry` is the resolved
/// `profile.release.registry` answer, absent where the target never
/// answered it.
#[must_use]
pub fn failures(
    tech: &str,
    forge: &str,
    destination: &str,
    bytes: &[u8],
    registry: Option<RegistryAnswer>,
) -> Vec<InvariantFailure> {
    match (tech, forge, destination) {
        ("rust", "github", "dist-workspace.toml") => dist_workspace(destination, bytes),
        ("rust", _, "release-plz.toml") => release_plz(destination, bytes, registry),
        _ => Vec::new(),
    }
}

/// The parsed TOML of a landed configuration, or the one failure that
/// says it cannot be judged.
fn parsed(destination: &str, bytes: &[u8]) -> Result<toml::Table, InvariantFailure> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        InvariantFailure::new(
            "unparsable-configuration",
            destination,
            "the file is not UTF-8, so its configuration cannot be judged",
            "repair the file so it parses as TOML",
        )
    })?;
    text.parse().map_err(|error| {
        InvariantFailure::new(
            "unparsable-configuration",
            destination,
            format!("the file does not parse as TOML: {error}"),
            "repair the file so it parses as TOML",
        )
    })
}

/// One boolean key of `release-plz.toml`, as the file states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Flag {
    /// The key is not set.
    Absent,
    /// The key is set to a boolean.
    Set(bool),
    /// The key is set to a value of another type.
    Malformed,
}

impl Flag {
    /// A package value over the workspace one.
    const fn over(self, workspace: Self) -> Self {
        match self {
            Self::Absent => workspace,
            set => set,
        }
    }
}

/// Every scope of `release-plz.toml` that states `publish` or `git_only`:
/// `[workspace]`, then each `[[package]]` that sets either, a package value
/// over the workspace one.
///
/// A present value of another type is not an absent key: release-plz
/// refuses it, so it is reported once by scope and key into `failures`
/// and judged no further, never read as a default or a contradiction.
fn release_plz_scopes(
    destination: &str,
    table: &toml::Table,
    failures: &mut Vec<InvariantFailure>,
) -> Vec<(String, Flag, Flag)> {
    let mut flag = |scope_name: &str, scope: Option<&toml::Table>, key: &str| -> Flag {
        match scope.and_then(|scope| scope.get(key)) {
            None => Flag::Absent,
            Some(toml::Value::Boolean(value)) => Flag::Set(*value),
            Some(other) => {
                failures.push(InvariantFailure::new(
                    "release-plz-flag-not-boolean",
                    destination,
                    format!(
                        "{scope_name} sets {key} = {other}, which is no boolean, so release-plz refuses the file"
                    ),
                    "write publish and git_only as true or false, unquoted",
                ));
                Flag::Malformed
            }
        }
    };
    let workspace = table.get("workspace").and_then(toml::Value::as_table);
    let workspace_flags = (
        flag("[workspace]", workspace, "publish"),
        flag("[workspace]", workspace, "git_only"),
    );
    let mut scopes = vec![(
        "[workspace]".to_owned(),
        workspace_flags.0,
        workspace_flags.1,
    )];
    for package in table
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_table)
    {
        let name = format!(
            "the [[package]] {}",
            package
                .get("name")
                .and_then(toml::Value::as_str)
                .unwrap_or("an unnamed package")
        );
        let (publish, git_only) = (
            flag(&name, Some(package), "publish"),
            flag(&name, Some(package), "git_only"),
        );
        if publish == Flag::Absent && git_only == Flag::Absent {
            continue;
        }
        scopes.push((
            name,
            publish.over(workspace_flags.0),
            git_only.over(workspace_flags.1),
        ));
    }
    scopes
}

/// The rust release-plz configuration: `publish` and `git_only` agree
/// with each other and with where the crate publishes. release-plz
/// compares with the registry unless `git_only` is on, so a crate that
/// publishes nothing and reads no tags never moves past its last
/// published version; release-plz refuses `publish = true` beside
/// `git_only = true`. The judgment reads `[workspace]` and every
/// `[[package]]` that overrides either key, a package value over the
/// workspace one.
///
/// SATISFIES landing:a-seeded-file-still-carries-the-invariants
fn release_plz(
    destination: &str,
    bytes: &[u8],
    registry: Option<RegistryAnswer>,
) -> Vec<InvariantFailure> {
    let table = match parsed(destination, bytes) {
        Ok(table) => table,
        Err(failure) => return vec![failure],
    };
    let mut failures = Vec::new();
    let scopes = release_plz_scopes(destination, &table, &mut failures);
    let answer = registry.unwrap_or(RegistryAnswer::CratesIo);
    for (scope, publish, git_only) in scopes {
        if publish == Flag::Malformed || git_only == Flag::Malformed {
            continue;
        }
        let git_only = git_only == Flag::Set(true);
        if publish == Flag::Set(false) && !git_only {
            failures.push(InvariantFailure::new(
                "publish-off-without-git-only",
                destination,
                format!(
                    "{scope} sets publish = false without git_only = true, so release-plz compares with the registry and never proposes a version past the last published one"
                ),
                "set git_only = true beside publish = false, and set profile.release.registry = \"none\" in .release-kit/config.toml",
            ));
        }
        if publish == Flag::Set(true) && git_only {
            failures.push(InvariantFailure::new(
                "git-only-with-publish",
                destination,
                format!(
                    "{scope} sets git_only = true beside publish = true, a pair release-plz refuses"
                ),
                "remove publish = true, or set publish = false",
            ));
        }
        match (answer, git_only) {
            (RegistryAnswer::None, false) => failures.push(InvariantFailure::new(
                "registry-answer-disagrees",
                destination,
                format!(
                    "profile.release.registry is none, and {scope} does not set git_only = true, so release-plz reads the registry for a crate that publishes to none"
                ),
                "set publish = false and git_only = true in [workspace] of release-plz.toml, or set profile.release.registry = \"crates-io\" in .release-kit/config.toml",
            )),
            (RegistryAnswer::CratesIo, true) => failures.push(InvariantFailure::new(
                "registry-answer-disagrees",
                destination,
                format!(
                    "profile.release.registry is crates-io or unanswered, and {scope} sets git_only = true, so release-plz never publishes the crate"
                ),
                "set profile.release.registry = \"none\" in .release-kit/config.toml, or remove git_only from release-plz.toml and set publish = true",
            )),
            _ => {}
        }
    }
    failures
}

/// The rust/github attestation configuration: attestations on, minted in
/// the `host` phase where every hosted asset is gathered before the
/// release page exists, the release creation paired with that phase, and
/// no narrowing filter — the default `["*"]` covers every hosted file,
/// where an enumerated list goes quiet when an archive format moves.
fn dist_workspace(destination: &str, bytes: &[u8]) -> Vec<InvariantFailure> {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return vec![InvariantFailure::new(
            "unparsable-configuration",
            destination,
            "the file is not UTF-8, so its configuration cannot be judged",
            "repair the file so it parses as TOML",
        )];
    };
    let table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(error) => {
            return vec![InvariantFailure::new(
                "unparsable-configuration",
                destination,
                format!("the file does not parse as TOML: {error}"),
                "repair the file so it parses as TOML",
            )];
        }
    };
    let dist = table.get("dist").and_then(toml::Value::as_table);
    let mut failures = Vec::new();
    let value = |key: &str| dist.and_then(|dist| dist.get(key));
    if value("github-attestations").and_then(toml::Value::as_bool) != Some(true) {
        failures.push(InvariantFailure::new(
            "attestations-disabled",
            destination,
            "github-attestations is not effectively true, so no release artifact is attested",
            "set github-attestations = true in [dist]",
        ));
    }
    let phase = value("github-attestations-phase").and_then(toml::Value::as_str);
    if phase != Some("host") {
        failures.push(InvariantFailure::new(
            "attestation-phase-not-host",
            destination,
            phase.map_or_else(
                || "github-attestations-phase is unset, so the default phase attests only the per-platform archives and the curled installers ship unattested".to_owned(),
                |other| format!(
                    "github-attestations-phase is \"{other}\"; only the host phase attests every asset before the release page exists"
                ),
            ),
            "set github-attestations-phase = \"host\" in [dist]",
        ));
    }
    if value("github-release").and_then(toml::Value::as_str) != Some("host") {
        failures.push(InvariantFailure::new(
            "release-phase-unpaired",
            destination,
            "github-release is not \"host\", leaving the release creation unpaired with the attest phase",
            "set github-release = \"host\" in [dist], pairing the release creation with the phase that attests",
        ));
    }
    if value("github-attestations-filters").is_some() {
        failures.push(InvariantFailure::new(
            "attestation-filters-narrowed",
            destination,
            "github-attestations-filters narrows what is attested below the whole set of release artifacts",
            "remove github-attestations-filters from [dist]; the default [\"*\"] attests every hosted file",
        ));
    }
    // cargo-dist's default puts a plan job on every pull request. The
    // trunk protection requires one project-owned context beside the
    // landed title check, and `needs` resolves inside one workflow file,
    // so a job in the generated file is in no gate's needs: it reports a
    // status nothing holds, and under the standing arm a red one merges.
    let mode = value("pr-run-mode").and_then(toml::Value::as_str);
    if mode != Some("skip") {
        failures.push(InvariantFailure::new(
            "pr-run-mode-not-skip",
            destination,
            mode.map_or_else(
                || "pr-run-mode is unset, so it defaults to plan and the generated workflow reports a job on every pull request that no gate can need".to_owned(),
                |other| format!(
                    "pr-run-mode is \"{other}\", so the generated workflow reports a job on every pull request that no gate can need"
                ),
            ),
            "set pr-run-mode = \"skip\" in [dist] and regenerate with dist generate, then run dist plan and the dist generate proof as a job of the workflow the required check gates",
        ));
    }
    failures.extend(action_commit_failures(
        destination,
        value("github-action-commits").and_then(toml::Value::as_table),
    ));
    failures
}

/// The build that signs is itself pinned by digest: the seed's
/// `[dist.github-action-commits]` table pins the actions cargo-dist
/// injects — the attest step among them — and a landed target must carry
/// the same effective table, or its signer runs code a moved tag can
/// swap.
fn action_commit_failures(destination: &str, found: Option<&toml::Table>) -> Vec<InvariantFailure> {
    let remediation = "bring the [dist.github-action-commits] table to the embedded seed's (rk snippet rust/github/dist-workspace.toml) and regenerate with dist generate --mode ci";
    let mut failures = Vec::new();
    for (action, commit) in &seed_action_commits() {
        // Three distinct states, each with its own true reason: an
        // absent entry falls back to the movable tag, a non-string value
        // is invalid configuration, and a mismatched string executes an
        // immutable commit that is just not the seed's.
        match found.and_then(|table| table.get(action)) {
            Some(value) => match value.as_str() {
                Some(pinned) if pinned == commit.as_str() => {}
                Some(pinned) => failures.push(InvariantFailure::new(
                    "action-commit-stale",
                    destination,
                    format!(
                        "[dist.github-action-commits] pins {action} at {pinned}, where the seed pins {commit}"
                    ),
                    remediation,
                )),
                None => failures.push(InvariantFailure::new(
                    "action-commit-invalid",
                    destination,
                    format!(
                        "[dist.github-action-commits] pins {action} with a non-string value; a pin is a full commit SHA string"
                    ),
                    remediation,
                )),
            },
            None => failures.push(InvariantFailure::new(
                "action-commit-missing",
                destination,
                format!(
                    "[dist.github-action-commits] does not pin {action}, so the workflow runs whatever the movable tag names"
                ),
                remediation,
            )),
        }
    }
    failures
}

/// The action commits the embedded seed pins, read from the
/// embedded snippet so the judgment and the seed cannot drift apart.
fn seed_action_commits() -> Vec<(String, String)> {
    let Some(text) = embedded::SNIPPETS
        .get_file("rust/github/dist-workspace.toml")
        .and_then(|file| file.contents_utf8())
    else {
        return Vec::new();
    };
    let Ok(table) = text.parse::<toml::Table>() else {
        return Vec::new();
    };
    table
        .get("dist")
        .and_then(toml::Value::as_table)
        .and_then(|dist| dist.get("github-action-commits"))
        .and_then(toml::Value::as_table)
        .map(|commits| {
            commits
                .iter()
                .filter_map(|(action, commit)| {
                    commit
                        .as_str()
                        .map(|commit| (action.clone(), commit.to_owned()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The generated file the cross-file failures name: cargo-dist writes it
/// from `dist-workspace.toml`, the seeds ship no copy, and the forge
/// executes it.
const GENERATED_WORKFLOW: &str = ".github/workflows/release.yml";

/// Judge target-wide relationships, keyed by `(technology, forge)`.
///
/// A destination-keyed rule cannot reach a second file: the byte reader
/// receives only one landed destination. The pair-keyed rules read files
/// from the target's own disk, including generated files the seeds ship
/// no copy of and target-owned files landed configuration requires.
#[must_use]
pub fn target_failures(tech: &str, forge: &str, target: &Utf8Path) -> Vec<InvariantFailure> {
    match (tech, forge) {
        ("rust", "github") => {
            let mut failures = generated_release_workflow(target);
            failures.extend(dist_profile(target));
            failures
        }
        _ => Vec::new(),
    }
}

/// The code of a gate that names no check: the judgment then reads every
/// pull-request workflow, which it states as a limitation.
pub const GATE_UNNAMED: &str = "gate-unnamed";

/// The code of a `dist build` that runs under a condition: it proves no
/// merge, which the judgment states as a limitation.
pub const DIST_BUILD_CONDITIONAL: &str = "dist-build-conditional";

/// The directory every forge workflow lives in.
const WORKFLOW_DIR: &str = ".github/workflows";

/// What the dist target judgment found: the failures `--check` counts,
/// and the limitations a report states without counting them.
#[derive(Debug, Default)]
pub struct DistGate {
    /// The failures, each a violation under `--check`.
    pub failures: Vec<InvariantFailure>,
    /// The limitations, as a stable code and the reason.
    pub limitations: Vec<(&'static str, String)>,
}

/// One workflow the gate can reach, parsed.
struct GatedWorkflow {
    /// The path, relative to the target.
    path: String,
    /// Whether it runs on `pull_request` or `merge_group`.
    on_requests: bool,
    /// Whether that request trigger carries a `paths` or `paths-ignore`
    /// filter, so the workflow does not run on every request.
    filtered: bool,
    /// The workflow-level environment.
    env: std::collections::BTreeMap<String, String>,
    /// Every job, by id, in file order.
    jobs: Vec<(String, yaml_serde::Value)>,
}

/// What an artifacts matrix covers.
enum PlanCredit {
    /// Every target, from an unnarrowed plan.
    Every,
    /// The triples a narrowing `--target` names.
    Named(std::collections::BTreeSet<String>),
}

/// What one `dist build` step proves.
enum Credit {
    /// The triples it builds before a merge.
    Targets(std::collections::BTreeSet<String>),
    /// It runs under a condition, so it proves no merge.
    Conditional,
    /// Its workflow runs only on the requests a path filter selects.
    Filtered,
    /// Its arguments or its matrix cannot be resolved.
    Unreadable(String),
}

/// SATISFIES landing:a-seeded-file-still-carries-the-invariants
///
/// Every target of `dist-workspace.toml` is built by a job the gate needs
/// before a merge. The gate is the job whose check context is
/// `setup.required_check`, or, where that names nothing, every job of
/// every pull-request workflow. A target counts as built where a judged
/// job runs `dist build` with no condition and either names the triple,
/// takes it from a literal matrix list, or consumes `matrix.dist_args`
/// from the `artifacts_matrix` of an unnarrowed `dist plan` in a judged
/// job. The generated `release.yml` runs on the tag and proves nothing
/// before a merge, so it is not read. An absent configuration or an
/// absent workflow directory reports nothing. `dev_run` is the run prefix
/// of the recorded `dev_environment`, empty where none is recorded: a
/// `dist` command after exactly that prefix, word for word, runs in the
/// target's own environment and is read like a bare one.
#[must_use]
pub fn dist_targets_built(target: &Utf8Path, required_check: &str, dev_run: &str) -> DistGate {
    let recorded: Vec<&str> = dev_run.split_whitespace().collect();
    let mut gate = DistGate::default();
    let Some(targets) = dist_targets(target) else {
        return gate;
    };
    let workflows = gated_workflows(target);
    if workflows.is_empty() {
        return gate;
    }
    let Some(judged) = judged_jobs(&workflows, required_check, &mut gate) else {
        return gate;
    };
    let mut built = std::collections::BTreeSet::new();
    for (index, id) in &judged {
        let workflow = &workflows[*index];
        let Some(job) = job_of(workflow, id) else {
            continue;
        };
        let peers: Vec<&str> = judged
            .iter()
            .filter(|(other, _)| other == index)
            .map(|(_, peer)| peer.as_str())
            .collect();
        for step in steps_of(job) {
            let Some(run) = step.get("run").and_then(yaml_serde::Value::as_str) else {
                continue;
            };
            if !run.contains("dist build") {
                continue;
            }
            match credit(workflow, job, step, run, &peers, &targets, &recorded) {
                Credit::Targets(triples) => built.extend(triples),
                Credit::Conditional => gate.limitations.push((
                    DIST_BUILD_CONDITIONAL,
                    format!(
                        "{} job {id} runs dist build under a condition, so it proves no merge and counts no target",
                        workflow.path
                    ),
                )),
                Credit::Filtered => gate.limitations.push((
                    DIST_BUILD_CONDITIONAL,
                    format!(
                        "{} job {id} runs dist build in a workflow whose request trigger carries a paths filter, so it proves no merge and counts no target",
                        workflow.path
                    ),
                )),
                Credit::Unreadable(what) => gate.failures.push(InvariantFailure::new(
                    "dist-build-unreadable",
                    &workflow.path,
                    format!("job {id} runs dist build with {what}, which the judgment cannot resolve"),
                    "write the dist build step in the shape rk binding rust gives, or name its targets literally",
                )),
            }
        }
    }
    for triple in targets.iter().filter(|triple| !built.contains(*triple)) {
        gate.failures.push(InvariantFailure::new(
            "dist-target-unbuilt",
            "dist-workspace.toml",
            format!(
                "no job the gate needs builds {triple} with dist build before a merge, so a target the code cannot build fails only after the tag exists"
            ),
            "add the dist-plan and dist-build jobs of rk binding rust to the gate's needs, or drop the target from dist-workspace.toml",
        ));
    }
    gate
}

/// The targets `dist-workspace.toml` lists, or `None` where nothing is
/// there to judge.
fn dist_targets(target: &Utf8Path) -> Option<Vec<String>> {
    let text = std::fs::read_to_string(target.join("dist-workspace.toml")).ok()?;
    let table: toml::Table = text.parse().ok()?;
    let targets = table
        .get("dist")
        .and_then(toml::Value::as_table)
        .and_then(|dist| dist.get("targets"))
        .and_then(toml::Value::as_array)?;
    let targets: Vec<String> = targets
        .iter()
        .filter_map(toml::Value::as_str)
        .map(str::to_owned)
        .collect();
    (!targets.is_empty()).then_some(targets)
}

/// Every workflow under `.github/workflows/` but the generated one, in
/// name order, each parsed. A file that does not parse is skipped: the
/// forge cannot run it either.
fn gated_workflows(target: &Utf8Path) -> Vec<GatedWorkflow> {
    let Ok(entries) = std::fs::read_dir(target.join(WORKFLOW_DIR)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            std::path::Path::new(name)
                .extension()
                .is_some_and(|extension| extension == "yml" || extension == "yaml")
                && format!("{WORKFLOW_DIR}/{name}") != GENERATED_WORKFLOW
        })
        .collect();
    names.sort();
    names
        .into_iter()
        .filter_map(|name| {
            let path = format!("{WORKFLOW_DIR}/{name}");
            let text = std::fs::read_to_string(target.join(&path)).ok()?;
            let value: yaml_serde::Value = yaml_serde::from_str(&text).ok()?;
            let jobs = value
                .get("jobs")
                .and_then(yaml_serde::Value::as_mapping)
                .map(|jobs| {
                    jobs.iter()
                        .filter_map(|(id, job)| Some((id.as_str()?.to_owned(), job.clone())))
                        .collect()
                })
                .unwrap_or_default();
            let (on_requests, filtered) = request_trigger(&value);
            Some(GatedWorkflow {
                path,
                on_requests,
                filtered,
                env: env_of(value.get("env")),
                jobs,
            })
        })
        .collect()
}

/// Whether a workflow's trigger names `pull_request` or `merge_group`,
/// and whether that trigger carries a path filter. YAML 1.1 reads an
/// unquoted `on` as `true`, so both keys are read.
fn request_trigger(workflow: &yaml_serde::Value) -> (bool, bool) {
    let trigger = workflow
        .get("on")
        .or_else(|| workflow.get(yaml_serde::Value::Bool(true)));
    let request = |name: &str| matches!(name, "pull_request" | "merge_group");
    match trigger {
        Some(yaml_serde::Value::String(name)) => (request(name), false),
        Some(yaml_serde::Value::Sequence(names)) => (
            names
                .iter()
                .filter_map(yaml_serde::Value::as_str)
                .any(request),
            false,
        ),
        Some(yaml_serde::Value::Mapping(events)) => {
            let requests: Vec<&yaml_serde::Value> = events
                .iter()
                .filter(|(name, _)| name.as_str().is_some_and(request))
                .map(|(_, config)| config)
                .collect();
            let filtered = requests.iter().any(|config| {
                config.get("paths").is_some() || config.get("paths-ignore").is_some()
            });
            (!requests.is_empty(), filtered)
        }
        _ => (false, false),
    }
}

/// The jobs the gate judges, as `(workflow index, job id)`, or `None`
/// where the gate cannot prove any target and a failure says why.
fn judged_jobs(
    workflows: &[GatedWorkflow],
    required_check: &str,
    gate: &mut DistGate,
) -> Option<Vec<(usize, String)>> {
    if required_check.is_empty() {
        gate.limitations.push((
            GATE_UNNAMED,
            "setup.required_check names no gate, so every job of every pull-request workflow is judged".to_owned(),
        ));
        return Some(
            workflows
                .iter()
                .enumerate()
                .filter(|(_, workflow)| workflow.on_requests)
                .flat_map(|(index, workflow)| {
                    workflow.jobs.iter().map(move |(id, _)| (index, id.clone()))
                })
                .collect(),
        );
    }
    let remediation =
        "name the gate's job in setup.required_check, and run its workflow on pull_request";
    // A check context is a job's name, or its id where it has none.
    let matches: Vec<(usize, &str)> = workflows
        .iter()
        .enumerate()
        .flat_map(|(index, workflow)| {
            workflow.jobs.iter().filter_map(move |(id, job)| {
                let context = job
                    .get("name")
                    .and_then(yaml_serde::Value::as_str)
                    .unwrap_or(id);
                (context == required_check).then_some((index, id.as_str()))
            })
        })
        .collect();
    let [(index, gate_id)] = matches.as_slice() else {
        gate.failures.push(InvariantFailure::new(
            "gate-unresolved",
            "setup.required_check",
            format!(
                "setup.required_check is \"{required_check}\", and {} jobs of the workflows carry that check context, where exactly one must",
                matches.len()
            ),
            remediation,
        ));
        return None;
    };
    let workflow = &workflows[*index];
    if !workflow.on_requests {
        gate.failures.push(InvariantFailure::new(
            "gate-not-on-requests",
            &workflow.path,
            format!(
                "the gate job {gate_id} runs in a workflow that runs on neither pull_request nor merge_group, so its jobs prove nothing before a merge"
            ),
            remediation,
        ));
        return None;
    }
    let mut judged = vec![(*index, (*gate_id).to_owned())];
    let mut next = 0;
    while next < judged.len() {
        let (_, id) = judged[next].clone();
        next += 1;
        for need in job_of(workflow, &id).map(needs_of).unwrap_or_default() {
            if !judged.iter().any(|(_, seen)| *seen == need) {
                judged.push((*index, need));
            }
        }
    }
    Some(judged)
}

fn job_of<'w>(workflow: &'w GatedWorkflow, id: &str) -> Option<&'w yaml_serde::Value> {
    workflow
        .jobs
        .iter()
        .find(|(job_id, _)| job_id == id)
        .map(|(_, job)| job)
}

/// A job's `needs`, a string or a list.
fn needs_of(job: &yaml_serde::Value) -> Vec<String> {
    match job.get("needs") {
        Some(yaml_serde::Value::String(one)) => vec![one.clone()],
        Some(yaml_serde::Value::Sequence(many)) => many
            .iter()
            .filter_map(yaml_serde::Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn steps_of(job: &yaml_serde::Value) -> &[yaml_serde::Value] {
    job.get("steps")
        .and_then(yaml_serde::Value::as_sequence)
        .map_or(&[], Vec::as_slice)
}

/// A string-valued `env` mapping.
fn env_of(env: Option<&yaml_serde::Value>) -> std::collections::BTreeMap<String, String> {
    env.and_then(yaml_serde::Value::as_mapping)
        .map(|env| {
            env.iter()
                .filter_map(|(name, value)| {
                    let value = match value {
                        yaml_serde::Value::String(text) => text.clone(),
                        yaml_serde::Value::Bool(flag) => flag.to_string(),
                        yaml_serde::Value::Number(number) => number.to_string(),
                        _ => return None,
                    };
                    Some((name.as_str()?.to_owned(), value))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// An expression with every space inside `${{ }}` removed, so one token
/// stands for one expression.
fn squeeze(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${{") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find("}}") else {
            out.push_str(&rest[start..]);
            return out;
        };
        out.extend(
            rest[start..start + end + 2]
                .chars()
                .filter(|c| !c.is_whitespace()),
        );
        rest = &rest[start + end + 2..];
    }
    out.push_str(rest);
    out
}

/// The text with each `$NAME` and `${NAME}` of `env` replaced by its value.
fn expand(text: &str, env: &std::collections::BTreeMap<String, String>) -> String {
    let mut out = text.to_owned();
    for (name, value) in env {
        out = out.replace(&format!("${{{name}}}"), value);
        let marker = format!("${name}");
        let mut expanded = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(at) = rest.find(&marker) {
            let after = &rest[at + marker.len()..];
            expanded.push_str(&rest[..at]);
            if after
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                expanded.push_str(&marker);
            } else {
                expanded.push_str(value);
            }
            rest = after;
        }
        expanded.push_str(rest);
        out = expanded;
    }
    out
}

/// One `dist` invocation a step runs: its arguments, and the file its
/// standard output is redirected to, where it is.
struct Invocation {
    /// The arguments after the subcommand, unquoted.
    arguments: Vec<String>,
    /// The target of a `>` redirection.
    stdout: Option<String>,
}

/// How a shell word was quoted: whether any part of it sat inside double
/// quotes, and whether any part sat inside single quotes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Quoting {
    /// A part of the word was double-quoted.
    double: bool,
    /// A part of the word was single-quoted.
    single: bool,
}

impl Quoting {
    const fn any(self) -> bool {
        self.double || self.single
    }
}

/// The words of each shell command segment of one line, each with how it
/// was quoted, read up to the first unquoted comment.
fn segments_of(line: &str) -> Vec<Vec<(String, Quoting)>> {
    let mut segments = vec![Vec::new()];
    let mut word = String::new();
    let mut quoting = Quoting::default();
    let mut in_word = false;
    let mut chars = line.chars().peekable();
    let mut quote: Option<char> = None;
    let finish = |segments: &mut Vec<Vec<(String, Quoting)>>,
                  word: &mut String,
                  quoting: &mut Quoting,
                  in_word: &mut bool| {
        if *in_word && let Some(segment) = segments.last_mut() {
            segment.push((std::mem::take(word), *quoting));
        }
        *quoting = Quoting::default();
        *in_word = false;
    };
    while let Some(c) = chars.next() {
        if let Some(open) = quote {
            if c == open {
                quote = None;
            } else {
                word.push(c);
            }
            continue;
        }
        match c {
            '\u{22}' => {
                quote = Some(c);
                quoting.double = true;
                in_word = true;
            }
            '\u{27}' => {
                quote = Some(c);
                quoting.single = true;
                in_word = true;
            }
            '#' if !in_word => break,
            ';' | '|' | '&' => {
                finish(&mut segments, &mut word, &mut quoting, &mut in_word);
                while chars.peek().is_some_and(|next| matches!(next, '|' | '&')) {
                    chars.next();
                }
                segments.push(Vec::new());
            }
            c if c.is_whitespace() => {
                finish(&mut segments, &mut word, &mut quoting, &mut in_word);
            }
            c => {
                word.push(c);
                in_word = true;
            }
        }
    }
    finish(&mut segments, &mut word, &mut quoting, &mut in_word);
    segments
}

/// Whether the words before `dist` are a wrapper the binding names: none,
/// `nix develop` with any flake reference or flag before `--command`,
/// `mise exec --`, or `devbox run --`, every word unquoted.
fn wrapper_supported(prefix: &[(String, Quoting)], recorded: &[&str]) -> bool {
    if prefix.iter().any(|(_, quoting)| quoting.any()) {
        return false;
    }
    let words: Vec<&str> = prefix.iter().map(|(word, _)| word.as_str()).collect();
    if !recorded.is_empty() && words == recorded {
        return true;
    }
    matches!(
        words.as_slice(),
        [] | ["mise", "exec", "--"] | ["devbox", "run", "--"] | ["nix", "develop", .., "--command"]
    )
}

/// Every `dist <command>` a run executes, in the grammar the binding
/// writes: the subcommand at the start of a command segment, or after a
/// wrapper the binding names or the target's recorded run prefix. Text inside quotes or after a comment is no
/// command. An unquoted `dist <command>` after any other words is
/// unreadable rather than credited.
fn invocations(run: &str, command: &str, recorded: &[&str]) -> Result<Vec<Invocation>, String> {
    let mut found = Vec::new();
    for line in run.lines() {
        for words in segments_of(&squeeze(line)) {
            let Some(at) = words.windows(2).position(|pair| {
                !pair[0].1.any() && !pair[1].1.any() && pair[0].0 == "dist" && pair[1].0 == command
            }) else {
                continue;
            };
            if !wrapper_supported(&words[..at], recorded) {
                let prefix: Vec<&str> = words[..at].iter().map(|(word, _)| word.as_str()).collect();
                return Err(format!(
                    "dist {command} after {}, which is no wrapper the binding names",
                    prefix.join(" ")
                ));
            }
            let mut arguments = Vec::new();
            let mut stdout = None;
            let mut rest = words[at + 2..].iter();
            while let Some((word, _)) = rest.next() {
                if word == ">" {
                    stdout = rest.next().map(|(file, _)| file.clone());
                } else if let Some(file) = word.strip_prefix('>') {
                    stdout = Some(file.to_owned());
                } else if word.starts_with('<') || word.starts_with("2>") {
                    rest.next();
                } else {
                    arguments.push(word.clone());
                }
            }
            found.push(Invocation { arguments, stdout });
        }
    }
    Ok(found)
}

/// Whether a run writes output `name` as `jq -c . <file>`, in the one
/// executable shape the binding writes.
///
/// The shape is `echo`, the export word `<name>=$(jq -c . <file>)`, `>>`,
/// and `$GITHUB_OUTPUT`. The export word must be double-quoted, the
/// quoting inside which the command substitution runs, and never
/// single-quoted, which writes the literal text. The destination is never
/// single-quoted either, which would name a file `$GITHUB_OUTPUT`. A
/// comment is no export.
fn exports_plan(run: &str, name: &str, file: &str) -> bool {
    let wanted = format!("{name}=$(jq -c . {file})");
    run.lines().any(|line| {
        segments_of(line)
            .iter()
            .any(|words| match words.as_slice() {
                [
                    (echo, echo_quoting),
                    (export, export_quoting),
                    (append, append_quoting),
                    (sink, sink_quoting),
                ] => {
                    echo == "echo"
                        && !echo_quoting.any()
                        && export_quoting.double
                        && !export_quoting.single
                        && export.replace("'.'", ".") == wanted
                        && append == ">>"
                        && !append_quoting.any()
                        && !sink_quoting.single
                        && (sink == "$GITHUB_OUTPUT" || sink == "${GITHUB_OUTPUT}")
                }
                _ => false,
            })
    })
}

/// The values of every `--target` argument, in both spellings.
fn target_arguments(arguments: &[String]) -> Vec<String> {
    flag_values(arguments, "--target", None)
}

/// The values of a long flag, as `--flag=value` or `--flag value`, and of
/// its short form `-x value` or `-xvalue` where it has one.
fn flag_values(arguments: &[String], long: &str, short: Option<&str>) -> Vec<String> {
    let mut found = Vec::new();
    let mut iter = arguments.iter();
    while let Some(argument) = iter.next() {
        if let Some(value) = argument.strip_prefix(&format!("{long}=")) {
            found.push(value.to_owned());
        } else if argument == long || short.is_some_and(|short| argument == short) {
            if let Some(value) = iter.next() {
                found.push(value.clone());
            }
        } else if let Some(value) = short.and_then(|short| argument.strip_prefix(short)) {
            found.push(value.to_owned());
        }
    }
    found
}

/// The literal triples one `--target` value names: a triple, or a matrix
/// key whose list is literal in a matrix no `include` or `exclude`
/// reshapes.
fn triples_of(
    value: &str,
    job: &yaml_serde::Value,
) -> Result<std::collections::BTreeSet<String>, String> {
    if let Some(key) = value
        .strip_prefix("${{matrix.")
        .and_then(|rest| rest.strip_suffix("}}"))
    {
        let matrix = job
            .get("strategy")
            .and_then(|strategy| strategy.get("matrix"));
        if matrix.is_some_and(|matrix| {
            matrix.get("include").is_some() || matrix.get("exclude").is_some()
        }) {
            return Err(format!(
                "--target from matrix.{key} in a matrix that include or exclude reshapes"
            ));
        }
        let list = matrix
            .and_then(|matrix| matrix.get(key))
            .and_then(yaml_serde::Value::as_sequence)
            .ok_or_else(|| format!("--target from matrix.{key}, which is no literal list"))?;
        return list
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("--target from matrix.{key}, which holds a non-string"))
            })
            .collect();
    }
    if value.contains('$') {
        return Err(format!("--target {value}"));
    }
    Ok(std::iter::once(value.to_owned()).collect())
}

/// Whether the artifact mode builds the target-specific artifacts: `local`
/// and `all` do, `global` and `lies` do not, and no mode at all is the
/// fuzzy `host` mode, which the judgment cannot read.
fn builds_local_artifacts(arguments: &[String]) -> Result<bool, String> {
    let modes = flag_values(arguments, "--artifacts", Some("-a"));
    match modes.as_slice() {
        [] => Err("no --artifacts, so dist builds in its host mode".to_owned()),
        [mode] if mode == "local" || mode == "all" => Ok(true),
        [mode] if mode == "global" || mode == "lies" => Ok(false),
        _ => Err(format!("--artifacts {}", modes.join(" "))),
    }
}

/// What one `dist build` step of a judged job proves.
fn credit(
    workflow: &GatedWorkflow,
    job: &yaml_serde::Value,
    step: &yaml_serde::Value,
    run: &str,
    peers: &[&str],
    targets: &[String],
    recorded: &[&str],
) -> Credit {
    if job.get("if").is_some() || step.get("if").is_some() {
        return Credit::Conditional;
    }
    if workflow.filtered {
        return Credit::Filtered;
    }
    let mut env = workflow.env.clone();
    env.extend(env_of(job.get("env")));
    env.extend(env_of(step.get("env")));
    let builds = match invocations(&expand(run, &env), "build", recorded) {
        Ok(builds) => builds,
        Err(what) => return Credit::Unreadable(what),
    };
    let mut triples = std::collections::BTreeSet::new();
    for build in builds {
        let arguments = &build.arguments;
        let named = target_arguments(arguments);
        // A fixed target proves its own triple, whatever matrix runs it,
        // and only where it builds the target-specific artifacts.
        if !named.is_empty() {
            match builds_local_artifacts(arguments) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(what) => return Credit::Unreadable(what),
            }
            for value in named {
                match triples_of(&value, job) {
                    Ok(found) => triples.extend(found),
                    Err(what) => return Credit::Unreadable(what),
                }
            }
            continue;
        }
        if arguments
            .iter()
            .any(|argument| argument == "${{matrix.dist_args}}")
        {
            // The plan's own dist_args carry --artifacts=local.
            if !flag_values(arguments, "--artifacts", Some("-a")).is_empty() {
                return Credit::Unreadable(
                    "--artifacts beside matrix.dist_args, which already carries it".to_owned(),
                );
            }
            match planned_triples(workflow, job, peers, recorded) {
                Ok(PlanCredit::Every) => triples.extend(targets.iter().cloned()),
                Ok(PlanCredit::Named(found)) => triples.extend(found),
                Err(what) => return Credit::Unreadable(what),
            }
        }
    }
    Credit::Targets(triples)
}

/// The triples the `artifacts_matrix` of a judged job's `dist plan`
/// covers: every target where the plan is unnarrowed, the named ones
/// where `--target` narrows it. The output the matrix reads must come
/// from that plan, in the pattern the binding writes: the plan into a
/// file, and `jq -c . <file>` into the output.
fn planned_triples(
    workflow: &GatedWorkflow,
    job: &yaml_serde::Value,
    peers: &[&str],
    recorded: &[&str],
) -> Result<PlanCredit, String> {
    let matrix = job
        .get("strategy")
        .and_then(|strategy| strategy.get("matrix"))
        .and_then(yaml_serde::Value::as_str)
        .map(squeeze)
        .ok_or_else(|| "matrix.dist_args from a matrix that is no expression".to_owned())?;
    let inner = matrix
        .strip_prefix("${{fromJson(needs.")
        .or_else(|| matrix.strip_prefix("${{fromJSON(needs."))
        .and_then(|rest| rest.strip_suffix(").ci.github.artifacts_matrix}}"))
        .ok_or_else(|| format!("matrix.dist_args from the matrix {matrix}"))?;
    let (planner, output) = inner
        .split_once(".outputs.")
        .ok_or_else(|| format!("matrix.dist_args from the matrix {matrix}"))?;
    if !peers.contains(&planner) {
        return Err(format!(
            "matrix.dist_args from job {planner}, which the gate does not need"
        ));
    }
    let planner_job =
        job_of(workflow, planner).ok_or_else(|| format!("matrix.dist_args from job {planner}"))?;
    let source = planner_job
        .get("outputs")
        .and_then(|outputs| outputs.get(output))
        .and_then(yaml_serde::Value::as_str)
        .map(squeeze)
        .ok_or_else(|| format!("matrix.dist_args from {planner}.outputs.{output}"))?;
    let (step_id, written) = source
        .strip_prefix("${{steps.")
        .and_then(|rest| rest.strip_suffix("}}"))
        .and_then(|rest| rest.split_once(".outputs."))
        .ok_or_else(|| format!("matrix.dist_args from {planner}.outputs.{output} = {source}"))?;
    let plan = steps_of(planner_job)
        .iter()
        .find(|step| step.get("id").and_then(yaml_serde::Value::as_str) == Some(step_id))
        .and_then(|step| step.get("run"))
        .and_then(yaml_serde::Value::as_str)
        .ok_or_else(|| format!("matrix.dist_args from step {step_id} of {planner}"))?;
    let plans = invocations(plan, "plan", recorded)?;
    let [invocation] = plans.as_slice() else {
        return Err(format!(
            "matrix.dist_args from step {step_id} of {planner}, which runs dist plan {} times",
            plans.len()
        ));
    };
    let file = invocation.stdout.as_deref().ok_or_else(|| {
        format!("matrix.dist_args from step {step_id} of {planner}, whose dist plan writes no file")
    })?;
    // The output carries that file and nothing else.
    if !exports_plan(plan, written, file) {
        return Err(format!(
            "matrix.dist_args from {planner}.outputs.{output}, which carries no jq -c . {file} as {written}"
        ));
    }
    let mut json = false;
    let mut narrowed = std::collections::BTreeSet::new();
    let mut iter = invocation.arguments.iter();
    while let Some(argument) = iter.next() {
        match argument.as_str() {
            "--output-format=json" => json = true,
            "--output-format" if iter.next().is_some_and(|value| value == "json") => json = true,
            "--target" => match iter.next() {
                Some(value) if !value.contains('$') => {
                    narrowed.insert(value.clone());
                }
                _ => return Err("dist plan --target with no literal value".to_owned()),
            },
            other => match other.strip_prefix("--target=") {
                Some(value) if !value.contains('$') => {
                    narrowed.insert(value.to_owned());
                }
                _ => return Err(format!("dist plan {other}")),
            },
        }
    }
    if !json {
        return Err("dist plan with no --output-format=json".to_owned());
    }
    Ok(if narrowed.is_empty() {
        PlanCredit::Every
    } else {
        PlanCredit::Named(narrowed)
    })
}

/// The root manifest configuration GitHub CI builds with. Incomplete
/// inputs are silent: the landing record reports a missing landed
/// configuration, while target-owned manifests are outside that record.
fn dist_profile(target: &Utf8Path) -> Vec<InvariantFailure> {
    let Ok(config) = std::fs::read_to_string(target.join("dist-workspace.toml")) else {
        return Vec::new();
    };
    let Ok(config) = config.parse::<toml::Table>() else {
        return Vec::new();
    };
    let Some(ci) = config
        .get("dist")
        .and_then(toml::Value::as_table)
        .and_then(|dist| dist.get("ci"))
    else {
        return Vec::new();
    };
    let github = ci.as_str() == Some("github")
        || ci
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value.as_str() == Some("github")));
    if !github {
        return Vec::new();
    }

    let Ok(manifest) = std::fs::read_to_string(target.join("Cargo.toml")) else {
        return Vec::new();
    };
    let Ok(manifest) = manifest.parse::<toml::Table>() else {
        return Vec::new();
    };
    if manifest
        .get("profile")
        .and_then(toml::Value::as_table)
        .and_then(|profile| profile.get("dist"))
        .is_some_and(toml::Value::is_table)
    {
        return Vec::new();
    }

    vec![InvariantFailure::new(
        "dist-profile-missing",
        "Cargo.toml",
        "dist-workspace.toml enables GitHub CI, whose generated workflow builds with `--profile dist`, but the root Cargo.toml defines no [profile.dist] table",
        concat!(
            "add this exact block to Cargo.toml:\n",
            "\n",
            "[profile.dist]\n",
            "inherits = \"release\""
        ),
    )]
}

/// The pair's one generated file. Either file absent reports nothing:
/// `rk init` lands the configuration and writes no workflow, the operator
/// generates it afterwards, and a missing `dist-workspace.toml` is already
/// the record's own `missing` line. An absence is the generator's story.
fn generated_release_workflow(target: &Utf8Path) -> Vec<InvariantFailure> {
    let Ok(config) = std::fs::read_to_string(target.join("dist-workspace.toml")) else {
        return Vec::new();
    };
    let workflow = match std::fs::read_to_string(target.join(GENERATED_WORKFLOW)) {
        Ok(text) => text,
        // Absence alone is silent. A file that is there and cannot be
        // read as text is not an absent one, and a run that cannot read
        // what the forge executes has not judged it.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            return vec![InvariantFailure::new(
                "workflow-file-unreadable",
                GENERATED_WORKFLOW,
                format!("the workflow is present and cannot be read as text: {error}"),
                "repair the file so it reads as UTF-8 text, or regenerate it with dist generate --mode ci",
            )];
        }
    };
    workflow_matches_configuration(&config, &workflow)
}

/// The judgment, over the workflow's own text: `dist` is not available to
/// regenerate and diff — the binding states the devshell carries none —
/// and the text is what the forge executes. It reads in one direction,
/// from the workflow to the configuration: a pin the configuration
/// carries and the workflow never runs is the target's own tuning of its
/// installers and its platforms, not drift. Every reference the workflow
/// executes must be immutable whatever the configuration says about it,
/// because a table entry naming a movable tag pins nothing.
fn workflow_matches_configuration(config: &str, workflow: &str) -> Vec<InvariantFailure> {
    let Ok(table) = config.parse::<toml::Table>() else {
        return Vec::new();
    };
    let dist = table.get("dist").and_then(toml::Value::as_table);
    let pinned = dist
        .and_then(|dist| dist.get("github-action-commits"))
        .and_then(toml::Value::as_table);
    let attested = dist
        .and_then(|dist| dist.get("github-attestations"))
        .and_then(toml::Value::as_bool)
        == Some(true);

    let mut failures = Vec::new();
    let steps = workflow_uses(workflow);
    for step in &steps {
        let (action, reference) = match step {
            // A value this reader cannot resolve is never a pass: an
            // alias, an unfamiliar shape, or a form a later generator
            // emits would otherwise erase a real step from the judgment.
            Step::Opaque(value) => {
                failures.push(InvariantFailure::new(
                    "workflow-step-unreadable",
                    GENERATED_WORKFLOW,
                    format!(
                        "the workflow runs `uses: {value}`, which this check cannot resolve into an action and an immutable reference"
                    ),
                    "write the step as <action>@<full commit SHA>, resolving any alias, so what the workflow runs can be read; regenerating with dist generate --mode ci writes that form",
                ));
                continue;
            }
            Step::Action(action, reference) => (action, reference),
        };
        // A non-string table entry pins nothing, and neither does a
        // string that is not itself immutable, so both fall through to
        // the reference check rather than blessing the step.
        let pin = pinned
            .and_then(|table| table.get(action.as_str()))
            .and_then(toml::Value::as_str);
        if let Some(commit) = pin
            && commit != reference
        {
            failures.push(InvariantFailure::new(
                "workflow-action-stale",
                GENERATED_WORKFLOW,
                format!(
                    "the workflow runs {action}@{reference}, where dist-workspace.toml pins {commit}"
                ),
                "regenerate the workflow from the configuration with dist generate --mode ci and commit it; a hand edit is reverted at the next generate",
            ));
            continue;
        }
        if !is_immutable(reference) {
            failures.push(InvariantFailure::new(
                "workflow-action-unpinned",
                GENERATED_WORKFLOW,
                format!(
                    "the workflow runs {action}@{reference}, which is no immutable reference, so the step runs whatever that name points at today"
                ),
                "pin the action at a full commit SHA in [dist.github-action-commits] in dist-workspace.toml, then regenerate with dist generate --mode ci",
            ));
        }
    }
    if attested
        && !steps.iter().any(|step| match step {
            Step::Action(action, _) => {
                action == "actions/attest" || action.starts_with("actions/attest-")
            }
            Step::Opaque(_) => false,
        })
    {
        failures.push(InvariantFailure::new(
            "workflow-attestation-missing",
            GENERATED_WORKFLOW,
            "dist-workspace.toml sets github-attestations = true, and the workflow carries no attest step, so what this workflow builds ships unattested",
            "regenerate the workflow with dist generate --mode ci and commit it, so the configured attest step is what runs",
        ));
    }
    // The forge executes the workflow, not the configuration: a target
    // that set skip and never regenerated still reports the job.
    // This reader asks only whether a request trigger exists at all, and
    // never which branches it names, so the trunk it passes is immaterial.
    if crate::setup::workflow_jobs::request_trigger(workflow, crate::config::TRUNK_DEFAULT)
        .is_some()
    {
        failures.push(InvariantFailure::new(
            "workflow-runs-on-a-request",
            GENERATED_WORKFLOW,
            "the workflow triggers on a pull request, and no gate in another file can need a job declared here, so the one required check does not hold what this workflow reports",
            "set pr-run-mode = \"skip\" in [dist] in dist-workspace.toml and regenerate with dist generate, so the artifact workflow is tag-only; the dist plan and dist generate proofs belong to the workflow the required check gates",
        ));
    }
    failures
}

/// One `uses:` value the workflow carries.
enum Step {
    /// The judgeable form: an action and the reference it runs at.
    Action(String, String),
    /// A value this text reader cannot resolve into the pair — a YAML
    /// alias, which GitHub Actions has accepted since September 2025, or
    /// any shape a later generator emits. Carried rather than dropped,
    /// because a step nobody can read is not a step nobody runs.
    Opaque(String),
}

/// Every distinct `uses:` value the workflow carries.
///
/// A step is read in either YAML style, block or flow. A commented line
/// and a value the reader can prove is same-repository —
/// the workspace-relative `./` form and the `$/` self-repository form,
/// which resolves to the running commit — are no movable external
/// reference. Everything else is carried, including a key whose value
/// sits on another line: a trailing comment and surrounding quotes are
/// stripped, so the readable tag kept beside a commit does not read as
/// part of it. One value is reported once however many jobs run it: the
/// operator fixes the pin, not the steps.
fn workflow_uses(workflow: &str) -> Vec<Step> {
    let mut seen: Vec<String> = Vec::new();
    let mut steps = Vec::new();
    for fragment in workflow.lines().flat_map(line_fragments) {
        let fragment = fragment.trim_start();
        // A step may or may not open its list item on the same fragment.
        let fragment = fragment
            .strip_prefix("- ")
            .map_or(fragment, str::trim_start);
        let Some(rest) = uses_value(fragment) else {
            continue;
        };
        let rest = before_comment(rest).trim();
        let rest = rest
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'))
            .or_else(|| {
                rest.strip_prefix('\'')
                    .and_then(|rest| rest.strip_suffix('\''))
            })
            .unwrap_or(rest);
        if rest.starts_with("./") || rest.starts_with("$/") {
            continue;
        }
        if seen.iter().any(|value| value == rest) {
            continue;
        }
        seen.push(rest.to_owned());
        steps.push(match rest.split_once('@') {
            Some((action, reference)) => Step::Action(action.to_owned(), reference.to_owned()),
            // An empty value is a scalar continued on a later line, which
            // this line reader does not follow, so it is unreadable
            // rather than absent.
            None if rest.is_empty() => Step::Opaque("a value carried on another line".to_owned()),
            None => Step::Opaque(rest.to_owned()),
        });
    }
    steps
}

/// One line's mapping fragments.
///
/// A step is a mapping in either YAML style. A block line is one
/// fragment, keeping every comma its scalar carries, because a git ref
/// may hold one and splitting there would read a movable ref as the
/// immutable prefix of itself. A line whose item opens a flow collection,
/// or that carries a `uses` key beside a brace, is its delimiters apart —
/// except where it also carries a quote, which can hold a delimiter
/// inside a scalar: the reader does not guess there, and hands on a
/// stand-in that reads as a step it cannot resolve. Every other braced
/// line is an expression in some other key's value, never a step.
fn line_fragments(line: &str) -> Vec<&str> {
    let item = line.trim_start();
    let item = item.strip_prefix("- ").map_or(item, str::trim_start);
    let flow = item.starts_with('{')
        || item.starts_with('[')
        || ((line.contains('{') || line.contains('[')) && line.contains("uses"));
    if !flow {
        return vec![line];
    }
    if line.contains(QUOTES) {
        return vec![UNSPLITTABLE_FLOW_LINE];
    }
    line.split(['{', '}', '[', ']', ',']).collect()
}

/// The scalar before its comment. A hash opens a YAML comment only where
/// a space precedes it, and a git ref may carry one, so the readable tag
/// kept beside a commit is stripped while `<sha>#dev` stays whole.
pub(crate) fn before_comment(value: &str) -> &str {
    let mut previous = ' ';
    for (index, character) in value.char_indices() {
        if character == '#' && (previous == ' ' || previous == '\t') {
            return &value[..index];
        }
        previous = character;
    }
    value
}

/// The two quote characters a YAML scalar is written with, named by code
/// point because the artifact-body scan reads a lone quote in these
/// sources as a literal opening.
const QUOTES: [char; 2] = ['\u{22}', '\u{27}'];

/// The stand-in a quoted flow line becomes: it names no action, so the
/// judgment reads it as a step it cannot resolve.
const UNSPLITTABLE_FLOW_LINE: &str = "uses: a flow-style step carrying a quoted value";

/// The value of a `uses` mapping key, however the key is spelled: bare or
/// quoted, as YAML permits for any implicit key, and padded before its
/// colon. A key whose name merely starts with `uses` is not this key.
fn uses_value(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("\"uses\"")
        .or_else(|| line.strip_prefix("'uses'"))
        .or_else(|| line.strip_prefix("uses"))?;
    rest.trim_start().strip_prefix(':')
}

/// An immutable execution reference: a full commit SHA, or the image
/// digest a `docker://` step pins, which no tag move can swap.
fn is_immutable(reference: &str) -> bool {
    let digest = reference
        .strip_prefix("sha256:")
        .filter(|digest| digest.len() == 64);
    let commit = Some(reference).filter(|reference| reference.len() == 40);
    digest
        .or(commit)
        .is_some_and(|value| value.chars().all(|char| char.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::{failures, target_failures, workflow_matches_configuration};

    const CLEAN: &str = r#"
[dist]
pr-run-mode = "skip"
github-attestations = true
github-attestations-phase = "host"
github-release = "host"

[dist.github-action-commits]
"actions/checkout" = "d23441a48e516b6c34aea4fa41551a30e30af803"
"actions/download-artifact" = "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c"
"actions/upload-artifact" = "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
"actions/attest" = "1e69f48acb82d1966a394da916b4c1698aa569d6"
"#;

    /// The correct configuration fails nothing, whatever the whitespace
    /// and key order, and the embedded seed is the exemplar: the
    /// judgment is over the effective TOML, and the seed must satisfy
    /// the rule it seeds.
    #[test]
    fn the_seeded_configuration_is_judged_effectively() {
        assert!(
            failures(
                "rust",
                "github",
                "dist-workspace.toml",
                CLEAN.as_bytes(),
                None
            )
            .is_empty()
        );
        let seed = crate::embedded::SNIPPETS
            .get_file("rust/github/dist-workspace.toml")
            .and_then(|file| file.contents_utf8())
            .expect("the seed is embedded");
        assert!(
            failures(
                "rust",
                "github",
                "dist-workspace.toml",
                seed.as_bytes(),
                None
            )
            .is_empty(),
            "the embedded seed satisfies the invariants it seeds"
        );
    }

    /// A missing or stale action-commit table fails: the signer's own
    /// steps would otherwise run whatever a movable tag names.
    #[test]
    fn a_missing_or_stale_action_commit_table_fails() {
        let missing = "[dist]\ngithub-attestations=true\ngithub-attestations-phase='host'\ngithub-release='host'\n";
        let found = failures(
            "rust",
            "github",
            "dist-workspace.toml",
            missing.as_bytes(),
            None,
        );
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "action-commit-missing"),
            "a missing entry falls back to the movable tag: {found:?}"
        );
        let stale = CLEAN.replace(
            "d23441a48e516b6c34aea4fa41551a30e30af803",
            "0000000000000000000000000000000000000000",
        );
        let found = failures(
            "rust",
            "github",
            "dist-workspace.toml",
            stale.as_bytes(),
            None,
        );
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "action-commit-stale"
                    && failure.reason.contains("actions/checkout")
                    && failure
                        .reason
                        .contains("0000000000000000000000000000000000000000")),
            "a mismatch names the found and expected commits: {found:?}"
        );
        let invalid = CLEAN.replace("\"d23441a48e516b6c34aea4fa41551a30e30af803\"", "123");
        let found_invalid = failures(
            "rust",
            "github",
            "dist-workspace.toml",
            invalid.as_bytes(),
            None,
        );
        assert!(
            found_invalid
                .iter()
                .any(|failure| failure.code == "action-commit-invalid"
                    && failure.reason.contains("actions/checkout")),
            "a non-string value is invalid configuration, not an absent pin: {found_invalid:?}"
        );
        assert!(
            !found
                .iter()
                .any(|failure| failure.reason.contains("actions/attest")),
            "only the stale action is named: {found:?}"
        );
    }

    /// Every degraded form fails with its own code: a commented key, a
    /// false value, the default phase, an unpaired release phase, a
    /// narrowing filter, and malformed TOML.
    #[test]
    fn each_degraded_form_fails_with_its_code() {
        let cases: &[(&str, &str)] = &[
            (
                "[dist]\n# github-attestations = true\ngithub-attestations-phase='host'\ngithub-release='host'\n",
                "attestations-disabled",
            ),
            (
                "[dist]\ngithub-attestations = false\ngithub-attestations-phase='host'\ngithub-release='host'\n",
                "attestations-disabled",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-release='host'\n",
                "attestation-phase-not-host",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-attestations-phase='build-local-artifacts'\ngithub-release='host'\n",
                "attestation-phase-not-host",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-attestations-phase='host'\ngithub-release='announce'\n",
                "release-phase-unpaired",
            ),
            (
                "[dist]\ngithub-attestations = true\ngithub-attestations-phase='host'\ngithub-release='host'\ngithub-attestations-filters=['*.tar.gz']\n",
                "attestation-filters-narrowed",
            ),
            ("not toml at [all", "unparsable-configuration"),
        ];
        for (text, code) in cases {
            let found = failures(
                "rust",
                "github",
                "dist-workspace.toml",
                text.as_bytes(),
                None,
            );
            assert!(
                found.iter().any(|failure| failure.code == *code),
                "{text:?} must fail with {code}, got {found:?}"
            );
        }
    }

    /// Every run mode but `skip` puts a job on a pull request that no
    /// gate can need, and an unset key is the default `plan`, so the
    /// absent case carries its own reason rather than the value's.
    #[test]
    fn a_configuration_that_reports_on_a_request_fails() {
        let absent = CLEAN.replace("pr-run-mode = \"skip\"\n", "");
        let found = failures(
            "rust",
            "github",
            "dist-workspace.toml",
            absent.as_bytes(),
            None,
        );
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "pr-run-mode-not-skip"
                    && failure.reason.contains("unset")
                    && failure.reason.contains("plan")),
            "an unset key defaults to plan and says so: {found:?}"
        );
        for other in ["plan", "upload"] {
            let text = CLEAN.replace("\"skip\"", &format!("\"{other}\""));
            let found = failures(
                "rust",
                "github",
                "dist-workspace.toml",
                text.as_bytes(),
                None,
            );
            assert!(
                found
                    .iter()
                    .any(|failure| failure.code == "pr-run-mode-not-skip"
                        && failure.reason.contains(other)),
                "{other} fails and is named: {found:?}"
            );
        }
        // A non-string value pins nothing either, and falls to the same
        // code rather than passing on a shape the reader cannot use.
        let non_string = CLEAN.replace("\"skip\"", "3");
        assert!(
            failures(
                "rust",
                "github",
                "dist-workspace.toml",
                non_string.as_bytes(),
                None,
            )
            .iter()
            .any(|failure| failure.code == "pr-run-mode-not-skip"),
            "a non-string run mode is not skip"
        );
        assert!(
            !failures(
                "rust",
                "github",
                "dist-workspace.toml",
                CLEAN.as_bytes(),
                None
            )
            .iter()
            .any(|failure| failure.code == "pr-run-mode-not-skip"),
            "skip fails nothing"
        );
    }

    /// The forge executes the workflow, not the configuration: a target
    /// that set `skip` and never regenerated still reports the job, in
    /// whichever form of `on` the generator left behind.
    #[test]
    fn a_generated_workflow_that_triggers_on_a_request_fails() {
        let attest = "      - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        for trigger in [
            "on:\n  pull_request:\n",
            "on: [push, pull_request]\n",
            "on: pull_request_target\n",
        ] {
            let workflow = format!("{trigger}jobs:\n  plan:\n    steps:\n{attest}");
            assert!(
                workflow_matches_configuration(CLEAN, &workflow)
                    .iter()
                    .any(|failure| failure.code == "workflow-runs-on-a-request"
                        && failure.destination == super::GENERATED_WORKFLOW),
                "{trigger:?} reports a check no gate can need"
            );
        }
        let tag_only =
            format!("on:\n  push:\n    tags:\n      - '**'\njobs:\n  plan:\n    steps:\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &tag_only).is_empty(),
            "a tag-only workflow reports nothing on a request"
        );
    }

    /// A workflow generated from the clean configuration fails nothing.
    /// Both indentations cargo-dist emits parse, a commented line is no
    /// step, and a readable tag kept beside a commit is not the commit.
    #[test]
    fn the_generated_workflow_at_the_configured_commits_fails_nothing() {
        let workflow = "\
jobs:
  plan:
    steps:
      - uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803
      # - uses: actions/checkout@v4
      - name: Upload
        uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a
      - name: Attest
        uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v3
";
        let found = workflow_matches_configuration(CLEAN, workflow);
        assert!(
            found.is_empty(),
            "the generated workflow is clean: {found:?}"
        );
    }

    /// A workflow left behind by a configuration change fails, whether
    /// the reference it kept is a movable tag or a superseded commit: the
    /// forge executes the workflow, never the configuration.
    #[test]
    fn a_workflow_left_at_a_movable_tag_fails() {
        for stale in ["v4", "0000000000000000000000000000000000000000"] {
            let workflow = format!(
                "steps:\n  - uses: actions/checkout@{stale}\n  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n"
            );
            let found = workflow_matches_configuration(CLEAN, &workflow);
            assert!(
                found
                    .iter()
                    .any(|failure| failure.code == "workflow-action-stale"
                        && failure.destination == ".github/workflows/release.yml"
                        && failure.reason.contains("actions/checkout")
                        && failure.reason.contains(stale)
                        && failure
                            .reason
                            .contains("d23441a48e516b6c34aea4fa41551a30e30af803")),
                "{stale} names both sides of the disagreement: {found:?}"
            );
        }
        let twice = "steps:\n  - uses: actions/checkout@v4\n  - uses: actions/checkout@v4\n  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        assert_eq!(
            workflow_matches_configuration(CLEAN, twice).len(),
            1,
            "one reference is one failure, however many jobs run it"
        );
    }

    /// A configured attestation the workflow does not carry fails: the
    /// configuration is the only place the operator reads, and the
    /// release ships unattested. The build-provenance variant satisfies
    /// it, so a cargo-dist version that renames the step is no failure.
    #[test]
    fn a_configured_attestation_with_no_attest_step_fails() {
        let bare = "steps:\n  - uses: actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803\n";
        let found = workflow_matches_configuration(CLEAN, bare);
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "workflow-attestation-missing"),
            "an unattested workflow fails: {found:?}"
        );
        assert!(
            !found
                .iter()
                .any(|failure| failure.code.starts_with("workflow-action-")),
            "the pinned step itself is clean: {found:?}"
        );
        let variant = format!(
            "{bare}  - uses: actions/attest-build-provenance@1e69f48acb82d1966a394da916b4c1698aa569d6\n"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &variant).is_empty(),
            "the build-provenance variant is an attest step"
        );
    }

    /// A reference is judged for itself: an action the configuration
    /// pins with a movable tag, or with a non-string value, fails just as
    /// one the configuration never names, because a table entry naming a
    /// tag pins nothing. A pin the workflow never runs fails nothing:
    /// which actions cargo-dist emits follows the target's own installers
    /// and platforms, which a seeded file leaves the target to tune.
    #[test]
    fn a_movable_reference_fails_whatever_the_configuration_says() {
        let attest = "  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        let movable = format!("steps:\n  - uses: third/party@v1\n{attest}");
        let found = workflow_matches_configuration(CLEAN, &movable);
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("third/party")),
            "an action the configuration never names fails: {found:?}"
        );
        // The configuration agreeing with the movable tag is the hole
        // this case exists to hold shut.
        let agreed = CLEAN.replace(
            "[dist.github-action-commits]",
            "[dist.github-action-commits]\n\"third/party\" = \"v1\"",
        );
        let found = workflow_matches_configuration(&agreed, &movable);
        assert!(
            found
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("third/party")),
            "a table entry naming the same movable tag pins nothing: {found:?}"
        );
        let non_string = CLEAN.replace(
            "[dist.github-action-commits]",
            "[dist.github-action-commits]\n\"third/party\" = 1",
        );
        assert!(
            workflow_matches_configuration(&non_string, &movable)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"),
            "a non-string entry pins nothing either"
        );
        let pinned = format!(
            "steps:\n  - uses: third/party@1111111111111111111111111111111111111111\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &pinned).is_empty(),
            "a commit-pinned action the configuration does not name is the target's own"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &format!("steps:\n{attest}")).is_empty(),
            "a pin no step runs is the target's tuning, not drift"
        );
    }

    /// Every real `uses:` shape reaches the judgment: a padded or quoted
    /// key, a flow mapping, a compact flow sequence, a ref carrying a
    /// comma or a hash. A local `./` or `$/` step is the repository's own
    /// file at the running commit, and a `docker://` image pinned by
    /// digest is immutable.
    #[test]
    fn every_real_step_shape_reaches_the_judgment() {
        let attest = "  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        let padded = format!("steps:\n  - uses : actions/checkout@v4\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &padded)
                .iter()
                .any(|failure| failure.code == "workflow-action-stale"),
            "a padded key is the same mapping"
        );
        let quoted = format!("steps:\n  - \"uses\": actions/checkout@v4\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &quoted)
                .iter()
                .any(|failure| failure.code == "workflow-action-stale"),
            "a quoted key is the same mapping"
        );
        let flow =
            format!("steps:\n  - {{ uses: actions/checkout@v4, with: {{ ref: main }} }}\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &flow)
                .iter()
                .any(|failure| failure.code == "workflow-action-stale"),
            "a flow-style step is the same mapping"
        );
        // A git ref may carry a comma, so a block line is never split on
        // one: the immutable-looking prefix is not the reference.
        let comma = format!(
            "steps:\n  - uses: third/party@1111111111111111111111111111111111111111,dev\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &comma)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains(",dev")),
            "the whole reference is judged, never its prefix"
        );
        // A hash opens a comment only after a space, and a git ref may
        // carry one, so the reference is never read as its prefix.
        let hashed = format!(
            "steps:\n  - uses: third/party@1111111111111111111111111111111111111111#dev\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &hashed)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("#dev")),
            "an adjacent hash is scalar content, not a comment"
        );
        // A compact single-pair mapping is a flow-sequence entry.
        let compact = format!("steps: [ uses: third/party@v1 ]\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &compact)
                .iter()
                .any(|failure| failure.code == "workflow-action-unpinned"
                    && failure.reason.contains("third/party")),
            "a compact flow sequence carries its uses key"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &format!("steps:\n  - usesful: no\n{attest}"))
                .is_empty(),
            "a key that merely starts with uses is another key"
        );
        for same_repository in ["./.github/actions/build", "$/.github/actions/build"] {
            let local = format!("steps:\n  - uses: {same_repository}\n{attest}");
            assert!(
                workflow_matches_configuration(CLEAN, &local).is_empty(),
                "{same_repository} is the repository's own file at the running commit"
            );
        }
        let tagged = format!("steps:\n  - uses: docker://alpine:3.8\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &tagged)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"),
            "a docker image with no digest is not immutable"
        );
        let digested = format!(
            "steps:\n  - uses: docker://alpine@sha256:0000000000000000000000000000000000000000000000000000000000000000\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &digested).is_empty(),
            "a docker image pinned by digest is immutable"
        );
    }

    /// A value the reader cannot resolve is reported rather than passed.
    /// GitHub Actions accepts YAML aliases, a scalar may continue on
    /// another line, and a quote may hold a flow delimiter the reader
    /// would otherwise split on: a step nobody can read is not a step
    /// nobody runs. A braced expression in another key's value is no step
    /// at all, and a generated workflow is full of them.
    #[test]
    fn a_step_the_reader_cannot_resolve_is_reported() {
        let attest = "  - uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6\n";
        let aliased = format!("steps:\n  - uses: *checkout\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &aliased)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"
                    && failure.reason.contains("*checkout")),
            "an alias is unreadable, never clean"
        );
        let continued = format!("steps:\n  - uses:\n      actions/checkout@v4\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &continued)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"),
            "a value on another line is unreadable, never clean"
        );
        let quoted_flow = format!("steps:\n  - {{ uses: \"third/party@1,dev\" }}\n{attest}");
        assert!(
            workflow_matches_configuration(CLEAN, &quoted_flow)
                .iter()
                .any(|failure| failure.code == "workflow-step-unreadable"),
            "a quoted flow line is not split on a guess"
        );
        let expression = format!(
            "jobs:\n  host:\n    if: ${{{{ fromJson(needs.plan.outputs.val).ci != null && x == 'true' }}}}\n    steps:\n{attest}"
        );
        assert!(
            workflow_matches_configuration(CLEAN, &expression).is_empty(),
            "an expression is not a step this reader cannot resolve"
        );
    }

    /// The cross-file judgment needs both files and its own pair. Either
    /// one absent reports nothing: release-kit writes neither the
    /// workflow nor a record of it, so an absence is the generator's
    /// story and a fresh landing never fails on its first day.
    #[test]
    fn the_cross_file_judgment_needs_both_files() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let target = Utf8Path::from_path(dir.path()).expect("a utf-8 path");
        let broken = "steps:\n  - uses: actions/checkout@v4\n";
        assert!(
            target_failures("rust", "github", target).is_empty(),
            "an empty target"
        );
        std::fs::write(target.join("dist-workspace.toml"), CLEAN).expect("the configuration");
        assert!(
            target_failures("rust", "github", target).is_empty(),
            "a configuration with no generated workflow"
        );
        std::fs::create_dir_all(target.join(".github/workflows")).expect("the workflow directory");
        std::fs::write(target.join(".github/workflows/release.yml"), broken).expect("the workflow");
        assert!(
            !target_failures("rust", "github", target).is_empty(),
            "both files present, and they disagree"
        );
        for (tech, forge) in [("rust", "gitlab"), ("bash", "github")] {
            assert!(
                target_failures(tech, forge, target).is_empty(),
                "{tech}/{forge} generates no artifact workflow"
            );
        }
        // A workflow that is there and cannot be read as text is not an
        // absent one: only absence is silent.
        std::fs::write(
            target.join(".github/workflows/release.yml"),
            [0x66, 0xff, 0xfe],
        )
        .expect("the workflow");
        assert!(
            target_failures("rust", "github", target)
                .iter()
                .any(|failure| failure.code == "workflow-file-unreadable"),
            "a present workflow that does not read as text is reported"
        );
        std::fs::remove_file(target.join("dist-workspace.toml")).expect("the configuration");
        assert!(
            target_failures("rust", "github", target).is_empty(),
            "a workflow with no configuration to judge it against"
        );
    }

    /// The key is the pair plus the destination: the same bytes under
    /// another pair or another destination fail nothing, so a second pair
    /// sharing a destination cannot silently inherit this rule.
    #[test]
    fn the_rule_is_keyed_by_pair_and_destination() {
        let broken = b"[dist]\ngithub-attestations = false\n";
        assert!(failures("rust", "gitlab", "dist-workspace.toml", broken, None).is_empty());
        assert!(failures("bash", "github", "dist-workspace.toml", broken, None).is_empty());
        assert!(failures("rust", "github", "release-plz.toml", broken, None).is_empty());
    }
}
