//! Starting work from a forge issue: the pure half.
//!
//! An issue becomes a branch name at the forge, never in this binary's
//! imagination. GitHub names the branch server-side, so nothing here
//! renders one. GitLab exposes no such endpoint — its branch-name
//! template is a project setting the web UI applies — so rk reproduces
//! `Issue.to_branch_name` exactly, and this module is where that
//! reproduction lives. Spawning stays in the handler, exactly as
//! [`crate::branches`] declares for the branch half.

use std::path::Path;
use std::process::Output;

use serde_json::Value;

use crate::detect::{Detection, Forge};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;

/// GitLab's documented default when a project sets no template.
///
/// Kept for the report and the prose. The rendering follows the code path
/// GitLab takes when the template is absent, which joins the present
/// values rather than substituting this string.
pub const GITLAB_DEFAULT_TEMPLATE: &str = "%{id}-%{title}";

/// The longest branch name GitLab renders from an issue, in characters.
const GITLAB_NAME_CAP: usize = 100;

/// What the operator named, and where it points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The issue number, as the forge counts it: a GitHub number or a
    /// GitLab iid.
    pub number: u64,
    /// The project path the reference names, where it carried one.
    pub repo: Option<String>,
    /// The host the reference names, where it carried one.
    pub host: Option<String>,
}

/// The forms a reference is accepted in, for a refusal message.
const ACCEPTED_FORMS: &str = "a number, #<number>, or the forge's issue URL (…/issues/<number> on GitHub, …/-/issues/<number> on GitLab)";

/// Parse what the operator typed into the issue it names.
///
/// A bare number names no project, so it always agrees with the clone. A
/// URL names both, and [`agrees`] is what holds it to the clone the verb
/// was pointed at.
///
/// # Errors
///
/// A string matching none of the accepted forms, or naming issue zero:
/// no forge numbers an issue zero, and a zero is what a failed parse
/// produces.
pub fn parse_reference(text: &str) -> Result<Reference, String> {
    let text = text.trim();
    let bare = text.strip_prefix('#').unwrap_or(text);
    if !bare.is_empty() && bare.chars().all(|c| c.is_ascii_digit()) {
        let number = bare
            .parse()
            .map_err(|_| format!("'{text}' is not an issue number this forge can carry"))?;
        return numbered(number, None, None, text);
    }
    let Some((host, path)) = crate::detect::split_remote(text) else {
        return Err(format!(
            "'{text}' is not an issue reference; pass {ACCEPTED_FORMS}"
        ));
    };
    // A query or a fragment is not part of the path the forge routes on.
    let path = path
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/');
    // GitLab nests projects under groups, so every segment before the
    // separator belongs to the project path.
    let split = path
        .rsplit_once("/-/issues/")
        .or_else(|| path.rsplit_once("/issues/"));
    let Some((repo, tail)) = split else {
        return Err(format!("'{text}' names no issue; pass {ACCEPTED_FORMS}"));
    };
    let number = tail
        .split('/')
        .next()
        .unwrap_or_default()
        .parse()
        .map_err(|_| format!("'{text}' names no issue number; pass {ACCEPTED_FORMS}"))?;
    numbered(number, Some(repo.to_owned()), Some(host), text)
}

/// One parsed reference, refusing issue zero.
fn numbered(
    number: u64,
    repo: Option<String>,
    host: Option<String>,
    text: &str,
) -> Result<Reference, String> {
    if number == 0 {
        return Err(format!("'{text}' names issue 0, which no forge carries"));
    }
    Ok(Reference { number, repo, host })
}

/// Whether a reference names the clone the verb was pointed at.
///
/// The ordinary mistake this guards is an agent pasting a URL while
/// sitting in another checkout: the branch would be minted on one project
/// and seated in another.
///
/// # Errors
///
/// A reference whose project path or host disagrees with the detected
/// remote.
pub fn agrees(reference: &Reference, detected: &Detection) -> Result<(), String> {
    if let (Some(named), Some(found)) = (reference.host.as_deref(), detected.host.as_deref()) {
        if !named.eq_ignore_ascii_case(found) {
            return Err(format!(
                "the reference names {named} and this clone's origin is {found}"
            ));
        }
    }
    if let (Some(named), Some(found)) = (reference.repo.as_deref(), detected.repo.as_deref()) {
        if named != found {
            return Err(format!(
                "the reference names {named} and this clone's origin is {found}"
            ));
        }
    }
    Ok(())
}

/// A rendered branch name, and whether any character fell outside the
/// transliteration table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// The name as rendered.
    pub name: String,
    /// Whether a character was replaced rather than approximated, so the
    /// name can differ from the one GitLab's own button produces.
    pub approximated: bool,
}

/// Render a GitLab branch name for one issue, the way GitLab renders it.
///
/// This reproduces `Issue.to_branch_name`. The order is the source's own:
/// a confidential issue keeps its title out of the branch and ignores the
/// template entirely, the three template parameters are parameterized
/// first, an absent template joins the present values, an unresolved
/// placeholder stays in the text, and a name over 100 characters is cut
/// and loses its trailing partial segment.
#[must_use]
pub fn gitlab_branch_name(
    iid: u64,
    title: &str,
    confidential: bool,
    template: Option<&str>,
    branch_creator: Option<&str>,
) -> Rendered {
    // GitLab renders this before anything else, so a branch name never
    // leaks a confidential title, and the template never applies.
    if confidential {
        return cap(format!("{iid}-confidential-issue"), false);
    }
    let id = parameterize_reporting(&iid.to_string(), true);
    let title = parameterize_reporting(title, false);
    let creator = branch_creator.map(|name| parameterize_reporting(name, true));
    let approximated =
        id.approximated || title.approximated || creator.as_ref().is_some_and(|c| c.approximated);
    let name = match template.filter(|text| !text.trim().is_empty()) {
        None => [id.name, title.name]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("-"),
        Some(template) => substitute(
            template,
            &id.name,
            &title.name,
            creator.as_ref().map_or("", |c| c.name.as_str()),
        ),
    };
    cap(name, approximated)
}

/// Replace every placeholder the template names.
///
/// A placeholder resolving to nothing is left in the text unchanged,
/// which is what `Gitlab::StringPlaceholderReplacer` does, and an unknown
/// placeholder is left for the same reason. Neither is an error here: the
/// grammar check one step later refuses the name and names the template.
fn substitute(template: &str, id: &str, title: &str, creator: &str) -> String {
    let mut name = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find("%{") {
        name.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find('}') else {
            rest = &rest[open..];
            break;
        };
        let key = &after[..close];
        let value = match key {
            "id" => id,
            "title" => title,
            "branch_creator" => creator,
            _ => "",
        };
        if value.is_empty() {
            name.push_str(&rest[open..=(open + 2 + close)]);
        } else {
            name.push_str(value);
        }
        rest = &after[close + 1..];
    }
    name.push_str(rest);
    name
}

/// Cut a name over the cap and drop the trailing partial segment, which
/// is `sub(/-[^-]*\Z/, '')` in the source. A capped name carrying no `-`
/// is left as the cut produced it, because that substitution matches
/// nothing.
fn cap(name: String, approximated: bool) -> Rendered {
    if name.chars().count() <= GITLAB_NAME_CAP {
        return Rendered { name, approximated };
    }
    let cut: String = name.chars().take(GITLAB_NAME_CAP).collect();
    let name = cut
        .rfind('-')
        .map_or_else(|| cut.clone(), |at| cut[..at].to_owned());
    Rendered { name, approximated }
}

/// Rails `String#parameterize`, the one GitLab calls.
///
/// In order: transliterate to an ASCII approximation, replace every run
/// of characters outside `[A-Za-z0-9_-]` with `-`, squeeze repeated
/// separators into one, drop a leading and a trailing separator, and
/// downcase unless the case is preserved.
#[must_use]
pub fn parameterize(text: &str, preserve_case: bool) -> String {
    parameterize_reporting(text, preserve_case).name
}

/// [`parameterize`], reporting whether the transliteration table covered
/// every character it was given.
#[must_use]
pub fn parameterize_reporting(text: &str, preserve_case: bool) -> Rendered {
    let mut approximated = false;
    let mut transliterated = String::with_capacity(text.len());
    for source in text.chars() {
        if source.is_ascii() {
            transliterated.push(source);
        } else if let Some(ascii) = transliterate(source) {
            transliterated.push_str(ascii);
        } else {
            // Rails transliterates an uncovered character to `?`, which
            // its own run replacement then turns into the separator. The
            // flag records that the table, not the source text, decided
            // it, so the report can say the name may differ.
            approximated = true;
            transliterated.push('?');
        }
    }
    // Every run outside [A-Za-z0-9_-] becomes one separator.
    let mut replaced = String::with_capacity(transliterated.len());
    let mut in_run = false;
    for held in transliterated.chars() {
        if held.is_ascii_alphanumeric() || matches!(held, '_' | '-') {
            replaced.push(held);
            in_run = false;
        } else if !in_run {
            replaced.push('-');
            in_run = true;
        }
    }
    // Repeated separators squeeze into one, and a leading and a trailing
    // separator go.
    let mut squeezed = String::with_capacity(replaced.len());
    let mut last_was_separator = false;
    for held in replaced.chars() {
        if held == '-' {
            if last_was_separator {
                continue;
            }
            last_was_separator = true;
        } else {
            last_was_separator = false;
        }
        squeezed.push(held);
    }
    let trimmed = squeezed.trim_matches('-');
    let name = if preserve_case {
        trimmed.to_owned()
    } else {
        trimmed.to_lowercase()
    };
    Rendered { name, approximated }
}

/// The ASCII approximation of one character, over Latin-1 Supplement and
/// Latin Extended-A. A character outside the table answers `None`.
fn transliterate(source: char) -> Option<&'static str> {
    let index = (source as u32).checked_sub(0x00C0)? as usize;
    TRANSLITERATIONS
        .get(index)
        .copied()
        .filter(|s| !s.is_empty())
}

/// Latin-1 Supplement and Latin Extended-A, from `U+00C0` upward, one
/// entry per code point. An empty entry is a code point the table does
/// not approximate.
const TRANSLITERATIONS: [&str; 192] = [
    // U+00C0..U+00FF
    "A", "A", "A", "A", "A", "A", "AE", "C", "E", "E", "E", "E", "I", "I", "I", "I", "D", "N", "O",
    "O", "O", "O", "O", "x", "O", "U", "U", "U", "U", "Y", "Th", "ss", "a", "a", "a", "a", "a",
    "a", "ae", "c", "e", "e", "e", "e", "i", "i", "i", "i", "d", "n", "o", "o", "o", "o", "o", "",
    "o", "u", "u", "u", "u", "y", "th", "y", // U+0100..U+017F
    "A", "a", "A", "a", "A", "a", "C", "c", "C", "c", "C", "c", "C", "c", "D", "d", "D", "d", "E",
    "e", "E", "e", "E", "e", "E", "e", "E", "e", "G", "g", "G", "g", "G", "g", "G", "g", "H", "h",
    "H", "h", "I", "i", "I", "i", "I", "i", "I", "i", "I", "i", "IJ", "ij", "J", "j", "K", "k",
    "k", "L", "l", "L", "l", "L", "l", "L", "l", "L", "l", "N", "n", "N", "n", "N", "n", "n", "NG",
    "ng", "O", "o", "O", "o", "O", "o", "OE", "oe", "R", "r", "R", "r", "R", "r", "S", "s", "S",
    "s", "S", "s", "S", "s", "T", "t", "T", "t", "T", "t", "U", "u", "U", "u", "U", "u", "U", "u",
    "U", "u", "U", "u", "W", "w", "Y", "y", "Y", "Z", "z", "Z", "z", "Z", "z", "s",
];

/// What the forge already carries for one issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Minted {
    /// The forge already carries a branch for this issue.
    Already {
        /// The branch the verb adopts.
        branch: String,
        /// Every other branch linked to the same issue.
        others: Vec<String>,
    },
    /// The forge carries none.
    Absent,
    /// The forge could not answer; nothing is created.
    Unknown {
        /// What the answer was, for the report.
        detail: String,
    },
}

/// Read a GitHub `linkedBranches` answer.
///
/// More than one node keeps one and reports the rest: an issue with two
/// linked branches is a state rk did not create, and picking from it
/// silently would look like a choice rk made. The one kept is the first
/// in sort order, which is a rule rather than whatever order the API
/// answered in.
///
/// Every node must carry a ref name, and the connection must be whole: a
/// node that does not, or a page the query did not reach, is unknown
/// rather than absent, because absence is what authorizes a mint.
#[must_use]
pub fn linked_branch(body: &Value) -> Minted {
    let nodes = body
        .pointer("/data/repository/issue/linkedBranches/nodes")
        .and_then(Value::as_array);
    let Some(nodes) = nodes else {
        return Minted::Unknown {
            detail: "the answer carries no linkedBranches list".to_owned(),
        };
    };
    // Complete only where the answer says so. A `hasNextPage` that is
    // missing, null, or not a boolean says nothing, and absence is what
    // authorizes a mint.
    match body
        .pointer("/data/repository/issue/linkedBranches/pageInfo/hasNextPage")
        .and_then(Value::as_bool)
    {
        Some(false) => {}
        Some(true) => {
            return Minted::Unknown {
                detail: format!(
                    "the issue links more than the {LINKED_BRANCH_PAGE} branches one read carries"
                ),
            };
        }
        None => {
            return Minted::Unknown {
                detail: "the answer does not say whether it carries every linked branch".to_owned(),
            };
        }
    }
    let mut names = Vec::with_capacity(nodes.len());
    for node in nodes {
        let Some(name) = node.pointer("/ref/name").and_then(Value::as_str) else {
            return Minted::Unknown {
                detail: "a linked branch carries no ref name".to_owned(),
            };
        };
        names.push(name.to_owned());
    }
    names.sort_unstable();
    if names.is_empty() {
        return Minted::Absent;
    }
    let branch = names.remove(0);
    Minted::Already {
        branch,
        others: names,
    }
}

/// How many linked branches one read carries. An issue with more is a
/// state this verb reports rather than guesses at.
const LINKED_BRANCH_PAGE: u32 = 100;

/// Whether the landed grammar admits a branch name.
///
/// A named re-export, so the call site reads as intent.
/// [`crate::worktree::matches_grammar`] stays the single owner.
#[must_use]
pub fn admissible(branch: &str) -> bool {
    crate::worktree::matches_grammar(branch)
}

/// What one issue resolves to, before anything is seated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The issue, as the forge numbers it.
    pub number: u64,
    /// The issue's title, for the report.
    pub title: String,
    /// The branch the seat takes.
    ///
    /// Absent for one case alone: a GitHub preview of an issue with no
    /// linked branch. The server names that branch at the moment it mints
    /// it, so no honest preview can print a name.
    pub branch: Option<String>,
    /// Where the name came from: `already` for one the forge carried,
    /// `forge` for a fresh mint, `pending` for a name the forge has not
    /// been asked to make yet.
    pub origin: &'static str,
    /// Other branches the forge links to the same issue, GitHub only.
    pub others: Vec<String>,
    /// A note the report prints: a template that was read, a title the
    /// transliteration table did not cover, a confidential issue.
    pub detail: Option<String>,
}

/// Resolve one issue to its branch, minting at the forge under `apply`.
///
/// Every failure returns before any local mutation, so a forge that
/// refuses, rate-limits, or answers nothing leaves the clone as it was.
///
/// # Errors
///
/// A forge call that does not run or does not answer, and — on GitLab —
/// a project template rendering a name the landed grammar refuses.
pub fn resolve(cli: &Path, target: &Path, ask: &Ask<'_>) -> Result<Resolved, RkError> {
    match ask.forge {
        Forge::Github => resolve_github(cli, target, ask),
        Forge::Gitlab => resolve_gitlab(cli, target, ask),
    }
}

/// What one call asks the forge for.
pub struct Ask<'a> {
    /// The forge to act on.
    pub forge: Forge,
    /// The project path.
    pub repo: &'a str,
    /// The issue, as the operator named it.
    pub reference: &'a Reference,
    /// The remote branch a new branch starts from.
    pub base: Option<&'a str>,
    /// Whether to write, at the forge and afterwards.
    pub apply: bool,
    /// Every local refusal the seat carries, run where the forge lets rk
    /// know the name before it writes.
    pub seatable: &'a dyn Fn(&str) -> Result<(), RkError>,
}

/// The GitHub path: one GraphQL read, a mint where nothing is linked, and
/// the same read again for the name the server chose.
fn resolve_github(cli: &Path, target: &Path, ask: &Ask<'_>) -> Result<Resolved, RkError> {
    let (repo, reference, base, apply) = (ask.repo, ask.reference, ask.base, ask.apply);
    let Some((owner, name)) = repo.split_once('/') else {
        return Err(RkError::Usage(format!(
            "'{repo}' is not a GitHub project path; pass --repo <owner/name>"
        )));
    };
    let number = reference.number.to_string();
    let read = || -> Result<Value, RkError> {
        let out = forge_call(
            cli,
            target,
            &[
                "api",
                "graphql",
                "-f",
                &format!("query={LINKED_BRANCHES_QUERY}"),
                "-F",
                &format!("owner={owner}"),
                "-F",
                &format!("name={name}"),
                // `number` is `Int!`, and only `-F` renders an integer as
                // a JSON number; `-f` would send the string "57".
                "-F",
                &format!("number={number}"),
            ],
        )?;
        answered(&out, "the issue read")
    };
    let body = read()?;
    let title = body
        .pointer("/data/repository/issue/title")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    match linked_branch(&body) {
        Minted::Already { branch, others } => Ok(Resolved {
            number: reference.number,
            title,
            branch: Some(branch),
            origin: "already",
            others,
            detail: None,
        }),
        Minted::Unknown { detail } => Err(forge_failure(format!(
            "the issue read did not answer with linked branches: {detail}"
        ))),
        Minted::Absent if !apply => Ok(Resolved {
            number: reference.number,
            title,
            branch: None,
            origin: "pending",
            others: Vec::new(),
            detail: Some(
                "GitHub names the branch when it mints it, so the exact name appears on the apply"
                    .to_owned(),
            ),
        }),
        Minted::Absent => {
            // No `--name`: an omitted name is a request for the forge's
            // own, which the mutation documents as the issue number and
            // title. No `--checkout` either: the seat belongs to the verb
            // and to the recorded mode.
            let mut args = vec!["issue", "develop", number.as_str(), "--repo", repo];
            if let Some(base) = base {
                args.push("--base");
                args.push(base);
            }
            succeeded(&forge_call(cli, target, &args)?, "the mint")?;
            // The read is the authority, not the line the mint printed,
            // and it is the same path the idempotent second run takes.
            let after = read()?;
            match linked_branch(&after) {
                Minted::Already { branch, others } => Ok(Resolved {
                    number: reference.number,
                    title,
                    branch: Some(branch),
                    origin: "forge",
                    others,
                    detail: None,
                }),
                _ => Err(forge_failure(
                    "the mint reported success and the issue still carries no linked branch"
                        .to_owned(),
                )),
            }
        }
    }
}

/// The read GitHub answers both before and after a mint.
const LINKED_BRANCHES_QUERY: &str = "query($owner: String!, $name: String!, $number: Int!) { repository(owner: $owner, name: $name) { issue(number: $number) { title linkedBranches(first: 100) { pageInfo { hasNextPage } nodes { ref { name } } } } } }";

/// What the GitLab reads settled, before the branch itself is looked at.
struct Planned {
    /// The issue's own iid, as GitLab counts it.
    iid: u64,
    /// The issue's title, for the report.
    title: String,
    /// The name GitLab's own rules produce.
    name: String,
    /// The project's default branch, the ref a mint starts from.
    default_branch: String,
    /// What the report says about how the name came out.
    detail: Option<String>,
}

/// The GitLab reads: the project, the issue, and — only where the template
/// names the creator — the user. The name is rendered and judged here, so
/// a template nobody can land through fails before any write.
fn plan_gitlab(
    cli: &Path,
    target: &Path,
    encoded: &str,
    reference: &Reference,
) -> Result<Planned, RkError> {
    let project = answered(
        &forge_call(cli, target, &["api", &format!("projects/{encoded}")])?,
        "the project read",
    )?;
    // A template the answer does not carry is not the same as one the
    // project does not set: the first is a partial read, and rendering
    // the default name from it would quietly ignore a template that
    // exists. GitLab answers an unset template as null.
    let template = match project.get("issue_branch_template") {
        Some(Value::Null) => None,
        Some(Value::String(text)) if text.trim().is_empty() => None,
        Some(Value::String(text)) => Some(text.clone()),
        _ => {
            return Err(forge_failure(
                "the project read answered without a usable 'issue_branch_template'".to_owned(),
            ));
        }
    };
    // The default branch is the ref a mint starts from where `--base`
    // names none. Inventing one could create the branch from the wrong
    // commit, which is a wrong remote write rather than a failed read.
    let default_branch = required(&project, "default_branch", |held| {
        held.as_str()
            .filter(|name| !name.trim().is_empty())
            .map(ToOwned::to_owned)
    })
    .map_err(|_| {
        forge_failure("the project read answered without a usable 'default_branch'".to_owned())
    })?;
    let issue = answered(
        &forge_call(
            cli,
            target,
            &[
                "api",
                &format!("projects/{encoded}/issues/{}", reference.number),
            ],
        )?,
        "the issue read",
    )?;
    // Every one of these is decoded strictly. `confidential` is the
    // reason: a missing or mistyped field defaulted to public would put
    // the issue's own title into a branch name anyone can read, and a
    // partial answer is exactly when that happens.
    let iid = required(&issue, "iid", Value::as_u64)?;
    let title = required(&issue, "title", |held| held.as_str().map(ToOwned::to_owned))?;
    let confidential = required(&issue, "confidential", Value::as_bool)?;
    // The user read happens only where the template names the creator,
    // which keeps the ordinary case at three calls.
    let creator = match template.as_deref() {
        Some(text) if text.contains("%{branch_creator}") => {
            let user = answered(&forge_call(cli, target, &["api", "user"])?, "the user read")?;
            user["username"].as_str().map(ToOwned::to_owned)
        }
        _ => None,
    };
    let rendered = gitlab_branch_name(
        iid,
        &title,
        confidential,
        template.as_deref(),
        creator.as_deref(),
    );
    if !admissible(&rendered.name) {
        return Err(refuse_template(&rendered.name, GRAMMAR_REFUSED));
    }
    // GitLab links a branch to an issue by name, and it matches on the
    // iid followed by a hyphen. A template can render a name the landed
    // grammar admits and GitLab links to nothing — `feat/%{id}-%{title}`
    // is the ordinary case — and the verb would then report a link that
    // does not exist.
    if !links_to(&rendered.name, iid) {
        return Err(refuse_template(&rendered.name, &link_refused(iid)));
    }
    Ok(Planned {
        iid,
        title,
        detail: gitlab_detail(confidential, template.as_deref(), rendered.approximated),
        name: rendered.name,
        default_branch,
    })
}

/// What a rendered name has to satisfy for the landed grammar.
const GRAMMAR_REFUSED: &str = "a template whose names match <type>/<slug> or <issue-id>-<slug>";

/// Whether GitLab links a branch of this name to the issue.
///
/// GitLab matches the issue's own iid followed by a hyphen at the start
/// of the name, so a prefix of any other shape links to nothing.
#[must_use]
pub fn links_to(branch: &str, iid: u64) -> bool {
    branch
        .strip_prefix(&iid.to_string())
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|slug| !slug.is_empty())
}

/// What a rendered name has to satisfy for GitLab to link it.
fn link_refused(iid: u64) -> String {
    format!(
        "a template whose names start with {iid}-, which is how GitLab links a branch to its issue"
    )
}

/// A rendered name this verb cannot use, named with its cause.
fn refuse_template(name: &str, expected: &str) -> RkError {
    RkError::refusal(
        Diagnostic::new(
            Reason::PrerequisiteUnmet,
            format!("the project's issue_branch_template renders '{name}', which this verb cannot use"),
        )
        .expected(expected)
        .action(
            "change Settings > Repository > Branch defaults > Branch name template, or pass a branch to rk worktree add instead",
        )
        .target_state("unchanged"),
    )
}

/// What the report says about how a GitLab name came out. Each note is a
/// state the operator must see rather than one rk decides quietly.
fn gitlab_detail(confidential: bool, template: Option<&str>, approximated: bool) -> Option<String> {
    let mut notes = Vec::new();
    if confidential {
        notes.push(
            "the issue is confidential, so GitLab keeps its title out of the branch and applies no template"
                .to_owned(),
        );
    }
    if let Some(text) = template {
        notes.push(format!("the project's branch name template is '{text}'"));
    }
    if approximated {
        notes.push(
            "the title carries characters outside the transliteration table, so this name can differ from the one GitLab's own button produces"
                .to_owned(),
        );
    }
    (!notes.is_empty()).then(|| notes.join("; "))
}

/// The GitLab path: plan the name from the project's own rules, then read
/// the branch and create it where it is absent.
fn resolve_gitlab(cli: &Path, target: &Path, ask: &Ask<'_>) -> Result<Resolved, RkError> {
    let (reference, base, apply) = (ask.reference, ask.base, ask.apply);
    let encoded = ask.repo.replace('/', "%2F");
    let planned = plan_gitlab(cli, target, &encoded, reference)?;
    // One read answers the whole question. Every admissible name carries
    // the issue's link prefix, so the prefix search is a superset of the
    // exact name: it finds the branch this rendering would produce, and
    // it finds one an earlier title or template produced instead.
    let linked = linked_branches(cli, target, &encoded, planned.iid)?;
    if let Some((primary, others)) = pick(linked, &planned.name) {
        let detail = if primary == planned.name {
            planned.detail
        } else {
            let took =
                format!("the forge already links '{primary}' to this issue, so it was taken");
            Some(
                planned
                    .detail
                    .map_or_else(|| took.clone(), |had| format!("{had}; {took}")),
            )
        };
        return Ok(Resolved {
            number: planned.iid,
            title: planned.title,
            branch: Some(primary),
            origin: "already",
            others,
            detail,
        });
    }
    let origin = if apply {
        // The name is known before the write here, unlike GitHub's, so
        // every local refusal the seat carries runs first. A branch
        // created at the forge and then refused locally would leave a
        // remote change no report accounts for.
        (ask.seatable)(&planned.name)?;
        // POST /projects/:id/repository/branches takes the name and the
        // ref, and resolves no template — which is why the reads exist.
        let start = base.unwrap_or(&planned.default_branch);
        forge_call(
            cli,
            target,
            &[
                "api",
                "--method",
                "POST",
                &format!(
                    "projects/{encoded}/repository/branches?branch={}&ref={}",
                    encode(&planned.name),
                    encode(start)
                ),
            ],
        )
        .and_then(|out| succeeded(&out, "the branch creation"))?;
        "forge"
    } else {
        "pending"
    };
    Ok(Resolved {
        number: planned.iid,
        title: planned.title,
        // GitLab links an issue and a branch by name, so the name rk
        // rendered is the name that now exists, and no read-back follows.
        branch: Some(planned.name),
        origin,
        others: Vec::new(),
        detail: planned.detail,
    })
}

/// The branch to take, and every other one linked to the same issue.
///
/// The rendering this run produced wins where the forge carries it, so a
/// steady project keeps taking the same branch. Otherwise the first name
/// in sort order wins, which is a rule rather than whatever order the
/// API answered in, and the rest are reported.
fn pick(mut linked: Vec<String>, rendered: &str) -> Option<(String, Vec<String>)> {
    if linked.is_empty() {
        return None;
    }
    linked.sort_unstable();
    let at = linked.iter().position(|name| name == rendered).unwrap_or(0);
    let primary = linked.remove(at);
    Some((primary, linked))
}

/// One field the API documents, or a failure naming it.
///
/// A field this verb reads is never defaulted: the shape of the answer
/// decides what the branch is called and whether the title may appear in
/// it, so a partial answer stops the run rather than being filled in.
fn required<T>(
    body: &Value,
    field: &str,
    read: impl Fn(&Value) -> Option<T>,
) -> Result<T, RkError> {
    read(&body[field]).ok_or_else(|| {
        forge_failure(format!(
            "the issue read answered without a usable '{field}'"
        ))
    })
}

/// Every branch the project carries under this issue's link prefix.
///
/// The Branches API's `search` takes `^term` for a starts-with match, so
/// one read answers what the issue already owns. An empty list is the
/// only proof of absence: a call that fails, or output that does not
/// parse, is unknown — and acting on unknown as if it were absence is
/// what creates a second branch for one issue.
///
/// # Errors
///
/// The call failing, classified from the forge's own answer, and a body
/// that is not the array the API documents.
fn linked_branches(
    cli: &Path,
    target: &Path,
    encoded: &str,
    iid: u64,
) -> Result<Vec<String>, RkError> {
    let found = forge_call(
        cli,
        target,
        &[
            "api",
            // A list endpoint answers one page of twenty by default, and
            // this read is the authority on what the issue owns: a
            // second page left unread would read as absence.
            "--paginate",
            &format!(
                "projects/{encoded}/repository/branches?search={}",
                encode(&format!("^{iid}-"))
            ),
        ],
    )?;
    let body = answered(&found, "the linked branch read")?;
    let Some(held) = body.as_array() else {
        return Err(forge_failure(
            "the linked branch read did not answer with a branch list".to_owned(),
        ));
    };
    let mut names = Vec::with_capacity(held.len());
    for branch in held {
        // A member the API documents as carrying a name and that does
        // not is unknown, not absent. Skipping it would turn a partial
        // answer into proof that the issue owns nothing.
        let Some(name) = branch["name"].as_str() else {
            return Err(forge_failure(
                "a branch in the linked branch read carries no name".to_owned(),
            ));
        };
        // The forge's own search decides what it matched; the link rule
        // decides what belongs to this issue.
        if links_to(name, iid) {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}

/// One forge CLI call, in the shape [`crate::branches::merged_request_for`]
/// already uses: the target's directory, and both pagers silenced.
///
/// A non-zero exit is returned rather than raised, because a caller reads
/// a not-found as an answer.
fn forge_call(cli: &Path, target: &Path, args: &[&str]) -> Result<Output, RkError> {
    std::process::Command::new(cli)
        .args(args)
        .current_dir(target)
        .env("GH_PAGER", "")
        .env("GLAB_PAGER", "")
        .output()
        .map_err(|source| {
            RkError::subprocess(
                Diagnostic::new(
                    Reason::SubprocessSpawn,
                    format!("the forge CLI did not run: {source}"),
                )
                .target_state("unchanged"),
            )
        })
}

/// A successful call's body, parsed.
fn answered(out: &Output, what: &str) -> Result<Value, RkError> {
    succeeded(out, what)?;
    serde_json::from_slice(&out.stdout)
        .map_err(|_| forge_failure(format!("{what} did not answer with JSON")))
}

/// A call that had to succeed, whose body nothing reads.
fn succeeded(out: &Output, what: &str) -> Result<(), RkError> {
    if out.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&out.stderr);
    Err(forge_failure_from(
        format!("{what} failed: {}", last_line(&out.stderr)),
        &stderr,
    ))
}

/// A forge call that failed, leaving nothing behind.
///
/// The reason is read from the forge CLI's own answer rather than
/// asserted. Only a failure the forge itself reports as transient is
/// [`Reason::ForgeTemporary`], because that reason tells the operator a
/// rerun can cure it — and a rerun cures neither a logged-out CLI nor a
/// permission the account does not have.
fn forge_failure(message: String) -> RkError {
    forge_failure_from(message, "")
}

/// [`forge_failure`], classified from the forge CLI's stderr.
fn forge_failure_from(message: String, stderr: &str) -> RkError {
    let (reason, action) = classify_forge_answer(stderr);
    let diagnostic = Diagnostic::new(reason, message)
        .action(action)
        .target_state("unchanged");
    match reason {
        Reason::ForgeAuthentication
        | Reason::ForgePermission
        | Reason::ForgeRateLimit
        | Reason::RemoteConflict => RkError::refusal(diagnostic),
        Reason::TargetNotFound => RkError::missing(diagnostic),
        _ => RkError::subprocess(diagnostic),
    }
}

/// The reason a forge CLI's own answer carries, and what fixes it.
///
/// `gh` renders an HTTP status as `HTTP <code>` and `glab` as
/// `<code> <phrase>`, so both spellings are matched. A status nothing
/// recognizes stays [`Reason::SubprocessFailed`] rather than claiming a
/// retry will help.
fn classify_forge_answer(stderr: &str) -> (Reason, &'static str) {
    let text = stderr.to_ascii_lowercase();
    let status =
        |code: &str| text.contains(&format!("http {code}")) || text.contains(&format!("{code} "));
    if text.contains("rate limit") || status("429") {
        return (
            Reason::ForgeRateLimit,
            "wait for the forge's limit to reset, then rerun",
        );
    }
    if status("401") || text.contains("not logged in") || text.contains("authentication") {
        return (
            Reason::ForgeAuthentication,
            "authenticate the forge CLI, then rerun",
        );
    }
    if status("403") {
        return (
            Reason::ForgePermission,
            "grant this account access to the project, then rerun",
        );
    }
    if status("404") {
        return (
            Reason::TargetNotFound,
            "check the issue number and the project, then rerun",
        );
    }
    if status("409") {
        return (
            Reason::RemoteConflict,
            "read what the forge already carries, then rerun",
        );
    }
    if status("500") || status("502") || status("503") || status("504") {
        return (
            Reason::ForgeTemporary,
            "rerun; the forge failed transiently",
        );
    }
    (
        Reason::SubprocessFailed,
        "read the forge's own answer, then decide",
    )
}

/// The last non-empty stderr line, for a one-line detail.
fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("no output")
        .to_owned()
}

/// Percent-encode everything outside the unreserved set, so a branch name
/// carrying `/` reaches the API as one path segment.
fn encode(text: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        Minted, Reference, admissible, agrees, gitlab_branch_name, linked_branch, parameterize,
        parse_reference,
    };
    use crate::detect::Detection;

    fn clone_of(host: &str, repo: &str) -> Detection {
        Detection {
            host: Some(host.to_owned()),
            repo: Some(repo.to_owned()),
            forge: None,
        }
    }

    #[test]
    fn a_reference_parses_from_every_accepted_form() {
        assert_eq!(
            parse_reference("57").expect("a number parses"),
            Reference {
                number: 57,
                repo: None,
                host: None
            }
        );
        assert_eq!(
            parse_reference("#57").expect("a hashed number parses"),
            Reference {
                number: 57,
                repo: None,
                host: None
            }
        );
        assert_eq!(
            parse_reference("https://github.com/acme/widget/issues/57").expect("a URL parses"),
            Reference {
                number: 57,
                repo: Some("acme/widget".into()),
                host: Some("github.com".into())
            }
        );
        assert_eq!(
            parse_reference("https://gitlab.example.com/acme/widget/-/issues/57")
                .expect("a self-hosted URL parses"),
            Reference {
                number: 57,
                repo: Some("acme/widget".into()),
                host: Some("gitlab.example.com".into())
            }
        );
        assert!(parse_reference("nonsense").is_err());
        assert!(parse_reference("0").is_err(), "no forge carries issue 0");
    }

    #[test]
    fn a_nested_gitlab_group_keeps_every_segment() {
        let parsed = parse_reference("https://gitlab.com/acme/team/widget/-/issues/57#note_9")
            .expect("a nested URL parses");
        assert_eq!(parsed.repo.as_deref(), Some("acme/team/widget"));
        assert_eq!(parsed.number, 57);
    }

    #[test]
    fn a_reference_that_names_another_project_disagrees() {
        let parsed =
            parse_reference("https://github.com/other/thing/issues/1").expect("a URL parses");
        assert!(agrees(&parsed, &clone_of("github.com", "acme/widget")).is_err());
    }

    #[test]
    fn a_bare_number_agrees_with_any_clone() {
        let parsed = parse_reference("57").expect("a number parses");
        assert!(agrees(&parsed, &clone_of("github.com", "acme/widget")).is_ok());
        assert!(agrees(&parsed, &clone_of("gitlab.com", "other/thing")).is_ok());
    }

    /// The Rails documentation's own example, which exercises
    /// transliteration, the run replacement, the squeeze, and the trim in
    /// one string.
    #[test]
    fn parameterize_matches_the_documented_example() {
        assert_eq!(parameterize("^très|Jolie-- ", false), "tres-jolie");
    }

    #[test]
    fn parameterize_preserves_case_when_asked() {
        assert_eq!(parameterize("Donald E. Knuth", true), "Donald-E-Knuth");
        assert_eq!(parameterize("Donald E. Knuth", false), "donald-e-knuth");
    }

    #[test]
    fn a_name_renders_from_id_and_title_without_a_template() {
        let rendered = gitlab_branch_name(57, "Fix the CSV upload!", false, None, None);
        assert_eq!(rendered.name, "57-fix-the-csv-upload");
        assert!(!rendered.approximated);
        assert!(admissible(&rendered.name));
    }

    #[test]
    fn an_empty_title_yields_the_bare_number() {
        assert_eq!(gitlab_branch_name(57, "", false, None, None).name, "57");
    }

    #[test]
    fn a_template_substitutes_every_supported_variable() {
        let rendered = gitlab_branch_name(
            57,
            "Fix the CSV upload",
            false,
            Some("%{branch_creator}-%{id}-%{title}"),
            Some("Ada Lovelace"),
        );
        assert_eq!(rendered.name, "Ada-Lovelace-57-fix-the-csv-upload");
    }

    /// GitLab leaves a placeholder it cannot resolve in the text, so rk
    /// does too. The grammar check one step later is where it fails.
    #[test]
    fn an_unknown_placeholder_survives_into_the_name() {
        let rendered = gitlab_branch_name(57, "Upload", false, Some("%{author}-%{id}"), None);
        assert_eq!(rendered.name, "%{author}-57");
        assert!(!admissible(&rendered.name));
    }

    #[test]
    fn a_confidential_issue_ignores_the_template() {
        let rendered = gitlab_branch_name(
            57,
            "The secret title",
            true,
            Some("%{id}-%{title}"),
            Some("ada"),
        );
        assert_eq!(rendered.name, "57-confidential-issue");
        assert!(admissible(&rendered.name));
    }

    #[test]
    fn a_long_name_truncates_at_100_and_drops_the_partial_segment() {
        let title = "alpha bravo charlie delta echo foxtrot golf hotel india juliett kilo lima mike november oscar papa";
        let rendered = gitlab_branch_name(57, title, false, None, None);
        assert!(rendered.name.len() <= 100, "{}", rendered.name);
        assert!(
            rendered.name.ends_with("-oscar"),
            "the partial trailing segment is dropped: {}",
            rendered.name
        );
        assert!(
            !rendered.name.contains("papa"),
            "the cut segment does not survive: {}",
            rendered.name
        );
    }

    #[test]
    fn a_title_outside_the_table_reports_approximated() {
        let rendered = gitlab_branch_name(57, "Исправить загрузку", false, None, None);
        assert!(rendered.approximated);
        assert_eq!(rendered.name, "57");
    }

    #[test]
    fn a_linked_branch_answer_judges_absent_one_and_many() {
        let answer = |names: &[&str]| {
            let nodes: Vec<_> = names
                .iter()
                .map(|name| serde_json::json!({ "ref": { "name": name } }))
                .collect();
            serde_json::json!({
                "data": { "repository": { "issue": { "linkedBranches": {
                    "pageInfo": { "hasNextPage": false },
                    "nodes": nodes
                } } } }
            })
        };
        assert_eq!(linked_branch(&answer(&[])), Minted::Absent);
        assert_eq!(
            linked_branch(&answer(&["57-fix"])),
            Minted::Already {
                branch: "57-fix".into(),
                others: vec![]
            }
        );
        // Sorted, so the choice is a rule rather than the order the
        // forge answered in.
        assert_eq!(
            linked_branch(&answer(&["57-fix-again", "57-fix"])),
            Minted::Already {
                branch: "57-fix".into(),
                others: vec!["57-fix-again".into()]
            }
        );
    }

    #[test]
    fn a_malformed_linked_branch_answer_is_unknown() {
        let body = serde_json::json!({ "errors": [{ "message": "Could not resolve" }] });
        assert!(matches!(linked_branch(&body), Minted::Unknown { .. }));
    }

    /// A realistic template that renders a name the landed grammar
    /// refuses, because `feature` is not a Conventional Commit type. This
    /// is the case the verb stops on before anything is created.
    #[test]
    fn a_customized_template_can_render_a_name_the_grammar_refuses() {
        let rendered = gitlab_branch_name(
            57,
            "Fix the upload",
            false,
            Some("feature/%{id}-%{title}"),
            None,
        );
        assert_eq!(rendered.name, "feature/57-fix-the-upload");
        assert!(!admissible(&rendered.name));
    }
}
