//! `rk lines`: the release-line lifecycle — inventory, open, candidates,
//! and retirement.
//!
//! A line is `release/<major>.<minor>`, a second trunk with four rules
//! this module holds: it is cut from an explicit base, never the tip by
//! default (`maintenance:a-line-is-cut-from-an-explicit-base`); `mint`
//! writes a candidate's version and pushes it, and the release half tags
//! it, so `rk` creates no tag and `rc` only reads them; a mint acts only
//! on the tip its caller saw; and a line retires only behind its tags,
//! seat before branch, with the remote deletion left to the operator
//! (`maintenance:a-line-is-never-retired-before-its-tags`). Every
//! mutating verb previews by default.

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::lines::{LinesAction, LinesArgs};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::maintenance::{self, Deletion};
use crate::output::Output;
use crate::release_engine::adapter::{self, Version};
use crate::worktree::{Worktree, parse_worktrees};

/// Route one `rk lines` invocation.
///
/// # Errors
///
/// Each verb's own refusals; every failure is typed.
pub fn run(args: &LinesArgs) -> Result<(), RkError> {
    crate::requirements::require("rk lines", &[crate::requirements::Tool::Git])?;
    match &args.action {
        LinesAction::List { target, json } => list(target, Output::new(*json)),
        LinesAction::Open {
            line,
            base,
            target,
            apply,
            json,
        } => open(line, base.as_deref(), target, *apply, Output::new(*json)),
        LinesAction::Mint {
            line,
            expect,
            promote,
            target,
            as_bot,
            apply,
            json,
        } => mint(
            line,
            expect.as_deref(),
            *promote,
            target,
            *as_bot,
            *apply,
            Output::new(*json),
        ),
        LinesAction::Rc { line, target, json } => rc(line, target, Output::new(*json)),
        LinesAction::Retire {
            line,
            target,
            apply,
            json,
        } => retire(line, target, *apply, Output::new(*json)),
    }
}

/// One line's row in the inventory.
#[derive(Debug, Clone, Serialize)]
pub struct LineRow {
    /// The `<major>.<minor>` name.
    pub line: String,
    /// The branch, `release/<line>`.
    pub branch: String,
    /// Where the branch exists: `local`, `remote`, or `both`.
    pub presence: &'static str,
    /// The newest `v<line>.*` release tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_release: Option<String>,
    /// The newest `v<line>.*-rc.*` candidate tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest_candidate: Option<String>,
    /// Whether every commit the local branch holds beyond the trunk is
    /// reachable from a tag — what makes a retirement safe. Absent for a
    /// remote-only line, which this clone cannot judge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag_covered: Option<bool>,
    /// The worktree seating the branch, where one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seat: Option<String>,
}

/// The machine form of `rk lines list`.
#[derive(Debug, Serialize)]
struct ListReport {
    /// The shape version of this document.
    schema: &'static str,
    /// Every line, sorted by name.
    lines: Vec<LineRow>,
}

fn list(target: &Utf8Path, out: Output) -> Result<(), RkError> {
    let rows = rows(target)?;
    if rows.is_empty() {
        out.result_line("no release lines; the trunk is the only line alive");
    }
    for row in &rows {
        let mut parts = vec![format!("{} ({})", row.branch, row.presence)];
        if let Some(tag) = &row.newest_release {
            parts.push(format!("newest release {tag}"));
        }
        if let Some(tag) = &row.newest_candidate {
            parts.push(format!("newest candidate {tag}"));
        }
        match row.tag_covered {
            Some(true) => parts.push("tag-covered".to_owned()),
            Some(false) => parts.push("commits beyond the tags; not retirable".to_owned()),
            None => {}
        }
        if let Some(seat) = &row.seat {
            parts.push(format!("seated at {seat}"));
        }
        out.result_line(parts.join(" — "));
    }
    out.emit(&ListReport {
        schema: "rk.lines-list/1",
        lines: rows,
    })
}

/// Every release line at `target`, sorted by name, read offline from the
/// clone: the inventory `rk lines list` prints and `rk release status`
/// reports.
///
/// # Errors
///
/// A configuration that does not load, and a git call that fails.
pub fn rows(target: &Utf8Path) -> Result<Vec<LineRow>, RkError> {
    let prefix = crate::config::line_prefix_of(target.as_std_path())?;
    let local = ref_names(target, &format!("refs/heads/{prefix}"))?;
    let remote: Vec<String> = ref_names(target, &format!("refs/remotes/origin/{prefix}"))?
        .into_iter()
        .filter_map(|name| name.strip_prefix("origin/").map(str::to_owned))
        .collect();
    let mut names: Vec<String> = local.iter().chain(remote.iter()).cloned().collect();
    names.sort();
    names.dedup();
    let seats = inventory(target)?;
    let mut rows = Vec::new();
    for branch in names {
        let Some(line) = branch.strip_prefix(&prefix) else {
            continue;
        };
        let is_local = local.contains(&branch);
        let presence = match (is_local, remote.contains(&branch)) {
            (true, true) => "both",
            (true, false) => "local",
            _ => "remote",
        };
        rows.push(LineRow {
            line: line.to_owned(),
            branch: branch.clone(),
            presence,
            newest_release: newest_tag(target, line, false)?,
            newest_candidate: newest_tag(target, line, true)?,
            tag_covered: if is_local {
                Some(uncovered_commits(target, &branch)?.is_empty())
            } else {
                None
            },
            seat: seats
                .iter()
                .find(|seat| seat.branch.as_deref() == Some(branch.as_str()))
                .map(|seat| seat.path.to_string()),
        });
    }
    Ok(rows)
}

/// The machine form of `rk lines open`, on every path: the seat is the
/// worktree verb's own job, named as the next action rather than run here,
/// so the verb's one schema holds in both workflow modes.
#[derive(Debug, Serialize)]
struct OpenReport {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview`, `created`, or `satisfied`.
    mode: &'static str,
    /// The branch the verb acted on.
    branch: String,
    /// What follows, in order.
    next: Vec<String>,
}

fn open(
    line: &str,
    base: Option<&str>,
    target: &Utf8Path,
    apply: bool,
    out: Output,
) -> Result<(), RkError> {
    let branch = line_branch(line, &crate::config::line_prefix_of(target.as_std_path())?)?;
    let Some(base) = base else {
        return Err(RkError::Usage(format!(
            "a line is a snapshot of a chosen commit, so {branch} takes no default base; pass --base \"v<version>\", the tag it patches"
        )));
    };
    if branch_exists(target, &branch)? {
        out.result_line(format!(
            "satisfied: {branch} already exists; the open adopts it"
        ));
        let next = vec![format!(
            "rk worktree add {branch} --apply seats it; in the branches mode, git checkout {branch} works in the main checkout"
        )];
        return out.emit(&OpenReport {
            schema: "rk.lines-open/1",
            mode: "satisfied",
            branch,
            next,
        });
    }
    // A remote-only line is adopted at its own tip, never recreated from
    // the base: the line may have advanced past the tag it was cut from,
    // and a local branch behind the remote cannot push.
    let remote = format!("origin/{branch}");
    if remote_exists(target, &branch)? {
        if !apply {
            out.result_line(format!(
                "DRY RUN: would adopt {branch} from {remote}, tracking it at the remote tip; --base is not used, because the line already exists"
            ));
            let next = vec![format!(
                "rk lines open {line} --base \"{base}\" --target {target} --apply"
            )];
            out.next(&next);
            return out.emit(&OpenReport {
                schema: "rk.lines-open/1",
                mode: "preview",
                branch,
                next,
            });
        }
        let tracked = git(target, &["branch", "--track", &branch, &remote])?;
        if !tracked.status.success() {
            return Err(RkError::refusal(
                Diagnostic::new(Reason::StateDrift, last_line(&tracked.stderr))
                    .expected("a tracking branch git can create from the remote line")
                    .target_state("unchanged"),
            ));
        }
        out.result_line(format!(
            "adopted {branch} from {remote} at the remote tip; --base was not used, because the line already exists"
        ));
        let next = vec![format!(
            "rk worktree add {branch} --apply seats it; in the branches mode, git checkout {branch} works in the main checkout"
        )];
        out.next(&next);
        return out.emit(&OpenReport {
            schema: "rk.lines-open/1",
            mode: "created",
            branch,
            next,
        });
    }
    let resolved = resolve_commit(target, base)?;
    if !apply {
        out.result_line(format!(
            "DRY RUN: would create {branch} at {base} ({resolved})"
        ));
        let next = vec![format!(
            "rk lines open {line} --base \"{base}\" --target {target} --apply"
        )];
        out.next(&next);
        return out.emit(&OpenReport {
            schema: "rk.lines-open/1",
            mode: "preview",
            branch,
            next,
        });
    }
    let created = git(target, &["branch", &branch, &resolved])?;
    if !created.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::StateDrift, last_line(&created.stderr))
                .expected("a branch git can create at the named base")
                .target_state("unchanged"),
        ));
    }
    out.result_line(format!("created {branch} at {base} ({resolved})"));
    let next = vec![
        format!(
            "rk worktree add {branch} --apply seats it; in the branches mode, git checkout {branch} works in the main checkout"
        ),
        format!("git push -u origin {branch} publishes it with its upstream set"),
        "rk setup step protect-release-lines --apply protects every line, once per repository"
            .to_owned(),
    ];
    out.next(&next);
    out.emit(&OpenReport {
        schema: "rk.lines-open/1",
        mode: "created",
        branch,
        next,
    })
}

/// The machine form of `rk lines rc`.
#[derive(Debug, Serialize)]
struct RcReport {
    /// The shape version of this document.
    schema: &'static str,
    /// The line the verb read.
    line: String,
    /// The newest `v<line>.*` release tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    newest_release: Option<String>,
    /// The newest candidate tag, absent while none exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    newest_candidate: Option<String>,
    /// The number the next candidate takes, absent while none exists yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    next_candidate: Option<u64>,
}

fn rc(line: &str, target: &Utf8Path, out: Output) -> Result<(), RkError> {
    line_branch(line, &crate::config::line_prefix_of(target.as_std_path())?)?;
    let newest_release = newest_tag(target, line, false)?;
    let newest_candidate = newest_tag(target, line, true)?;
    let next_candidate = newest_candidate
        .as_deref()
        .and_then(|tag| tag.rsplit_once("-rc.")?.1.parse::<u64>().ok())
        .map(|n| n + 1);
    match &newest_candidate {
        Some(tag) => {
            out.result_line(format!("newest candidate {tag}"));
            if let Some(next) = next_candidate {
                out.result_line(format!(
                    "the next candidate is rc.{next}; an rc number is single-use"
                ));
            }
        }
        None => out.result_line(empty_candidate_message(target)),
    }
    if let Some(tag) = &newest_release {
        out.result_line(format!("newest release {tag}"));
    }
    out.emit(&RcReport {
        schema: "rk.lines-rc/1",
        line: line.to_owned(),
        newest_release,
        newest_candidate,
        next_candidate,
    })
}

/// What a mint does, decided from the remote tags and the manifest alone.
#[derive(Debug, PartialEq, Eq)]
enum MintDecision {
    /// Write this candidate's version, commit, and push.
    Mint(Version),
    /// The manifest already carries a candidate whose tag does not exist:
    /// the release half has not published it yet, so nothing is written
    /// and the caller's release step publishes it.
    Pending(Version),
}

/// Decide the next candidate on line `<major>.<minor>`.
///
/// The stable base is the newest stable release of the line plus one
/// patch, or `<major>.<minor>.0` while the line has released nothing; the
/// number is the highest candidate of that base plus one. A line only
/// ships `<major>.<minor>.x`, so no argument names the base.
fn decide_mint(
    line: (u64, u64),
    remote_tags: &[Version],
    manifest: Option<Version>,
) -> Result<MintDecision, String> {
    let on_line: Vec<Version> = remote_tags
        .iter()
        .copied()
        .filter(|tag| (tag.major, tag.minor) == line)
        .collect();
    let base_patch = on_line
        .iter()
        .filter(|tag| tag.rc.is_none())
        .map(|tag| tag.patch)
        .max()
        .map_or(0, |patch| patch + 1);
    if let Some(current) = manifest.filter(|version| version.rc.is_some())
        && !on_line.contains(&current)
    {
        if (current.major, current.minor, current.patch) == (line.0, line.1, base_patch) {
            return Ok(MintDecision::Pending(current));
        }
        return Err(format!(
            "the manifest carries the untagged candidate {}, which is not a candidate of the next release {}.{}.{}",
            current.tag(),
            line.0,
            line.1,
            base_patch
        ));
    }
    let number = on_line
        .iter()
        .filter(|tag| tag.patch == base_patch)
        .filter_map(|tag| tag.rc)
        .max()
        .map_or(1, |n| n + 1);
    Ok(MintDecision::Mint(Version {
        major: line.0,
        minor: line.1,
        patch: base_patch,
        rc: Some(number),
    }))
}

/// The machine form of `rk lines mint`.
#[derive(Debug, Serialize)]
struct MintReport {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview`, `minted`, `promoted`, or `pending`.
    mode: &'static str,
    /// The line the verb acted on.
    line: String,
    /// The branch the verb pushes, the line prefix plus the line.
    branch: String,
    /// The line's tip on the remote when the verb read it.
    tip: String,
    /// The version the verb writes or found, in the manifest's own
    /// spelling: the candidate's, or the stable one a promotion writes.
    version: String,
    /// The tag the release half creates for that version.
    tag: String,
    /// The package the version file names, where it names one: the name a
    /// validator installs the candidate by.
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<String>,
    /// The commit the verb pushed, under `minted` and `promoted` alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
}

/// What every mint path reads before it decides: the line, its remote
/// state, and the target's version file.
struct MintContext<'a> {
    line: &'a str,
    numbers: (u64, u64),
    branch: String,
    target: &'a Utf8Path,
    driver: String,
    file: &'static str,
    tip: String,
    tags: Vec<RemoteTag>,
    manifest: Option<Version>,
    /// Whether the commit takes the bot's identity instead of the caller's.
    as_bot: bool,
}

impl MintContext<'_> {
    fn report(&self, mode: &'static str, version: Version, commit: Option<String>) -> MintReport {
        MintReport {
            schema: "rk.lines-mint/1",
            mode,
            line: self.line.to_owned(),
            branch: self.branch.clone(),
            tip: self.tip.clone(),
            version: version.spelled(&self.driver),
            tag: version.tag(),
            package: adapter::package_name(self.target.as_std_path(), &self.driver),
            commit,
        }
    }

    fn versions(&self) -> Vec<Version> {
        self.tags.iter().map(|tag| tag.version).collect()
    }
}

fn mint(
    line: &str,
    expect: Option<&str>,
    promote: bool,
    target: &Utf8Path,
    as_bot: bool,
    apply: bool,
    out: Output,
) -> Result<(), RkError> {
    let branch = line_branch(line, &crate::config::line_prefix_of(target.as_std_path())?)?;
    let numbers = line
        .split_once('.')
        .and_then(|(major, minor)| Some((major.parse().ok()?, minor.parse().ok()?)))
        .ok_or_else(|| RkError::Usage(format!("'{line}' is not a line")))?;
    if apply && expect.is_none() {
        return Err(RkError::Usage(format!(
            "a mint acts only on the tip its caller saw; pass --expect <sha>, the tip of {branch} you read"
        )));
    }
    let driver = recorded_driver(target)?;
    // Only the engine drivers commit a cut in a job that holds a bot
    // credential; a rust line's jobs set the App's identity themselves.
    if as_bot && !matches!(driver.as_str(), "bash" | "python") {
        return Err(RkError::Usage(format!(
            "--as-bot commits as the bot on a bash or python line, and this line releases {driver}; drop --as-bot"
        )));
    }
    let (file, _) = adapter::version_file(&driver).ok_or_else(|| {
        RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!("the release driver {driver} carries no version file this verb writes"),
            )
            .expected("a rust, python, or bash target")
            .target_state("unchanged"),
        )
    })?;
    let (tip, tags) = remote_line(target, &branch, line)?;
    if let Some(expect) = expect {
        refuse_a_moved_line(target, &branch, &tip, expect)?;
    }
    let manifest = adapter::manifest_version(target.as_std_path(), &driver)
        .as_deref()
        .and_then(Version::parse);
    let context = MintContext {
        line,
        numbers,
        branch,
        target,
        driver,
        file,
        tip,
        tags,
        manifest,
        as_bot,
    };
    if promote {
        promote_candidate(&context, apply, out)
    } else {
        mint_candidate(&context, apply, out)
    }
}

/// Mint the line's next candidate, or report the pending one.
fn mint_candidate(context: &MintContext<'_>, apply: bool, out: Output) -> Result<(), RkError> {
    let MintContext {
        line,
        branch,
        target,
        driver,
        file,
        tip,
        ..
    } = context;
    let decision = decide_mint(context.numbers, &context.versions(), context.manifest).map_err(
        |detail| {
            RkError::refusal(
                Diagnostic::new(Reason::StateDrift, detail)
                    .expected("a manifest that carries a released version or the pending candidate of the next release")
                    .action(format!("restore {file} to the line's released version, then mint again"))
                    .target_state("unchanged"),
            )
        },
    )?;
    let candidate = match decision {
        MintDecision::Pending(version) => {
            out.result_line(format!(
                "pending: {file} already carries {}, and {} does not exist yet; nothing is written, and the release step publishes it",
                version.spelled(driver),
                version.tag()
            ));
            return out.emit(&context.report("pending", version, None));
        }
        MintDecision::Mint(version) => version,
    };
    if !apply {
        out.result_line(format!(
            "DRY RUN: would write {} into {file}, commit, and push {branch} from {tip}; the release half tags {}",
            candidate.spelled(driver),
            candidate.tag()
        ));
        let next = vec![format!(
            "rk lines mint {line} --expect {tip} --target {target} --apply"
        )];
        out.next(&next);
        return out.emit(&context.report("preview", candidate, None));
    }
    let message = format!("chore(release): mint {}", candidate.tag());
    let commit = commit_and_push(context, candidate, &message)?;
    out.result_line(format!(
        "minted {} on {branch} at {commit}; the release half tags {}",
        candidate.spelled(driver),
        candidate.tag()
    ));
    out.emit(&context.report("minted", candidate, Some(commit)))
}

/// Promote the newest candidate of the next release: write its stable
/// version, so the line's release request proposes it. Only the code that
/// was tested as that candidate is promoted.
fn promote_candidate(context: &MintContext<'_>, apply: bool, out: Output) -> Result<(), RkError> {
    let MintContext {
        line,
        branch,
        target,
        driver,
        file,
        tip,
        ..
    } = context;
    let newest = newest_candidate(context.numbers, &context.tags).ok_or_else(|| {
        RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!("{branch} has no tagged candidate of its next release to promote"),
            )
            .expected("a candidate the release half tagged and a human validated")
            .action(format!("rk lines mint {line} mints one"))
            .target_state("unchanged"),
        )
    })?;
    let stable = Version {
        rc: None,
        ..newest.version
    };
    let untested = untested_commits(context, &newest.commit, tip)?;
    if !untested.is_empty() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "{branch} holds {} commit(s) after {}, which no candidate tested: {}",
                    untested.len(),
                    newest.version.tag(),
                    untested.join("; ")
                ),
            )
            .expected(format!(
                "{branch} at the commit {} tagged",
                newest.version.tag()
            ))
            .action(format!(
                "rk lines mint {line} mints a candidate of them; promote that one once it stands"
            ))
            .target_state("unchanged"),
        ));
    }
    if context.manifest == Some(stable) {
        out.result_line(format!(
            "pending: {file} already carries {}; nothing is written, and the line's release request releases it",
            stable.spelled(driver)
        ));
        return out.emit(&context.report("pending", stable, None));
    }
    if !apply {
        out.result_line(format!(
            "DRY RUN: would write {} into {file}, commit, and push {branch} from {tip}; the line's release request then proposes it",
            stable.spelled(driver)
        ));
        let next = vec![format!(
            "rk lines mint {line} --promote --expect {tip} --target {target} --apply"
        )];
        out.next(&next);
        return out.emit(&context.report("preview", stable, None));
    }
    let message = format!(
        "chore(release): promote {} to {}",
        newest.version.tag(),
        stable.tag()
    );
    let commit = commit_and_push(context, stable, &message)?;
    out.result_line(format!(
        "promoted {} to {} on {branch} at {commit}; merge the line's release request to release it",
        newest.version.tag(),
        stable.spelled(driver)
    ));
    out.emit(&context.report("promoted", stable, Some(commit)))
}

/// The newest tagged candidate of the line's next release.
fn newest_candidate(line: (u64, u64), tags: &[RemoteTag]) -> Option<&RemoteTag> {
    let versions: Vec<Version> = tags.iter().map(|tag| tag.version).collect();
    let MintDecision::Mint(next) = decide_mint(line, &versions, None).ok()? else {
        return None;
    };
    tags.iter()
        .filter(|tag| tag.version.rc.is_some() && tag.version.patch == next.patch)
        .filter(|tag| (tag.version.major, tag.version.minor) == line)
        .max_by_key(|tag| tag.version.rc)
}

/// Write the version, commit it, and push the line by name; a rejected
/// push is the lost race, named with the tip that won.
fn commit_and_push(
    context: &MintContext<'_>,
    version: Version,
    message: &str,
) -> Result<String, RkError> {
    let MintContext {
        line,
        branch,
        target,
        driver,
        file,
        tip,
        ..
    } = context;
    // The bot's identity is read before any file is written, so a forge
    // that names no account leaves the checkout and the remote unchanged.
    let identity = if context.as_bot {
        Some(bot_identity(target)?)
    } else {
        None
    };
    // A python version is written through the adapter, uv version, which
    // relocks, so the commit carries pyproject.toml and uv.lock together.
    let written = if *driver == "python" {
        let python = adapter::for_driver(driver, "")
            .ok_or_else(|| RkError::Usage(format!("no version writer for the {driver} driver")))?;
        python
            .write_version(target, version)?
            .into_iter()
            .map(String::from)
            .collect()
    } else {
        write_manifest_and_lock(target, driver, file, version)?
    };
    let mut add = vec!["add", "--"];
    add.extend(written.iter().map(String::as_str));
    run_git(target, &add, "the version staged")?;
    if let Some((name, email)) = identity {
        // As the engine commits: the identity the forge names, with every
        // identity variable the job exports scrubbed so none displaces it.
        let user = format!("user.name={name}");
        let mail = format!("user.email={email}");
        let committed = crate::release_engine::git(
            target,
            &["-c", &user, "-c", &mail, "commit", "-q", "-m", message],
        )?;
        if !committed.status.success() {
            return Err(RkError::subprocess(
                Diagnostic::new(Reason::SubprocessFailed, last_line(&committed.stderr))
                    .expected("a commit under the identity the forge names for the bot")
                    .target_state("the version file is written and not pushed"),
            ));
        }
    } else {
        run_git(
            target,
            &["commit", "-q", "-m", message],
            "a commit under the identity the checkout configures",
        )?;
    }
    let commit = resolve_commit(target, "HEAD")?;
    let refspec = format!("HEAD:refs/heads/{branch}");
    let pushed = git(target, &["push", "-q", "origin", &refspec])?;
    if pushed.status.success() {
        return Ok(commit);
    }
    // Only a remote tip that moved is the lost race; a policy or hook
    // refusal leaves the tip where it was and keeps the server's words.
    if let Ok((now, _)) = remote_line(target, branch, line)
        && now != *tip
    {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::RemoteConflict,
                format!(
                    "{branch} moved to {now} while this mint ran; the push of {} was rejected",
                    version.tag()
                ),
            )
            .expected(format!("{branch} still at {tip}"))
            .action(format!(
                "read the new tip, then rk lines mint {line} --expect {now} --apply"
            ))
            .target_state("the remote is unchanged; the local commit is not pushed"),
        ));
    }
    let server: Vec<String> = String::from_utf8_lossy(&pushed.stderr)
        .lines()
        .map(str::trim)
        .filter(|row| !row.is_empty())
        .map(str::to_owned)
        .collect();
    Err(RkError::subprocess(
        Diagnostic::new(Reason::SubprocessFailed, server.join("; "))
            .expected(format!("a push of {branch} the remote accepts"))
            .target_state("the remote is unchanged; the local commit is not pushed"),
    ))
}

/// The name and the email of the account the forge names for the job's
/// bot credential, read through the engine's own reader, so a cut and a
/// release request commit under one identity.
///
/// SATISFIES bot-identity:the-bot-identity-reaches-one-project
fn bot_identity(target: &Utf8Path) -> Result<(String, String), RkError> {
    let engine = crate::release_engine::Context::load(target)?;
    crate::release_engine::forge::Client::resolve(engine.forge, target, &engine.repo)?.identity()
}

/// Write the version into the rust or bash version file and, where git
/// tracks it, the rust lockfile, and name every file written.
fn write_manifest_and_lock(
    target: &Utf8Path,
    driver: &str,
    file: &str,
    version: Version,
) -> Result<Vec<String>, RkError> {
    let spelled = version.spelled(driver);
    // Both new contents are prepared and validated before either file is
    // written, so a refusal leaves the checkout as it found it.
    let manifest = versioned_manifest(target, driver, &spelled)?;
    // The lockfile is part of the update only where git tracks it: an
    // ignored local one is never read, rewritten, or staged.
    let lock_tracked = driver == "rust"
        && git(target, &["ls-files", "--error-unmatch", "--", "Cargo.lock"])?
            .status
            .success();
    let lock = if lock_tracked {
        let old = adapter::manifest_version(target.as_std_path(), driver).unwrap_or_default();
        let name = adapter::package_name(target.as_std_path(), driver).unwrap_or_default();
        versioned_lock(target, &name, &old, &spelled)?
    } else {
        None
    };
    std::fs::write(target.join(file), manifest).map_err(RkError::Io)?;
    let mut written = vec![file.to_owned()];
    if let Some(lock) = lock {
        std::fs::write(target.join("Cargo.lock"), lock).map_err(RkError::Io)?;
        written.push("Cargo.lock".to_owned());
    }
    Ok(written)
}

/// One `v<line>.*` tag on the remote: its version and the commit it names.
#[derive(Debug, Clone)]
struct RemoteTag {
    version: Version,
    commit: String,
}

/// The release driver the landing record names.
fn recorded_driver(target: &Utf8Path) -> Result<String, RkError> {
    crate::landing::manifest::load(target)?
        .and_then(|manifest| manifest.profile.release.driver)
        .ok_or_else(|| {
            RkError::refusal(
                Diagnostic::new(
                    Reason::StateDrift,
                    "the landing record names no release driver, so no version file is known",
                )
                .expected("a target release-kit landed, checked out at the line")
                .target_state("unchanged"),
            )
        })
}

/// The line's remote tip and every `v<line>.*` tag the remote holds, with
/// the commit each names, read from the remote itself, because a clone
/// holds only the tags it fetched.
fn remote_line(
    target: &Utf8Path,
    branch: &str,
    line: &str,
) -> Result<(String, Vec<RemoteTag>), RkError> {
    let head = format!("refs/heads/{branch}");
    let tags = format!("refs/tags/v{line}.*");
    let output = git(target, &["ls-remote", "origin", &head, &tags])?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, last_line(&output.stderr))
                .expected("an origin remote git can read")
                .target_state("unchanged"),
        ));
    }
    let mut tip = None;
    // An annotated tag lists its own object, then the commit it peels to
    // under `^{}`; the peeled row wins.
    let mut commits = std::collections::BTreeMap::new();
    for row in String::from_utf8_lossy(&output.stdout).lines() {
        let Some((sha, name)) = row.split_once('\t') else {
            continue;
        };
        if name == head {
            tip = Some(sha.to_owned());
        } else if let Some(peeled) = name.strip_suffix("^{}") {
            commits.insert(peeled.to_owned(), sha.to_owned());
        } else {
            commits
                .entry(name.to_owned())
                .or_insert_with(|| sha.to_owned());
        }
    }
    let versions = commits
        .into_iter()
        .filter_map(|(name, commit)| {
            let version = name
                .strip_prefix("refs/tags/v")
                .and_then(Version::parse)
                .filter(|version| version.rc.is_none() || name.contains("-rc."))?;
            Some(RemoteTag { version, commit })
        })
        .collect();
    let tip = tip.ok_or_else(|| {
        RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, format!("origin holds no {branch}"))
                .expected("a line published to the origin remote")
                .action(format!("git push -u origin {branch} publishes it"))
                .target_state("unchanged"),
        )
    })?;
    Ok((tip, versions))
}

/// Refuse unless the remote tip, the checkout's HEAD, and the working tree
/// all stand at the tip the caller saw, so the mint edits exactly the tree
/// of the requested line. A detached checkout at that commit passes.
fn refuse_a_moved_line(
    target: &Utf8Path,
    branch: &str,
    tip: &str,
    expect: &str,
) -> Result<(), RkError> {
    let expected = resolve_commit(target, expect).unwrap_or_else(|_| expect.to_owned());
    if tip != expected {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!("{branch} is at {tip} on the remote, not at the expected {expected}"),
            )
            .expected(format!("{branch} at {expected}"))
            .action(format!(
                "read the line again, then mint with --expect {tip}"
            ))
            .target_state("unchanged"),
        ));
    }
    let head = resolve_commit(target, "HEAD")?;
    if head != expected {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "the checkout is at {head}, not at the expected tip {expected} of {branch}"
                ),
            )
            .expected(format!("a checkout of {branch} at {expected}"))
            .target_state("unchanged"),
        ));
    }
    let status = git(
        target,
        &["status", "--porcelain", "--untracked-files=normal"],
    )?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                "the working tree carries uncommitted or untracked changes",
            )
            .expected("a clean checkout of the line")
            .target_state("unchanged"),
        ));
    }
    Ok(())
}

/// The version file's new content with `version` written in, every other
/// byte of a TOML manifest kept.
fn versioned_manifest(target: &Utf8Path, driver: &str, version: &str) -> Result<String, RkError> {
    let (file, table) = adapter::version_file(driver)
        .ok_or_else(|| RkError::Usage(format!("no version file for {driver}")))?;
    let Some(table) = table else {
        return Ok(format!("{version}\n"));
    };
    let text = std::fs::read_to_string(target.join(file)).map_err(RkError::Io)?;
    let mut document: toml_edit::DocumentMut = text.parse().map_err(|err| {
        RkError::refusal(
            Diagnostic::new(Reason::StateDrift, format!("{file} does not parse: {err}"))
                .expected(format!("a {file} with a [{table}] version"))
                .target_state("unchanged"),
        )
    })?;
    let Some(slot) = document
        .get_mut(table)
        .and_then(toml_edit::Item::as_table_like_mut)
        .and_then(|table| table.get_mut("version"))
        .and_then(toml_edit::Item::as_value_mut)
        .filter(|value| value.is_str())
    else {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::StateDrift,
                format!("{file} carries no literal [{table}] version to write"),
            )
            .expected(format!("a {file} whose [{table}] version is a string"))
            .target_state("unchanged"),
        ));
    };
    let decor = slot.decor().clone();
    *slot = toml_edit::Value::from(version);
    *slot.decor_mut() = decor;
    Ok(document.to_string())
}

/// `Cargo.lock`'s new content with the package's own entry moved from
/// `old` to `new`, or `None` where the target keeps no lockfile.
///
/// The own entry is the one with the package's name, no `source`, and the
/// old version, so a same-name path crate keeps its version. Every
/// dependency reference qualified with the old version moves with it; a
/// bare `"<name>"` reference stays valid. Nothing else changes, so no
/// dependency selection can move and no resolve is needed.
fn versioned_lock(
    target: &Utf8Path,
    name: &str,
    old: &str,
    new: &str,
) -> Result<Option<String>, RkError> {
    let path = target.join("Cargo.lock");
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(RkError::Io)?;
    let refuse = |detail: String| {
        RkError::refusal(
            Diagnostic::new(Reason::StateDrift, detail)
                .expected(format!(
                    "a Cargo.lock cargo wrote, with one {name} {old} entry and no source"
                ))
                .action("run cargo generate-lockfile or cargo update --workspace, commit it, then mint again")
                .target_state("unchanged"),
        )
    };
    let mut document: toml_edit::DocumentMut = text
        .parse()
        .map_err(|err| refuse(format!("Cargo.lock does not parse: {err}")))?;
    let Some(packages) = document
        .get_mut("package")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
    else {
        return Err(refuse("Cargo.lock lists no packages".to_owned()));
    };
    let old_reference = format!("{name} {old}");
    let new_reference = format!("{name} {new}");
    let mut own = 0;
    for package in packages.iter_mut() {
        let is_own = package.get("name").and_then(toml_edit::Item::as_str) == Some(name)
            && package.get("version").and_then(toml_edit::Item::as_str) == Some(old)
            && !package.contains_key("source");
        if is_own {
            own += 1;
            package["version"] = toml_edit::value(new);
        }
        if let Some(dependencies) = package
            .get_mut("dependencies")
            .and_then(toml_edit::Item::as_array_mut)
        {
            for reference in dependencies.iter_mut() {
                if reference.as_str() == Some(old_reference.as_str()) {
                    let decor = reference.decor().clone();
                    *reference = toml_edit::Value::from(new_reference.as_str());
                    *reference.decor_mut() = decor;
                }
            }
        }
    }
    if own != 1 {
        return Err(refuse(format!(
            "Cargo.lock holds {own} entries of {name} {old} with no source"
        )));
    }
    Ok(Some(document.to_string()))
}

/// The two sides of one file a commit changed, with the package's own
/// version on each side.
struct Sides<'a> {
    before: &'a str,
    after: &'a str,
    own_before: &'a str,
    own_after: &'a str,
}

/// Whether one version file reads the same on both sides once the
/// package's own version is set aside: a version-only change.
fn same_but_version(driver: &str, file: &str, name: &str, sides: &Sides<'_>) -> bool {
    if file == "Cargo.lock" {
        return masked_lock(sides.before, name, sides.own_before)
            .is_some_and(|before| Some(before) == masked_lock(sides.after, name, sides.own_after));
    }
    if file == "uv.lock" {
        return masked_uv_lock(sides.before, name, sides.own_before).is_some_and(|before| {
            Some(before) == masked_uv_lock(sides.after, name, sides.own_after)
        });
    }
    match adapter::version_file(driver) {
        Some((_, None)) => {
            Version::parse(sides.before.trim()).is_some()
                && Version::parse(sides.after.trim()).is_some()
        }
        Some((_, Some(table))) => {
            let masked = |text: &str| -> Option<toml::Table> {
                let mut value: toml::Table = toml::from_str(text).ok()?;
                value.get_mut(table)?.as_table_mut()?.remove("version")?;
                Some(value)
            };
            masked(sides.before).is_some_and(|before| Some(before) == masked(sides.after))
        }
        None => false,
    }
}

/// `Cargo.lock` with the package's own entry and the references qualified
/// with its version reduced to the name alone. The own entry is the one
/// with the name, no `source`, and the version `own`, so a same-name path
/// crate keeps its version in the comparison.
fn masked_lock(text: &str, name: &str, own: &str) -> Option<toml::Table> {
    let mut value: toml::Table = toml::from_str(text).ok()?;
    let qualified = format!("{name} {own}");
    let packages = value.get_mut("package")?.as_array_mut()?;
    for package in packages.iter_mut() {
        let package = package.as_table_mut()?;
        let is_own = package.get("name").and_then(toml::Value::as_str) == Some(name)
            && package.get("version").and_then(toml::Value::as_str) == Some(own)
            && !package.contains_key("source");
        if is_own {
            package.remove("version");
        }
        if let Some(dependencies) = package
            .get_mut("dependencies")
            .and_then(toml::Value::as_array_mut)
        {
            for reference in dependencies.iter_mut() {
                if reference.as_str() == Some(qualified.as_str()) {
                    *reference = toml::Value::String(name.to_owned());
                }
            }
        }
    }
    Some(value)
}

/// `uv.lock` with the project's own entry reduced to its name: the entry
/// whose normalized name is the project's, whose source is the project
/// itself, editable or virtual, and whose version is `own`. Every other
/// entry, a dependency of the same name from an index included, keeps its
/// version in the comparison.
fn masked_uv_lock(text: &str, name: &str, own: &str) -> Option<toml::Table> {
    let normalized = |raw: &str| -> String {
        let mut out = String::new();
        for c in raw.chars() {
            if matches!(c, '-' | '_' | '.') {
                if !out.ends_with('-') {
                    out.push('-');
                }
            } else {
                out.push(c.to_ascii_lowercase());
            }
        }
        out
    };
    let wanted = normalized(name);
    let mut value: toml::Table = toml::from_str(text).ok()?;
    let packages = value.get_mut("package")?.as_array_mut()?;
    for package in packages.iter_mut() {
        let package = package.as_table_mut()?;
        let local = package
            .get("source")
            .and_then(toml::Value::as_table)
            .is_some_and(|source| {
                source.contains_key("editable") || source.contains_key("virtual")
            });
        let is_own = local
            && package
                .get("name")
                .and_then(toml::Value::as_str)
                .is_some_and(|found| normalized(found) == wanted)
            && package.get("version").and_then(toml::Value::as_str) == Some(own);
        if is_own {
            package.remove("version");
        }
    }
    Some(value)
}

/// The commits `tip` holds after `base` that change more than the version,
/// one `<short sha> <subject>` each, oldest first: the commits no candidate
/// tested. A commit that writes only the package's own version, in the
/// version file and in the rust or uv lockfile, changes no tested code.
fn untested_commits(
    context: &MintContext<'_>,
    base: &str,
    tip: &str,
) -> Result<Vec<String>, RkError> {
    let MintContext { target, driver, .. } = context;
    let refuse = |detail: String| {
        RkError::refusal(
            Diagnostic::new(Reason::StateDrift, detail)
                .expected("a checkout that holds the line and its candidate's commit")
                .action("fetch the line and its tags, then promote again")
                .target_state("unchanged"),
        )
    };
    let range = format!("{base}..{tip}");
    let listed = git(target, &["rev-list", "--reverse", &range, "--"])?;
    if !listed.status.success() {
        return Err(refuse(last_line(&listed.stderr)));
    }
    let name = adapter::package_name(target.as_std_path(), driver).unwrap_or_default();
    let mut untested = Vec::new();
    for commit in String::from_utf8_lossy(&listed.stdout).lines() {
        if !version_only(context, &name, commit)? {
            let subject = git(target, &["log", "-1", "--format=%h %s", commit])?;
            untested.push(String::from_utf8_lossy(&subject.stdout).trim().to_owned());
        }
    }
    Ok(untested)
}

/// Whether one commit changes only the package's own version.
fn version_only(context: &MintContext<'_>, name: &str, commit: &str) -> Result<bool, RkError> {
    let MintContext {
        target,
        driver,
        file,
        ..
    } = context;
    let parent = format!("{commit}^1");
    let changed = git(target, &["diff", "--name-only", &parent, commit, "--"])?;
    if !changed.status.success() {
        return Ok(false);
    }
    let own_at = |rev: &str| -> Option<String> {
        let text = show(target, rev, file)?;
        adapter::version_in(driver, &text)
    };
    for path in String::from_utf8_lossy(&changed.stdout).lines() {
        let lockfile = (*driver == "rust" && path == "Cargo.lock")
            || (*driver == "python" && path == "uv.lock");
        if path != *file && !lockfile {
            return Ok(false);
        }
        let (Some(before), Some(after)) = (show(target, &parent, path), show(target, commit, path))
        else {
            return Ok(false);
        };
        let (own_before, own_after) = if lockfile {
            match (own_at(&parent), own_at(commit)) {
                (Some(before), Some(after)) => (before, after),
                _ => return Ok(false),
            }
        } else {
            (String::new(), String::new())
        };
        let sides = Sides {
            before: &before,
            after: &after,
            own_before: &own_before,
            own_after: &own_after,
        };
        if !same_but_version(driver, path, name, &sides) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// One file's content at a revision, or `None` where it does not exist.
fn show(target: &Utf8Path, rev: &str, path: &str) -> Option<String> {
    let spec = format!("{rev}:{path}");
    let output = git(target, &["show", &spec]).ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Run one git command that must succeed, its failure typed.
fn run_git(target: &Utf8Path, args: &[&str], expected: &str) -> Result<(), RkError> {
    let output = git(target, args)?;
    if output.status.success() {
        return Ok(());
    }
    Err(RkError::subprocess(
        Diagnostic::new(Reason::SubprocessFailed, last_line(&output.stderr))
            .expected(expected.to_owned())
            .target_state("the version file is written and not pushed"),
    ))
}

/// What `rc` says where a line carries no candidate yet: the mint verb on
/// a pair whose release pipeline carries the candidate job, the rust,
/// bash, and python pairs, and the plain gap on any other driver.
fn empty_candidate_message(target: &Utf8Path) -> &'static str {
    let driver = crate::landing::manifest::load(target)
        .ok()
        .flatten()
        .and_then(|manifest| manifest.profile.release.driver);
    match driver.as_deref() {
        Some("rust" | "bash" | "python") => {
            "no candidate is tagged on the line yet; rk lines mint previews the first one, and the release workflow's candidate job mints it"
        }
        Some(_) => {
            "no candidate is tagged on the line; release-kit lands no candidate job for this driver, so the line's release request stays the human gate"
        }
        None => {
            "no candidate is tagged on the line yet; the target records no release driver, so rk binding <tech> names its candidate path"
        }
    }
}

/// The machine form of `rk lines retire`.
#[derive(Debug, Serialize)]
struct RetireReport {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// The branch the verb acted on.
    branch: String,
    /// The seat that stood — removed under apply — where one existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    seat: Option<String>,
    /// What stays the operator's, in order.
    next: Vec<String>,
}

fn retire(line: &str, target: &Utf8Path, apply: bool, out: Output) -> Result<(), RkError> {
    let branch = line_branch(line, &crate::config::line_prefix_of(target.as_std_path())?)?;
    if !branch_exists(target, &branch)? {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::TargetNotFound,
                format!("no local {branch} to retire"),
            )
            .expected("a local release line")
            .action(format!(
                "the remote half stays yours either way: git push origin --delete {branch}"
            ))
            .target_state("unchanged"),
        ));
    }
    // The tag gate: every commit the line holds beyond the trunk must be
    // reachable from a tag, or the deletion garbage-collects the line.
    let uncovered = uncovered_commits(target, &branch)?;
    if !uncovered.is_empty() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::DestructiveRefusal,
                format!(
                    "{branch} holds {} commit(s) no tag reaches, {} first",
                    uncovered.len(),
                    uncovered[0]
                ),
            )
            .expected("every line-only commit reachable from a tag")
            .action("tag what the line still owes — the release automation mints tags — or accept losing the commits is not offered")
            .target_state("unchanged"),
        ));
    }
    let tip = resolve_commit(target, &branch)?;
    let seats = inventory(target)?;
    let seat = seats
        .iter()
        .find(|seat| seat.branch.as_deref() == Some(branch.as_str()))
        .map(|seat| seat.path.clone());
    if !apply {
        if let Some(path) = &seat {
            out.result_line(format!(
                "would remove the seat {path}, then delete {branch}"
            ));
        } else {
            out.result_line(format!("would delete {branch} ({tip})"));
        }
        let next = vec![format!("rk lines retire {line} --target {target} --apply")];
        out.next(&next);
        return out.emit(&RetireReport {
            schema: "rk.lines-retire/1",
            mode: "preview",
            branch,
            seat: seat.map(|path| path.to_string()),
            next,
        });
    }
    // Seat before branch: a worktree holds the checkout, so the branch
    // deletion below would otherwise refuse — and git itself refuses a
    // dirty or locked seat, which is the guard this verb wants.
    if let Some(path) = &seat {
        let removed = git(target, &["worktree", "remove", path.as_str()])?;
        if !removed.status.success() {
            return Err(RkError::refusal(
                Diagnostic::new(Reason::DestructiveRefusal, last_line(&removed.stderr))
                    .expected("a clean, unlocked seat")
                    .action(format!(
                        "resolve what the seat holds, then rerun; the branch {branch} survives"
                    ))
                    .target_state("unchanged"),
            ));
        }
        out.result_line(format!("removed the seat {path}"));
    }
    match maintenance::delete_branch(target, &branch, &tip) {
        Deletion::Deleted => out.result_line(format!("deleted {branch} ({tip})")),
        Deletion::ConfigSurvived { detail } => {
            out.result_line(format!("deleted {branch} ({tip}); {detail}"));
        }
        Deletion::Refused { detail } => {
            return Err(RkError::refusal(
                Diagnostic::new(Reason::StateDrift, detail)
                    .expected("a tip that did not move after verification")
                    .target_state("the branch survives"),
            ));
        }
    }
    let next = vec![format!(
        "git push origin --delete {branch} retires the remote half; the tags keep the line recoverable"
    )];
    out.next(&next);
    out.emit(&RetireReport {
        schema: "rk.lines-retire/1",
        mode: "apply",
        branch,
        seat: seat.map(|path| path.to_string()),
        next,
    })
}

/// `<prefix><line>` for a well-formed `<major>.<minor>`.
fn line_branch(line: &str, prefix: &str) -> Result<String, RkError> {
    let well_formed = line.split_once('.').is_some_and(|(major, minor)| {
        !major.is_empty()
            && !minor.is_empty()
            && major.bytes().all(|b| b.is_ascii_digit())
            && minor.bytes().all(|b| b.is_ascii_digit())
    });
    if !well_formed {
        return Err(RkError::Usage(format!(
            "'{line}' is not a line; a line is <major>.<minor>, as in 1.1"
        )));
    }
    Ok(format!("{prefix}{line}"))
}

/// The short names under one ref prefix.
fn ref_names(target: &Utf8Path, prefix: &str) -> Result<Vec<String>, RkError> {
    let output = git(
        target,
        &["for-each-ref", "--format=%(refname:short)", prefix],
    )?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, last_line(&output.stderr))
                .expected("a git repository at the target"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// The newest `v<line>.*` tag, candidates or releases.
fn newest_tag(target: &Utf8Path, line: &str, candidates: bool) -> Result<Option<String>, RkError> {
    let pattern = format!("v{line}.*");
    let output = git(target, &["tag", "-l", &pattern, "--sort=-v:refname"])?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, last_line(&output.stderr))
                .expected("a git repository at the target"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|tag| tag.contains("-rc.") == candidates)
        .map(str::to_owned))
}

/// The commits the branch holds that neither the line's own tags nor a
/// trunk ref reaches. The negation is the line's `v<line>.*` pattern, not
/// every tag: an unrelated tag that happens to reach the tip is not what
/// makes a retirement safe, per
/// `maintenance:a-line-is-never-retired-before-its-tags`.
fn uncovered_commits(target: &Utf8Path, branch: &str) -> Result<Vec<String>, RkError> {
    let prefix = crate::config::line_prefix_of(target.as_std_path())?;
    let trunk = crate::config::trunk_of(target.as_std_path())?;
    let line = branch.strip_prefix(&prefix).unwrap_or(branch);
    let mut args = vec![
        "rev-list".to_owned(),
        branch.to_owned(),
        "--not".to_owned(),
        format!("--tags=v{line}.*"),
    ];
    for reference in [trunk.clone(), format!("origin/{trunk}")] {
        if resolve_commit(target, &reference).is_ok() {
            args.push(reference);
        }
    }
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = git(target, &arg_refs)?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::StateDrift, last_line(&output.stderr))
                .expected("a readable branch history"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect())
}

/// Whether a local branch exists.
fn branch_exists(target: &Utf8Path, branch: &str) -> Result<bool, RkError> {
    let ref_name = format!("refs/heads/{branch}");
    Ok(
        git(target, &["show-ref", "--verify", "--quiet", &ref_name])?
            .status
            .success(),
    )
}

/// Whether the origin remote tracks the branch.
fn remote_exists(target: &Utf8Path, branch: &str) -> Result<bool, RkError> {
    let ref_name = format!("refs/remotes/origin/{branch}");
    Ok(
        git(target, &["show-ref", "--verify", "--quiet", &ref_name])?
            .status
            .success(),
    )
}

/// One commit-ish resolved to its commit, or a refusal naming it.
fn resolve_commit(target: &Utf8Path, name: &str) -> Result<String, RkError> {
    let spec = format!("{name}^{{commit}}");
    let output = git(target, &["rev-parse", "--verify", "--quiet", &spec])?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(
                Reason::Usage,
                format!("'{name}' does not resolve to a commit"),
            )
            .expected("a base git can resolve — fetch the tags first")
            .target_state("unchanged"),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The worktree inventory, for the seat lookups.
fn inventory(target: &Utf8Path) -> Result<Vec<Worktree>, RkError> {
    let output = git(target, &["worktree", "list", "--porcelain", "-z"])?;
    if !output.status.success() {
        return Err(RkError::refusal(
            Diagnostic::new(Reason::TargetNotFound, last_line(&output.stderr))
                .expected("a git repository at the target"),
        ));
    }
    parse_worktrees(&output.stdout).map_err(|detail| {
        RkError::refusal(
            Diagnostic::new(Reason::StateDrift, detail).expected("a parseable worktree inventory"),
        )
    })
}

/// Run one git command against the target, spawn failure typed.
fn git(target: &Utf8Path, args: &[&str]) -> Result<std::process::Output, RkError> {
    let mut command = std::process::Command::new(crate::probes::git_bin());
    for var in maintenance::GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .arg("-C")
        .arg(target.as_std_path())
        .args(args)
        .output()
        .map_err(|source| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("git did not run: {source}"),
                )
                .expected("git installed and on PATH"),
            )
        })
}

/// The last non-empty stderr line, for a one-line detail.
fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("git reported no detail")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{MintDecision, Version, decide_mint};

    fn version(text: &str) -> Version {
        Version::parse(text).expect("a version this verb reads")
    }

    #[test]
    fn the_base_is_the_next_patch_of_the_line() {
        let tags = [version("1.4.0"), version("1.4.2"), version("1.5.0")];
        assert_eq!(
            decide_mint((1, 4), &tags, Some(version("1.4.2"))),
            Ok(MintDecision::Mint(version("1.4.3-rc.1")))
        );
        // A line that has released nothing prepares its .0.
        assert_eq!(
            decide_mint((2, 0), &tags, Some(version("1.9.4"))),
            Ok(MintDecision::Mint(version("2.0.0-rc.1")))
        );
    }

    #[test]
    fn the_number_follows_the_remote_candidates_of_the_base() {
        let tags = [
            version("1.4.2"),
            version("1.4.2-rc.7"),
            version("1.4.3-rc.1"),
            version("1.4.3-rc.2"),
        ];
        assert_eq!(
            decide_mint((1, 4), &tags, Some(version("1.4.3-rc.2"))),
            Ok(MintDecision::Mint(version("1.4.3-rc.3")))
        );
        // After the promotion released 1.4.3, the next base is 1.4.4.
        let promoted = [version("1.4.3-rc.2"), version("1.4.3")];
        assert_eq!(
            decide_mint((1, 4), &promoted, Some(version("1.4.3"))),
            Ok(MintDecision::Mint(version("1.4.4-rc.1")))
        );
    }

    #[test]
    fn an_untagged_candidate_of_the_base_is_pending() {
        let tags = [version("1.4.2"), version("1.4.3-rc.1")];
        assert_eq!(
            decide_mint((1, 4), &tags, Some(version("1.4.3-rc.2"))),
            Ok(MintDecision::Pending(version("1.4.3-rc.2")))
        );
        // An untagged candidate of a base the line already released is not
        // pending: publishing it would follow its own stable release.
        let released = [version("1.4.3")];
        assert!(decide_mint((1, 4), &released, Some(version("1.4.3-rc.2"))).is_err());
    }
}
