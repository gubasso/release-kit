//! The observe-and-verify half of every step's lifecycle.
//!
//! One implementation per forge and step, called by preview never, by apply
//! before and after the mutation, and by `check` as its whole job — so the
//! three modes cannot drift apart, and the mutating half is unreachable from
//! here by construction: nothing spawned from this module mutates anything —
//! read-only forge-CLI calls, the technology's own dry-run check, and the
//! App-credential read [`super::app_jwt`] carries for `install-bot`.

use serde_json::Value;

use crate::cargo_package::{self, Answer, POLICY_DESTINATION, Probe};
use crate::detect::Forge;
use crate::error::RkError;
use crate::setup::app_jwt::{self, AppApi};
use crate::setup::context::Ctx;
use crate::setup::process::{Exec, Outcome};
use crate::setup::workflow_jobs;

/// The executor observes run through: the command layer wraps echoing,
/// journaling, and redaction around the process adapter.
pub type Runner<'a> = dyn FnMut(&Exec) -> Result<Outcome, RkError> + 'a;

// The long-lived branch names `single-trunk` retires when each is an
// ancestor of the trunk come from the target's own configuration, read
// through `Ctx::retired_branches`. The compiled default is the common
// default branch and the retired second branch, so a target that names
// none behaves exactly as it did.

// The landed forge gate's context is the job in `pr-title.yml` that holds
// the squash title to the commit convention under `squash`, or the job in
// `pr-commits.yml` that holds every commit of the request to it under
// `rebase`. The target names the title check in `protection.title_check`,
// read through `Ctx::title_check`, which answers the commit gate's job
// under `rebase`; the landed job keeps its own name as a source constant,
// so a target that renames the key without renaming the job breaks its
// own trunk protection and this observer reports it.

const GITLAB_PRIVATE_REPORTING_LIMITATION: &str = "GitLab has no project-level private reporting switch; the reporter must enable confidentiality; this proves project feature access, not successful submission by every external reporter";

/// What one observation found.
#[derive(Debug)]
pub enum StepState {
    /// The desired state holds; a limitation names what the forge enforces
    /// less strongly than the step's proof claims.
    Satisfied {
        /// What was found, one line.
        detail: String,
        /// The weaker guarantee, by name, where the forge enforces less.
        limitation: Option<String>,
    },
    /// The desired state does not hold.
    Unsatisfied {
        /// What was found instead.
        detail: String,
    },
    /// Eligibility or an optional step's condition does not hold: nothing
    /// is proven, and `check` reports it as skipped.
    Inapplicable {
        /// Why the step does not apply here.
        detail: String,
    },
    /// The observation could not decide.
    Unknown {
        /// Why not.
        detail: String,
    },
    /// The bot private key file is not readable by the current process:
    /// an unmet requirement with a remedy, which proves nothing wrong on
    /// the forge.
    Unreadable {
        /// The requirement, the path, the reason, and the remedy.
        detail: String,
    },
}

impl StepState {
    /// What was found, one line.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::Satisfied { detail, .. }
            | Self::Unsatisfied { detail }
            | Self::Inapplicable { detail }
            | Self::Unknown { detail }
            | Self::Unreadable { detail } => detail,
        }
    }

    /// Whether the desired state holds.
    #[must_use]
    pub const fn satisfied(&self) -> bool {
        matches!(self, Self::Satisfied { .. })
    }

    fn ok(detail: impl Into<String>) -> Self {
        Self::Satisfied {
            detail: detail.into(),
            limitation: None,
        }
    }

    fn ok_with_limitation(detail: impl Into<String>, limitation: impl Into<String>) -> Self {
        Self::Satisfied {
            detail: detail.into(),
            limitation: Some(limitation.into()),
        }
    }

    fn not(detail: impl Into<String>) -> Self {
        Self::Unsatisfied {
            detail: detail.into(),
        }
    }

    fn inapplicable(detail: impl Into<String>) -> Self {
        Self::Inapplicable {
            detail: detail.into(),
        }
    }

    fn unknown(detail: impl Into<String>) -> Self {
        Self::Unknown {
            detail: detail.into(),
        }
    }
}

/// One read-only forge API answer.
pub(crate) enum Api {
    /// The call succeeded and parsed.
    Ok(Value),
    /// The forge answered 404: the thing is not there.
    Missing,
    /// The call failed for another reason, with the CLI's own words.
    Failed(String),
}

/// Observe one step's desired state.
///
/// # Errors
///
/// Propagates executor failures; a forge answer that merely disagrees is a
/// [`StepState`], not an error.
pub fn observe(ctx: &Ctx, step: &str, run: &mut Runner) -> Result<StepState, RkError> {
    if step == "package-check" {
        return package_check(ctx, run);
    }
    if step == "branch-reminder" {
        return Ok(branch_reminder_state(ctx));
    }
    if step == "forge-version" {
        return forge_version(ctx, run);
    }
    match ctx.forge {
        Some(Forge::Github) => github(ctx, step, run),
        Some(Forge::Gitlab) => gitlab(ctx, step, run),
        // A forge step at a target with no adapter proves nothing and
        // reads nothing: the applicability gate states why, and this is
        // the observation saying the same.
        None => Ok(StepState::inapplicable(
            "the profile names no forge this release drives",
        )),
    }
}

/// What Python's check cannot answer. PEP 517 lets a project choose its
/// build backend, and an sdist and a wheel can carry different files, so
/// one `uv build` run supplies no listing contract across both outputs.
const PYTHON_LIMITATION: &str = "sdist and wheel policy inclusion is unproved: PEP 517 leaves the file set to the build backend and the two outputs can differ; inspect both before publishing";

/// What no setup step proves about a bash release's package: `rk release
/// build` packs it from `HEAD` at the release, and only `--list` reads it
/// before then.
const BASH_LIMITATION: &str = "the package is not listed here: its contents, SECURITY.md inclusion among them, are unproved; run rk release build --list before the first release, and name SECURITY.md in profile.release.build.include to ship it";

/// Whether a bash release's verify command can run before the first
/// release: a make command needs a makefile in the target, and a make
/// target that resolves where make is on this host; another command needs
/// its first word to resolve where this host can tell. A command with
/// shell syntax reads ok with its resolution unproved, because only a
/// plain command passes its words to the program unchanged. With no verify
/// command the step has nothing to resolve. The package itself is never
/// listed here, so the limitation names `rk release build --list` every
/// time.
///
/// SATISFIES forge-setup:a-package-check-states-policy-reach
fn bash_build_check(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    const KEY: &str = "profile.release.build.verify";
    let Some(command) = ctx
        .profile
        .release
        .build
        .as_ref()
        .and_then(|build| build.verify.as_deref())
    else {
        return Ok(StepState::ok_with_limitation(
            format!(
                "rk release build packs the declared tracked files of HEAD, and {KEY} names no command that proves the package builds"
            ),
            BASH_LIMITATION.to_owned(),
        ));
    };
    let limitation = BASH_LIMITATION.to_owned();
    // The build runs the command through sh -c. Only a command whose words
    // the shell passes on unchanged can be judged word by word here.
    if !plain_command(command) {
        return Ok(StepState::ok_with_limitation(
            format!(
                "{command} carries shell syntax, so its resolution is not proved here. The build runs it through sh -c in the unpacked package"
            ),
            limitation,
        ));
    }
    let mut words = command.split_whitespace();
    let program = words.next().unwrap_or_default();
    let env = ctx.child_env("package-check");
    // A file this host would run: present, and executable by someone.
    let runnable = |path: &std::path::Path| {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    };
    let on_path = |name: &str| {
        env.iter()
            .find(|(key, _)| key == "PATH")
            .map(|(_, value)| value.clone())
            .or_else(|| std::env::var_os("PATH"))
            .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| runnable(&dir.join(name))))
    };
    if program == "make" {
        let makefile = ["GNUmakefile", "makefile", "Makefile"]
            .iter()
            .any(|name| ctx.target.join(name).is_file());
        if !makefile {
            return Ok(StepState::not(format!(
                "the verify command is {command}, from {KEY}, and the target holds no Makefile, GNUmakefile, or makefile; ship one in the package, or set {KEY} to the command that proves it"
            )));
        }
        if !on_path("make") {
            return Ok(StepState::ok_with_limitation(
                format!(
                    "{command} has its makefile; make is not on this host, so the target was not resolved"
                ),
                limitation,
            ));
        }
        let mut args: Vec<&str> = vec!["-n"];
        args.extend(words);
        let outcome = run(&cargo_exec(ctx, "make", &args))?;
        if !outcome.success() {
            return Ok(StepState::not(format!(
                "make -n does not resolve the verify command {command}: {}; give the makefile that target, or correct {KEY}",
                last_line(&outcome.stderr)
            )));
        }
        return Ok(StepState::ok_with_limitation(
            format!("{command} resolves in the target's makefile"),
            limitation,
        ));
    }
    // A path is the project's own file, which this host can judge: one that
    // is absent or not executable cannot run in any image.
    if program.contains('/') {
        if !runnable(ctx.target.join(program).as_std_path()) {
            return Ok(StepState::not(format!(
                "the verify command is {command}, from {KEY}, and {program} is not an executable file in the target; make it one, or correct {KEY}"
            )));
        }
        return Ok(StepState::ok_with_limitation(
            format!(
                "{command} is the project's own verify command, and {program} is executable here; it runs in the unpacked package"
            ),
            limitation,
        ));
    }
    let resolves = on_path(program);
    Ok(StepState::ok_with_limitation(
        if resolves {
            format!(
                "{command} is the project's own verify command, and {program} resolves here; it runs in the unpacked package"
            )
        } else {
            format!(
                "{command} is the project's own verify command, and {program} does not resolve on this host, where profile.release.build.packages need not be installed; it runs in the unpacked package"
            )
        },
        limitation,
    ))
}

/// Whether `sh -c` runs `command` as one program with its words as they
/// stand: no quoting, escape, operator, redirection, substitution, glob,
/// comment, or leading assignment.
fn plain_command(command: &str) -> bool {
    const SHELL: &[char] = &[
        '\u{27}', '\u{22}', '\u{60}', '\\', '$', ';', '&', '|', '<', '>', '(', ')', '{', '}', '*',
        '?', '[', ']', '~', '#', '!', '\n',
    ];
    !command.contains(SHELL)
        && command
            .split_whitespace()
            .next()
            .is_some_and(|program| !program.contains('='))
}

/// §0: the technology's own no-credential packaging check; the one step that
/// reads its command from the binding rather than from a forge tree.
///
/// Publishability is the whole of the check for every binding. Policy reach
/// is asserted only where the target requests the reporting policy and the
/// binding has a deterministic listing command the step can run with no
/// credentials, which today is a sole Cargo package rooted at the target;
/// every other shape reports its successful packaging result with the
/// unproved inclusion named. A target that opted out of the policy owes no
/// policy in its artifact, so nothing about one is judged or named there.
fn package_check(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    // The stance already reports the step as not applicable; this keeps
    // any caller that observes it anyway from running a packaging command.
    if let Some(reason) = super::steps::no_registry(ctx) {
        return Ok(StepState::ok(reason));
    }
    let (program, args): (&str, &[&str]) = match ctx.tech {
        Some("rust") => ("cargo", &["publish", "--dry-run", "--allow-dirty"]),
        Some("python") => return python_build_check(ctx, run, ctx.reporting_policy()),
        Some("bash") => return bash_build_check(ctx, run),
        Some(other) => {
            return Ok(StepState::unknown(format!(
                "no packaging check is defined for {other}"
            )));
        }
        None => {
            return Ok(StepState::unknown(
                "no version file names a technology; see rk binding --list",
            ));
        }
    };
    let outcome = run(&cargo_exec(ctx, program, args))?;
    if !outcome.success() {
        return Ok(StepState::not(format!(
            "the packaging check failed: {}",
            last_line(&outcome.stderr)
        )));
    }
    let built = "the package builds and passes the registry's dry run";
    policy_in_the_crate(ctx, run, built)
}

/// Whether the uv program resolves to an executable file the way the spawn
/// resolves it from `cwd`, the directory the child runs in: a path with a
/// directory part, or a bare name on the child's PATH, else this
/// process's. A relative path or PATH entry resolves against `cwd`.
fn uv_runs(
    program: &std::ffi::OsStr,
    env: &[(std::ffi::OsString, std::ffi::OsString)],
    cwd: &std::path::Path,
) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    let runnable = |path: &std::path::Path| {
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    };
    let path = std::path::Path::new(program);
    if path.components().count() > 1 {
        return runnable(&cwd.join(path));
    }
    env.iter()
        .find(|(key, _)| key == "PATH")
        .map(|(_, value)| value.clone())
        .or_else(|| std::env::var_os("PATH"))
        .is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| runnable(&cwd.join(dir).join(path)))
        })
}

/// The python check: `uv build`, the build the release runs, then the sdist
/// and the wheel it left in `dist/`, named. Their contents stay unproved
/// where the target requests the reporting policy.
fn python_build_check(ctx: &Ctx, run: &mut Runner, policy: bool) -> Result<StepState, RkError> {
    let program = crate::probes::uv_bin();
    let env = ctx.child_env("package-check");
    if !uv_runs(&program, &env, ctx.target.as_std_path()) {
        // A host with no uv cannot build the package. That is a limit of
        // this observation, not a defect of the target.
        return Ok(StepState::Unknown {
            detail: format!(
                "uv ({}) is not runnable on this host, so the package build was not checked; install uv, or point RK_UV_BIN at it",
                program.to_string_lossy()
            ),
        });
    }
    let exec = Exec {
        program,
        args: vec!["build".into()],
        env,
        cwd: ctx.target.as_std_path().to_path_buf(),
        stdin: None,
    };
    let outcome = run(&exec)?;
    if !outcome.success() {
        return Ok(StepState::not(format!(
            "the packaging check failed: {}",
            last_line(&outcome.stderr)
        )));
    }
    let listed = |suffix: &str| -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(ctx.target.join("dist").as_std_path())
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(suffix))
            .collect();
        names.sort();
        names
    };
    let (sdists, wheels) = (listed(".tar.gz"), listed(".whl"));
    if sdists.is_empty() || wheels.is_empty() {
        return Ok(StepState::not(
            "uv build left no sdist and wheel pair in dist/; the release uploads both",
        ));
    }
    let built = format!(
        "uv build wrote the sdist {} and the wheel {}",
        sdists.join(", "),
        wheels.join(", ")
    );
    Ok(if policy {
        StepState::ok_with_limitation(built, PYTHON_LIMITATION)
    } else {
        StepState::ok(built)
    })
}

/// One no-credential Cargo invocation against the target.
fn cargo_exec(ctx: &Ctx, program: &str, args: &[&str]) -> Exec {
    Exec {
        program: program.into(),
        args: args.iter().map(Into::into).collect(),
        env: ctx.child_env("package-check"),
        cwd: ctx.target.as_std_path().to_path_buf(),
        stdin: None,
    }
}

/// Whether the published crate carries the root policy and none of
/// release-kit's own files, for the one shape Cargo answers unambiguously.
///
/// [`cargo_package::probe`] lists files only for a sole selected default
/// member whose manifest is the target's own `Cargo.toml`; a virtual
/// workspace, several default members, and a sole nested member each keep
/// the successful publishability result and name the limitation instead of
/// claiming a reach they cannot prove.
///
/// release-plz attributes a commit to the crate when the commit changes a
/// file the crate ships, so a packaged file every landing rewrites makes
/// every upgrade a release with no code change. The fault names each such
/// path and the `exclude` entry that removes it, and edits nothing.
///
/// SATISFIES forge-setup:a-package-check-states-policy-reach
fn policy_in_the_crate(ctx: &Ctx, run: &mut Runner, built: &str) -> Result<StepState, RkError> {
    let policy = ctx.reporting_policy();
    let root_manifest = ctx.target.as_std_path().join("Cargo.toml");
    let probe = cargo_package::probe(&root_manifest, &[], |args| {
        run(&cargo_exec(ctx, "cargo", args)).map(|outcome| Answer {
            success: outcome.success(),
            stdout: outcome.stdout,
            stderr: outcome.stderr,
        })
    })?;
    let listing = match probe {
        Probe::Listed(listing) => listing,
        Probe::OtherShape => {
            let unproved = if policy {
                format!(
                    "{POLICY_DESTINATION} inclusion and the absence of release-kit's own files are"
                )
            } else {
                "the absence of release-kit's own files is".to_owned()
            };
            return Ok(StepState::ok_with_limitation(
                built,
                format!(
                    "{unproved} unproved: the package check lists files only for a single default package rooted at the target, and this workspace selects a different shape; inspect the published archive before releasing"
                ),
            ));
        }
        Probe::MetadataFailed(stderr) => {
            return Ok(StepState::unknown(format!(
                "{built}, and the listing check could not run: cargo metadata failed: {}",
                last_line(&stderr)
            )));
        }
        Probe::ListingFailed(stderr) => {
            return Ok(StepState::unknown(format!(
                "{built}, and the listing check could not run: cargo package --list failed: {}",
                last_line(&stderr)
            )));
        }
    };
    let forbidden = cargo_package::release_kit_paths(ctx.landed_destinations());
    let shipped = listing.shipped(&forbidden);
    Ok(boundary_state(
        built,
        policy,
        !policy || listing.carries(POLICY_DESTINATION),
        &shipped,
    ))
}

/// The verdict over one listing: the policy must ship where the target
/// requests it, and no file release-kit lands or owns may. Both faults
/// share one detail, because a step reports one line.
fn boundary_state(built: &str, policy: bool, carried: bool, shipped: &[&str]) -> StepState {
    const WHY: &str = "and release-plz attributes every commit touching one of them to the package, so a landing that rewrites one asks for a release with no code change";
    let policy_fix = format!(
        "add /{POLICY_DESTINATION} to [package].include, remove the [package].exclude entry matching it, or stop ignoring the file"
    );
    let own = || {
        let entries = cargo_package::exclude_entries(shipped)
            .iter()
            .map(|entry| format!("\"{entry}\""))
            .collect::<Vec<_>>()
            .join(", ");
        (
            shipped.join(", "),
            format!("add {entries} to [package].exclude, or drop them from [package].include"),
        )
    };
    match (carried, shipped.is_empty()) {
        (true, true) if policy => StepState::ok(format!(
            "{built}, and the published package carries {POLICY_DESTINATION} and no file release-kit lands or owns"
        )),
        (true, true) => StepState::ok(format!(
            "{built}, and the published package ships no file release-kit lands or owns"
        )),
        (false, true) => StepState::not(format!(
            "{built}, but the published package omits {POLICY_DESTINATION}: {policy_fix}"
        )),
        (true, false) => {
            let (list, fix) = own();
            StepState::not(format!(
                "{built}, but the published package ships files release-kit lands or owns ({list}), {WHY}: {fix}"
            ))
        }
        (false, false) => {
            let (list, fix) = own();
            StepState::not(format!(
                "{built}, but the published package omits {POLICY_DESTINATION} and ships files release-kit lands or owns ({list}), {WHY}: {policy_fix}; and {fix}"
            ))
        }
    }
}

/// §1: the post-merge reminder hook, judged from the target's own files;
/// the one step whose observation asks no forge and spawns no CLI.
fn branch_reminder_state(ctx: &Ctx) -> StepState {
    use crate::setup::branch_reminder::{HookState, observe_hook};
    match observe_hook(&ctx.target) {
        HookState::Installed => {
            StepState::ok("the post-merge hook carries the release-kit reminder")
        }
        HookState::Absent => StepState::not("no post-merge hook is installed"),
        HookState::Foreign => {
            StepState::not("a post-merge hook exists without the release-kit marker")
        }
        HookState::Drifted => StepState::not("the reminder hook drifted from this binary's body"),
        HookState::Unreadable(detail) => StepState::unknown(detail),
    }
}

/// The GitLab version this convention needs, as major and minor.
///
/// `trigger: strategy: mirror` arrived in GitLab 18.2, and the merge-request
/// pipeline's `project-jobs` bridge rests on it: below the floor the child
/// pipeline's status never reaches the parent, so a failing project job
/// merges.
pub const GITLAB_VERSION_FLOOR: (u64, u64) = (18, 2);

/// The two suffixes that name an edition rather than a pre-release. Every
/// other suffix is a pre-release, and the step fails closed on one, except
/// on gitlab.com, where the suffix names the deployment's own continuous
/// build.
const GITLAB_EDITIONS: [&str; 2] = ["ee", "ce"];

/// The refusal an instance below the floor reads: the reading, the reason,
/// and the fix. On gitlab.com the fix never names gitlab.com, the state the
/// target is already in.
fn version_refusal(found: &str, prerelease: Option<&str>, on_gitlab_com: bool) -> String {
    let (major, minor) = GITLAB_VERSION_FLOOR;
    let mut said = vec![format!(
        "this GitLab instance reports {found}; the convention needs {major}.{minor} or newer"
    )];
    if let Some(suffix) = prerelease {
        said.push(format!(
            "the -{suffix} suffix is a pre-release, and nothing proves the feature shipped in it, so this step fails closed"
        ));
    }
    said.push(format!(
        "the merge-request pipeline triggers a child pipeline with `strategy: mirror`, which GitLab added in {major}.{minor}"
    ));
    said.push(
        "below it the child's status never reaches the parent pipeline, so a failing project job merges".to_owned(),
    );
    if on_gitlab_com {
        said.push(format!(
            "gitlab.com runs below {major}.{minor} only until its next deployment; rerun the check after it"
        ));
    } else {
        said.push(format!(
            "upgrade the instance to {major}.{minor} or newer, or host the project on gitlab.com"
        ));
    }
    said.join("; ")
}

/// The major and minor pair of a GitLab version string, such as `19.7.0-pre`,
/// or `None` where the string names no such pair.
fn gitlab_version_pair(found: &str) -> Option<(u64, u64)> {
    let number = found.split_once('-').map_or(found, |(n, _)| n);
    let mut parts = number.split('.');
    parts
        .next()
        .and_then(|major| major.parse::<u64>().ok())
        .zip(parts.next().and_then(|minor| minor.parse::<u64>().ok()))
}

/// §3: the forge's own version against the convention's floor.
///
/// GitHub is a rolling service and is answered without a call. GitLab is one
/// read-only `GET /version`, and every failure to read is `Unknown`, which
/// blocks the `protect-trunk` prerequisite exactly as `Unsatisfied` does.
fn forge_version(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    if ctx.forge == Some(Forge::Github) {
        return Ok(StepState::ok(
            "github.com is a rolling service and declares no version floor",
        ));
    }
    let body = match api_get(ctx, run, "version")? {
        Api::Ok(body) => body,
        Api::Missing => {
            return Ok(StepState::unknown(
                "this instance answers no GET /version; the floor cannot be read. Check that glab is authenticated against it: glab auth login",
            ));
        }
        Api::Failed(err) => {
            return Ok(StepState::unknown(format!(
                "the version could not be read: {err}. Check that glab is authenticated against this instance: glab auth login"
            )));
        }
    };
    let Some(found) = body["version"].as_str() else {
        return Ok(StepState::unknown(
            "the forge answer carries no version field; the floor cannot be read. Check that glab is authenticated against this instance: glab auth login",
        ));
    };
    let suffix = found.split_once('-').map(|(_, s)| s);
    let Some(pair) = gitlab_version_pair(found) else {
        return Ok(StepState::unknown(format!(
            "the forge reports the version as '{found}', which names no major and minor pair; the floor cannot be read"
        )));
    };
    // gitlab.com answers every deployment with a `-pre` suffix, so there
    // the suffix is the SaaS build and the number alone decides. Only a
    // detected gitlab.com loosens the rule: a self-managed or unknown host
    // still fails closed on every suffix that is not an edition.
    let on_gitlab_com = ctx.on_gitlab_com();
    let prerelease = suffix.filter(|s| !GITLAB_EDITIONS.contains(s));
    if let Some(suffix) = prerelease.filter(|_| !on_gitlab_com) {
        return Ok(StepState::not(version_refusal(found, Some(suffix), false)));
    }
    if pair < GITLAB_VERSION_FLOOR {
        return Ok(StepState::not(version_refusal(found, None, on_gitlab_com)));
    }
    let (major, minor) = GITLAB_VERSION_FLOOR;
    Ok(StepState::ok(prerelease.map_or_else(
        || format!("this instance reports {found}, at or above the {major}.{minor} floor"),
        |suffix| {
            format!(
                "gitlab.com reports {found}, at or above the {major}.{minor} floor; the -{suffix} suffix is the SaaS deployment's own"
            )
        },
    )))
}

/// The destructive step's own guard: whether deleting a candidate branch
/// can lose work.
///
/// `Satisfied` means every candidate is already gone or is an ancestor of
/// the trunk; `Unsatisfied` means the deletion must refuse.
///
/// # Errors
///
/// Propagates executor failures.
pub fn single_trunk_guard(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    for candidate in ctx.retired_branches() {
        let candidate = candidate.as_str();
        if candidate == trunk {
            continue;
        }
        let state = match ctx.forge {
            Some(Forge::Github) => github_candidate_guard(ctx, run, candidate)?,
            Some(Forge::Gitlab) => gitlab_candidate_guard(ctx, run, candidate)?,
            // A destructive step fails closed, and an absent adapter is
            // one more thing the guard cannot establish.
            None => StepState::unknown("the profile names no forge this release drives"),
        };
        if !state.satisfied() {
            return Ok(state);
        }
    }
    Ok(StepState::ok(
        "every candidate branch is absent, or an ancestor of the trunk",
    ))
}

/// One candidate branch's ancestry, on GitHub.
fn github_candidate_guard(
    ctx: &Ctx,
    run: &mut Runner,
    candidate: &str,
) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    match api_get(
        ctx,
        run,
        &format!("repos/{}/git/ref/heads/{candidate}", ctx.repo),
    )? {
        Api::Missing => return Ok(StepState::ok(format!("{candidate} is already gone"))),
        Api::Failed(err) => return Ok(StepState::unknown(err)),
        Api::Ok(_) => {}
    }
    match api_get(
        ctx,
        run,
        &format!("repos/{}/compare/{candidate}...{trunk}", ctx.repo),
    )? {
        Api::Ok(body) => {
            let status = body["status"].as_str().unwrap_or("");
            Ok(if matches!(status, "ahead" | "identical") {
                StepState::ok(format!("{candidate} is an ancestor of {trunk}"))
            } else {
                StepState::not(format!(
                    "{candidate} is not an ancestor of {trunk} ({status}); deleting it would lose work"
                ))
            })
        }
        Api::Missing => Ok(StepState::unknown("the comparison is not readable")),
        Api::Failed(err) => Ok(StepState::unknown(err)),
    }
}

/// One candidate branch's ancestry, on GitLab.
fn gitlab_candidate_guard(
    ctx: &Ctx,
    run: &mut Runner,
    candidate: &str,
) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    let project = ctx.repo.replace('/', "%2F");
    match api_get(
        ctx,
        run,
        &format!("projects/{project}/repository/branches/{candidate}"),
    )? {
        Api::Missing => return Ok(StepState::ok(format!("{candidate} is already gone"))),
        Api::Failed(err) => return Ok(StepState::unknown(err)),
        Api::Ok(_) => {}
    }
    match api_get(
        ctx,
        run,
        &format!("projects/{project}/repository/compare?from={trunk}&to={candidate}"),
    )? {
        Api::Ok(body) => {
            let ahead = body["commits"]
                .as_array()
                .is_some_and(|list| !list.is_empty());
            Ok(if ahead {
                StepState::not(format!(
                    "{candidate} carries commits {trunk} does not; deleting it would lose work"
                ))
            } else {
                StepState::ok(format!("{candidate} is an ancestor of {trunk}"))
            })
        }
        Api::Missing => Ok(StepState::unknown("the comparison is not readable")),
        Api::Failed(err) => Ok(StepState::unknown(err)),
    }
}

/// One captured, read-only forge API call.
pub(crate) fn api_get(ctx: &Ctx, run: &mut Runner, path: &str) -> Result<Api, RkError> {
    let exec = Exec {
        program: ctx.cli.clone().into_os_string(),
        args: vec!["api".into(), path.into()],
        env: ctx.child_env("observe"),
        cwd: ctx.target.as_std_path().to_path_buf(),
        stdin: None,
    };
    let outcome = run(&exec)?;
    if outcome.success() {
        return Ok(
            serde_json::from_slice::<Value>(&outcome.stdout).map_or_else(
                |_| Api::Failed("the forge answer did not parse as JSON".into()),
                Api::Ok,
            ),
        );
    }
    let stderr = String::from_utf8_lossy(&outcome.stderr).into_owned();
    if stderr.contains("404") {
        Ok(Api::Missing)
    } else {
        Ok(Api::Failed(last_line(&outcome.stderr)))
    }
}

/// The last non-empty line of a byte stream, for one-line detail fields.
fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .to_owned()
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per setup step, so the match is what makes an unobserved step a compile error"
)]
fn github(ctx: &Ctx, step: &str, run: &mut Runner) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    let repo = &ctx.repo;
    match step {
        "private-vulnerability-reporting" => {
            let visibility_path = format!("repos/{repo}");
            match api_get(ctx, run, &visibility_path)? {
                Api::Ok(body) => match body["private"].as_bool() {
                    Some(true) => {
                        return Ok(StepState::inapplicable(
                            "private vulnerability reporting is available for public repositories",
                        ));
                    }
                    Some(false) => {}
                    None => {
                        return Ok(StepState::unknown(format!(
                            "{visibility_path}: repository visibility is unreadable"
                        )));
                    }
                },
                Api::Missing => {
                    return Ok(StepState::unknown(format!(
                        "{visibility_path}: repository visibility is unreadable (404)"
                    )));
                }
                Api::Failed(err) => {
                    return Ok(StepState::unknown(format!("{visibility_path}: {err}")));
                }
            }
            let path = format!("repos/{repo}/private-vulnerability-reporting");
            Ok(match api_get(ctx, run, &path)? {
                Api::Ok(body) => match body["enabled"].as_bool() {
                    Some(true) => StepState::ok("private vulnerability reporting is enabled"),
                    Some(false) => StepState::not("private vulnerability reporting is disabled"),
                    None => StepState::unknown(format!("{path}: enabled is unreadable")),
                },
                Api::Missing => {
                    StepState::unknown(format!("{path}: reporting state is unreadable (404)"))
                }
                Api::Failed(err) => StepState::unknown(format!("{path}: {err}")),
            })
        }

        "default-branch" => Ok(match api_get(ctx, run, &format!("repos/{repo}"))? {
            Api::Ok(body) => {
                let found = body["default_branch"].as_str().unwrap_or("");
                if found == trunk {
                    StepState::ok(format!("{trunk} is the default branch"))
                } else {
                    StepState::not(format!("the default branch is {found}"))
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {repo}")),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "single-trunk" => {
            for candidate in ctx.retired_branches() {
                let candidate = candidate.as_str();
                if candidate == trunk {
                    continue;
                }
                match api_get(ctx, run, &format!("repos/{repo}/git/ref/heads/{candidate}"))? {
                    Api::Missing => {}
                    Api::Ok(_) => {
                        return Ok(StepState::not(format!("a {candidate} branch still exists")));
                    }
                    Api::Failed(err) => return Ok(StepState::unknown(err)),
                }
            }
            Ok(StepState::ok(
                "no long-lived branch besides the trunk remains",
            ))
        }
        "merge-cleanup" => Ok(match api_get(ctx, run, &format!("repos/{repo}"))? {
            Api::Ok(body) => {
                if body["delete_branch_on_merge"].as_bool().unwrap_or(false) {
                    StepState::ok("a merged branch is deleted by the forge")
                } else {
                    StepState::not("a merged branch outlives its merge")
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {repo}")),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "merge-on-order" => {
            // Every page is read: an armed request past the first page
            // still faults, and a listing that does not exhaust decides
            // nothing.
            let requests = match open_requests(ctx, run, &format!("repos/{repo}/pulls?state=open"))?
            {
                Ok(requests) => requests,
                Err(state) => return Ok(state),
            };
            if let Some(fault) = armed_fault(ctx, &requests, "head", "ref") {
                return Ok(StepState::not(fault));
            }
            // The switch serves the project's own requests too, so it is
            // reported and never judged: no landed job arms anything.
            let switch = match api_get(ctx, run, &format!("repos/{repo}"))? {
                Api::Ok(body) => body["allow_auto_merge"]
                    .as_bool()
                    .map_or("unread", |on| if on { "on" } else { "off" }),
                Api::Missing => {
                    return Ok(StepState::not(format!("the forge does not know {repo}")));
                }
                Api::Failed(_) => "unread",
            };
            Ok(StepState::ok(format!(
                "no release request stands armed to merge itself; the repository's allow_auto_merge switch is {switch}, and no landed job uses it"
            )))
        }
        "ci-permissions" => Ok(
            match api_get(
                ctx,
                run,
                &format!("repos/{repo}/actions/permissions/workflow"),
            )? {
                Api::Ok(body) => {
                    let write = body["default_workflow_permissions"] == "write";
                    let approve = body["can_approve_pull_request_reviews"] == true;
                    if write && approve {
                        StepState::ok("CI may write and open requests")
                    } else {
                        StepState::not(format!(
                            "workflow permissions are {} with request approval {}",
                            body["default_workflow_permissions"],
                            body["can_approve_pull_request_reviews"]
                        ))
                    }
                }
                Api::Missing => StepState::not("no workflow permissions are readable"),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "bot-secrets" => github_bot_secrets(ctx, run),
        "protect-trunk" => github_trunk_ruleset(ctx, run),
        "protect-tags" => github_tag_rulesets(ctx, run),
        "protect-release-lines" => {
            match github_ruleset_body(ctx, run, ctx.lines_ruleset())? {
                RulesetLookup::Absent => {
                    return Ok(StepState::inapplicable(
                        "release/* is unprotected; optional — applied only where older lines exist",
                    ));
                }
                RulesetLookup::Unreadable(err) => return Ok(StepState::unknown(err)),
                RulesetLookup::Found(_) => {}
            }
            github_ruleset(
                ctx,
                run,
                ctx.lines_ruleset(),
                "branch",
                "refs/heads/release/*",
                &["deletion", "non_fast_forward"],
            )
        }
        "protect-release-branches" => github_release_branch_ruleset(ctx, run),
        "protections-check" => {
            // Confirmed drift and unreadable answers stay apart: a proven
            // mismatch is drift even beside an outage, and an outage with
            // nothing proven wrong stays unknown, never drift.
            let mut failures = Vec::new();
            let mut unknowns = Vec::new();
            // Every satisfied step's limitation survives the aggregate.
            let mut limitations: Vec<String> = Vec::new();
            for owned in owned_protections(ctx) {
                match github(ctx, owned, run)? {
                    StepState::Satisfied {
                        limitation: found, ..
                    } => limitations.extend(found),
                    StepState::Inapplicable { .. } => {}
                    StepState::Unsatisfied { detail } => {
                        failures.push(format!("{owned}: {detail}"));
                    }
                    StepState::Unknown { detail } | StepState::Unreadable { detail } => {
                        unknowns.push(format!("{owned}: {detail}"));
                    }
                }
            }
            // The bot secrets, where an automatic release stores them: in
            // the release environments alone. A project that stores them
            // nowhere yet is the gap of bot-secrets, not of a protection.
            if ctx.automatic_release() && !ctx.release_environments().is_empty() {
                match github_environment_support(ctx, run)? {
                    EnvironmentSupport::Unsupported(limitation) => {
                        limitations.push(format!("bot-secrets: {limitation}"));
                    }
                    EnvironmentSupport::Unknown(detail) => {
                        unknowns.push(format!("bot-secrets: {detail}"));
                    }
                    EnvironmentSupport::Supported => match github_bot_secret_faults(ctx, run)? {
                        Err(state) => unknowns.push(format!("bot-secrets: {}", state.detail())),
                        Ok(faults) => {
                            if let Some((remaining, stale)) = &faults.repository {
                                failures.push(format!(
                                    "bot-secrets: {}",
                                    repository_secret_fault(remaining, stale)
                                ));
                            }
                            if !(faults.absent && faults.repository.is_none()) {
                                failures.extend(
                                    faults
                                        .environments
                                        .iter()
                                        .map(|fault| format!("bot-secrets: {fault}")),
                                );
                            }
                        }
                    },
                }
            }
            match api_get(ctx, run, &format!("repos/{repo}/rulesets"))? {
                Api::Ok(body) => {
                    let owned = [
                        ctx.trunk_ruleset().to_owned(),
                        ctx.safety_ruleset().to_owned(),
                        ctx.updates_ruleset().to_owned(),
                        ctx.tag_ruleset().to_owned(),
                        ctx.tag_creation_ruleset().to_owned(),
                        ctx.release_branch_ruleset().to_owned(),
                        ctx.lines_ruleset().to_owned(),
                    ];
                    for ruleset in body.as_array().into_iter().flatten() {
                        let name = ruleset["name"].as_str().unwrap_or("");
                        if !owned.iter().any(|expected| expected == name) {
                            failures.push(format!("a ruleset no step owns: {name}"));
                        }
                    }
                }
                Api::Missing | Api::Failed(_) => {
                    unknowns.push("the ruleset inventory is not readable".to_owned());
                }
            }
            Ok(if !failures.is_empty() {
                StepState::not(failures.join("; "))
            } else if !unknowns.is_empty() {
                StepState::unknown(unknowns.join("; "))
            } else {
                StepState::Satisfied {
                    detail: "exactly the owned protections, with those rules".into(),
                    limitation: if limitations.is_empty() {
                        None
                    } else {
                        Some(limitations.join("; "))
                    },
                }
            })
        }
        _ => Ok(StepState::unknown(format!("no observation for {step}"))),
    }
}

/// The protection steps `protections-check` aggregates for one target: the
/// trunk, the release tags and the release branches wherever the release
/// mode is not `none`, and the optional release lines.
fn owned_protections(ctx: &Ctx) -> Vec<&'static str> {
    let mut owned = vec!["protect-trunk"];
    if ctx.tag_protection() {
        owned.push("protect-tags");
        owned.push("protect-release-branches");
    }
    owned.push("protect-release-lines");
    owned
}

/// The names of the two bot secrets every App-token job reads.
pub const BOT_SECRET_NAMES: [&str; 2] = ["RELEASE_BOT_APP_ID", "RELEASE_BOT_APP_PRIVATE_KEY"];

/// Whether a GitHub repository can hold environment secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvironmentSupport {
    /// A public repository, or a private one on a plan that holds them.
    Supported,
    /// A private repository on a plan that holds no environment secrets,
    /// with the limitation to report.
    Unsupported(String),
    /// The visibility or the plan could not be read.
    Unknown(String),
}

/// The limitation of a private repository on the Free plan.
pub const FREE_PRIVATE_LIMITATION: &str = "on the Free plan a private repository holds no environment secrets, so the bot secrets stay repository secrets that a workflow on any branch reads";

/// Whether this repository can hold environment secrets: every public
/// repository can, and a private one only on a paid plan of its owner.
///
/// # Errors
/// A runner failure.
pub fn github_environment_support(
    ctx: &Ctx,
    run: &mut Runner,
) -> Result<EnvironmentSupport, RkError> {
    let path = format!("repos/{}", ctx.repo);
    let body = match api_get(ctx, run, &path)? {
        Api::Ok(body) => body,
        Api::Missing => return Ok(EnvironmentSupport::Unknown(format!("{path}: not found"))),
        Api::Failed(err) => return Ok(EnvironmentSupport::Unknown(format!("{path}: {err}"))),
    };
    match body["private"].as_bool() {
        Some(false) => return Ok(EnvironmentSupport::Supported),
        Some(true) => {}
        None => {
            return Ok(EnvironmentSupport::Unknown(format!(
                "{path}: repository visibility is unreadable"
            )));
        }
    }
    let owner = body["owner"]["login"].as_str().unwrap_or_default();
    let plan_path = if body["owner"]["type"] == "Organization" {
        format!("orgs/{owner}")
    } else {
        "user".to_owned()
    };
    let plan = match api_get(ctx, run, &plan_path)? {
        Api::Ok(account) if plan_path != "user" || account["login"] == owner => {
            account["plan"]["name"].as_str().map(str::to_owned)
        }
        _ => None,
    };
    Ok(match plan.as_deref() {
        Some("free") => EnvironmentSupport::Unsupported(FREE_PRIVATE_LIMITATION.to_owned()),
        Some(_) => EnvironmentSupport::Supported,
        None => EnvironmentSupport::Unknown(format!(
            "the plan of {owner} is unreadable, so whether this private repository holds environment secrets is unknown"
        )),
    })
}

/// The secret names of one listing answer.
fn secret_names(body: &Value) -> Vec<String> {
    body["secrets"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|secret| secret["name"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// What the GitHub bot secrets lack, split in two.
///
/// The first half is the faults of the release environments. The second
/// is the bot secrets that remain at the repository level, which the apply
/// removes only once every release ref runs a workflow that names the
/// environment.
pub struct BotSecretFaults {
    /// What an environment lacks: its existence, its policy, its secrets.
    pub environments: Vec<String>,
    /// The repository-level bot secrets that remain, with the refs whose
    /// release workflow still names no environment.
    pub repository: Option<(Vec<String>, Vec<String>)>,
    /// Whether no release environment exists yet.
    pub absent: bool,
}

/// The faults of the GitHub bot secrets where environments are supported,
/// or the state that stops the judgment.
///
/// # Errors
/// A runner failure.
pub fn github_bot_secret_faults(
    ctx: &Ctx,
    run: &mut Runner,
) -> Result<Result<BotSecretFaults, StepState>, RkError> {
    let repo = &ctx.repo;
    let branches = ctx.environment_branch_patterns();
    let tags = ctx.environment_tag_patterns();
    let mut wanted: Vec<(String, String)> = branches
        .iter()
        .map(|name| ("branch".to_owned(), name.clone()))
        .chain(tags.iter().map(|name| ("tag".to_owned(), name.clone())))
        .collect();
    wanted.sort();
    let mut environments = Vec::new();
    let mut missing = 0;
    for environment in ctx.release_environments() {
        match github_environment_faults(ctx, run, environment, &wanted)? {
            Err(state) => return Ok(Err(state)),
            Ok(None) => {
                environments.push(format!("the environment {environment} does not exist"));
                missing += 1;
            }
            Ok(Some(faults)) => environments.extend(faults),
        }
    }
    let remaining: Vec<String> = match api_get(ctx, run, &format!("repos/{repo}/actions/secrets"))?
    {
        Api::Ok(body) => secret_names(&body)
            .into_iter()
            .filter(|name| BOT_SECRET_NAMES.contains(&name.as_str()))
            .collect(),
        Api::Missing => Vec::new(),
        Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
    };
    let repository = if remaining.is_empty() {
        None
    } else {
        match github_stale_workflow_refs(ctx, run)? {
            Ok(stale) => Some((remaining, stale)),
            Err(detail) => return Ok(Err(StepState::unknown(detail))),
        }
    };
    Ok(Ok(BotSecretFaults {
        environments,
        repository,
        absent: missing == ctx.release_environments().len(),
    }))
}

/// What one release environment lacks: its policy, its patterns, and its
/// secrets. `None` where the environment does not exist.
fn github_environment_faults(
    ctx: &Ctx,
    run: &mut Runner,
    environment: &str,
    wanted: &[(String, String)],
) -> Result<Result<Option<Vec<String>>, StepState>, RkError> {
    let base = format!("repos/{}/environments/{environment}", ctx.repo);
    let mut faults = Vec::new();
    match api_get(ctx, run, &base)? {
        Api::Ok(body) => {
            let policy = &body["deployment_branch_policy"];
            if policy["custom_branch_policies"] != true || policy["protected_branches"] != false {
                faults.push(format!(
                    "the environment {environment} admits more than the release refs"
                ));
            }
        }
        Api::Missing => return Ok(Ok(None)),
        Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
    }
    match api_get(
        ctx,
        run,
        &format!("{base}/deployment-branch-policies?per_page=100"),
    )? {
        Api::Ok(body) => {
            let mut found: Vec<(String, String)> = body["branch_policies"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .map(|policy| {
                            (
                                policy["type"].as_str().unwrap_or("branch").to_owned(),
                                policy["name"].as_str().unwrap_or_default().to_owned(),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            found.sort();
            if found != wanted {
                let named = |from: &[(String, String)], against: &[(String, String)]| {
                    from.iter()
                        .filter(|policy| !against.contains(policy))
                        .map(|(kind, name)| format!("{kind} {name}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                faults.push(format!(
                    "the policy of {environment} misses [{}] and adds [{}]",
                    named(wanted, &found),
                    named(&found, wanted)
                ));
            }
        }
        Api::Missing => {
            faults.push(format!("the policy of {environment} is not readable"));
        }
        Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
    }
    match api_get(ctx, run, &format!("{base}/secrets"))? {
        Api::Ok(body) => {
            let names = secret_names(&body);
            let absent: Vec<&str> = BOT_SECRET_NAMES
                .iter()
                .copied()
                .filter(|name| !names.iter().any(|found| found == name))
                .collect();
            if !absent.is_empty() {
                faults.push(format!(
                    "the environment {environment} lacks {}",
                    absent.join(" and ")
                ));
            }
        }
        Api::Missing => {
            faults.push(format!("the secrets of {environment} are not readable"));
        }
        Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
    }
    Ok(Ok(Some(faults)))
}

/// The detail of bot secrets that remain at the repository level.
#[must_use]
pub fn repository_secret_fault(remaining: &[String], stale: &[String]) -> String {
    if stale.is_empty() {
        format!(
            "{} remain repository secrets, which a workflow on any branch reads; rk setup --apply removes them",
            remaining.join(" and ")
        )
    } else {
        format!(
            "{} remain repository secrets until the release workflow names the environment on {}; push the upgraded workflow there, then rk setup --apply",
            remaining.join(" and "),
            stale.join(", ")
        )
    }
}

/// The release refs that still run a workflow from before the environment.
///
/// Each is the trunk, or a release line that the clone knows at its
/// remote, whose landed release workflow, read at the forge, still has a
/// job that mints the App token and names none of the release
/// environments. A ref with no release workflow runs no App-token job.
///
/// # Errors
/// A runner failure.
pub fn github_stale_workflow_refs(
    ctx: &Ctx,
    run: &mut Runner,
) -> Result<Result<Vec<String>, String>, RkError> {
    let Some(destination) = crate::projection::release_workflow_destination(ctx.driver()) else {
        return Ok(Ok(Vec::new()));
    };
    let mut refs = vec![ctx.trunk().to_owned()];
    if let Ok(rows) = crate::commands::lines::rows(&ctx.target) {
        refs.extend(
            rows.into_iter()
                .filter(|row| row.presence != "local")
                .map(|row| row.branch),
        );
    }
    let mut stale = Vec::new();
    for branch in refs {
        let path = format!("repos/{}/contents/{destination}?ref={branch}", ctx.repo);
        let exec = Exec {
            program: ctx.cli.clone().into_os_string(),
            args: vec![
                "api".into(),
                "-H".into(),
                "Accept: application/vnd.github.raw+json".into(),
                path.clone().into(),
            ],
            env: ctx.child_env("observe"),
            cwd: ctx.target.as_std_path().to_path_buf(),
            stdin: None,
        };
        let outcome = run(&exec)?;
        if !outcome.success() {
            if String::from_utf8_lossy(&outcome.stderr).contains("404") {
                continue;
            }
            return Ok(Err(format!("{path}: {}", last_line(&outcome.stderr))));
        }
        let text = String::from_utf8_lossy(&outcome.stdout);
        let named = crate::projection::app_token_jobs(&text)
            .iter()
            .all(|(_, environment)| {
                environment
                    .as_ref()
                    .is_some_and(|name| ctx.release_environments().contains(name))
            });
        if !named {
            stale.push(branch);
        }
    }
    Ok(Ok(stale))
}

/// The GitHub bot secrets: in the release environments behind their
/// policy, and gone from the repository level, or at the repository level
/// alone where the plan holds no environment secrets.
fn github_bot_secrets(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let repository_names = |run: &mut Runner| -> Result<Result<Vec<String>, String>, RkError> {
        Ok(
            match api_get(ctx, run, &format!("repos/{}/actions/secrets", ctx.repo))? {
                Api::Ok(body) => Ok(secret_names(&body)),
                Api::Missing => Ok(Vec::new()),
                Api::Failed(err) => Err(err),
            },
        )
    };
    let support = if ctx.release_environments().is_empty() {
        EnvironmentSupport::Unsupported(String::new())
    } else {
        github_environment_support(ctx, run)?
    };
    match support {
        EnvironmentSupport::Unknown(detail) => Ok(StepState::unknown(detail)),
        EnvironmentSupport::Unsupported(limitation) => {
            let names = match repository_names(run)? {
                Ok(names) => names,
                Err(err) => return Ok(StepState::unknown(err)),
            };
            Ok(
                if BOT_SECRET_NAMES
                    .iter()
                    .all(|name| names.iter().any(|found| found == name))
                {
                    StepState::Satisfied {
                        detail: "both bot secrets are stored in the repository".into(),
                        limitation: (!limitation.is_empty()).then_some(limitation),
                    }
                } else if names.is_empty() {
                    StepState::not("no bot secrets are stored")
                } else {
                    StepState::not(format!("stored secrets: {}", names.join(", ")))
                },
            )
        }
        EnvironmentSupport::Supported => Ok(match github_bot_secret_faults(ctx, run)? {
            Err(state) => state,
            Ok(faults) => {
                let mut all = faults.environments;
                if let Some((remaining, stale)) = &faults.repository {
                    all.push(repository_secret_fault(remaining, stale));
                }
                if all.is_empty() {
                    StepState::ok(format!(
                        "both bot secrets live in {}, whose policy admits the release refs alone",
                        ctx.release_environments().join(" and ")
                    ))
                } else {
                    StepState::not(all.join("; "))
                }
            }
        }),
    }
}

/// The two release tag rulesets on GitHub. The immutable one covers
/// `protection.tag_pattern` alone, carries exactly `deletion` and `update`,
/// and names no bypass actor, so no actor moves or deletes a published tag.
/// The creation one lets only the release bypass actors create a tag that
/// starts a release.
fn github_tag_rulesets(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let name = ctx.tag_ruleset();
    let pattern = ctx.protection().tag_pattern.clone();
    let mut faults = Vec::new();
    match github_ruleset(ctx, run, name, "tag", &pattern, &["deletion", "update"])? {
        StepState::Satisfied { .. } => {}
        StepState::Unsatisfied { detail } => faults.push(detail),
        other => return Ok(other),
    }
    if let RulesetLookup::Found(detail) = github_ruleset_body(ctx, run, name)?
        && !detail["bypass_actors"].as_array().is_none_or(Vec::is_empty)
    {
        faults.push(format!(
            "{name} names a bypass actor, so a published tag can move or vanish"
        ));
    }
    let creation = ctx.tag_creation_ruleset();
    let patterns = ctx.tag_creation_patterns();
    match release_ruleset_faults(ctx, run, creation, "tag", &patterns, &["creation"])? {
        Ok(proven) => faults.extend(proven),
        Err(err) => {
            if faults.is_empty() {
                return Ok(StepState::unknown(err));
            }
        }
    }
    Ok(if faults.is_empty() {
        StepState::ok(format!(
            "{name} keeps {pattern} immutable, and {creation} lets only the release actors create {}",
            patterns.join(", ")
        ))
    } else {
        StepState::not(faults.join("; "))
    })
}

/// The installation, observed as the App itself.
///
/// The forge serves `repos/{owner}/{repo}/installation` to an App JWT and
/// to nothing a user can hold. The caller mints `jwt` — once per run, with
/// the token and the key bytes already registered as redaction needles —
/// which is why this lives outside the name dispatch above: an observation
/// entered without that token has no honest answer.
#[must_use]
pub fn github_install_bot(ctx: &Ctx, jwt: &str) -> StepState {
    match app_jwt::api_get(ctx, jwt, &format!("repos/{}/installation", ctx.repo)) {
        AppApi::Ok(body) => {
            let id = body["id"].as_i64().unwrap_or_default();
            let held = &body["permissions"];
            let short: Vec<String> = minimum_grant(ctx)
                .into_iter()
                .filter(|(key, level)| held[key] != *level)
                .map(|(key, level)| format!("{key}: {level}"))
                .collect();
            if short.is_empty() {
                StepState::ok(format!("installation {id} covers {}", ctx.repo))
            } else {
                // An installation predating a widened grant reads
                // unsatisfied until its owner approves the new permission
                // in the App's installation settings; no token this run
                // can mint grants it.
                StepState::not(format!(
                    "installation {id} covers {} and does not hold [{}]; approve the App's updated permissions on the installation's own settings page",
                    ctx.repo,
                    short.join(", ")
                ))
            }
        }
        AppApi::Missing => StepState::not(format!("the App is not installed on {}", ctx.repo)),
        AppApi::Refused(detail) | AppApi::Failed(detail) => StepState::unknown(detail),
    }
}

/// The release App's minimum grant for this target, as the installation
/// reports it.
///
/// Contents and pull requests carry the release itself: the tag, the bump
/// branch, and the request. The merge job that `rk release merge`
/// dispatches reads the required check run beside them, and every
/// automatic release lands that job, so an automatic release needs the
/// third permission and no other target is asked for it.
fn minimum_grant(ctx: &Ctx) -> Vec<(&'static str, &'static str)> {
    let mut grant = vec![("contents", "write"), ("pull_requests", "write")];
    if ctx.profile.release.mode == crate::profile::ReleaseMode::Automatic {
        grant.push(("checks", "read"));
    }
    grant
}

/// A plain ruleset: active, and carrying exactly the expected rule types —
/// not one fewer, and not one more, because an extra rule here is a rule the
/// setup cannot reproduce or explain and can block the very push the method
/// depends on.
fn github_ruleset(
    ctx: &Ctx,
    run: &mut Runner,
    name: &str,
    target: &str,
    include: &str,
    rules: &[&str],
) -> Result<StepState, RkError> {
    let detail = match github_ruleset_body(ctx, run, name)? {
        RulesetLookup::Found(detail) => detail,
        RulesetLookup::Absent => {
            return Ok(StepState::not(format!("no ruleset named {name}")));
        }
        RulesetLookup::Unreadable(err) => return Ok(StepState::unknown(err)),
    };
    if detail["enforcement"] != "active" {
        return Ok(StepState::not(format!("{name} is not active")));
    }
    // The name proves nothing: the ruleset must cover exactly the declared
    // refs, or the protection it reports exists somewhere else.
    if detail["target"] != target {
        return Ok(StepState::not(format!(
            "{name} does not target {target} refs"
        )));
    }
    if detail["conditions"]["ref_name"]["include"] != serde_json::json!([include]) {
        return Ok(StepState::not(format!(
            "{name} does not cover {include} alone"
        )));
    }
    if detail["conditions"]["ref_name"]["exclude"] != serde_json::json!([]) {
        return Ok(StepState::not(format!(
            "{name} excludes refs from its own coverage"
        )));
    }
    let mut held: Vec<&str> = detail["rules"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|rule| rule["type"].as_str())
                .collect()
        })
        .unwrap_or_default();
    held.sort_unstable();
    let mut expected: Vec<&str> = rules.to_vec();
    expected.sort_unstable();
    if held == expected {
        Ok(StepState::ok(format!(
            "{name} is active with exactly its rules"
        )))
    } else {
        Ok(StepState::not(format!(
            "{name} carries the rules [{}] where the setup owns [{}]",
            held.join(", "),
            expected.join(", ")
        )))
    }
}

// The trunk ruleset is checked for the shape a release merge needs.
// The rule kinds the setup writes and can reproduce come from
// `protection.owned_trunk_rules`, floored to contain all four. The set
// also drives the missing-rule fault, so a kind this convention refuses
// must stay out of it: adding one would demand that rule on every target.
// The floor is what stops a target dropping one it needs.

/// A fault line for every rule on the trunk that the setup does not own.
///
/// The merge queue gets its own text, because this convention refuses one
/// deliberately and the operator needs the consequence and the remedy. Every
/// other unowned kind reads generically: an unowned rule is one the setup
/// cannot reproduce or explain, and it can block the very merge the method
/// depends on.
fn unowned_rule_faults(rules: &[Value], owned: &[String]) -> Vec<String> {
    rules
        .iter()
        .filter_map(|rule| rule["type"].as_str())
        .filter(|kind| !owned.iter().any(|name| name == kind))
        .map(|kind| {
            if kind == "merge_queue" {
                MERGE_QUEUE_FAULT.to_owned()
            } else {
                format!("an unowned rule is present: {kind}")
            }
        })
        .collect()
}

fn github_trunk_ruleset(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    let name = ctx.trunk_ruleset().to_owned();
    let detail = match github_ruleset_body(ctx, run, &name)? {
        RulesetLookup::Found(detail) => detail,
        RulesetLookup::Absent => {
            return Ok(StepState::not(format!("no ruleset named {name}")));
        }
        RulesetLookup::Unreadable(err) => return Ok(StepState::unknown(err)),
    };
    let rules = detail["rules"].as_array().cloned().unwrap_or_default();
    let mut faults = Vec::new();
    if detail["enforcement"] != "active" {
        faults.push(format!("{name} is not active"));
    }
    // The name proves nothing: a ruleset applies only where its conditions
    // say, so a right-named ruleset covering another ref would otherwise
    // read as a protected trunk.
    if detail["target"] != "branch" {
        faults.push(format!("{name} does not target branches"));
    }
    let expected_ref = serde_json::json!([format!("refs/heads/{trunk}")]);
    if detail["conditions"]["ref_name"]["include"] != expected_ref {
        faults.push(format!("{name} does not cover refs/heads/{trunk} alone"));
    }
    // A matching exclusion negates the include, so the owned shape is an
    // exclusion list that is exactly empty.
    if detail["conditions"]["ref_name"]["exclude"] != serde_json::json!([]) {
        faults.push(format!("{name} excludes refs from its own coverage"));
    }
    let expected_bypass = super::context::github_bypass_actors(&ctx.protection().bypass_actors);
    if detail["bypass_actors"] != expected_bypass {
        faults.push("the bypass actors do not match the recorded authority".to_owned());
    }
    faults.extend(trunk_rule_faults(ctx, &rules, &name));
    faults.extend(merge_method_fault(ctx, &rules));
    if let Some(checks) = rules
        .iter()
        .find(|rule| rule["type"] == "required_status_checks")
    {
        if checks["parameters"]["strict_required_status_checks_policy"]
            != ctx.protection().strict_required_status_checks
        {
            faults.push(STALE_MERGE_FAULT.to_owned());
        }
        let contexts: Vec<&str> = checks["parameters"]["required_status_checks"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|check| check["context"].as_str())
                    .collect()
            })
            .unwrap_or_default();
        // Where the expected check is known, the context set must be exactly
        // it plus the title check: an extra stale context does not fail a
        // merge, it hangs one, and a missing title check lets an
        // unconventional squash title land on the trunk.
        if contexts.is_empty() {
            faults.push("no status check is required".to_owned());
        } else if let Some(expected) = &ctx.required_check {
            let mut held = contexts.clone();
            held.sort_unstable();
            let title_check = ctx.title_check();
            let mut owned_contexts = [expected.as_str(), title_check];
            owned_contexts.sort_unstable();
            if held != owned_contexts {
                faults.push(format!(
                    "the required checks are [{}] where the setup owns [{}]",
                    contexts.join(", "),
                    owned_contexts.join(", ")
                ));
            }
        } else if !contexts.contains(&ctx.title_check()) {
            faults.push(format!("the {} check is not required", ctx.title_check()));
        }
    }
    match repository_merge_settings(ctx, run)? {
        MergeSources::Owned => {}
        MergeSources::Faults(proven) => faults.extend(proven),
        // Proven drift wins over an outage: an unreadable settings read
        // downgrades the answer to unknown only when nothing above it was
        // proven wrong.
        MergeSources::Unreadable(err) => {
            if faults.is_empty() {
                return Ok(StepState::unknown(err));
            }
        }
    }
    match github_safety_ruleset(ctx, run)? {
        SafetyRuleset::Owned => {}
        SafetyRuleset::Faults(proven) => faults.extend(proven),
        SafetyRuleset::Unreadable(err) => {
            if faults.is_empty() {
                return Ok(StepState::unknown(err));
            }
        }
    }
    match github_updates_ruleset(ctx, run)? {
        Ok(proven) => faults.extend(proven),
        Err(err) if faults.is_empty() => return Ok(StepState::unknown(err)),
        Err(_) => {}
    }
    if let Some(shape) = gate_faults(ctx) {
        faults.push(shape);
    }
    if !faults.is_empty() {
        return Ok(StepState::not(faults.join("; ")));
    }
    Ok(StepState::ok(format!(
        "{name} holds the release-merge shape beside {} and {}",
        ctx.safety_ruleset(),
        ctx.updates_ruleset()
    )))
}

/// The faults of the trunk updates ruleset: the `update` rule alone over
/// the trunk alone, passed by exactly the release bypass actors.
fn github_updates_ruleset(
    ctx: &Ctx,
    run: &mut Runner,
) -> Result<Result<Vec<String>, String>, RkError> {
    let trunk = [format!("refs/heads/{}", ctx.trunk())];
    release_ruleset_faults(
        ctx,
        run,
        ctx.updates_ruleset(),
        "branch",
        &trunk,
        &["update"],
    )
}

/// The fault of a request rule allowing any merge method but the one the
/// recorded merge form names.
fn merge_method_fault(ctx: &Ctx, rules: &[Value]) -> Option<String> {
    let request = rules.iter().find(|rule| rule["type"] == "pull_request")?;
    (request["parameters"]["allowed_merge_methods"]
        != serde_json::json!(ctx.protection().allowed_merge_methods))
    .then(|| {
        format!(
            "the merge method is not exactly a {} merge",
            ctx.merge_form().as_str()
        )
    })
}

/// Which rules the trunk ruleset must carry, and which it must not.
///
/// Each ruleset carries the half of the owned rules its bypass fits, so the
/// trunk ruleset holds what a recorded actor may be excused from. A safety
/// rule here is the shape from before the split: it reads as protection
/// while inheriting this ruleset's bypass, so it gets its own words rather
/// than the generic unowned-rule fault.
fn trunk_rule_faults(ctx: &Ctx, rules: &[Value], name: &str) -> Vec<String> {
    let has = |kind: &str| rules.iter().any(|rule| rule["type"] == kind);
    let mut faults = Vec::new();
    let mut accounted: Vec<String> = ctx
        .protection()
        .owned_trunk_rules
        .iter()
        .filter(|rule| crate::config::REQUEST_RULES.contains(&rule.as_str()))
        .cloned()
        .collect();
    for required in &accounted {
        if !has(required) {
            faults.push(format!("the {required} rule is missing"));
        }
    }
    for stray in crate::config::SAFETY_RULES {
        if has(stray) {
            faults.push(format!(
                "the {stray} rule sits in {name}, where a bypass actor excuses it"
            ));
        }
        accounted.push(stray.to_owned());
    }
    faults.extend(unowned_rule_faults(rules, &accounted));
    faults
}

/// What the safety ruleset's own read answered.
enum SafetyRuleset {
    Owned,
    Faults(Vec<String>),
    Unreadable(String),
}

/// The ruleset no actor is excused from.
///
/// A bypass actor recorded here would hand whoever it names the deletion
/// and the force-push along with the trunk push, which is the one thing the
/// split exists to prevent. The rules it carries are the safety half of
/// `protection.owned_trunk_rules`, so one key still answers what the setup
/// owns on the trunk.
fn github_safety_ruleset(ctx: &Ctx, run: &mut Runner) -> Result<SafetyRuleset, RkError> {
    let trunk = ctx.trunk();
    let name = ctx.safety_ruleset().to_owned();
    let detail = match github_ruleset_body(ctx, run, &name)? {
        RulesetLookup::Found(detail) => detail,
        RulesetLookup::Absent => {
            return Ok(SafetyRuleset::Faults(vec![format!(
                "no ruleset named {name} holds the trunk against deletion and force-push"
            )]));
        }
        RulesetLookup::Unreadable(err) => return Ok(SafetyRuleset::Unreadable(err)),
    };
    let mut faults = Vec::new();
    if detail["enforcement"] != "active" {
        faults.push(format!("{name} is not active"));
    }
    if detail["target"] != "branch" {
        faults.push(format!("{name} does not target branches"));
    }
    if detail["conditions"]["ref_name"]["include"]
        != serde_json::json!([format!("refs/heads/{trunk}")])
    {
        faults.push(format!("{name} does not cover refs/heads/{trunk} alone"));
    }
    if detail["conditions"]["ref_name"]["exclude"] != serde_json::json!([]) {
        faults.push(format!("{name} excludes refs from its own coverage"));
    }
    if !detail["bypass_actors"].as_array().is_none_or(Vec::is_empty) {
        faults.push(format!(
            "{name} names a bypass actor, so deletion and force-push hold against nobody"
        ));
    }
    let rules = detail["rules"].as_array().cloned().unwrap_or_default();
    let safety_rules: Vec<String> = ctx
        .protection()
        .owned_trunk_rules
        .iter()
        .filter(|rule| crate::config::SAFETY_RULES.contains(&rule.as_str()))
        .cloned()
        .collect();
    for required in &safety_rules {
        if !rules.iter().any(|rule| rule["type"] == required.as_str()) {
            faults.push(format!("the {required} rule is missing from {name}"));
        }
    }
    faults.extend(unowned_rule_faults(&rules, &safety_rules));
    if faults.is_empty() {
        Ok(SafetyRuleset::Owned)
    } else {
        Ok(SafetyRuleset::Faults(faults))
    }
}

/// The ways the named gate is shaped so that it cannot report a blocking
/// answer. A required check that never reports is a broken trunk
/// protection, not a weaker guarantee, so each of these is a fault rather
/// than a limitation. Read only where the check is named: without the flag
/// the observation knows no gate.
///
/// It judges the gate alone. Which other jobs a project means to block a
/// merge is intent, no file states it, and `forges/github.md` carries that
/// as a convention instead.
fn gate_faults(ctx: &Ctx) -> Option<String> {
    if !ctx.forge_gate() {
        return None;
    }
    let check = ctx.required_check.as_deref()?;
    workflow_jobs::faults(
        &workflow_jobs::read_gate(&ctx.target, check, ctx.trunk()),
        check,
        ctx.trunk(),
    )
}

/// What the repository's merge settings hold.
enum MergeSources {
    /// The settings the recorded merge form needs, as the setup owns.
    Owned,
    /// Proven other values, one fault line each.
    Faults(Vec<String>),
    /// The settings could not be read.
    Unreadable(String),
}

/// The repository's own merge settings, beside the ruleset, as the
/// recorded merge form needs them. One GET answers for each form.
///
/// Under `squash` they are the squash message sources: with the title
/// source unset, a one-commit request offers that commit's own subject as
/// the trunk's message, which the bot then reads for the version; with the
/// message source on another value, the trunk's body is not the request's
/// description the content gates judged. Each is faulted by name.
///
/// Under `rebase` it is the repository switch `allow_rebase_merge`. The
/// ruleset allows the rebase merge alone, and a repository that disallows
/// it offers no merge method at all, so a request can never merge. A
/// switch that is missing or not a boolean is a fault rather than a pass,
/// and no squash source is read, because no rebase merge reads one.
fn repository_merge_settings(ctx: &Ctx, run: &mut Runner) -> Result<MergeSources, RkError> {
    let squash = ctx.merge_form() == crate::landing::MergeForm::Squash;
    Ok(match api_get(ctx, run, &format!("repos/{}", ctx.repo))? {
        Api::Ok(body) => {
            let mut faults = Vec::new();
            if squash {
                let owned_title = ctx.protection().github.squash_title_source.as_str();
                let owned_body = ctx.protection().github.squash_body_source.as_str();
                if body["squash_merge_commit_title"] != owned_title {
                    faults.push(format!(
                        "the squash title source is {} where the setup owns {owned_title}",
                        body["squash_merge_commit_title"]
                    ));
                }
                if body["squash_merge_commit_message"] != owned_body {
                    faults.push(format!(
                        "the squash message source is {} where the setup owns {owned_body}",
                        body["squash_merge_commit_message"]
                    ));
                }
            } else if let Some(fault) = rebase_switch_fault(body["allow_rebase_merge"].as_bool()) {
                faults.push(fault);
            }
            if faults.is_empty() {
                MergeSources::Owned
            } else {
                MergeSources::Faults(faults)
            }
        }
        Api::Missing => MergeSources::Faults(vec![format!("the forge does not know {}", ctx.repo)]),
        Api::Failed(err) => MergeSources::Unreadable(err),
    })
}

/// The fault of the repository's `allow_rebase_merge` switch under the
/// `rebase` merge form, or `None` where it is on.
fn rebase_switch_fault(allowed: Option<bool>) -> Option<String> {
    match allowed {
        Some(true) => None,
        Some(false) => Some(
            "the repository disallows rebase merges (allow_rebase_merge is false), so the ruleset's one merge method is refused and no request can merge"
                .to_owned(),
        ),
        None => Some("the repository's allow_rebase_merge switch is missing or unreadable".to_owned()),
    }
}

/// The bypass actors of a ruleset body as `(type, id, mode)`, sorted, the
/// order the release bypass is built in.
fn bypass_key(actors: &Value) -> Vec<(String, i64, String)> {
    let mut held: Vec<(String, i64, String)> = actors
        .as_array()
        .into_iter()
        .flatten()
        .map(|actor| {
            (
                actor["actor_type"].as_str().unwrap_or_default().to_owned(),
                actor["actor_id"].as_i64().unwrap_or_default(),
                actor["bypass_mode"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect();
    held.sort();
    held
}

/// The release branch ruleset: active, on branches, covering exactly the
/// release branch patterns, carrying exactly `creation` and `update`, and
/// passed by exactly the release bypass actors.
fn github_release_branch_ruleset(ctx: &Ctx, run: &mut Runner) -> Result<StepState, RkError> {
    let name = ctx.release_branch_ruleset();
    let patterns = ctx.release_branch_patterns();
    Ok(
        match release_ruleset_faults(ctx, run, name, "branch", &patterns, &["creation", "update"])?
        {
            Ok(faults) if faults.is_empty() => StepState::ok(format!(
                "{name} lets only the release actors create or update {}",
                patterns.join(", ")
            )),
            Ok(faults) => StepState::not(faults.join("; ")),
            Err(err) => StepState::unknown(err),
        },
    )
}

/// The faults of a ruleset that only the release bypass actors pass:
/// active, on `target` refs, covering exactly `patterns` with no
/// exclusion, carrying exactly `rules`, and naming exactly the release
/// bypass actors. A missing ruleset is one fault. `Err` carries an
/// unreadable answer, which proves nothing either way.
fn release_ruleset_faults(
    ctx: &Ctx,
    run: &mut Runner,
    name: &str,
    target: &str,
    patterns: &[String],
    rules: &[&str],
) -> Result<Result<Vec<String>, String>, RkError> {
    let detail = match github_ruleset_body(ctx, run, name)? {
        RulesetLookup::Found(detail) => detail,
        RulesetLookup::Absent => return Ok(Ok(vec![format!("no ruleset named {name}")])),
        RulesetLookup::Unreadable(err) => return Ok(Err(err)),
    };
    let mut faults = Vec::new();
    if detail["enforcement"] != "active" {
        faults.push(format!("{name} is not active"));
    }
    if detail["target"] != target {
        faults.push(format!("{name} does not target {target} refs"));
    }
    let include: Vec<&str> = detail["conditions"]["ref_name"]["include"]
        .as_array()
        .map(|list| list.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    for pattern in patterns {
        if !include.contains(&pattern.as_str()) {
            faults.push(format!("{name} does not cover {pattern}"));
        }
    }
    for extra in include
        .iter()
        .filter(|held| !patterns.iter().any(|pattern| pattern == *held))
    {
        faults.push(format!(
            "{name} covers {extra}, which the setup does not own"
        ));
    }
    if detail["conditions"]["ref_name"]["exclude"] != serde_json::json!([]) {
        faults.push(format!("{name} excludes refs from its own coverage"));
    }
    let mut held: Vec<&str> = detail["rules"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|rule| rule["type"].as_str())
                .collect()
        })
        .unwrap_or_default();
    held.sort_unstable();
    let mut expected = rules.to_vec();
    expected.sort_unstable();
    if held != expected {
        faults.push(format!(
            "{name} carries the rules [{}] where the setup owns [{}]",
            held.join(", "),
            expected.join(", ")
        ));
    }
    if bypass_key(&detail["bypass_actors"]) != bypass_key(&ctx.release_bypass_actors()) {
        faults.push(format!(
            "{name} names other bypass actors than the repository administrator and the release App"
        ));
    }
    Ok(Ok(faults))
}

/// Each release branch pattern on GitLab: protected with exactly one push
/// grant and one merge grant at Maintainer, and force pushes off.
fn gitlab_release_branches(
    ctx: &Ctx,
    run: &mut Runner,
    project: &str,
) -> Result<StepState, RkError> {
    let level_ok = |levels: &Value| {
        levels
            .as_array()
            .is_some_and(|list| list.len() == 1 && list[0]["access_level"] == 40)
    };
    let patterns = ctx.release_branch_patterns();
    let mut faults = Vec::new();
    for pattern in &patterns {
        let encoded = pattern.replace('/', "%2F").replace('*', "%2A");
        match api_get(
            ctx,
            run,
            &format!("projects/{project}/protected_branches/{encoded}"),
        )? {
            Api::Ok(body) => {
                if body["allow_force_push"] != false {
                    faults.push(format!("{pattern} allows force pushes"));
                }
                if !level_ok(&body["push_access_levels"]) || !level_ok(&body["merge_access_levels"])
                {
                    faults.push(format!(
                        "{pattern} grants are not exactly one push and one merge at Maintainer"
                    ));
                }
            }
            Api::Missing => faults.push(format!("{pattern} is not protected")),
            Api::Failed(err) => return Ok(StepState::unknown(err)),
        }
    }
    Ok(if faults.is_empty() {
        StepState::ok(format!(
            "{} take a push or a merge at Maintainer alone",
            patterns.join(", ")
        ))
    } else {
        StepState::not(faults.join("; "))
    })
}

/// The value of `protect_merge_request_pipelines` that keeps merge request
/// pipelines away from the protected variables and runners.
pub const GITLAB_MERGE_REQUEST_PIPELINE_ACCESS_OFF: bool = false;

/// The GitLab release that added the merge request pipeline access to
/// protected variables and runners. Below it, a merge request pipeline never
/// reads a protected variable.
pub const GITLAB_MERGE_REQUEST_PIPELINE_SETTING_SINCE: (u64, u64) = (18, 1);

/// What an answer with no merge request pipeline setting reads from 18.1.
///
/// The projects API returns the attribute from GitLab 18.10, and only to a
/// reader at Maintainer or above, while from 18.1 a new project has the
/// access on, so the access is unproved.
pub const GITLAB_MERGE_REQUEST_PIPELINE_UNEXPOSED: &str = "this GitLab does not show protect_merge_request_pipelines to this reader: the projects API returns it from GitLab 18.10, and only to a Maintainer or above. From 18.1 a new project lets its merge request pipelines reach the protected variables and runners, so that access is unproved. Read the project as a Maintainer on GitLab 18.10 or later, or clear \"Allow merge request pipelines to access protected variables and runners\" under Settings > CI/CD > Variables by hand";

/// The state of a GitLab project answer that carries no merge request
/// pipeline setting, judged by the instance version: below 18.1 the
/// setting does not exist, at or above it the setting is not exposed to
/// this reader, and a version that cannot be read leaves it unknown.
///
/// SATISFIES release-ref-rules:merge-request-pipelines-cannot-read-protected-variables
fn unexposed_merge_request_pipeline_state(
    ctx: &Ctx,
    run: &mut Runner,
) -> Result<StepState, RkError> {
    let version = match api_get(ctx, run, "version")? {
        Api::Ok(body) => body["version"].as_str().and_then(gitlab_version_pair),
        Api::Missing | Api::Failed(_) => None,
    };
    Ok(match version {
        Some(pair) if pair < GITLAB_MERGE_REQUEST_PIPELINE_SETTING_SINCE => StepState::ok(
            "pipelines are enabled; this GitLab predates 18.1, where a merge request pipeline never reads a protected variable",
        ),
        Some(_) => StepState::not(GITLAB_MERGE_REQUEST_PIPELINE_UNEXPOSED),
        None => StepState::unknown(
            "the project answer carries no protect_merge_request_pipelines, and the GitLab version cannot be read to tell an instance older than 18.1 from a reader the setting is not shown to",
        ),
    })
}

/// What is wrong with the merge request pipeline setting of a GitLab
/// project answer that carries it, if anything. An answer with no such
/// attribute is judged by [`unexposed_merge_request_pipeline_state`].
fn merge_request_pipeline_fault(project: &Value) -> Option<String> {
    match project.get("protect_merge_request_pipelines") {
        None => None,
        Some(value) if *value == GITLAB_MERGE_REQUEST_PIPELINE_ACCESS_OFF => None,
        Some(Value::Bool(_)) => {
            Some("merge request pipelines can reach the protected variables and runners".to_owned())
        }
        Some(_) => Some("the merge request pipeline setting is unreadable".to_owned()),
    }
}

/// One ruleset lookup by name: found, provably absent, or unreadable —
/// an unreadable inventory must never read as an absent ruleset.
enum RulesetLookup {
    /// The ruleset exists; its detail body.
    Found(Value),
    /// The inventory was read successfully and no ruleset carries the
    /// name.
    Absent,
    /// The inventory or the detail could not be read.
    Unreadable(String),
}

/// A ruleset's detail body by name.
fn github_ruleset_body(ctx: &Ctx, run: &mut Runner, name: &str) -> Result<RulesetLookup, RkError> {
    // A 404 on the collection is an unreachable inventory — a missing
    // repository or an unauthorized read — never an empty one: an empty
    // inventory answers 200 with an empty list.
    let list = match api_get(ctx, run, &format!("repos/{}/rulesets", ctx.repo))? {
        Api::Ok(body) => body,
        Api::Missing => {
            return Ok(RulesetLookup::Unreadable(
                "the ruleset inventory is not readable".into(),
            ));
        }
        Api::Failed(err) => return Ok(RulesetLookup::Unreadable(err)),
    };
    let id = list
        .as_array()
        .into_iter()
        .flatten()
        .find(|ruleset| ruleset["name"] == name)
        .and_then(|ruleset| ruleset["id"].as_i64());
    let Some(id) = id else {
        return Ok(RulesetLookup::Absent);
    };
    match api_get(ctx, run, &format!("repos/{}/rulesets/{id}", ctx.repo))? {
        Api::Ok(body) => Ok(RulesetLookup::Found(body)),
        // A listed id that answers 404 is not proof of absence either — the
        // forge also answers 404 for an unauthorized read — so a rerun
        // decides, rather than a false drift.
        Api::Missing => Ok(RulesetLookup::Unreadable(format!(
            "the {name} detail is not readable"
        ))),
        Api::Failed(err) => Ok(RulesetLookup::Unreadable(err)),
    }
}

/// The fault for each open release request that stands armed to merge
/// itself, with the call that disarms it, or `None` where none does.
///
/// A request is the release driver's own where its head branch carries
/// the driver's prefix. `head` and `field` name where the request states
/// its head branch: `head.ref` on GitHub, `source_branch` on GitLab.
///
/// SATISFIES forge-setup:an-armed-release-request-faults
fn armed_fault(ctx: &Ctx, requests: &Value, head: &str, field: &str) -> Option<String> {
    let prefix = crate::projection::release_driver_shape(ctx.driver())?;
    let github = ctx.forge == Some(Forge::Github);
    let armed: Vec<String> = requests
        .as_array()?
        .iter()
        .filter(|request| {
            let branch = if field.is_empty() {
                &request[head]
            } else {
                &request[head][field]
            };
            branch.as_str().is_some_and(|name| name.starts_with(prefix))
        })
        .filter(|request| {
            if github {
                !request["auto_merge"].is_null()
            } else {
                request["merge_when_pipeline_succeeds"] == true
                    || request["auto_merge_enabled"] == true
            }
        })
        .filter_map(|request| {
            let number = request[if github { "number" } else { "iid" }].as_u64()?;
            let forge = ctx.forge?;
            Some(format!(
                "{} disarms with {}",
                crate::release_state::request_name(forge, number),
                crate::release_state::disarm_command(forge, &ctx.repo, number)
            ))
        })
        .collect();
    (!armed.is_empty()).then(|| {
        format!(
            "an open release request stands armed to merge itself, and a release waits for the operator's order: {}; rk setup step merge-on-order --apply disarms every one",
            armed.join("; ")
        )
    })
}

/// The GitLab limitation `protect-tags` and `protections-check` report.
const GITLAB_TAG_LIMITATION: &str =
    "an Owner or Maintainer can still delete a protected tag through the UI or API";

/// The fault a merge queue on the trunk reads as: what is enabled, what it
/// costs, and how to undo it. This convention refuses a queue rather than
/// owning one, so the operator needs the consequence rather than a rule
/// type's bare name.
const MERGE_QUEUE_FAULT: &str = "a merge queue is enabled on the trunk; this convention lands no workflow that triggers on merge_group, so the queue waits on a required check that never reports and drops the request when its CI timeout expires. rk setup step protect-trunk --apply rewrites the ruleset without it";

/// The freshness defect is independent of an absent required check.
const STALE_MERGE_FAULT: &str = "the trunk permits a merge from a branch that does not carry the trunk's tip; a release request can therefore ship a version computed against a trunk that moved. rk setup step protect-trunk --apply rewrites the ruleset with the freshness requirement";

/// The GitLab limitation `protect-trunk` and `protections-check` report:
/// the title gate rides the request's own pipeline on this forge.
const GITLAB_TITLE_LIMITATION: &str = "the title gate stops accident, not authority: a merge request runs its own CI configuration, and a title edit starts no new pipeline";

/// The fault of an observed pipeline requirement against the forge gate's
/// answer, or `None` where the two agree.
///
/// SATISFIES target-config:the-effective-protection-follows-the-profile
fn pipeline_fault(forge_gate: bool, observed: Option<bool>) -> Option<String> {
    match (forge_gate, observed) {
        (true, Some(true)) | (false, Some(false)) => None,
        (true, Some(false)) => Some("the pipeline requirement is off".to_owned()),
        (false, Some(true)) => Some(
            "the pipeline requirement is on, and no forge gate lands to run a pipeline, so no merge request can merge"
                .to_owned(),
        ),
        (_, None) => Some("the pipeline requirement is missing or unreadable".to_owned()),
    }
}

/// The same limitation under the `rebase` form: the commit gate rides the
/// request's own pipeline too, and every push starts one, so only the
/// first half holds.
const GITLAB_COMMIT_LIMITATION: &str =
    "the commit gate stops accident, not authority: a merge request runs its own CI configuration";

/// The bot identity form a GitLab run takes: a project access token, or a
/// service account of the top-level group whose id this carries.
enum GitlabIdentity {
    Token,
    ServiceAccount(u64),
}

/// The identity form the forge and the tier allow today, read the way the
/// `install-bot` script reads it: gitlab.com reads the namespace's plan and
/// kind, and every other instance takes a token with any license. `Err`
/// carries the state that ends the observation.
fn gitlab_identity_form(
    ctx: &Ctx,
    run: &mut Runner,
) -> Result<Result<GitlabIdentity, StepState>, RkError> {
    if !ctx.on_gitlab_com() {
        return Ok(Ok(GitlabIdentity::Token));
    }
    let root = ctx.repo.split('/').next().unwrap_or_default();
    let namespace = match api_get(ctx, run, &format!("namespaces/{root}"))? {
        Api::Ok(body) => body,
        Api::Missing => {
            return Ok(Err(StepState::not(format!(
                "the forge does not know the namespace {root}"
            ))));
        }
        Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
    };
    let plan = namespace["plan"].as_str().unwrap_or_default();
    let kind = namespace["kind"].as_str().unwrap_or_default();
    let paid = ["premium", "ultimate", "silver", "gold"]
        .iter()
        .any(|tier| plan.starts_with(tier))
        || plan == "opensource";
    Ok(match (plan, kind) {
        _ if paid => Ok(GitlabIdentity::Token),
        ("free", "group") => namespace["id"].as_u64().map_or_else(
            || {
                Err(StepState::unknown(format!(
                    "the namespace {root} answers no id"
                )))
            },
            |id| Ok(GitlabIdentity::ServiceAccount(id)),
        ),
        ("free", "user") => Err(StepState::not(format!(
            "the personal namespace {root} has no bot identity form on GitLab.com Free; move the project under a group, or use a Premium or Ultimate tier"
        ))),
        _ => Err(StepState::unknown(format!(
            "the namespace {root} reads plan {plan:?} and kind {kind:?}, which name no identity form"
        ))),
    })
}

/// Whether the bot identity a GitLab run takes today is a service account,
/// or why the form could not be read.
///
/// A failed reading is never taken for the token form: a caller that
/// forces an action on a service account must stop on it.
///
/// # Errors
/// A runner failure.
pub fn gitlab_bot_is_a_service_account(
    ctx: &Ctx,
    run: &mut Runner,
) -> Result<Result<bool, String>, RkError> {
    Ok(match gitlab_identity_form(ctx, run)? {
        Ok(form) => Ok(matches!(form, GitlabIdentity::ServiceAccount(_))),
        Err(state) => Err(state.detail().to_owned()),
    })
}

/// What the GitLab project holds for the release bot's token variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotVariable {
    /// The forge answered 404: no variable is stored.
    Missing,
    /// Stored, but a pipeline on any branch reads it.
    Unprotected,
    /// Stored, and only a pipeline on a protected branch or a protected
    /// tag reads it.
    Protected,
    /// The read failed, with the CLI's own words.
    Unreadable(String),
}

/// The state of `RELEASE_BOT_TOKEN` on the target's GitLab project.
///
/// It reads whether the token is stored, and whether it is protected. One
/// reader serves the observation of `bot-secrets`, `protections-check`,
/// and the apply that protects it.
///
/// # Errors
/// A runner failure.
pub fn gitlab_bot_variable_state(ctx: &Ctx, run: &mut Runner) -> Result<BotVariable, RkError> {
    let project = ctx.repo.replace('/', "%2F");
    Ok(
        match api_get(
            ctx,
            run,
            &format!("projects/{project}/variables/RELEASE_BOT_TOKEN"),
        )? {
            Api::Ok(body) => match body.get("protected") {
                Some(Value::Bool(true)) => BotVariable::Protected,
                Some(Value::Bool(false)) => BotVariable::Unprotected,
                _ => BotVariable::Unreadable(
                    "the protected flag of RELEASE_BOT_TOKEN is unreadable".to_owned(),
                ),
            },
            Api::Missing => BotVariable::Missing,
            Api::Failed(err) => BotVariable::Unreadable(err),
        },
    )
}

/// The remedy for a bot token that a pipeline on any branch reads.
pub const GITLAB_UNPROTECTED_BOT_VARIABLE: &str = "RELEASE_BOT_TOKEN is not protected, so a pipeline on any branch reads it; rk setup --apply protects it";

/// Whether the bot's variable is stored, as the last half of either form.
fn gitlab_bot_variable(
    ctx: &Ctx,
    run: &mut Runner,
    project: &str,
    holds: &str,
) -> Result<StepState, RkError> {
    Ok(
        match api_get(
            ctx,
            run,
            &format!("projects/{project}/variables/RELEASE_BOT_TOKEN"),
        )? {
            Api::Ok(_) => StepState::ok(format!("{holds} and its variable is stored")),
            Api::Missing => StepState::not(format!(
                "{holds} with no stored variable; a rerun replaces its token"
            )),
            Api::Failed(err) => StepState::unknown(err),
        },
    )
}

/// One paginated listing, read until a short page proves it exhausted, or
/// until `found` holds. `None` is a bound reached on a full page, which
/// decides nothing.
fn gitlab_listing(
    ctx: &Ctx,
    run: &mut Runner,
    path: &str,
    found: impl Fn(&serde_json::Value) -> bool,
) -> Result<Result<Option<serde_json::Value>, StepState>, RkError> {
    for page in 1..=10u32 {
        let list = match api_get(ctx, run, &format!("{path}&per_page=100&page={page}"))? {
            Api::Ok(body) => body.as_array().cloned().unwrap_or_default(),
            Api::Missing => Vec::new(),
            Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
        };
        if let Some(hit) = list.iter().find(|entry| found(entry)) {
            return Ok(Ok(Some(hit.clone())));
        }
        if list.len() < 100 {
            return Ok(Ok(None));
        }
    }
    Ok(Err(StepState::unknown(
        "the listing did not exhaust within ten pages; nothing was decided",
    )))
}

/// Every entry of one paginated listing, read until a short page proves
/// it exhausted. A bound reached on a full page decides nothing.
fn gitlab_all(
    ctx: &Ctx,
    run: &mut Runner,
    path: &str,
) -> Result<Result<Vec<serde_json::Value>, StepState>, RkError> {
    let mut all = Vec::new();
    for page in 1..=10u32 {
        let list = match api_get(ctx, run, &format!("{path}&per_page=100&page={page}"))? {
            Api::Ok(body) => body.as_array().cloned().unwrap_or_default(),
            Api::Missing => Vec::new(),
            Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
        };
        let short = list.len() < 100;
        all.extend(list);
        if short {
            return Ok(Ok(all));
        }
    }
    Ok(Err(StepState::unknown(
        "the listing did not exhaust within ten pages; nothing was decided",
    )))
}

/// Every open request, read page by page until a parsed page shorter
/// than the page size proves the listing exhausted. A missing listing, a
/// body that is no array, a failed read, or a bound reached on a full
/// page decides nothing, so no armed request hides behind an unread page.
fn open_requests(
    ctx: &Ctx,
    run: &mut Runner,
    path: &str,
) -> Result<Result<Value, StepState>, RkError> {
    let mut all = Vec::new();
    for page in 1..=10u32 {
        let list = match api_get(ctx, run, &format!("{path}&per_page=100&page={page}"))? {
            Api::Ok(Value::Array(list)) => list,
            Api::Ok(_) => {
                return Ok(Err(StepState::unknown(
                    "the open requests did not read as a list; nothing was decided",
                )));
            }
            Api::Missing => {
                return Ok(Err(StepState::unknown(
                    "the forge refused the listing of open requests; nothing was decided",
                )));
            }
            Api::Failed(err) => return Ok(Err(StepState::unknown(err))),
        };
        let short = list.len() < 100;
        all.extend(list);
        if short {
            return Ok(Ok(Value::Array(all)));
        }
    }
    Ok(Err(StepState::unknown(
        "the listing did not exhaust within ten pages; nothing was decided",
    )))
}

/// The description every token `install-bot` mints carries, so the live
/// bot is found by what marks it rather than by a default's spelling.
const BOT_MARKER: &str = "release-kit bot identity";

/// The state a configured name that is not the live bot's takes: a rename
/// would strand the identity that serves the project.
fn gitlab_rename(live: &str, name: &str) -> StepState {
    StepState::not(format!(
        "the release-kit bot {live} serves this project, and setup.bot.name asks for {name}; a rename strands the first identity, so revoke it by hand or set the name back"
    ))
}

/// The state of a service account that already serves this project under
/// another name than the configured one, `None` where there is none: an account of the group that is a member
/// here and either holds a marked token or carries the default username
/// from before the marker existed. An unrelated member is neither.
fn gitlab_live_account(
    ctx: &Ctx,
    run: &mut Runner,
    project: &str,
    group: u64,
    default: &str,
    name: &str,
) -> Result<Option<StepState>, RkError> {
    let accounts = match gitlab_all(
        ctx,
        run,
        &format!("groups/{group}/service_accounts?order_by=id"),
    )? {
        Ok(accounts) => accounts,
        Err(state) => return Ok(Some(state)),
    };
    let members = match gitlab_all(
        ctx,
        run,
        &format!("projects/{project}/members/all?order_by=id"),
    )? {
        Ok(members) => members,
        Err(state) => return Ok(Some(state)),
    };
    for account in &accounts {
        let (Some(id), Some(user)) = (account["id"].as_u64(), account["username"].as_str()) else {
            continue;
        };
        if user == name
            || !members
                .iter()
                .any(|member| member["id"].as_u64() == Some(id))
        {
            continue;
        }
        if user == default {
            return Ok(Some(gitlab_rename(user, name)));
        }
        let path =
            format!("groups/{group}/service_accounts/{id}/personal_access_tokens?state=active");
        let tokens = match gitlab_all(ctx, run, &path)? {
            Ok(tokens) => tokens,
            Err(state) => return Ok(Some(state)),
        };
        if tokens
            .iter()
            .any(|token| token["description"] == BOT_MARKER && token["active"] != false)
        {
            return Ok(Some(gitlab_rename(user, name)));
        }
    }
    Ok(None)
}

/// The `install-bot` observation: the form the tier selects, then that
/// form's identity under the configured name, then the stored variable.
/// The listings paginate exactly as the script's do, so an identity past
/// the first page does not read as absent.
fn gitlab_install_bot(ctx: &Ctx, run: &mut Runner, project: &str) -> Result<StepState, RkError> {
    let form = match gitlab_identity_form(ctx, run)? {
        Ok(form) => form,
        Err(state) => return Ok(state),
    };
    match form {
        GitlabIdentity::Token => {
            let name = ctx.bot_name().unwrap_or("release-bot");
            let path = format!("projects/{project}/access_tokens?state=active");
            let tokens = match gitlab_all(ctx, run, &path)? {
                Ok(tokens) => tokens,
                Err(state) => return Ok(state),
            };
            let active: Vec<_> = tokens
                .iter()
                .filter(|token| token["revoked"] == false && token["active"] != false)
                .collect();
            // The live bot under another name is a rename whether or not a
            // token under the requested name exists.
            let live = active.iter().find(|token| {
                token["name"] != name
                    && (token["description"] == BOT_MARKER || token["name"] == "release-bot")
            });
            if let Some(live) = live.and_then(|token| token["name"].as_str()) {
                return Ok(gitlab_rename(live, name));
            }
            if !active.iter().any(|token| token["name"] == name) {
                return Ok(StepState::not(format!("no active {name} token exists")));
            }
            gitlab_bot_variable(ctx, run, project, &format!("an active {name} token exists"))
        }
        GitlabIdentity::ServiceAccount(group) => {
            let project_id = match api_get(ctx, run, &format!("projects/{project}"))? {
                Api::Ok(body) => body["id"].as_u64(),
                Api::Missing => {
                    return Ok(StepState::not(format!(
                        "the forge does not know {}",
                        ctx.repo
                    )));
                }
                Api::Failed(err) => return Ok(StepState::unknown(err)),
            };
            let Some(project_id) = project_id else {
                return Ok(StepState::unknown("the project answers no id"));
            };
            let name = ctx
                .bot_name()
                .map_or_else(|| format!("release-bot-{project_id}"), str::to_owned);
            let path = format!("groups/{group}/service_accounts?order_by=id");
            let account = match gitlab_listing(ctx, run, &path, |account| {
                account["username"] == name.as_str()
            })? {
                Ok(account) => account,
                Err(state) => return Ok(state),
            };
            // The live bot under another name is a rename whether or not an
            // account under the requested name exists.
            let default = format!("release-bot-{project_id}");
            if let Some(state) = gitlab_live_account(ctx, run, project, group, &default, &name)? {
                return Ok(state);
            }
            let Some(id) = account.and_then(|account| account["id"].as_u64()) else {
                return Ok(StepState::not(format!(
                    "no service account named {name} exists"
                )));
            };
            let level = match api_get(ctx, run, &format!("projects/{project}/members/all/{id}"))? {
                Api::Ok(body) => body["access_level"].as_u64().unwrap_or(0),
                Api::Missing => {
                    return Ok(StepState::not(format!(
                        "the service account {name} is not a member of this project"
                    )));
                }
                Api::Failed(err) => return Ok(StepState::unknown(err)),
            };
            if level < 40 {
                return Ok(StepState::not(format!(
                    "the service account {name} is a member of this project below Maintainer"
                )));
            }
            gitlab_bot_variable(
                ctx,
                run,
                project,
                &format!("a service account named {name} is a Maintainer of this project"),
            )
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per setup step, so the match is what makes an unobserved step a compile error"
)]
fn gitlab(ctx: &Ctx, step: &str, run: &mut Runner) -> Result<StepState, RkError> {
    let trunk = ctx.trunk();
    let project = ctx.repo.replace('/', "%2F");
    match step {
        "private-vulnerability-reporting" => {
            let path = format!("projects/{project}");
            Ok(match api_get(ctx, run, &path)? {
                Api::Ok(body) => {
                    let access = body["issues_access_level"].as_str();
                    if !matches!(access, Some("enabled" | "private" | "disabled")) {
                        StepState::unknown("issue intake access is unreadable")
                    } else if body
                        .get("issues_enabled")
                        .is_some_and(|flag| !flag.is_boolean())
                    {
                        StepState::unknown("legacy issue intake flag is unreadable")
                    } else if body["issues_enabled"] == false || access == Some("disabled") {
                        StepState::not("issue intake is disabled; see setup guide step 3g")
                    } else if access == Some("private") {
                        StepState::not("issue intake is restricted; see setup guide step 3g")
                    } else {
                        StepState::ok_with_limitation(
                            "issue intake is enabled",
                            GITLAB_PRIVATE_REPORTING_LIMITATION,
                        )
                    }
                }
                Api::Missing => {
                    StepState::unknown(format!("{path}: issue intake is unreadable (404)"))
                }
                Api::Failed(err) => StepState::unknown(format!("{path}: {err}")),
            })
        }

        "default-branch" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                let found = body["default_branch"].as_str().unwrap_or("");
                if found == trunk {
                    StepState::ok(format!("{trunk} is the default branch"))
                } else {
                    StepState::not(format!("the default branch is {found}"))
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "single-trunk" => {
            for candidate in ctx.retired_branches() {
                let candidate = candidate.as_str();
                if candidate == trunk {
                    continue;
                }
                match api_get(
                    ctx,
                    run,
                    &format!("projects/{project}/repository/branches/{candidate}"),
                )? {
                    Api::Missing => {}
                    Api::Ok(_) => {
                        return Ok(StepState::not(format!("a {candidate} branch still exists")));
                    }
                    Api::Failed(err) => return Ok(StepState::unknown(err)),
                }
            }
            Ok(StepState::ok(
                "no long-lived branch besides the trunk remains",
            ))
        }
        "merge-cleanup" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                if body["remove_source_branch_after_merge"]
                    .as_bool()
                    .unwrap_or(false)
                {
                    StepState::ok("a merged branch is deleted by the forge")
                } else {
                    StepState::not("a merged branch outlives its merge")
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "merge-on-order" => {
            match api_get(ctx, run, &format!("projects/{project}"))? {
                Api::Ok(body) => {
                    if !body["only_allow_merge_if_pipeline_succeeds"]
                        .as_bool()
                        .unwrap_or(false)
                    {
                        return Ok(StepState::not(
                            "the pipeline requirement that holds a person's merge behind a passing pipeline is off; rk setup step merge-on-order --apply asserts it",
                        ));
                    }
                }
                Api::Missing => {
                    return Ok(StepState::not(format!(
                        "the forge does not know {}",
                        ctx.repo
                    )));
                }
                Api::Failed(err) => return Ok(StepState::unknown(err)),
            }
            let path = format!("projects/{project}/merge_requests?state=opened");
            Ok(match open_requests(ctx, run, &path)? {
                Ok(requests) => armed_fault(ctx, &requests, "source_branch", "")
                    .map_or_else(
                        || {
                            StepState::ok(
                                "no release request stands armed to merge itself, and a merge waits for a passing pipeline",
                            )
                        },
                        StepState::not,
                    ),
                Err(state) => state,
            })
        }
        "ci-permissions" => Ok(match api_get(ctx, run, &format!("projects/{project}"))? {
            Api::Ok(body) => {
                if body["jobs_enabled"] != true {
                    StepState::not("pipelines are disabled")
                } else if let Some(fault) = merge_request_pipeline_fault(&body) {
                    StepState::not(fault)
                } else if body.get("protect_merge_request_pipelines").is_none() {
                    unexposed_merge_request_pipeline_state(ctx, run)?
                } else {
                    StepState::ok(
                        "pipelines are enabled, and merge request pipelines cannot reach the protected variables and runners",
                    )
                }
            }
            Api::Missing => StepState::not(format!("the forge does not know {}", ctx.repo)),
            Api::Failed(err) => StepState::unknown(err),
        }),
        "install-bot" => gitlab_install_bot(ctx, run, &project),
        "bot-secrets" => Ok(match gitlab_bot_variable_state(ctx, run)? {
            BotVariable::Protected => StepState::ok(
                "RELEASE_BOT_TOKEN is stored and protected, so only a pipeline on a protected branch or tag reads it",
            ),
            BotVariable::Unprotected => StepState::not(GITLAB_UNPROTECTED_BOT_VARIABLE),
            BotVariable::Missing => StepState::not("RELEASE_BOT_TOKEN is not stored"),
            BotVariable::Unreadable(err) => StepState::unknown(err),
        }),
        "protect-trunk" => {
            let protection = match api_get(
                ctx,
                run,
                &format!("projects/{project}/protected_branches/{trunk}"),
            )? {
                Api::Ok(body) => body,
                Api::Missing => {
                    return Ok(StepState::not(format!("{trunk} is not protected")));
                }
                Api::Failed(err) => return Ok(StepState::unknown(err)),
            };
            // Exactly one push grant, and it is the no-access entry: the
            // forge honors the most permissive grant, so a second entry
            // beside access level 0 is a branch that still takes a push.
            let grants = protection["push_access_levels"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let policy = ctx.protection();
            let no_push =
                grants.len() == 1 && grants[0]["access_level"] == policy.gitlab.push_access_level;
            // The merge grant is owned exactly too: a merge level of 0 keeps
            // every release request unmergeable while the push shape reads
            // clean, so both halves are checked.
            let merges = protection["merge_access_levels"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let can_merge =
                merges.len() == 1 && merges[0]["access_level"] == policy.gitlab.merge_access_level;
            let settings = match api_get(ctx, run, &format!("projects/{project}"))? {
                Api::Ok(body) => body,
                Api::Missing | Api::Failed(_) => Value::Null,
            };
            let mut faults = Vec::new();
            if !no_push {
                faults.push(format!(
                    "{trunk} still takes a direct push: the forge honors the most permissive of {} push grants",
                    grants.len()
                ));
            }
            if !can_merge {
                faults.push(format!(
                    "{trunk} merge grants are not exactly the one owned maintainer level"
                ));
            }
            if protection["allow_force_push"] != false {
                faults.push(format!("{trunk} allows force pushes"));
            }
            // The pipeline is required exactly where a forge gate lands to
            // run in it. With none, no pipeline runs, so a requirement left
            // on would hold every merge request; it is judged in both
            // directions, and a value that cannot be read is no answer.
            if let Some(fault) = pipeline_fault(
                ctx.forge_gate(),
                settings["only_allow_merge_if_pipeline_succeeds"].as_bool(),
            ) {
                faults.push(fault);
            }
            if settings["merge_method"] != policy.gitlab.merge_method.as_str() {
                faults.push("the merge method is not fast-forward".to_owned());
            }
            let squash = ctx.merge_form() == crate::landing::MergeForm::Squash;
            if settings["squash_option"] != policy.gitlab.squash_option.as_str() {
                faults.push(if squash {
                    "merge requests do not always squash".to_owned()
                } else {
                    "merge requests may squash, so a request's commits would not keep their own messages".to_owned()
                });
            }
            // The template is the squash commit's message source, so it is
            // asserted only where a merge squashes.
            if squash
                && settings["squash_commit_template"]
                    != policy.gitlab.squash_commit_template.as_str()
            {
                faults.push("the squash template is not the merge request's title".to_owned());
            }
            Ok(if faults.is_empty() {
                let detail = format!("{trunk} holds the release-merge shape");
                if ctx.forge_gate() {
                    StepState::ok_with_limitation(
                        detail,
                        if squash {
                            GITLAB_TITLE_LIMITATION
                        } else {
                            GITLAB_COMMIT_LIMITATION
                        },
                    )
                } else {
                    // No gate lands, so no gate's limitation applies.
                    StepState::ok(detail)
                }
            } else {
                StepState::not(faults.join("; "))
            })
        }
        "protect-tags" => {
            let pattern = ctx.tag_pattern();
            let encoded = pattern.replace('/', "%2F").replace('*', "%2A");
            Ok(
                match api_get(
                    ctx,
                    run,
                    &format!("projects/{project}/protected_tags/{encoded}"),
                )? {
                    Api::Ok(body) => {
                        let grants = body["create_access_levels"].as_array().map_or(0, Vec::len);
                        if grants == 1 && body["create_access_levels"][0]["access_level"] == 40 {
                            StepState::ok_with_limitation(
                                format!("{pattern} is created at Maintainer alone"),
                                GITLAB_TAG_LIMITATION,
                            )
                        } else {
                            StepState::not(format!(
                                "{pattern} create grants are not exactly one at Maintainer: the forge honors the most permissive of {grants}"
                            ))
                        }
                    }
                    Api::Missing => StepState::not(format!("{pattern} is not protected")),
                    Api::Failed(err) => StepState::unknown(err),
                },
            )
        }
        "protect-release-lines" => Ok(
            match api_get(
                ctx,
                run,
                &format!("projects/{project}/protected_branches/release%2F%2A"),
            )? {
                Api::Ok(body) => {
                    let level_ok = |levels: &Value| {
                        levels
                            .as_array()
                            .is_some_and(|list| list.len() == 1 && list[0]["access_level"] == 40)
                    };
                    if body["allow_force_push"] != false {
                        StepState::not("release/* allows force pushes")
                    } else if !level_ok(&body["push_access_levels"])
                        || !level_ok(&body["merge_access_levels"])
                    {
                        // A push level of 0 blocks the documented
                        // cherry-pick-by-push path while force-push reads
                        // clean, so the grant shape is owned exactly.
                        StepState::not(
                            "release/* grants are not exactly the owned maintainer levels",
                        )
                    } else {
                        StepState::ok("release/* refuses force pushes and deletion by git clients")
                    }
                }
                Api::Missing => StepState::inapplicable(
                    "release/* is unprotected; optional — applied only where older lines exist",
                ),
                Api::Failed(err) => StepState::unknown(err),
            },
        ),
        "protect-release-branches" => gitlab_release_branches(ctx, run, &project),
        "protections-check" => {
            // Same separation as the sibling forge: proven drift wins,
            // an outage with nothing proven wrong stays unknown.
            let mut failures = Vec::new();
            let mut unknowns = Vec::new();
            // Every satisfied step's limitation survives the aggregate: a
            // first limitation must not shadow a second.
            let mut limitations: Vec<String> = Vec::new();
            for owned in owned_protections(ctx) {
                match gitlab(ctx, owned, run)? {
                    StepState::Satisfied {
                        limitation: found, ..
                    } => limitations.extend(found),
                    StepState::Inapplicable { .. } => {}
                    StepState::Unsatisfied { detail } => {
                        failures.push(format!("{owned}: {detail}"));
                    }
                    StepState::Unknown { detail } | StepState::Unreadable { detail } => {
                        unknowns.push(format!("{owned}: {detail}"));
                    }
                }
            }
            // The merge request pipeline setting, which the script judges
            // beside the protections wherever ci-permissions applies and
            // writes it: on an automatic release.
            match if ctx.automatic_release() {
                api_get(ctx, run, &format!("projects/{project}"))?
            } else {
                Api::Ok(Value::Null)
            } {
                Api::Ok(body) => {
                    if let Some(fault) = merge_request_pipeline_fault(&body) {
                        failures.push(format!(
                            "ci-permissions: {fault}; rk setup step ci-permissions --apply"
                        ));
                    } else if body.is_object()
                        && body.get("protect_merge_request_pipelines").is_none()
                    {
                        match unexposed_merge_request_pipeline_state(ctx, run)? {
                            StepState::Unsatisfied { detail } => {
                                failures.push(format!("ci-permissions: {detail}"));
                            }
                            StepState::Unknown { detail } | StepState::Unreadable { detail } => {
                                unknowns.push(format!("ci-permissions: {detail}"));
                            }
                            _ => {}
                        }
                    }
                }
                Api::Missing | Api::Failed(_) => {
                    unknowns.push("the merge request pipeline setting is not readable".to_owned());
                }
            }
            // The bot token, where an automatic release stores one: only a
            // pipeline on a protected ref reads it. A missing variable is
            // the bot-secrets step's own gap, not a protection's.
            if ctx.automatic_release() {
                match gitlab_bot_variable_state(ctx, run)? {
                    BotVariable::Unprotected => {
                        failures.push(format!("bot-secrets: {GITLAB_UNPROTECTED_BOT_VARIABLE}"));
                    }
                    BotVariable::Unreadable(err) => {
                        unknowns.push(format!("bot-secrets: {err}"));
                    }
                    BotVariable::Protected | BotVariable::Missing => {}
                }
            }
            Ok(if !failures.is_empty() {
                StepState::not(failures.join("; "))
            } else if !unknowns.is_empty() {
                StepState::unknown(unknowns.join("; "))
            } else {
                StepState::Satisfied {
                    detail: "the protections hold, as far as this forge enforces them".into(),
                    limitation: if limitations.is_empty() {
                        None
                    } else {
                        Some(limitations.join("; "))
                    },
                }
            })
        }
        _ => Ok(StepState::unknown(format!("no observation for {step}"))),
    }
}

#[cfg(test)]
mod tests {
    /// The ci-permissions observation of one GitLab project answer, with
    /// the instance version that a second read answers.
    fn ci_permissions_with(project: &str, version: Option<&str>) -> super::StepState {
        let ctx = crate::setup::context::Ctx::for_tests(
            camino::Utf8PathBuf::from("/tmp/target"),
            "acme/widget".to_owned(),
            crate::detect::Forge::Gitlab,
            std::path::PathBuf::from("glab"),
            Some("bash"),
        );
        let mut answers = vec![
            Some(project.to_owned()),
            version.map(|v| format!(r#"{{"version":"{v}"}}"#)),
        ]
        .into_iter();
        super::observe(&ctx, "ci-permissions", &mut |_exec| {
            Ok(answers.next().flatten().map_or_else(
                || crate::setup::process::Outcome {
                    exit_code: 1,
                    stdout: Vec::new(),
                    stderr: b"glab: dial tcp: connection refused".to_vec(),
                },
                |body| crate::setup::process::Outcome {
                    exit_code: 0,
                    stdout: body.into_bytes(),
                    stderr: Vec::new(),
                },
            ))
        })
        .expect("the observation runs")
    }

    /// The merge request pipeline setting in all four of its branches: a
    /// present attribute is judged by its value, and an absent one by the
    /// instance version, where an unreadable version never passes.
    ///
    /// SATISFIES release-ref-rules:merge-request-pipelines-cannot-read-protected-variables
    #[test]
    fn the_merge_request_pipeline_setting_is_judged_in_four_branches() {
        use super::StepState;
        let present = |value: &str| {
            ci_permissions_with(
                &format!(r#"{{"jobs_enabled":true,"protect_merge_request_pipelines":{value}}}"#),
                None,
            )
        };
        assert!(matches!(present("false"), StepState::Satisfied { .. }));
        assert!(matches!(present("true"), StepState::Unsatisfied { .. }));
        let absent = r#"{"jobs_enabled":true}"#;
        match ci_permissions_with(absent, Some("18.0.3-ee")) {
            StepState::Satisfied { detail, .. } => {
                assert!(detail.contains("predates 18.1"), "{detail}");
            }
            other => panic!("below 18.1: {other:?}"),
        }
        match ci_permissions_with(absent, Some("19.7.0-pre")) {
            StepState::Unsatisfied { detail } => assert!(
                detail.contains("18.10")
                    && detail.contains("Maintainer")
                    && detail.contains(
                        "Allow merge request pipelines to access protected variables and runners"
                    ),
                "{detail}"
            ),
            other => panic!("from 18.1: {other:?}"),
        }
        assert!(
            matches!(ci_permissions_with(absent, None), StepState::Unknown { .. }),
            "an unreadable version never reads satisfied"
        );
    }

    /// The repository's rebase switch passes only where it reads on.
    #[test]
    fn merge_form_rebase_switch_is_judged() {
        assert_eq!(super::rebase_switch_fault(Some(true)), None);
        assert!(
            super::rebase_switch_fault(Some(false))
                .is_some_and(|fault| fault.contains("allow_rebase_merge is false"))
        );
        assert!(
            super::rebase_switch_fault(None)
                .is_some_and(|fault| fault.contains("missing or unreadable"))
        );
    }

    /// The pipeline requirement is judged in both directions, and a value
    /// that cannot be read is never ready.
    ///
    /// SATISFIES target-config:the-effective-protection-follows-the-profile
    #[test]
    fn release_less_pipeline_requirement_is_judged_both_ways() {
        assert_eq!(super::pipeline_fault(true, Some(true)), None);
        assert_eq!(super::pipeline_fault(false, Some(false)), None);
        for (gate, observed) in [
            (true, Some(false)),
            (false, Some(true)),
            (true, None),
            (false, None),
        ] {
            assert!(
                super::pipeline_fault(gate, observed).is_some(),
                "gate {gate}, observed {observed:?}"
            );
        }
    }
}
