//! Starting work from a forge issue: the pure half.
//!
//! An issue becomes a branch name at the forge, never in this binary's
//! imagination. GitHub names the branch server-side, so nothing here
//! renders one. GitLab exposes no such endpoint — its branch-name
//! template is a project setting the web UI applies — so rk reproduces
//! `Issue.to_branch_name` exactly, and this module is where that
//! reproduction lives. Spawning stays in the handler, exactly as
//! [`crate::branches`] declares for the branch half.

use serde_json::Value;

use crate::detect::Detection;

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
/// More than one node keeps the first and reports the rest: an issue with
/// two linked branches is a state rk did not create, and picking from it
/// silently would look like a choice rk made.
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
    let mut names = nodes
        .iter()
        .filter_map(|node| node.pointer("/ref/name").and_then(Value::as_str))
        .map(ToOwned::to_owned);
    match names.next() {
        None if nodes.is_empty() => Minted::Absent,
        None => Minted::Unknown {
            detail: "a linked branch carries no ref name".to_owned(),
        },
        Some(branch) => Minted::Already {
            branch,
            others: names.collect(),
        },
    }
}

/// Whether the landed grammar admits a branch name.
///
/// A named re-export, so the call site reads as intent.
/// [`crate::worktree::matches_grammar`] stays the single owner.
#[must_use]
pub fn admissible(branch: &str) -> bool {
    crate::worktree::matches_grammar(branch)
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
                "data": { "repository": { "issue": { "linkedBranches": { "nodes": nodes } } } }
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
        assert_eq!(
            linked_branch(&answer(&["57-fix", "57-fix-again"])),
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
