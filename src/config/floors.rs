//! The single owner of configuration policy judgments and their method sources.

use super::{Config, Protection, invalid};
use crate::error::RkError;

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

/// Every floored configuration key, including permissive boolean floors.
pub const FLOORS: &[Floor] = &[
    Floor {
        key: "protection.tag_pattern",
        minimum: "refs/tags/v* or refs/tags/*, covering every published version",
        heading: "A published version is immutable",
        accepts: |p| matches!(p.tag_pattern.as_str(), "refs/tags/v*" | "refs/tags/*"),
    },
    Floor {
        key: "protection.bypass_actors",
        minimum: "empty",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.bypass_actors.is_empty(),
    },
    Floor {
        key: "protection.allowed_merge_methods",
        minimum: "exactly [squash]",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.allowed_merge_methods == ["squash"],
    },
    Floor {
        key: "protection.strict_required_status_checks",
        minimum: "true",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.strict_required_status_checks,
    },
    Floor {
        key: "protection.owned_trunk_rules",
        minimum: "contains deletion, non_fast_forward, pull_request, required_status_checks",
        heading: "Trunk is written through pull requests only",
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
        key: "protection.required_approving_review_count",
        minimum: "at least 0",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.required_approving_review_count >= 0,
    },
    Floor {
        key: "protection.dismiss_stale_reviews_on_push",
        minimum: "false; true is stricter",
        heading: "Trunk is written through pull requests only",
        accepts: |p| {
            let _ = p.dismiss_stale_reviews_on_push;
            true
        },
    },
    Floor {
        key: "protection.require_code_owner_review",
        minimum: "false; true is stricter",
        heading: "Trunk is written through pull requests only",
        accepts: |p| {
            let _ = p.require_code_owner_review;
            true
        },
    },
    Floor {
        key: "protection.require_last_push_approval",
        minimum: "false; true is stricter",
        heading: "Trunk is written through pull requests only",
        accepts: |p| {
            let _ = p.require_last_push_approval;
            true
        },
    },
    Floor {
        key: "protection.github.squash_title_source",
        minimum: "PR_TITLE",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.github.squash_title_source == "PR_TITLE",
    },
    Floor {
        key: "protection.github.squash_body_source",
        minimum: "PR_BODY",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.github.squash_body_source == "PR_BODY",
    },
    Floor {
        key: "protection.gitlab.merge_method",
        minimum: "ff",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.gitlab.merge_method == "ff",
    },
    Floor {
        key: "protection.gitlab.squash_option",
        minimum: "always",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.gitlab.squash_option == "always",
    },
    Floor {
        key: "protection.gitlab.squash_commit_template",
        minimum: "references %{title}",
        heading: "Trunk is written through pull requests only",
        // The title alone, deliberately. `forge-setup:the-setup-asserts-
        // the-squash-body-source` states that GitLab's template puts no
        // request description on the trunk, so requiring `%{description}`
        // here would contradict the setup this table is meant to floor.
        accepts: |p| p.gitlab.squash_commit_template.contains("%{title}"),
    },
    Floor {
        key: "protection.gitlab.push_access_level",
        minimum: "0 (no direct pushes)",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.gitlab.push_access_level == 0,
    },
    Floor {
        key: "protection.gitlab.merge_access_level",
        minimum: "at least 30",
        heading: "Trunk is written through pull requests only",
        accepts: |p| p.gitlab.merge_access_level >= 30,
    },
];

/// Judge a configuration before any consumer uses it.
///
/// # Errors
/// Refuses the first weakened policy, naming its key, floor and method source.
pub fn check(config: &Config) -> Result<(), RkError> {
    for floor in FLOORS {
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
    #![allow(clippy::expect_used)]

    use super::{FLOORS, check};
    use crate::config::Config;

    #[test]
    fn every_floor_names_a_real_invariant_heading() {
        let chapter = crate::embedded::METHOD
            .get_file("01-invariants.md")
            .expect("the invariants ship")
            .contents_utf8()
            .expect("prose is utf8");
        for floor in FLOORS {
            assert!(
                chapter
                    .lines()
                    .any(|line| line.strip_prefix("## ") == Some(floor.heading)),
                "{}: {}",
                floor.key,
                floor.heading
            );
        }
        check(&Config::default()).expect("defaults meet every floor");
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
}
