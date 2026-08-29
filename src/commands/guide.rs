//! `rk guide`: print a runbook with what detection knows filled in.
//!
//! The line between substituted and not is honesty, not convenience: a
//! value detection resolved — the project path, the forge, the technology —
//! is filled in, and a value `rk` would have to guess stays a placeholder.
//! `<release pr>` and its siblings exist only once a bot has opened them; a
//! substituted-but-stale number merges someone else's work, where a visible
//! placeholder fails loudly.

use crate::cli::guide::GuideArgs;
use crate::commands::walk;
use crate::detect;
use crate::embedded;
use crate::error::RkError;
use crate::output::Output;

/// Print one runbook, or list them.
///
/// # Errors
///
/// Returns [`RkError::NotFound`] for an unknown runbook and
/// [`RkError::Usage`] when neither a name nor `--list` is given, or a flag
/// value is not one of the known axes.
pub fn run(args: &GuideArgs) -> Result<(), RkError> {
    let out = Output::human();
    let entries = walk(&embedded::RUNBOOKS);
    if args.list {
        for (path, _) in &entries {
            out.result_line(path.trim_end_matches(".md").to_ascii_lowercase());
        }
        return Ok(());
    }
    let Some(name) = args.name.as_deref() else {
        return Err(RkError::Usage(
            "name a runbook, or pass --list to see them".into(),
        ));
    };
    let wanted = name.to_ascii_lowercase();
    let wanted = wanted.trim_end_matches(".md");
    let Some((_, contents)) = entries
        .iter()
        .find(|(path, _)| path.trim_end_matches(".md").eq_ignore_ascii_case(wanted))
    else {
        return Err(RkError::NotFound {
            kind: "runbook",
            name: name.to_owned(),
        });
    };
    let text = String::from_utf8_lossy(contents);

    let forge = match args.forge.as_deref() {
        Some(value) => Some(
            detect::Forge::parse(value)
                .ok_or_else(|| {
                    RkError::Usage(format!(
                        "unknown forge '{value}'; the forges are: github, gitlab"
                    ))
                })?
                .as_str(),
        ),
        None => None,
    };
    let tech = match args.tech.as_deref() {
        Some(value @ ("rust" | "python" | "bash")) => Some(value.to_owned()),
        Some(other) => {
            return Err(RkError::Usage(format!(
                "unknown tech '{other}'; the bindings are: rust, python, bash"
            )));
        }
        None => None,
    };

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let detected = detect::detect(&cwd);
    let forge = forge.or_else(|| detected.forge.map(detect::Forge::as_str));
    let tech = tech.or_else(|| detect::tech_of(&cwd).map(str::to_owned));
    let repo = args.repo.clone().or(detected.repo);

    let rendered = render(&text, forge, tech.as_deref(), repo.as_deref());
    let unresolved = repo.is_none() && rendered.contains("<repo>");
    out.result_raw(&rendered);
    if unresolved {
        out.frame("note: <repo> is unresolved; pass --repo <owner/name> to fill it");
    }
    Ok(())
}

/// Which axis a variant label selects on.
fn axis_of(selector: &str) -> Option<&'static str> {
    match selector {
        "github" | "gitlab" => Some("forge"),
        "rust" | "python" | "bash" => Some("tech"),
        _ => None,
    }
}

/// The selector of a variant label line, `On <selector>:`.
fn label_of(line: &str) -> Option<&str> {
    let selector = line.strip_prefix("On ")?.strip_suffix(":")?;
    axis_of(selector).map(|_| selector)
}

/// Render one runbook: keep the matching variant of every resolved axis and
/// drop its siblings, substitute `<repo>` where it is known, and leave
/// everything else byte-identical.
fn render(text: &str, forge: Option<&str>, tech: Option<&str>, repo: Option<&str>) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut idx = 0;
    while idx < lines.len() {
        let line = lines[idx];
        let Some(selector) = label_of(line) else {
            out.push(substitute(line, repo));
            idx += 1;
            continue;
        };
        let resolved = match axis_of(selector) {
            Some("forge") => forge,
            Some("tech") => tech,
            _ => None,
        };
        let Some(resolved) = resolved else {
            out.push(substitute(line, repo));
            idx += 1;
            continue;
        };
        // The variant grammar: the label line, one blank line, then one
        // fenced block or one paragraph.
        let body_start = idx + 2;
        let body_end = if lines.get(body_start).is_some_and(|l| l.starts_with("```")) {
            lines[body_start + 1..]
                .iter()
                .position(|l| l.starts_with("```"))
                .map_or(lines.len(), |offset| body_start + 1 + offset + 1)
        } else {
            lines[body_start..]
                .iter()
                .position(|l| l.trim().is_empty())
                .map_or(lines.len(), |offset| body_start + offset)
        };
        if selector == resolved {
            for kept in lines.iter().take(body_end).skip(body_start) {
                out.push(substitute(kept, repo));
            }
            idx = body_end;
        } else {
            idx = body_end;
            // Swallow one following blank line, so a dropped variant does
            // not leave a double gap.
            if lines.get(idx).is_some_and(|l| l.trim().is_empty()) {
                idx += 1;
            }
        }
    }
    out.join("\n")
}

/// Fill `<repo>` where detection or `--repo` resolved it; everything else
/// stays a placeholder.
fn substitute(line: &str, repo: Option<&str>) -> String {
    repo.map_or_else(|| line.to_owned(), |slug| line.replace("<repo>", slug))
}

#[cfg(test)]
mod tests {
    use super::render;

    const DOC: &str = "# T\n\nOn github:\n\n```bash\ngh pr list --repo <repo>\n```\n\nOn gitlab:\n\n```bash\nglab mr list\n```\n\ntail <release pr>\n";

    /// Nothing resolved: the output is byte-identical to the source.
    #[test]
    fn an_unresolved_render_is_byte_identical() {
        assert_eq!(render(DOC, None, None, None), DOC);
    }

    /// A resolved forge keeps its variant, drops the sibling and both
    /// labels, and a resolved repo fills `<repo>` while `<release pr>`
    /// stays a placeholder.
    #[test]
    fn a_resolved_render_selects_and_substitutes() {
        let rendered = render(DOC, Some("github"), None, Some("acme/widget"));
        assert!(rendered.contains("gh pr list --repo acme/widget"));
        assert!(!rendered.contains("glab"));
        assert!(!rendered.contains("On github:"));
        assert!(!rendered.contains("<repo>"));
        assert!(rendered.contains("<release pr>"));
        let gitlab = render(DOC, Some("gitlab"), None, None);
        assert!(gitlab.contains("glab mr list"));
        assert!(!gitlab.contains("gh pr list"));
    }

    /// A paragraph variant is selected the same way a fenced one is.
    #[test]
    fn a_paragraph_variant_renders() {
        let doc = "On github:\n\nthe force-push refresh survives.\n\nOn gitlab:\n\nthe request is replaced.\n\nend\n";
        let rendered = render(doc, Some("gitlab"), None, None);
        assert_eq!(rendered, "the request is replaced.\n\nend\n");
    }
}
