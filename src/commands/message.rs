//! `rk message`: the content guards over a commit message, a title, or a
//! request body.
//!
//! Two classes of finding, declared in `blocks/message-guards`:
//! attribution an agent left in the text, and a reference to a path the
//! target repository ignores — an internal artifact that would leak into
//! the permanent record. The release bot's request is exempt from the
//! attribution class alone, recognized by exactly the title shape the
//! landed title check admits for the bot; the ignored-path class always
//! runs. The guard file owns the patterns; the matchers here implement
//! them by hand — the binary carries no regex engine — and a unit test
//! pins the file to the set the matchers cover, so a pattern edit and its
//! matcher move together.

use std::io::Read as _;
use std::io::Write as _;

use camino::Utf8Path;
use serde::Serialize;

use crate::cli::message::{MessageArgs, MessageKind};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::output::Output;

/// The guard patterns, verbatim: `blocks/message-guards`.
static GUARDS: &str = include_str!("../../blocks/message-guards");

/// One finding.
#[derive(Debug, Serialize)]
struct Finding {
    /// `attribution` or `internal-path`.
    class: &'static str,
    /// The 1-based line the finding sits on.
    line: usize,
    /// What matched.
    detail: String,
}

/// The machine form of a report.
#[derive(Debug, Serialize)]
struct Report {
    /// The shape version of this document.
    schema: &'static str,
    /// What the text was judged as.
    kind: &'static str,
    /// Whether the bot exemption applied to the attribution class.
    exempt: bool,
    /// Every finding, in line order.
    findings: Vec<Finding>,
}

/// Judge the text and report; exit 1 under `--check` when a finding stands.
///
/// # Errors
///
/// Returns [`RkError::CheckFailed`] under `--check` when any finding
/// stands, [`RkError::Io`] when the input cannot be read, and an error
/// when the report cannot serialize.
pub fn run(args: &MessageArgs) -> Result<(), RkError> {
    let text = read_input(args.file.as_deref().map(camino::Utf8Path::as_str))?;
    let out = Output::new(args.json);

    let title = match args.kind {
        MessageKind::Commit | MessageKind::Title => text.lines().next().unwrap_or(""),
        MessageKind::Body => args.title.as_deref().unwrap_or(""),
    };
    let exempt = bot_title(title);

    let mut findings = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if !exempt {
            findings.extend(attribution_hits(line).into_iter().map(|detail| Finding {
                class: "attribution",
                line: index + 1,
                detail,
            }));
        }
    }
    let mut seen: std::collections::BTreeSet<(usize, String)> = std::collections::BTreeSet::new();
    match ignored_paths(&args.target, &text) {
        IgnoreJudgment::Repo(hits) => {
            for (line, token) in hits {
                findings.push(Finding {
                    class: "internal-path",
                    line,
                    detail: format!("{token} is git-ignored in {}", args.target),
                });
                seen.insert((line, token));
            }
        }
        IgnoreJudgment::NoRepo => {
            if !args.json {
                out.warn(format!(
                    "{} is not a git repository; only the fixed .draft/ pattern was tested",
                    args.target
                ));
            }
        }
    }
    // A fixed-pattern fragment already inside a reported token on the
    // same line — `nested/.draft/plan.md` carrying `.draft/plan.md` — is
    // the same reference, not a second finding.
    findings.extend(
        fixed_draft_hits(&text)
            .into_iter()
            .filter(|(line, fragment)| {
                !seen
                    .iter()
                    .any(|(seen_line, token)| seen_line == line && token.contains(fragment))
            })
            .map(|(line, fragment)| Finding {
                class: "internal-path",
                line,
                detail: format!("{fragment} references the internal .draft/ tree"),
            }),
    );
    findings.sort_by_key(|finding| finding.line);

    if exempt {
        out.result_line("exempt: the release bot's request, by its title");
    }
    for finding in &findings {
        out.result_line(format!(
            "{}:{} {}",
            finding.class, finding.line, finding.detail
        ));
    }
    if findings.is_empty() {
        out.result_line(format!("clean {}", args.kind.as_str()));
    }
    let count = findings.len();
    out.emit(&Report {
        schema: "rk.message/1",
        kind: args.kind.as_str(),
        exempt,
        findings,
    })?;

    if args.check && count > 0 {
        return Err(RkError::check_failed(
            Diagnostic::new(
                Reason::StateDrift,
                format!(
                    "the {} carries {count} finding{}",
                    args.kind.as_str(),
                    if count == 1 { "" } else { "s" }
                ),
            )
            .expected("no agent attribution and no reference to a git-ignored path")
            .action("reword the text; the findings above name each line"),
        ));
    }
    Ok(())
}

/// The text: stdin for `-` or no file, the file otherwise.
fn read_input(file: Option<&str>) -> Result<String, RkError> {
    match file {
        None | Some("-") => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            Ok(text)
        }
        Some(path) => Ok(std::fs::read_to_string(path)?),
    }
}

/// Whether the title is the release bot's, by exactly the shape the
/// landed title check admits for it:
/// `^chore(\((release|master|main)\))?: (release|v).+$`.
fn bot_title(title: &str) -> bool {
    let Some(rest) = title.strip_prefix("chore") else {
        return false;
    };
    let rest = if rest.starts_with('(') {
        let Some(rest) = ["(release)", "(master)", "(main)"]
            .iter()
            .find_map(|scope| rest.strip_prefix(scope))
        else {
            return false;
        };
        rest
    } else {
        rest
    };
    let Some(rest) = rest.strip_prefix(": ") else {
        return false;
    };
    ["release", "v"].iter().any(|stem| {
        rest.strip_suffix('\n')
            .unwrap_or(rest)
            .strip_prefix(stem)
            .is_some_and(|tail| !tail.is_empty())
    })
}

/// Every attribution match on one line, as the matched fragment.
///
/// Each matcher implements one pattern of the guard file's attribution
/// class by hand, exactly — each bracketed case class expands to its
/// literal variants, never to a broader case-insensitive search — and
/// [`guard_patterns`] with its test holds the two in step.
fn attribution_hits(line: &str) -> Vec<String> {
    let mut hits = Vec::new();
    // [Gg]enerated with \[?Claude
    if [
        "Generated with Claude",
        "generated with Claude",
        "Generated with [Claude",
        "generated with [Claude",
    ]
    .iter()
    .any(|variant| line.contains(variant))
    {
        hits.push("generated-with-claude attribution".to_owned());
    }
    // 🤖 Generated with
    if line.contains("🤖 Generated with") {
        hits.push("robot generated-with attribution".to_owned());
    }
    // [Cc]o-[Aa]uthored-[Bb]y:.*([Cc]laude|[Cc]opilot|Codex|ChatGPT)
    let trailer = [
        "Co-Authored-By:",
        "Co-Authored-by:",
        "Co-authored-By:",
        "Co-authored-by:",
        "co-Authored-By:",
        "co-Authored-by:",
        "co-authored-By:",
        "co-authored-by:",
    ]
    .iter()
    .filter_map(|variant| line.find(variant))
    .min();
    if let Some(at) = trailer {
        let tail = &line[at..];
        if ["Claude", "claude", "Copilot", "copilot", "Codex", "ChatGPT"]
            .iter()
            .any(|agent| tail.contains(agent))
        {
            hits.push("agent co-authored-by trailer".to_owned());
        }
    }
    // noreply@anthropic\.com
    if line.contains("noreply@anthropic.com") {
        hits.push("anthropic noreply address".to_owned());
    }
    hits
}

/// What the ignored-path check could determine.
enum IgnoreJudgment {
    /// The target is a repository; these `(line, token)` pairs are ignored.
    Repo(Vec<(usize, String)>),
    /// No repository at the target; only the fixed pattern judged.
    NoRepo,
}

/// The path-shaped tokens of the text the target's ignore rules reject,
/// degraded to no repository judgment where the target is none. The
/// fixed `.draft/` pattern is not here: it runs unconditionally in
/// [`fixed_draft_hits`], so a decorated reference no clean token carries
/// still answers, repository or not.
fn ignored_paths(target: &Utf8Path, text: &str) -> IgnoreJudgment {
    let candidates: Vec<(usize, String)> = text
        .lines()
        .enumerate()
        .flat_map(|(index, line)| {
            line.split_whitespace()
                .filter_map(path_token)
                .map(move |token| (index + 1, token))
        })
        .collect();
    if candidates.is_empty() {
        return IgnoreJudgment::Repo(Vec::new());
    }
    check_ignore(target, &candidates).map_or(IgnoreJudgment::NoRepo, IgnoreJudgment::Repo)
}

/// Every `(line, fragment)` the fixed internal-path pattern matches:
/// `(^|[^A-Za-z0-9])\.draft/`, implemented exactly — a `.draft/` whose
/// preceding character, where one exists, is not ASCII-alphanumeric —
/// with the fragment read forward to the surrounding whitespace and
/// trimmed of trailing wrappers, so a decorated reference like
/// `path=.draft/plan.md` or a markdown link still answers.
fn fixed_draft_hits(text: &str) -> Vec<(usize, String)> {
    let mut hits = Vec::new();
    for (index, line) in text.lines().enumerate() {
        for (at, _) in line.match_indices(".draft/") {
            let boundary = line[..at]
                .chars()
                .next_back()
                .is_none_or(|c| !c.is_ascii_alphanumeric());
            if !boundary {
                continue;
            }
            let tail = &line[at..];
            let end = tail.find(char::is_whitespace).unwrap_or(tail.len());
            let fragment = tail[..end].trim_end_matches(|c: char| "()[]<>`'\".,;:".contains(c));
            hits.push((index + 1, fragment.to_owned()));
        }
    }
    hits
}

/// A whitespace token reduced to its path candidate: wrapping brackets
/// and quotes trimmed from both edges, sentence punctuation only from the
/// end — a leading dot is part of a hidden path — URLs, flags, and
/// variables skipped, and only a token of two or more segments kept,
/// because a bare word is prose, not a reference.
fn path_token(token: &str) -> Option<String> {
    let token = token
        .trim_start_matches(|c: char| "()[]<>`'\"".contains(c))
        .trim_end_matches(|c: char| "()[]<>`'\".,;:".contains(c));
    if token.contains("://") || token.starts_with('-') || token.contains('$') {
        return None;
    }
    let (head, tail) = token.split_once('/')?;
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    Some(token.to_owned())
}

/// The candidates the target's git ignores, or `None` where the target is
/// not a repository the check could consult.
///
/// `-z` on both sides: input and output are NUL-delimited, so a non-ASCII
/// path comes back verbatim rather than `core.quotePath`-escaped and the
/// byte comparison holds. The writer is its own thread, because git may
/// fill its stdout pipe while this process is still writing stdin — the
/// buffering deadlock its documentation assigns the caller.
fn check_ignore(target: &Utf8Path, candidates: &[(usize, String)]) -> Option<Vec<(usize, String)>> {
    let mut command = std::process::Command::new(crate::probes::git_bin());
    let mut child = command
        .arg("-C")
        .arg(target.as_std_path())
        .args(["check-ignore", "--stdin", "-z"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let writer = child.stdin.take().map(|mut stdin| {
        let payload: Vec<u8> = candidates
            .iter()
            .flat_map(|(_, token)| token.as_bytes().iter().copied().chain([0]))
            .collect();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&payload);
        })
    });
    let output = child.wait_with_output().ok()?;
    if let Some(writer) = writer {
        let _ = writer.join();
    }
    // 0: some input is ignored; 1: none is. Anything else — 128 for a
    // missing repository above all — is a target the check cannot judge.
    if !matches!(output.status.code(), Some(0 | 1)) {
        return None;
    }
    let ignored: std::collections::BTreeSet<&[u8]> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .collect();
    Some(
        candidates
            .iter()
            .filter(|(_, token)| ignored.contains(token.as_bytes()))
            .cloned()
            .collect(),
    )
}

/// The guard file's `(class, pattern)` lines, in order.
#[must_use]
pub fn guard_patterns() -> Vec<(&'static str, &'static str)> {
    let mut class = "";
    let mut patterns = Vec::new();
    for line in GUARDS.lines() {
        if let Some(named) = line.strip_prefix("# class: ") {
            class = named;
        } else if !line.starts_with('#') && !line.is_empty() {
            patterns.push((class, line));
        }
    }
    patterns
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        Finding, Report, attribution_hits, bot_title, fixed_draft_hits, guard_patterns, path_token,
    };

    /// The complete `rk.message/1` shape, held by snapshot.
    #[test]
    fn the_message_schema_snapshot_holds() {
        let report = Report {
            schema: "rk.message/1",
            kind: "commit",
            exempt: false,
            findings: vec![Finding {
                class: "internal-path",
                line: 3,
                detail: ".draft/plan.md is git-ignored in .".into(),
            }],
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.message/1","kind":"commit","exempt":false,"findings":[{"class":"internal-path","line":3,"detail":".draft/plan.md is git-ignored in ."}]}"#
        );
    }

    /// The guard file and the hand matchers move together: this is the
    /// exact pattern set the matchers above implement, so an edit to
    /// `blocks/message-guards` fails here until the matcher follows.
    #[test]
    fn the_guard_file_holds_the_patterns_the_matchers_implement() {
        assert_eq!(
            guard_patterns(),
            [
                ("attribution", r"[Gg]enerated with \[?Claude"),
                ("attribution", "🤖 Generated with"),
                (
                    "attribution",
                    r"[Cc]o-[Aa]uthored-[Bb]y:.*([Cc]laude|[Cc]opilot|Codex|ChatGPT)"
                ),
                ("attribution", r"noreply@anthropic\.com"),
                ("internal-path", r"(^|[^A-Za-z0-9])\.draft/"),
            ]
        );
    }

    #[test]
    fn the_attribution_matchers_cover_the_patterns() {
        for line in [
            "Generated with Claude Code",
            "generated with [Claude Code](https://claude.com/claude-code)",
            "🤖 Generated with tooling",
            "Co-Authored-By: Claude <x@y>",
            "co-authored-by: github-copilot",
            "Co-authored-by: Codex",
            "Co-Authored-By: ChatGPT",
            "Signed noreply@anthropic.com",
        ] {
            assert!(!attribution_hits(line).is_empty(), "{line} must match");
        }
        for line in [
            "Generated with release-plz",
            "Co-authored-by: A Person <person@example.com>",
            "the claude skill route",
            "Co-authored-by: Autopilot Team",
            "Xenerated with Claude",
            "CO-AUTHORED-BY: Claude",
        ] {
            assert!(attribution_hits(line).is_empty(), "{line} must not match");
        }
    }

    /// Case transformation never feeds an offset back into the original:
    /// multibyte text before a trailer must match without panicking.
    #[test]
    fn a_multibyte_prefix_neither_panics_nor_hides_the_trailer() {
        // Enough expanding characters that a lowercased offset would
        // fall past the original string's end, not merely drift.
        let line = format!("{} Co-Authored-By: Claude", "İ".repeat(40));
        assert!(!attribution_hits(&line).is_empty());
        assert!(attribution_hits(&format!("{} nothing here", "İ".repeat(40))).is_empty());
    }

    /// The fixed pattern is `(^|[^A-Za-z0-9])\.draft/`, exactly: a
    /// decorated reference answers, an alphanumeric-adjacent one does not.
    #[test]
    fn the_fixed_pattern_matches_decorated_references_only_at_a_boundary() {
        assert_eq!(
            fixed_draft_hits(
                "path=.draft/plan.md
"
            ),
            vec![(1, ".draft/plan.md".to_owned())]
        );
        assert_eq!(
            fixed_draft_hits(
                "a [plan](.draft/plan.md) link
"
            ),
            vec![(1, ".draft/plan.md".to_owned())]
        );
        assert_eq!(
            fixed_draft_hits(
                ".draft/x
"
            ),
            vec![(1, ".draft/x".to_owned())]
        );
        assert!(
            fixed_draft_hits(
                "archived.draft/x
"
            )
            .is_empty()
        );
        assert!(
            fixed_draft_hits(
                "no reference here
"
            )
            .is_empty()
        );
    }

    /// Exactly the landed title check's bot alternative:
    /// `^chore(\((release|master|main)\))?: (release|v).+$`.
    #[test]
    fn the_bot_exemption_is_the_title_checks_bot_alternative() {
        for title in [
            "chore: release v0.2.6",
            "chore(release): v0.3.0",
            "chore(master): release 1.0.0",
            "chore(main): v2",
        ] {
            assert!(bot_title(title), "{title} is the bot's");
        }
        for title in [
            "chore: bump deps",
            "chore(deps): release v1",
            "feat(cli): release v1",
            "chore(release): ",
            "chore(release): v",
            "chore:release v1",
        ] {
            assert!(!bot_title(title), "{title} is not the bot's");
        }
    }

    #[test]
    fn a_path_token_is_two_segments_without_url_flag_or_variable() {
        assert_eq!(
            path_token("(.draft/plan.md)"),
            Some(".draft/plan.md".into())
        );
        assert_eq!(path_token("`src/main.rs`,"), Some("src/main.rs".into()));
        assert_eq!(path_token("https://a.b/c"), None);
        assert_eq!(path_token("--flag/value"), None);
        assert_eq!(path_token("$HOME/x"), None);
        assert_eq!(path_token("and/or"), Some("and/or".into()));
        assert_eq!(path_token("word"), None);
        assert_eq!(path_token("trailing/"), None);
    }
}
