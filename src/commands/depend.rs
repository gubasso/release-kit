//! `rk depend assess | add`: another project as a dependency of a
//! target.
//!
//! `assess` reads the source and the target offline and reports every
//! way the dependency can land, exiting 0 on every verdict; `add` serves
//! one way — the fragments for a manager, or the technology's own
//! command for a prod dependency — and under `--apply` seeds a manager
//! file only where the target has none, never editing a file the target
//! owns. Every report goes through the output boundary with a versioned
//! schema.

use serde::Serialize;

use crate::cli::depend::{AddArgs, AssessArgs, DependAction, DependArgs};
use crate::depend::fragments::Fragment;
use crate::depend::matrix::{self, Mode, Recommendation};
use crate::depend::source::{ChannelEvidence, Source, TagStyle};
use crate::depend::target::{Already, Target};
use crate::depend::version::{self, Resolved};
use crate::depend::{self, Channel, Kind, Manager, source, target};
use crate::diagnostic::{Diagnostic, Reason};
use crate::error::RkError;
use crate::output::Output;
use crate::self_depend::Presence;

/// The `rk.depend-assess/1` document.
#[derive(Debug, Serialize)]
struct AssessReport<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// `ready`, `manual-only`, `version-unknown`, or `source-unknown`.
    verdict: &'static str,
    /// What the source declares.
    source: SourceView<'a>,
    /// What the target manages.
    target: TargetView<'a>,
    /// The pin the options render, where the source declares a version.
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved: Option<&'a Resolved>,
    /// Every dev option, in report order.
    dev: &'a [Recommendation],
    /// The prod option, where the source is a library the target can take.
    #[serde(skip_serializing_if = "Option::is_none")]
    prod: Option<&'a Recommendation>,
    /// What plausibly follows.
    next: &'a [String],
}

/// The source half of the assessment.
#[derive(Debug, Serialize)]
struct SourceView<'a> {
    /// The checkout, canonical.
    path: &'a str,
    /// The technology, where a manifest says.
    #[serde(skip_serializing_if = "Option::is_none")]
    tech: Option<&'static str>,
    /// The package name.
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    /// The declared version.
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<&'a str>,
    /// The executables it installs.
    bins: &'a [String],
    /// The forge path.
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_repo: Option<&'a str>,
    /// The remote's host.
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<&'a str>,
    /// The shape of the release tags.
    tag_style: TagStyle,
    /// The viable channels and their evidence.
    channels: &'a [ChannelEvidence],
}

/// The target half of the assessment.
#[derive(Debug, Serialize)]
struct TargetView<'a> {
    /// The target, canonical.
    path: &'a str,
    /// The technology, where a manifest says.
    #[serde(skip_serializing_if = "Option::is_none")]
    tech: Option<&'static str>,
    /// The managers present and their files.
    managers: Vec<ManagerRow<'a>>,
    /// Whether `.envrc` carries `use flake`.
    envrc_use_flake: bool,
    /// Where a manager file already names the dependency.
    already: &'a [Already],
}

/// One present manager.
#[derive(Debug, Serialize)]
struct ManagerRow<'a> {
    /// The manager.
    manager: Manager,
    /// Its file, relative to the target.
    file: &'a str,
}

/// The `rk.depend-add/1` document.
#[derive(Debug, Serialize)]
struct AddReport<'a> {
    /// The shape version of this document.
    schema: &'static str,
    /// `preview` or `apply`.
    mode: &'static str,
    /// `dev` or `prod`.
    kind: Kind,
    /// The manager, for a dev dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    manager: Option<Manager>,
    /// `detected` or `argument`, for a dev dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    manager_origin: Option<&'static str>,
    /// The channel.
    channel: Channel,
    /// `fragment`, `native`, or `manual`.
    landing: Mode,
    /// The target, canonical.
    target: &'a str,
    /// The source, canonical.
    source: &'a str,
    /// The package name.
    name: &'a str,
    /// The bare version.
    version: &'a str,
    /// The release tag.
    tag: &'a str,
    /// `argument` or `source-tree`.
    version_origin: &'static str,
    /// The manager file the fragments go into, for a dev dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    /// Whether that file existed before the run.
    #[serde(skip_serializing_if = "Option::is_none")]
    file_present: Option<Presence>,
    /// The seed file this run wrote, relative to the target; empty in
    /// preview.
    written: &'a [String],
    /// Why an owned file was refused, where one was.
    #[serde(skip_serializing_if = "Option::is_none")]
    refusal: Option<&'a str>,
    /// The fragments, in application order.
    fragments: &'a [Fragment],
    /// The native command, for a prod dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    /// The manual reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'static str>,
    /// The manager's own update verb.
    freshness: &'a str,
    /// What plausibly follows.
    next: &'a [String],
}

/// Dispatch one depend action.
///
/// # Errors
///
/// Returns the action's own failure.
pub fn run(args: &DependArgs) -> Result<(), RkError> {
    match &args.action {
        DependAction::Assess(args) => assess(args),
        DependAction::Add(args) => add(args),
    }
}

/// Read both trees and lay out every option.
fn assess(args: &AssessArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    depend::reject_url(&args.source)?;
    let source = source::observe(&args.source)?;
    let target = target::observe(&args.target, source.name.as_deref())?;
    let resolved = version::resolve(&source, None).ok();
    let (dev, prod) = resolved.as_ref().map_or_else(
        || (Vec::new(), None),
        |resolved| {
            (
                matrix::recommend(&source, &target, Kind::Dev, resolved),
                matrix::recommend(&source, &target, Kind::Prod, resolved)
                    .into_iter()
                    .next(),
            )
        },
    );
    let verdict = verdict(&source, resolved.as_ref(), &dev, prod.as_ref());
    out.result_line(format!(
        "source {}: {} {} {}",
        source.path,
        source.tech.unwrap_or("unknown technology"),
        source.name.as_deref().unwrap_or("(unnamed)"),
        source.version.as_deref().unwrap_or("(no version)")
    ));
    out.result_line(format!(
        "channels: {}",
        list(source.channels.iter().map(|c| c.channel.as_str()))
    ));
    out.result_line(format!(
        "target {}: {}; managers {}",
        target.path,
        target.tech.unwrap_or("unknown technology"),
        list(target.managers.iter().map(|m| m.file.as_str()))
    ));
    for already in &target.already {
        out.result_line(format!(
            "already named in {}:{}",
            already.file, already.line
        ));
    }
    for option in &dev {
        out.result_line(option_line(option));
    }
    if let Some(option) = &prod {
        out.result_line(option_line(option));
    }
    out.result_line(format!("verdict {verdict}"));
    let next = assess_next(verdict, &target, &dev);
    out.next(&next);
    out.emit(&AssessReport {
        schema: "rk.depend-assess/1",
        verdict,
        source: source_view(&source),
        target: target_view(&target),
        resolved: resolved.as_ref(),
        dev: &dev,
        prod: prod.as_ref(),
        next: &next,
    })
}

/// Serve one option; seed the manager file a target lacks under `--apply`.
fn add(args: &AddArgs) -> Result<(), RkError> {
    let out = Output::new(args.json);
    depend::reject_url(&args.source)?;
    let source = source::observe(&args.source)?;
    let target = target::observe(&args.target, source.name.as_deref())?;
    let resolved = version::resolve(&source, args.pin.as_deref())?;
    let options = matrix::recommend(&source, &target, args.kind, &resolved);
    let option = matrix::choose(&options, args.manager, args.channel)?;
    let manager_origin = option.manager.map(|_| {
        if args.manager.is_some() {
            "argument"
        } else {
            "detected"
        }
    });
    let file_present = option
        .file
        .as_deref()
        .map(|file| Presence::of(&target.path.join(file)));
    let mode = if args.apply { "apply" } else { "preview" };
    let (written, refusal) = if args.apply {
        seed_or_refuse(option, &target, file_present)?
    } else {
        (Vec::new(), None)
    };
    let name = source.name.as_deref().unwrap_or_default();
    if args.apply {
        for file in &written {
            out.result_line(format!("wrote {file}"));
        }
    } else {
        out.result_line("DRY RUN: rk depend add prints the fragment or the command; --apply seeds only a manager file the target lacks");
    }
    out.result_line(format!(
        "{name} {} (tag {}, from the {})",
        resolved.version, resolved.tag, resolved.origin
    ));
    render_option(out, option, file_present);
    let next = add_next(option, &target, args.apply, &written);
    out.next(&next);
    out.emit(&AddReport {
        schema: "rk.depend-add/1",
        mode,
        kind: option.kind,
        manager: option.manager,
        manager_origin,
        channel: option.channel,
        landing: option.mode,
        target: target.path.as_str(),
        source: source.path.as_str(),
        name,
        version: &resolved.version,
        tag: &resolved.tag,
        version_origin: resolved.origin,
        file: option.file.as_deref(),
        file_present,
        written: &written,
        refusal: refusal.as_deref(),
        fragments: &option.fragments,
        command: option.command.as_deref(),
        reason: option.reason,
        freshness: &option.freshness,
        next: &next,
    })?;
    if args.apply && option.kind == Kind::Prod {
        return Err(RkError::Usage(
            "rk never edits Cargo.toml, pyproject.toml, or package.json; run the printed command instead of --apply".into(),
        ));
    }
    let Some(message) = refusal else {
        return Ok(());
    };
    Err(RkError::refusal(
        Diagnostic::new(Reason::DestructiveRefusal, message)
            .expected("a target with no file for the manager, or the fragments applied by hand")
            .target_state("nothing was written; the owned file is byte-identical"),
    ))
}

/// Under `--apply`: seed the absent manager file, or name the owned one
/// as the refusal the report carries before the run fails.
fn seed_or_refuse(
    option: &Recommendation,
    target: &Target,
    file_present: Option<Presence>,
) -> Result<(Vec<String>, Option<String>), RkError> {
    match (option.mode, &option.seed, file_present) {
        (Mode::Fragment, Some(seed), Some(Presence::Absent)) => {
            crate::atomic::write(
                target.path.join(&seed.file).as_std_path(),
                seed.text.as_bytes(),
            )?;
            Ok((vec![seed.file.clone()], None))
        }
        (Mode::Fragment, _, _) => Ok((
            Vec::new(),
            Some(format!(
                "the target already carries {}; rk depend add never edits a file the target owns",
                option.file.as_deref().unwrap_or("its manager file")
            )),
        )),
        (Mode::Manual, _, _) => Err(RkError::Usage(format!(
            "the pair is manual ({}); apply the printed text by hand",
            option.reason.unwrap_or("no reason")
        ))),
        (Mode::Native, _, _) => Ok((Vec::new(), None)),
    }
}

/// The human lines of one option: its summary, its file, its fragments
/// with their anchors, and its command.
fn render_option(out: Output, option: &Recommendation, file_present: Option<Presence>) {
    out.result_line(option_line(option));
    if let (Some(file), Some(present)) = (option.file.as_deref(), file_present) {
        out.result_line(match present {
            Presence::Present => {
                format!("{file} present: the target owns it, so the fragments are applied by hand")
            }
            Presence::Absent => format!("{file} absent: --apply seeds it"),
        });
    }
    for fragment in &option.fragments {
        out.result_line(format!(
            "--- {} into {} ({} at {}){}",
            fragment.id,
            fragment.file,
            fragment.placement,
            fragment.anchor.path,
            match fragment.present {
                Some(true) => ": already present",
                Some(false) => ": missing",
                None => ": not judged",
            }
        ));
        out.result_line(&fragment.text);
    }
    if let Some(command) = &option.command {
        out.result_line(format!("run: {command}"));
    }
}

/// The one-line verdict: `ready` where any option lands by fragment or
/// command, `manual-only` where the source is understood but every pair
/// is a hand edit, `version-unknown` where the source declares no
/// version to pin, `source-unknown` where no channel has evidence.
fn verdict(
    source: &Source,
    resolved: Option<&Resolved>,
    dev: &[Recommendation],
    prod: Option<&Recommendation>,
) -> &'static str {
    if source.channels.is_empty() {
        return "source-unknown";
    }
    if resolved.is_none() {
        return "version-unknown";
    }
    let lands = |option: &Recommendation| option.mode != Mode::Manual;
    if dev.iter().any(lands) || prod.is_some_and(lands) {
        "ready"
    } else {
        "manual-only"
    }
}

fn option_line(option: &Recommendation) -> String {
    use std::fmt::Write as _;
    let kind = option
        .manager
        .map_or_else(|| "prod".to_owned(), |m| format!("dev {}", m.as_str()));
    let mut line = format!(
        "{kind} via {}: {}",
        option.channel.as_str(),
        mode_word(option.mode)
    );
    if let Some(reason) = option.reason {
        let _ = write!(line, " ({reason})");
    }
    if let Some(command) = &option.command {
        let _ = write!(line, " {command}");
    }
    if option.manager.is_some() && !option.manager_present {
        line.push_str(" (seeds the file)");
    }
    line
}

const fn mode_word(mode: Mode) -> &'static str {
    match mode {
        Mode::Fragment => "fragment",
        Mode::Native => "native",
        Mode::Manual => "manual",
    }
}

fn list<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let joined: Vec<&str> = items.collect();
    if joined.is_empty() {
        "none".to_owned()
    } else {
        joined.join(", ")
    }
}

fn source_view(source: &Source) -> SourceView<'_> {
    SourceView {
        path: source.path.as_str(),
        tech: source.tech,
        name: source.name.as_deref(),
        version: source.version.as_deref(),
        bins: &source.bins,
        owner_repo: source.owner_repo.as_deref(),
        host: source.host.as_deref(),
        tag_style: source.tag_style,
        channels: &source.channels,
    }
}

fn target_view(target: &Target) -> TargetView<'_> {
    TargetView {
        path: target.path.as_str(),
        tech: target.tech,
        managers: target
            .managers
            .iter()
            .map(|m| ManagerRow {
                manager: m.manager,
                file: &m.file,
            })
            .collect(),
        envrc_use_flake: target.envrc_use_flake,
        already: &target.already,
    }
}

fn assess_next(verdict: &str, target: &Target, dev: &[Recommendation]) -> Vec<String> {
    let mut next = Vec::new();
    match verdict {
        "source-unknown" => {
            next.push("the source declares no channel this binary reads: a Cargo.toml package, a flake with packages, a pyproject project, a package.json, or dist-workspace.toml".to_owned());
        }
        "version-unknown" => {
            next.push(
                "the source declares no version; rk depend add --pin <version> names the release to pin"
                    .to_owned(),
            );
        }
        "manual-only" => {
            next.push(
                "every pair is a hand edit; apply the printed text with the reason in view"
                    .to_owned(),
            );
        }
        _ => {
            if target.managers.len() > 1 {
                next.push("rk depend add --kind dev --manager <manager> (the target carries more than one)".to_owned());
            } else if target.managers.is_empty() && !dev.is_empty() {
                next.push(
                    "rk depend add --kind dev --manager <manager> seeds the file the target lacks"
                        .to_owned(),
                );
            } else {
                next.push("rk depend add --kind dev|prod previews the landing".to_owned());
            }
        }
    }
    next
}

fn add_next(
    option: &Recommendation,
    target: &Target,
    applied: bool,
    written: &[String],
) -> Vec<String> {
    let mut next = Vec::new();
    match option.mode {
        Mode::Native => {
            if let Some(command) = &option.command {
                next.push(format!(
                    "run {command} in the target, then commit the manifest and its lock"
                ));
            }
        }
        Mode::Manual => {
            next.push(format!(
                "apply the printed text by hand: {}",
                option.reason.unwrap_or("manual")
            ));
        }
        Mode::Fragment => {
            if !applied && option.manager_present {
                next.push("apply each fragment at its anchor in the order printed".to_owned());
            } else if !applied {
                next.push("rk depend add --apply seeds the manager file".to_owned());
            }
            if written.iter().any(|f| f == "flake.nix") && !target.envrc_use_flake {
                next.push(
                    "let direnv load the flake from .envrc, or enter the shell with nix develop"
                        .to_owned(),
                );
            }
            if let Some(manager) = option.manager {
                next.push(match manager {
                    Manager::Flake => {
                        "nix flake lock, then commit flake.nix and flake.lock".to_owned()
                    }
                    Manager::Mise => "mise install, then commit the mise configuration".to_owned(),
                    Manager::Asdf => "asdf install, then commit .tool-versions".to_owned(),
                    Manager::Devbox => {
                        "devbox install, then commit devbox.json and devbox.lock".to_owned()
                    }
                });
            }
        }
    }
    if !option.freshness.is_empty() {
        next.push(format!(
            "freshness is the manager's own verb: {}",
            option.freshness
        ));
    }
    next
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::{AddReport, AssessReport, ManagerRow, SourceView, TargetView};
    use crate::depend::fragments::{Anchor, Fragment};
    use crate::depend::matrix::{Mode, Recommendation, Seed};
    use crate::depend::source::{ChannelEvidence, TagStyle};
    use crate::depend::target::Already;
    use crate::depend::version::Resolved;
    use crate::depend::{Channel, Kind, Manager};
    use crate::self_depend::Presence;

    fn fragment() -> Fragment {
        Fragment {
            id: "mise-tool",
            file: "mise.toml".to_owned(),
            role: "the pinned tool entry",
            placement: "insert-into-table",
            anchor: Anchor {
                kind: "table",
                path: "tools".to_owned(),
                needle: Some("[tools]"),
            },
            text: "\"cargo:sample-tool\" = \"1.4.0\"".to_owned(),
            present: Some(false),
        }
    }

    /// The complete `rk.depend-assess/1` shape, held by snapshot.
    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the snapshot is one literal shape, and splitting it would hide what the schema holds"
    )]
    fn the_depend_assess_schema_snapshot_holds() {
        let channels = vec![ChannelEvidence {
            channel: Channel::Crates,
            evidence: vec!["Cargo.toml names a package".to_owned()],
        }];
        let bins = vec!["sam".to_owned()];
        let already = vec![Already {
            manager: Manager::Mise,
            file: "mise.toml".to_owned(),
            line: 3,
        }];
        let resolved = Resolved {
            version: "1.4.0".to_owned(),
            tag: "v1.4.0".to_owned(),
            origin: "source-tree",
        };
        let dev = vec![Recommendation {
            kind: Kind::Dev,
            manager: Some(Manager::Mise),
            manager_present: true,
            channel: Channel::Crates,
            mode: Mode::Fragment,
            file: Some("mise.toml".to_owned()),
            fragments: vec![fragment()],
            seed: None,
            command: None,
            reason: None,
            freshness: "mise upgrade --bump cargo:sample-tool".to_owned(),
        }];
        let prod = Recommendation {
            kind: Kind::Prod,
            manager: None,
            manager_present: false,
            channel: Channel::Crates,
            mode: Mode::Native,
            file: None,
            fragments: Vec::new(),
            seed: None,
            command: Some("cargo add sample-tool@1.4.0".to_owned()),
            reason: None,
            freshness: "cargo update -p sample-tool".to_owned(),
        };
        let next = vec!["rk depend add --kind dev|prod previews the landing".to_owned()];
        let report = AssessReport {
            schema: "rk.depend-assess/1",
            verdict: "ready",
            source: SourceView {
                path: "/srv/sample",
                tech: Some("rust"),
                name: Some("sample-tool"),
                version: Some("1.4.0"),
                bins: &bins,
                owner_repo: Some("acme/sample-tool"),
                host: Some("github.com"),
                tag_style: TagStyle::Prefixed,
                channels: &channels,
            },
            target: TargetView {
                path: "/srv/widget",
                tech: Some("rust"),
                managers: vec![ManagerRow {
                    manager: Manager::Mise,
                    file: "mise.toml",
                }],
                envrc_use_flake: false,
                already: &already,
            },
            resolved: Some(&resolved),
            dev: &dev,
            prod: Some(&prod),
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.depend-assess/1","verdict":"ready","source":{"path":"/srv/sample","tech":"rust","name":"sample-tool","version":"1.4.0","bins":["sam"],"owner_repo":"acme/sample-tool","host":"github.com","tag_style":"prefixed","channels":[{"channel":"crates","evidence":["Cargo.toml names a package"]}]},"target":{"path":"/srv/widget","tech":"rust","managers":[{"manager":"mise","file":"mise.toml"}],"envrc_use_flake":false,"already":[{"manager":"mise","file":"mise.toml","line":3}]},"resolved":{"version":"1.4.0","tag":"v1.4.0","origin":"source-tree"},"dev":[{"kind":"dev","manager":"mise","manager_present":true,"channel":"crates","mode":"fragment","file":"mise.toml","fragments":[{"id":"mise-tool","file":"mise.toml","role":"the pinned tool entry","placement":"insert-into-table","anchor":{"kind":"table","path":"tools","needle":"[tools]"},"text":"\"cargo:sample-tool\" = \"1.4.0\"","present":false}],"freshness":"mise upgrade --bump cargo:sample-tool"}],"prod":{"kind":"prod","manager_present":false,"channel":"crates","mode":"native","fragments":[],"command":"cargo add sample-tool@1.4.0","freshness":"cargo update -p sample-tool"},"next":["rk depend add --kind dev|prod previews the landing"]}"#
        );
        let bare = AssessReport {
            schema: "rk.depend-assess/1",
            verdict: "source-unknown",
            source: SourceView {
                path: "/srv/sample",
                tech: None,
                name: None,
                version: None,
                bins: &[],
                owner_repo: None,
                host: None,
                tag_style: TagStyle::Unknown,
                channels: &[],
            },
            target: TargetView {
                path: "/srv/widget",
                tech: None,
                managers: Vec::new(),
                envrc_use_flake: false,
                already: &[],
            },
            resolved: None,
            dev: &[],
            prod: None,
            next: &[],
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("a report serializes"),
            r#"{"schema":"rk.depend-assess/1","verdict":"source-unknown","source":{"path":"/srv/sample","bins":[],"tag_style":"unknown","channels":[]},"target":{"path":"/srv/widget","managers":[],"envrc_use_flake":false,"already":[]},"dev":[],"next":[]}"#,
            "an unknown value is omitted, never null"
        );
    }

    /// The complete `rk.depend-add/1` shape, held by snapshot.
    #[test]
    fn the_depend_add_schema_snapshot_holds() {
        let fragments = vec![fragment()];
        let written = vec!["mise.toml".to_owned()];
        let next = vec!["mise install, then commit the mise configuration".to_owned()];
        let report = AddReport {
            schema: "rk.depend-add/1",
            mode: "apply",
            kind: Kind::Dev,
            manager: Some(Manager::Mise),
            manager_origin: Some("detected"),
            channel: Channel::Crates,
            landing: Mode::Fragment,
            target: "/srv/widget",
            source: "/srv/sample",
            name: "sample-tool",
            version: "1.4.0",
            tag: "v1.4.0",
            version_origin: "source-tree",
            file: Some("mise.toml"),
            file_present: Some(Presence::Absent),
            written: &written,
            refusal: None,
            fragments: &fragments,
            command: None,
            reason: None,
            freshness: "mise upgrade --bump cargo:sample-tool",
            next: &next,
        };
        assert_eq!(
            serde_json::to_string(&report).expect("a report serializes"),
            r#"{"schema":"rk.depend-add/1","mode":"apply","kind":"dev","manager":"mise","manager_origin":"detected","channel":"crates","landing":"fragment","target":"/srv/widget","source":"/srv/sample","name":"sample-tool","version":"1.4.0","tag":"v1.4.0","version_origin":"source-tree","file":"mise.toml","file_present":"absent","written":["mise.toml"],"fragments":[{"id":"mise-tool","file":"mise.toml","role":"the pinned tool entry","placement":"insert-into-table","anchor":{"kind":"table","path":"tools","needle":"[tools]"},"text":"\"cargo:sample-tool\" = \"1.4.0\"","present":false}],"freshness":"mise upgrade --bump cargo:sample-tool","next":["mise install, then commit the mise configuration"]}"#
        );
        let bare = AddReport {
            schema: "rk.depend-add/1",
            mode: "preview",
            kind: Kind::Prod,
            manager: None,
            manager_origin: None,
            channel: Channel::Crates,
            landing: Mode::Native,
            target: "/srv/widget",
            source: "/srv/sample",
            name: "sample-tool",
            version: "1.4.0",
            tag: "v1.4.0",
            version_origin: "argument",
            file: None,
            file_present: None,
            written: &[],
            refusal: None,
            fragments: &[],
            command: Some("cargo add sample-tool@1.4.0"),
            reason: None,
            freshness: "cargo update -p sample-tool",
            next: &[],
        };
        assert_eq!(
            serde_json::to_string(&bare).expect("a report serializes"),
            r#"{"schema":"rk.depend-add/1","mode":"preview","kind":"prod","channel":"crates","landing":"native","target":"/srv/widget","source":"/srv/sample","name":"sample-tool","version":"1.4.0","tag":"v1.4.0","version_origin":"argument","written":[],"fragments":[],"command":"cargo add sample-tool@1.4.0","freshness":"cargo update -p sample-tool","next":[]}"#,
            "an unknown value is omitted, never null"
        );
        let seed = Seed {
            file: "mise.toml".to_owned(),
            text: "[tools]\n".to_owned(),
        };
        assert_eq!(
            serde_json::to_string(&seed).expect("a seed serializes"),
            r#"{"file":"mise.toml","text":"[tools]\n"}"#
        );
        let _ = Utf8PathBuf::from("/srv/widget");
    }
}
