//! `rk release request`: keep the one release request of the pushed base.
//!
//! It runs on a push to the trunk or to a release line. It reads the
//! version through the adapter and proposes the next one. After a release
//! it proposes one only where the release packs a different set of files
//! than at the release tag or a file it packs changed since, and git-cliff
//! reads only the commits that touch those files. Where nothing remains to release it closes a stale request, and
//! otherwise it writes the version and the changelog on
//! `rk-release/<base>`, force-pushes it as the bot, and opens or edits the
//! request. It never arms or merges it: a release waits for the operator's
//! order.

use camino::Utf8Path;
use serde::Serialize;

use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

use super::adapter::Version;
use super::forge::Client;
use super::guard::{self, Proposal};
use super::packed::{self, Comparison};
use super::{Context, Done, cliff, eligible};

/// The report's shape version.
pub const SCHEMA: &str = "rk.release-request/2";

/// The machine form of `rk release request`.
#[derive(Debug, Serialize)]
pub struct RequestReport {
    /// The shape version of this document.
    pub schema: &'static str,
    /// `opened`, `refreshed`, `closed`, `nothing`, or `skip`.
    pub state: &'static str,
    /// Why nothing was proposed, where nothing was.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// The base the request merges into.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// The version the base carries, in the driver's spelling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The last release tag the packed files were compared with, where
    /// the request compared them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compared: Option<String>,
    /// How many files the release packs at `HEAD` or at that tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packed: Option<usize>,
    /// The version the request proposes, in the driver's spelling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    /// The tag the proposal releases as.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// The request's number on the forge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<u64>,
    /// Every file the request commit changed.
    pub files: Vec<String>,
}

impl RequestReport {
    const fn empty(state: &'static str) -> Self {
        Self {
            schema: SCHEMA,
            state,
            reason: None,
            base: None,
            version: None,
            compared: None,
            packed: None,
            proposal: None,
            tag: None,
            request: None,
            files: Vec::new(),
        }
    }
}

/// Keep the release request of the pushed base.
///
/// # Errors
///
/// A refusal of the default CI token, of git-cliff off its pin, or of a
/// prerelease proposal; a forge or a push failure.
pub fn run(target: &Utf8Path) -> Result<Done<RequestReport>, RkError> {
    let context = Context::load(target)?;
    refuse_default_token(&context)?;
    context.require_tools("request")?;
    let Some(base) = context
        .pushed_branch()
        .filter(|branch| context.is_base(branch))
    else {
        let mut report = RequestReport::empty("skip");
        report.reason = Some("the push is to no release base".to_owned());
        return Ok(Done {
            lines: vec!["skip: the push is to neither the trunk nor a release line".to_owned()],
            report,
            next: Vec::new(),
        });
    };
    cliff::require_pin(target)?;
    let client = Client::resolve(context.forge, target, &context.repo)?;
    let current = context.adapter.read_version(target)?;
    let branch = super::request_branch(&base);
    let (proposal, comparison) = decide(&context, &client, target, current)?;
    let mut report = RequestReport::empty("nothing");
    report.base = Some(base.clone());
    report.version = Some(current.spelled(context.driver()));
    let compared = record_comparison(&mut report, comparison.as_ref());
    let paths = comparison
        .map(|comparison| comparison.history)
        .unwrap_or_default();
    let next = match proposal {
        Proposal::Refuse(reason) => {
            return Err(RkError::refusal(
                Diagnostic::new(Reason::StateDrift, reason)
                    .expected("a final version for the request to propose")
                    .action("rk lines mint numbers a candidate; the request proposes final versions alone")
                    .target_state("unchanged"),
            ));
        }
        Proposal::Nothing(reason) => {
            let mut done = close_stale(&context, &client, &base, report, reason)?;
            done.lines.splice(0..0, compared);
            return Ok(done);
        }
        Proposal::Propose(next) => next,
    };
    let tag = context.adapter.tag_of(next);
    let files = commit_request(&context, &client, next, &tag, &branch, &paths)?;
    let title = format!("chore(release): {tag}");
    let body = super::forge_text(
        "blocks/release-request-body.md.in",
        &[("RK_TAG", &tag), ("RK_BASE", &base)],
    )?;
    let (number, state) = match client.open_request(&base, &branch)? {
        Some(number) => {
            client.edit_request(number, &title, &body)?;
            (number, "refreshed")
        }
        None => (
            client.create_request(&base, &branch, &title, &body)?,
            "opened",
        ),
    };
    // SATISFIES release-engine:the-request-never-arms
    let mut lines = compared;
    lines.push(format!(
        "{state} release request #{number} proposing {} on {branch} into {base}",
        next.spelled(context.driver())
    ));
    lines.push("not armed: the request waits for the operator's order to merge".to_owned());
    report.state = state;
    report.proposal = Some(next.spelled(context.driver()));
    report.tag = Some(tag);
    report.request = Some(number);
    report.files = files;
    Ok(Done {
        report,
        lines,
        next: Vec::new(),
    })
}

/// Refuse where the push credential is the forge's default CI token.
fn refuse_default_token(context: &Context) -> Result<(), RkError> {
    let Some(var) = super::default_ci_token(context.forge, |name| std::env::var(name).ok()) else {
        return Ok(());
    };
    Err(RkError::refusal(
        Diagnostic::new(
            Reason::DefaultCiToken,
            format!(
                "the push credential is {var}, the forge's default CI token, and a push with it starts no pipeline"
            ),
        )
        .expected(match context.forge {
            crate::detect::Forge::Github => "GH_TOKEN set to the release App's token",
            crate::detect::Forge::Gitlab => "GITLAB_TOKEN set from RELEASE_BOT_TOKEN",
        })
        .target_state("unchanged; nothing was pushed"),
    ))
}

/// Put the comparison of the packed files into the report, and answer the
/// human line that states it.
fn record_comparison(report: &mut RequestReport, comparison: Option<&Comparison>) -> Vec<String> {
    let Some(comparison) = comparison else {
        return Vec::new();
    };
    report.compared = Some(comparison.tag.clone());
    report.packed = Some(comparison.paths.len());
    vec![format!(
        "compared the {} files the release packs with {}",
        comparison.paths.len(),
        comparison.tag
    )]
}

/// Whether the forge holds a published release of `version`: a release
/// that is not a draft. A tag alone is no release, because a failed
/// release save on GitLab leaves its tag behind, and a GitHub draft is
/// unpublished. A read that failed refuses, because a release nobody read
/// may exist.
fn released(context: &Context, client: &Client, version: Version) -> Result<bool, RkError> {
    // SATISFIES release-engine:the-request-never-numbers-a-candidate
    Ok(client
        .release(&context.adapter.tag_of(version))?
        .is_some_and(|release| !release.draft))
}

/// The proposal for the version the base carries, with the comparison of
/// the packed files where the version already released.
///
/// SATISFIES release-engine:a-request-follows-the-packed-files
/// SATISFIES release-engine:a-request-needs-a-releasing-commit
///
/// A released version proposes only where the packed set or a packed file
/// differs from its tag and a commit that touched the packed set since is
/// a releasing commit. git-cliff then reads only the commits that touch
/// the packed files, and the configuration where its declaration changed
/// the set. A version with no release is judged the same way since the
/// last stable release tag, and keeps its declared number once that holds.
/// With no stable release tag at all, the first release proposes only
/// where a commit of the whole history that touched the packed files at
/// `HEAD` is a releasing commit, and then as it always did.
fn decide(
    context: &Context,
    client: &Client,
    target: &Utf8Path,
    current: Version,
) -> Result<(Proposal, Option<Comparison>), RkError> {
    if current.is_candidate() {
        return Ok((guard::propose(current, false, None), None));
    }
    let was_released = released(context, client, current)?;
    let tag = context.adapter.tag_of(current);
    // A version with no release, set by hand past an earlier release or
    // promoted from a candidate, is judged since the last stable release,
    // and keeps its declared version once that judgment holds.
    let since = if was_released {
        Some(tag)
    } else {
        packed::stable_tag_below(target, Some(&tag))?
    };
    let (verdict, comparison) = eligible::engine(context, since.as_deref())?;
    if !verdict.eligible {
        return Ok((Proposal::Nothing(verdict.reason), comparison));
    }
    let answer = if guard::needs_cliff(current, was_released) {
        let paths = comparison
            .as_ref()
            .map(|compared| compared.history.clone())
            .unwrap_or_default();
        Some(cliff::bumped(target, &paths)?)
    } else {
        None
    };
    let proposal = guard::propose(current, was_released, answer.as_deref());
    if let Proposal::Propose(next) = proposal
        && next != current
        && released(context, client, next)?
    {
        return Ok((
            Proposal::Nothing(format!("{} is already released", next.tag())),
            comparison,
        ));
    }
    Ok((proposal, comparison))
}

/// Close the open request of `base`, where one is open, and delete its
/// branch: nothing remains for it to release.
fn close_stale(
    context: &Context,
    client: &Client,
    base: &str,
    mut report: RequestReport,
    reason: String,
) -> Result<Done<RequestReport>, RkError> {
    let branch = super::request_branch(base);
    let mut lines = vec![format!("nothing to release: {reason}")];
    if let Some(number) = client.open_request(base, &branch)? {
        client.close_request(number)?;
        let held = super::git_ok(
            &context.target,
            &["ls-remote", "--heads", "origin", &branch],
            "an origin remote git can read",
        )?;
        if !held.trim().is_empty() {
            super::push(context, &["--delete", &branch])?;
        }
        lines.push(format!(
            "closed the stale release request #{number} and deleted {branch}"
        ));
        report.state = "closed";
        report.request = Some(number);
    }
    report.reason = Some(reason);
    Ok(Done {
        report,
        lines,
        next: Vec::new(),
    })
}

/// Write the version and the changelog on a fresh `branch` from the pushed
/// tip, commit them as the bot, force-push, and return the checkout to
/// where it stood.
fn commit_request(
    context: &Context,
    client: &Client,
    next: Version,
    tag: &str,
    branch: &str,
    paths: &[String],
) -> Result<Vec<String>, RkError> {
    let target = &context.target;
    let original = super::git(target, &["symbolic-ref", "--short", "-q", "HEAD"])?;
    let original = if original.status.success() {
        String::from_utf8_lossy(&original.stdout).trim().to_owned()
    } else {
        context.head()?
    };
    super::git_ok(
        target,
        &["checkout", "-q", "-B", branch],
        "a fresh request branch from the pushed tip",
    )?;
    let committed = (|| {
        let mut files: Vec<String> = context
            .adapter
            .write_version(target, next)?
            .into_iter()
            .map(String::from)
            .collect();
        cliff::changelog(target, tag, paths)?;
        files.push("CHANGELOG.md".to_owned());
        // SATISFIES bot-identity:the-bot-identity-reaches-one-project
        let (name, email) = client.identity()?;
        let mut add = vec!["add", "--"];
        add.extend(files.iter().map(String::as_str));
        super::git_ok(target, &add, "the release files staged")?;
        let user = format!("user.name={name}");
        let mail = format!("user.email={email}");
        let message = format!("chore(release): {tag}");
        super::git_ok(
            target,
            &[
                "-c",
                &user,
                "-c",
                &mail,
                "commit",
                "-q",
                "--allow-empty",
                "-m",
                &message,
            ],
            "the release commit",
        )?;
        // SATISFIES release-engine:one-request-per-base
        let refspec = format!("HEAD:refs/heads/{branch}");
        super::push(context, &["--force", &refspec])?;
        Ok::<_, RkError>(files)
    })();
    let restored = super::git_ok(
        target,
        &["checkout", "-q", "-f", &original],
        "the checkout returned to the pushed tip",
    );
    let files = committed?;
    restored?;
    Ok(files)
}
