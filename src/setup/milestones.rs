//! The publish milestones a setup check reads back: what the forge and
//! the registry can show, and what only the operator can.
//!
//! Two milestones have a public read-back: the first version published,
//! which the registry answers, and an automated release proven, which
//! the forge answers by a successful publish run on the trunk and a
//! release beside it. Every registry form no API exposes reads
//! `operator`, with the binding's own read-back check, so a skill
//! sequences the operator's steps without guessing where they stand.
//! Milestones are information: none weighs on the check's verdict.
//!
//! SATISFIES setup-proof:the-check-reads-the-publish-milestones

use serde_json::Value;

use super::context::Ctx;
use crate::detect::Forge;
use crate::release_state::{self, Answer, Api, ForgeCli, Registry};

/// One milestone, as the check reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Milestone {
    /// The stable milestone id.
    pub id: &'static str,
    /// `observed`, `not-observed`, `unknown`, `operator`, or
    /// `not-applicable`.
    pub status: &'static str,
    /// What was read, or the read-back check that proves it.
    pub detail: String,
}

impl Milestone {
    /// The human line.
    #[must_use]
    pub fn line(&self) -> String {
        format!("milestone {}: {} ({})", self.id, self.status, self.detail)
    }
}

/// The milestone for the first version on the registry.
pub const FIRST_VERSION: &str = "first-version-published";
/// The milestone for one release through the automated path.
pub const AUTOMATED_RELEASE: &str = "automated-release-proven";

/// The status word for an answer the read gave.
const fn word(answer: Answer) -> &'static str {
    match answer {
        Answer::Yes => "observed",
        Answer::No => "not-observed",
        Answer::Unknown => "unknown",
        Answer::NotApplicable => "not-applicable",
    }
}

/// The registry forms no API exposes, per driver, each with the
/// binding's read-back check.
fn operator_forms(driver: &str, package: &str) -> Vec<(&'static str, String)> {
    match driver {
        "rust" => vec![
            (
                "trusted-publisher",
                format!(
                    "rk binding rust, the trusted publisher: the Trusted Publishing table on crates.io/crates/{package}/settings lists the owner, the repository, and release-plz.yml"
                ),
            ),
            (
                "bootstrap-token-revoked",
                "rk binding rust, revoking the bootstrap token: the token is gone from crates.io/settings/tokens, and the host's cargo credentials name no crates-io token".to_owned(),
            ),
            (
                "trusted-publishing-required",
                format!(
                    "rk binding rust, the trusted publisher: on crates.io/crates/{package}/settings, require trusted publishing for all new versions reads as enabled after a reload"
                ),
            ),
        ],
        "python" => vec![
            (
                "trusted-publisher",
                format!(
                    "rk binding python, setup specifics: the publishing settings of {package} on PyPI list the owner, the repository, and release-please.yml"
                ),
            ),
            (
                "bootstrap-token-revoked",
                "rk binding python, setup specifics: the project-scoped API token is gone from the PyPI account's API tokens".to_owned(),
            ),
        ],
        _ => Vec::new(),
    }
}

/// Read every milestone this target's pair has.
#[must_use]
pub fn observe(ctx: &Ctx) -> Vec<Milestone> {
    let Some(driver) = ctx.driver().filter(|_| ctx.automatic_release()) else {
        let reason = "the target selects no automated release".to_owned();
        return vec![
            Milestone {
                id: FIRST_VERSION,
                status: "not-applicable",
                detail: reason.clone(),
            },
            Milestone {
                id: AUTOMATED_RELEASE,
                status: "not-applicable",
                detail: reason,
            },
        ];
    };
    let registry = match driver {
        "rust" => Some(Registry::CratesIo),
        "python" => Some(Registry::Pypi),
        _ => None,
    };
    let package = release_state::package_name(ctx.target.as_std_path(), driver);
    let mut found = vec![first_version(registry, package.as_deref())];
    found.push(automated_release(ctx, driver));
    if let Some(package) = package.as_deref() {
        for (id, check) in operator_forms(driver, package) {
            found.push(Milestone {
                id,
                status: "operator",
                detail: check,
            });
        }
    }
    found
}

fn first_version(registry: Option<Registry>, package: Option<&str>) -> Milestone {
    let (status, detail) = match (registry, package) {
        (None, _) => (
            "not-applicable",
            "the driver publishes to no registry; the release page is the distribution".to_owned(),
        ),
        (Some(_), None) => (
            "unknown",
            "the version file names no package the registry can be asked about".to_owned(),
        ),
        (Some(registry), Some(package)) => {
            let answer = release_state::registry_serves(registry, package, None);
            (
                word(answer),
                format!("{} answers for {package}", registry.url(package, None)),
            )
        }
    };
    Milestone {
        id: FIRST_VERSION,
        status,
        detail,
    }
}

fn automated_release(ctx: &Ctx, driver: &str) -> Milestone {
    let milestone = |status, detail: String| Milestone {
        id: AUTOMATED_RELEASE,
        status,
        detail,
    };
    let Some(forge) = ctx.forge else {
        return milestone(
            "not-applicable",
            "the target names no forge this release reads".to_owned(),
        );
    };
    if ctx.repo.is_empty() {
        return milestone("unknown", "the target names no forge repository".to_owned());
    }
    let cli = ForgeCli::new(ctx.cli.clone(), ctx.target.as_std_path().to_path_buf());
    let trunk = ctx.trunk();
    let unknown =
        |reason: String| milestone("unknown", format!("the forge did not answer: {reason}"));
    // The newest release, the commit its tag names, and a successful
    // publish run on the trunk at that commit: a release page alone can
    // predate the automation, and a successful run can publish nothing.
    let releases_path = match forge {
        Forge::Github => format!("repos/{}/releases?per_page=1", ctx.repo),
        Forge::Gitlab => format!(
            "projects/{}/releases?per_page=1",
            release_state::gitlab_project(&ctx.repo)
        ),
    };
    let tag = match cli.api(&releases_path) {
        Api::Ok(body) => body
            .as_array()
            .and_then(|list| list.first())
            .and_then(|release| release["tag_name"].as_str())
            .map(str::to_owned),
        Api::Missing => None,
        Api::Failed(reason) => return unknown(reason),
    };
    let Some(tag) = tag else {
        return milestone("not-observed", "no release exists yet".to_owned());
    };
    let (commit_path, commit_pointer) = match forge {
        Forge::Github => (format!("repos/{}/commits/{tag}", ctx.repo), "/sha"),
        Forge::Gitlab => (
            format!(
                "projects/{}/repository/tags/{tag}",
                release_state::gitlab_project(&ctx.repo)
            ),
            "/commit/id",
        ),
    };
    let commit = match cli.api(&commit_path) {
        Api::Ok(body) => body
            .pointer(commit_pointer)
            .and_then(Value::as_str)
            .map(str::to_owned),
        Api::Missing => None,
        Api::Failed(reason) => return unknown(reason),
    };
    let Some(commit) = commit else {
        return milestone(
            "not-observed",
            format!("the newest release, {tag}, names no commit the forge serves"),
        );
    };
    let runs_path = match forge {
        Forge::Github => {
            let workflow = match driver {
                "rust" => "release-plz.yml",
                "python" => "release-please.yml",
                _ => "release.yml",
            };
            format!(
                "repos/{}/actions/workflows/{workflow}/runs?branch={trunk}&head_sha={commit}&event=push&status=success&per_page=1",
                ctx.repo
            )
        }
        Forge::Gitlab => format!(
            "projects/{}/pipelines?ref={trunk}&sha={commit}&status=success&per_page=1",
            release_state::gitlab_project(&ctx.repo)
        ),
    };
    let ran = match cli.api(&runs_path) {
        Api::Ok(body) => body["total_count"].as_u64().map_or_else(
            || body.as_array().is_some_and(|list| !list.is_empty()),
            |count| count > 0,
        ),
        Api::Missing => false,
        Api::Failed(reason) => return unknown(reason),
    };
    if ran {
        milestone(
            "observed",
            format!("a publish run succeeded on {trunk} at the commit {tag} names"),
        )
    } else {
        milestone(
            "not-observed",
            format!("the newest release, {tag}, came from no successful publish run on {trunk}"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{operator_forms, word};
    use crate::release_state::Answer;

    #[test]
    fn each_registry_form_carries_its_read_back_check() {
        let rust: Vec<&str> = operator_forms("rust", "widget")
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(
            rust,
            [
                "trusted-publisher",
                "bootstrap-token-revoked",
                "trusted-publishing-required"
            ]
        );
        assert!(
            operator_forms("rust", "widget")[0]
                .1
                .contains("crates.io/crates/widget/settings")
        );
        assert_eq!(operator_forms("python", "widget").len(), 2);
        assert!(operator_forms("bash", "widget").is_empty());
        assert_eq!(word(Answer::Yes), "observed");
        assert_eq!(word(Answer::No), "not-observed");
    }
}
