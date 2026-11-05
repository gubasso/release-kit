//! The `dev_environment` table: how a command runs in the project's own
//! development environment, and how CI puts that environment on a runner.
//!
//! release-kit assumes no environment manager. The operator records the
//! answer, nothing infers it from a file the target carries, and a target
//! with no answer renders what it rendered before the table existed.
//!
//! SATISFIES target-config:a-dev-environment-is-a-recorded-answer

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The recorded answer. A configuration with no table gives no answer, and
/// a table whose `run` is empty states that the project gives none, which
/// clears a recorded one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct DevEnvironment {
    /// The command prefix that runs a command in the environment, such as
    /// `nix develop --command` or `mise exec --`.
    pub run: String,
    /// How a GitHub runner obtains the environment.
    #[serde(skip_serializing_if = "GithubSetup::is_empty")]
    pub github: GithubSetup,
    /// How a GitLab job obtains the environment.
    #[serde(skip_serializing_if = "GitlabSetup::is_empty")]
    pub gitlab: GitlabSetup,
}

/// The GitHub steps that put the environment on a runner.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct GithubSetup {
    /// Steps every job that runs a project tool takes first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub setup: Vec<ActionStep>,
    /// Steps that cache the environment. A job that publishes or holds a
    /// write token never takes them, because any workflow that writes the
    /// same cache would then feed the job that publishes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cache: Vec<ActionStep>,
}

impl GithubSetup {
    const fn is_empty(&self) -> bool {
        self.setup.is_empty() && self.cache.is_empty()
    }
}

/// The GitLab image and setup lines of a job that runs a project tool.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct GitlabSetup {
    /// The image the job runs in.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub image: String,
    /// Lines the job runs before its script.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub before_script: Vec<String>,
}

impl GitlabSetup {
    const fn is_empty(&self) -> bool {
        self.image.is_empty() && self.before_script.is_empty()
    }
}

/// One action step, pinned to a full commit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct ActionStep {
    /// `owner/repo[/path]@<40 hex digits>`.
    pub uses: String,
    /// The release the commit belongs to, rendered as a trailing comment.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tag: String,
    /// The step's inputs.
    #[serde(rename = "with", skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<String, Input>,
}

/// One input value: the scalar kinds a workflow input takes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Input {
    /// `true` or `false`.
    Bool(bool),
    /// A whole number.
    Integer(i64),
    /// Any one-line text.
    Text(String),
}

impl Input {
    /// The YAML scalar a workflow carries. Text renders as a JSON string,
    /// which is a valid YAML double-quoted scalar, so it stays data.
    #[must_use]
    pub fn yaml(&self) -> String {
        match self {
            Self::Bool(value) => value.to_string(),
            Self::Integer(value) => value.to_string(),
            Self::Text(value) => serde_json::Value::String(value.clone()).to_string(),
        }
    }
}

impl DevEnvironment {
    /// The answer a landing renders through, or `None` where the table
    /// states that the project gives none.
    #[must_use]
    pub fn answered(self) -> Option<Self> {
        (!self.run.is_empty()).then_some(self)
    }

    /// Judge the table, naming the first key that fails.
    ///
    /// # Errors
    /// The refusal text, naming the key and what it must be.
    pub fn check(&self) -> Result<(), String> {
        if self.run.is_empty() {
            if self.github.is_empty() && self.gitlab.is_empty() {
                return Ok(());
            }
            return Err("dev_environment.run is empty while the table names a setup; an empty run states that the project gives no answer".to_owned());
        }
        check_run(&self.run)?;
        for (list, steps) in [("setup", &self.github.setup), ("cache", &self.github.cache)] {
            for (at, step) in steps.iter().enumerate() {
                check_step(&format!("dev_environment.github.{list}[{at}]"), step)?;
            }
        }
        if !self.gitlab.image.is_empty() && !is_image(&self.gitlab.image) {
            return Err(format!(
                "dev_environment.gitlab.image is {:?}, which is no image reference; write one such as nixos/nix:2.35.2",
                self.gitlab.image
            ));
        }
        for (at, line) in self.gitlab.before_script.iter().enumerate() {
            one_line(&format!("dev_environment.gitlab.before_script[{at}]"), line)?;
        }
        Ok(())
    }

    /// Judge the answer against the forge it lands on: a job runs the
    /// environment only where that forge's setup is named.
    ///
    /// # Errors
    /// The refusal text, naming the missing key.
    pub fn check_forge(&self, forge: Option<&str>) -> Result<(), String> {
        match forge {
            Some("github") if self.github.setup.is_empty() => Err(
                "dev_environment.run is set and dev_environment.github.setup names no step; a GitHub runner needs the steps that put the environment on it".to_owned(),
            ),
            Some("gitlab") if self.gitlab.image.is_empty() => Err(
                "dev_environment.run is set and dev_environment.gitlab.image is empty; a GitLab job needs the image that carries the environment".to_owned(),
            ),
            _ => Ok(()),
        }
    }
}

/// The prefix lands in a shell line and in a plain YAML scalar, so it is
/// one line that starts with a word character and carries nothing YAML
/// reads as structure.
fn check_run(run: &str) -> Result<(), String> {
    one_line("dev_environment.run", run)?;
    let starts = run
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '_'));
    if !starts || run.trim() != run || run.contains(" #") || run.contains(": ") {
        return Err(format!(
            "dev_environment.run is {run:?}; a prefix starts with a letter, a digit, '.', '/', or '_', has no surrounding space, and carries no ' #' and no ': '"
        ));
    }
    Ok(())
}

fn check_step(key: &str, step: &ActionStep) -> Result<(), String> {
    if !is_pinned(&step.uses) {
        return Err(format!(
            "{key}.uses is {:?}, which is not pinned to a full commit SHA; write owner/repo@<40 hex digits>",
            step.uses
        ));
    }
    if !step.tag.is_empty()
        && !step
            .tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
    {
        return Err(format!(
            "{key}.tag is {:?}; a tag is letters, digits, '.', '_', '-', and '/'",
            step.tag
        ));
    }
    for (name, value) in &step.inputs {
        let named = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'));
        if !named {
            return Err(format!(
                "{key}.with names {name:?}; an input name is letters, digits, '_', and '-'"
            ));
        }
        if let Input::Text(text) = value {
            one_line(&format!("{key}.with.{name}"), text)?;
        }
    }
    Ok(())
}

/// `owner/repo[/path]@<40 lowercase hex digits>`.
fn is_pinned(uses: &str) -> bool {
    let Some((action, commit)) = uses.rsplit_once('@') else {
        return false;
    };
    let mut parts = action.split('/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts.next().unwrap_or_default();
    let word = |part: &str| {
        !part.is_empty()
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    word(owner)
        && word(repo)
        && parts.all(word)
        && commit.len() == 40
        && commit
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn is_image(image: &str) -> bool {
    image
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
        && image
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@'))
}

fn one_line(key: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(format!(
            "{key} is {value:?}; it is one non-empty line with no control character"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ActionStep, DevEnvironment, GithubSetup, GitlabSetup, Input};

    const SHA: &str = "90bb610b90bf290cad97484ba341453bd1cbefea";

    fn answer(uses: &str) -> DevEnvironment {
        DevEnvironment {
            run: "nix develop --command".to_owned(),
            github: GithubSetup {
                setup: vec![ActionStep {
                    uses: uses.to_owned(),
                    tag: "v19".to_owned(),
                    inputs: [("use-flakehub".to_owned(), Input::Bool(false))].into(),
                }],
                cache: Vec::new(),
            },
            gitlab: GitlabSetup::default(),
        }
    }

    #[test]
    fn a_pinned_answer_is_accepted() {
        let uses = format!("DeterminateSystems/nix-installer-action@{SHA}");
        assert_eq!(answer(&uses).check(), Ok(()));
        assert_eq!(answer(&uses).check_forge(Some("github")), Ok(()));
    }

    #[test]
    fn an_unpinned_setup_reference_refuses_by_name() {
        for uses in [
            "DeterminateSystems/nix-installer-action@v19",
            "DeterminateSystems/nix-installer-action",
            "nix-installer-action@90bb610b90bf290cad97484ba341453bd1cbefea",
            "DeterminateSystems/nix-installer-action@90BB610B90BF290CAD97484BA341453BD1CBEFEA",
        ] {
            let refusal = answer(uses).check().expect_err(uses);
            assert!(
                refusal.contains("dev_environment.github.setup[0].uses")
                    && refusal.contains("full commit SHA"),
                "{refusal}"
            );
        }
    }

    #[test]
    fn a_run_that_breaks_its_line_refuses() {
        for run in [
            "",
            " nix develop",
            "nix develop # here",
            "a: b",
            "nix\ndevelop",
            "-x",
        ] {
            let mut held = answer(&format!("o/r@{SHA}"));
            held.run = run.to_owned();
            assert!(held.check().is_err(), "{run:?}");
        }
    }

    #[test]
    fn a_run_with_no_forge_setup_refuses_on_that_forge() {
        let mut held = answer(&format!("o/r@{SHA}"));
        held.github.setup.clear();
        assert!(
            held.check_forge(Some("github"))
                .is_err_and(|refusal| refusal.contains("dev_environment.github.setup"))
        );
        assert!(
            held.check_forge(Some("gitlab"))
                .is_err_and(|refusal| refusal.contains("dev_environment.gitlab.image"))
        );
        assert_eq!(held.check_forge(None), Ok(()));
    }

    #[test]
    fn an_empty_run_states_no_answer() {
        assert_eq!(DevEnvironment::default().check(), Ok(()));
        assert_eq!(DevEnvironment::default().answered(), None);
        let mut held = answer(&format!("o/r@{SHA}"));
        held.run.clear();
        assert!(held.check().is_err());
    }
}
