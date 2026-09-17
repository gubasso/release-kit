//! The single owner of configuration policy judgments and their method sources.

use super::{Config, LOCAL_GITHUB_BYPASS, Protection, invalid, legacy_local_protection};
use crate::error::RkError;
use crate::landing::Integration;

/// A minimum policy and the method heading it serves.
pub struct Floor {
    /// Fully qualified configuration key.
    pub key: &'static str,
    /// Human-readable minimum or invariant.
    pub minimum: &'static str,
    /// Exact heading in the invariants chapter.
    pub heading: &'static str,
    accepts: fn(&Protection) -> bool,
}

/// The heading every floor that holds under both integration modes cites.
const SQUASH: &str = "The trunk takes one validated squash commit at a time";

/// The heading the floors that only a forge can enforce cite.
const REQUEST: &str = "A forge-integrated trunk merges through a checked request";

/// The tag floor and the squash floors, identical under both integration
/// modes: the release request integrates at the forge either way, so what
/// protects it and what protects a published version never varies.
const SHARED: &[Floor] = &[
    Floor {
        key: "protection.tag_pattern",
        minimum: "refs/tags/v* or refs/tags/*, covering every published version",
        heading: "A published version is immutable",
        accepts: |p| matches!(p.tag_pattern.as_str(), "refs/tags/v*" | "refs/tags/*"),
    },
    Floor {
        key: "protection.allowed_merge_methods",
        minimum: "exactly [squash]",
        heading: SQUASH,
        accepts: |p| p.allowed_merge_methods == ["squash"],
    },
    Floor {
        key: "protection.github.squash_title_source",
        minimum: "PR_TITLE",
        heading: SQUASH,
        accepts: |p| p.github.squash_title_source == "PR_TITLE",
    },
    Floor {
        key: "protection.github.squash_body_source",
        minimum: "PR_BODY",
        heading: SQUASH,
        accepts: |p| p.github.squash_body_source == "PR_BODY",
    },
    Floor {
        key: "protection.gitlab.merge_method",
        minimum: "ff",
        heading: SQUASH,
        accepts: |p| p.gitlab.merge_method == "ff",
    },
    Floor {
        key: "protection.gitlab.squash_option",
        minimum: "always",
        heading: SQUASH,
        accepts: |p| p.gitlab.squash_option == "always",
    },
    Floor {
        key: "protection.gitlab.squash_commit_template",
        minimum: "references %{title}",
        heading: SQUASH,
        // The title alone, deliberately. `forge-setup:the-setup-asserts-
        // the-squash-body-source` states that GitLab's template puts no
        // request description on the trunk, so requiring `%{description}`
        // here would contradict the setup this table is meant to floor.
        accepts: |p| p.gitlab.squash_commit_template.contains("%{title}"),
    },
    Floor {
        key: "protection.gitlab.merge_access_level",
        minimum: "at least 30",
        heading: SQUASH,
        accepts: |p| p.gitlab.merge_access_level >= 30,
    },
];

/// What a forge-integrated trunk adds: the request rule, the check the
/// request must carry, and the review policy above them.
const FORGE_ONLY: &[Floor] = &[
    Floor {
        key: "protection.bypass_actors",
        minimum: "empty",
        heading: SQUASH,
        accepts: |p| p.bypass_actors.is_empty(),
    },
    Floor {
        key: "protection.strict_required_status_checks",
        minimum: "true",
        heading: REQUEST,
        accepts: |p| p.strict_required_status_checks,
    },
    Floor {
        key: "protection.owned_trunk_rules",
        minimum: "contains deletion, non_fast_forward, pull_request, required_status_checks",
        heading: REQUEST,
        accepts: |p| {
            [
                "deletion",
                "non_fast_forward",
                "pull_request",
                "required_status_checks",
            ]
            .iter()
            .all(|rule| p.owned_trunk_rules.iter().any(|owned| owned == rule))
        },
    },
    Floor {
        key: "protection.gitlab.push_access_level",
        minimum: "0 (no direct pushes)",
        heading: REQUEST,
        accepts: |p| p.gitlab.push_access_level == 0,
    },
    Floor {
        key: "protection.required_approving_review_count",
        minimum: "at least 0",
        heading: REQUEST,
        accepts: |p| p.required_approving_review_count >= 0,
    },
    Floor {
        key: "protection.dismiss_stale_reviews_on_push",
        minimum: "false; true is stricter",
        heading: REQUEST,
        accepts: |p| {
            let _ = p.dismiss_stale_reviews_on_push;
            true
        },
    },
    Floor {
        key: "protection.require_code_owner_review",
        minimum: "false; true is stricter",
        heading: REQUEST,
        accepts: |p| {
            let _ = p.require_code_owner_review;
            true
        },
    },
    Floor {
        key: "protection.require_last_push_approval",
        minimum: "false; true is stricter",
        heading: REQUEST,
        accepts: |p| {
            let _ = p.require_last_push_approval;
            true
        },
    },
];

/// What a locally integrated trunk changes.
///
/// GitHub keeps the request and strict-check rules so they can hold a
/// release request on the trunk it was tested against. The repository
/// administrator alone bypasses the ruleset carrying them, for the direct
/// push this mode exists to make. Deletion and force-push protection sit in
/// the safety ruleset, which names nobody, so that bypass never reaches
/// them. GitLab keeps its native access-level answer.
const LOCAL_ONLY: &[Floor] = &[
    Floor {
        key: "protection.bypass_actors",
        minimum: "exactly [repository-admin]",
        heading: SQUASH,
        accepts: |p| p.bypass_actors == [LOCAL_GITHUB_BYPASS] || legacy_local_protection(p),
    },
    Floor {
        key: "protection.strict_required_status_checks",
        minimum: "true",
        heading: REQUEST,
        accepts: |p| p.strict_required_status_checks,
    },
    Floor {
        key: "protection.owned_trunk_rules",
        minimum: "contains deletion, non_fast_forward, pull_request, required_status_checks",
        heading: REQUEST,
        accepts: |p| {
            [
                "deletion",
                "non_fast_forward",
                "pull_request",
                "required_status_checks",
            ]
            .iter()
            .all(|rule| p.owned_trunk_rules.iter().any(|owned| owned == rule))
                || legacy_local_protection(p)
        },
    },
    Floor {
        key: "protection.gitlab.push_access_level",
        // 0 closes the trunk to every push, which is stricter than a
        // restriction and what a target that never flipped the axis
        // still carries. 30 is developer, which is everyone, and is the
        // one value this mode refuses.
        minimum: "at least 40 (maintainers alone); 0 is stricter and closes the trunk entirely",
        heading: SQUASH,
        accepts: |p| p.gitlab.push_access_level == 0 || p.gitlab.push_access_level >= 40,
    },
    Floor {
        key: "protection.required_approving_review_count",
        minimum: "at least 0",
        heading: REQUEST,
        accepts: |p| p.required_approving_review_count >= 0,
    },
    Floor {
        key: "protection.dismiss_stale_reviews_on_push",
        minimum: "false; true is stricter",
        heading: REQUEST,
        accepts: |p| {
            let _ = p.dismiss_stale_reviews_on_push;
            true
        },
    },
    Floor {
        key: "protection.require_code_owner_review",
        minimum: "false; true is stricter",
        heading: REQUEST,
        accepts: |p| {
            let _ = p.require_code_owner_review;
            true
        },
    },
    Floor {
        key: "protection.require_last_push_approval",
        minimum: "false; true is stricter",
        heading: REQUEST,
        accepts: |p| {
            let _ = p.require_last_push_approval;
            true
        },
    },
];

/// Every floored configuration key under the given integration mode.
#[must_use]
pub fn floors(integration: Integration) -> Vec<&'static Floor> {
    let extra = match integration {
        Integration::Forge => FORGE_ONLY,
        Integration::Local => LOCAL_ONLY,
    };
    SHARED.iter().chain(extra).collect()
}

/// Judge a configuration before any consumer uses it.
///
/// A configuration silent about `git.integration` is judged as a
/// forge-integrated one: that is the shape every target landed before the
/// axis existed, and reading silence as `local` would lift two floors a
/// target still relies on.
///
/// # Errors
/// Refuses the first weakened policy, naming its key, floor and method source.
pub fn check(config: &Config) -> Result<(), RkError> {
    let integration = config.git.integration.unwrap_or(Integration::Forge);
    for floor in floors(integration) {
        if !(floor.accepts)(&config.protection) {
            return Err(invalid(format!(
                "{}: floor is {}; see rk method invariants ({})",
                floor.key, floor.minimum, floor.heading
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{check, floors};
    use crate::config::Config;
    use crate::landing::Integration;

    #[test]
    fn every_floor_names_a_real_invariant_heading() {
        let chapter = crate::embedded::METHOD
            .get_file("01-invariants.md")
            .expect("the invariants ship")
            .contents_utf8()
            .expect("prose is utf8");
        for integration in [Integration::Forge, Integration::Local] {
            for floor in floors(integration) {
                assert!(
                    chapter
                        .lines()
                        .any(|line| line.strip_prefix("## ") == Some(floor.heading)),
                    "{}: {}",
                    floor.key,
                    floor.heading
                );
            }
        }
        check(&Config::default()).expect("defaults meet every floor");
        let mut local = Config::default();
        local.git.integration = Some(Integration::Local);
        local.protection = crate::config::local_protection();
        check(&local).expect("defaults meet every local floor too");
    }

    #[test]
    fn a_value_below_a_floor_refuses_naming_the_invariants() {
        let mut config = Config::default();
        config.protection.allowed_merge_methods.push("merge".into());
        let error = check(&config)
            .expect_err("merge violates squash only")
            .to_string();
        for expected in [
            "protection.allowed_merge_methods",
            "floor",
            "squash",
            "rk method invariants",
        ] {
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn a_stricter_value_passes_the_floor() {
        let mut config = Config::default();
        config.protection.required_approving_review_count = 3;
        config.protection.dismiss_stale_reviews_on_push = true;
        config.protection.require_code_owner_review = true;
        config.protection.require_last_push_approval = true;
        config.protection.gitlab.merge_access_level = 40;
        config.protection.tag_pattern = "refs/tags/*".into();
        config
            .protection
            .owned_trunk_rules
            .push("required_signatures".into());
        check(&config).expect("a target can be stricter");
    }

    #[test]
    fn one_policy_is_judged_by_the_recorded_integration_mode() {
        let mut config = Config::default();
        config.protection.owned_trunk_rules = vec!["deletion".into(), "non_fast_forward".into()];
        config.git.integration = Some(Integration::Forge);
        let error = check(&config)
            .expect_err("a forge-integrated trunk needs the request rule")
            .to_string();
        assert!(error.contains("protection.owned_trunk_rules"), "{error}");
        assert!(error.contains("pull_request"), "{error}");
        config.git.integration = Some(Integration::Local);
        config.protection.gitlab.push_access_level = 40;
        check(&config).expect("the legacy local policy remains readable for migration");
    }

    #[test]
    fn a_silent_config_is_judged_as_forge_integrated() {
        let mut config = Config::default();
        config.protection.owned_trunk_rules = vec!["deletion".into(), "non_fast_forward".into()];
        config.git.integration = None;
        check(&config).expect_err("silence is judged by the stricter table");
    }

    #[test]
    fn a_local_target_still_refuses_a_developer_wide_trunk_push() {
        let mut config = Config::default();
        config.git.integration = Some(Integration::Local);
        config.protection = crate::config::local_protection();
        config.protection.gitlab.push_access_level = 30;
        let error = check(&config)
            .expect_err("developer level is everyone")
            .to_string();
        assert!(
            error.contains("protection.gitlab.push_access_level"),
            "{error}"
        );
        config.protection.gitlab.push_access_level = 40;
        check(&config).expect("maintainers alone is a restriction");
    }

    #[test]
    fn the_tag_floors_hold_under_both_integration_modes() {
        for integration in [Integration::Forge, Integration::Local] {
            let mut config = Config::default();
            config.git.integration = Some(integration);
            config.protection.tag_pattern = "refs/tags/release-*".into();
            let error = check(&config)
                .expect_err("a pattern missing published versions refuses")
                .to_string();
            assert!(error.contains("protection.tag_pattern"), "{error}");
        }
    }
}
