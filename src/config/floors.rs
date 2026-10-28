//! The single owner of configuration policy judgments and their method sources.

use super::{Config, LOCAL_GITHUB_BYPASS, Protection, invalid, legacy_local_protection};
use crate::error::RkError;
use crate::landing::{Integration, MergeForm};

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

/// The heading every floor that holds under both integration modes cites:
/// the linear trunk, in the recorded merge form.
const LINEAR_TRUNK: &str = "The trunk takes one validated integration at a time";

/// The heading the floors that only a forge can enforce cite.
const REQUEST: &str = "A forge-integrated trunk merges through a checked request";

/// The tag floor and the linear-trunk floors, identical under both
/// integration modes and both merge forms: the release request integrates
/// at the forge either way, so what protects it and what protects a
/// published version never varies.
const SHARED: &[Floor] = &[
    Floor {
        key: "protection.tag_pattern",
        minimum: "refs/tags/v* or refs/tags/*, covering every published version",
        heading: "A published version is immutable",
        accepts: |p| matches!(p.tag_pattern.as_str(), "refs/tags/v*" | "refs/tags/*"),
    },
    Floor {
        key: "protection.gitlab.merge_method",
        minimum: "ff",
        heading: LINEAR_TRUNK,
        accepts: |p| p.gitlab.merge_method == "ff",
    },
    Floor {
        key: "protection.gitlab.squash_commit_template",
        minimum: "references %{title}",
        heading: LINEAR_TRUNK,
        // The title alone, deliberately. `forge-setup:the-setup-asserts-
        // the-squash-body-source` states that GitLab's template puts no
        // request description on the trunk, so requiring `%{description}`
        // here would contradict the setup this table is meant to floor.
        accepts: |p| p.gitlab.squash_commit_template.contains("%{title}"),
    },
    Floor {
        key: "protection.gitlab.merge_access_level",
        minimum: "at least 30",
        heading: LINEAR_TRUNK,
        accepts: |p| p.gitlab.merge_access_level >= 30,
    },
];

/// What a forge-integrated trunk adds: the request rule, the check the
/// request must carry, and the review policy above them.
const FORGE_ONLY: &[Floor] = &[
    Floor {
        key: "protection.bypass_actors",
        minimum: "empty",
        heading: LINEAR_TRUNK,
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
        heading: LINEAR_TRUNK,
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
        heading: LINEAR_TRUNK,
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

/// The merge floors of the `squash` form: the squash merge alone, with
/// the request's title and body as the trunk commit's message, and GitLab
/// squashing every merge request.
const SQUASH_FORM: &[Floor] = &[
    Floor {
        key: "protection.allowed_merge_methods",
        minimum: "exactly [squash]",
        heading: LINEAR_TRUNK,
        accepts: |p| p.allowed_merge_methods == ["squash"],
    },
    Floor {
        key: "protection.github.squash_title_source",
        minimum: "PR_TITLE",
        heading: LINEAR_TRUNK,
        accepts: |p| p.github.squash_title_source == "PR_TITLE",
    },
    Floor {
        key: "protection.github.squash_body_source",
        minimum: "PR_BODY",
        heading: LINEAR_TRUNK,
        accepts: |p| p.github.squash_body_source == "PR_BODY",
    },
    Floor {
        key: "protection.gitlab.squash_option",
        minimum: "always",
        heading: LINEAR_TRUNK,
        accepts: |p| p.gitlab.squash_option == "always",
    },
];

/// The merge floors of the `rebase` form: the rebase merge alone, and
/// GitLab squashing nothing, so every commit of a branch keeps its own
/// message. No squash source applies, so neither squash source floor is
/// here.
const REBASE_FORM: &[Floor] = &[
    Floor {
        key: "protection.allowed_merge_methods",
        minimum: "exactly [rebase]",
        heading: LINEAR_TRUNK,
        accepts: |p| p.allowed_merge_methods == ["rebase"],
    },
    Floor {
        key: "protection.gitlab.squash_option",
        minimum: "never",
        heading: LINEAR_TRUNK,
        accepts: |p| p.gitlab.squash_option == "never",
    },
];

/// Every floored configuration key under the given integration mode and
/// merge form.
#[must_use]
pub fn floors(integration: Integration, form: MergeForm) -> Vec<&'static Floor> {
    let extra = match integration {
        Integration::Forge => FORGE_ONLY,
        Integration::Local => LOCAL_ONLY,
    };
    let merge = match form {
        MergeForm::Squash => SQUASH_FORM,
        MergeForm::Rebase => REBASE_FORM,
    };
    SHARED.iter().chain(merge).chain(extra).collect()
}

/// Judge a configuration before any consumer uses it.
///
/// A configuration silent about `git.integration` is judged as a
/// forge-integrated one: that is the shape every target landed before the
/// axis existed, and reading silence as `local` would lift two floors a
/// target still relies on.
///
/// A configuration that states `git.merge_form` is judged by that form's
/// merge floors alone, so a rebase value refuses under `squash` and a
/// squash value under `rebase`. One silent about it resolves the form from
/// the landing record, which this reader does not see, so it passes where
/// its merge values are exactly one form's, and is otherwise judged as
/// `squash`, the form every target took before the parameter existed. The
/// setup derives the merge values from the recorded form either way, and
/// the next landing writes the resolved form's values back.
///
/// # Errors
/// Refuses the first weakened policy, naming its key, floor and method source.
pub fn check(config: &Config) -> Result<(), RkError> {
    let integration = config.git.integration.unwrap_or(Integration::Forge);
    let form = config.git.merge_form.unwrap_or_else(|| {
        let holds = |table: &[Floor]| {
            table
                .iter()
                .all(|floor| (floor.accepts)(&config.protection))
        };
        if !holds(SQUASH_FORM) && holds(REBASE_FORM) {
            MergeForm::Rebase
        } else {
            MergeForm::Squash
        }
    });
    for floor in floors(integration, form) {
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
    use crate::landing::{Integration, MergeForm};

    #[test]
    fn every_floor_names_a_real_invariant_heading() {
        let chapter = crate::embedded::METHOD
            .get_file("01-invariants.md")
            .expect("the invariants ship")
            .contents_utf8()
            .expect("prose is utf8");
        for integration in [Integration::Forge, Integration::Local] {
            for form in [MergeForm::Squash, MergeForm::Rebase] {
                for floor in floors(integration, form) {
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

    /// The rebase policy: the rebase merge alone, and GitLab squashing
    /// nothing. The squash sources keep whatever the file holds.
    fn rebase_policy(config: &mut Config) {
        config.protection = crate::config::with_merge_form(&config.protection, MergeForm::Rebase);
    }

    /// SATISFIES target-config:an-invariant-bearing-key-carries-a-floor
    #[test]
    fn merge_form_rebase_values_pass_only_under_the_rebase_form() {
        for integration in [Integration::Forge, Integration::Local] {
            let mut config = Config::default();
            config.git.integration = Some(integration);
            if integration == Integration::Local {
                config.protection = crate::config::local_protection();
            }
            rebase_policy(&mut config);
            config.git.merge_form = Some(MergeForm::Rebase);
            check(&config).expect("the rebase form accepts its own values");
            config.git.merge_form = Some(MergeForm::Squash);
            let error = check(&config)
                .expect_err("the squash form refuses the rebase method")
                .to_string();
            assert!(
                error.contains("protection.allowed_merge_methods"),
                "{error}"
            );
            assert!(error.contains("exactly [squash]"), "{error}");
            assert!(error.contains("rk method invariants"), "{error}");
        }
    }

    /// A file silent about the form resolves it from the record, which
    /// the reader does not see: one form's whole values pass, and a mix of
    /// the two is judged as squash.
    #[test]
    fn merge_form_silence_takes_one_forms_whole_values() {
        let mut config = Config::default();
        config.git.merge_form = None;
        check(&config).expect("the squash values pass a silent file");
        rebase_policy(&mut config);
        check(&config).expect("the rebase values pass a silent file");
        config.protection.gitlab.squash_option = "always".into();
        let error = check(&config)
            .expect_err("a mix of the two forms is no form")
            .to_string();
        assert!(error.contains("exactly [squash]"), "{error}");
    }

    #[test]
    fn merge_form_squash_values_refuse_under_the_rebase_form() {
        let mut config = Config::default();
        config.git.merge_form = Some(MergeForm::Rebase);
        let error = check(&config)
            .expect_err("the rebase form refuses the squash method")
            .to_string();
        assert!(
            error.contains("protection.allowed_merge_methods"),
            "{error}"
        );
        assert!(error.contains("exactly [rebase]"), "{error}");
        // The method alone is not enough: GitLab must squash nothing.
        config.protection.allowed_merge_methods = vec!["rebase".into()];
        let error = check(&config)
            .expect_err("a squashing GitLab refuses under rebase")
            .to_string();
        assert!(error.contains("protection.gitlab.squash_option"), "{error}");
        assert!(error.contains("never"), "{error}");
    }

    #[test]
    fn merge_form_rebase_lifts_the_squash_source_floors() {
        let mut config = Config::default();
        config.git.merge_form = Some(MergeForm::Rebase);
        rebase_policy(&mut config);
        config.protection.github.squash_title_source = "COMMIT_OR_PR_TITLE".into();
        config.protection.github.squash_body_source = "BLANK".into();
        check(&config).expect("no squash source applies under rebase");
        config.git.merge_form = Some(MergeForm::Squash);
        config.protection = crate::config::with_merge_form(&config.protection, MergeForm::Squash);
        let error = check(&config)
            .expect_err("the squash form floors its title source")
            .to_string();
        assert!(
            error.contains("protection.github.squash_title_source"),
            "{error}"
        );
    }

    #[test]
    fn merge_form_never_admits_a_second_method() {
        let mut config = Config::default();
        config.git.merge_form = Some(MergeForm::Rebase);
        rebase_policy(&mut config);
        config.protection.allowed_merge_methods = vec!["rebase".into(), "squash".into()];
        let error = check(&config)
            .expect_err("two methods are not one linear form")
            .to_string();
        assert!(
            error.contains("protection.allowed_merge_methods"),
            "{error}"
        );
    }
}
