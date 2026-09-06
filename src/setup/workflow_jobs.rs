//! What a target's own workflows say about the job `--required-check` names.
//!
//! The trunk protection requires exactly two status-check contexts: the
//! named check and the title check. Every other job the project's workflow
//! reports on a pull request gates nothing, and under the trunk style's
//! standing arm nobody reads the check list before the merge. This reader
//! finds those jobs so `rk setup check` can name them as a limitation on
//! `protect-trunk`. It reads the workflow text line by line, in the same
//! spirit as the landing invariants: where a value sits somewhere this
//! reader does not follow, it says so rather than guessing.

use camino::Utf8Path;

use crate::landing::invariants::before_comment;
use crate::setup::observe::TITLE_CHECK;

/// What the workflows say about the required check.
#[derive(Debug, PartialEq, Eq)]
pub enum GateReading {
    /// No workflow directory, or no workflow runs on a pull request.
    NoRequestWorkflows,
    /// Every request-reporting context is the check, the title check, or a
    /// job the check needs.
    Gated,
    /// No job reports the required check's context on a pull request.
    NoSuchJob {
        /// The contexts that do report, in file order.
        contexts: Vec<String>,
    },
    /// The check's `needs` value is one this reader does not follow.
    OpaqueNeeds {
        /// The workflow file that carries it.
        workflow: String,
    },
    /// Contexts that report on a pull request and gate nothing.
    Ungated {
        /// Their names, in file order.
        jobs: Vec<String>,
    },
}

/// The reading and the one property of the gate job it judges beside it.
#[derive(Debug, PartialEq, Eq)]
pub struct GateReport {
    /// What the workflows say.
    pub reading: GateReading,
    /// The gate job runs without `if: always()`: a needed job that fails
    /// skips it, and the forge reports a skipped job as success.
    pub gate_lacks_always: bool,
}

/// One job as the line reader sees it.
#[derive(Debug, PartialEq, Eq)]
struct Job {
    id: String,
    name: Option<String>,
    needs: Needs,
    always: bool,
}

impl Job {
    /// The status-check context the job reports: its name where it sets
    /// one, else its id.
    fn context(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

/// A job's `needs` value.
#[derive(Debug, PartialEq, Eq)]
enum Needs {
    /// The key is absent.
    None,
    /// The job ids named, in a scalar, a flow list, or a block list.
    Listed(Vec<String>),
    /// An expression, an anchor, or a folded scalar: not followed.
    Opaque,
}

/// Read every workflow under the target's `.github/workflows` and judge
/// the named check against the jobs that report on a pull request.
#[must_use]
pub fn read_gate(target: &Utf8Path, required_check: &str) -> GateReport {
    let none = GateReport {
        reading: GateReading::NoRequestWorkflows,
        gate_lacks_always: false,
    };
    let Ok(entries) = std::fs::read_dir(target.join(".github/workflows")) else {
        return none;
    };
    let mut files: Vec<(String, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_workflow = std::path::Path::new(&name)
                .extension()
                .is_some_and(|ext| ext == "yml" || ext == "yaml");
            if !is_workflow {
                return None;
            }
            let text = std::fs::read_to_string(entry.path()).ok()?;
            runs_on_request(&text).then_some((name, text))
        })
        .collect();
    files.sort();
    let workflows: Vec<(String, Vec<Job>)> = files
        .into_iter()
        .map(|(name, text)| (name, jobs(&text)))
        .collect();
    if workflows.iter().all(|(_, jobs)| jobs.is_empty()) {
        return none;
    }
    let contexts: Vec<String> = workflows
        .iter()
        .flat_map(|(_, jobs)| jobs.iter().map(|job| job.context().to_owned()))
        .collect();
    let Some((workflow, own, gate)) = workflows.iter().find_map(|(name, jobs)| {
        jobs.iter()
            .find(|job| job.context() == required_check)
            .map(|job| (name, jobs, job))
    }) else {
        return GateReport {
            reading: GateReading::NoSuchJob { contexts },
            gate_lacks_always: false,
        };
    };
    let gate_lacks_always = !gate.always;
    // A `needs` entry is a job id, and ids are per workflow file, so the
    // gated contexts resolve inside the gate's own file.
    let gated: Vec<&str> = match &gate.needs {
        Needs::Opaque => {
            return GateReport {
                reading: GateReading::OpaqueNeeds {
                    workflow: workflow.clone(),
                },
                gate_lacks_always,
            };
        }
        Needs::None => Vec::new(),
        Needs::Listed(ids) => ids
            .iter()
            .filter_map(|id| own.iter().find(|job| &job.id == id))
            .map(Job::context)
            .collect(),
    };
    let mut ungated: Vec<String> = Vec::new();
    for context in &contexts {
        if context == required_check
            || context == TITLE_CHECK
            || gated.contains(&context.as_str())
            || ungated.contains(context)
        {
            continue;
        }
        ungated.push(context.clone());
    }
    GateReport {
        reading: if ungated.is_empty() {
            GateReading::Gated
        } else {
            GateReading::Ungated { jobs: ungated }
        },
        gate_lacks_always,
    }
}

/// The one-line limitation a report earns, or nothing where the check
/// stands for every request-reporting job.
#[must_use]
pub fn limitation(report: &GateReport, required_check: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    match &report.reading {
        GateReading::NoRequestWorkflows | GateReading::Gated => {}
        GateReading::NoSuchJob { contexts } => parts.push(format!(
            "no job in .github/workflows reports the context {required_check} on a pull request, so the required check cannot be satisfied; the request-reporting contexts are [{}]",
            contexts.join(", ")
        )),
        GateReading::OpaqueNeeds { workflow } => parts.push(format!(
            "the needs value of {required_check} in {workflow} is one this reader does not follow; whether every request-reporting job is gated could not be read"
        )),
        GateReading::Ungated { jobs } => parts.push(format!(
            "the required check {required_check} gates nothing from [{}]: those jobs report on a pull request but the gate does not need them, so a failure there does not hold the merge",
            jobs.join(", ")
        )),
    }
    if report.gate_lacks_always {
        parts.push(format!(
            "the job {required_check} runs without if: always(), so a needed job that fails skips it and the skip reports success"
        ));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// Whether the workflow's `on` names a pull-request event, in the block,
/// the flow, the scalar, or the block-list form.
fn runs_on_request(workflow: &str) -> bool {
    let mut in_on = false;
    for line in workflow.lines() {
        if is_blank(line) {
            continue;
        }
        if indent(line) == 0 {
            in_on = false;
            let Some((key, value)) = key_value(line) else {
                continue;
            };
            if key != "on" {
                continue;
            }
            if value.is_empty() {
                in_on = true;
                continue;
            }
            if list_items(value).iter().any(|item| is_request_event(item)) {
                return true;
            }
            continue;
        }
        if !in_on {
            continue;
        }
        let item = line.trim_start();
        let item = item.strip_prefix("- ").map_or(item, str::trim_start);
        let key = key_value(item).map_or_else(|| before_comment(item).trim(), |(key, _)| key);
        if is_request_event(key) {
            return true;
        }
    }
    false
}

fn is_request_event(name: &str) -> bool {
    matches!(name, "pull_request" | "pull_request_target")
}

/// The jobs the workflow declares under its top-level `jobs` key, with
/// the properties the gate judgment reads. Steps and every deeper mapping
/// are passed over, so a `jobs` key nested in a reusable-workflow call or
/// a matrix opens no job.
fn jobs(workflow: &str) -> Vec<Job> {
    let mut found: Vec<Job> = Vec::new();
    let mut in_jobs = false;
    let mut job_indent: Option<usize> = None;
    let mut property_indent: Option<usize> = None;
    let mut reading_needs_list = false;
    for line in workflow.lines() {
        if is_blank(line) {
            continue;
        }
        let depth = indent(line);
        if depth == 0 {
            in_jobs = key_value(line).is_some_and(|(key, value)| key == "jobs" && value.is_empty());
            job_indent = None;
            property_indent = None;
            reading_needs_list = false;
            continue;
        }
        if !in_jobs {
            continue;
        }
        let job_depth = *job_indent.get_or_insert(depth);
        if depth == job_depth {
            reading_needs_list = false;
            property_indent = None;
            if let Some((id, _)) = key_value(line) {
                found.push(Job {
                    id: id.to_owned(),
                    name: None,
                    needs: Needs::None,
                    always: false,
                });
            }
            continue;
        }
        if depth < job_depth {
            continue;
        }
        let Some(job) = found.last_mut() else {
            continue;
        };
        let property_depth = *property_indent.get_or_insert(depth);
        if reading_needs_list && depth > property_depth {
            if let Some(item) = line.trim_start().strip_prefix("- ") {
                if let Needs::Listed(ids) = &mut job.needs {
                    ids.push(unquote(before_comment(item).trim()).to_owned());
                }
                continue;
            }
        }
        reading_needs_list = false;
        if depth != property_depth {
            continue;
        }
        let Some((key, value)) = key_value(line) else {
            continue;
        };
        match key {
            "name" => {
                let value = unquote(before_comment(value).trim());
                // A name built from an expression resolves per run, so
                // the id is the readable handle for it.
                if !value.is_empty() && !value.contains("${{") {
                    job.name = Some(value.to_owned());
                }
            }
            "if" => job.always = value.contains("always()"),
            "needs" => {
                let value = before_comment(value).trim();
                if value.is_empty() {
                    job.needs = Needs::Listed(Vec::new());
                    reading_needs_list = true;
                } else if value.starts_with(['|', '>', '*', '&', '$']) {
                    job.needs = Needs::Opaque;
                } else {
                    job.needs =
                        Needs::Listed(list_items(value).into_iter().map(str::to_owned).collect());
                }
            }
            _ => {}
        }
    }
    found
}

/// A scalar or a flow list, as its items: `a`, `[a, b]`, or `"a"`.
fn list_items(value: &str) -> Vec<&str> {
    before_comment(value)
        .split(['[', ']', ','])
        .map(|item| unquote(item.trim()))
        .filter(|item| !item.is_empty())
        .collect()
}

/// A `key: value` line split at its first mapping colon, the key bare or
/// quoted as YAML permits for an implicit key.
fn key_value(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    let (key, rest) = if let Some(quoted) = line.strip_prefix(QUOTES) {
        let quote = line.chars().next()?;
        let end = quoted.find(quote)?;
        (&quoted[..end], quoted[end + 1..].trim_start())
    } else {
        let end = line.find(':')?;
        (&line[..end], &line[end..])
    };
    let value = rest.strip_prefix(':')?;
    if !(value.is_empty() || value.starts_with([' ', '\t'])) {
        return None;
    }
    let key = key.trim();
    if key.is_empty() || key.contains([' ', '\t']) {
        return None;
    }
    Some((key, value.trim()))
}

/// The two quote characters a YAML scalar is written with, named by code
/// point because the artifact-body scan reads a lone quote in these
/// sources as a literal opening.
const QUOTES: [char; 2] = ['\u{22}', '\u{27}'];

fn unquote(value: &str) -> &str {
    value
        .strip_prefix(QUOTES[0])
        .and_then(|rest| rest.strip_suffix(QUOTES[0]))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

fn is_blank(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#') || trimmed == "---"
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn report(text: &str, check: &str) -> GateReport {
        let dir = tempfile::tempdir().expect("a tempdir");
        let workflows = dir.path().join(".github/workflows");
        std::fs::create_dir_all(&workflows).expect("the workflows dir");
        std::fs::write(workflows.join("ci.yml"), text).expect("the workflow writes");
        read_gate(
            Utf8Path::from_path(dir.path()).expect("utf-8 tempdir"),
            check,
        )
    }

    #[test]
    fn runs_on_request_reads_every_on_form() {
        assert!(runs_on_request(
            "on:\n  push:\n  pull_request:\n    branches: [main]\n"
        ));
        assert!(runs_on_request("on: [push, pull_request]\n"));
        assert!(runs_on_request("on: pull_request_target\n"));
        assert!(runs_on_request("on:\n  - push\n  - pull_request\n"));
        assert!(runs_on_request("\"on\":\n  pull_request:\n"));
        assert!(!runs_on_request("on: push\n"));
        assert!(!runs_on_request(
            "on:\n  push:\n  workflow_dispatch:\njobs:\n  pull_request:\n"
        ));
    }

    #[test]
    fn jobs_take_name_over_id_and_read_needs_in_every_form() {
        let text = "\
jobs:
  lint:
    runs-on: ubuntu-latest
  build:
    name: \"Build it\" # the context
    needs: lint
  docs:
    needs: [lint, build]
  gate:
    name: build-${{ matrix.os }}
    if: ${{ always() }}
    needs:
      - lint
      - 'docs'
    steps:
      - uses: x@y
        with:
          needs: nothing
  odd:
    needs: ${{ fromJSON(x) }}
";
        let found = jobs(text);
        let ids: Vec<&str> = found.iter().map(|job| job.id.as_str()).collect();
        assert_eq!(ids, ["lint", "build", "docs", "gate", "odd"]);
        assert_eq!(found[1].context(), "Build it");
        assert_eq!(found[1].needs, Needs::Listed(vec!["lint".to_owned()]));
        assert_eq!(
            found[2].needs,
            Needs::Listed(vec!["lint".to_owned(), "build".to_owned()])
        );
        assert!(found[3].always);
        assert_eq!(found[3].context(), "gate");
        assert_eq!(
            found[3].needs,
            Needs::Listed(vec!["lint".to_owned(), "docs".to_owned()])
        );
        assert_eq!(found[4].needs, Needs::Opaque);
        assert_eq!(found[0].needs, Needs::None);
    }

    #[test]
    fn a_nested_jobs_key_opens_no_region() {
        let text = "\
jobs:
  call:
    uses: org/repo/.github/workflows/x.yml@main
    with:
      jobs: 3
  other:
    strategy:
      matrix:
        jobs: [a, b]
";
        let ids: Vec<String> = jobs(text).into_iter().map(|job| job.id).collect();
        assert_eq!(ids, ["call", "other"]);
    }

    #[test]
    fn read_gate_partitions_the_contexts() {
        let gated = report(
            "on: [pull_request]\njobs:\n  lint:\n  test:\n    if: always()\n    needs: [lint]\n",
            "test",
        );
        assert_eq!(gated.reading, GateReading::Gated);
        assert!(!gated.gate_lacks_always);

        let ungated = report(
            "on: [pull_request]\njobs:\n  lint:\n  build:\n  docs:\n  pr-title:\n  test:\n    needs: lint\n",
            "test",
        );
        assert_eq!(
            ungated.reading,
            GateReading::Ungated {
                jobs: vec!["build".to_owned(), "docs".to_owned()]
            }
        );
        assert!(ungated.gate_lacks_always);

        let missing = report("on: [pull_request]\njobs:\n  lint:\n  unit:\n", "test");
        assert_eq!(
            missing.reading,
            GateReading::NoSuchJob {
                contexts: vec!["lint".to_owned(), "unit".to_owned()]
            }
        );

        let opaque = report(
            "on: [pull_request]\njobs:\n  lint:\n  test:\n    needs: *all\n",
            "test",
        );
        assert_eq!(
            opaque.reading,
            GateReading::OpaqueNeeds {
                workflow: "ci.yml".to_owned()
            }
        );

        let push_only = report("on: push\njobs:\n  lint:\n  test:\n", "test");
        assert_eq!(push_only.reading, GateReading::NoRequestWorkflows);

        let dir = tempfile::tempdir().expect("a tempdir");
        let empty = read_gate(Utf8Path::from_path(dir.path()).expect("utf-8"), "test");
        assert_eq!(empty.reading, GateReading::NoRequestWorkflows);
    }

    #[test]
    fn limitation_texts_are_one_line_each() {
        let cases = [
            GateReport {
                reading: GateReading::NoSuchJob {
                    contexts: vec!["lint".to_owned()],
                },
                gate_lacks_always: false,
            },
            GateReport {
                reading: GateReading::OpaqueNeeds {
                    workflow: "ci.yml".to_owned(),
                },
                gate_lacks_always: true,
            },
            GateReport {
                reading: GateReading::Ungated {
                    jobs: vec!["a".to_owned(), "b".to_owned()],
                },
                gate_lacks_always: true,
            },
            GateReport {
                reading: GateReading::Gated,
                gate_lacks_always: true,
            },
        ];
        for case in &cases {
            let text = limitation(case, "test").expect("a limitation");
            assert!(!text.contains('\n'), "{text}");
            assert!(text.starts_with(|c: char| c.is_lowercase()), "{text}");
        }
        let clean = GateReport {
            reading: GateReading::Gated,
            gate_lacks_always: false,
        };
        assert_eq!(limitation(&clean, "test"), None);
        let none = GateReport {
            reading: GateReading::NoRequestWorkflows,
            gate_lacks_always: false,
        };
        assert_eq!(limitation(&none, "test"), None);
    }
}
