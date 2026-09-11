//! The manager × channel matrix: which pairs land as a fragment, which
//! as the technology's own command, and which are a hand edit with a
//! named reason.
//!
//! The matrix guesses nothing it cannot read offline: a nixpkgs
//! attribute, an asdf plugin name, a source hash are unknown here, so
//! the pairs that need one are `manual` with that reason, and the
//! operator or the agent finishes them from the report.

use serde::Serialize;

use super::fragments::{self, Anchor, Fragment, Tokens};
use super::source::Source;
use super::target::Target;
use super::version::Resolved;
use super::{Channel, Kind, Manager};
use crate::error::RkError;

/// How a pair lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// Text to place in the manager's file, seeded where the file is absent.
    Fragment,
    /// The technology's own command, run by the operator.
    Native,
    /// A hand edit the report describes; nothing is written.
    Manual,
}

/// Whether a pair is supported, and why not where it is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// The pair renders a fragment.
    Fragment,
    /// The pair needs knowledge the binary does not have offline.
    Manual(&'static str),
}

/// A seed file for a manager the target has no file for.
#[derive(Debug, Clone, Serialize)]
pub struct Seed {
    /// The file, relative to the target.
    pub file: String,
    /// Its whole text.
    pub text: String,
}

/// One way the dependency can land.
#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    /// `dev` or `prod`.
    pub kind: Kind,
    /// The manager, for a dev dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manager: Option<Manager>,
    /// Whether the target already carries the manager's file.
    pub manager_present: bool,
    /// The channel.
    pub channel: Channel,
    /// `fragment`, `native`, or `manual`.
    pub mode: Mode,
    /// The manager file the fragments go into, for a dev dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The fragments, in application order.
    pub fragments: Vec<Fragment>,
    /// The seed, where the manager file is absent and the pair renders one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<Seed>,
    /// The native command, for a prod dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// The manual reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<&'static str>,
    /// The manager's own update verb.
    pub freshness: String,
}

/// The support of one pair.
#[must_use]
pub const fn support(manager: Manager, channel: Channel) -> Support {
    match (manager, channel) {
        (Manager::Flake | Manager::Devbox, Channel::Flake)
        | (
            Manager::Mise,
            Channel::Crates | Channel::Pypi | Channel::Npm | Channel::GithubRelease,
        ) => Support::Fragment,
        (Manager::Flake, Channel::GithubRelease) => Support::Manual("network-hash-needed"),
        (Manager::Flake | Manager::Devbox, _) => Support::Manual("nixpkgs-attribute-unknown"),
        (Manager::Mise, Channel::Flake) => Support::Manual("no-mise-flake-backend"),
        (Manager::Asdf, _) => Support::Manual("asdf-plugin-unknown"),
    }
}

/// The channels a manager prefers, first first.
#[must_use]
pub const fn preference(manager: Manager) -> [Channel; 5] {
    match manager {
        Manager::Flake | Manager::Devbox => [
            Channel::Flake,
            Channel::Crates,
            Channel::GithubRelease,
            Channel::Pypi,
            Channel::Npm,
        ],
        Manager::Mise | Manager::Asdf => [
            Channel::Crates,
            Channel::GithubRelease,
            Channel::Pypi,
            Channel::Npm,
            Channel::Flake,
        ],
    }
}

/// Every way the dependency can land as the given kind, in report order.
#[must_use]
pub fn recommend(
    source: &Source,
    target: &Target,
    kind: Kind,
    resolved: &Resolved,
) -> Vec<Recommendation> {
    match kind {
        Kind::Dev => dev_options(source, target, resolved),
        Kind::Prod => prod_option(source, target, resolved).into_iter().collect(),
    }
}

/// Pick the one option `add` serves, from the flags and the target.
///
/// # Errors
///
/// Returns [`RkError::Usage`] where the manager is ambiguous or the pair
/// is not viable, naming the choices.
pub fn choose(
    options: &[Recommendation],
    manager: Option<Manager>,
    channel: Option<Channel>,
) -> Result<&Recommendation, RkError> {
    let Some(first) = options.first() else {
        return Err(RkError::Usage(
            "the source declares no distribution channel; nothing can land".into(),
        ));
    };
    if first.kind == Kind::Prod {
        return Ok(first);
    }
    let manager = match manager {
        Some(manager) => manager,
        None => detected_manager(options)?,
    };
    let viable: Vec<&Recommendation> = options
        .iter()
        .filter(|o| o.manager == Some(manager))
        .collect();
    if viable.is_empty() && options.iter().any(|o| o.manager_present) {
        let names: Vec<&str> = options
            .iter()
            .filter(|o| o.manager_present)
            .filter_map(|o| o.manager.map(Manager::as_str))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        return Err(RkError::Usage(format!(
            "the target carries no {} file; its managers are {}, and a second manager for one tool is two pins",
            manager.as_str(),
            names.join(", ")
        )));
    }
    let Some(channel) = channel else {
        return viable.first().copied().ok_or_else(|| {
            RkError::Usage(format!(
                "the source offers no channel {} can take",
                manager.as_str()
            ))
        });
    };
    viable
        .iter()
        .find(|o| o.channel == channel)
        .copied()
        .ok_or_else(|| {
            let names: Vec<&str> = viable.iter().map(|o| o.channel.as_str()).collect();
            RkError::Usage(format!(
                "{} is not a channel the source offers for {}; the viable channels are {}",
                channel.as_str(),
                manager.as_str(),
                names.join(", ")
            ))
        })
}

/// The one manager the target carries, or the usage error naming why
/// `--manager` is needed.
fn detected_manager(options: &[Recommendation]) -> Result<Manager, RkError> {
    let present: Vec<Manager> = Manager::ALL
        .into_iter()
        .filter(|m| {
            options
                .iter()
                .any(|o| o.manager == Some(*m) && o.manager_present)
        })
        .collect();
    match present.as_slice() {
        [one] => Ok(*one),
        [] => Err(RkError::Usage(
            "the target carries no tool manager file; pass --manager to seed one".into(),
        )),
        many => {
            let names: Vec<&str> = many.iter().map(|m| m.as_str()).collect();
            Err(RkError::Usage(format!(
                "the target carries {}; pass --manager to choose",
                names.join(" and ")
            )))
        }
    }
}

/// The tokens every block renders from.
fn tokens(source: &Source, resolved: &Resolved) -> Tokens {
    let name = source.name.clone().unwrap_or_default();
    Tokens {
        input: fragments::nix_input_name(&name),
        name,
        version: resolved.version.clone(),
        tag: resolved.tag.clone(),
        owner_repo: source.owner_repo.clone(),
        bin: source.bin().map(str::to_owned),
        flake_ref: fragments::flake_ref(
            source.host.as_deref(),
            source.owner_repo.as_deref(),
            &resolved.tag,
        ),
        tool_line: None,
    }
}

fn dev_options(source: &Source, target: &Target, resolved: &Resolved) -> Vec<Recommendation> {
    let managers: Vec<(Manager, bool)> = if target.managers.is_empty() {
        Manager::ALL.into_iter().map(|m| (m, false)).collect()
    } else {
        target.managers.iter().map(|m| (m.manager, true)).collect()
    };
    let tokens = tokens(source, resolved);
    let mut out = Vec::new();
    for (manager, present) in managers {
        let file = target.file_of(manager);
        let file_name = file.map_or_else(
            || Target::default_file(manager).to_owned(),
            |f| f.file.clone(),
        );
        let text = file.map(|f| f.text.as_str());
        for channel in preference(manager) {
            if !source.has(channel) {
                continue;
            }
            let mut option = Recommendation {
                kind: Kind::Dev,
                manager: Some(manager),
                manager_present: present,
                channel,
                mode: Mode::Manual,
                file: Some(file_name.clone()),
                fragments: Vec::new(),
                seed: None,
                command: None,
                reason: None,
                freshness: freshness(manager, channel, &tokens),
            };
            match support(manager, channel) {
                Support::Manual(reason) => {
                    option.reason = Some(reason);
                    if manager == Manager::Asdf {
                        option.fragments = vec![asdf_fragment(&tokens, text)];
                    }
                }
                Support::Fragment if channel == Channel::Flake && tokens.flake_ref.is_none() => {
                    option.reason = Some("forge-undetected");
                }
                Support::Fragment
                    if manager == Manager::Flake && input_name_taken(&tokens, text) =>
                {
                    option.reason = Some("flake-input-name-taken");
                }
                Support::Fragment => {
                    option.mode = Mode::Fragment;
                    let (fragments, seed) = match manager {
                        Manager::Flake => flake_fragments(&tokens, &file_name, text),
                        Manager::Mise => mise_fragment(channel, &tokens, &file_name, text),
                        Manager::Devbox => devbox_fragment(&tokens, &file_name, text),
                        Manager::Asdf => (Vec::new(), None),
                    };
                    option.fragments = fragments;
                    option.seed = (!present).then_some(seed).flatten();
                }
            }
            out.push(option);
        }
    }
    out
}

fn prod_option(source: &Source, target: &Target, resolved: &Resolved) -> Option<Recommendation> {
    let name = source.name.as_deref()?;
    let tech = target.tech?;
    let version = &resolved.version;
    let mut option = Recommendation {
        kind: Kind::Prod,
        manager: None,
        manager_present: false,
        channel: source.channels.first()?.channel,
        mode: Mode::Native,
        file: None,
        fragments: Vec::new(),
        seed: None,
        command: None,
        reason: None,
        freshness: String::new(),
    };
    let native = match tech {
        "rust" if source.has(Channel::Crates) => Some((
            Channel::Crates,
            format!("cargo add {name}@{version}"),
            format!("cargo update -p {name}"),
        )),
        "python" if source.has(Channel::Pypi) => Some((
            Channel::Pypi,
            format!("uv add \"{name}=={version}\""),
            format!("uv lock --upgrade-package {name}"),
        )),
        "node" if source.has(Channel::Npm) => Some((
            Channel::Npm,
            format!("npm install {name}@{version}"),
            format!("npm update {name}"),
        )),
        _ => None,
    };
    if let Some((channel, command, freshness)) = native {
        option.channel = channel;
        option.command = Some(command);
        option.freshness = freshness;
    } else {
        option.mode = Mode::Manual;
        option.reason = Some("technology-mismatch");
    }
    Some(option)
}

fn freshness(manager: Manager, channel: Channel, tokens: &Tokens) -> String {
    let name = &tokens.name;
    match manager {
        Manager::Flake => format!("nix flake update {}", tokens.input),
        Manager::Mise => {
            let id = match channel {
                Channel::Crates => format!("cargo:{name}"),
                Channel::Pypi => format!("pipx:{name}"),
                Channel::Npm => format!("npm:{name}"),
                Channel::GithubRelease => {
                    format!("ubi:{}", tokens.owner_repo.clone().unwrap_or_default())
                }
                Channel::Flake => name.clone(),
            };
            format!("mise upgrade --bump {id}")
        }
        Manager::Asdf => format!("edit the {name} line in .tool-versions, then asdf install"),
        Manager::Devbox => "devbox update".to_owned(),
    }
}

/// Whether the target's flake already binds the input name to another
/// source: the alias is derived from the package name, so two names can
/// share it, and a binding whose URL is not this repository's is a
/// conflict the report names rather than a presence.
fn input_name_taken(tokens: &Tokens, text: Option<&str>) -> bool {
    let Some(text) = text else {
        return false;
    };
    let Some(body) = fragments::input_binding(text, &tokens.input) else {
        return false;
    };
    let ours = tokens
        .flake_ref
        .as_deref()
        .and_then(|reference| reference.rsplit_once('/'))
        .map_or_else(String::new, |(prefix, _)| format!("{prefix}/"));
    !body.contains(&ours)
}

fn flake_fragments(
    tokens: &Tokens,
    file: &str,
    text: Option<&str>,
) -> (Vec<Fragment>, Option<Seed>) {
    let input = tokens.input.as_str();
    let package_prefix = format!("{input}.packages.");
    let fragments = vec![
        Fragment {
            id: "flake-input",
            file: file.to_owned(),
            role: "the pinned input",
            placement: "insert-into-attrset",
            anchor: Anchor {
                kind: "attrset",
                path: "inputs".to_owned(),
                needle: text.and_then(|t| fragments::first_found(t, &["inputs = {", "inputs ="])),
            },
            text: fragments::fragment("depend-flake-input.nix.in", tokens),
            present: Some(text.is_some_and(|t| fragments::input_binding(t, input).is_some())),
        },
        Fragment {
            id: "outputs-argument",
            file: file.to_owned(),
            role: "the input as an argument of the outputs function",
            placement: "add-to-function-head",
            anchor: Anchor {
                kind: "function-head",
                path: "outputs".to_owned(),
                needle: text.and_then(|t| fragments::first_found(t, &["outputs =", "outputs"])),
            },
            text: fragments::fragment("depend-flake-outputs-arg.nix.in", tokens),
            present: text.map_or(Some(false), |t| {
                fragments::outputs_argument_present(t, input)
            }),
        },
        Fragment {
            id: "devshell-package",
            file: file.to_owned(),
            role: "the package in the default devshell",
            placement: "append-to-list",
            anchor: Anchor {
                kind: "list",
                path: "devShells.<system>.default.packages".to_owned(),
                needle: text
                    .and_then(|t| fragments::first_found(t, &["packages = [", "devShells"])),
            },
            text: fragments::fragment("depend-flake-package.nix.in", tokens),
            present: text.map_or(Some(false), |t| {
                if t.contains(&package_prefix) {
                    Some(true)
                } else {
                    t.contains("devShells").then_some(false)
                }
            }),
        },
    ];
    let seed = Seed {
        file: file.to_owned(),
        text: fragments::seed("depend-seed-flake.nix.in", tokens),
    };
    (fragments, Some(seed))
}

fn mise_fragment(
    channel: Channel,
    tokens: &Tokens,
    file: &str,
    text: Option<&str>,
) -> (Vec<Fragment>, Option<Seed>) {
    let block = match channel {
        Channel::GithubRelease => "depend-mise-ubi.toml.in",
        Channel::Pypi => "depend-mise-pipx.toml.in",
        Channel::Npm => "depend-mise-npm.toml.in",
        Channel::Crates | Channel::Flake => "depend-mise-cargo.toml.in",
    };
    let line = fragments::fragment(block, tokens);
    let key = line.split(" = ").next().unwrap_or(&line).to_owned();
    let seed_tokens = Tokens {
        tool_line: Some(line.clone()),
        ..tokens.clone()
    };
    let fragment = Fragment {
        id: "mise-tool",
        file: file.to_owned(),
        role: "the pinned tool entry",
        placement: "insert-into-table",
        anchor: Anchor {
            kind: "table",
            path: "tools".to_owned(),
            needle: text.and_then(|t| fragments::first_found(t, &["[tools]"])),
        },
        text: line,
        present: Some(text.is_some_and(|t| t.contains(&key))),
    };
    let seed = Seed {
        file: file.to_owned(),
        text: fragments::seed("depend-seed-mise.toml.in", &seed_tokens),
    };
    (vec![fragment], Some(seed))
}

fn devbox_fragment(
    tokens: &Tokens,
    file: &str,
    text: Option<&str>,
) -> (Vec<Fragment>, Option<Seed>) {
    let reference = tokens.flake_ref.clone().unwrap_or_default();
    let fragment = Fragment {
        id: "devbox-package",
        file: file.to_owned(),
        role: "the flake package entry",
        placement: "append-to-array",
        anchor: Anchor {
            kind: "array",
            path: "packages".to_owned(),
            needle: text.and_then(|t| fragments::first_found(t, &["\"packages\""])),
        },
        text: fragments::fragment("depend-devbox-flake.json.in", tokens),
        present: Some(text.is_some_and(|t| t.contains(&reference))),
    };
    let seed = Seed {
        file: file.to_owned(),
        text: fragments::seed("depend-seed-devbox.json.in", tokens),
    };
    (vec![fragment], Some(seed))
}

fn asdf_fragment(tokens: &Tokens, text: Option<&str>) -> Fragment {
    let prefix = format!("{} ", tokens.name);
    Fragment {
        id: "asdf-line",
        file: ".tool-versions".to_owned(),
        role: "the line the plugin would take, once the plugin is known",
        placement: "append-line",
        anchor: Anchor {
            kind: "file",
            path: ".tool-versions".to_owned(),
            needle: None,
        },
        text: fragments::fragment("depend-asdf-line.in", tokens),
        present: Some(text.is_some_and(|t| t.lines().any(|l| l.starts_with(&prefix)))),
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::{Channel, Kind, Manager, Mode, Support, choose, preference, recommend, support};
    use crate::depend::source::{ChannelEvidence, Source, TagStyle};
    use crate::depend::target::{ManagerFile, Target};
    use crate::depend::version::Resolved;
    use crate::error::RkError;

    fn source(channels: &[Channel]) -> Source {
        Source {
            path: Utf8PathBuf::from("/srv/sample"),
            tech: Some("rust"),
            name: Some("sample-tool".into()),
            version: Some("1.4.0".into()),
            bins: vec!["sam".into()],
            owner_repo: Some("acme/sample-tool".into()),
            host: Some("github.com".into()),
            flake_package: channels.contains(&Channel::Flake),
            dist_github: channels.contains(&Channel::GithubRelease),
            binstall_github: false,
            tag_style: TagStyle::Prefixed,
            channels: channels
                .iter()
                .map(|c| ChannelEvidence {
                    channel: *c,
                    evidence: Vec::new(),
                })
                .collect(),
        }
    }

    fn target(tech: Option<&'static str>, managers: &[(Manager, &str, &str)]) -> Target {
        Target {
            path: Utf8PathBuf::from("/srv/widget"),
            tech,
            managers: managers
                .iter()
                .map(|(manager, file, text)| ManagerFile {
                    manager: *manager,
                    file: (*file).to_owned(),
                    text: (*text).to_owned(),
                })
                .collect(),
            envrc_use_flake: false,
            already: Vec::new(),
        }
    }

    fn resolved() -> Resolved {
        Resolved {
            version: "1.4.0".into(),
            tag: "v1.4.0".into(),
            origin: "source-tree",
        }
    }

    /// SATISFIES dependencies:an-unjudgeable-pair-is-manual-with-its-reason
    #[test]
    fn every_pair_in_the_matrix_is_classified_once() {
        let mut fragment_pairs = 0;
        for manager in Manager::ALL {
            let mut seen = Vec::new();
            for channel in preference(manager) {
                assert!(
                    !seen.contains(&channel),
                    "{manager:?} lists {channel:?} once"
                );
                seen.push(channel);
                match support(manager, channel) {
                    Support::Fragment => fragment_pairs += 1,
                    Support::Manual(reason) => {
                        assert!(
                            !reason.is_empty(),
                            "{manager:?}/{channel:?} names its reason"
                        );
                    }
                }
            }
            assert_eq!(
                seen.len(),
                Channel::ALL.len(),
                "{manager:?} covers every channel"
            );
        }
        assert_eq!(
            fragment_pairs, 6,
            "flake, devbox, and four mise pairs render"
        );
    }

    #[test]
    fn a_cargo_dist_source_offers_crates_before_the_archive_on_mise() {
        let options = recommend(
            &source(&[Channel::Crates, Channel::GithubRelease]),
            &target(None, &[(Manager::Mise, "mise.toml", "[tools]\n")]),
            Kind::Dev,
            &resolved(),
        );
        let channels: Vec<Channel> = options.iter().map(|o| o.channel).collect();
        assert_eq!(channels, [Channel::Crates, Channel::GithubRelease]);
        assert!(options.iter().all(|o| o.mode == Mode::Fragment));
        assert_eq!(
            options[0].fragments[0].text,
            "\"cargo:sample-tool\" = \"1.4.0\""
        );
        assert_eq!(options[0].fragments[0].anchor.needle, Some("[tools]"));
        assert!(options[0].seed.is_none(), "a present file is never seeded");
        assert_eq!(
            options[1].fragments[0].text,
            "\"ubi:acme/sample-tool\" = { version = \"1.4.0\", exe = \"sam\" }"
        );
        assert_eq!(
            options[1].freshness,
            "mise upgrade --bump ubi:acme/sample-tool"
        );
    }

    #[test]
    fn a_flake_source_is_the_only_fragment_channel_for_flake_and_devbox() {
        let source = source(&[Channel::Crates, Channel::Flake]);
        let flake_target = target(
            None,
            &[(
                Manager::Flake,
                "flake.nix",
                "{ inputs = {}; outputs = { self }: {}; }",
            )],
        );
        let options = recommend(&source, &flake_target, Kind::Dev, &resolved());
        assert_eq!(options[0].channel, Channel::Flake);
        assert_eq!(options[0].mode, Mode::Fragment);
        assert_eq!(options[0].fragments.len(), 3);
        assert_eq!(options[0].fragments[0].present, Some(false));
        assert_eq!(options[1].channel, Channel::Crates);
        assert_eq!(options[1].mode, Mode::Manual);
        assert_eq!(options[1].reason, Some("nixpkgs-attribute-unknown"));
        let devbox = recommend(
            &source,
            &target(
                None,
                &[(Manager::Devbox, "devbox.json", "{\"packages\": []}")],
            ),
            Kind::Dev,
            &resolved(),
        );
        assert_eq!(devbox[0].mode, Mode::Fragment);
        assert_eq!(
            devbox[0].fragments[0].text,
            "\"github:acme/sample-tool/v1.4.0#default\""
        );
        assert_eq!(devbox[0].fragments[0].anchor.needle, Some("\"packages\""));
        let taken = target(
            None,
            &[(
                Manager::Flake,
                "flake.nix",
                "{ inputs = { sample-tool = { url = \"github:other/thing/v9\"; }; }; outputs = { self, sample-tool }: {}; }",
            )],
        );
        let conflict = recommend(&source, &taken, Kind::Dev, &resolved());
        assert_eq!(conflict[0].mode, Mode::Manual);
        assert_eq!(conflict[0].reason, Some("flake-input-name-taken"));
        let dotted = target(
            None,
            &[(
                Manager::Flake,
                "flake.nix",
                "{ inputs.sample-tool.url = \"github:other/thing/v9\"; outputs = { self, sample-tool }: {}; }",
            )],
        );
        let dotted_conflict = recommend(&source, &dotted, Kind::Dev, &resolved());
        assert_eq!(dotted_conflict[0].reason, Some("flake-input-name-taken"));
        let ours = target(
            None,
            &[(
                Manager::Flake,
                "flake.nix",
                "{ inputs = { sample-tool = { url = \"github:acme/sample-tool/v1.3.0\"; }; }; outputs = { self, sample-tool }: { devShells = {}; }; }",
            )],
        );
        let present = recommend(&source, &ours, Kind::Dev, &resolved());
        assert_eq!(present[0].mode, Mode::Fragment);
        assert_eq!(present[0].fragments[0].present, Some(true));
        assert_eq!(present[0].fragments[1].present, Some(true));
        let mut foreign = source;
        foreign.host = Some("codeberg.org".into());
        foreign.channels.retain(|c| c.channel == Channel::Flake);
        let unknown = recommend(&foreign, &flake_target, Kind::Dev, &resolved());
        assert_eq!(unknown[0].mode, Mode::Manual);
        assert_eq!(unknown[0].reason, Some("forge-undetected"));
    }

    /// SATISFIES dependencies:an-unjudgeable-pair-is-manual-with-its-reason
    #[test]
    fn asdf_is_always_manual_with_its_reason() {
        let options = recommend(
            &source(&[Channel::Crates, Channel::Flake, Channel::GithubRelease]),
            &target(
                None,
                &[(Manager::Asdf, ".tool-versions", "nodejs 24.0.0\n")],
            ),
            Kind::Dev,
            &resolved(),
        );
        assert_eq!(options.len(), 3);
        for option in &options {
            assert_eq!(option.mode, Mode::Manual);
            assert_eq!(option.reason, Some("asdf-plugin-unknown"));
            assert_eq!(option.fragments[0].text, "sample-tool 1.4.0");
            assert!(option.seed.is_none());
        }
    }

    #[test]
    fn a_target_with_no_manager_lists_every_manager_as_a_seed() {
        let options = recommend(
            &source(&[Channel::Crates]),
            &target(None, &[]),
            Kind::Dev,
            &resolved(),
        );
        let managers: Vec<Manager> = options.iter().filter_map(|o| o.manager).collect();
        assert_eq!(managers, Manager::ALL);
        assert!(options.iter().all(|o| !o.manager_present));
        let mise = options
            .iter()
            .find(|o| o.manager == Some(Manager::Mise))
            .expect("mise");
        assert_eq!(mise.file.as_deref(), Some("mise.toml"));
        assert_eq!(
            mise.seed.as_ref().map(|s| s.text.as_str()),
            Some("[tools]\n\"cargo:sample-tool\" = \"1.4.0\"\n")
        );
    }

    /// SATISFIES dependencies:a-prod-dependency-lands-through-the-native-command
    #[test]
    fn prod_returns_the_native_command_per_technology() {
        let rust = recommend(
            &source(&[Channel::Crates]),
            &target(Some("rust"), &[]),
            Kind::Prod,
            &resolved(),
        );
        assert_eq!(rust[0].mode, Mode::Native);
        assert_eq!(
            rust[0].command.as_deref(),
            Some("cargo add sample-tool@1.4.0")
        );
        assert_eq!(rust[0].freshness, "cargo update -p sample-tool");
        let mut python = source(&[Channel::Pypi]);
        python.tech = Some("python");
        let py = recommend(
            &python,
            &target(Some("python"), &[]),
            Kind::Prod,
            &resolved(),
        );
        assert_eq!(
            py[0].command.as_deref(),
            Some("uv add \"sample-tool==1.4.0\"")
        );
        let mut node = source(&[Channel::Npm]);
        node.tech = Some("node");
        let js = recommend(&node, &target(Some("node"), &[]), Kind::Prod, &resolved());
        assert_eq!(
            js[0].command.as_deref(),
            Some("npm install sample-tool@1.4.0")
        );
    }

    #[test]
    fn a_technology_mismatch_is_manual_for_prod() {
        let options = recommend(
            &source(&[Channel::Crates]),
            &target(Some("python"), &[]),
            Kind::Prod,
            &resolved(),
        );
        assert_eq!(options[0].mode, Mode::Manual);
        assert_eq!(options[0].reason, Some("technology-mismatch"));
        assert!(options[0].command.is_none());
        let mut nameless = source(&[]);
        nameless.name = None;
        assert!(
            recommend(
                &nameless,
                &target(Some("rust"), &[]),
                Kind::Prod,
                &resolved()
            )
            .is_empty()
        );
    }

    #[test]
    fn one_present_manager_is_chosen_without_a_flag() {
        let options = recommend(
            &source(&[Channel::Crates, Channel::GithubRelease]),
            &target(None, &[(Manager::Mise, "mise.toml", "")]),
            Kind::Dev,
            &resolved(),
        );
        let chosen = choose(&options, None, None).expect("chooses");
        assert_eq!(chosen.manager, Some(Manager::Mise));
        assert_eq!(chosen.channel, Channel::Crates);
        let archive = choose(&options, None, Some(Channel::GithubRelease)).expect("chooses");
        assert_eq!(archive.channel, Channel::GithubRelease);
        assert!(matches!(
            choose(&options, None, Some(Channel::Pypi)),
            Err(RkError::Usage(_))
        ));
        assert!(matches!(
            choose(&options, Some(Manager::Flake), None),
            Err(RkError::Usage(_))
        ));
    }

    #[test]
    fn two_present_managers_need_the_flag() {
        let options = recommend(
            &source(&[Channel::Crates]),
            &target(
                None,
                &[
                    (Manager::Flake, "flake.nix", ""),
                    (Manager::Mise, "mise.toml", ""),
                ],
            ),
            Kind::Dev,
            &resolved(),
        );
        let message = match choose(&options, None, None) {
            Err(RkError::Usage(message)) => message,
            other => format!("two managers need --manager: {other:?}"),
        };
        assert!(message.contains("flake and mise"), "{message}");
        assert_eq!(
            choose(&options, Some(Manager::Mise), None)
                .expect("chooses")
                .manager,
            Some(Manager::Mise)
        );
        let none = recommend(
            &source(&[Channel::Crates]),
            &target(None, &[]),
            Kind::Dev,
            &resolved(),
        );
        assert!(matches!(choose(&none, None, None), Err(RkError::Usage(_))));
        assert!(
            !choose(&none, Some(Manager::Mise), None)
                .expect("seeds")
                .manager_present
        );
    }
}
