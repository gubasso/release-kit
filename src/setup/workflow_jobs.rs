//! Whether the job `--required-check` names is shaped to report a blocking
//! answer.
//!
//! The trunk protection requires exactly two status-check contexts: the
//! named check and the title check. Which other jobs a project means to
//! block a merge is intent, and no file states it, so this reader makes no
//! claim about them: `gate.needs` is the voting list by convention, and
//! `forges/github.md` owns that convention. What this reader judges is the
//! gate itself, in five ways it can fail to report: no job reports the
//! context, more than one does, the condition is not proven to survive a
//! failed dependency, the `needs` value is not a literal list, and the
//! trigger filters the request away. It reads the workflow text line by
//! line, in the same spirit as the landing invariants: where a value sits
//! somewhere this reader does not follow, it says so rather than guessing.
//!
//! It does not prove that the gate holds a merge. A gate under a proven
//! condition with a literal `needs` still passes if its steps never inspect
//! the results, and that is script semantics this reader does not run.

use camino::Utf8Path;

use crate::landing::invariants::before_comment;

/// What the workflows say about the required check.
#[derive(Debug, PartialEq, Eq)]
pub enum GateReading {
    /// No workflow runs on a pull request, so no job reports the check.
    NoRequestWorkflows,
    /// Every request-reporting context is the check, the title check, or a
    /// job the check needs.
    Gated,
    /// No job reports the required check's context on a pull request.
    NoSuchJob {
        /// The contexts that do report, in file order.
        contexts: Vec<String>,
    },
    /// A job carries the check's id, but names itself by an expression or
    /// runs a reusable workflow, so the context it reports is not in the
    /// file.
    UnprovenGateName {
        /// The job id.
        job: String,
    },
    /// The check's `needs` value is one this reader does not follow.
    OpaqueNeeds {
        /// The workflow file that carries it.
        workflow: String,
    },
}

/// The gate job's `if` condition, as far as the reader proves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// No `if` key: the job is skipped when a needed job fails.
    Absent,
    /// `always()`, or `!cancelled()` written so that YAML reads it as text:
    /// the job runs when a needed job fails, which is the property the gate
    /// rests on. `rust-lang/cargo` uses the second deliberately, so that a
    /// manual cancel does not turn the gate red.
    Proven,
    /// A scalar opening with `!`, which YAML reads as a tag rather than as
    /// text, carried verbatim. The forge never sees the expression, so the
    /// workflow does not parse and the check never reports.
    UnquotedTag(String),
    /// Any other expression, carried verbatim: not proven to run on a
    /// failed dependency.
    Other(String),
}

/// The reading and what the reader judges beside it.
#[derive(Debug, PartialEq, Eq)]
pub struct GateReport {
    /// What the workflows say.
    pub reading: GateReading,
    /// The gate job's condition, where a gate job was found.
    pub gate_condition: Option<Condition>,
    /// What the gate's workflow filters its pull-request trigger by.
    pub gate_trigger: Trigger,
    /// How many jobs report the required context on a pull request. Where
    /// a name is required, every reporter of it must pass, so a second one
    /// takes the merge decision out of the gate's hands.
    pub reporting: usize,
    /// Workflow files that could not be read, so their jobs are unjudged.
    pub unreadable: Vec<String>,
}

/// One job as the line reader sees it.
#[derive(Debug, PartialEq, Eq)]
struct Job {
    id: String,
    name: Name,
    /// The job calls a reusable workflow, whose jobs report their own
    /// contexts, named after both the caller and the callee.
    reusable: bool,
    needs: Needs,
    condition: Condition,
}

/// How a job's status-check context is known.
#[derive(Debug, PartialEq, Eq)]
enum Name {
    /// No `name` key: the context is the id.
    Id,
    /// A literal `name` value.
    Fixed(String),
    /// A name built from an expression: not in this file.
    Unproven,
}

impl Job {
    /// The status-check context the job reports, where the file states it.
    fn context(&self) -> Option<&str> {
        if self.reusable {
            return None;
        }
        match &self.name {
            Name::Id => Some(&self.id),
            Name::Fixed(name) => Some(name),
            Name::Unproven => None,
        }
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

/// What a workflow's pull-request trigger filters by.
///
/// Every filter can keep the gate from reporting on a request the trunk
/// protection covers, and a required context that never appears leaves
/// the merge hanging.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Trigger {
    /// The trigger carries `paths` or `paths-ignore`.
    pub paths_filtered: bool,
    /// The trigger's branch filter leaves the trunk out, quoted.
    pub misses_trunk: Option<String>,
    /// The trigger's activity types leave out an opened, reopened, or
    /// synchronized request, quoted.
    pub types_filtered: Option<String>,
}

impl Trigger {
    /// Read the filters one request event carries.
    fn from_filters(filters: &[(String, Vec<String>)], trunk: &str) -> Self {
        let mut trigger = Self::default();
        for (key, items) in filters {
            match key.as_str() {
                "paths" | "paths-ignore" => trigger.paths_filtered = true,
                // A negative pattern later in the list can take the trunk
                // back out, so a list carrying one is not proven either way.
                "branches" => {
                    let negated = items.iter().any(|item| item.starts_with('!'));
                    if negated || !items.iter().any(|item| covers_trunk(item, trunk)) {
                        trigger.misses_trunk = Some(format!("branches: [{}]", items.join(", ")));
                    }
                }
                // A glob here may match the trunk, and the reader does not
                // run the forge's matcher, so only a literal other name is
                // proven harmless.
                "branches-ignore" => {
                    if items
                        .iter()
                        .any(|item| covers_trunk(item, trunk) || is_glob(item))
                    {
                        trigger.misses_trunk =
                            Some(format!("branches-ignore: [{}]", items.join(", ")));
                    }
                }
                "types" => {
                    let needed = ["opened", "synchronize", "reopened"];
                    if !needed
                        .iter()
                        .all(|kind| items.iter().any(|item| item == kind))
                    {
                        trigger.types_filtered = Some(format!("types: [{}]", items.join(", ")));
                    }
                }
                _ => {}
            }
        }
        trigger
    }

    /// Fold a second request event's filters in: a filter on either event
    /// is reported.
    fn merge(&mut self, other: Self) {
        self.paths_filtered |= other.paths_filtered;
        if self.misses_trunk.is_none() {
            self.misses_trunk = other.misses_trunk;
        }
        if self.types_filtered.is_none() {
            self.types_filtered = other.types_filtered;
        }
    }
}

/// Whether a branch pattern names the trunk: its exact name, or a glob
/// that matches every branch. Any other glob is not proven to.
fn covers_trunk(pattern: &str, trunk: &str) -> bool {
    pattern == trunk || pattern == "*" || pattern == "**"
}

/// Whether a branch pattern carries a glob or negation character, so its
/// matches are the forge's to decide, not this reader's.
fn is_glob(pattern: &str) -> bool {
    pattern.contains(['*', '?', '[', ']', '+', '!'])
}

/// One workflow file that runs on a pull request.
struct Workflow {
    name: String,
    trigger: Trigger,
    jobs: Vec<Job>,
}

/// Read every workflow under the target's `.github/workflows` and judge
/// the named check against the jobs that report on a pull request.
#[must_use]
pub fn read_gate(target: &Utf8Path, required_check: &str, trunk: &str) -> GateReport {
    let (workflows, unreadable) = read_workflows(&target.join(".github/workflows"), trunk);
    let mut report = GateReport {
        reading: GateReading::NoRequestWorkflows,
        gate_condition: None,
        gate_trigger: Trigger::default(),
        reporting: 0,
        unreadable,
    };
    if workflows.iter().all(|workflow| workflow.jobs.is_empty()) {
        return report;
    }
    judge(&mut report, &workflows, required_check);
    report
}

/// Every request-running workflow under the directory, in name order, and
/// every path that could not be read. A missing directory is neither: a
/// target with no workflows reads as none, not as unreadable.
fn read_workflows(dir: &Utf8Path, trunk: &str) -> (Vec<Workflow>, Vec<String>) {
    let mut unreadable: Vec<String> = Vec::new();
    let mut workflows: Vec<Workflow> = Vec::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            let mut names: Vec<String> = Vec::new();
            for entry in entries {
                match entry {
                    Ok(entry) => names.push(entry.file_name().to_string_lossy().into_owned()),
                    Err(_) => unreadable.push(dir.to_string()),
                }
            }
            names.sort();
            for name in names {
                let is_workflow = std::path::Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext == "yml" || ext == "yaml");
                if !is_workflow {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(dir.join(&name)) else {
                    unreadable.push(name);
                    continue;
                };
                if let Some(trigger) = request_trigger(&text, trunk) {
                    workflows.push(Workflow {
                        name,
                        trigger,
                        jobs: jobs(&text),
                    });
                }
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => unreadable.push(dir.to_string()),
    }
    (workflows, unreadable)
}

/// The gate judgment over workflows that declare at least one job.
///
/// The judgment is about the gate alone. Which other jobs a project means
/// to block a merge is intent, and no file states it, so a job outside the
/// gate's `needs` is neither counted nor named here.
fn judge(report: &mut GateReport, workflows: &[Workflow], required_check: &str) {
    // Every job reporting the required context, across every request
    // workflow: one is the gate, and a second makes the required check
    // ambiguous.
    report.reporting = workflows
        .iter()
        .flat_map(|workflow| &workflow.jobs)
        .filter(|job| job.context() == Some(required_check))
        .count();
    let Some((workflow, gate)) = workflows.iter().find_map(|workflow| {
        workflow
            .jobs
            .iter()
            .find(|job| job.context() == Some(required_check))
            .map(|job| (workflow, job))
    }) else {
        let unproven = workflows
            .iter()
            .flat_map(|workflow| &workflow.jobs)
            .find(|job| job.id == required_check && job.context().is_none());
        report.reading = unproven.map_or_else(
            || GateReading::NoSuchJob {
                // The contexts that do report, which is the remediation an
                // operator acts on: one of these is the name to require.
                contexts: workflows
                    .iter()
                    .flat_map(|workflow| workflow.jobs.iter().filter_map(Job::context))
                    .map(str::to_owned)
                    .collect(),
            },
            |job| GateReading::UnprovenGateName {
                job: job.id.clone(),
            },
        );
        return;
    };
    report.gate_condition = Some(gate.condition.clone());
    report.gate_trigger = workflow.trigger.clone();
    // A `needs` value the reader does not follow is refused rather than
    // interpreted: an anchor, an alias, or an expression names a voting
    // list nobody can read from the file.
    report.reading = match &gate.needs {
        Needs::Opaque => GateReading::OpaqueNeeds {
            workflow: workflow.name.clone(),
        },
        Needs::None | Needs::Listed(_) => GateReading::Gated,
    };
}

/// The ways the gate is shaped so that it cannot report a blocking answer,
/// or nothing where its shape is sound.
///
/// Every part is a fault on `protect-trunk`, not a limitation: a required
/// check that cannot report is a broken trunk protection.
#[must_use]
pub fn faults(report: &GateReport, required_check: &str, trunk: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    match &report.reading {
        GateReading::Gated => {}
        GateReading::NoRequestWorkflows => parts.push(format!(
            "no workflow in .github/workflows runs on a pull request, so the required check {required_check} never reports and every merge hangs; name a job that runs on a pull request, or remove the required context"
        )),
        GateReading::NoSuchJob { contexts } => parts.push(format!(
            "no job in .github/workflows reports the context {required_check} on a pull request, so the required check never reports and every merge hangs; the contexts that do report are [{}]",
            contexts.join(", ")
        )),
        GateReading::UnprovenGateName { job } => parts.push(format!(
            "the job {job} names itself by an expression or runs a reusable workflow, so the context it reports is not in the file and {required_check} is not proven to exist; give the job a literal name equal to the required context"
        )),
        GateReading::OpaqueNeeds { workflow } => parts.push(format!(
            "the needs value of {required_check} in {workflow} is an anchor, an alias, or an expression, which this reader refuses rather than interprets; write it as a literal list of job ids"
        )),
    }
    if report.reporting > 1 {
        parts.push(format!(
            "the context {required_check} is reported by {} jobs on a pull request, so the required check no longer stands for the gate alone: every reporter of a required name must pass, and a job outside the gate can hold or release the merge; rename all but one",
            report.reporting
        ));
    }
    match &report.gate_condition {
        None | Some(Condition::Proven) => {}
        Some(Condition::Absent) => parts.push(format!(
            "the job {required_check} runs under no if condition, so a needed job that fails skips it and the forge reads a skip as success; use if: always(), or if: ${{{{ !cancelled() }}}}"
        )),
        Some(Condition::UnquotedTag(raw)) => parts.push(format!(
            "the condition of {required_check} reads {raw}, and an unquoted scalar opening with ! is a YAML tag rather than text, so the workflow does not parse and the check never reports; write it as ${{{{ !cancelled() }}}} or quote it"
        )),
        Some(Condition::Other(expression)) => parts.push(format!(
            "the job {required_check} runs under the condition {expression}, which this reader cannot prove holds when a needed job fails; always() or ${{{{ !cancelled() }}}} is the proven form"
        )),
    }
    if report.gate_trigger.paths_filtered {
        parts.push(format!(
            "the pull_request trigger of the workflow carrying {required_check} filters by paths, so a request outside them never reports the check and its merge hangs"
        ));
    }
    if let Some(filter) = &report.gate_trigger.misses_trunk {
        parts.push(format!(
            "the pull_request trigger of the workflow carrying {required_check} reads {filter}, which does not prove it runs for a request against {trunk}, so the check would never report there"
        ));
    }
    if let Some(filter) = &report.gate_trigger.types_filtered {
        parts.push(format!(
            "the pull_request trigger of the workflow carrying {required_check} reads {filter}, which leaves out one of opened, reopened, and synchronize, so a request in that state never reports the check"
        ));
    }
    if !report.unreadable.is_empty() {
        parts.push(format!(
            "[{}] could not be read, so no job there is judged and the context is not proven unique",
            report.unreadable.join(", ")
        ));
    }
    (!parts.is_empty()).then(|| parts.join("; "))
}

/// The workflow's pull-request trigger, in the block, the flow, the
/// scalar, or the block-list form of `on`, where it has one, with the
/// filters a block-form event carries under it.
///
/// The landing invariant reads it too, to ask whether a generated
/// workflow reports on a request at all: one reader owns the forms `on`
/// takes, so a form one of them learns is a form both know.
pub(crate) fn request_trigger(workflow: &str, trunk: &str) -> Option<Trigger> {
    let mut in_on = false;
    let mut event_indent: Option<usize> = None;
    let mut in_request_event = false;
    let mut filter_indent: Option<usize> = None;
    let mut filters: Vec<(String, Vec<String>)> = Vec::new();
    let mut found: Option<Trigger> = None;
    let close_event = |filters: &mut Vec<(String, Vec<String>)>, found: &mut Option<Trigger>| {
        if let Some(trigger) = found {
            trigger.merge(Trigger::from_filters(filters, trunk));
        }
        filters.clear();
    };
    for line in workflow.lines() {
        if is_blank(line) {
            continue;
        }
        let depth = indent(line);
        if depth == 0 {
            if in_request_event {
                close_event(&mut filters, &mut found);
            }
            in_on = false;
            in_request_event = false;
            event_indent = None;
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
                found.get_or_insert_with(Trigger::default);
            }
            continue;
        }
        if !in_on {
            continue;
        }
        let event_depth = *event_indent.get_or_insert(depth);
        if depth == event_depth {
            if in_request_event {
                close_event(&mut filters, &mut found);
            }
            filter_indent = None;
            let item = line.trim_start();
            let item = item.strip_prefix("- ").map_or(item, str::trim_start);
            let key = key_value(item).map_or_else(|| before_comment(item).trim(), |(key, _)| key);
            in_request_event = is_request_event(key);
            if in_request_event {
                found.get_or_insert_with(Trigger::default);
            }
            continue;
        }
        if !in_request_event || depth <= event_depth {
            continue;
        }
        let filter_depth = *filter_indent.get_or_insert(depth);
        if depth == filter_depth {
            if let Some((key, value)) = key_value(line) {
                let items = if value.is_empty() {
                    Vec::new()
                } else {
                    list_items(value).into_iter().map(str::to_owned).collect()
                };
                filters.push((key.to_owned(), items));
            }
            continue;
        }
        // A block-list item under the last filter key.
        if let Some(item) = line.trim_start().strip_prefix("- ") {
            if let Some((_, items)) = filters.last_mut() {
                items.push(unquote(before_comment(item).trim()).to_owned());
            }
        }
    }
    if in_request_event {
        close_event(&mut filters, &mut found);
    }
    found
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
                    name: Name::Id,
                    reusable: false,
                    needs: Needs::None,
                    condition: Condition::Absent,
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
                // the context it reports is not in the file.
                if value.contains("${{") || value.is_empty() {
                    job.name = Name::Unproven;
                } else {
                    job.name = Name::Fixed(value.to_owned());
                }
            }
            // A reusable-workflow call reports the called jobs' contexts,
            // named after both the caller and the callee, whatever name
            // the caller sets and in whatever key order.
            "uses" => job.reusable = true,
            "if" => job.condition = condition(value),
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

/// A job's `if` value: `always()` and `!cancelled()` are the two
/// expressions proven to run on a failed dependency. Anything else is
/// carried verbatim, because `always() && x` skips when `x` is false and a
/// skipped job reports success.
///
/// `!cancelled()` is here because `rust-lang/cargo` uses it deliberately,
/// so that a manual cancel does not turn the gate red. It runs on a failed
/// dependency exactly as `always()` does, which is the property the gate
/// rests on.
///
/// The `!` needs the expression braces or quotes to survive YAML: an
/// unquoted scalar opening with `!` is a tag, not text, so `if:
/// !cancelled()` does not parse and the forge never runs the workflow. The
/// raw scalar is therefore read before it is unquoted, and the bare form is
/// its own fault rather than a pass.
fn condition(value: &str) -> Condition {
    let raw = before_comment(value).trim();
    let value = unquote(raw);
    let inner = value
        .strip_prefix("${{")
        .and_then(|rest| rest.strip_suffix("}}"))
        .map_or(value, str::trim);
    if raw.starts_with('!') {
        return Condition::UnquotedTag(raw.to_owned());
    }
    if inner == "always()" || inner == "!cancelled()" {
        Condition::Proven
    } else if inner.is_empty() {
        Condition::Other("(a value carried on another line)".to_owned())
    } else {
        Condition::Other(inner.to_owned())
    }
}

/// A scalar or a flow list, as its items: `a`, `[a, b]`, or `"a"`. The
/// outer brackets alone delimit the list, and a comma inside a quoted
/// scalar separates nothing, so a bracketed glob such as `'ma[as]ter'`
/// stays one item and reaches the judgment whole.
fn list_items(value: &str) -> Vec<&str> {
    let value = before_comment(value).trim();
    let inner = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(value);
    let mut items = Vec::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut start = 0;
    for (index, character) in inner.char_indices() {
        if let Some(open) = quote {
            // A double-quoted scalar escapes with a backslash, so the
            // quote after one is content, not the close.
            if escaped {
                escaped = false;
            } else if open == QUOTES[0] && character == '\\' {
                escaped = true;
            } else if character == open {
                quote = None;
            }
        } else if QUOTES.contains(&character) {
            quote = Some(character);
        } else if character == ',' {
            items.push(&inner[start..index]);
            start = index + 1;
        }
    }
    items.push(&inner[start..]);
    items
        .into_iter()
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
            "master",
        )
    }

    fn unfiltered() -> Trigger {
        Trigger::default()
    }

    #[test]
    fn the_trigger_is_read_in_every_on_form() {
        assert_eq!(
            request_trigger(
                "on:\n  push:\n  pull_request:\n    branches: [master]\n",
                "master"
            ),
            Some(unfiltered())
        );
        assert_eq!(
            request_trigger(
                "on:\n  push:\n  pull_request:\n    branches: [main]\n",
                "master"
            ),
            Some(Trigger {
                misses_trunk: Some("branches: [main]".to_owned()),
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches-ignore:\n      - master\n    types: [opened]\n",
                "master"
            ),
            Some(Trigger {
                misses_trunk: Some("branches-ignore: [master]".to_owned()),
                types_filtered: Some("types: [opened]".to_owned()),
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches: ['**']\n    types: [opened, synchronize, reopened]\n",
                "master"
            ),
            Some(unfiltered())
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches: ['**', '!master']\n",
                "master"
            ),
            Some(Trigger {
                misses_trunk: Some("branches: [**, !master]".to_owned()),
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches: ['!master', '**']\n",
                "master"
            ),
            Some(Trigger {
                misses_trunk: Some("branches: [!master, **]".to_owned()),
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches-ignore: ['mast*']\n",
                "master"
            ),
            Some(Trigger {
                misses_trunk: Some("branches-ignore: [mast*]".to_owned()),
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches-ignore: [dependabot]\n",
                "master"
            ),
            Some(unfiltered())
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches-ignore: ['ma[as]ter']\n",
                "master"
            ),
            Some(Trigger {
                misses_trunk: Some("branches-ignore: [ma[as]ter]".to_owned()),
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches: [\"release/**\", 'a,b', master]\n",
                "master"
            ),
            Some(unfiltered())
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    branches: [\"topic\\\",master,tail\"]\n",
                "master"
            ),
            Some(Trigger {
                misses_trunk: Some("branches: [topic\\\",master,tail]".to_owned()),
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger("on: [push, pull_request]\n", "master"),
            Some(unfiltered())
        );
        assert_eq!(
            request_trigger("on: pull_request_target\n", "master"),
            Some(unfiltered())
        );
        assert_eq!(
            request_trigger("on:\n  - push\n  - pull_request\n", "master"),
            Some(unfiltered())
        );
        assert_eq!(
            request_trigger("\"on\":\n  pull_request:\n", "master"),
            Some(unfiltered())
        );
        assert_eq!(request_trigger("on: push\n", "master"), None);
        assert_eq!(
            request_trigger(
                "on:\n  push:\n  workflow_dispatch:\njobs:\n  pull_request:\n",
                "master"
            ),
            None
        );
        assert_eq!(
            request_trigger(
                "on:\n  pull_request:\n    paths:\n      - 'docs/**'\n  push:\n",
                "master"
            ),
            Some(Trigger {
                paths_filtered: true,
                ..Trigger::default()
            })
        );
        assert_eq!(
            request_trigger(
                "on:\n  push:\n    paths: [x]\n  pull_request:\n    branches: [master]\n",
                "master"
            ),
            Some(unfiltered())
        );
    }

    #[test]
    fn jobs_read_names_needs_and_conditions_in_every_form() {
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
    name: gate-${{ matrix.os }}
    if: ${{ always() }}
    needs:
      - lint
      - 'docs'
    steps:
      - uses: x@y
        with:
          needs: nothing
  odd:
    if: always() && needs.lint.result == 'success'
    needs: ${{ fromJSON(x) }}
  called:
    uses: org/repo/.github/workflows/x.yml@main
    name: called
  named-first:
    name: gate
    uses: org/repo/.github/workflows/x.yml@main
";
        let found = jobs(text);
        let ids: Vec<&str> = found.iter().map(|job| job.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "lint",
                "build",
                "docs",
                "gate",
                "odd",
                "called",
                "named-first"
            ]
        );
        assert_eq!(found[0].needs, Needs::None);
        assert_eq!(found[0].condition, Condition::Absent);
        assert_eq!(found[1].context(), Some("Build it"));
        assert_eq!(found[1].needs, Needs::Listed(vec!["lint".to_owned()]));
        assert_eq!(
            found[2].needs,
            Needs::Listed(vec!["lint".to_owned(), "build".to_owned()])
        );
        assert_eq!(found[3].name, Name::Unproven);
        assert_eq!(found[3].context(), None);
        assert_eq!(found[3].condition, Condition::Proven);
        assert_eq!(
            found[3].needs,
            Needs::Listed(vec!["lint".to_owned(), "docs".to_owned()])
        );
        assert_eq!(
            found[4].condition,
            Condition::Other("always() && needs.lint.result == 'success'".to_owned())
        );
        assert_eq!(found[4].needs, Needs::Opaque);
        assert!(found[5].reusable);
        assert_eq!(found[5].context(), None);
        assert!(found[6].reusable);
        assert_eq!(found[6].context(), None);
    }

    #[test]
    fn flow_lists_keep_quoted_scalars_whole() {
        assert_eq!(list_items("[a, b]"), ["a", "b"]);
        assert_eq!(list_items("a"), ["a"]);
        assert_eq!(list_items("\"a\" # c"), ["a"]);
        assert_eq!(
            list_items("['ma[as]ter', \"x,y\", z]"),
            ["ma[as]ter", "x,y", "z"]
        );
        assert_eq!(list_items("[]"), Vec::<&str>::new());
        assert_eq!(
            list_items("[\"topic\\\",master,tail\", x]"),
            ["topic\\\",master,tail", "x"]
        );
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
    fn a_condition_is_proven_only_as_always_or_a_readable_not_cancelled() {
        assert_eq!(condition("always()"), Condition::Proven);
        assert_eq!(condition("${{ always() }}"), Condition::Proven);
        assert_eq!(condition("'${{always()}}'"), Condition::Proven);
        // The `!` survives YAML only inside the braces or inside quotes.
        assert_eq!(condition("${{ !cancelled() }}"), Condition::Proven);
        assert_eq!(condition("'!cancelled()'"), Condition::Proven);
        assert_eq!(condition("\"!cancelled()\""), Condition::Proven);
        // Unquoted, it is a YAML tag: the workflow does not parse at all.
        assert_eq!(
            condition("!cancelled()"),
            Condition::UnquotedTag("!cancelled()".to_owned())
        );
        assert_eq!(
            condition("${{ always() && false }}"),
            Condition::Other("always() && false".to_owned())
        );
        assert_eq!(
            condition("'!cancelled() && x'"),
            Condition::Other("!cancelled() && x".to_owned())
        );
        assert_eq!(
            condition("!always()"),
            Condition::UnquotedTag("!always()".to_owned())
        );
        assert_eq!(
            condition(""),
            Condition::Other("(a value carried on another line)".to_owned())
        );
    }

    #[test]
    fn read_gate_judges_the_gates_shape() {
        let gated = report(
            "on: [pull_request]\njobs:\n  lint:\n  test:\n    if: always()\n    needs: [lint]\n",
            "test",
        );
        assert_eq!(gated.reading, GateReading::Gated);
        assert_eq!(gated.gate_condition, Some(Condition::Proven));
        assert_eq!(gated.gate_trigger, Trigger::default());
        assert_eq!(gated.reporting, 1);
        assert!(gated.unreadable.is_empty());

        // A gate that needs one job of five is sound: which of the others
        // votes is the project's convention, and no file states it.
        let subset = report(
            "on: [pull_request]\njobs:\n  lint:\n  build:\n  docs:\n  pr-title:\n  test:\n    if: always()\n    needs: lint\n",
            "test",
        );
        assert_eq!(subset.reading, GateReading::Gated);
        assert_eq!(faults(&subset, "test", "master"), None);

        let missing = report("on: [pull_request]\njobs:\n  lint:\n  unit:\n", "test");
        assert_eq!(
            missing.reading,
            GateReading::NoSuchJob {
                contexts: vec!["lint".to_owned(), "unit".to_owned()]
            }
        );
        assert_eq!(missing.gate_condition, None);

        let dynamic = report(
            "on: [pull_request]\njobs:\n  lint:\n  test:\n    name: test-${{ matrix.os }}\n    needs: [lint]\n",
            "test",
        );
        assert_eq!(
            dynamic.reading,
            GateReading::UnprovenGateName {
                job: "test".to_owned()
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

        let filtered = report(
            "on:\n  pull_request:\n    paths: ['src/**']\njobs:\n  test:\n    if: always()\n",
            "test",
        );
        assert_eq!(filtered.reading, GateReading::Gated);
        assert!(filtered.gate_trigger.paths_filtered);

        let off_trunk = report(
            "on:\n  pull_request:\n    branches: [main]\njobs:\n  test:\n    if: always()\n",
            "test",
        );
        assert_eq!(
            off_trunk.gate_trigger.misses_trunk,
            Some("branches: [main]".to_owned())
        );

        let reusable = report(
            "on: [pull_request]\njobs:\n  test:\n    uses: org/repo/.github/workflows/x.yml@main\n    name: test\n",
            "test",
        );
        assert_eq!(
            reusable.reading,
            GateReading::UnprovenGateName {
                job: "test".to_owned()
            }
        );

        let push_only = report("on: push\njobs:\n  lint:\n  test:\n", "test");
        assert_eq!(push_only.reading, GateReading::NoRequestWorkflows);

        // Two jobs reporting one context leave the protection unable to say
        // which one it is holding for.
        let duplicated = report(
            "on: [pull_request]\njobs:\n  test:\n    if: always()\n  other:\n    name: test\n",
            "test",
        );
        assert_eq!(duplicated.reporting, 2);
        let text = faults(&duplicated, "test", "master").expect("a fault");
        assert!(
            text.contains("no longer stands for the gate alone"),
            "{text}"
        );

        let dir = tempfile::tempdir().expect("a tempdir");
        let empty = read_gate(
            Utf8Path::from_path(dir.path()).expect("utf-8"),
            "test",
            "master",
        );
        assert_eq!(empty.reading, GateReading::NoRequestWorkflows);
        assert!(empty.unreadable.is_empty());
    }

    #[test]
    fn an_unreadable_workflow_is_named_not_skipped() {
        let dir = tempfile::tempdir().expect("a tempdir");
        let workflows = dir.path().join(".github/workflows");
        std::fs::create_dir_all(workflows.join("broken.yml")).expect("a directory named as a file");
        std::fs::write(
            workflows.join("ci.yml"),
            "on: [pull_request]\njobs:\n  test:\n    if: always()\n",
        )
        .expect("the workflow writes");
        let report = read_gate(
            Utf8Path::from_path(dir.path()).expect("utf-8"),
            "test",
            "master",
        );
        assert_eq!(report.reading, GateReading::Gated);
        assert_eq!(report.unreadable, vec!["broken.yml".to_owned()]);
        let text = faults(&report, "test", "master").expect("a fault");
        assert!(text.contains("[broken.yml] could not be read"), "{text}");
        // The unreadable file leaves uniqueness unproven; the text must not
        // convert that into a claim of uniqueness.
        assert!(!text.contains("stands for the gate alone"), "{text}");
    }

    #[test]
    fn fault_texts_are_one_line_each() {
        let base = || GateReport {
            reading: GateReading::Gated,
            gate_condition: Some(Condition::Proven),
            gate_trigger: Trigger::default(),
            reporting: 1,
            unreadable: Vec::new(),
        };
        assert_eq!(faults(&base(), "test", "master"), None);
        let cases = [
            GateReport {
                reading: GateReading::NoRequestWorkflows,
                gate_condition: None,
                ..base()
            },
            GateReport {
                reading: GateReading::NoSuchJob {
                    contexts: vec!["lint".to_owned()],
                },
                gate_condition: None,
                ..base()
            },
            GateReport {
                reading: GateReading::UnprovenGateName {
                    job: "test".to_owned(),
                },
                gate_condition: None,
                ..base()
            },
            GateReport {
                reading: GateReading::OpaqueNeeds {
                    workflow: "ci.yml".to_owned(),
                },
                gate_condition: Some(Condition::Absent),
                ..base()
            },
            GateReport {
                gate_condition: Some(Condition::Other("always() && x".to_owned())),
                ..base()
            },
            GateReport {
                gate_condition: Some(Condition::UnquotedTag("!cancelled()".to_owned())),
                ..base()
            },
            GateReport {
                reporting: 2,
                ..base()
            },
            GateReport {
                gate_trigger: Trigger {
                    paths_filtered: true,
                    misses_trunk: Some("branches: [main]".to_owned()),
                    types_filtered: Some("types: [opened]".to_owned()),
                },
                ..base()
            },
            GateReport {
                unreadable: vec!["x.yml".to_owned()],
                ..base()
            },
        ];
        for case in &cases {
            let text = faults(case, "test", "master").expect("a fault");
            assert!(!text.contains('\n'), "{text}");
            assert!(
                text.starts_with(|c: char| c.is_lowercase() || c == '['),
                "{text}"
            );
        }
    }
}
