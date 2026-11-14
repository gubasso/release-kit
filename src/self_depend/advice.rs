//! The texts that guide a pin move.
//!
//! They are the warning that a newer release is out, the route to the
//! setup skill of the rk that the move brings, and the follows lines that
//! keep a consumer's `flake.lock` free of nested copies.
//!
//! The two sentences are one shared wording with sdd, word for word, so
//! a reader who moves both pins reads one procedure. Every verb that
//! warns or routes takes its text from here, so the wording has one
//! owner. Reading the follows lines is pure: lock bytes in, lines out.
//!
//! SATISFIES packaging:the-unattended-caller-never-fails-the-shell

use super::discover::version_order;

/// The skill that runs an upgrade.
pub const SKILL: &str = "rk-setup";

/// The inputs of rk's own flake that serve only its development shell.
/// A consumer that has no input of the same name points each at its own
/// flake, so its lock holds no node for them.
const DEV_ONLY_INPUTS: &[&str] = &["spec-driven-docs", "exobrain-tech"];

/// The input name the consumer gives rk, and rk's own input for itself.
const SELF_INPUT: &str = "release-kit";

/// The warning that a newer release is out and the pin is behind it.
#[must_use]
pub fn pending(latest: &str, pin: &str) -> String {
    format!(
        "rk {} is out, this project pins {}. To move: rk self-depend sync --apply, then upgrade through the {SKILL} skill of the new rk.",
        latest.trim_start_matches('v'),
        pin.trim_start_matches('v'),
    )
}

/// The route after a pin move: the setup skill of the rk in the
/// devshell runs the upgrade.
#[must_use]
pub fn after_move() -> String {
    format!(
        "After the pin move, load the devshell and run the upgrade through the {SKILL} skill of the rk in that devshell, and do every migration it names. Before `rk upgrade`, read it with `rk skill show {SKILL}`. After it, the project's .claude/skills/{SKILL} is that skill. Never use a host copy or an older skill."
    )
}

/// The pending warning, where the pin is behind the latest release that
/// the check cache knows. It reads the cache and fetches nothing.
#[must_use]
pub fn pending_from_cache(key: &str, pin: &str) -> Option<String> {
    let latest = cached_latest(key)?;
    version_order(&latest, pin)
        .is_gt()
        .then(|| pending(&latest, pin))
}

/// The latest release the check cache of one checkout records.
#[must_use]
pub fn cached_latest(key: &str) -> Option<String> {
    let paths = super::check::Paths::for_key(key)?;
    super::check::read(&paths.cache)?.latest
}

/// The follows lines that rk's flake input lacks, from the consumer's
/// `flake.lock`, each as the exact line to add.
///
/// Only an input that the lock gives a node of its own counts: a follows
/// path is already chosen. rk's copy of itself points at the consumer's
/// flake. An input whose name the consumer's flake also declares follows
/// that input. A development-only input that the consumer lacks points
/// at the consumer's flake. Every other input keeps its own node.
#[must_use]
pub fn follows_lines(lock: &[u8]) -> Vec<String> {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(lock) else {
        return Vec::new();
    };
    let nodes = &value["nodes"];
    let root = value["root"].as_str().unwrap_or("root");
    let Some(rk) = super::input_node(nodes, root, root, SELF_INPUT, 0) else {
        return Vec::new();
    };
    let Some(inputs) = nodes[rk.as_str()]["inputs"].as_object() else {
        return Vec::new();
    };
    let root_inputs = &nodes[root]["inputs"];
    let mut lines = Vec::new();
    for (name, target) in inputs {
        if !target.is_string() {
            continue;
        }
        let follows = if name == SELF_INPUT {
            ""
        } else if root_inputs.get(name).is_some() {
            name.as_str()
        } else if DEV_ONLY_INPUTS.contains(&name.as_str()) {
            ""
        } else {
            continue;
        };
        lines.push(format!("inputs.{name}.follows = \u{22}{follows}\u{22};"));
    }
    lines
}

/// The next line that names the missing follows lines, where any is
/// missing.
#[must_use]
pub fn follows_next(lines: &[String]) -> Option<String> {
    (!lines.is_empty()).then(|| {
        format!(
            "the release-kit input of flake.nix lacks these lines: {} Add them beside its url, then run nix flake lock, which drops the nested nodes",
            lines.join(" ")
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{after_move, follows_lines, follows_next, pending};

    #[test]
    fn the_pending_warning_names_both_versions_and_the_skill() {
        assert_eq!(
            pending("v0.11.0", "v0.10.0"),
            "rk 0.11.0 is out, this project pins 0.10.0. To move: rk self-depend sync --apply, then upgrade through the rk-setup skill of the new rk."
        );
    }

    #[test]
    fn the_route_names_the_devshell_skill_and_refuses_other_copies() {
        let text = after_move();
        assert!(text.starts_with("After the pin move, load the devshell"));
        assert!(text.contains("`rk skill show rk-setup`"));
        assert!(text.ends_with("Never use a host copy or an older skill."));
    }

    /// A lock in the shape a consumer that pins rk 0.10.0 alone writes:
    /// rk's own copy of itself and its sdd input nest, nixpkgs follows.
    const RK_ALONE: &str = r#"{"root":"root","version":7,"nodes":{
        "root":{"inputs":{"nixpkgs":"nixpkgs","release-kit":"release-kit_2"}},
        "nixpkgs":{},
        "release-kit_2":{"inputs":{"nixpkgs":["nixpkgs"],"release-kit":"release-kit_3",
            "rust-overlay":"rust-overlay","spec-driven-docs":"spec-driven-docs","exobrain-tech":"exobrain-tech"}},
        "release-kit_3":{},"rust-overlay":{},"spec-driven-docs":{},"exobrain-tech":{}}}"#;

    #[test]
    fn a_consumer_that_pins_rk_alone_drops_every_nested_copy() {
        assert_eq!(
            follows_lines(RK_ALONE.as_bytes()),
            vec![
                r#"inputs.exobrain-tech.follows = "";"#,
                r#"inputs.release-kit.follows = "";"#,
                r#"inputs.spec-driven-docs.follows = "";"#,
            ]
        );
    }

    #[test]
    fn a_consumer_that_pins_sdd_too_points_rk_at_it() {
        let lock = RK_ALONE.replace(
            r#""root":{"inputs":{"nixpkgs":"nixpkgs","#,
            r#""root":{"inputs":{"nixpkgs":"nixpkgs","spec-driven-docs":"spec-driven-docs_2","#,
        );
        let lines = follows_lines(lock.as_bytes());
        assert!(
            lines.contains(&r#"inputs.spec-driven-docs.follows = "spec-driven-docs";"#.to_owned())
        );
    }

    #[test]
    fn a_followed_input_needs_no_line() {
        let lock = RK_ALONE
            .replace(r#""release-kit":"release-kit_3""#, r#""release-kit":[]"#)
            .replace(
                r#""spec-driven-docs":"spec-driven-docs","exobrain-tech":"exobrain-tech""#,
                r#""spec-driven-docs":[],"exobrain-tech":[]"#,
            );
        assert!(follows_lines(lock.as_bytes()).is_empty());
        assert_eq!(follows_next(&[]), None);
    }

    #[test]
    fn an_unreadable_lock_advises_nothing() {
        assert!(follows_lines(b"not json").is_empty());
    }
}
