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
use crate::landing::manifest::{self, Style, Workflow};
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
    let workflow = args.workflow.as_deref().map(Workflow::parse).transpose()?;
    let style = args.style.as_deref().map(Style::parse).transpose()?;

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let detected = detect::detect(&cwd);
    let forge = forge.or_else(|| detected.forge.map(detect::Forge::as_str));
    let tech = tech.or_else(|| detect::tech_of(&cwd).map(str::to_owned));
    let repo = args.repo.clone().or(detected.repo);
    // The workflow axis resolves from the landing record — the mode is a
    // committed project decision, not a detection guess — and stays open
    // where no record exists, the honest pre-landing fallback.
    let record =
        camino::Utf8Path::from_path(&cwd).and_then(|path| manifest::load(path).ok().flatten());
    let workflow = workflow.or_else(|| record.as_ref().map(|record| record.parameters.workflow));
    // The style axis resolves the same way: a committed project decision,
    // open where no record exists.
    let style = style.or_else(|| record.as_ref().and_then(|record| record.parameters.style));

    let rendered = render(
        &text,
        forge,
        tech.as_deref(),
        repo.as_deref(),
        workflow.map(Workflow::as_str),
        style.map(Style::as_str),
    );
    let unresolved = repo.is_none() && rendered.contains("<repo>");
    out.result_raw(&rendered);
    if unresolved {
        out.frame("note: <repo> is unresolved; pass --repo <owner/name> to fill it");
    }
    Ok(())
}

/// Which axis a variant label selects on. A `tech/forge` pair selects on
/// both at once, for the steps whose answer differs per pair rather than
/// per axis — the provenance verifier is one.
fn axis_of(selector: &str) -> Option<&'static str> {
    if let Some((tech, forge)) = selector.split_once('/') {
        return (axis_of(tech) == Some("tech") && axis_of(forge) == Some("forge"))
            .then_some("pair");
    }
    match selector {
        "github" | "gitlab" => Some("forge"),
        "rust" | "python" | "bash" => Some("tech"),
        "worktree" | "branches" => Some("workflow"),
        "trunk" | "lines" => Some("style"),
        _ => None,
    }
}

/// The selector of a variant label line, `On <selector>:`.
fn label_of(line: &str) -> Option<&str> {
    let selector = line.strip_prefix("On ")?.strip_suffix(":")?;
    axis_of(selector).map(|_| selector)
}

/// Render one runbook: keep the matching variant of every resolved axis and
/// drop its siblings, substitute `<repo>` and `<tech>` where they are known,
/// and leave everything else byte-identical.
fn render(
    text: &str,
    forge: Option<&str>,
    tech: Option<&str>,
    repo: Option<&str>,
    workflow: Option<&str>,
    style: Option<&str>,
) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut idx = 0;
    while idx < lines.len() {
        let line = lines[idx];
        let Some(selector) = label_of(line) else {
            out.push(substitute(line, repo, tech));
            idx += 1;
            continue;
        };
        let resolved = match axis_of(selector) {
            Some("forge") => forge.map(str::to_owned),
            Some("tech") => tech.map(str::to_owned),
            Some("workflow") => workflow.map(str::to_owned),
            Some("style") => style.map(str::to_owned),
            // A pair resolves only once both halves have: with either axis
            // open, every pair variant stays visible, label and all.
            Some("pair") => match (tech, forge) {
                (Some(tech), Some(forge)) => Some(format!("{tech}/{forge}")),
                _ => None,
            },
            _ => None,
        };
        let Some(resolved) = resolved else {
            out.push(substitute(line, repo, tech));
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
                out.push(substitute(kept, repo, tech));
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

/// Fill `<repo>` and `<tech>` where detection or a flag resolved them;
/// everything else stays a placeholder.
fn substitute(line: &str, repo: Option<&str>, tech: Option<&str>) -> String {
    let mut line = line.to_owned();
    if let Some(slug) = repo {
        line = line.replace("<repo>", slug);
    }
    if let Some(tech) = tech {
        line = line.replace("<tech>", tech);
    }
    line
}

#[cfg(test)]
mod tests {
    use super::render;

    const DOC: &str = "# T\n\nOn github:\n\n```bash\ngh pr list --repo <repo>\n```\n\nOn gitlab:\n\n```bash\nglab mr list\n```\n\ntail <release pr>\n";

    /// Nothing resolved: the output is byte-identical to the source.
    #[test]
    fn an_unresolved_render_is_byte_identical() {
        assert_eq!(render(DOC, None, None, None, None, None), DOC);
    }

    /// A resolved forge keeps its variant, drops the sibling and both
    /// labels, and a resolved repo fills `<repo>` while `<release pr>`
    /// stays a placeholder.
    #[test]
    fn a_resolved_render_selects_and_substitutes() {
        let rendered = render(DOC, Some("github"), None, Some("acme/widget"), None, None);
        assert!(rendered.contains("gh pr list --repo acme/widget"));
        assert!(!rendered.contains("glab"));
        assert!(!rendered.contains("On github:"));
        assert!(!rendered.contains("<repo>"));
        assert!(rendered.contains("<release pr>"));
        let gitlab = render(DOC, Some("gitlab"), None, None, None, None);
        assert!(gitlab.contains("glab mr list"));
        assert!(!gitlab.contains("gh pr list"));
    }

    /// A paragraph variant is selected the same way a fenced one is.
    #[test]
    fn a_paragraph_variant_renders() {
        let doc = "On github:\n\nthe force-push refresh survives.\n\nOn gitlab:\n\nthe request is replaced.\n\nend\n";
        let rendered = render(doc, Some("gitlab"), None, None, None, None);
        assert_eq!(rendered, "the request is replaced.\n\nend\n");
    }

    /// A pair variant renders only for its exact pair, drops for every
    /// other resolved pair, and stays visible — label and all — while
    /// either axis is open, so an unresolved render still shows every
    /// pair's answer.
    #[test]
    fn a_pair_variant_selects_on_both_axes() {
        let doc = "On bash/gitlab:\n\n```bash\ncosign verify-blob-attestation\n```\n\nOn rust/gitlab:\n\nno provenance surface.\n\nend\n";
        let matched = render(doc, Some("gitlab"), Some("bash"), None, None, None);
        assert!(matched.contains("cosign verify-blob-attestation"));
        assert!(!matched.contains("no provenance surface"));
        assert!(!matched.contains("On bash/gitlab:"));
        let sibling = render(doc, Some("gitlab"), Some("rust"), None, None, None);
        assert!(!sibling.contains("cosign"));
        assert!(sibling.contains("no provenance surface."));
        let open_axis = render(doc, Some("gitlab"), None, None, None, None);
        assert_eq!(open_axis, doc, "an open axis keeps every pair variant");
    }

    /// The workflow axis renders like the others: resolved, the matching
    /// variant is kept and its sibling dropped; open, every variant
    /// prints with its label.
    #[test]
    fn a_workflow_variant_selects_on_the_mode() {
        let doc = "On worktree:\n\nrk worktree add release-branch --apply\n\nOn branches:\n\ngh pr checkout 7\n\nend\n";
        let worktree = render(doc, None, None, None, Some("worktree"), None);
        assert!(worktree.contains("rk worktree add"));
        assert!(!worktree.contains("gh pr checkout"));
        let branches = render(doc, None, None, None, Some("branches"), None);
        assert!(branches.contains("gh pr checkout"));
        assert!(!branches.contains("rk worktree add"));
        assert_eq!(
            render(doc, None, None, None, None, None),
            doc,
            "an unresolved mode keeps every variant, label and all"
        );
    }

    /// The style axis renders like the workflow axis: resolved, the
    /// matching variant is kept and its sibling dropped; open, every
    /// variant prints with its label.
    #[test]
    fn a_style_variant_selects_on_the_style() {
        let doc = "On trunk:\n\nthe request merges itself when the last check passes.\n\nOn lines:\n\nthe merge is yours.\n\nend\n";
        let trunk = render(doc, None, None, None, None, Some("trunk"));
        assert!(trunk.contains("merges itself"));
        assert!(!trunk.contains("the merge is yours"));
        let lines = render(doc, None, None, None, None, Some("lines"));
        assert!(lines.contains("the merge is yours"));
        assert!(!lines.contains("merges itself"));
        assert_eq!(
            render(doc, None, None, None, None, None),
            doc,
            "an unresolved style keeps every variant, label and all"
        );
    }

    /// A resolved tech fills `<tech>` everywhere; unresolved it stays.
    #[test]
    fn a_resolved_tech_fills_the_placeholder() {
        let doc = "rk init --tech <tech> --target .\n";
        assert_eq!(
            render(doc, None, Some("rust"), None, None, None),
            "rk init --tech rust --target .\n"
        );
        assert_eq!(render(doc, None, None, None, None, None), doc);
    }
}
