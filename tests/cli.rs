//! End-to-end tests over the built binary: every subcommand, the landing
//! round-trip, and the payload's presence in the embedded form.

// Integration tests: assertion style is the point, so the production
// restrictions on unwrap/expect/panic do not apply here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use camino::Utf8PathBuf;
use predicates::prelude::*;
use release_kit::skills::Digest;
use release_kit::skills::record::{RECORD_PATH, Record};

/// Every skill the payload carries, and the roots an install writes them to.
const SKILLS: [&str; 3] = ["rk-migrate", "rk-release", "rk-setup"];
const ROOTS: [&str; 2] = [".claude/skills", ".agents/skills"];

/// What every skill shares, installed once outside the agent roots, in the
/// sorted order the payload walks them.
const SHARED: [&str; 2] = ["plan-gate.md", "pre-flight-gate.md"];
const SHARED_ROOT: &str = ".local/state/release-kit/skills/shared";

/// The same root as the skills themselves name it: an absolute path under the
/// home, because no relative path reaches it from both agent roots.
const SHARED_ROOT_HOME: &str = "~/.local/state/release-kit/skills/shared";

fn rk() -> Command {
    // Scrubbed of the variables a running git hook exports: a suite that
    // executes under pre-commit from a linked worktree would otherwise
    // hand every child a GIT_DIR resolving to the real repository.
    let mut command = Command::cargo_bin("rk").expect("the rk binary builds");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
}

/// A scratch home the skill commands run against, so no test touches the
/// operator's own agent directories.
struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("a scratch home exists"),
        }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn rk(&self) -> Command {
        let mut command = rk_scrubbed();
        command.env("HOME", self.path());
        command
    }

    fn destination(&self, root: &str, skill: &str) -> PathBuf {
        self.path().join(root).join(skill).join("SKILL.md")
    }

    fn record(&self) -> PathBuf {
        self.path().join(RECORD_PATH)
    }

    fn shared_gate(&self) -> PathBuf {
        self.path().join(SHARED_ROOT).join(SHARED[0])
    }

    fn load_record(&self) -> Record {
        Record::load(&utf8(&self.record()))
    }

    fn write_record(&self, record: &Record) {
        let parent = self
            .record()
            .parent()
            .expect("the record has a parent")
            .to_path_buf();
        std::fs::create_dir_all(parent).expect("the record directory creates");
        std::fs::write(self.record(), record.to_text()).expect("the record writes");
    }
}

fn utf8(path: &Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path.to_path_buf()).expect("the scratch path is UTF-8")
}

/// Land the rust payload into `target` under the standard test parameters
/// and return the assertion to judge.
fn land_rust(target: &Path) -> assert_cmd::assert::Assert {
    rk().args(["init", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--scopes", "api,cli", "--target"])
        .arg(target)
        .arg("--apply")
        .assert()
}

/// The landing record a target carries, parsed.
fn read_manifest(target: &Path) -> serde_json::Value {
    let bytes = std::fs::read(target.join(".release-kit/manifest.json"))
        .expect("the landing record exists");
    serde_json::from_slice(&bytes).expect("the landing record parses")
}

/// Rewrite a target's landing record, standing in for another release or
/// a doctored history.
fn write_manifest(target: &Path, manifest: &serde_json::Value) {
    std::fs::write(
        target.join(".release-kit/manifest.json"),
        serde_json::to_string_pretty(manifest).expect("the record serializes"),
    )
    .expect("the record writes");
}

/// The record's entry for one destination.
fn manifest_file<'a>(manifest: &'a serde_json::Value, destination: &str) -> &'a serde_json::Value {
    manifest["files"]
        .as_array()
        .expect("the record names its files")
        .iter()
        .find(|file| file["destination"] == destination)
        .unwrap_or_else(|| panic!("{destination} is not in the record"))
}

#[test]
fn method_lists_every_chapter() {
    rk().args(["method", "--list"]).assert().success().stdout(
        predicate::str::contains("model")
            .and(predicate::str::contains("invariants"))
            .and(predicate::str::contains("setup"))
            .and(predicate::str::contains("operate"))
            .and(predicate::str::contains("recovery"))
            .and(predicate::str::contains("diff-surface")),
    );
}

#[test]
fn method_serves_a_chapter_by_short_name() {
    rk().args(["method", "operate"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("# 03 — Operate"));
}

#[test]
fn method_serves_a_chapter_by_file_stem() {
    rk().args(["method", "03-operate"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("# 03 — Operate"));
}

#[test]
fn binding_serves_each_technology() {
    for (tech, heading) in [
        ("rust", "# Rust binding"),
        ("python", "# Python binding"),
        ("bash", "# Bash binding"),
    ] {
        rk().args(["binding", tech])
            .assert()
            .success()
            .stdout(predicate::str::starts_with(heading));
    }
}

#[test]
fn an_unknown_name_exits_66() {
    rk().args(["method", "no-such-chapter"])
        .assert()
        .code(66)
        .stderr(predicate::str::contains("no chapter named"));
}

#[test]
fn a_read_without_name_or_list_exits_64() {
    rk().arg("binding").assert().code(64);
}

/// An argument error goes through the same contract as every other
/// failure: exit 64 from the matrix, never clap's own exit 2 — and under
/// --json, one diagnostic line carrying its reason.
#[test]
fn an_argument_error_exits_64_and_answers_json_with_a_diagnostic() {
    rk().args(["payload", "--bogus"])
        .assert()
        .code(64)
        .stderr(predicate::str::contains("--bogus"));
    let output = rk()
        .args(["payload", "--json", "--bogus"])
        .assert()
        .code(64)
        .get_output()
        .clone();
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&output.stderr).expect("stderr is one JSON diagnostic");
    assert_eq!(diagnostic["schema"], "rk.diagnostic/1");
    assert_eq!(diagnostic["reason"], "usage");
    rk().arg("--help").assert().success();
    rk().arg("--version").assert().success();
}

/// The argument-error handler must survive the very input it reports:
/// an invalid-UTF-8 argument still exits 64 without a panic.
#[cfg(unix)]
#[test]
fn a_non_unicode_argument_error_exits_64_without_panic() {
    use std::os::unix::ffi::OsStringExt as _;
    let bogus = std::ffi::OsString::from_vec(vec![b'-', b'-', 0xff, 0xfe]);
    let out = rk()
        .arg("payload")
        .arg(bogus)
        .assert()
        .code(64)
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
}

/// Spawn `rk` behind a start gate so it provably begins only after the
/// stdout pipe's read end is closed: the wrapper blocks on `cat` until
/// the parent closes stdin, and the parent closes the read end first.
/// Every write `rk` makes then deterministically sees `BrokenPipe`.
/// Returns the exit status and captured stderr.
fn run_with_closed_stdout(args: &[&str]) -> (std::process::ExitStatus, String) {
    let rk_bin = assert_cmd::cargo::cargo_bin("rk");
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(r#"cat >/dev/null; exec "$0" "$@""#)
        .arg(&rk_bin)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the child spawns");
    drop(child.stdout.take());
    drop(child.stdin.take());
    let out = child.wait_with_output().expect("the child finishes");
    (
        out.status,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// A consumer closing its pipe mid-apply must not cut the landing short:
/// the work completes, only the rendering stops.
#[test]
fn a_closed_pipe_does_not_interrupt_an_apply() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    let target_path = target.path().to_string_lossy().into_owned();
    let (status, stderr) = run_with_closed_stdout(&[
        "init",
        "--tech",
        "rust",
        "--forge",
        "github",
        "--repo",
        "acme/widget",
        "--scopes",
        "api,cli",
        "--target",
        &target_path,
        "--apply",
    ]);
    assert!(status.success(), "exit: {status:?}, stderr: {stderr}");
    for landed in [
        "release-plz.toml",
        "dist-workspace.toml",
        ".github/workflows/release-plz.yml",
    ] {
        assert!(
            target.path().join(landed).is_file(),
            "{landed}: the landing stopped short when the pipe closed"
        );
    }
}

/// A consumer that closes its pipe ends the run cleanly — no panic, no
/// error framing — because a closed pipe is the consumer done listening.
#[test]
fn a_closed_pipe_terminates_cleanly() {
    let (status, stderr) = run_with_closed_stdout(&["license"]);
    assert!(status.success(), "exit: {status:?}, stderr: {stderr}");
    assert!(
        !stderr.contains("panicked"),
        "a closed pipe must not panic: {stderr}"
    );
}

/// A stdout that fails outright is a real failure: the run exits through
/// the matrix's I/O code, carries one typed diagnostic, and the
/// application log agrees with the exit.
#[cfg(target_os = "linux")]
#[test]
fn a_failing_stdout_is_one_typed_io_failure() {
    if !Path::new("/dev/full").exists() {
        return;
    }
    let home = Home::new();
    let sink = std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/full")
        .expect("/dev/full opens");
    let out = std::process::Command::new(assert_cmd::cargo::cargo_bin("rk"))
        .args(["payload", "--json"])
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", home.path())
        .stdout(std::process::Stdio::from(sink))
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("the child runs");
    assert_eq!(
        out.status.code(),
        Some(74),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&out.stderr).expect("stderr is one JSON diagnostic");
    assert_eq!(diagnostic["reason"], "io");
    let log = std::fs::read_to_string(home.path().join("release-kit/release-kit.log"))
        .expect("the log exists");
    assert!(
        log.contains("op=payload status=io"),
        "the log must agree with the exit: {log}"
    );
}

#[test]
fn snippet_lists_every_landable_file() {
    rk().args(["snippet", "--list"]).assert().success().stdout(
        predicate::str::contains("rust/github/release-plz.toml")
            .and(predicate::str::contains(
                "rust/github/.github/workflows/release-plz.yml",
            ))
            .and(predicate::str::contains(
                "python/github/.github/workflows/release-please.yml",
            ))
            .and(predicate::str::contains("bash/github/VERSION"))
            .and(predicate::str::contains("bash/github/cliff.toml")),
    );
}

#[test]
fn payload_reports_the_version_and_every_root() {
    let mut expected =
        predicate::str::contains(format!("release-kit {}", env!("CARGO_PKG_VERSION")))
            .and(predicate::str::contains("payload sha256 "))
            .boxed();
    for root in release_kit::payload_roots::PAYLOAD_ROOTS {
        expected = expected
            .and(predicate::str::contains(format!("{root}: ")))
            .boxed();
    }
    rk().arg("payload").assert().success().stdout(expected);
}

#[test]
fn payload_json_parses_and_its_digests_match_the_embedded_bytes() {
    let out = rk()
        .args(["payload", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("the report parses");
    assert_eq!(
        report["release_kit_version"],
        serde_json::Value::from(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(report["payload_schema"], serde_json::Value::from(1));
    let embedded: std::collections::BTreeMap<String, &[u8]> =
        release_kit::embedded::artifacts().into_iter().collect();
    let artifacts = report["artifacts"].as_array().expect("an artifact list");
    assert_eq!(artifacts.len(), embedded.len());
    for artifact in artifacts {
        let path = artifact["path"].as_str().expect("a path");
        let bytes = embedded[path];
        assert_eq!(
            artifact["sha256"].as_str().expect("a digest"),
            Digest::of(bytes).to_string(),
            "{path}: the reported digest differs from the embedded bytes"
        );
    }
}

/// Names a reader of this repository alone could not resolve: the sibling
/// this canon was extracted from, its variable prefix, its bot identity,
/// and the author's account. The forges, registries, and agents the method
/// documents are integrations it describes, and naming those is the point.
const FOREIGN_NAMES: &[&str] = &[
    "spec-driven-docs",
    "spec_driven_docs",
    "sdd_",
    "sdd ",
    "exobrain",
    "gubasso",
];

/// A home directory belonging to a person, assembled rather than written:
/// this scan reads its own source file, and a literal would be a finding.
fn personal_home(user: &str) -> String {
    format!("/home/{user}")
}

fn payload_lines() -> Vec<(String, usize, String)> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("a readable root").flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = Vec::new();
    for name in release_kit::payload_roots::PAYLOAD_ROOTS {
        let path = root.join(name);
        assert!(path.exists(), "{name} is not on disk; the payload moved");
        if path.is_dir() {
            walk(&path, &mut paths);
        } else {
            paths.push(path);
        }
    }
    assert!(!paths.is_empty(), "the payload scan matched no file");
    let mut lines = Vec::new();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .expect("a path under the manifest dir")
            .display()
            .to_string();
        for (index, line) in text.lines().enumerate() {
            lines.push((relative.clone(), index + 1, line.to_lowercase()));
        }
    }
    lines
}

/// SATISFIES distribution:the-payload-names-no-other-project
#[test]
fn the_payload_names_no_other_project() {
    for (file, number, line) in payload_lines() {
        for name in FOREIGN_NAMES {
            assert!(
                !line.contains(name),
                "{file}:{number}: the payload names '{name}', which a reader of this repository alone cannot resolve"
            );
        }
    }
}

/// SATISFIES distribution:the-payload-names-no-other-project
#[test]
fn the_payload_carries_no_path_into_a_home_directory() {
    let marker = personal_home("").to_lowercase();
    for (file, number, line) in payload_lines() {
        for root in [marker.as_str(), "/users/"] {
            let Some(offset) = line.find(root) else {
                continue;
            };
            let rest = &line[offset + root.len()..];
            let end = rest.find(['/', ' ', '`', '"', ')']).unwrap_or(rest.len());
            let user = &rest[..end];
            assert!(
                user.is_empty() || user.starts_with(['<', '$', '{']) || user == "user",
                "{file}:{number}: the payload names the home directory of '{user}'"
            );
        }
    }
}

/// An `exclude` entry in `Cargo.toml` must never remove a payload root
/// from the published crate; the failure would otherwise surface as a
/// crate that does not compile at the consumer.
#[test]
#[ignore = "packages the crate; run through the just build gate"]
fn the_published_crate_carries_every_root() {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["package", "--list", "--allow-dirty"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo package --list runs");
    assert!(
        out.status.success(),
        "cargo package --list failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let listed = String::from_utf8_lossy(&out.stdout);
    for root in release_kit::payload_roots::PAYLOAD_ROOTS {
        assert!(
            listed
                .lines()
                .any(|line| line == root || line.starts_with(&format!("{root}/"))),
            "{root}: the published crate drops this payload root"
        );
    }
}

#[test]
fn versions_prints_the_registry() {
    rk().arg("versions").assert().success().stdout(
        predicate::str::contains("release-plz")
            .and(predicate::str::contains("git-cliff"))
            .and(predicate::str::contains("release-please")),
    );
}

#[test]
fn license_prints_both_halves() {
    rk().arg("license").assert().success().stdout(
        predicate::str::contains("MIT AND CC-BY-4.0")
            .and(predicate::str::contains("LICENSE-MIT"))
            .and(predicate::str::contains("LICENSE-CC-BY-4.0")),
    );
}

#[test]
fn skill_list_names_both_skills() {
    rk().args(["skill", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rk-setup").and(predicate::str::contains("rk-release")));
}

#[test]
fn skill_show_prints_the_frontmatter() {
    rk().args(["skill", "show", "rk-setup"])
        .assert()
        .success()
        .stdout(predicate::str::contains("name: rk-setup"));
}

/// The human preview, snapshot-held line for line: the lines an operator
/// learned keep their shape, and the `Next:` block closes the output.
#[test]
fn init_preview_human_lines_are_snapshot_held() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    let path = target.path().to_string_lossy().into_owned();
    let expected = format!(
        "DRY RUN: rk init writes these files into {path}; re-run with --apply\n\
         .github/workflows/pr-title.yml\n\
         .github/workflows/release-plz.yml\n\
         .pre-commit-config.yaml\n\
         AGENTS.md\n\
         dist-workspace.toml\n\
         release-plz.toml\n\
         Next:\n  rk init --tech rust --forge github --repo <owner/name> --scopes <scope,scope> --workflow worktree --style trunk --target {path} --apply\n"
    );
    rk().args([
        "init", "--tech", "rust", "--forge", "github", "--target", &path,
    ])
    .assert()
    .success()
    .stdout(predicate::eq(expected));
}

#[test]
fn init_json_emits_one_object_and_nothing_else() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    for (mode, extra) in [("preview", None), ("apply", Some("--apply"))] {
        let mut cmd = rk();
        cmd.args(["init", "--tech", "rust", "--forge", "github"])
            .args(["--repo", "acme/widget", "--scopes", "api,cli", "--target"])
            .arg(target.path());
        if let Some(flag) = extra {
            cmd.arg(flag);
        }
        let out = cmd
            .arg("--json")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
        assert_eq!(report["schema"], "rk.init/4");
        assert_eq!(report["mode"], mode);
        assert!(
            report["files"].as_array().is_some_and(|f| !f.is_empty()),
            "{mode}: the report names the files"
        );
        assert!(
            report["next"].as_array().is_some_and(|n| !n.is_empty()),
            "{mode}: the report carries a next block"
        );
    }
}

/// Under --json a failure is one diagnostic line on stderr, carrying its
/// reason from the closed vocabulary, and stdout stays clean.
#[test]
fn init_json_failure_is_a_diagnostic_on_stderr() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    let workflow = target.path().join(".github/workflows/release-plz.yml");
    std::fs::create_dir_all(workflow.parent().unwrap()).expect("the workflow dir creates");
    std::fs::write(&workflow, "something local\n").expect("the conflict file writes");
    let output = rk()
        .args(["init", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--scopes", "api,cli", "--target"])
        .arg(target.path())
        .args(["--apply", "--json"])
        .assert()
        .code(73)
        .get_output()
        .clone();
    assert!(output.stdout.is_empty(), "no result on a refused landing");
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&output.stderr).expect("stderr is one JSON diagnostic");
    assert_eq!(diagnostic["schema"], "rk.diagnostic/1");
    assert_eq!(diagnostic["reason"], "state-drift");
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|m| m.contains("release-plz.yml")),
        "{diagnostic}"
    );
}

#[test]
fn init_dry_runs_by_default_and_writes_nothing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "rust", "--forge", "github", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"));
    assert!(
        !target.path().join("release-plz.toml").exists(),
        "a dry run must write nothing"
    );
}

#[test]
fn init_apply_lands_reports_sentinels_and_a_relanding_names_upgrade() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success().stdout(
        predicate::str::contains("wrote release-plz.toml")
            .and(predicate::str::contains("TODO(release-kit)")),
    );
    assert!(
        target
            .path()
            .join(".github/workflows/release-plz.yml")
            .is_file()
    );
    // A re-landing over an existing record is `rk upgrade`'s job.
    land_rust(target.path())
        .code(73)
        .stderr(predicate::str::contains("rk upgrade"));
}

/// A skill has one owner, so a landing projects none into the target: a
/// second copy under one name is a second entry offering the same skill.
#[test]
fn init_lands_no_skill_into_the_target() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    for root in ROOTS {
        assert!(
            !target.path().join(root).exists(),
            "{root} must not be landed into a target"
        );
    }
}

#[test]
fn init_refuses_a_conflicting_target_and_writes_neither_files_nor_record() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    let workflow = target.path().join(".github/workflows/release-plz.yml");
    std::fs::create_dir_all(workflow.parent().unwrap()).expect("the workflow dir creates");
    std::fs::write(&workflow, "something local\n").expect("the conflict file writes");
    land_rust(target.path())
        .code(73)
        .stderr(predicate::str::contains("release-plz.yml"));
    assert!(
        !target.path().join("dist-workspace.toml").exists(),
        "a refused landing must write nothing"
    );
    assert!(
        !target.path().join(".release-kit").exists(),
        "a refused landing must write no record"
    );
}

/// A differing seeded file is the target's own, not a conflict: the
/// landing keeps it, records the target's digest beside the payload's
/// baseline, and completes.
#[test]
fn init_keeps_a_differing_seeded_file_and_lands_the_rest() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(target.path().join("release-plz.toml"), "tuned = true\n")
        .expect("the seeded file writes");
    land_rust(target.path())
        .success()
        .stdout(predicate::str::contains(
            "kept (target-owned) release-plz.toml",
        ));
    assert_eq!(
        std::fs::read_to_string(target.path().join("release-plz.toml"))
            .expect("the seeded file survives"),
        "tuned = true\n"
    );
    let manifest = read_manifest(target.path());
    let seeded = manifest_file(&manifest, "release-plz.toml");
    assert_eq!(
        seeded["sha256"].as_str().expect("a digest"),
        Digest::of(b"tuned = true\n").to_string(),
        "the record must carry the target's bytes, not the payload's"
    );
    assert_ne!(seeded["sha256"], seeded["baseline_sha256"]);
}

#[test]
fn init_rejects_an_unknown_tech_with_usage() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "fortran", "--forge", "github", "--target"])
        .arg(target.path())
        .assert()
        .code(64)
        .stderr(predicate::str::contains("unknown tech"));
}

#[test]
fn init_refuses_a_missing_target() {
    rk().args([
        "init",
        "--tech",
        "rust",
        "--forge",
        "github",
        "--target",
        "/no/such/dir",
    ])
    .assert()
    .code(73);
}

#[test]
fn skill_install_previews_every_destination_and_writes_nothing() {
    let home = Home::new();
    let mut expected = predicate::str::contains("DRY RUN").boxed();
    for root in ROOTS {
        for name in ["rk-setup", "rk-release"] {
            let destination = home.destination(root, name);
            expected = expected
                .and(predicate::str::contains(
                    destination.to_string_lossy().into_owned(),
                ))
                .boxed();
        }
    }
    home.rk()
        .args(["skill", "install"])
        .assert()
        .success()
        .stdout(expected);
    assert!(!home.path().join(".claude").exists());
    assert!(!home.record().exists());
}

#[test]
fn skill_install_apply_and_uninstall_round_trip() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    for root in ROOTS {
        for name in SKILLS {
            assert!(home.destination(root, name).is_file());
        }
    }
    assert!(
        home.shared_gate().is_file(),
        "an apply lands the shared plan gate the skills name"
    );
    assert_eq!(
        home.load_record().written.len(),
        ROOTS.len() * SKILLS.len() + SHARED.len(),
        "an apply records every destination it wrote"
    );

    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unchanged "));

    home.rk()
        .args(["skill", "uninstall", "--apply"])
        .assert()
        .success();
    for root in ROOTS {
        for name in SKILLS {
            assert!(!home.destination(root, name).exists());
        }
    }
    assert!(!home.record().exists(), "an emptied record is removed");
    // A re-run over an emptied home is a no-op, not a failure.
    home.rk()
        .args(["skill", "uninstall", "--apply"])
        .assert()
        .success();
}

#[test]
fn skill_install_refuses_a_differing_destination_without_force() {
    let home = Home::new();
    let destination = home.destination(".claude/skills", "rk-release");
    std::fs::create_dir_all(destination.parent().unwrap()).expect("the skill dir creates");
    // Non-UTF-8 bytes: the existing file must still count as a conflict.
    std::fs::write(&destination, [0xff, 0xfe, 0x00]).expect("the conflict file writes");
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("--force"));
    assert_eq!(
        std::fs::read(&destination).expect("the conflict file survives"),
        [0xff, 0xfe, 0x00],
        "a refused install must not overwrite"
    );
    assert!(
        !home.path().join(".agents").exists(),
        "a refused install must not write the other root"
    );
    home.rk()
        .args(["skill", "install", "--apply", "--force"])
        .assert()
        .success();
    assert!(
        std::fs::read_to_string(&destination)
            .expect("the forced install wrote the payload")
            .contains("name: rk-release")
    );
}

/// The defect the record exists for: bytes an older release wrote are not the
/// user's, so a newer release must replace them without asking for --force.
#[test]
fn skill_install_replaces_a_copy_a_previous_release_wrote() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();

    // Stand in for an older release: rewrite every destination and record its
    // digest, exactly as that release's apply would have left the home.
    let mut older = Record::default();
    for destination in home.load_record().written.into_keys() {
        std::fs::write(&destination, "older canon bytes\n").expect("the older copy writes");
        older
            .written
            .insert(destination, Digest::of(b"older canon bytes\n"));
    }
    home.write_record(&older);

    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("wrote "));
    assert!(
        std::fs::read_to_string(home.destination(".claude/skills", "rk-setup"))
            .expect("the upgraded skill reads")
            .contains("name: rk-setup")
    );
}

/// A skill a later release renames or drops leaves a file an agent keeps
/// offering, so the install that supersedes it takes it back.
#[test]
fn skill_install_sweeps_a_destination_the_payload_dropped() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();

    let dropped = home.destination(".claude/skills", "rk-retired");
    std::fs::create_dir_all(dropped.parent().unwrap()).expect("the retired dir creates");
    std::fs::write(&dropped, "a skill a later release dropped\n").expect("the leftover writes");
    let mut record = home.load_record();
    record.written.insert(
        utf8(&dropped),
        Digest::of(b"a skill a later release dropped\n"),
    );
    home.write_record(&record);

    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("swept "));
    assert!(!dropped.exists(), "a swept leftover is gone");
    assert!(
        !dropped.parent().unwrap().exists(),
        "the directory a sweep empties goes with it"
    );
    assert!(!home.load_record().written.contains_key(&utf8(&dropped)));
}

/// A leftover the user has since edited is theirs, and no sweep takes it.
#[test]
fn skill_install_leaves_an_edited_leftover_alone() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();

    let dropped = home.destination(".claude/skills", "rk-retired");
    std::fs::create_dir_all(dropped.parent().unwrap()).expect("the retired dir creates");
    std::fs::write(&dropped, "the user rewrote this\n").expect("the leftover writes");
    let mut record = home.load_record();
    record
        .written
        .insert(utf8(&dropped), Digest::of(b"what we wrote\n"));
    home.write_record(&record);

    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(&dropped).expect("the leftover survives"),
        "the user rewrote this\n"
    );
}

#[cfg(unix)]
#[test]
fn skill_install_refuses_a_symlinked_destination() {
    let home = Home::new();
    let destination = home.destination(".claude/skills", "rk-release");
    std::fs::create_dir_all(destination.parent().unwrap()).expect("the skill dir creates");
    let elsewhere = home.path().join("elsewhere.md");
    std::fs::write(&elsewhere, "the user's file\n").expect("the target file writes");
    std::os::unix::fs::symlink(&elsewhere, &destination).expect("the symlink creates");

    home.rk()
        .args(["skill", "install", "--apply", "--force"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("symlink"));
    assert_eq!(
        std::fs::read_to_string(&elsewhere).expect("the symlink target survives"),
        "the user's file\n",
        "an install must never write through a symlink"
    );
    assert!(!home.path().join(".agents").exists());
}

#[cfg(unix)]
#[test]
fn skill_install_refuses_a_symlinked_shared_root() {
    let home = Home::new();
    let elsewhere = home.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("the outside directory creates");
    let state_dir = home
        .record()
        .parent()
        .expect("the record has a parent")
        .to_path_buf();
    std::fs::create_dir_all(&state_dir).expect("the state directory creates");
    std::os::unix::fs::symlink(&elsewhere, state_dir.join("skills")).expect("the symlink creates");

    home.rk()
        .args(["skill", "install", "--apply", "--force"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("symlink"));
    assert_eq!(
        std::fs::read_dir(&elsewhere)
            .expect("the symlink target survives")
            .count(),
        0,
        "an install must never write through a symlinked shared root"
    );
    assert!(!home.path().join(".claude").exists());
}

#[test]
fn skill_agent_flag_touches_one_root_only() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--agent", "claude", "--apply"])
        .assert()
        .success();
    assert!(home.destination(".claude/skills", "rk-setup").is_file());
    assert!(!home.path().join(".agents").exists());

    home.rk()
        .args(["skill", "install", "--agent", "codex", "--apply"])
        .assert()
        .success();
    assert!(home.destination(".agents/skills", "rk-setup").is_file());

    home.rk()
        .args(["skill", "uninstall", "--agent", "codex", "--apply"])
        .assert()
        .success();
    assert!(!home.destination(".agents/skills", "rk-setup").exists());
    assert!(
        home.destination(".claude/skills", "rk-setup").is_file(),
        "one agent's uninstall must not touch the other's root"
    );
}

/// An installed `SKILL.md` the user has since edited is theirs: uninstall
/// names it and leaves it, matching the install-side conflict refusal.
#[test]
fn skill_uninstall_keeps_an_edited_skill() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    let edited = home.destination(".claude/skills", "rk-setup");
    std::fs::write(&edited, "the user rewrote this\n").expect("the edit writes");

    home.rk()
        .args(["skill", "uninstall", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "kept (edited by you) {}",
            edited.display()
        )));
    assert_eq!(
        std::fs::read_to_string(&edited).expect("the edit survives"),
        "the user rewrote this\n"
    );
}

#[test]
fn skill_uninstall_keeps_what_a_user_put_beside_a_skill() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    let beside = home
        .destination(".claude/skills", "rk-setup")
        .parent()
        .unwrap()
        .join("notes.md");
    std::fs::write(&beside, "the user's notes\n").expect("the note writes");

    home.rk()
        .args(["skill", "uninstall", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("kept (not empty)"));
    assert!(beside.is_file(), "a file beside a skill must survive");
}

/// The human preview, snapshot-held line for line, `Next:` block included.
#[test]
fn skill_install_preview_human_lines_are_snapshot_held() {
    let home = Home::new();
    let mut expected =
        String::from("DRY RUN: rk skill install writes these files; re-run with --apply\n");
    for root in ROOTS {
        for name in SKILLS {
            expected.push_str(&home.destination(root, name).to_string_lossy());
            expected.push('\n');
        }
    }
    // The shared artifacts follow the skills, in the payload's sorted order.
    for artifact in SHARED {
        expected.push_str(
            &home
                .path()
                .join(SHARED_ROOT)
                .join(artifact)
                .to_string_lossy(),
        );
        expected.push('\n');
    }
    expected.push_str("Next:\n  rk skill install --apply\n");
    home.rk()
        .args(["skill", "install"])
        .assert()
        .success()
        .stdout(predicate::eq(expected));
}

#[test]
fn skill_install_json_reports_typed_actions() {
    let home = Home::new();
    let out = home
        .rk()
        .args(["skill", "install", "--apply", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.skill/1");
    assert_eq!(report["command"], "install");
    assert_eq!(report["mode"], "apply");
    let actions = report["actions"].as_array().expect("an action list");
    assert_eq!(actions.len(), ROOTS.len() * SKILLS.len() + SHARED.len());
    assert!(
        actions.iter().all(|action| action["action"] == "write"),
        "{actions:?}"
    );
}

/// A doctor run answers on any host: probes fail as results, the exit code
/// stays 0, and the overridable forge binaries keep the test hermetic.
#[test]
fn doctor_reports_every_probe_and_exits_0() {
    let home = Home::new();
    let gh = home.path().join("fake-gh");
    let glab = home.path().join("fake-glab");
    std::fs::write(&gh, "#!/bin/sh\nexit 0\n").expect("the mock writes");
    std::fs::write(&glab, "#!/bin/sh\nexit 1\n").expect("the mock writes");
    #[cfg(unix)]
    for mock in [&gh, &glab] {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(mock, std::fs::Permissions::from_mode(0o755))
            .expect("the mock is executable");
    }
    let out = home
        .rk()
        .args(["doctor", "--json"])
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .env("RK_GLAB_BIN", &glab)
        .current_dir(home.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.doctor/1");
    let probes = report["probes"].as_array().expect("a probe list");
    let ids: Vec<&str> = probes
        .iter()
        .map(|probe| probe["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(
        ids,
        [
            "sh",
            "git",
            "state-root",
            "skill-roots",
            "skill-gate",
            "skill-payload",
            "git-remote",
            "gh-auth",
            "glab-auth",
            "openssl",
            "curl",
            "nix",
            "direnv",
            "cosign",
            "pypi-attestations"
        ]
    );
    let by_id = |id: &str| {
        probes
            .iter()
            .find(|probe| probe["id"] == id)
            .expect("the probe reports")
            .clone()
    };
    assert_eq!(by_id("gh-auth")["status"], "ok");
    assert_eq!(by_id("glab-auth")["status"], "failed");
    assert_eq!(by_id("glab-auth")["remediation"], "run glab auth login");
    assert_eq!(by_id("git")["class"], "hard");
    assert_eq!(by_id("sh")["class"], "hard");
}

/// One doctor report with the forge binaries mocked away and the given
/// extra environment, parsed.
fn doctor_with(home: &Home, vars: &[(&str, &str)]) -> serde_json::Value {
    let mut command = home.rk();
    command
        .args(["doctor", "--json"])
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", "/no/such/gh")
        .env("RK_GLAB_BIN", "/no/such/glab")
        .current_dir(home.path());
    for (key, value) in vars {
        command.env(key, value);
    }
    let out = command.assert().success().get_output().stdout.clone();
    serde_json::from_slice(&out).expect("one JSON object")
}

/// The Hard git probe resolves through `RK_GIT_BIN`: a broken override
/// fails visibly even while a working git sits on PATH, which is what
/// proves the override — not the PATH — owns the answer.
#[test]
fn the_git_probe_honors_its_override() {
    let home = Home::new();
    let report = doctor_with(&home, &[("RK_GIT_BIN", "/no/such/git")]);
    let git = probe(&report, "git");
    assert_eq!(git["class"], "hard");
    assert_eq!(git["status"], "failed");
    assert_eq!(git["remediation"], "install git");
    let report = doctor_with(&home, &[]);
    assert_eq!(probe(&report, "git")["status"], "ok");
}

/// The Hard sh probe resolves through `RK_SH_BIN` the same way.
#[test]
fn the_sh_probe_honors_its_override() {
    let home = Home::new();
    let report = doctor_with(&home, &[("RK_SH_BIN", "/no/such/sh")]);
    let sh = probe(&report, "sh");
    assert_eq!(sh["class"], "hard");
    assert_eq!(sh["status"], "failed");
}

/// A git-launching verb resolves through the shared resolver too: the verb
/// works in a real repository, and breaking only the override breaks it,
/// so the PATH's git demonstrably does not answer for it.
#[test]
fn a_git_verb_resolves_through_the_override() {
    let home = Home::new();
    let mut init = std::process::Command::new(release_kit::probes::git_bin());
    for var in GIT_HOOK_VARS {
        init.env_remove(var);
    }
    init.args(["init", "-q"])
        .current_dir(home.path())
        .status()
        .expect("git init runs");
    home.rk()
        .args(["worktree", "list"])
        .current_dir(home.path())
        .assert()
        .success();
    home.rk()
        .args(["worktree", "list"])
        .env("RK_GIT_BIN", "/no/such/git")
        .current_dir(home.path())
        .assert()
        .failure();
}

/// The wrapper's package list in `nix/package.nix` agrees with the Hard
/// tool registry the probes own. Two hand-kept lists in two languages —
/// Nix cannot read the Rust registry — so this test is what makes the
/// mirrored contract safe: a divergence fails here, by name.
#[test]
fn the_package_wrapper_mirrors_the_hard_tool_registry() {
    let nix =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("nix/package.nix"))
            .expect("nix/package.nix reads");
    let marker = "makeBinPath [";
    let start = nix.find(marker).expect("the wrapper names a package list");
    let rest = &nix[start + marker.len()..];
    let end = rest.find(']').expect("the package list closes");
    let wrapped: std::collections::BTreeSet<&str> = rest[..end].split_whitespace().collect();
    let registry: std::collections::BTreeSet<&str> = release_kit::probes::HARD_RUNTIME_TOOLS
        .iter()
        .map(|(_, package)| *package)
        .collect();
    assert_eq!(
        wrapped, registry,
        "nix/package.nix wraps {wrapped:?}; src/probes.rs requires {registry:?}"
    );
    let executables: Vec<&str> = release_kit::probes::HARD_RUNTIME_TOOLS
        .iter()
        .map(|(executable, _)| *executable)
        .collect();
    assert_eq!(executables, ["git", "sh"]);
}

/// No production code launches git or sh by literal name: every launcher
/// goes through the shared resolvers, so a new call site cannot bypass
/// `RK_GIT_BIN` or `RK_SH_BIN` silently. Offenders are named by file and
/// line.
#[test]
fn every_git_and_sh_launch_resolves_through_the_shared_resolver() {
    fn scan(dir: &Path, offenders: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("src reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                scan(&path, offenders);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a source file reads");
            for (index, line) in text.lines().enumerate() {
                // Both launcher forms: a direct spawn, and an Exec built
                // with a literal program that src/setup/process.rs later
                // launches.
                if line.contains(r#"Command::new("git")"#)
                    || line.contains(r#"Command::new("sh")"#)
                    || line.contains(r#"program: "git""#)
                    || line.contains(r#"program: "sh""#)
                {
                    offenders.push(format!("{}:{}", path.display(), index + 1));
                }
            }
        }
    }
    let mut offenders = Vec::new();
    scan(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut offenders,
    );
    assert!(
        offenders.is_empty(),
        "direct launches bypassing the shared resolver: {offenders:?}"
    );
}

/// A doctor run against a scratch home, with the forge binaries mocked away
/// so only the skill probes decide the answers.
fn skill_probes(home: &Home) -> serde_json::Value {
    let out = home
        .rk()
        .args(["doctor", "--json"])
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", "/no/such/gh")
        .env("RK_GLAB_BIN", "/no/such/glab")
        .current_dir(home.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).expect("one JSON object")
}

fn probe(report: &serde_json::Value, id: &str) -> serde_json::Value {
    report["probes"]
        .as_array()
        .expect("a probe list")
        .iter()
        .find(|probe| probe["id"] == id)
        .unwrap_or_else(|| panic!("no {id} probe"))
        .clone()
}

/// The agent roots and the shared root are separate directories, so a home
/// can carry the skills and not the gate they are told to read first — which
/// is what a container sharing one and not the other produces. The probe
/// names that home before an agent acts in it, and clears once the gate is
/// where the skills look for it.
#[test]
fn the_gate_probe_names_a_home_whose_skills_cannot_read_their_gate() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    assert_eq!(probe(&skill_probes(&home), "skill-gate")["status"], "ok");

    // The one difference between a working home and the failing one: the
    // shared root is gone while every agent root stays.
    std::fs::remove_dir_all(home.path().join(SHARED_ROOT)).expect("the shared root removes");
    let report = skill_probes(&home);
    let gate = probe(&report, "skill-gate");
    assert_eq!(gate["status"], "failed");
    assert!(
        gate["message"]
            .as_str()
            .expect("a message")
            .contains("plan-gate.md"),
        "{gate:?}"
    );
    assert_eq!(gate["remediation"], "rk skill install --apply");
    assert_eq!(
        probe(&report, "skill-payload")["status"],
        "ok",
        "the skills are installed; only their gate is missing"
    );
}

/// One binary serves every repository, so an installed skill and the `rk` on
/// PATH can be updated apart. The probe reports bytes that are not this
/// binary's, and asks for the `--force` only where the record cannot vouch
/// for what it would overwrite.
#[test]
fn the_payload_probe_names_a_skill_that_is_not_this_binarys() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    assert_eq!(probe(&skill_probes(&home), "skill-payload")["status"], "ok");

    // Bytes the record cannot account for are the operator's own.
    let edited = home.destination(ROOTS[0], SKILLS[0]);
    std::fs::write(&edited, "mine now\n").expect("the skill rewrites");
    let payload = probe(&skill_probes(&home), "skill-payload");
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["remediation"], "rk skill install --apply --force");

    // Bytes the record vouches for are an older release's, and the plain
    // apply corrects them.
    let mut record = home.load_record();
    record
        .written
        .insert(utf8(&edited), Digest::of(b"mine now\n"));
    home.write_record(&record);
    let payload = probe(&skill_probes(&home), "skill-payload");
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["remediation"], "rk skill install --apply");

    // A skill destination that is simply absent is neither.
    std::fs::remove_file(&edited).expect("the skill removes");
    let payload = probe(&skill_probes(&home), "skill-payload");
    assert_eq!(payload["status"], "failed");
    assert_eq!(payload["remediation"], "rk skill install --apply");
}

/// A home nobody has installed into fails both skill probes and neither
/// panics: the doctor answers on any host.
#[test]
fn the_skill_probes_answer_on_a_home_with_no_install() {
    let home = Home::new();
    let report = skill_probes(&home);
    for id in ["skill-gate", "skill-payload"] {
        let found = probe(&report, id);
        assert_eq!(found["status"], "failed", "{id}");
        assert_eq!(found["remediation"], "rk skill install --apply", "{id}");
    }
    assert_eq!(
        probe(&report, "skill-roots")["status"],
        "ok",
        "an absent root under a writable home is not a refusal"
    );
}

/// The gh probe asks `auth status --active` first: the bare form fails
/// while any stored account is broken, even though the active one — the
/// only credential this tool would use — works. A mock that passes only
/// with `--active` is that host; the bare-form-only probe this replaced
/// would report it unauthenticated.
#[test]
fn the_gh_probe_judges_only_the_active_account() {
    let home = Home::new();
    let gh = home.path().join("fake-gh");
    std::fs::write(
        &gh,
        "#!/bin/sh\nfor arg in \"$@\"; do [ \"$arg\" = --active ] && exit 0; done\nexit 1\n",
    )
    .expect("the mock writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755))
            .expect("the mock is executable");
    }
    let out = home
        .rk()
        .args(["doctor", "--json"])
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .env("RK_GLAB_BIN", "/no/such/glab")
        .current_dir(home.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let gh_auth = report["probes"]
        .as_array()
        .expect("a probe list")
        .iter()
        .find(|probe| probe["id"] == "gh-auth")
        .expect("the probe reports")
        .clone();
    assert_eq!(gh_auth["status"], "ok");
}

/// The gh probe prefers `auth status --active` — the bare form fails
/// while any stored account is broken, even though the active one works
/// — and falls back to the bare form where the CLI predates the flag,
/// so an older gh that is authenticated still reports authenticated.
#[test]
fn the_gh_probe_falls_back_where_active_is_unknown() {
    let home = Home::new();
    let gh = home.path().join("fake-gh");
    std::fs::write(
        &gh,
        "#!/bin/sh\nfor arg in \"$@\"; do [ \"$arg\" = --active ] && exit 1; done\nexit 0\n",
    )
    .expect("the mock writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755))
            .expect("the mock is executable");
    }
    let out = home
        .rk()
        .args(["doctor", "--json"])
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .env("RK_GLAB_BIN", "/no/such/glab")
        .current_dir(home.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let gh_auth = report["probes"]
        .as_array()
        .expect("a probe list")
        .iter()
        .find(|probe| probe["id"] == "gh-auth")
        .expect("the probe reports")
        .clone();
    assert_eq!(gh_auth["status"], "ok");
}

/// A credential in a malformed remote must never reach a probe message:
/// probe results land in captured output and CI logs.
#[test]
fn doctor_never_echoes_a_credential_bearing_remote() {
    let home = Home::new();
    for args in [
        &["init", "-q"][..],
        &["remote", "add", "origin", "https://user:sekret-token@/repo"][..],
    ] {
        git_in(home.path(), args);
    }
    let out = home
        .rk()
        .args(["doctor", "--json"])
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", "/no/such/gh")
        .env("RK_GLAB_BIN", "/no/such/glab")
        .current_dir(home.path())
        .assert()
        .success()
        .get_output()
        .clone();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        !text.contains("sekret-token") && !text.contains("user:"),
        "a probe result leaked the remote's credential: {text}"
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    let remote = report["probes"]
        .as_array()
        .expect("a probe list")
        .iter()
        .find(|probe| probe["id"] == "git-remote")
        .expect("the remote probe reports")
        .clone();
    assert_eq!(remote["status"], "failed");
}

#[test]
fn usage_examples_carry_the_either_or_requirement() {
    rk().arg("usage").assert().success().stdout(
        predicate::str::contains("example: rk method <NAME|--list>")
            .and(predicate::str::contains(
                "example: rk binding <NAME|--list>",
            ))
            .and(predicate::str::contains(
                "example: rk snippet <NAME|--list>",
            )),
    );
}

#[test]
fn usage_dumps_every_verb_in_one_call() {
    let expected = [
        "rk method",
        "rk binding",
        "rk snippet",
        "rk guide",
        "rk forge",
        "rk versions",
        "rk payload",
        "rk init",
        "rk setup check",
        "rk setup step",
        "rk setup script",
        "rk branches prune",
        "rk message",
        "rk worktree list",
        "rk worktree add",
        "rk worktree prune",
        "rk runs list",
        "rk runs show",
        "rk runs prune",
        "rk skill install",
        "rk skill uninstall",
        "rk devshell status",
        "rk devshell add",
        "rk devshell clean",
        "rk devshell sync",
        "rk status",
        "rk upgrade",
        "rk adopt",
        "rk versions",
        "rk doctor",
        "rk usage",
        "rk license",
        "rk completions",
    ]
    .iter()
    .fold(
        predicate::str::contains("example: rk init").boxed(),
        |acc, verb| acc.and(predicate::str::contains(*verb)).boxed(),
    );
    rk().arg("usage").assert().success().stdout(expected);
}

/// One invocation leaves one record in the application log at the XDG
/// state root, and `RUST_LOG=off` silences it.
#[test]
fn a_command_leaves_one_application_log_record() {
    let home = Home::new();
    home.rk()
        .args(["method", "--list"])
        .env("XDG_STATE_HOME", home.path())
        .assert()
        .success();
    let log = home.path().join("release-kit/release-kit.log");
    let text = std::fs::read_to_string(&log).expect("the log exists");
    assert!(
        text.contains("op=method status=ok"),
        "the record names the op: {text}"
    );
    home.rk()
        .args(["versions"])
        .env("XDG_STATE_HOME", home.path())
        .env("RUST_LOG", "off")
        .assert()
        .success();
    let after = std::fs::read_to_string(&log).expect("the log still reads");
    assert_eq!(text, after, "RUST_LOG=off must silence the log");
}

#[test]
fn skill_uninstall_previews_by_default_and_removes_nothing() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    home.rk()
        .args(["skill", "uninstall"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"));
    assert!(home.destination(".claude/skills", "rk-setup").is_file());
}

#[test]
fn skill_force_without_apply_is_rejected() {
    let home = Home::new();
    home.rk()
        .args(["skill", "install", "--force"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--apply"));
}

#[test]
fn skill_show_is_byte_identical_to_the_authored_file() {
    for name in SKILLS {
        let authored = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("skills")
                .join(name)
                .join("SKILL.md"),
        )
        .expect("the authored skill reads");
        let printed = rk()
            .args(["skill", "show", name])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        assert_eq!(printed, authored, "{name}: the payload differs from source");
    }
}

#[test]
fn skill_show_of_an_unknown_name_exits_66() {
    rk().args(["skill", "show", "nope"])
        .assert()
        .code(66)
        .stderr(predicate::str::contains("no skill named"));
}

/// One authored file serves every agent, so the frontmatter stays inside the
/// portable Agent Skills fields no vendor extends.
#[test]
fn every_skill_carries_the_portable_frontmatter() {
    const PORTABLE: [&str; 6] = [
        "name",
        "description",
        "license",
        "compatibility",
        "metadata",
        "allowed-tools",
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
    let mut seen = 0;
    for entry in std::fs::read_dir(&root).expect("the skills directory reads") {
        let dir = entry.expect("the skill entry reads").path();
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(dir.join("SKILL.md")).expect("the skill reads");
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("---"), "{name}: no opening frontmatter");

        let mut fields = std::collections::BTreeMap::new();
        let mut closed = false;
        for line in lines.by_ref() {
            if line == "---" {
                closed = true;
                break;
            }
            if line.starts_with([' ', '\t']) {
                continue;
            }
            let (key, value) = line
                .split_once(':')
                .unwrap_or_else(|| panic!("{name}: not a key line: {line}"));
            assert!(
                PORTABLE.contains(&key),
                "{name}: field '{key}' is not in the portable Agent Skills format"
            );
            // A YAML plain scalar never opens with '|' or '>', so this rejects
            // every block-scalar header form and nothing valid.
            assert!(
                !value.trim().starts_with(['|', '>']),
                "{name}: '{key}' uses a block scalar; keep portable values on one plain line"
            );
            assert!(
                fields.insert(key, value.trim().to_owned()).is_none(),
                "{name}: '{key}' appears twice"
            );
        }
        assert!(closed, "{name}: the frontmatter never closes");
        assert_eq!(
            fields.get("name").map(String::as_str),
            Some(name.as_str()),
            "{name}: the frontmatter name differs from the directory"
        );
        assert!(
            fields.contains_key("description"),
            "{name}: no description for an agent to route on"
        );
        let body = lines.count();
        assert!(body <= 150, "{name}: the body runs {body} lines, over 150");
        seen += 1;
    }
    assert_eq!(seen, SKILLS.len(), "the authored skills changed");
}

/// Every skill drives operations that write files, mutate a forge, or publish
/// a version, so each one routes to both shared gates before it acts, and
/// states the one flag that changes the plan gate's shape. The test holds the
/// instruction's presence; no test can hold a model to it.
#[test]
fn every_skill_routes_to_the_plan_gate_before_acting() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
    for name in SKILLS {
        let text =
            std::fs::read_to_string(root.join(name).join("SKILL.md")).expect("the skill reads");
        let gate = text
            .find("## Before acting")
            .unwrap_or_else(|| panic!("{name}: no '## Before acting' section"));
        // Both shared artifacts are named, by the absolute path they install
        // to: the two agent roots make no relative path reach one file from
        // both, so the skills name them the one way that resolves.
        for artifact in SHARED {
            let named = format!("{SHARED_ROOT_HOME}/{artifact}");
            assert!(
                text[gate..].contains(&named),
                "{name}: the gate section does not name {named}"
            );
        }
        assert!(
            text[gate..].contains("--no-plan"),
            "{name}: the gate section does not state the --no-plan rule"
        );
        assert!(
            text[gate..].contains("No flag skips it"),
            "{name}: the gate section does not state that the pre-flight is unconditional"
        );
        // Every other section is an acting section, so the gate leads.
        let first = text
            .find("\n## ")
            .expect("a skill carries at least one section");
        assert_eq!(
            first + 1,
            gate,
            "{name}: a section precedes the plan gate, so acting can start before it is read"
        );
    }
}

/// The shared artifacts are files, carried by the payload and installed once,
/// so correcting one corrects every skill under every agent root. Each has its
/// own duty and the payload carries both.
#[test]
fn the_payload_carries_the_shared_plan_gate() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("skill-shared");
    let read = |name: &str| {
        std::fs::read_to_string(root.join(name)).unwrap_or_else(|_| panic!("{name} reads"))
    };

    let plan = read("plan-gate.md");
    for phase in ["## 1. Plan", "## 2. Validate", "## 3. Execute"] {
        assert!(
            plan.contains(phase),
            "the plan gate carries no {phase} phase"
        );
    }
    assert!(
        plan.contains("--no-plan"),
        "the plan gate does not state what --no-plan changes"
    );
    assert!(
        plan.contains("## What a request authorizes"),
        "the plan gate does not bound what a request authorizes"
    );

    let pre_flight = read("pre-flight-gate.md");
    assert!(
        pre_flight.contains("rk doctor"),
        "the pre-flight gate does not run the probe catalog"
    );
    // Read from the catalog's own declaration, not listed here: a skill probe
    // added without a line in the pre-flight gate is a probe no agent
    // following it ever reads.
    for id in release_kit::probes::SKILL_PROBES {
        assert!(
            pre_flight.contains(id),
            "the pre-flight gate does not read the {id} probe"
        );
    }
    // The pre-flight runs whatever the request carries; only the plan gate
    // has a flag. A pre-flight that could be waived is one no skill can rely
    // on having run.
    assert!(
        pre_flight.contains("No flag skips it"),
        "the pre-flight gate does not state that it is unconditional"
    );
    assert!(
        pre_flight.contains("plan-gate.md"),
        "the pre-flight gate does not hand the task to the plan gate"
    );
}

/// The landed block says who acts, per
/// `landing:the-routing-block-bounds-the-agents-initiative`: it names the
/// git and forge actions an agent takes only on the operator's order, and
/// carries no sentence ordering one to branch, commit, or merge on its own.
#[test]
fn the_routing_block_bounds_the_agents_initiative() {
    for workflow in [
        release_kit::landing::Workflow::Worktree,
        release_kit::landing::Workflow::Branches,
    ] {
        let block = release_kit::landing::routing_block(workflow);
        for phrase in [
            "guides and never drives",
            "unless the operator's request named that action",
            "authorizes the file changes alone",
            "creating or removing a worktree",
            "names no internal planning artifact and carries no agent attribution",
        ] {
            assert!(
                block.contains(phrase),
                "the block drops '{phrase}': {block}"
            );
        }
        for order in ["branch first", "Land work through"] {
            assert!(
                !block.contains(order),
                "the block still orders the agent to act: '{order}'"
            );
        }
    }
}

#[test]
fn init_propagates_an_unreadable_destination_and_writes_nothing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    // A directory where a file should land fails the pre-write read pass.
    std::fs::create_dir(target.path().join("release-plz.toml")).expect("the blocking dir creates");
    land_rust(target.path()).code(74);
    assert!(
        !target.path().join("dist-workspace.toml").exists(),
        "a failed pre-write pass must write nothing"
    );
}

// ---------------------------------------------------------------------------
// The host track: runbooks, forge documents, setup scripts, and the runner.
// ---------------------------------------------------------------------------

/// The forge-mutating step names: every one exists in both trees under the
/// same name, per `forge-setup:every-supported-forge-runs-every-step`.
const FORGE_STEPS: [&str; 11] = [
    "auto-merge",
    "bot-secrets",
    "ci-permissions",
    "default-branch",
    "install-bot",
    "merge-cleanup",
    "protect-release-lines",
    "protect-tags",
    "protect-trunk",
    "protections-check",
    "single-trunk",
];

fn repo_path(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn script_files(forge: &str) -> Vec<(String, String)> {
    let dir = repo_path("setup").join(forge);
    let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
        .expect("the tree reads")
        .map(|entry| {
            let path = entry.expect("an entry").path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read_to_string(&path).expect("a script reads"),
            )
        })
        .collect();
    files.sort();
    files
}

/// Both trees hold identical step-name sets — the parity rule, one
/// assertion for a guarantee review cannot hold.
#[test]
fn the_two_setup_trees_hold_identical_step_sets() {
    let github: Vec<String> = script_files("github").into_iter().map(|(n, _)| n).collect();
    let gitlab: Vec<String> = script_files("gitlab").into_iter().map(|(n, _)| n).collect();
    assert_eq!(github, gitlab, "the forge trees disagree on step names");
    let mut expected: Vec<&str> = FORGE_STEPS.to_vec();
    expected.sort_unstable();
    assert_eq!(github, expected, "the trees and the step table disagree");
}

/// Every step in `rk setup --list` resolves to an embedded script in every
/// tree, except `package-check` and `branch-reminder`, the two steps
/// outside the parity rule.
#[test]
fn every_listed_step_resolves_to_a_script_in_every_tree() {
    let out = rk().args(["setup", "--list"]).assert().success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    let mut listed: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let rest = line.trim_start();
            let (number, rest) = rest.split_once(". ")?;
            number.trim().parse::<u32>().ok()?;
            Some(rest.split_whitespace().next()?.to_owned())
        })
        .collect();
    assert_eq!(listed.len(), 13, "thirteen steps list: {text}");
    listed.retain(|name| !["package-check", "branch-reminder"].contains(&name.as_str()));
    listed.sort();
    let filed: Vec<String> = script_files("github").into_iter().map(|(n, _)| n).collect();
    assert_eq!(listed, filed, "the list and the tree disagree");
}

/// The static battery: `sh -n`, `set -eu`, no self-relative `cd`, and no
/// variable outside the declared environment.
#[test]
fn every_setup_script_passes_the_static_battery() {
    let allowed = [
        "RK_REPO",
        "RK_FORGE",
        "RK_TRUNK_BRANCH",
        "RK_REQUIRED_CHECK",
        "RK_BOT_APP_ID",
        "RK_BOT_PRIVATE_KEY_FILE",
        "RK_BOT_TOKEN",
        "RK_BOT_INSTALLATION",
    ];
    for forge in ["github", "gitlab"] {
        for (name, text) in script_files(forge) {
            let path = repo_path("setup").join(forge).join(&name);
            let ok = std::process::Command::new("sh")
                .arg("-n")
                .arg(&path)
                .status()
                .expect("sh runs")
                .success();
            assert!(ok, "{forge}/{name}: sh -n rejects the script");
            assert!(
                text.lines().any(|line| line.trim() == "set -eu"),
                "{forge}/{name}: no set -eu"
            );
            assert!(
                !text.contains("dirname \"$0\"") && !text.contains("cd \""),
                "{forge}/{name}: a self-relative cd survived the migration"
            );
            assert!(
                !text.contains("set -x"),
                "{forge}/{name}: tracing would print expanded variables"
            );
            for (idx, ch) in text.char_indices() {
                if ch != '$' {
                    continue;
                }
                let rest = &text[idx + 1..];
                let rest = rest.strip_prefix('{').unwrap_or(rest);
                let var: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if var.len() > 1 && var.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
                    assert!(
                        allowed.contains(&var.as_str()),
                        "{forge}/{name}: undeclared variable {var}"
                    );
                }
            }
        }
    }
}

/// No script invokes the other forge's CLI — the copy-paste a parity test
/// cannot see.
#[test]
fn no_script_invokes_the_other_forges_cli() {
    for (name, text) in script_files("github") {
        assert!(!text.contains("glab"), "github/{name} invokes glab");
    }
    for (name, text) in script_files("gitlab") {
        let names_gh = text
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|token| token == "gh");
        assert!(!names_gh, "gitlab/{name} invokes gh");
    }
}

/// Every forge with a script tree has a document, every document has a
/// tree, and neither document names the other forge's CLI.
#[test]
fn forge_documents_close_over_the_script_trees() {
    let docs: Vec<String> = std::fs::read_dir(repo_path("forges"))
        .expect("forges/ reads")
        .filter_map(|entry| {
            let name = entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            let stem = name.strip_suffix(".md")?.to_owned();
            (stem != "README").then_some(stem)
        })
        .collect();
    let trees: Vec<String> = std::fs::read_dir(repo_path("setup"))
        .expect("setup/ reads")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let mut docs = docs;
    let mut trees = trees;
    docs.sort();
    trees.sort();
    assert_eq!(docs, trees, "the documents and the trees disagree");

    let github = std::fs::read_to_string(repo_path("forges/github.md")).expect("reads");
    assert!(!github.contains("glab"), "the github document names glab");
    let gitlab = std::fs::read_to_string(repo_path("forges/gitlab.md")).expect("reads");
    let names_gh = gitlab
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| token == "gh");
    assert!(!names_gh, "the gitlab document names gh");
}

#[test]
fn forge_serves_byte_identically_and_lists() {
    rk().args(["forge", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("github").and(predicate::str::contains("gitlab")));
    let authored = std::fs::read(repo_path("forges/gitlab.md")).expect("the doc reads");
    let printed = rk()
        .args(["forge", "gitlab"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(printed, authored);
    rk().args(["forge", "sourcehut"])
        .assert()
        .code(66)
        .stderr(predicate::str::contains("no forge named"));
}

/// The `## N.` step headings of a chapter or runbook, in order.
fn numbered_headings(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| {
            line.strip_prefix("## ")
                .and_then(|rest| rest.split_once('.'))
                .is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
        })
        .collect()
}

/// The pair states the procedure once: the chapter owns each step's why, the
/// runbook owns its how, and they share the `## N.` spine — same steps, same
/// order. Per `distribution:a-runbook-renders-the-spine`.
#[test]
fn the_runbooks_match_their_method_chapters() {
    let chapter = std::fs::read_to_string(repo_path("method/02-setup.md")).expect("reads");
    let runbook = std::fs::read_to_string(repo_path("runbooks/setup.md")).expect("reads");
    assert_eq!(
        numbered_headings(&chapter),
        numbered_headings(&runbook),
        "the setup runbook's steps drifted from the chapter's"
    );

    let sequence_count = |path: &str| {
        std::fs::read_to_string(repo_path(path))
            .expect("reads")
            .lines()
            .filter(|line| {
                line.split_once(". ")
                    .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
            })
            .count()
    };
    for (chapter, runbook) in [
        ("method/03-operate.md", "runbooks/release.md"),
        ("method/07-branch-for-release.md", "runbooks/backport.md"),
        ("method/09-release-lines.md", "runbooks/release-lines.md"),
        ("method/08-worktrees.md", "runbooks/worktree.md"),
    ] {
        let rendered = std::fs::read_to_string(repo_path(runbook)).expect("reads");
        assert_eq!(
            sequence_count(chapter),
            numbered_headings(&rendered).len(),
            "{runbook}'s step count drifted from {chapter}'s"
        );
    }
}

/// A substep elaborates a step and never adds one: every `### ` heading in a
/// runbook is `Na.` where `## N.` exists in the same file.
#[test]
fn every_runbook_substep_names_its_step() {
    for entry in std::fs::read_dir(repo_path("runbooks")).expect("reads") {
        let path = entry.expect("an entry").path();
        let text = std::fs::read_to_string(&path).expect("reads");
        let name = path.file_name().and_then(|n| n.to_str()).expect("a name");
        let steps: Vec<String> = numbered_headings(&text)
            .iter()
            .map(|h| {
                h.trim_start_matches("## ")
                    .split('.')
                    .next()
                    .expect("a number")
                    .to_owned()
            })
            .collect();
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("### ") else {
                continue;
            };
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let shaped = rest[digits.len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase())
                && rest[digits.len() + 1..].starts_with(". ");
            assert!(
                !digits.is_empty() && shaped,
                "{name}: substep '{rest}' is not of the form 'Na. <title>'"
            );
            assert!(
                steps.contains(&digits),
                "{name}: substep '{rest}' names step {digits}, which the file does not have"
            );
        }
    }
}

#[test]
fn every_runbook_fence_declares_a_language() {
    for entry in std::fs::read_dir(repo_path("runbooks")).expect("reads") {
        let path = entry.expect("an entry").path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("a name")
            .to_owned();
        let text = std::fs::read_to_string(&path).expect("reads");
        let mut open = false;
        for line in text.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("```") {
                if !open {
                    assert!(
                        !rest.trim().is_empty(),
                        "{name}: a fence opens without a language"
                    );
                }
                open = !open;
            }
        }
    }
}

#[test]
fn guide_lists_and_serves_byte_identically_when_nothing_resolves() {
    rk().args(["guide", "--list"]).assert().success().stdout(
        predicate::str::contains("release")
            .and(predicate::str::contains("setup"))
            .and(predicate::str::contains("backport")),
    );
    let bare = tempfile::tempdir().expect("a bare dir exists");
    let authored = std::fs::read(repo_path("runbooks/release.md")).expect("reads");
    let printed = rk()
        .args(["guide", "release"])
        .current_dir(bare.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("--repo"))
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        printed, authored,
        "with nothing detected the runbook must print unchanged"
    );
    rk().args(["guide", "nope"])
        .current_dir(bare.path())
        .assert()
        .code(66)
        .stderr(predicate::str::contains("no runbook named"));
}

/// D6: the resolved value appears, no `<repo>` survives, and `<release pr>`
/// is still a placeholder — the middle assertion catches a future edit that
/// starts substituting the pull request number too.
#[test]
fn guide_substitutes_the_repo_and_keeps_the_pr_placeholders() {
    let bare = tempfile::tempdir().expect("a bare dir exists");
    let out = rk()
        .args([
            "guide",
            "release",
            "--forge",
            "github",
            "--repo",
            "acme/widget",
        ])
        .current_dir(bare.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("acme/widget"));
    assert!(
        !text.contains("<repo>"),
        "a placeholder survived substitution"
    );
    assert!(
        text.contains("<release pr>"),
        "the pr number must stay a placeholder"
    );
    assert!(text.contains("gh pr merge"), "the github variant is kept");
    assert!(
        !text.contains("glab mr merge"),
        "the gitlab variant is dropped"
    );
    assert!(
        !text.contains("On github:"),
        "a kept variant drops its label"
    );
}

/// A resolved technology fills `<tech>` the way `--repo` fills `<repo>`,
/// while the run-scoped placeholders stay visible.
#[test]
fn guide_substitutes_the_tech() {
    let bare = tempfile::tempdir().expect("a bare dir exists");
    let out = rk()
        .args([
            "guide",
            "setup",
            "--forge",
            "github",
            "--tech",
            "rust",
            "--repo",
            "acme/widget",
        ])
        .current_dir(bare.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("--tech rust"), "the tech is filled in");
    assert!(!text.contains("<tech>"), "a placeholder survived");
    assert!(!text.contains("<repo>"), "a placeholder survived");
}

#[test]
fn setup_list_marks_required_check_only_on_github() {
    rk().args(["setup", "--list", "--forge", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("needs --required-check"));
    rk().args(["setup", "--list", "--forge", "gitlab"])
        .assert()
        .success()
        .stdout(predicate::str::contains("needs --required-check").not());
}

#[test]
fn setup_script_prints_the_embedded_script() {
    let authored = std::fs::read(repo_path("setup/github/default-branch")).expect("reads");
    let printed = rk()
        .args(["setup", "script", "default-branch"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(printed, authored);
    let gitlab = rk()
        .args(["setup", "script", "single-trunk", "--forge", "gitlab"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        gitlab,
        std::fs::read(repo_path("setup/gitlab/single-trunk")).expect("reads")
    );
    rk().args(["setup", "script", "no-such-step"])
        .assert()
        .code(66);
    rk().args(["setup", "script", "package-check"])
        .assert()
        .code(64)
        .stderr(predicate::str::contains("binding"));
    rk().args(["setup", "script", "branch-reminder"])
        .assert()
        .code(64)
        .stderr(predicate::str::contains("no script"));
}

/// The new roots must never leak into a target.
#[test]
fn init_lands_nothing_from_the_host_only_roots() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    for root in ["setup", "runbooks", "forges"] {
        assert!(
            !target.path().join(root).exists(),
            "{root} must not be landed into a target"
        );
    }
}

#[test]
fn init_refuses_an_unsupported_pair_and_an_undetectable_forge() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "python", "--forge", "gitlab", "--target"])
        .arg(target.path())
        .assert()
        .code(64)
        .stderr(predicate::str::contains("no landable files"));
    rk().args(["init", "--tech", "rust", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains("--forge"));
}

#[test]
fn snippet_lists_the_gitlab_trees() {
    rk().args(["snippet", "--list"]).assert().success().stdout(
        predicate::str::contains("rust/gitlab/.gitlab-ci.yml")
            .and(predicate::str::contains("rust/gitlab/release-plz.toml"))
            .and(predicate::str::contains("bash/gitlab/.gitlab-ci.yml")),
    );
}

/// The reference does not leak: the promoted zones carry no identifier from
/// the implementation this work generalized. The tokens are assembled at
/// run time so this file cannot trip its own scan.
#[test]
fn the_promoted_zones_carry_no_reference_identifier() {
    let deny: Vec<String> = vec![
        format!("SDD{}", "_"),
        format!("gubasso{}ci-bot", "-"),
        format!("RELEASE_PLZ{}APP_ID", "_"),
        format!("RELEASE_PLZ{}APP_PRIVATE_KEY", "_"),
        format!("release{}setup.md", "-"),
        format!("app{}secrets", "-"),
        format!("actions{}permissions", "-"),
        format!("create{}master", "-"),
        format!("ruleset{}master", "-"),
        format!("ruleset{}develop", "-"),
        format!("ruleset{}tags", "-"),
        format!("rulesets{}check", "-"),
    ];
    let mut offenders = Vec::new();
    for zone in [
        "method",
        "bindings",
        "runbooks",
        "forges",
        "setup",
        "snippets",
        "skills",
        "src",
        "tests",
        "_docs",
        "README.md",
        "AGENTS.md",
    ] {
        scan_for_tokens(&repo_path(zone), &deny, &mut offenders);
    }
    assert!(
        offenders.is_empty(),
        "reference identifiers leaked: {offenders:?}"
    );
}

fn scan_for_tokens(path: &Path, deny: &[String], offenders: &mut Vec<String>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).expect("the zone reads") {
            scan_for_tokens(&entry.expect("an entry").path(), deny, offenders);
        }
        return;
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    for token in deny {
        if name.contains(token.as_str()) {
            offenders.push(format!("{} (file name)", path.display()));
        }
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    for (idx, line) in text.lines().enumerate() {
        for token in deny {
            if line.contains(token.as_str()) {
                offenders.push(format!("{}:{}", path.display(), idx + 1));
            }
        }
    }
}

/// The shipped payload carries no trace of the retired branch model: no
/// second long-lived branch as a word, no back-merge, and no gate job — the
/// mechanical half of the cleanup promise. Two exemptions: the single-trunk
/// scripts, whose candidate list legitimately names the branches they
/// retire, and the forge subcommand that generates a branch from an issue,
/// whose name collides with the retired branch's without meaning it. The
/// tokens are assembled at run time so this file cannot trip its own scan.
#[test]
fn the_payload_carries_no_retired_branch_model() {
    let branch = format!("dev{}", "elop");
    let issue_subcommand = format!("issue dev{}", "elop");
    let substrings = [
        format!("back{}merge", "-"),
        format!("open-release{}gate", "-"),
    ];
    let word_hit = |text: &str| -> bool {
        text.match_indices(branch.as_str()).any(|(idx, _)| {
            let boundary =
                |c: Option<char>| c.is_none_or(|c| !c.is_ascii_alphanumeric() && c != '_');
            boundary(text[..idx].chars().next_back())
                && boundary(text[idx + branch.len()..].chars().next())
        })
    };
    let mut offenders = Vec::new();
    for root in release_kit::payload_roots::PAYLOAD_ROOTS {
        let mut stack = vec![repo_path(root)];
        while let Some(path) = stack.pop() {
            if path.is_dir() {
                for entry in std::fs::read_dir(&path).expect("the root reads") {
                    stack.push(entry.expect("an entry").path());
                }
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let exempt =
                path.ends_with("github/single-trunk") || path.ends_with("gitlab/single-trunk");
            for (idx, line) in text.lines().enumerate() {
                let linked = line.contains(issue_subcommand.as_str());
                let stale = substrings.iter().any(|token| line.contains(token.as_str()))
                    || (!exempt && !linked && word_hit(line));
                if stale {
                    offenders.push(format!("{}:{}", path.display(), idx + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the retired branch model leaked into the payload: {offenders:?}"
    );
}

// ---------------------------------------------------------------------------
// The mocked forge harness.
// ---------------------------------------------------------------------------

/// A key file's contents for the tests: RFC 7468 armor around a base64
/// body, since that is what rk now demands, with a needle inside that every
/// leak assertion looks for. The body decodes to `sekret-pem-bytes!`.
const FAKE_PEM: &str =
    "-----BEGIN FAKE PRIVATE KEY-----\nc2VrcmV0LXBlbS1ieXRlcyE=\n-----END FAKE PRIVATE KEY-----\n";

/// The needle inside `FAKE_PEM`: what must reach stdin and nothing else.
const PEM_NEEDLE: &str = "c2VrcmV0LXBlbS1ieXRlcyE=";

const MOCK_GH: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/log"
env >> "$STATE/env-log"
stdin=""
case "$*" in
  *"--input -"*) stdin="$(cat)"; printf '%s\n' "$stdin" >> "$STATE/stdin-log";;
esac
cmd="$1"; shift
branch_json() {
  if [[ -f "$STATE/branch_$1" ]]; then
    sha="$(cat "$STATE/branch_$1")"
    if [[ "$2" == ".object.sha" ]]; then echo "$sha"; else echo "{\"ref\":\"refs/heads/$1\",\"object\":{\"sha\":\"$sha\"}}"; fi
  else
    echo "gh: Not Found (HTTP 404)" >&2; exit 1
  fi
}
case "$cmd" in
auth) exit 0;;
secret)
  sub="$1"; shift
  if [[ "$sub" == set ]]; then
    value="$(cat)"
    printf '%s\n' "$value" >> "$STATE/stdin-log"
    echo "$1" >> "$STATE/secrets.index"
    exit 0
  fi
  sort -u "$STATE/secrets.index" 2>/dev/null || true
  exit 0;;
repo)
  sub="$1"; shift
  if [[ "$sub" == edit ]]; then
    while (($#)); do [[ "$1" == --default-branch ]] && echo "$2" > "$STATE/default_branch"; shift; done
    exit 0
  fi
  cat "$STATE/default_branch"; exit 0;;
api)
  method=GET; path=""; query=""; fields=()
  while (($#)); do
    case "$1" in
      -X) method="$2"; shift;;
      -f|-F) fields+=("$2"); shift;;
      -q) query="$2"; shift;;
      --input) shift;;
      --paginate) ;;
      -*) ;;
      *) [[ -z "$path" ]] && path="$1";;
    esac
    shift
  done
  path="${path#/}"; path="${path%%\?*}"
  case "$method $path" in
  "GET repos/acme/widget")
    if [[ -f "$STATE/fail_repo" ]]; then echo "gh: Internal Server Error (HTTP 500)" >&2; exit 1; fi
    deleting="$(cat "$STATE/delete_branch_on_merge" 2>/dev/null || echo false)"
    self_merging="$(cat "$STATE/allow_auto_merge" 2>/dev/null || echo false)"
    squash_title="null"
    [[ -f "$STATE/squash_merge_commit_title" ]] && squash_title="\"$(cat "$STATE/squash_merge_commit_title")\""
    squash_message="null"
    [[ -f "$STATE/squash_merge_commit_message" ]] && squash_message="\"$(cat "$STATE/squash_merge_commit_message")\""
    if [[ "$query" == ".id" ]]; then echo 1
    elif [[ "$query" == ".delete_branch_on_merge" ]]; then echo "$deleting"
    elif [[ "$query" == ".allow_auto_merge" ]]; then echo "$self_merging"
    elif [[ "$query" == ".squash_merge_commit_title" ]]; then echo "${squash_title//\"/}"
    elif [[ "$query" == ".squash_merge_commit_message" ]]; then echo "${squash_message//\"/}"
    else echo "{\"id\":1,\"default_branch\":\"$(cat "$STATE/default_branch")\",\"delete_branch_on_merge\":$deleting,\"allow_auto_merge\":$self_merging,\"squash_merge_commit_title\":$squash_title,\"squash_merge_commit_message\":$squash_message}"; fi;;
  "PATCH repos/acme/widget")
    for f in "${fields[@]}"; do
      case "$f" in
        delete_branch_on_merge=*) echo "${f#delete_branch_on_merge=}" > "$STATE/delete_branch_on_merge";;
        allow_auto_merge=*) echo "${f#allow_auto_merge=}" > "$STATE/allow_auto_merge";;
        squash_merge_commit_title=*) echo "${f#squash_merge_commit_title=}" > "$STATE/squash_merge_commit_title";;
        squash_merge_commit_message=*) echo "${f#squash_merge_commit_message=}" > "$STATE/squash_merge_commit_message";;
      esac
    done
    echo '{}';;
  "GET repos/"*"/git/ref/heads/"*)
    branch_json "${path##*/heads/}" "$query";;
  "POST repos/"*"/git/refs")
    ref=""; sha=""
    for f in "${fields[@]}"; do case "$f" in ref=*) ref="${f#ref=}";; sha=*) sha="${f#sha=}";; esac; done
    echo "$sha" > "$STATE/branch_${ref##*/}"
    echo '{}';;
  "DELETE repos/"*"/git/refs/heads/"*)
    rm -f "$STATE/branch_${path##*/heads/}";;
  "GET repos/"*"/compare/"*)
    pair="${path##*/compare/}"
    key="compare_${pair//.../_}"
    status="identical"
    [[ -f "$STATE/$key" ]] && status="$(cat "$STATE/$key")"
    if [[ "$status" == "error" ]]; then echo "gh: Internal Server Error (HTTP 500)" >&2; exit 1; fi
    if [[ "$query" == ".status" ]]; then echo "$status"; else echo "{\"status\":\"$status\"}"; fi;;
  "PUT repos/"*"/actions/permissions/workflow")
    echo "write true" > "$STATE/perms";;
  "GET repos/"*"/actions/permissions/workflow")
    perms="none false"; [[ -f "$STATE/perms" ]] && perms="$(cat "$STATE/perms")"
    if [[ -n "$query" ]]; then echo "$perms"; else
      echo "{\"default_workflow_permissions\":\"${perms%% *}\",\"can_approve_pull_request_reviews\":${perms##* }}"
    fi;;
  "GET repos/"*"/actions/secrets")
    names=""
    if [[ -f "$STATE/secrets.index" ]]; then
      while read -r n; do names+="{\"name\":\"$n\"},"; done < <(sort -u "$STATE/secrets.index")
    fi
    echo "{\"secrets\":[${names%,}]}";;
  "PUT user/installations/"*"/repositories/"*)
    touch "$STATE/installed"; echo '{}';;
  "GET repos/acme/widget/rulesets")
    if [[ "$query" == ".[].name" ]]; then
      cat "$STATE/rulesets.index" 2>/dev/null || true
    elif [[ "$query" == *"select(.name =="* ]]; then
      want="$(sed 's/.*select(.name == "\([^"]*\)").*/\1/' <<<"$query")"
      line=""
      [[ -f "$STATE/rulesets.index" ]] && line="$(grep -n -x -F "$want" "$STATE/rulesets.index" | head -1 | cut -d: -f1)"
      if [[ -n "$line" ]]; then
        if [[ "$query" == *"| .id" ]]; then echo "$line"; else echo "$want"; fi
      fi
    else
      out="["
      if [[ -f "$STATE/rulesets.index" ]]; then
        i=0
        while read -r n; do i=$((i+1)); out+="{\"name\":\"$n\",\"id\":$i},"; done < "$STATE/rulesets.index"
      fi
      out="${out%,}"
      echo "$out]"
    fi;;
  "POST repos/acme/widget/rulesets")
    name="$(grep -o '"name": "[^"]*"' <<<"$stdin" | head -1 | cut -d'"' -f4)"
    echo "$name" >> "$STATE/rulesets.index"
    printf '%s\n' "$stdin" > "$STATE/ruleset_$name"
    echo '{}';;
  "PUT repos/acme/widget/rulesets/"*)
    n="${path##*/}"
    name="$(sed -n "${n}p" "$STATE/rulesets.index")"
    printf '%s\n' "$stdin" > "$STATE/ruleset_$name"
    echo '{}';;
  "GET repos/acme/widget/rulesets/"*)
    n="${path##*/}"
    name="$(sed -n "${n}p" "$STATE/rulesets.index")"
    body="$(cat "$STATE/ruleset_$name" 2>/dev/null)"
    case "$query" in
      "") printf '%s\n' "$body";;
      *"bypass_actors"*) echo 0;;
      *'contains(["pull_request"])'*) echo true;;
      *'contains(["required_status_checks"])'*) echo true;;
      *'contains(["deletion","non_fast_forward"])'*) echo true;;
      *"allowed_merge_methods"*) echo '["squash"]';;
      *"required_status_checks[].context"*) grep -o '"context": "[^"]*"' <<<"$body" | head -1 | cut -d'"' -f4;;
      *"required_approving_review_count"*) echo 0;;
      *) echo null;;
    esac;;
  *) echo '{}';;
  esac
  exit 0;;
esac
exit 0
"#;

const MOCK_GLAB: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/log"
cmd="$1"; shift
case "$cmd" in
auth) exit 0;;
variable)
  sub="$1"; shift
  value="$(cat)"
  printf '%s\n' "$value" >> "$STATE/stdin-log"
  touch "$STATE/var_$1"
  exit 0;;
api)
  method=GET; path=""; fields=()
  while (($#)); do
    case "$1" in
      -X) method="$2"; shift;;
      -f|-F) fields+=("$2"); shift;;
      -*) ;;
      *) [[ -z "$path" ]] && path="$1";;
    esac
    shift
  done
  case "$method $path" in
  "GET "*"/variables/RELEASE_BOT_TOKEN")
    if [[ -f "$STATE/var_RELEASE_BOT_TOKEN" ]]; then
      echo '{"key":"RELEASE_BOT_TOKEN","masked":true}'
    else
      echo "glab: 404 Not Found (HTTP 404)" >&2; exit 1
    fi;;
  "GET "*"/access_tokens") echo '[]';;
  "GET "*"/protected_tags/v%2A")
    if [[ -f "$STATE/tag_protected" ]]; then echo '{"name":"v*"}'; else echo "glab: 404 Not Found (HTTP 404)" >&2; exit 1; fi;;
  "POST "*"/protected_tags")
    touch "$STATE/tag_protected"; echo '{}';;
  "GET "*"/protected_branches/master")
    if [[ -f "$STATE/protected_master" ]]; then
      echo '{"name":"master","push_access_levels":[{"access_level":0}],"merge_access_levels":[{"access_level":40}],"allow_force_push":false}'
    else
      echo "glab: 404 Not Found (HTTP 404)" >&2; exit 1
    fi;;
  "GET "*"/protected_branches/"*)
    echo "glab: 404 Not Found (HTTP 404)" >&2; exit 1;;
  "POST "*"/protected_branches")
    touch "$STATE/protected_master"; echo '{}';;
  "PATCH "*"/protected_branches/master")
    echo '{}';;
  "PUT projects/"*)
    for f in "${fields[@]}"; do
      case "$f" in
        default_branch=*) echo "${f#default_branch=}" > "$STATE/default_branch";;
        remove_source_branch_after_merge=*) echo "${f#remove_source_branch_after_merge=}" > "$STATE/remove_source_branch";;
        only_allow_merge_if_pipeline_succeeds=*) echo "${f#only_allow_merge_if_pipeline_succeeds=}" > "$STATE/pipeline_required";;
        merge_method=*) echo "${f#merge_method=}" > "$STATE/merge_method";;
        squash_option=*) echo "${f#squash_option=}" > "$STATE/squash_option";;
        squash_commit_template=*) echo "${f#squash_commit_template=}" > "$STATE/squash_commit_template";;
      esac
    done
    echo '{}';;
  "GET projects/"*)
    removing="$(cat "$STATE/remove_source_branch" 2>/dev/null || echo false)"
    piped="$(cat "$STATE/pipeline_required" 2>/dev/null || echo false)"
    merging="$(cat "$STATE/merge_method" 2>/dev/null || echo merge)"
    squashing="$(cat "$STATE/squash_option" 2>/dev/null || echo never)"
    template="$(cat "$STATE/squash_commit_template" 2>/dev/null || echo)"
    echo "{\"id\":1,\"default_branch\":\"$(cat "$STATE/default_branch")\",\"jobs_enabled\":true,\"only_allow_merge_if_pipeline_succeeds\":$piped,\"merge_method\":\"$merging\",\"squash_option\":\"$squashing\",\"remove_source_branch_after_merge\":$removing,\"squash_commit_template\":\"$template\"}";;
  *) echo '{}';;
  esac
  exit 0;;
esac
exit 0
"#;

/// The OpenSSL stand-in: it records its argument list, its environment,
/// and the key bytes it received on standard input, then emits a fixed
/// signature — enough for the JWT to assemble and for the tests to prove
/// where the key travelled.
const MOCK_OPENSSL: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/openssl-log"
env >> "$STATE/openssl-env-log"
cat >> "$STATE/openssl-stdin-log"
printf 'sig-bytes'
"#;

/// The curl stand-in for the App-credential reads: the repository
/// installation answers by the shared `installed` state file, so a grant
/// the mock forge records flips the observation, and the account listing
/// names installation 42 — the id the grant must then carry.
const MOCK_CURL: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/curl-log"
cat >> "$STATE/curl-stdin-log"
if [[ -f "$STATE/curl_fail" ]]; then
  echo "curl: (6) Could not resolve host: api.github.com" >&2
  exit 6
fi
url="${@: -1}"
case "$url" in
  *"/users/acme/installation")
    printf '{"id":42,"account":{"login":"acme"}}\n200';;
  *"/orgs/"*"/installation")
    printf '{"message":"Not Found"}\n404';;
  *"/repos/"*"/installation")
    if [[ -f "$STATE/installed" ]]; then
      printf '{"id":42,"app_id":7,"app_slug":"bot"}\n200'
    else
      printf '{"message":"Not Found"}\n404'
    fi;;
  *)
    printf '{}\n200';;
esac
"#;

/// One mocked setup fixture: a scratch home, a scratch target, and a mock
/// forge CLI recording every invocation and every stdin byte.
struct ForgeFixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    mock: tempfile::TempDir,
}

impl ForgeFixture {
    fn new() -> Self {
        let fixture = Self {
            home: tempfile::tempdir().expect("a scratch home exists"),
            target: tempfile::tempdir().expect("a scratch target exists"),
            mock: tempfile::tempdir().expect("a scratch mock dir exists"),
        };
        for (name, body) in [
            ("gh", MOCK_GH),
            ("glab", MOCK_GLAB),
            ("openssl", MOCK_OPENSSL),
            ("curl", MOCK_CURL),
        ] {
            let path = fixture.mock.path().join(name);
            std::fs::write(
                &path,
                body.replace("__STATE__", &fixture.mock.path().to_string_lossy()),
            )
            .expect("the mock writes");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("the mock is executable");
            }
        }
        // A fresh, unconfigured repository: default branch main, one commit
        // on develop and main, bash so package-check has nothing to run.
        fixture.seed("default_branch", "main");
        fixture.seed("branch_develop", "abc123");
        fixture.seed("branch_main", "abc123");
        std::fs::write(fixture.target.path().join("VERSION"), "0.1.0\n").expect("VERSION writes");
        // A real repository, so the branch-reminder step has a hooks
        // directory to write into; it has no remote, so detection still
        // resolves nothing and every test passes --repo and --forge.
        // Scrubbed like every other fixture git: under a pre-commit run
        // from a linked worktree, the exported hook variables resolve to
        // the real repository's shared config, and an unscrubbed init
        // acts there instead of in the tempdir.
        let mut init = std::process::Command::new("git");
        for var in GIT_HOOK_VARS {
            init.env_remove(var);
        }
        let init = init
            .args(["init", "-q"])
            .current_dir(fixture.target.path())
            .status()
            .expect("git runs");
        assert!(init.success(), "the fixture target initializes");
        fixture
    }

    fn seed(&self, name: &str, value: &str) {
        std::fs::write(self.mock.path().join(name), value).expect("the state seeds");
    }

    /// A key file outside the target, in the mode rk demands. `mode` is the
    /// lever the refusal tests pull.
    fn key_file_with(&self, name: &str, body: &str, mode: u32) -> PathBuf {
        let path = self.home.path().join(name);
        std::fs::write(&path, body).expect("the key file writes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
                .expect("the key file takes its mode");
        }
        path
    }

    /// The ordinary case: a well-formed, owner-only PEM.
    fn key_file(&self) -> PathBuf {
        self.key_file_with("bot.pem", FAKE_PEM, 0o600)
    }

    /// Substitute the mock forge CLI, for the cases that need a child
    /// with a particular stream or signal behavior rather than a
    /// working forge.
    fn replace_gh(&self, body: &str) {
        let path = self.mock.path().join("gh");
        std::fs::write(&path, body).expect("the mock writes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("the mock is executable");
        }
    }

    /// Substitute the curl stand-in, for the cases that need the App
    /// read to fail or to echo.
    fn replace_curl(&self, body: &str) {
        let path = self.mock.path().join("curl");
        std::fs::write(&path, body).expect("the mock writes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("the mock is executable");
        }
    }

    /// The newest run directory, by the timestamp its id opens with.
    fn latest_run(&self) -> PathBuf {
        let mut runs: Vec<PathBuf> = std::fs::read_dir(self.runs_root())
            .expect("the runs root reads")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        runs.sort();
        runs.pop().expect("a run was journalled")
    }

    fn state(&self, name: &str) -> PathBuf {
        self.mock.path().join(name)
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.state("log")).unwrap_or_default()
    }

    fn stdin_log(&self) -> String {
        std::fs::read_to_string(self.state("stdin-log")).unwrap_or_default()
    }

    /// Everything the mock forge CLI found in its own environment.
    fn env_log(&self) -> String {
        std::fs::read_to_string(self.state("env-log")).unwrap_or_default()
    }

    fn runs_root(&self) -> PathBuf {
        self.home.path().join("release-kit/runs")
    }

    fn rk(&self, args: &[&str]) -> Command {
        let mut command = rk_scrubbed();
        command
            .env("HOME", self.home.path())
            .env("XDG_STATE_HOME", self.home.path())
            .env("RK_GH_BIN", self.mock.path().join("gh"))
            .env("RK_GLAB_BIN", self.mock.path().join("glab"))
            .env("RK_OPENSSL_BIN", self.mock.path().join("openssl"))
            .env("RK_CURL_BIN", self.mock.path().join("curl"))
            .env_remove("RK_BOT_APP_ID")
            .env_remove("RK_BOT_PRIVATE_KEY")
            .env_remove("RK_BOT_PRIVATE_KEY_FILE")
            .env_remove("RK_BOT_TOKEN")
            .args(args)
            .args(["--target"])
            .arg(self.target.path());
        command
    }
}

/// Preview writes nothing and calls no external command.
#[test]
fn setup_preview_calls_nothing_and_materializes_nothing() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN").and(predicate::str::contains("setup/github")));
    assert_eq!(fixture.log(), "", "preview invoked the forge CLI");
    let runs: Vec<_> = std::fs::read_dir(fixture.runs_root())
        .expect("the preview persisted a journal")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(runs.len(), 1);
    assert!(
        !runs[0].join("scripts").exists(),
        "preview materialized a script"
    );
}

/// Under --json the stream is NDJSON: every stdout line parses, the first
/// names the schema.
#[test]
fn setup_json_is_ndjson_opening_with_the_schema() {
    let fixture = ForgeFixture::new();
    let out = fixture
        .rk(&["setup"])
        .args(["--repo", "acme/widget", "--forge", "github", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let lines: Vec<serde_json::Value> = String::from_utf8_lossy(&out)
        .lines()
        .map(|line| serde_json::from_str(line).expect("every stdout line is one JSON object"))
        .collect();
    assert!(!lines.is_empty());
    assert_eq!(lines[0]["type"], "schema");
    assert_eq!(lines[0]["schema"], "rk.events/1");
}

/// The flagship: a full apply runs every step against the mocked forge, a
/// rerun re-asserts, no secret reaches argv or the journal, and check then
/// reports every step satisfied.
#[test]
#[allow(clippy::too_many_lines)]
fn a_full_github_apply_lands_reasserts_and_checks_clean() {
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    let apply = |fixture: &ForgeFixture| {
        let mut command = fixture.rk(&["setup"]);
        command
            .args(["--repo", "acme/widget", "--forge", "github"])
            .args(["--apply", "--required-check", "test-check"])
            .env("RK_BOT_APP_ID", "314159")
            .env("RK_BOT_PRIVATE_KEY_FILE", &key);
        command
    };
    apply(&fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains("skipped protect-release-lines"));

    // The forge now holds the desired state: the trunk is the default and
    // the sole long-lived branch, under exactly the two owned protections.
    assert_eq!(
        std::fs::read_to_string(fixture.state("default_branch")).expect("state reads"),
        "master\n"
    );
    assert!(fixture.state("branch_master").is_file());
    assert!(!fixture.state("branch_main").exists());
    assert!(!fixture.state("branch_develop").exists());
    assert!(fixture.state("installed").is_file());
    let rulesets = std::fs::read_to_string(fixture.state("rulesets.index")).expect("reads");
    assert_eq!(
        rulesets.lines().count(),
        2,
        "exactly two protections: {rulesets}"
    );
    let body = std::fs::read_to_string(fixture.state("ruleset_master-protection")).expect("reads");
    assert!(body.contains(r#""context": "test-check""#));
    assert!(body.contains(r#""context": "pr-title""#));
    assert!(body.contains(r#""allowed_merge_methods": ["squash"]"#));
    assert_eq!(
        std::fs::read_to_string(fixture.state("squash_merge_commit_title")).expect("state reads"),
        "PR_TITLE\n",
        "the squash title source is the request's title"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.state("squash_merge_commit_message")).expect("state reads"),
        "PR_BODY\n",
        "the squash message source is the request's body"
    );

    // The key reached stdin and nothing else — not an argument list, and
    // not the environment, which carried only the path.
    assert!(fixture.stdin_log().contains(PEM_NEEDLE));
    assert!(
        !fixture.log().contains(PEM_NEEDLE),
        "a secret reached a process argument list"
    );
    assert!(
        !fixture.env_log().contains(PEM_NEEDLE),
        "a secret reached a child environment"
    );
    assert!(
        !fixture.env_log().contains("RK_BOT_PRIVATE_KEY_FILE"),
        "a child was told where the key file is, and could have reopened it"
    );

    // The App JWT path: the key bytes reached openssl on standard input
    // and nowhere else, and the token itself travelled to curl in a
    // header read from standard input, never an argument list.
    let openssl_stdin =
        std::fs::read_to_string(fixture.state("openssl-stdin-log")).expect("openssl saw stdin");
    assert!(openssl_stdin.contains(PEM_NEEDLE));
    let openssl_args =
        std::fs::read_to_string(fixture.state("openssl-log")).expect("openssl logged its argv");
    assert!(!openssl_args.contains(PEM_NEEDLE));
    assert!(
        !openssl_args.contains("bot.pem"),
        "the signer was told the key's path: {openssl_args}"
    );
    let openssl_args_full =
        std::fs::read_to_string(fixture.state("openssl-log")).expect("openssl logged its argv");
    assert_eq!(
        openssl_args_full.lines().count(),
        1,
        "a full apply mints one JWT, so the key is read once: {openssl_args_full}"
    );
    let curl_args = std::fs::read_to_string(fixture.state("curl-log")).expect("curl logged argv");
    assert!(
        !curl_args.contains("Bearer"),
        "the App JWT reached an argument list: {curl_args}"
    );
    assert!(
        std::fs::read_to_string(fixture.state("curl-stdin-log"))
            .expect("curl saw stdin")
            .contains("Authorization: Bearer "),
        "the App JWT travels as a bearer header on standard input"
    );
    let mut journal_dirs: Vec<PathBuf> = std::fs::read_dir(fixture.runs_root())
        .expect("the journal root reads")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    journal_dirs.sort();
    assert_eq!(journal_dirs.len(), 1);
    let run = &journal_dirs[0];
    for file in ["meta.json", "events.jsonl", "transcript.txt"] {
        let text = std::fs::read_to_string(run.join(file)).expect("the journal file reads");
        assert!(!text.contains(PEM_NEEDLE), "{file} carries key material");
    }
    assert!(
        !run.join("scripts").exists(),
        "a clean run keeps its materialized scripts"
    );
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(run.join("meta.json")).expect("meta reads"))
            .expect("meta parses");
    assert_eq!(meta["schema"], "rk.run-meta/1");
    assert_eq!(meta["exit_code"], 0);
    assert!(
        meta["scripts"]
            .as_array()
            .is_some_and(|list| !list.is_empty()),
        "the journal records the materialized digests"
    );
    assert!(
        meta["secrets"].as_array().is_some_and(|list| list
            .iter()
            .any(|s| { s["secret"] == "RK_BOT_PRIVATE_KEY_FILE" && s["source"] == "file" })),
        "the journal records the secret handling"
    );

    // A rerun re-asserts: every step reports satisfied, nothing mutates.
    let before = fixture.log();
    apply(&fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains("satisfied"));
    let after = fixture.log();
    let new_calls = &after[before.len()..];
    assert!(
        !new_calls.contains("-X POST")
            && !new_calls.contains("-X PUT")
            && !new_calls.contains("-X DELETE"),
        "a rerun mutated the forge: {new_calls}"
    );

    // Two runs leave two distinct journal directories.
    assert_eq!(
        std::fs::read_dir(fixture.runs_root())
            .expect("reads")
            .count(),
        2
    );

    // The optional step applies by name, and the ownership check still
    // holds with the third protection present.
    fixture
        .rk(&["setup", "step", "protect-release-lines"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .assert()
        .success();
    let rulesets = std::fs::read_to_string(fixture.state("rulesets.index")).expect("reads");
    assert_eq!(
        rulesets.lines().count(),
        3,
        "the optional protection joins the owned set: {rulesets}"
    );

    // And check reports clean at exit 0, with the same two App exports
    // the apply carried — install-bot is readable only to the App itself.
    fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &key)
        .assert()
        .success()
        .stdout(predicate::str::contains("ok install-bot"))
        .stdout(predicate::str::contains("ok protections-check"));
}

/// Without the App credentials the installation is unreadable: the check
/// reports the step unknown by name, with the exports that answer it, and
/// spends no forge call finding out.
#[test]
fn check_reports_install_bot_unknown_without_app_credentials() {
    let fixture = ForgeFixture::new();
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("unknown install-bot"), "{text}");
    assert!(
        text.contains("RK_BOT_APP_ID and RK_BOT_PRIVATE_KEY_FILE"),
        "the remediation names the exports: {text}"
    );
    assert!(
        !fixture.state("curl-log").exists(),
        "no credentials, no forge call"
    );
}

/// The grant: the observation answers 404 as the App, rk reads the
/// installation id from the App's own list and hands it to the script,
/// the script issues exactly the documented PUT, and the readback — as
/// the App again — proves it. The dead user-token listings are gone.
#[test]
fn install_bot_grants_with_the_discovered_installation_id() {
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    let step = |fixture: &ForgeFixture| {
        let mut command = fixture.rk(&["setup", "step", "install-bot"]);
        command
            .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
            .env("RK_BOT_APP_ID", "314159")
            .env("RK_BOT_PRIVATE_KEY_FILE", &key);
        command
    };
    step(&fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains("applied install-bot"))
        .stderr(predicate::str::contains(
            "installation 42 covers acme/widget",
        ));
    assert!(fixture.state("installed").is_file());
    let log = fixture.log();
    assert!(
        log.contains("-X PUT user/installations/42/repositories/"),
        "the grant carries the discovered id: {log}"
    );
    assert!(
        !log.contains("GET user/installations"),
        "a user token cannot list installations, and nothing may try: {log}"
    );
    let curl = std::fs::read_to_string(fixture.state("curl-log")).expect("curl ran");
    assert!(curl.contains("repos/acme/widget/installation"), "{curl}");
    assert!(curl.contains("users/acme/installation"), "{curl}");
    let openssl = std::fs::read_to_string(fixture.state("openssl-log")).expect("openssl ran");
    assert_eq!(
        openssl.lines().count(),
        1,
        "one run mints one JWT, so the key is read exactly once: {openssl}"
    );

    // A rerun observes satisfied and grants nothing twice.
    let before = fixture.log();
    step(&fixture)
        .assert()
        .success()
        .stdout(predicate::str::contains("satisfied"));
    let after = fixture.log();
    assert!(
        !after[before.len()..].contains("-X PUT"),
        "a rerun mutated the forge"
    );
}

/// A curl that echoes what it was handed cannot leak the token, because
/// the credential-carrying spawn bypasses the journaling executor
/// entirely: no stream of it reaches any journal file, and the token and
/// its signature segment are redaction needles besides.
#[test]
fn an_echoing_curl_cannot_put_the_jwt_in_the_journal() {
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    fixture.replace_curl(
        "#!/usr/bin/env bash
cat
printf '{}\n500'
",
    );
    fixture
        .rk(&["setup", "step", "install-bot"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &key)
        .assert()
        .code(73);
    let run = fixture.latest_run();
    for file in ["meta.json", "events.jsonl", "transcript.txt"] {
        let text = std::fs::read_to_string(run.join(file)).expect("the journal file reads");
        assert!(
            !text.contains("Authorization"),
            "{file} carries the curl child's streams"
        );
        assert!(
            !text.contains("eyJhbGciOiJSUzI1NiI"),
            "{file} carries the App JWT"
        );
        assert!(!text.contains(PEM_NEEDLE), "{file} carries key material");
    }
}

/// A curl that echoes its input and then fails cannot leak the token into
/// the surfaced diagnostic: the failure detail is scrubbed against the
/// token and its signature before anything is printed or journaled.
#[test]
fn a_failing_curl_cannot_put_the_jwt_in_the_diagnostic() {
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    fixture.replace_curl(
        "#!/usr/bin/env bash
grep Authorization >&2
exit 7
",
    );
    let out = fixture
        .rk(&["setup", "step", "install-bot"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &key)
        .assert()
        .code(73)
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("eyJhbGciOiJSUzI1NiI"),
        "the diagnostic carries the App JWT: {stderr}"
    );
    assert!(
        stderr.contains("[redacted]"),
        "the echoed header must surface redacted: {stderr}"
    );
    let run = fixture.latest_run();
    for file in ["meta.json", "events.jsonl", "transcript.txt"] {
        let text = std::fs::read_to_string(run.join(file)).expect("the journal file reads");
        assert!(
            !text.contains("eyJhbGciOiJSUzI1NiI"),
            "{file} carries the App JWT"
        );
    }
}

/// The signing and carrying helpers inherit no forge credential, so a
/// helper that dumps its whole environment on failure has nothing of the
/// operator's to dump. The carrier alone inherits the variables naming
/// this host's certificate authorities — every one of them, because a
/// `curl` that locates its trust store by environment cannot verify the
/// forge without the one its host uses — and the signer, which opens no
/// connection, inherits none.
#[test]
fn the_carrier_alone_inherits_the_trust_store_and_neither_helper_the_forge_token() {
    const TRUST: [(&str, &str); 4] = [
        ("CURL_CA_BUNDLE", "/scratch/curl-bundle.pem"),
        ("SSL_CERT_DIR", "/scratch/certs"),
        ("SSL_CERT_FILE", "/scratch/ca-bundle.pem"),
        ("NIX_SSL_CERT_FILE", "/scratch/nix-bundle.pem"),
    ];
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    let env_log = fixture.state("curl-env-log");
    fixture.replace_curl(&format!(
        "#!/usr/bin/env bash
cat > /dev/null
env > '{}'
exit 7
",
        env_log.display()
    ));
    let mut command = fixture.rk(&["setup", "step", "install-bot"]);
    command
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("GH_TOKEN", "gh-secret-value")
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &key);
    for (name, value) in TRUST {
        command.env(name, value);
    }
    let out = command.assert().code(73).get_output().clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("gh-secret-value"),
        "a helper child was handed the forge token: {stderr}"
    );
    let carrier = std::fs::read_to_string(&env_log).expect("curl saw an env");
    let signer =
        std::fs::read_to_string(fixture.state("openssl-env-log")).expect("openssl saw an env");
    for (helper, seen) in [("the carrier", &carrier), ("the signer", &signer)] {
        assert!(
            !seen.contains("gh-secret-value"),
            "{helper} was handed the forge token"
        );
    }
    for (name, value) in TRUST {
        assert!(
            carrier.contains(&format!("{name}={value}")),
            "the carrier lost {name}: {carrier}"
        );
        assert!(
            !signer.contains(&format!("{name}={value}")),
            "the signer was handed {name}, which it has no connection to verify: {signer}"
        );
    }
}

/// A curl failure surfaces the line curl leads with: what follows it is
/// prose pointing at a web page, so a diagnostic built from the last line
/// would name nothing an operator could act on.
#[test]
fn a_curl_failure_names_the_reason_curl_led_with() {
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    fixture.replace_curl(
        "#!/usr/bin/env bash
cat > /dev/null
cat >&2 <<'REASON'
curl: (60) SSL certificate problem: unable to get local issuer certificate
More details here: https://curl.se/docs/sslcerts.html

curl failed to verify the legitimacy of the server and therefore could not
establish a secure connection to it. To learn more about this situation and
how to fix it, please visit the web page mentioned above.
REASON
exit 60
",
    );
    let out = fixture
        .rk(&["setup", "step", "install-bot"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &key)
        .assert()
        .code(73)
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("exit 60")
            && stderr.contains("SSL certificate problem: unable to get local issuer certificate"),
        "the diagnostic drops what curl named: {stderr}"
    );
    assert!(
        !stderr.contains("web page mentioned above"),
        "the diagnostic carries the epilogue instead of the reason: {stderr}"
    );
}

/// An unreachable forge leaves the observation undecided, and an apply on
/// an undecided state refuses before anything mutates.
#[test]
fn install_bot_refuses_to_apply_when_the_forge_is_unreachable() {
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    fixture.seed("curl_fail", "1");
    fixture
        .rk(&["setup", "step", "install-bot"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &key)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("cannot observe the current state"));
    assert!(
        !fixture.log().contains("-X PUT"),
        "an undecided observation must precede every mutation"
    );
}

/// A check against an unconfigured forge reports per step and exits 1.
#[test]
fn setup_check_reports_per_step_and_exits_1_on_violations() {
    let fixture = ForgeFixture::new();
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .clone();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("ok package-check"), "{text}");
    assert!(text.contains("unsatisfied default-branch"), "{text}");
    assert!(text.contains("unsatisfied auto-merge"), "{text}");
    assert!(text.contains("unsatisfied protect-tags"), "{text}");
    assert!(text.contains("skipped protect-release-lines"), "{text}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not satisfied"), "{stderr}");
}

/// On GitHub a full apply refuses without --required-check before anything
/// runs; on GitLab the flag is a usage error.
#[test]
fn the_required_check_flag_is_demanded_and_refused_per_forge() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("--required-check"));
    assert_eq!(fixture.log(), "", "the refusal must precede every step");

    fixture
        .rk(&["setup"])
        .args(["--repo", "acme/widget", "--forge", "gitlab"])
        .args(["--required-check", "test-check"])
        .assert()
        .code(64)
        .stderr(predicate::str::contains("whole pipeline"));
}

/// The check name reaches the protection body verbatim, spaces included.
#[test]
fn the_check_name_reaches_the_protection_body_verbatim() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");
    fixture
        .rk(&["setup", "step", "protect-trunk"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .args(["--required-check", "build (matrix, 1)"])
        .assert()
        .success();
    assert!(
        fixture
            .stdin_log()
            .contains(r#""context": "build (matrix, 1)""#),
        "the check name was altered on the way to the body: {}",
        fixture.stdin_log()
    );
    assert!(
        fixture.stdin_log().contains(r#""context": "pr-title""#),
        "the title check rides beside the named one: {}",
        fixture.stdin_log()
    );
}

/// The GitLab trunk protection asserts the squash template beside the
/// merge shape, and its observation faults a project whose template is
/// anything but the merge request's title.
#[test]
fn a_gitlab_protect_trunk_apply_asserts_the_squash_template() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");

    // The protection exists but the project settings do not hold: the
    // observation names the squash template among the faults.
    fixture.seed("protected_master", "");
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "gitlab"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("the squash template is not the merge request's title"),
        "an unset template must read as drift: {text}"
    );

    // The apply asserts every setting, and the observation then agrees.
    fixture
        .rk(&["setup", "step", "protect-trunk"])
        .args(["--repo", "acme/widget", "--forge", "gitlab", "--apply"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(fixture.state("squash_commit_template")).expect("state reads"),
        "%{title}\n",
        "the squash template is the merge request's title"
    );
    assert_eq!(
        std::fs::read_to_string(fixture.state("squash_option")).expect("state reads"),
        "always\n"
    );

    // Both satisfied limitations survive the protections aggregate: the
    // title gate's beside the protected tags', neither shadowing the
    // other.
    fixture.seed("tag_protected", "");
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "gitlab"])
        .assert()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("stops accident, not authority"),
        "the title limitation reports: {text}"
    );
    assert!(
        text.contains("delete a protected tag"),
        "the tag limitation survives beside it: {text}"
    );
}

/// Ordering is enforced by observation: a protection step refuses while the
/// trunk has not been proven the default.
#[test]
fn a_protection_step_refuses_before_the_trunk_is_the_default() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup", "step", "protect-trunk"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .args(["--required-check", "test-check"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("default-branch"));
    assert!(
        !fixture.log().contains("-X POST"),
        "the refusal must write nothing"
    );
}

/// A right-named ruleset proves nothing on its own: one covering another
/// ref reads as drift, not as a protected trunk.
#[test]
fn check_reports_a_ruleset_covering_the_wrong_ref() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");
    fixture.seed("rulesets.index", "master-protection\n");
    fixture.seed(
        "ruleset_master-protection",
        r#"{
  "name": "master-protection",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": { "include": ["refs/heads/other"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": { "required_approving_review_count": 0, "allowed_merge_methods": ["squash"] }
    },
    {
      "type": "required_status_checks",
      "parameters": { "required_status_checks": [{ "context": "test" }] }
    }
  ]
}"#,
    );
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("unsatisfied protect-trunk") && text.contains("refs/heads/master"),
        "a ruleset covering the wrong ref must read as drift: {text}"
    );

    // A matching exclusion negates a canonical include the same way.
    fixture.seed(
        "ruleset_master-protection",
        r#"{
  "name": "master-protection",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": { "include": ["refs/heads/master"], "exclude": ["refs/heads/master"] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": { "required_approving_review_count": 0, "allowed_merge_methods": ["squash"] }
    },
    {
      "type": "required_status_checks",
      "parameters": { "required_status_checks": [{ "context": "test" }] }
    }
  ]
}"#,
    );
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("unsatisfied protect-trunk") && text.contains("excludes refs"),
        "a self-negating exclusion must read as drift: {text}"
    );
}

/// Proven trunk drift survives a repository-settings outage: the ruleset
/// faults were already proven, so the failing settings read downgrades
/// nothing to unknown.
#[test]
fn check_keeps_proven_trunk_drift_over_a_settings_outage() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");
    fixture.seed("rulesets.index", "master-protection\n");
    fixture.seed(
        "ruleset_master-protection",
        r#"{
  "name": "master-protection",
  "target": "branch",
  "enforcement": "disabled",
  "bypass_actors": [],
  "conditions": {
    "ref_name": { "include": ["refs/heads/master"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": { "required_approving_review_count": 0, "allowed_merge_methods": ["squash"] }
    },
    {
      "type": "required_status_checks",
      "parameters": { "required_status_checks": [{ "context": "test" }] }
    }
  ]
}"#,
    );
    fixture.seed("fail_repo", "");
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("unsatisfied protect-trunk") && text.contains("not active"),
        "proven drift must win over the settings outage: {text}"
    );
}

/// The executable protections-check passes only when the script and the
/// authoritative observation agree: a right-named ruleset covering the
/// wrong ref fails the step even though the script's own checks pass.
#[test]
fn protections_check_apply_reads_back_through_the_observation() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");
    fixture.seed("rulesets.index", "master-protection\nrelease-tags\n");
    fixture.seed(
        "ruleset_master-protection",
        r#"{
  "name": "master-protection",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": { "include": ["refs/heads/other"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": { "required_approving_review_count": 0, "allowed_merge_methods": ["squash"] }
    },
    {
      "type": "required_status_checks",
      "parameters": { "required_status_checks": [{ "context": "test" }] }
    }
  ]
}"#,
    );
    fixture.seed(
        "ruleset_release-tags",
        r#"{
  "name": "release-tags",
  "target": "tag",
  "enforcement": "active",
  "conditions": {
    "ref_name": { "include": ["refs/tags/v*"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "update" }
  ]
}"#,
    );
    fixture
        .rk(&["setup", "step", "protections-check", "--apply"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("the observation disagrees"));
}

/// An unreadable ruleset inventory stays unknown, never drift: nothing was
/// proven wrong, so the check refuses to claim either state.
#[test]
fn check_reports_an_unreadable_ruleset_inventory_as_unknown() {
    let fixture = ForgeFixture::new();
    fixture.replace_gh(
        "#!/bin/sh\ncase \"$*\" in\n*rulesets*) echo 'gh: Internal Server Error (HTTP 500)' >&2; exit 1;;\nesac\ncase \"$1\" in\nrepo) printf 'master\\n';;\napi)\n  case \"$*\" in\n  *'repos/acme/widget'*) printf '{\"id\":1,\"default_branch\":\"master\"}\\n';;\n  *) printf '{}\\n';;\n  esac;;\nesac\nexit 0\n",
    );
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("unknown protect-trunk"),
        "an unreadable inventory must stay unknown: {text}"
    );
    assert!(
        text.contains("unknown protections-check"),
        "the aggregate must not convert an outage into drift: {text}"
    );
}

/// A 404 on the ruleset inventory is an unreachable inventory, not an
/// empty one: nothing reads as absent, and nothing reads as drift.
#[test]
fn check_reports_a_missing_ruleset_inventory_as_unknown() {
    let fixture = ForgeFixture::new();
    fixture.replace_gh(
        "#!/bin/sh\ncase \"$*\" in\n*rulesets*) echo 'gh: Not Found (HTTP 404)' >&2; exit 1;;\nesac\ncase \"$1\" in\nrepo) printf 'master\\n';;\napi)\n  case \"$*\" in\n  *'repos/acme/widget'*) printf '{\"id\":1,\"default_branch\":\"master\"}\\n';;\n  *) printf '{}\\n';;\n  esac;;\nesac\nexit 0\n",
    );
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("unknown protect-trunk"),
        "a 404 inventory must stay unknown, not read as absent: {text}"
    );
    assert!(
        text.contains("unknown protections-check"),
        "the aggregate must not convert a 404 inventory into drift: {text}"
    );
}

/// single-trunk refuses when a candidate is not an ancestor of the trunk,
/// and every candidate survives the refusal: the guard is all-or-nothing.
#[test]
fn single_trunk_refuses_a_non_ancestor() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");
    fixture.seed("branch_master", "abc123");
    fixture.seed("compare_main_master", "diverged");
    fixture
        .rk(&["setup", "step", "single-trunk"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("lose work"));
    assert!(
        fixture.state("branch_main").is_file(),
        "the branch must survive the refusal"
    );
    assert!(
        fixture.state("branch_develop").is_file(),
        "no other candidate may fall to a refused run"
    );
}

/// A GitLab step runs against the gitlab tree with the same lifecycle.
#[test]
fn a_gitlab_step_applies_through_the_gitlab_tree() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup", "step", "default-branch"])
        .args(["--repo", "acme/widget", "--forge", "gitlab", "--apply"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_to_string(fixture.state("default_branch")).expect("state reads"),
        "master\n"
    );
    assert!(fixture.log().contains("api -X PUT projects/acme%2Fwidget"));
}

/// A GitLab credential travels on stdin, never in a process argument list.
#[test]
fn gitlab_bot_secrets_takes_the_value_on_stdin() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup", "step", "bot-secrets"])
        .args(["--repo", "acme/widget", "--forge", "gitlab", "--apply"])
        .env("RK_BOT_TOKEN", "glpat-sekret-value")
        .assert()
        .success();
    assert!(fixture.stdin_log().contains("glpat-sekret-value"));
    assert!(
        !fixture.log().contains("glpat-sekret-value"),
        "a secret reached a process argument list"
    );
}

/// The key's contents in the environment are refused wherever a run opens,
/// not only in the step that would have stored them, and the refusal names
/// the variable that replaces it.
#[test]
fn the_key_in_the_environment_is_refused() {
    let fixture = ForgeFixture::new();
    for action in [
        vec!["setup", "check"],
        vec!["setup", "step", "bot-secrets"],
        vec!["setup"],
    ] {
        fixture
            .rk(&action)
            .args(["--repo", "acme/widget", "--forge", "github"])
            .env("RK_BOT_APP_ID", "314159")
            .env("RK_BOT_PRIVATE_KEY", FAKE_PEM)
            .assert()
            .failure()
            .stderr(predicate::str::contains(
                "RK_BOT_PRIVATE_KEY carries key material",
            ))
            .stderr(predicate::str::contains("RK_BOT_PRIVATE_KEY_FILE"));
    }
    assert!(
        !fixture.log().contains("secret set"),
        "a refused run reached the forge"
    );
}

/// Every wrong key file is refused before the step spawns, so nothing
/// reaches the forge and the operator is told which fact was wrong.
#[test]
fn a_wrong_key_file_is_refused_before_the_forge_is_called() {
    let fixture = ForgeFixture::new();
    let missing = fixture.home.path().join("absent.pem");
    let world_readable = fixture.key_file_with("loose.pem", FAKE_PEM, 0o644);
    let not_a_key = fixture.key_file_with("id.txt", "314159\n", 0o600);
    let empty = fixture.key_file_with("empty.pem", "", 0o600);
    let in_tree = fixture.target.path().join("bot.pem");
    std::fs::write(&in_tree, FAKE_PEM).expect("the in-tree key writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&in_tree, std::fs::Permissions::from_mode(0o600))
            .expect("the in-tree key takes its mode");
    }

    let cases: [(PathBuf, &str); 6] = [
        (missing, "unreadable"),
        (world_readable, "readable by group or other"),
        (not_a_key, "is not a PEM-encoded private key"),
        (empty, "is empty"),
        (in_tree, "inside the repository being set up"),
        (PathBuf::from("~/bot.pem"), "unexpanded tilde"),
    ];
    for (path, expected) in cases {
        // Preview rehearses apply, so both refuse the same file.
        for extra in [vec![], vec!["--apply"]] {
            fixture
                .rk(&["setup", "step", "bot-secrets"])
                .args(["--repo", "acme/widget", "--forge", "github"])
                .args(&extra)
                .env("RK_BOT_APP_ID", "314159")
                .env("RK_BOT_PRIVATE_KEY_FILE", &path)
                .assert()
                .failure()
                .stderr(predicate::str::contains(expected));
        }
    }
    assert!(
        !fixture.log().contains("secret set"),
        "a refused run reached the forge"
    );
}

/// A FIFO is refused by kind rather than waited on. rk opens the named path
/// before it knows what it is, so the open must not block on a pipe with no
/// writer; the timeout is what makes the difference visible.
#[cfg(unix)]
#[test]
fn a_key_path_that_is_a_pipe_refuses_rather_than_blocking() {
    let fixture = ForgeFixture::new();
    let fifo = fixture.home.path().join("bot.fifo");
    let made = std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "the fifo was created");
    fixture
        .rk(&["setup", "step", "bot-secrets"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &fifo)
        .timeout(std::time::Duration::from_secs(20))
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a regular file"));
}

/// A directory is refused by kind: `rk` reads the named file, and a source
/// that yields its bytes once is not a key file.
#[test]
fn a_key_path_that_is_not_a_regular_file_is_refused() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup", "step", "bot-secrets"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", fixture.home.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("is not a regular file"));
}

/// What rk validated is what rk sends. The key file is replaced with other
/// material after the run resolves it and before gh could reopen it — which
/// gh cannot do, because it is never told the path — so the stored secret
/// is the validated one and the replacement reaches nothing.
#[test]
fn the_validated_bytes_are_the_transmitted_bytes() {
    let fixture = ForgeFixture::new();
    let key = fixture.key_file();
    // The mock replaces the key file the moment it is first called, which
    // is after rk read it. A run that reopened the path would store this.
    fixture.replace_gh(
        &MOCK_GH
            .replace("__STATE__", &fixture.mock.path().to_string_lossy())
            .replace(
                "env >> \"$STATE/env-log\"",
                &format!(
                    "env >> \"$STATE/env-log\"\nprintf '%s' 'swapped-after-validation' > {}",
                    key.display()
                ),
            ),
    );
    fixture
        .rk(&["setup", "step", "bot-secrets"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .env("RK_BOT_PRIVATE_KEY_FILE", &key)
        .assert()
        .success();
    assert!(
        fixture.stdin_log().contains(PEM_NEEDLE),
        "the validated bytes were not the ones stored"
    );
    assert!(
        !fixture.stdin_log().contains("swapped-after-validation"),
        "a replacement made after validation reached the forge"
    );
}

/// GitLab stores a token and reads no key file, so a stale key variable in
/// the environment is not its business and must not fail its step.
#[test]
fn gitlab_ignores_a_broken_key_file_variable() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup", "step", "bot-secrets"])
        .args(["--repo", "acme/widget", "--forge", "gitlab", "--apply"])
        .env("RK_BOT_TOKEN", "glpat-sekret-value")
        .env("RK_BOT_PRIVATE_KEY_FILE", "/nonexistent/stale.pem")
        .assert()
        .success();
    assert!(fixture.stdin_log().contains("glpat-sekret-value"));
}

/// Half an App identity stores nothing: the App ID without the key refuses
/// naming both variables.
#[test]
fn half_an_app_identity_refuses() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup", "step", "bot-secrets"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .env("RK_BOT_APP_ID", "314159")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "bot-secrets has no credentials to store",
        ))
        .stderr(predicate::str::contains("RK_BOT_PRIVATE_KEY_FILE"));
}

/// Detection selects the tree from the remote host, an unknown host refuses
/// naming the overrides, and a self-hosted GitLab warns about trusted
/// publishing at the start rather than at the registry step.
#[test]
fn detection_selects_the_tree_and_refuses_an_unknown_host() {
    let fixture = ForgeFixture::new();
    let git = |args: &[&str]| git_in(fixture.target.path(), args);
    git(&["init", "-q"]);
    git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/acme/widget.git",
    ]);
    fixture.rk(&["setup"]).assert().success().stdout(
        predicate::str::contains("setup/github").and(predicate::str::contains("acme/widget")),
    );

    git(&[
        "remote",
        "set-url",
        "origin",
        "https://gitlab.com/acme/widget.git",
    ]);
    fixture
        .rk(&["setup"])
        .assert()
        .success()
        .stdout(predicate::str::contains("setup/gitlab"));

    git(&[
        "remote",
        "set-url",
        "origin",
        "https://gitlab.example.com/acme/widget.git",
    ]);
    fixture
        .rk(&["setup"])
        .assert()
        .success()
        .stderr(predicate::str::contains("GitLab.com only"));

    git(&[
        "remote",
        "set-url",
        "origin",
        "https://code.example.com/acme/widget.git",
    ]);
    fixture
        .rk(&["setup"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("--forge").and(predicate::str::contains("--repo")));
}

/// An apply that cannot create its journal refuses unrun; a preview in the
/// same position warns and completes.
#[cfg(unix)]
#[test]
fn a_read_only_state_root_refuses_apply_and_warns_preview() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = ForgeFixture::new();
    let sealed = fixture.home.path().join("sealed");
    std::fs::create_dir(&sealed).expect("the sealed dir creates");
    std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o555))
        .expect("the dir seals");
    let run = |apply: bool| {
        let mut command = fixture.rk(&["setup"]);
        command
            .args(["--repo", "acme/widget", "--forge", "github"])
            .env("XDG_STATE_HOME", &sealed);
        if apply {
            command.args(["--apply", "--required-check", "test-check"]);
        }
        command
    };
    run(true)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("journal"));
    assert_eq!(
        fixture.log(),
        "",
        "a refused apply must not touch the forge"
    );
    run(false)
        .assert()
        .success()
        .stderr(predicate::str::contains("no run journal"));
    std::fs::set_permissions(&sealed, std::fs::Permissions::from_mode(0o755))
        .expect("the dir unseals");
}

/// An absent shell fails before any step runs.
#[test]
fn an_absent_sh_refuses_before_any_step() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .args(["--apply", "--required-check", "test-check"])
        .env("PATH", "/no/such/dir")
        .assert()
        .code(73)
        .stderr(predicate::str::contains("sh"));
    assert_eq!(fixture.log(), "", "nothing may run without a shell");
}

/// Human mode keeps the child's two streams apart: what the script
/// writes to stdout reaches this process's stdout, what it writes to
/// stderr reaches stderr, and neither crosses. Folding them corrupts
/// `rk setup … > file` for every caller downstream. The step itself
/// fails to verify here — the mock forge never records the edit — which
/// is beside the point: the routing is asserted, not the outcome.
#[test]
fn human_mode_passes_child_streams_through_uncrossed() {
    let fixture = ForgeFixture::new();
    fixture.replace_gh(
        "#!/bin/sh\nprintf 'CHILD-ERR\\n' >&2\ncase \"$1\" in\napi) printf '{\"default_branch\":\"main\"}\\n';;\nrepo) [ \"$2\" = view ] && printf 'develop\\n';;\nesac\nexit 0\n",
    );
    let out = fixture
        .rk(&["setup", "step", "default-branch", "--apply"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .get_output()
        .clone();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("check: prints"),
        "the child's stdout never reached stdout: {stdout}"
    );
    assert!(
        stderr.contains("CHILD-ERR"),
        "the child's stderr never reached stderr: {stderr}"
    );
    assert!(
        !stdout.contains("CHILD-ERR"),
        "the child's stderr crossed into stdout: {stdout}"
    );
    assert!(
        !stderr.contains("check: prints"),
        "the child's stdout crossed into stderr: {stderr}"
    );
}

/// A child killed by a signal names the signal rather than blaming the
/// forge for output it never got to write, and the journal closes with a
/// terminal status rather than with nothing: a run that ended on a
/// signal must be distinguishable afterwards from one still in flight,
/// which is exactly what the mutating-run guard reads.
#[test]
fn a_signalled_child_names_the_signal_and_closes_the_journal() {
    let fixture = ForgeFixture::new();
    // Only the apply-phase call signals, and it signals its own parent —
    // the shell running the step — so observation stays intact and
    // nothing outside this run is touched.
    fixture.replace_gh(
        "#!/bin/sh\ncase \"$1\" in\napi) printf '{\"default_branch\":\"main\"}\\n'; exit 0;;\nrepo) if [ \"$2\" = edit ]; then kill -TERM $PPID; sleep 2; fi;;\nesac\nexit 0\n",
    );
    fixture
        .rk(&["setup", "step", "default-branch", "--apply"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("killed by signal 15"));

    let meta: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture.latest_run().join("meta.json")).expect("meta reads"),
    )
    .expect("meta parses");
    assert_eq!(meta["schema"], "rk.run-meta/1");
    assert_eq!(meta["reason"], "subprocess-failed");
    assert!(
        meta["exit_code"].as_i64().is_some_and(|code| code != 0),
        "the journal must close with a terminal status, not look in flight: {meta}"
    );
    assert!(
        !meta["ended"].is_null(),
        "a closed run records when it ended: {meta}"
    );
}

/// Two runs at once keep their materialized scripts apart. The scripts
/// are payload written to disk to be executed, so a shared path is a
/// window in which one run rewrites the bytes another is about to run;
/// each run materializes under its own journal directory instead.
#[test]
fn concurrent_runs_do_not_share_a_materialized_script() {
    let fixture = ForgeFixture::new();
    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(|| {
                fixture
                    .rk(&["setup", "step", "ci-permissions", "--apply"])
                    .args(["--repo", "acme/widget", "--forge", "github"])
                    .assert()
                    .success();
            });
        }
    });

    let runs: Vec<PathBuf> = std::fs::read_dir(fixture.runs_root())
        .expect("the runs root reads")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(
        runs.len(),
        2,
        "two concurrent runs must claim two journal directories, not one"
    );
    for run in &runs {
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(run.join("meta.json")).expect("meta reads"))
                .expect("meta parses");
        assert_eq!(meta["exit_code"], 0);
        assert!(
            meta["scripts"]
                .as_array()
                .is_some_and(|list| !list.is_empty()),
            "each run records the digest of what it materialized: {meta}"
        );
    }
}

/// The journal keeps a bounded number of runs, and the runs verbs read it.
#[test]
fn the_journal_retention_bound_holds_and_runs_verbs_read_it() {
    let fixture = ForgeFixture::new();
    for _ in 0..23 {
        fixture
            .rk(&["setup"])
            .args(["--repo", "acme/widget", "--forge", "github"])
            .assert()
            .success();
    }
    let kept = std::fs::read_dir(fixture.runs_root())
        .expect("reads")
        .count();
    assert!(kept <= 20, "{kept} runs kept, past the bound");

    let list = fixture
        .rk_bare(&["runs", "list"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&list);
    let first_id = text
        .lines()
        .next()
        .expect("a run lists")
        .split_whitespace()
        .next()
        .expect("an id")
        .to_owned();
    assert!(text.contains("setup"), "{text}");

    fixture
        .rk_bare(&["runs", "show", &first_id])
        .assert()
        .success()
        .stdout(predicate::str::contains("rk.run-meta/1"));
    fixture
        .rk_bare(&["runs", "show", "no-such-run"])
        .assert()
        .code(66);
    fixture
        .rk_bare(&["runs", "prune", "--keep", "1"])
        .assert()
        .success();
    assert_eq!(
        std::fs::read_dir(fixture.runs_root())
            .expect("reads")
            .count(),
        1
    );
}

impl ForgeFixture {
    /// The same environment without the trailing `--target`, for verbs that
    /// take none.
    fn rk_bare(&self, args: &[&str]) -> Command {
        let mut command = rk();
        command
            .env("HOME", self.home.path())
            .env("XDG_STATE_HOME", self.home.path())
            .args(args);
        command
    }
}

/// Where the forge enforces less than the step claims, the check reports
/// the weaker guarantee by name rather than a pass.
#[test]
fn a_gitlab_check_reports_the_tag_protection_limitation() {
    let fixture = ForgeFixture::new();
    fixture.seed("tag_protected", "");
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "gitlab"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("ok protect-tags")
            && text.contains("limitation:")
            && text.contains("Owner or Maintainer"),
        "the check must state what the forge actually enforces: {text}"
    );
}

/// The destructive step fails closed: an ancestry the guard cannot read is
/// treated exactly like one it refuted.
#[test]
fn single_trunk_refuses_an_unreadable_comparison() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");
    fixture.seed("branch_master", "abc123");
    fixture.seed("compare_main_master", "error");
    fixture
        .rk(&["setup", "step", "single-trunk"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("single-trunk refuses"));
    assert!(
        fixture.state("branch_main").is_file(),
        "the branch must survive an unreadable guard"
    );
}

/// A run id is one directory name, never a path.
#[test]
fn runs_show_refuses_a_traversal_shaped_id() {
    let fixture = ForgeFixture::new();
    fixture
        .rk_bare(&["runs", "show", "../../../etc/passwd"])
        .assert()
        .code(66);
}

/// An override must name the binary the scripts invoke by name, or one
/// lifecycle would split across two binaries.
#[test]
fn a_misnamed_forge_cli_override_refuses() {
    let fixture = ForgeFixture::new();
    let wrapper = fixture.mock.path().join("gh-wrapper");
    std::fs::copy(fixture.mock.path().join("gh"), &wrapper).expect("the wrapper copies");
    fixture
        .rk(&["setup"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .env("RK_GH_BIN", &wrapper)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("must name a binary called gh"));
}

/// An observation that cannot decide fails closed before anything mutates.
#[test]
fn an_unreadable_observation_refuses_before_any_mutation() {
    let fixture = ForgeFixture::new();
    fixture.seed("fail_repo", "");
    fixture
        .rk(&["setup", "step", "default-branch"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("cannot observe the current state"));
    assert!(
        !fixture.log().contains("repo edit"),
        "an unreadable observation must never mutate: {}",
        fixture.log()
    );
}

// ---------------------------------------------------------------------------
// The target track: the landing record, status, upgrade, adopt, and the
// canon-side freshness check.
// ---------------------------------------------------------------------------

/// A landing writes the record last, and the record names the payload
/// that actually landed: version, aggregate digest, parameters, kinds,
/// and pins.
#[test]
fn a_landing_writes_the_record_with_its_identity() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path())
        .success()
        .stdout(predicate::str::contains("wrote .release-kit/manifest.json"));
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["schema_version"], 4);
    assert_eq!(manifest["rk_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["origin"], "init");
    assert_eq!(manifest["tech"], "rust");
    assert_eq!(manifest["forge"], "github");
    assert_eq!(manifest["parameters"]["repo"], "acme/widget");
    assert_eq!(
        manifest["parameters"]["scopes"],
        serde_json::json!(["api", "cli"])
    );
    assert_eq!(
        manifest["parameters"]["style"], "trunk",
        "the default style records the armed request"
    );

    let payload = rk()
        .args(["payload", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let payload: serde_json::Value = serde_json::from_slice(&payload).expect("the report parses");
    assert_eq!(
        manifest["payload_sha256"], payload["payload_sha256"],
        "the record must quote the aggregate payload digest"
    );

    assert_eq!(manifest_file(&manifest, "AGENTS.md")["kind"], "rendered");
    let seeded = manifest_file(&manifest, "release-plz.toml");
    assert_eq!(seeded["kind"], "seeded");
    assert!(seeded["baseline_sha256"].is_string());
    assert!(
        manifest["pins"]["release-plz"].is_string() && manifest["pins"]["cargo-dist"].is_string(),
        "the record copies the technology's pins: {manifest}"
    );
}

/// The mechanical sentinel is a parameter: no `OWNER` survives a landing,
/// the owner TODO is gone, and the one judgment sentinel stays in its
/// seeded file.
#[test]
fn substitution_is_total_and_only_judgment_sentinels_remain() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    let out = land_rust(target.path())
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8_lossy(&out);
    assert!(
        !stdout.contains("set the repository owner"),
        "the owner is a parameter, not operator work: {stdout}"
    );
    assert!(
        stdout.contains("release-plz.toml") && stdout.contains("TODO(release-kit)"),
        "the judgment sentinel must still be reported: {stdout}"
    );
    let workflow = std::fs::read_to_string(target.path().join(".github/workflows/release-plz.yml"))
        .expect("the workflow landed");
    assert!(!workflow.contains("OWNER"), "an owner token survived");
    assert!(workflow.contains("== 'acme'"), "{workflow}");
}

/// The routing block splices into a target's own `AGENTS.md` without
/// taking the document over, and lands whole where none exists.
#[test]
fn the_routing_block_splices_and_is_recorded() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("AGENTS.md"),
        "# Widget\n\nHouse rules.\n",
    )
    .expect("the target's own AGENTS.md writes");
    land_rust(target.path()).success();
    let agents = std::fs::read_to_string(target.path().join("AGENTS.md")).expect("AGENTS.md reads");
    assert!(agents.starts_with("# Widget\n\nHouse rules.\n"));
    assert!(agents.contains("<!-- BEGIN release-kit -->"));
    assert!(agents.contains("rk method invariants"));
    assert!(agents.contains("guides and never drives"));
    assert!(
        agents.contains("the scopes this project accepts are `api,cli`"),
        "{agents}"
    );
    assert!(agents.trim_end().ends_with("<!-- END release-kit -->"));
    assert_eq!(
        manifest_file(&read_manifest(target.path()), "AGENTS.md")["kind"],
        "rendered"
    );
}

/// The hook block splices into a target's own `.pre-commit-config.yaml`
/// under its `repos:` key without taking the file over, lands whole with
/// the hook-types key where none exists, and refuses by name a config
/// with no `repos:` line — leaving the target unchanged, record included.
#[test]
fn the_hook_block_splices_and_lands_whole_and_refuses_reposless() {
    // A target with its own hooks: the block lands under repos:, the
    // target's hooks survive, and the top level is untouched.
    let target = tempfile::tempdir().expect("a scratch dir exists");
    let own = "repos:\n  - repo: https://example.com/own\n    rev: v1\n    hooks:\n      - id: own-hook\n";
    std::fs::write(target.path().join(".pre-commit-config.yaml"), own)
        .expect("the target's own config writes");
    land_rust(target.path()).success();
    let config = std::fs::read_to_string(target.path().join(".pre-commit-config.yaml"))
        .expect("the config reads");
    assert!(
        config.starts_with("repos:\n# BEGIN release-kit\n"),
        "{config}"
    );
    assert!(
        config.contains("- id: own-hook"),
        "the target's hooks survive"
    );
    assert!(config.contains("--scopes, 'api,cli'"), "{config}");
    for hook in [
        "conventional-pre-commit",
        "rk-message",
        "no-commit-to-branch",
        "rk-branch-name",
        "rk-no-push-to-trunk",
        "rk-no-hand-authored-tag",
        "rk-status-check",
    ] {
        assert!(config.contains(hook), "the block carries {hook}");
    }
    assert!(
        config.contains("SKIP=no-commit-to-branch"),
        "the block names the CI-sweep skip beside the install command"
    );
    assert!(
        !config.contains("default_install_hook_types"),
        "an existing file's top level belongs to the target"
    );
    assert_eq!(
        manifest_file(&read_manifest(target.path()), ".pre-commit-config.yaml")["kind"],
        "rendered"
    );

    // No config at all: the fresh file carries the hook-types key.
    let fresh = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(fresh.path()).success();
    let config = std::fs::read_to_string(fresh.path().join(".pre-commit-config.yaml"))
        .expect("the fresh config reads");
    assert!(
        config.starts_with("default_install_hook_types: [pre-commit, commit-msg, pre-push]"),
        "{config}"
    );

    // A config with no repos: line refuses before anything lands.
    let reposless = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        reposless.path().join(".pre-commit-config.yaml"),
        "minimum_pre_commit_version: '3.2.0'\n",
    )
    .expect("the reposless config writes");
    land_rust(reposless.path())
        .code(73)
        .stderr(predicate::str::contains("repos:"));
    assert!(
        !reposless.path().join(".release-kit").exists(),
        "a refused landing writes nothing"
    );
    assert!(
        !reposless.path().join("release-plz.toml").exists(),
        "a refused landing writes nothing"
    );
}

/// A duplicated hook block still executes even when its first copy
/// matches the record, so status reads it as rendered drift and an
/// adoption refuses it — the same defect definition every reader shares.
#[test]
fn a_duplicated_hook_block_is_drift_everywhere() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let config_path = target.path().join(".pre-commit-config.yaml");
    let config = std::fs::read_to_string(&config_path).expect("the config reads");
    let begin = config
        .find("# BEGIN release-kit")
        .expect("the block landed");
    let end =
        config.find("# END release-kit").expect("the block closed") + "# END release-kit".len();
    let block = config[begin..end].to_owned();
    std::fs::write(&config_path, format!("{config}\n{block}\n")).expect("the duplicate writes");

    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("DRIFT .pre-commit-config.yaml"));
    rk().args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .code(1);

    // Upgrade sees the same defect as a conflict in preview and refuses
    // the apply, so preview and apply cannot disagree.
    rk().args(["upgrade", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("conflict .pre-commit-config.yaml"));
    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains(".pre-commit-config.yaml"));

    // Adoption lists the defect beside every other unadoptable fact in
    // one run, per its aggregate-verification contract.
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    std::fs::remove_file(target.path().join(".github/workflows/release-plz.yml"))
        .expect("the workflow removes");
    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args(["--repo", "acme/widget", "--target"])
    .arg(target.path())
    .arg("--apply")
    .assert()
    .code(73)
    .stderr(
        predicate::str::contains(".pre-commit-config.yaml")
            .and(predicate::str::contains("release-plz.yml")),
    );
}

/// A record from before the hook block, upgraded over a hook file with
/// no `repos:` line: the defect is a conflict in preview, the apply
/// refuses before anything writes, and the target stays as found.
#[test]
fn a_reposless_hook_file_conflicts_a_legacy_upgrade_before_any_write() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    let files = manifest["files"]
        .as_array()
        .expect("the record lists files")
        .iter()
        .filter(|file| file["destination"] != ".pre-commit-config.yaml")
        .cloned()
        .collect::<Vec<_>>();
    manifest["files"] = files.into();
    write_manifest(target.path(), &manifest);
    std::fs::write(
        target.path().join(".pre-commit-config.yaml"),
        "minimum_pre_commit_version: '3.2.0'\n",
    )
    .expect("the legacy config writes");
    let workflow_path = target.path().join(".github/workflows/release-plz.yml");
    let workflow = std::fs::read(&workflow_path).expect("the workflow reads");

    rk().args(["upgrade", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("conflict .pre-commit-config.yaml"));
    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains(".pre-commit-config.yaml"));
    assert_eq!(
        std::fs::read(&workflow_path).expect("the workflow still reads"),
        workflow,
        "a refused upgrade must write nothing"
    );
    assert!(
        read_manifest(target.path())
            .get("files")
            .and_then(|files| files.as_array())
            .is_some_and(|files| files
                .iter()
                .all(|file| file["destination"] != ".pre-commit-config.yaml")),
        "a refused upgrade must not rewrite the record"
    );
}

/// An apply without `--scopes` refuses naming the flag: the scope
/// vocabulary is a decision, not a default.
#[test]
fn init_apply_without_scopes_refuses_naming_the_flag() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .code(64)
        .stderr(predicate::str::contains("--scopes"));
    assert!(
        !target.path().join(".release-kit").exists(),
        "a refused landing writes nothing"
    );
}

/// A bare directory has no landing: one branchable field at exit 0, and
/// only `--check` turns that into a judgment.
#[test]
fn status_reports_no_landing_and_only_check_judges_it() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("no landing"));
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["landed"], false);
    assert!(report.get("tech").is_none(), "{report}");
    rk().args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .code(1);
}

/// Status is one object an agent can consume, and a fresh landing is
/// aligned with zero drift.
#[test]
fn status_json_is_one_object_over_a_fresh_landing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.status/6");
    assert_eq!(report["landed"], true);
    assert_eq!(report["tech"], "rust");
    assert_eq!(report["style"], "trunk");
    assert_eq!(report["forge"], "github");
    assert_eq!(report["alignment"], "aligned");
    assert_eq!(report["drift"]["rendered"], 0);
    assert_eq!(report["drift"]["seeded"], 0);
    assert_eq!(
        report["sentinels"], 1,
        "the seeded judgment sentinel reports: {report}"
    );
}

/// D17: `--check` computes the identical report and changes only the
/// judgment — an unresolved sentinel or rendered drift exits 1, seeded
/// drift alone exits 0, and the report bytes match the plain run's.
#[test]
fn status_check_judges_the_identical_report() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();

    let plain = rk()
        .args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    // The landed seeded file still carries its judgment sentinel, which
    // is a violation under --check and only there.
    let checked = rk()
        .args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert_eq!(plain, checked, "the report must not change under --check");

    // Fill the sentinel: seeded drift, informational in both modes.
    let seeded = target.path().join("release-plz.toml");
    let filled = std::fs::read_to_string(&seeded)
        .expect("the seeded file reads")
        .lines()
        .filter(|line| !line.contains("TODO(release-kit)"))
        .fold(String::new(), |mut text, line| {
            text.push_str(line);
            text.push('\n');
            text
        });
    std::fs::write(&seeded, filled).expect("the fill writes");
    rk().args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "DRIFT release-plz.toml (seeded, target-owned)",
        ));

    // Edit a rendered file: the target touched what release-kit owns.
    let workflow = target.path().join(".github/workflows/release-plz.yml");
    let mut text = std::fs::read_to_string(&workflow).expect("the workflow reads");
    text.push_str("# a local edit\n");
    std::fs::write(&workflow, text).expect("the edit writes");
    let out = rk()
        .args(["status", "--check", "--json", "--target"])
        .arg(target.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["drift"]["rendered"], 1);
    assert!(
        report["violations"]
            .as_array()
            .is_some_and(|violations| violations
                .iter()
                .any(|violation| violation.as_str().unwrap_or("").contains("rendered drift"))),
        "{report}"
    );
}

/// The pin comparison is offline: a doctored record reports STALE against
/// the binary's embedded registry with no network in reach.
#[test]
fn status_reports_a_stale_pin_offline() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    manifest["pins"]["release-plz"] = serde_json::Value::from("0.0.1");
    write_manifest(target.path(), &manifest);
    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("STALE release-plz 0.0.1 landed"));

    // Stale means behind: a pin ahead of this binary's registry — a
    // landing from a newer rk — is not a freshness complaint.
    manifest["pins"]["release-plz"] = serde_json::Value::from("999.0.0");
    write_manifest(target.path(), &manifest);
    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("STALE").not());
}

/// A rendered-to-seeded reclassification is safe and silent: an untouched
/// file — matching what release-kit last wrote — is not drift, and the
/// rewritten record carries those bytes as the seeded baseline.
#[test]
fn a_rendered_to_seeded_reclassification_is_silent_when_untouched() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    // Stand in for an older payload that classified the seeded file as
    // rendered: only the recorded kind differs from this binary's table.
    let mut manifest = read_manifest(target.path());
    for file in manifest["files"].as_array_mut().expect("files") {
        if file["destination"] == "release-plz.toml" {
            file["kind"] = serde_json::Value::from("rendered");
        }
    }
    write_manifest(target.path(), &manifest);

    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("unchanged release-plz.toml")
                .and(predicate::str::contains("drift release-plz.toml").not()),
        );
    let upgraded = read_manifest(target.path());
    let seeded = manifest_file(&upgraded, "release-plz.toml");
    assert_eq!(seeded["kind"], "seeded");
    assert_eq!(
        seeded["baseline_sha256"], seeded["sha256"],
        "the last-written bytes become the seeded baseline"
    );
}

/// The record's failure taxonomy: unparsable at a known schema is a
/// defect-class failure, an unknown schema refuses naming the record.
#[test]
fn a_broken_or_alien_record_fails_by_its_taxonomy() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::create_dir(target.path().join(".release-kit")).expect("the record dir creates");
    let record = target.path().join(".release-kit/manifest.json");

    std::fs::write(&record, "not json").expect("the garbage writes");
    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .code(70);

    std::fs::write(&record, r#"{"schema_version": 999}"#).expect("the alien record writes");
    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains("manifest.json").and(predicate::str::contains("999")));
    rk().args(["upgrade", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains("999"));
}

/// The round trip G5 names: land, tune the seeded file, upgrade — the
/// tune survives, and the record moves to this binary's version with the
/// landing instant preserved.
#[test]
fn an_upgrade_keeps_a_seeded_edit_and_moves_the_record() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let seeded = target.path().join("release-plz.toml");
    std::fs::write(&seeded, "semver_check = true\n").expect("the tune writes");

    // Stand in for an older landing: only the recorded version differs.
    let mut manifest = read_manifest(target.path());
    let landed_at = manifest["landed_at"].clone();
    manifest["rk_version"] = serde_json::Value::from("0.0.1");
    write_manifest(target.path(), &manifest);

    rk().args(["upgrade", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "drift release-plz.toml (seeded, target-owned)",
        ));
    assert_eq!(
        read_manifest(target.path())["rk_version"],
        "0.0.1",
        "a preview must not rewrite the record"
    );

    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "rewrote .release-kit/manifest.json",
        ));
    assert_eq!(
        std::fs::read_to_string(&seeded).expect("the seeded file survives"),
        "semver_check = true\n",
        "an upgrade must never rewrite a seeded file"
    );
    let upgraded = read_manifest(target.path());
    assert_eq!(upgraded["rk_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        upgraded["landed_at"], landed_at,
        "the landing instant is preserved"
    );
    assert_eq!(
        manifest_file(&upgraded, "release-plz.toml")["sha256"]
            .as_str()
            .expect("a digest"),
        Digest::of(b"semver_check = true\n").to_string(),
        "the record follows the target's seeded bytes"
    );
}

/// The three-digest comparison at work: a rendered file whose disk bytes
/// match what the record says was written is nobody's edit, and a newer
/// payload rewrites it.
#[test]
fn an_upgrade_rewrites_an_untouched_stale_rendered_file() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();

    // Stand in for an older payload's rendering: the file and its record
    // agree with each other and differ from this binary's candidate.
    let workflow = target.path().join(".github/workflows/release-plz.yml");
    let old = b"# an older release's workflow\n";
    std::fs::write(&workflow, old).expect("the old bytes write");
    let mut manifest = read_manifest(target.path());
    let digest = serde_json::Value::from(Digest::of(old).to_string());
    for file in manifest["files"].as_array_mut().expect("files") {
        if file["destination"] == ".github/workflows/release-plz.yml" {
            file["sha256"] = digest.clone();
            file["baseline_sha256"] = digest.clone();
        }
    }
    write_manifest(target.path(), &manifest);

    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "updated .github/workflows/release-plz.yml",
        ));
    let text = std::fs::read_to_string(&workflow).expect("the workflow reads");
    assert!(
        text.contains("== 'acme'"),
        "the candidate must land rendered under the recorded parameters"
    );
}

/// Owned drift refuses, every conflict collected in one run, and nothing
/// is written.
#[test]
fn an_upgrade_refuses_owned_drift_listing_every_conflict() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let workflow = target.path().join(".github/workflows/release-plz.yml");
    let mut text = std::fs::read_to_string(&workflow).expect("the workflow reads");
    text.push_str("# a local edit\n");
    std::fs::write(&workflow, &text).expect("the edit writes");
    let agents = target.path().join("AGENTS.md");
    let block = std::fs::read_to_string(&agents)
        .expect("AGENTS.md reads")
        .replace("Never author a tag", "Feel free to author tags");
    std::fs::write(&agents, &block).expect("the block edit writes");
    let record_before =
        std::fs::read(target.path().join(".release-kit/manifest.json")).expect("the record reads");

    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(
            predicate::str::contains(".github/workflows/release-plz.yml")
                .and(predicate::str::contains("AGENTS.md"))
                .and(predicate::str::contains("nothing was written")),
        );
    assert_eq!(
        std::fs::read_to_string(&workflow).expect("the workflow survives"),
        text,
        "a refused upgrade must not touch the edited file"
    );
    assert_eq!(
        std::fs::read(target.path().join(".release-kit/manifest.json"))
            .expect("the record survives"),
        record_before,
        "a refused upgrade must not rewrite the record"
    );
}

/// The upgrade refusals that precede any comparison: no record, and a
/// record from a newer binary.
#[test]
fn an_upgrade_refuses_without_a_record_and_never_downgrades() {
    let bare = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["upgrade", "--target"])
        .arg(bare.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains("rk init").and(predicate::str::contains("rk adopt")));

    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    manifest["rk_version"] = serde_json::Value::from("999.0.0");
    write_manifest(target.path(), &manifest);
    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains("downgrading"));
}

/// A file the payload stops shipping is the target's from that moment:
/// left in place, named, and gone from the record.
#[test]
fn a_dropped_file_stays_and_leaves_the_record() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let legacy = target.path().join("legacy.yml");
    std::fs::write(&legacy, "an older payload shipped this\n").expect("the legacy file writes");
    let mut manifest = read_manifest(target.path());
    manifest["files"]
        .as_array_mut()
        .expect("files")
        .push(serde_json::json!({
            "destination": "legacy.yml",
            "kind": "rendered",
            "sha256": Digest::of(b"an older payload shipped this\n").to_string(),
            "baseline_sha256": Digest::of(b"an older payload shipped this\n").to_string(),
        }));
    write_manifest(target.path(), &manifest);

    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("dropped legacy.yml"));
    assert!(
        legacy.is_file(),
        "deleting a workflow on a consumer's behalf is not a thing an upgrade does"
    );
    assert!(
        read_manifest(target.path())["files"]
            .as_array()
            .expect("files")
            .iter()
            .all(|file| file["destination"] != "legacy.yml"),
        "the record must stop carrying a dropped file"
    );
}

/// A matching target adopts: the manifest appears with its origin, and
/// nothing else changes — held by digesting the tree before and after.
#[test]
fn a_matching_target_adopts_writing_only_the_manifest() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    let before = tree_digests(target.path());

    let adopt = |apply: bool| {
        let mut cmd = rk();
        cmd.args([
            "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style",
            "trunk",
        ])
        .args([
            "--workflow",
            "worktree",
            "--repo",
            "acme/widget",
            "--target",
        ])
        .arg(target.path());
        if apply {
            cmd.arg("--apply");
        }
        cmd.assert()
    };
    // Preview verifies and writes nothing, the manifest included.
    adopt(false).success();
    assert!(!target.path().join(".release-kit").exists());

    adopt(true)
        .success()
        .stdout(predicate::str::contains("wrote .release-kit/manifest.json"));
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["origin"], "adopt");
    assert_eq!(manifest["parameters"]["repo"], "acme/widget");
    assert_eq!(
        manifest["parameters"]["scopes"],
        serde_json::json!(["api", "cli"])
    );
    assert_eq!(
        tree_digests(target.path())
            .into_iter()
            .filter(|(path, _)| !path.starts_with(".release-kit"))
            .collect::<Vec<_>>(),
        before,
        "an adoption changes no target file"
    );

    // After a successful adopt, status is clean and upgrade runs.
    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success();
    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success();
}

/// One edited rendered file refuses the whole adoption, listing every
/// mismatch in one run, and nothing is written.
#[test]
fn an_edited_rendered_file_refuses_adoption_listing_every_mismatch() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    let workflow = target.path().join(".github/workflows/release-plz.yml");
    let mut text = std::fs::read_to_string(&workflow).expect("the workflow reads");
    text.push_str("# drifted\n");
    std::fs::write(&workflow, text).expect("the edit writes");
    let agents = target.path().join("AGENTS.md");
    let block = std::fs::read_to_string(&agents)
        .expect("AGENTS.md reads")
        .replace("Never author a tag", "Do author tags");
    std::fs::write(&agents, block).expect("the block edit writes");

    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args(["--repo", "acme/widget", "--target"])
    .arg(target.path())
    .arg("--apply")
    .assert()
    .code(73)
    .stderr(
        predicate::str::contains("release-plz.yml")
            .and(predicate::str::contains("AGENTS.md"))
            .and(predicate::str::contains("no record was written")),
    );
    assert!(!target.path().join(".release-kit").exists());
}

/// A differing seeded file is what seeded means: the adoption records
/// both digests so a later upgrade has a real baseline.
#[test]
fn a_differing_seeded_file_adopts_with_both_digests() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    std::fs::write(
        target.path().join("release-plz.toml"),
        "semver_check = true\n",
    )
    .expect("the tune writes");

    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args([
        "--workflow",
        "worktree",
        "--repo",
        "acme/widget",
        "--target",
    ])
    .arg(target.path())
    .arg("--apply")
    .assert()
    .success()
    .stdout(predicate::str::contains(
        "differs release-plz.toml (seeded, target-owned)",
    ));
    let seeded = read_manifest(target.path());
    let seeded = manifest_file(&seeded, "release-plz.toml");
    assert_eq!(
        seeded["sha256"].as_str().expect("a digest"),
        Digest::of(b"semver_check = true\n").to_string()
    );
    assert_ne!(
        seeded["sha256"], seeded["baseline_sha256"],
        "the candidate's baseline must be recorded beside the target's bytes"
    );
}

/// An expected file that is missing refuses the adoption; the record may
/// not claim more than the disk holds.
#[test]
fn a_missing_expected_file_refuses_adoption() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    std::fs::remove_file(target.path().join("dist-workspace.toml")).expect("the file removes");

    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args(["--repo", "acme/widget", "--target"])
    .arg(target.path())
    .arg("--apply")
    .assert()
    .code(73)
    .stderr(predicate::str::contains("dist-workspace.toml"));
    assert!(!target.path().join(".release-kit").exists());
}

/// An `AGENTS.md` that exists without the marked block is not a missing
/// file; the refusal names the absent block, because the remedy —
/// splicing the block — differs from restoring a deleted file.
#[test]
fn an_agents_file_without_the_block_refuses_naming_the_block() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    std::fs::write(
        target.path().join("AGENTS.md"),
        "# The target's own orientation\n",
    )
    .expect("the rewrite writes");

    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args(["--repo", "acme/widget", "--target"])
    .arg(target.path())
    .arg("--apply")
    .assert()
    .code(73)
    .stderr(predicate::str::contains(
        "AGENTS.md (carries no release-kit block)",
    ));
    assert!(!target.path().join(".release-kit").exists());
}

/// A target that already has a record needs no adoption.
#[test]
fn an_existing_record_refuses_adoption_naming_upgrade() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args(["--repo", "acme/widget", "--target"])
    .arg(target.path())
    .arg("--apply")
    .assert()
    .code(73)
    .stderr(predicate::str::contains("rk upgrade"));
}

/// Every file's digest under a directory, for asserting a tree unchanged.
fn tree_digests(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).expect("the tree reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let bytes = std::fs::read(&path).expect("a file reads");
                let rel = path
                    .strip_prefix(root)
                    .expect("under the root")
                    .to_string_lossy()
                    .into_owned();
                out.push((rel, Digest::of(&bytes).to_string()));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// `rk versions --check` against a mocked fetch: all four per-pin results
/// render, an unreachable source is a reported result at exit 0, and the
/// registry file is untouched.
#[test]
fn versions_check_reports_each_pin_and_mutates_nothing() {
    let mock = tempfile::tempdir().expect("a scratch dir exists");
    let curl = mock.path().join("curl");
    std::fs::write(
        &curl,
        r#"#!/usr/bin/env bash
url="${@: -1}"
case "$url" in
  */commits/*) case "$url" in
    *release-plz/action*) printf '%s' '{"sha":"2eb1d8bcb770b4c48ccfaad919734b38b51958c9"}';;
    *create-github-app-token*) printf '%s' '{"sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}';;
    *rust-toolchain*) printf '%s' '{"sha":"4360b52568e2003a75bf9bc1d59f33a8e3fc893c"}';;
    *actions/checkout*) exit 22;;
    *) printf '%s' 'not json at all';;
  esac;;
  *crates.io*) printf '%s' '{"crate":{"max_stable_version":"0.3.160"}}';;
  *cargo-dist*) printf '%s' '{"tag_name":"v999.0.0"}';;
  *git-cliff*) printf '%s' 'not json at all';;
  *actions/checkout*) exit 22;;
  *) printf '%s' '{"tag_name":"v1.14.2"}';;
esac
"#,
    )
    .expect("the mock writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755))
            .expect("the mock is executable");
    }
    let registry = repo_path("versions.toml");
    let before = std::fs::read(&registry).expect("the registry reads");

    let out = rk()
        .args(["versions", "--check"])
        .env("RK_CURL_BIN", &curl)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("current release-plz"), "{text}");
    assert!(text.contains("update-available cargo-dist"), "{text}");
    assert!(text.contains("source-unparsable git-cliff"), "{text}");
    assert!(text.contains("source-unreachable checkout"), "{text}");
    assert_eq!(
        std::fs::read(&registry).expect("the registry still reads"),
        before,
        "the command that notices staleness must not resolve it"
    );

    let out = rk()
        .args(["versions", "--check", "--json"])
        .env("RK_CURL_BIN", &curl)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.versions-check/2");
    assert!(
        report["pins"]
            .as_array()
            .is_some_and(|pins| !pins.is_empty()),
        "{report}"
    );
    // The whole ref vocabulary, behaviorally: a matching sha is
    // ref-unmoved, a different sha is ref-moved with the commit named, a
    // failed fetch is ref-unreachable, a body with no sha is
    // ref-unparsable, and a pin with no version source still reports —
    // every one a result at exit 0, never an error.
    let pin_named = |tool: &str| {
        report["pins"]
            .as_array()
            .and_then(|pins| pins.iter().find(|pin| pin["tool"] == tool).cloned())
            .unwrap_or_else(|| panic!("the {tool} pin reports"))
    };
    let unmoved = pin_named("release-plz");
    assert_eq!(unmoved["ref_class"], "moving-minor-tag", "{unmoved}");
    assert_eq!(unmoved["ref_result"], "ref-unmoved", "{unmoved}");
    assert!(
        unmoved["commit"]
            .as_str()
            .is_some_and(|commit| commit.len() == 40),
        "{unmoved}"
    );
    let moved = pin_named("create-github-app-token");
    assert_eq!(moved["ref_result"], "ref-moved", "{moved}");
    assert_eq!(
        moved["ref_commit"], "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "{moved}"
    );
    assert_eq!(
        pin_named("checkout")["ref_result"],
        "ref-unreachable",
        "a failed ref fetch is a reported result"
    );
    assert_eq!(
        pin_named("attest")["ref_result"],
        "ref-unparsable",
        "a body with no sha is a reported result"
    );
    let ref_only = pin_named("rust-toolchain");
    assert_eq!(ref_only["result"], "no-version-source", "{ref_only}");
    assert_eq!(ref_only["ref_result"], "ref-unmoved", "{ref_only}");
    assert!(
        pin_named("git-cliff")["ref_result"].is_null(),
        "a non-action pin resolves no ref"
    );
}

// ---------------------------------------------------------------------------
// rk branches prune and the post-merge reminder step

/// The variables a running git hook exports; a scratch-repo test under
/// pre-commit inherits them and must not let them retarget its git.
const GIT_HOOK_VARS: [&str; 4] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
];

/// Run git in a scratch repository, asserting success.
fn git_in(dir: &Path, args: &[&str]) {
    let mut command = std::process::Command::new("git");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let status = command
        .args(args)
        .current_dir(dir)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?} failed");
}

/// A scratch repository: one commit on master, a github origin.
fn branch_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch repo exists");
    git_in(dir.path(), &["init", "-q", "-b", "master"]);
    git_in(dir.path(), &["config", "user.email", "rk@example.invalid"]);
    git_in(dir.path(), &["config", "user.name", "rk test"]);
    git_in(
        dir.path(),
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widget.git",
        ],
    );
    std::fs::write(dir.path().join("seed"), "x\n").expect("the seed writes");
    git_in(dir.path(), &["add", "seed"]);
    git_in(dir.path(), &["commit", "-qm", "chore: seed"]);
    dir
}

/// A branch whose configured upstream no longer exists: upstream config
/// with no remote-tracking ref renders `[gone]`.
fn gone_branch(dir: &Path, name: &str) {
    git_in(dir, &["branch", name]);
    git_in(dir, &["config", &format!("branch.{name}.remote"), "origin"]);
    git_in(
        dir,
        &[
            "config",
            &format!("branch.{name}.merge"),
            &format!("refs/heads/{name}"),
        ],
    );
}

/// A gone branch one commit ahead of the trunk, so its tip is its own.
fn advanced_gone_branch(dir: &Path, name: &str) {
    gone_branch(dir, name);
    git_in(dir, &["checkout", "-q", name]);
    std::fs::write(dir.join(format!("{}.txt", name.replace('/', "-"))), "y\n")
        .expect("the extra file writes");
    git_in(dir, &["add", "-A"]);
    git_in(dir, &["commit", "-qm", "chore: advance"]);
    git_in(dir, &["checkout", "-q", "master"]);
}

/// The full object name at a ref.
fn tip_of(dir: &Path, name: &str) -> String {
    let mut command = std::process::Command::new("git");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .args(["rev-parse", name])
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(out.status.success());
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// A mock forge CLI named `gh`, wired through `RK_GH_BIN`.
fn mock_gh(body: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a mock dir exists");
    let path = dir.path().join("gh");
    std::fs::write(&path, body).expect("the mock writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("the mock is executable");
    }
    (dir, path)
}

/// The names `git branch` reports.
fn branch_names(dir: &Path) -> String {
    let mut command = std::process::Command::new("git");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .args(["for-each-ref", "refs/heads", "--format", "%(refname:short)"])
        .current_dir(dir)
        .output()
        .expect("git runs");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// An rk command with the hook variables scrubbed, so the suite behaves
/// the same under a pre-commit run as it does standalone.
fn rk_scrubbed() -> Command {
    let mut command = rk();
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
}

#[test]
fn branches_prune_preview_reports_candidates_and_writes_nothing() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/x");
    git_in(repo.path(), &["branch", "local-only"]);
    let out = rk_scrubbed()
        .args(["branches", "prune", "--target"])
        .arg(repo.path())
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.contains("feat/x"), "{text}");
    assert!(text.contains("candidate"), "{text}");
    assert!(
        !text.contains("local-only"),
        "a branch with no upstream never reaches the report: {text}"
    );
    assert!(text.contains("--verify"), "{text}");
    assert!(text.contains("--apply"), "{text}");
    assert!(
        text.contains("Deleting a branch is the operator's action"),
        "{text}"
    );
    assert!(
        branch_names(repo.path()).contains("feat/x"),
        "a preview deletes nothing"
    );
}

#[test]
fn branches_prune_quiet_prints_nothing_when_clean_and_reports_otherwise() {
    let repo = branch_fixture();
    let clean = rk_scrubbed()
        .args(["branches", "prune", "--quiet", "--target"])
        .arg(repo.path())
        .assert()
        .success();
    assert!(clean.get_output().stdout.is_empty(), "quiet and clean");
    assert!(clean.get_output().stderr.is_empty(), "quiet and clean");
    gone_branch(repo.path(), "feat/x");
    rk_scrubbed()
        .args(["branches", "prune", "--quiet", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("feat/x"));
}

#[test]
fn branches_prune_json_is_one_object_with_schema_and_next() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/x");
    let out = rk_scrubbed()
        .args(["branches", "prune", "--json", "--target"])
        .arg(repo.path())
        .assert()
        .success();
    let report: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("one JSON object");
    assert_eq!(report["schema"], "rk.branches-prune/1");
    assert_eq!(report["mode"], "preview");
    assert_eq!(report["branches"][0]["name"], "feat/x");
    assert_eq!(report["branches"][0]["status"], "candidate");
    assert!(
        report["next"].as_array().is_some_and(|n| !n.is_empty()),
        "the report carries a next block"
    );
}

#[test]
fn branches_prune_json_failure_is_one_diagnostic_line() {
    let output = rk_scrubbed()
        .args(["branches", "prune", "--json", "--target", "/no/such/dir"])
        .assert()
        .code(66)
        .get_output()
        .clone();
    assert!(output.stdout.is_empty(), "no result on a missing target");
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&output.stderr).expect("stderr is one JSON diagnostic");
    assert_eq!(diagnostic["schema"], "rk.diagnostic/1");
    assert_eq!(diagnostic["reason"], "target-not-found");
}

#[test]
fn branches_prune_never_offers_the_current_checked_out_or_protected_branch() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "release/1.0");
    gone_branch(repo.path(), "feat/held");
    let worktree = tempfile::tempdir().expect("a worktree parent exists");
    let wt = worktree.path().join("held");
    git_in(
        repo.path(),
        &[
            "worktree",
            "add",
            "-q",
            wt.to_str().expect("utf8"),
            "feat/held",
        ],
    );
    // The checked-out master is gone-configured too: the worktree guard
    // covers it before the protected-branch guard would.
    git_in(repo.path(), &["config", "branch.master.remote", "origin"]);
    git_in(
        repo.path(),
        &["config", "branch.master.merge", "refs/heads/master"],
    );
    let out = rk_scrubbed()
        .args(["branches", "prune", "--target"])
        .arg(repo.path())
        // Every gone branch is guarded, so no candidate exists and an
        // apply resolves no forge and deletes nothing.
        .arg("--apply")
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.contains("kept"), "{text}");
    assert!(text.contains("worktree-bound"), "{text}");
    assert!(!text.contains("deleted"), "{text}");
    let names = branch_names(repo.path());
    for name in ["master", "release/1.0", "feat/held"] {
        assert!(names.contains(name), "{name} survives: {names}");
    }
}

#[test]
fn branches_prune_reports_a_worktree_bound_branch_as_actionable() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/x");
    let worktree = tempfile::tempdir().expect("a worktree parent exists");
    let wt = worktree.path().join("release-kit-feat-x");
    git_in(
        repo.path(),
        &[
            "worktree",
            "add",
            "-q",
            wt.to_str().expect("utf8"),
            "feat/x",
        ],
    );
    // A worktree-bound branch is exactly what the reminder surfaces, so
    // --quiet reports it rather than staying silent.
    let out = rk_scrubbed()
        .args(["branches", "prune", "--quiet", "--target"])
        .arg(repo.path())
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.contains("worktree-bound: checked out at"), "{text}");
    // An apply spends no forge call on it and never touches it: with no
    // candidate at all, no forge CLI is even resolved.
    rk_scrubbed()
        .args(["branches", "prune", "--apply", "--target"])
        .arg(repo.path())
        .assert()
        .success();
    assert!(
        branch_names(repo.path()).contains("feat/x"),
        "a worktree-bound branch survives an apply"
    );
    let report = rk_scrubbed()
        .args(["branches", "prune", "--json", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&report).expect("one JSON object");
    assert_eq!(report["branches"][0]["status"], "worktree-bound");
    assert!(
        report["branches"][0]["worktree"]
            .as_str()
            .is_some_and(|path| path.contains("release-kit-feat-x")),
        "{report}"
    );
}

#[test]
fn branches_prune_verify_confirms_against_the_forge() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/merged");
    advanced_gone_branch(repo.path(), "feat/advanced");
    let merged_tip = tip_of(repo.path(), "feat/merged");
    let body = format!(
        "#!/bin/sh\ncase \"$2\" in\n*/commits/{merged_tip}/pulls) printf '[{{\"number\":8,\"merged_at\":\"2026-01-01T00:00:00Z\",\"head\":{{\"sha\":\"{merged_tip}\"}}}}]' ;;\n*) printf '[]' ;;\nesac\n"
    );
    let (_mock, gh) = mock_gh(&body);
    let out = rk_scrubbed()
        .args(["branches", "prune", "--verify", "--target"])
        .arg(repo.path())
        .env("RK_GH_BIN", &gh)
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.contains("confirmed: merged request #8"), "{text}");
    assert!(text.contains("unconfirmed"), "{text}");
    let names = branch_names(repo.path());
    assert!(
        names.contains("feat/merged") && names.contains("feat/advanced"),
        "verify deletes nothing: {names}"
    );
}

#[test]
fn branches_prune_apply_deletes_confirmed_and_keeps_unknown() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/merged");
    advanced_gone_branch(repo.path(), "feat/opaque");
    let merged_tip = tip_of(repo.path(), "feat/merged");
    // The forge proves the merged tip and errors on everything else.
    let body = format!(
        "#!/bin/sh\ncase \"$2\" in\n*/commits/{merged_tip}/pulls) printf '[{{\"number\":8,\"merged_at\":\"2026-01-01T00:00:00Z\",\"head\":{{\"sha\":\"{merged_tip}\"}}}}]' ;;\n*) echo 'the forge is down' >&2; exit 1 ;;\nesac\n"
    );
    let (_mock, gh) = mock_gh(&body);
    let out = rk_scrubbed()
        .args(["branches", "prune", "--apply", "--target"])
        .arg(repo.path())
        .env("RK_GH_BIN", &gh)
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.contains("deleted (merged request #8)"), "{text}");
    assert!(text.contains("unknown"), "{text}");
    let names = branch_names(repo.path());
    assert!(!names.contains("feat/merged"), "the proven branch goes");
    assert!(
        names.contains("feat/opaque"),
        "an unanswerable branch stays: {names}"
    );
}

#[test]
fn branches_prune_quiet_conflicts_with_json() {
    rk_scrubbed()
        .args(["branches", "prune", "--quiet", "--json"])
        .assert()
        .code(64);
}

#[test]
fn branch_reminder_installs_the_hook_and_reapplies_idempotently() {
    let repo = branch_fixture();
    let home = tempfile::tempdir().expect("a scratch home exists");
    let (_mock, gh) = mock_gh("#!/bin/sh\nexit 0\n");
    let apply = || {
        let mut command = rk_scrubbed();
        command
            .env("HOME", home.path())
            .env("XDG_STATE_HOME", home.path())
            .env("RK_GH_BIN", &gh)
            .args(["setup", "step", "branch-reminder", "--apply", "--target"])
            .arg(repo.path());
        command
    };
    apply().assert().success().stderr(predicate::str::contains(
        "wrote the post-merge reminder hook",
    ));
    let hook = repo.path().join(".git/hooks/post-merge");
    let written = std::fs::read_to_string(&hook).expect("the hook reads");
    assert!(written.contains("# release-kit branch reminder"));
    assert!(written.contains("rk branches prune --quiet"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&hook)
            .expect("metadata")
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "the hook is executable: {mode:o}");
    }
    apply()
        .assert()
        .success()
        .stderr(predicate::str::contains("is installed"));
    // A drifted body that kept the marker is rewritten, not refused.
    std::fs::write(&hook, "#!/bin/sh\n# release-kit branch reminder\n").expect("the drift writes");
    apply().assert().success();
    assert_eq!(
        std::fs::read_to_string(&hook).expect("the hook reads"),
        written,
        "a drifted reminder is restored to this binary's body"
    );
}

#[test]
fn branch_reminder_refuses_a_foreign_post_merge_hook() {
    let repo = branch_fixture();
    let home = tempfile::tempdir().expect("a scratch home exists");
    let (_mock, gh) = mock_gh("#!/bin/sh\nexit 0\n");
    let hook = repo.path().join(".git/hooks/post-merge");
    std::fs::create_dir_all(hook.parent().expect("a parent")).expect("hooks dir");
    let foreign = "#!/bin/sh\necho mine\n";
    std::fs::write(&hook, foreign).expect("the foreign hook writes");
    let output = rk_scrubbed()
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .args(["setup", "step", "branch-reminder", "--apply", "--target"])
        .arg(repo.path())
        .assert()
        .code(73)
        .get_output()
        .clone();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("foreign"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&hook).expect("the hook reads"),
        foreign,
        "a foreign hook is never written over"
    );
}

#[test]
fn branch_reminder_honors_core_hooks_path() {
    let repo = branch_fixture();
    let home = tempfile::tempdir().expect("a scratch home exists");
    let (_mock, gh) = mock_gh("#!/bin/sh\nexit 0\n");
    git_in(repo.path(), &["config", "core.hooksPath", ".husky"]);
    rk_scrubbed()
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .args(["setup", "step", "branch-reminder", "--apply", "--target"])
        .arg(repo.path())
        .assert()
        .success();
    assert!(
        repo.path().join(".husky/post-merge").is_file(),
        "the hook lands where git says it will look"
    );
    assert!(!repo.path().join(".git/hooks/post-merge").exists());
}

#[test]
fn setup_preview_names_the_branch_reminder_write() {
    let repo = branch_fixture();
    let home = tempfile::tempdir().expect("a scratch home exists");
    let (_mock, gh) = mock_gh("#!/bin/sh\nexit 0\n");
    rk_scrubbed()
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .args(["setup", "step", "branch-reminder", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .stdout(
            predicate::str::contains("would write").and(predicate::str::contains("post-merge")),
        );
    assert!(
        !repo.path().join(".git/hooks/post-merge").exists(),
        "a preview writes nothing"
    );
}

#[test]
fn branches_prune_apply_refuses_a_branch_that_moved_after_verification() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/moved");
    let old_tip = tip_of(repo.path(), "feat/moved");
    let master_tip = tip_of(repo.path(), "master");
    // The forge confirms the enumerated tip, then the mock advances the
    // branch before answering - the race the compare-and-delete refuses.
    // A distinct object to move to: an empty-tree commit made on the spot.
    let new_commit = {
        let mut command = std::process::Command::new("git");
        for var in GIT_HOOK_VARS {
            command.env_remove(var);
        }
        let out = command
            .args([
                "commit-tree",
                "-m",
                "moved",
                &format!("{master_tip}^{{tree}}"),
            ])
            .current_dir(repo.path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("git runs");
        assert!(out.status.success());
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };
    let body = format!(
        "#!/bin/sh\ngit -C {repo} update-ref refs/heads/feat/moved {new_commit}\nprintf '[{{\"number\":9,\"merged_at\":\"2026-01-01T00:00:00Z\",\"head\":{{\"sha\":\"{old_tip}\"}}}}]'\n",
        repo = repo.path().display()
    );
    let (_mock, gh) = mock_gh(&body);
    let out = rk_scrubbed()
        .args(["branches", "prune", "--apply", "--target"])
        .arg(repo.path())
        .env("RK_GH_BIN", &gh)
        .assert()
        .code(70);
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(text.contains("delete failed"), "{text}");
    assert_eq!(
        tip_of(repo.path(), "feat/moved"),
        new_commit,
        "the moved branch survives at its new tip"
    );
}

#[test]
fn branch_reminder_refuses_a_dangling_hook_symlink() {
    let repo = branch_fixture();
    let home = tempfile::tempdir().expect("a scratch home exists");
    let (_mock, gh) = mock_gh("#!/bin/sh\nexit 0\n");
    let hook = repo.path().join(".git/hooks/post-merge");
    std::fs::create_dir_all(hook.parent().expect("a parent")).expect("hooks dir");
    #[cfg(unix)]
    std::os::unix::fs::symlink("/no/such/manager/post-merge", &hook)
        .expect("the dangling symlink writes");
    #[cfg(not(unix))]
    return;
    rk_scrubbed()
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .args(["setup", "step", "branch-reminder", "--apply", "--target"])
        .arg(repo.path())
        .assert()
        .code(73);
    assert!(
        std::fs::symlink_metadata(&hook).is_ok(),
        "the symlink survives untouched"
    );
    assert!(
        std::fs::symlink_metadata(&hook)
            .expect("metadata")
            .file_type()
            .is_symlink(),
        "still a symlink, not a written file"
    );
}

#[test]
fn branches_prune_apply_spares_a_branch_checked_out_mid_run() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/raced");
    let tip = tip_of(repo.path(), "feat/raced");
    let worktree = tempfile::tempdir().expect("a worktree parent exists");
    let wt = worktree.path().join("raced");
    // The mock forge checks the branch out into a worktree before
    // answering: the last-instant recheck must route it to the worktree
    // verb instead of deleting under a HEAD that now depends on it.
    let body = format!(
        "#!/bin/sh\ngit -C {repo} worktree add -q {wt} feat/raced >/dev/null 2>&1\nprintf '[{{\"number\":11,\"merged_at\":\"2026-01-01T00:00:00Z\",\"head\":{{\"sha\":\"{tip}\"}}}}]'\n",
        repo = repo.path().display(),
        wt = wt.display()
    );
    let (_mock, gh) = mock_gh(&body);
    let out = rk_scrubbed()
        .args(["branches", "prune", "--apply", "--json", "--target"])
        .arg(repo.path())
        .env("RK_GH_BIN", &gh)
        .assert()
        .success();
    let report: serde_json::Value =
        serde_json::from_slice(&out.get_output().stdout).expect("one JSON object");
    assert_eq!(
        report["branches"][0]["status"], "worktree-bound",
        "{report}"
    );
    assert!(
        branch_names(repo.path()).contains("feat/raced"),
        "the raced branch survives"
    );
}

#[test]
fn branches_prune_apply_removes_the_deleted_branch_configuration() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/merged");
    let tip = tip_of(repo.path(), "feat/merged");
    let body = format!(
        "#!/bin/sh\nprintf '[{{\"number\":8,\"merged_at\":\"2026-01-01T00:00:00Z\",\"head\":{{\"sha\":\"{tip}\"}}}}]'\n"
    );
    let (_mock, gh) = mock_gh(&body);
    rk_scrubbed()
        .args(["branches", "prune", "--apply", "--target"])
        .arg(repo.path())
        .env("RK_GH_BIN", &gh)
        .assert()
        .success();
    assert!(!branch_names(repo.path()).contains("feat/merged"));
    let leftover = std::process::Command::new("git")
        .args(["config", "--get-regexp", "^branch\\.feat/merged\\."])
        .current_dir(repo.path())
        .env_remove("GIT_DIR")
        .output()
        .expect("git runs");
    assert!(
        !leftover.status.success() && leftover.stdout.is_empty(),
        "no branch configuration survives the prune: {}",
        String::from_utf8_lossy(&leftover.stdout)
    );
}

#[test]
fn branches_prune_apply_reports_a_surviving_branch_configuration() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/merged");
    let tip = tip_of(repo.path(), "feat/merged");
    let body = format!(
        "#!/bin/sh\nprintf '[{{\"number\":8,\"merged_at\":\"2026-01-01T00:00:00Z\",\"head\":{{\"sha\":\"{tip}\"}}}}]'\n"
    );
    let (_mock, gh) = mock_gh(&body);
    // A held config lock: the ref deletion succeeds, the section removal
    // cannot, and the report says so instead of claiming a clean delete.
    std::fs::write(repo.path().join(".git/config.lock"), "").expect("the lock writes");
    let out = rk_scrubbed()
        .args(["branches", "prune", "--apply", "--target"])
        .arg(repo.path())
        .env("RK_GH_BIN", &gh)
        .assert()
        .success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(
        text.contains("deleted (merged request #8); the branch configuration survives: git config --remove-section branch.feat/merged"),
        "{text}"
    );
    assert!(
        text.contains("Deleting a branch is the operator's action"),
        "a surviving configuration still owes the operator a move: {text}"
    );
    assert!(!branch_names(repo.path()).contains("feat/merged"));
}

/// The rust/github seed attests the whole release payload. The flag alone
/// uses cargo-dist's default phase, which attests only the per-platform
/// archives; the installers a consumer actually curls are global artifacts
/// built later, so the seed must pin the `host` phase, pair the release
/// creation with it, and declare no narrowing filter. `announce` is a valid
/// cargo-dist phase but not this binding's: an announce-phase attestation is
/// minted after the release page exists, so accepting it would stop holding
/// the mint-before-publishable rule.
#[test]
fn the_rust_github_seed_attests_the_release_payload_in_the_host_phase() {
    let text = std::fs::read_to_string(repo_path("snippets/rust/github/dist-workspace.toml"))
        .expect("the seed reads");
    let value: toml::Table = text.parse().expect("the seed parses as TOML");
    let dist = value
        .get("dist")
        .and_then(toml::Value::as_table)
        .expect("a [dist] table");
    assert_eq!(
        dist.get("github-attestations")
            .and_then(toml::Value::as_bool),
        Some(true),
        "github-attestations must be enabled"
    );
    assert_eq!(
        dist.get("github-attestations-phase")
            .and_then(toml::Value::as_str),
        Some("host"),
        "the attest phase must be exactly host: the default attests only the per-platform archives, and announce attests after the page exists"
    );
    assert_eq!(
        dist.get("github-release").and_then(toml::Value::as_str),
        dist.get("github-attestations-phase")
            .and_then(toml::Value::as_str),
        "the release must be created in the same phase that attests"
    );
    assert!(
        !dist.contains_key("github-attestations-filters"),
        "no narrowing filter: the default [\"*\"] covers every hosted file, and an enumerated list goes quiet when an archive format moves"
    );
}

/// The bash/github release workflow mints the attestation before the release
/// page exists: the page is public the moment `gh release create` runs, so an
/// attest step after it leaves a window — permanent, if the run dies between
/// the two — where a public release points at an unattested tarball. The
/// assertion is on the order of the two in the file, not on their presence.
#[test]
fn the_bash_github_workflow_attests_before_it_creates_the_release() {
    let text = std::fs::read_to_string(repo_path(
        "snippets/bash/github/.github/workflows/release.yml",
    ))
    .expect("the workflow reads");
    let attest = text
        .find("uses: actions/attest@")
        .expect("the workflow carries an attest step");
    let create = text
        .find("gh release create")
        .expect("the workflow creates the release");
    assert!(
        attest < create,
        "the attest step must precede gh release create, so nothing publicly reachable exists before its attestation does"
    );
}

/// The bash/gitlab pipeline, split into its top-level sections: everything
/// from a column-zero `name:` line to the next column-zero line, comments
/// dropped so a job's prose neighbour cannot satisfy an assertion about its
/// commands. The provenance tests reason about which job carries what, so
/// they need the boundaries, not just the whole file.
fn bash_gitlab_sections() -> Vec<(String, String)> {
    let text = std::fs::read_to_string(repo_path("snippets/bash/gitlab/.gitlab-ci.yml"))
        .expect("the pipeline reads");
    let mut sections: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        if line.trim_start().starts_with('#') {
            continue;
        }
        let starts_section =
            !line.starts_with([' ', '#']) && !line.is_empty() && line.trim_end().ends_with(':');
        if starts_section {
            let name = line.trim_end().trim_end_matches(':').to_owned();
            sections.push((name, String::new()));
        }
        if let Some((_, body)) = sections.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    assert!(!sections.is_empty(), "the pipeline parsed to no sections");
    sections
}

fn bash_gitlab_job(name: &str) -> String {
    bash_gitlab_sections()
        .into_iter()
        .find(|(section, _)| section == name)
        .map_or_else(
            || panic!("the pipeline has no `{name}` job"),
            |(_, body)| body,
        )
}

/// The build job enables the runner's SLSA provenance metadata, and the
/// signing happens in an isolated downstream job: the job holding the
/// signing identity runs no project build steps, and the job that builds
/// holds no signing identity.
#[test]
fn the_bash_gitlab_build_emits_metadata_and_an_isolated_job_signs_it() {
    let build = bash_gitlab_job("tag-and-build");
    assert!(
        build.contains("RUNNER_GENERATE_ARTIFACTS_METADATA: \"true\""),
        "the build job must enable the runner's provenance metadata"
    );
    assert!(
        !build.contains("cosign"),
        "the build job must hold no signing identity"
    );
    let provenance = bash_gitlab_job("provenance");
    assert!(
        provenance.contains("cosign attest-blob") && provenance.contains("--type slsaprovenance1"),
        "the provenance job signs the runner's statement, not a bare blob"
    );
    assert!(
        !provenance.contains("make dist"),
        "the signing job must run no project build steps"
    );
    assert!(
        provenance.contains("SIGSTORE_ID_TOKEN"),
        "the signing job alone carries the sigstore-audience id token"
    );
    assert!(
        !build.contains("SIGSTORE_ID_TOKEN"),
        "the build job must not carry the signing token"
    );
}

/// The bundle self-verification pins the signer: the GitLab.com issuer and
/// the CI-configuration-plus-ref certificate identity, which is also what a
/// consumer checks. A release cut from a release/* line verifies against
/// that ref, so the identity must be built from the pipeline's own ref, not
/// a hard-coded trunk.
#[test]
fn the_bash_gitlab_bundle_verification_pins_issuer_and_identity() {
    let provenance = bash_gitlab_job("provenance");
    assert!(
        provenance.contains("cosign verify-blob-attestation"),
        "the bundle is verified before anything publishes"
    );
    assert!(
        provenance.contains("--certificate-oidc-issuer \"https://gitlab.com\""),
        "verification pins the GitLab.com issuer"
    );
    assert!(
        provenance.contains("${CI_PROJECT_PATH}//${CI_CONFIG_PATH}@refs/heads/${CI_COMMIT_BRANCH}"),
        "the certificate identity is the CI configuration path at the built ref"
    );
    assert!(
        !provenance.contains("@refs/heads/master"),
        "the identity must not hard-code the trunk: a release/* line verifies against its own ref"
    );
}

/// A push that releases nothing writes no release-version marker, and both
/// downstream jobs exit on its absence before taking any action, so an
/// ordinary work merge signs and publishes nothing.
#[test]
fn the_bash_gitlab_downstream_jobs_guard_on_the_release_marker() {
    for job in ["provenance", "attach"] {
        let body = bash_gitlab_job(job);
        let guard = body
            .find("[ ! -f release-version ]")
            .unwrap_or_else(|| panic!("{job} carries no release-version guard"));
        for action in ["cosign", "--upload-file", "releases"] {
            if let Some(position) = body.find(action) {
                assert!(
                    guard < position,
                    "{job}: the release-version guard must precede `{action}`"
                );
            }
        }
    }
    let build = bash_gitlab_job("tag-and-build");
    let clear = build
        .find("rm -f release-version")
        .expect("the build job clears a stale or tracked marker");
    let bail = build
        .find("nothing to release")
        .expect("the build job keeps the bump guard");
    let marker = build
        .find("> release-version")
        .expect("the build job writes the marker");
    assert!(
        clear < bail && bail < marker,
        "the marker is cleared before the bump guard and written only past it, so a pre-existing file cannot survive a no-bump push into the artifacts"
    );
}

/// A rerun repairs a partial release instead of skipping it: uploads are
/// per-file and conditional on what the package registry already holds, and
/// an existing release page gains its missing asset links rather than being
/// left as found.
#[test]
fn the_bash_gitlab_attach_job_reconciles_a_partial_release() {
    let attach = bash_gitlab_job("attach");
    assert!(
        attach.contains("package_files"),
        "the attach job reads what the package registry already holds"
    );
    assert!(
        attach.contains(
            r#"select(.package_type == "generic" and .name == "release" and .version == $v)"#
        ),
        "the package lookup selects the exact package: package_name is a fuzzy filter on this API"
    );
    assert!(
        attach.contains("assets/links"),
        "an existing release gains its missing links on a rerun"
    );
    assert!(
        !attach.contains("leaving it as it is"),
        "an existing release is reconciled, never skipped"
    );
}

/// A self-managed instance cannot mint keyless certificates, and the
/// pipeline says so at run time and releases without provenance, rather
/// than failing compilation or the release. The guard sits in the signing
/// job, before cosign is even fetched.
#[test]
fn the_bash_gitlab_pipeline_degrades_honestly_off_gitlab_com() {
    let provenance = bash_gitlab_job("provenance");
    let guard = provenance
        .find("\"$CI_SERVER_HOST\" != \"gitlab.com\"")
        .expect("the signing job guards on the instance");
    let fetch = provenance
        .find("cosign-linux-amd64")
        .expect("the signing job fetches the pinned cosign");
    assert!(
        guard < fetch,
        "the instance guard precedes the cosign fetch"
    );
    let attach = bash_gitlab_job("attach");
    assert!(
        attach.contains("-f \"$tarball.sigstore.json\""),
        "publication treats the bundle as conditionally present, so the self-managed path still releases"
    );
}

/// cosign is pinned like every other tool: the version in the pipeline is
/// the registry's, and the download is verified against a digest authored
/// beside it rather than trusted to a floating package index.
#[test]
fn the_bash_gitlab_cosign_pin_agrees_with_the_registry() {
    let registry = std::fs::read_to_string(repo_path("versions.toml")).expect("the registry reads");
    let value: toml::Table = registry.parse().expect("the registry parses");
    let pinned = value
        .get("tool")
        .and_then(toml::Value::as_array)
        .expect("tool entries")
        .iter()
        .find(|tool| tool.get("name").and_then(toml::Value::as_str) == Some("cosign"))
        .expect("a cosign entry")
        .get("version")
        .and_then(toml::Value::as_str)
        .expect("a cosign version")
        .to_owned();
    let provenance = bash_gitlab_job("provenance");
    assert!(
        provenance.contains(&format!("COSIGN_VERSION: \"{pinned}\"")),
        "the pipeline's cosign version must be the registry's ({pinned})"
    );
    let digest = provenance
        .lines()
        .find_map(|line| line.trim().strip_prefix("COSIGN_SHA256: \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .expect("an authored digest beside the version");
    assert!(
        digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()),
        "the authored digest is a sha256 hex literal"
    );
    assert!(
        provenance.contains("sha256sum -c"),
        "the fetched binary is checked against the authored digest before it runs"
    );
}

/// SATISFIES distribution:a-runbook-renders-the-spine
/// Step 6's provenance check renders the verifier of exactly the resolved
/// pair: the axis is technology and forge together, so a per-axis label
/// cannot say it, and the pair label must. Unresolved, every pair's answer
/// stays visible under its label.
#[test]
fn guide_release_renders_the_resolved_pairs_verifier() {
    let bare = tempfile::tempdir().expect("a bare dir exists");
    let render = |tech: &str, forge: &str| -> String {
        let out = rk()
            .args(["guide", "release", "--tech", tech, "--forge", forge])
            .current_dir(bare.path())
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        String::from_utf8_lossy(&out).into_owned()
    };
    let rust_github = render("rust", "github");
    assert!(rust_github.contains("gh attestation verify"));
    assert!(!rust_github.contains("cosign") && !rust_github.contains("pypi-attestations"));
    assert!(!rust_github.contains("no provenance surface"));
    // The flags are what bind the evidence to this release rather than to
    // any run that ever attested identical bytes, and the fail-fast exit is
    // what keeps one unattested asset from hiding behind the last one.
    assert!(rust_github.contains("--source-digest") && rust_github.contains("--signer-workflow"));
    assert!(rust_github.contains("|| exit 1"));
    let bash_github = render("bash", "github");
    assert!(bash_github.contains("gh attestation verify"));
    assert!(bash_github.contains("--source-digest") && bash_github.contains("--signer-workflow"));
    assert!(!bash_github.contains("cosign") && !bash_github.contains("pypi-attestations"));
    let bash_gitlab = render("bash", "gitlab");
    assert!(bash_gitlab.contains("cosign verify-blob-attestation"));
    assert!(!bash_gitlab.contains("gh attestation verify"));
    // The certificate ref is the branch the release was built from, so the
    // served command must carry the visible placeholder, never the trunk.
    assert!(bash_gitlab.contains("@<built ref>"));
    assert!(!bash_gitlab.contains("@refs/heads/master"));
    let rust_gitlab = render("rust", "gitlab");
    assert!(rust_gitlab.contains("declares no provenance surface"));
    assert!(!rust_gitlab.contains("cosign verify-blob-attestation"));
    let python_github = render("python", "github");
    assert!(python_github.contains("pypi-attestations verify pypi"));
    assert!(!python_github.contains("cosign"));
    let unresolved = rk()
        .args(["guide", "release"])
        .current_dir(bare.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let unresolved = String::from_utf8_lossy(&unresolved);
    for label in [
        "On rust/github:",
        "On bash/github:",
        "On bash/gitlab:",
        "On python/github:",
        "On rust/gitlab:",
    ] {
        assert!(
            unresolved.contains(label),
            "unresolved render must keep {label}"
        );
    }
}

/// SATISFIES landing:a-seeded-file-still-carries-the-invariants
/// A seeded file is judged, never rewritten: a target that turns
/// attestations off is reported by plain status (exit 0) and fails
/// `--check`, with the remediation stating what to write; the file itself
/// is untouched, and seeded drift alone stays informational.
#[test]
fn status_judges_a_seeded_file_that_dropped_the_invariants() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let seeded = target.path().join("dist-workspace.toml");
    let broken = "[workspace]\nmembers = [\"cargo:.\"]\n\n[dist]\ncargo-dist-version = \"0.32.0\"\nci = \"github\"\ngithub-attestations = false\n";
    std::fs::write(&seeded, broken).expect("the edit writes");

    // Plain status reports and exits 0; the file is the target's.
    let plain = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&plain).expect("one JSON object");
    let failures = report["invariant_failures"]
        .as_array()
        .expect("the plain report carries the failures too");
    assert!(
        failures
            .iter()
            .any(|failure| failure["code"] == "attestations-disabled"
                && failure["destination"] == "dist-workspace.toml"
                && failure["remediation"]
                    .as_str()
                    .is_some_and(|text| text.contains("github-attestations = true"))),
        "{report}"
    );
    assert!(
        failures
            .iter()
            .any(|failure| failure["code"] == "attestation-phase-not-host"),
        "{report}"
    );

    // The same report under --check is the violation.
    let checked = rk()
        .args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&checked);
    assert!(
        text.contains("INVARIANT dist-workspace.toml (attestations-disabled)"),
        "{text}"
    );
    assert!(
        text.contains("dist-workspace.toml: set github-attestations = true in [dist]"),
        "the next lines carry the remediation: {text}"
    );
    assert_eq!(
        std::fs::read_to_string(&seeded).expect("the seeded file reads"),
        broken,
        "the judgment rewrites nothing"
    );

    // A tuned targets list is seeded drift, informational as ever.
    let table = std::fs::read_to_string(repo_path("snippets/rust/github/dist-workspace.toml"))
        .expect("the seed reads");
    let table = table
        .split_once("[dist.github-action-commits]")
        .map(|(_, rest)| format!("[dist.github-action-commits]{rest}"))
        .expect("the seed carries the action-commit table");
    let tuned = format!(
        "[workspace]\nmembers = [\"cargo:.\"]\n\n[dist]\ncargo-dist-version = \"0.32.0\"\nci = \"github\"\ntargets = [\"x86_64-unknown-linux-gnu\"]\ngithub-attestations = true\ngithub-attestations-phase = \"host\"\ngithub-release = \"host\"\n\n{table}"
    );
    std::fs::write(&seeded, tuned).expect("the tune writes");
    rk().args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("DRIFT dist-workspace.toml"));
    // The remaining exit-1 is the landed judgment sentinel, not the
    // seeded drift: filling it proves the tuned file alone passes.
    let filled = std::fs::read_to_string(target.path().join("release-plz.toml"))
        .expect("the seeded file reads")
        .lines()
        .filter(|line| !line.contains("TODO(release-kit)"))
        .fold(String::new(), |mut text, line| {
            text.push_str(line);
            text.push('\n');
            text
        });
    std::fs::write(target.path().join("release-plz.toml"), filled).expect("the fill writes");
    rk().args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .success();
}

/// A recorded file that vanished is the missing violation, never a
/// validation attempt over absent bytes: plain status reports MISSING and
/// exits 0, the check exits 1 on the same report, and no invariant
/// failure is fabricated for a file that is not there.
#[test]
fn status_reports_a_missing_invariant_bearing_file_as_missing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    std::fs::remove_file(target.path().join("dist-workspace.toml")).expect("the file removes");
    let plain = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&plain).expect("one JSON object");
    assert!(
        report["missing"]
            .as_array()
            .is_some_and(|missing| missing.iter().any(|path| path == "dist-workspace.toml")),
        "{report}"
    );
    assert!(
        report["invariant_failures"]
            .as_array()
            .is_some_and(|failures| failures
                .iter()
                .all(|failure| failure["destination"] != "dist-workspace.toml")),
        "no invariant failure is fabricated for absent bytes: {report}"
    );
    let checked = rk()
        .args(["status", "--check", "--json", "--target"])
        .arg(target.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let checked: serde_json::Value = serde_json::from_slice(&checked).expect("one JSON object");
    assert!(
        checked["violations"]
            .as_array()
            .is_some_and(|violations| violations
                .iter()
                .any(|violation| violation == "missing: dist-workspace.toml")),
        "the missing file is itself the violation, not a bystander to the sentinel: {checked}"
    );
    assert!(
        !target.path().join("dist-workspace.toml").exists(),
        "the judgment rewrites nothing"
    );
}

/// Every `uses:` line across the snippet payload, as `(file, owner/action,
/// ref, trailing comment)`.
fn snippet_action_refs() -> Vec<(String, String, String, Option<String>)> {
    let mut refs = Vec::new();
    let root = repo_path("snippets");
    let mut stack = vec![root.clone()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).expect("the dir reads") {
                stack.push(entry.expect("an entry").path());
            }
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let file = path
            .strip_prefix(&root)
            .expect("a path under snippets")
            .display()
            .to_string();
        for line in text.lines() {
            let Some(spec) = line
                .trim_start()
                .strip_prefix("- uses: ")
                .or_else(|| line.trim_start().strip_prefix("uses: "))
            else {
                continue;
            };
            let (spec, comment) = spec
                .split_once('#')
                .map_or((spec, None), |(spec, comment)| {
                    (spec, Some(comment.trim().to_owned()))
                });
            let spec = spec.trim();
            let (action, reference) = spec.split_once('@').expect("a uses ref carries an @");
            refs.push((
                file.clone(),
                action.to_owned(),
                reference.to_owned(),
                comment,
            ));
        }
    }
    assert!(!refs.is_empty(), "the payload carries uses: references");
    refs
}

/// SATISFIES: the signer runs no movable code. Every action reference in
/// the payload is a full lowercase commit SHA with the readable discovery
/// ref kept as a trailing comment — the form that keeps a moved tag from
/// changing what executes before someone reviews it.
#[test]
fn every_snippet_action_is_pinned_by_full_commit() {
    for (file, action, reference, comment) in snippet_action_refs() {
        assert!(
            reference.len() == 40
                && reference
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "{file}: {action}@{reference} is not pinned to a full commit SHA"
        );
        assert!(
            comment.is_some_and(|comment| !comment.is_empty()),
            "{file}: {action} carries no readable ref comment beside its pin"
        );
    }
}

/// Every action the payload executes maps to exactly one registry entry
/// whose execution commit is the pinned SHA and whose discovery ref is the
/// readable comment; every entry with an action carries a valid commit and
/// a classified ref, and a non-action entry carries neither.
#[test]
fn every_payload_action_maps_to_one_registry_entry() {
    let registry = std::fs::read_to_string(repo_path("versions.toml")).expect("the registry reads");
    let registry: toml::Table = registry.parse().expect("the registry parses");
    let tools = registry
        .get("tool")
        .and_then(toml::Value::as_array)
        .expect("tool entries");
    let classes = [
        "moving-major-tag",
        "moving-minor-tag",
        "exact-tag",
        "maintained-branch",
    ];
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(toml::Value::as_str)
            .expect("a name");
        let action = tool.get("action").and_then(toml::Value::as_str);
        let commit = tool.get("commit").and_then(toml::Value::as_str);
        let ref_class = tool.get("ref_class").and_then(toml::Value::as_str);
        if action.is_some() {
            let commit = commit.unwrap_or_else(|| panic!("{name}: an action entry pins no commit"));
            assert!(
                commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit()),
                "{name}: the commit is not a full SHA"
            );
            assert!(
                ref_class.is_some_and(|class| classes.contains(&class)),
                "{name}: the discovery ref is not classified"
            );
        } else {
            assert!(
                commit.is_none() && ref_class.is_none(),
                "{name}: a non-action entry carries action-only fields"
            );
        }
    }
    for (file, action, reference, comment) in snippet_action_refs() {
        let matching: Vec<&toml::Value> = tools
            .iter()
            .filter(|tool| {
                tool.get("action")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|spec| spec.split_once('@').is_some_and(|(a, _)| a == action))
                    && tool.get("commit").and_then(toml::Value::as_str) == Some(reference.as_str())
            })
            .collect();
        assert_eq!(
            matching.len(),
            1,
            "{file}: {action}@{reference} must map to exactly one registry entry"
        );
        let entry = matching[0];
        assert_eq!(
            entry.get("commit").and_then(toml::Value::as_str),
            Some(reference.as_str()),
            "{file}: {action} pins a commit the registry does not carry"
        );
        let discovery = entry
            .get("action")
            .and_then(toml::Value::as_str)
            .and_then(|spec| spec.split_once('@'))
            .map(|(_, reference)| reference)
            .expect("a discovery ref");
        assert_eq!(
            comment.as_deref(),
            Some(discovery),
            "{file}: {action}'s readable comment must be the registry's discovery ref"
        );
    }
}

/// The cargo-dist action-commit table covers every action the pinned
/// backend actually emits, at the same commits: read from the generated
/// workflow, which `dist generate --mode ci --check` holds to the
/// configuration.
#[test]
fn the_action_commit_table_covers_what_dist_emits() {
    let seed = std::fs::read_to_string(repo_path("snippets/rust/github/dist-workspace.toml"))
        .expect("the seed reads");
    let seed: toml::Table = seed.parse().expect("the seed parses");
    let table = seed
        .get("dist")
        .and_then(toml::Value::as_table)
        .and_then(|dist| dist.get("github-action-commits"))
        .and_then(toml::Value::as_table)
        .expect("the seed pins the actions dist injects");
    let generated = std::fs::read_to_string(repo_path(".github/workflows/release.yml"))
        .expect("the generated workflow reads");
    let mut emitted = std::collections::BTreeSet::new();
    for line in generated.lines() {
        let Some(spec) = line
            .trim_start()
            .strip_prefix("- uses: ")
            .or_else(|| line.trim_start().strip_prefix("uses: "))
        else {
            continue;
        };
        let (action, reference) = spec
            .trim()
            .split_once('@')
            .expect("a uses ref carries an @");
        emitted.insert(action.to_owned());
        assert_eq!(
            table.get(action).and_then(toml::Value::as_str),
            Some(reference),
            "the generated workflow runs {action}@{reference}, which the table does not pin"
        );
    }
    for action in table.keys() {
        assert!(
            emitted.contains(action),
            "the table pins {action}, which the pinned dist backend does not emit"
        );
    }
    // Every table pin is also a registry record, so the dist-injected
    // actions share the freshness model instead of bypassing it: their
    // discovery refs resolve under rk versions --check like every other.
    let registry = std::fs::read_to_string(repo_path("versions.toml")).expect("the registry reads");
    let registry: toml::Table = registry.parse().expect("the registry parses");
    let tools = registry
        .get("tool")
        .and_then(toml::Value::as_array)
        .expect("tool entries");
    for (action, commit) in table {
        let matched: Vec<&toml::Value> = tools
            .iter()
            .filter(|tool| {
                tool.get("action")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|spec| spec.split_once('@').is_some_and(|(a, _)| a == action))
                    && tool.get("commit").and_then(toml::Value::as_str) == commit.as_str()
            })
            .collect();
        assert_eq!(
            matched.len(),
            1,
            "the table's {action} pin must map to exactly one registry entry"
        );
        assert!(
            matched[0]
                .get("used_by")
                .and_then(toml::Value::as_array)
                .is_some_and(|users| users.iter().any(|user| user.as_str() == Some("rust"))),
            "the table's {action} pin runs in the rust binding's workflow, so its registry entry must say so"
        );
    }
}

/// The boundary of the pillar, tested rather than implied: fetched CI
/// code is pinned by commit, and the GitLab payload fetches none — no
/// CI/CD catalog component is included, so there is nothing there to pin.
/// Container images stay pinned by version tag, not digest; they are the
/// execution environment, not fetched steps, and the ADR states it.
#[test]
fn the_gitlab_payload_includes_no_catalog_component() {
    for name in ["bash/gitlab/.gitlab-ci.yml", "rust/gitlab/.gitlab-ci.yml"] {
        let text = std::fs::read_to_string(repo_path("snippets").join(name)).expect("reads");
        assert!(
            !text.contains("component:"),
            "{name} includes a catalog component, which this pillar requires pinning by commit"
        );
    }
}

/// This repository's own landing record carries every pin the registry
/// declares for its technology: a record refreshed by an older binary
/// would silently lose the newer pins' offline staleness baseline, and
/// status iterates the record, not the registry.
#[test]
fn this_repos_own_record_carries_every_rust_pin() {
    let manifest =
        std::fs::read_to_string(repo_path(".release-kit/manifest.json")).expect("the record reads");
    let manifest: serde_json::Value = serde_json::from_str(&manifest).expect("the record parses");
    let recorded = manifest["pins"].as_object().expect("a pin map");
    let expected: std::collections::BTreeMap<String, String> =
        release_kit::registry::pins_for("rust")
            .into_iter()
            .map(|pin| (pin.name, pin.version))
            .collect();
    let recorded: std::collections::BTreeMap<String, String> = recorded
        .iter()
        .filter_map(|(name, version)| {
            version
                .as_str()
                .map(|version| (name.clone(), version.to_owned()))
        })
        .collect();
    assert_eq!(
        recorded, expected,
        "the record's pin map must equal the registry's rust pins — nothing missing, nothing obsolete; refresh it with this binary's rk upgrade --target . --apply"
    );
}

// ---------------------------------------------------------------------------
// The workflow mode: a landing parameter

/// Land the rust payload under one explicit workflow mode.
fn land_rust_with_workflow(target: &Path, workflow: &str) -> assert_cmd::assert::Assert {
    rk().args(["init", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--scopes", "api,cli", "--workflow"])
        .arg(workflow)
        .args(["--target"])
        .arg(target)
        .arg("--apply")
        .assert()
}

/// SATISFIES maintenance:the-workflow-mode-is-a-landing-parameter
/// SATISFIES maintenance:worktree-mode-guards-the-main-checkout
#[test]
fn init_defaults_to_the_worktree_workflow() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["workflow"], "worktree");
    let hooks = std::fs::read_to_string(target.path().join(".pre-commit-config.yaml"))
        .expect("the hook file reads");
    assert!(
        hooks.contains("- id: rk-worktree-location"),
        "the worktree mode lands the location guard: {hooks}"
    );
    assert!(
        hooks.contains("SKIP=no-commit-to-branch,rk-worktree-location"),
        "the sweep comment names the skip pair: {hooks}"
    );
    let agents = std::fs::read_to_string(target.path().join("AGENTS.md")).expect("AGENTS.md reads");
    assert!(
        agents.contains("This project works in worktrees"),
        "{agents}"
    );
}

/// SATISFIES maintenance:branches-mode-refuses-nothing
#[test]
fn init_lands_the_branches_workflow_without_the_guard() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust_with_workflow(target.path(), "branches").success();
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["workflow"], "branches");
    let hooks = std::fs::read_to_string(target.path().join(".pre-commit-config.yaml"))
        .expect("the hook file reads");
    assert!(
        !hooks.contains("rk-worktree-location"),
        "the branches mode lands no guard entry at all: {hooks}"
    );
    let agents = std::fs::read_to_string(target.path().join("AGENTS.md")).expect("AGENTS.md reads");
    assert!(
        agents.contains("Branches are worked in the main checkout"),
        "{agents}"
    );
    rk().args([
        "init",
        "--tech",
        "rust",
        "--forge",
        "github",
        "--workflow",
        "bogus",
    ])
    .args(["--target"])
    .arg(target.path())
    .assert()
    .code(64)
    .stderr(predicate::str::contains("worktree, branches"));
}

/// The flag chooses which candidate adoption verifies against — nothing
/// more: adoption defaults to `branches`, the compatibility-safe reading
/// of a pre-record target, and records the mode it verified.
#[test]
fn adopt_records_the_branches_workflow_by_default() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust_with_workflow(target.path(), "branches").success();
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args(["--repo", "acme/widget", "--target"])
    .arg(target.path())
    .arg("--apply")
    .assert()
    .success();
    assert_eq!(
        read_manifest(target.path())["parameters"]["workflow"],
        "branches"
    );
}

/// Adoption verifies against the selected candidate and never blesses the
/// disk: a target whose blocks carry the other mode refuses whole, every
/// mismatch listed, not a byte written — in both directions.
#[test]
fn adopt_refuses_a_target_whose_blocks_do_not_match_the_selected_candidate() {
    for (landed, selected) in [("worktree", "branches"), ("branches", "worktree")] {
        let target = tempfile::tempdir().expect("a scratch dir exists");
        land_rust_with_workflow(target.path(), landed).success();
        std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
        let before = tree_digests(target.path());
        let output = rk()
            .args([
                "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style",
                "trunk",
            ])
            .args(["--workflow", selected, "--repo", "acme/widget", "--target"])
            .arg(target.path())
            .arg("--apply")
            .assert()
            .code(73)
            .get_output()
            .clone();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("AGENTS.md") && stderr.contains(".pre-commit-config.yaml"),
            "every mismatching block is listed: {stderr}"
        );
        assert!(
            stderr.contains("align first"),
            "the refusal names the preview-first alignment: {stderr}"
        );
        assert_eq!(
            tree_digests(target.path()),
            before,
            "a refused adoption writes nothing"
        );
    }
}

/// SATISFIES landing:a-rendered-file-is-reproducible
/// Recorded digests alone cannot see a manifest edited only at
/// `parameters.workflow` — every file still matches its own record — so
/// `--check` re-renders the blocks from the record's own parameters and
/// flags the contradiction; the plain run stays exit 0.
#[test]
fn status_check_flags_a_manifest_whose_workflow_contradicts_its_landed_blocks() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    manifest["parameters"]["workflow"] = serde_json::json!("branches");
    write_manifest(target.path(), &manifest);

    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "the recorded parameters do not render the recorded bytes",
        ));
    let out = rk()
        .args(["status", "--check", "--json", "--target"])
        .arg(target.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert!(
        report["violations"]
            .as_array()
            .is_some_and(|violations| violations.iter().any(|violation| {
                violation
                    .as_str()
                    .is_some_and(|line| line.starts_with("parameter drift:"))
            })),
        "{report}"
    );
}

/// SATISFIES landing:a-record-states-its-schema
/// An upgrade of a schema-1 target is also the record's migration: the
/// absent parameter reads as `branches`, the blocks re-render to that
/// mode, and the rewrite records schema 2 with the mode stated.
#[test]
fn an_upgrade_migrates_a_schema_1_record_to_the_current_schema() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    manifest["schema_version"] = serde_json::json!(1);
    manifest
        .get_mut("parameters")
        .and_then(serde_json::Value::as_object_mut)
        .expect("a parameters object")
        .remove("workflow");
    manifest
        .get_mut("parameters")
        .and_then(serde_json::Value::as_object_mut)
        .expect("a parameters object")
        .remove("style");
    write_manifest(target.path(), &manifest);

    // A pre-style record refuses until --style names one: neither value is
    // a compatibility-safe reading of a target nobody asked.
    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .code(64)
        .stderr(predicate::str::contains("--style"));

    rk().args(["upgrade", "--apply", "--style", "trunk", "--target"])
        .arg(target.path())
        .assert()
        .success();
    let migrated = read_manifest(target.path());
    assert_eq!(migrated["schema_version"], 4);
    assert_eq!(migrated["parameters"]["workflow"], "branches");
    assert_eq!(migrated["parameters"]["style"], "trunk");
    let hooks = std::fs::read_to_string(target.path().join(".pre-commit-config.yaml"))
        .expect("the hook file reads");
    assert!(
        !hooks.contains("rk-worktree-location"),
        "a pre-mode record reads as branches, so no guard is imposed: {hooks}"
    );
}

/// SATISFIES landing:an-upgrade-refuses-on-owned-drift
/// A mode change on a drifted target refuses atomically: exit nonzero,
/// every file and the manifest byte-identical.
#[test]
fn a_mode_change_refuses_atomically_on_owned_drift() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let workflow = target.path().join(".github/workflows/release-plz.yml");
    let mut edited = std::fs::read_to_string(&workflow).expect("the workflow reads");
    edited.push_str("# a local edit\n");
    std::fs::write(&workflow, edited).expect("the drift writes");
    let before = tree_digests(target.path());

    rk().args(["upgrade", "--workflow", "branches", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .code(73)
        .stderr(predicate::str::contains("release-plz.yml"));
    assert_eq!(
        tree_digests(target.path()),
        before,
        "a refused mode change leaves every file and the manifest byte-identical"
    );
}

/// The mode change is an upgrade with exactly one overridden parameter:
/// the record and the two blocks flip, everything else stays
/// byte-identical, and no other flag is passed.
#[test]
fn a_mode_change_upgrades_from_the_record() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let before = tree_digests(target.path());

    rk().args(["upgrade", "--workflow", "branches", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success();
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["workflow"], "branches");
    let hooks = std::fs::read_to_string(target.path().join(".pre-commit-config.yaml"))
        .expect("the hook file reads");
    assert!(!hooks.contains("rk-worktree-location"), "{hooks}");
    let moved = [
        "AGENTS.md",
        ".pre-commit-config.yaml",
        ".release-kit/manifest.json",
    ];
    let after = tree_digests(target.path());
    for (path, digest) in &before {
        if moved.contains(&path.as_str()) {
            continue;
        }
        assert!(
            after.iter().any(|(other, d)| other == path && d == digest),
            "{path} moved under a mode change that does not own it"
        );
    }
}

/// A plain upgrade keeps the recorded mode across payload versions.
#[test]
fn upgrade_keeps_the_recorded_mode() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success();
    assert_eq!(
        read_manifest(target.path())["parameters"]["workflow"],
        "worktree"
    );
    let hooks = std::fs::read_to_string(target.path().join(".pre-commit-config.yaml"))
        .expect("the hook file reads");
    assert!(hooks.contains("- id: rk-worktree-location"), "{hooks}");
}

/// SATISFIES landing:status-judges-only-under-check
/// The mode is reported in both status forms, and a hand-removed guard
/// entry is caught the ordinary way: the file's digest moved.
#[test]
fn status_reports_the_mode_and_check_flags_a_hand_edited_guard() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["workflow"], "worktree");
    rk().args(["status", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("worktree workflow"));

    let hooks_path = target.path().join(".pre-commit-config.yaml");
    let hooks = std::fs::read_to_string(&hooks_path).expect("the hook file reads");
    let edited: String = hooks
        .lines()
        .filter(|line| !line.contains("rk worktree location"))
        .flat_map(|line| [line, "\n"])
        .collect();
    assert_ne!(hooks, edited, "the guard's name line is removed by hand");
    std::fs::write(&hooks_path, edited).expect("the edit writes");
    let out = rk()
        .args(["status", "--check", "--json", "--target"])
        .arg(target.path())
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert!(
        report["violations"]
            .as_array()
            .is_some_and(|violations| violations.iter().any(|violation| {
                violation
                    .as_str()
                    .is_some_and(|line| line.contains(".pre-commit-config.yaml"))
            })),
        "{report}"
    );
}

// ---------------------------------------------------------------------------
// rk worktree list | add | prune

/// A scratch parent holding the repo at `./widget`, so the derived
/// sibling worktrees stay inside the tempdir and die with it.
fn worktree_fixture() -> (tempfile::TempDir, PathBuf) {
    let parent = tempfile::tempdir().expect("a scratch parent exists");
    let repo = parent.path().join("widget");
    std::fs::create_dir(&repo).expect("the repo dir creates");
    git_in(&repo, &["init", "-q", "-b", "master"]);
    git_in(&repo, &["config", "user.email", "rk@example.invalid"]);
    git_in(&repo, &["config", "user.name", "rk test"]);
    git_in(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/acme/widget.git",
        ],
    );
    std::fs::write(repo.join("seed"), "x\n").expect("the seed writes");
    git_in(&repo, &["add", "seed"]);
    git_in(&repo, &["commit", "-qm", "chore: seed"]);
    (parent, repo)
}

/// Seat one existing branch in its derived sibling worktree, via git.
fn seat(repo: &Path, branch: &str) -> PathBuf {
    let path = repo
        .parent()
        .expect("a parent")
        .join(format!("widget@{}", branch.replace('/', "-")));
    git_in(
        repo,
        &[
            "worktree",
            "add",
            "-q",
            path.to_str().expect("utf-8"),
            branch,
        ],
    );
    path
}

#[test]
fn worktree_list_reports_main_linked_drift_and_missing() {
    let (_parent, repo) = worktree_fixture();
    gone_branch(&repo, "feat/x");
    let seat_x = seat(&repo, "feat/x");
    std::fs::write(seat_x.join("dirt"), "y\n").expect("the dirt writes");
    // An off-path seat, made by hand elsewhere: works, reported, never
    // refused.
    git_in(&repo, &["branch", "fix/y"]);
    let elsewhere = repo.parent().expect("a parent").join("somewhere-else");
    git_in(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            elsewhere.to_str().expect("utf-8"),
            "fix/y",
        ],
    );
    // A registered record whose directory is gone.
    git_in(&repo, &["branch", "fix/z"]);
    let missing = seat(&repo, "fix/z");
    std::fs::remove_dir_all(&missing).expect("the directory disappears");

    let out = rk_scrubbed()
        .args(["worktree", "list", "--json", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.worktree-list/1");
    let rows = report["worktrees"].as_array().expect("rows");
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0]["kind"], "main");
    assert_eq!(rows[0]["branch"], "master");
    let named = |branch: &str| {
        rows.iter()
            .find(|row| row["branch"] == branch)
            .unwrap_or_else(|| panic!("a row for {branch}"))
    };
    assert_eq!(named("feat/x")["state"], "dirty");
    assert_eq!(named("feat/x")["canonical"], true);
    assert_eq!(named("fix/y")["state"], "clean");
    assert_eq!(named("fix/y")["canonical"], false);
    assert_eq!(named("fix/z")["state"], "missing");

    let human = rk_scrubbed()
        .args(["worktree", "list", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&human);
    assert!(
        text.contains("off-path: expected ../widget@fix-y"),
        "{text}"
    );
    assert!(text.contains("rk worktree prune"), "{text}");
}

#[test]
fn worktree_add_previews_and_writes_nothing() {
    let (_parent, repo) = worktree_fixture();
    let before = branch_names(&repo);
    let out = rk_scrubbed()
        .args(["worktree", "add", "feat/oauth-login", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("path:   ../widget@feat-oauth-login"),
        "{text}"
    );
    assert!(text.contains("would run: git worktree add"), "{text}");
    assert_eq!(branch_names(&repo), before, "a preview creates no branch");
    assert!(
        !repo
            .parent()
            .expect("a parent")
            .join("widget@feat-oauth-login")
            .exists(),
        "a preview creates no worktree"
    );
}

/// SATISFIES maintenance:a-worktree-path-derives-from-project-and-branch
#[test]
fn worktree_add_applies_creates_branch_and_prints_the_path() {
    let (_parent, repo) = worktree_fixture();
    let expected = repo
        .parent()
        .expect("a parent")
        .join("widget@feat-oauth-login");
    let out = rk_scrubbed()
        .args([
            "worktree",
            "add",
            "feat/oauth-login",
            "--apply",
            "--json",
            "--target",
        ])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.worktree-add/1");
    assert_eq!(report["mode"], "apply");
    assert_eq!(report["created"], "branch");
    assert_eq!(report["source"], "trunk");
    assert_eq!(
        report["path"],
        expected.to_str().expect("utf-8"),
        "{report}"
    );
    assert!(expected.is_dir(), "the worktree exists");
    assert!(branch_names(&repo).contains("feat/oauth-login"));

    let human = rk_scrubbed()
        .args(["worktree", "add", "fix/second", "--apply", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&human);
    assert!(
        text.lines()
            .next()
            .is_some_and(|line| line.ends_with("widget@fix-second")),
        "the result line is the absolute path: {text}"
    );
}

#[test]
fn worktree_add_adopts_a_local_branch_and_is_satisfied_when_standing() {
    let (_parent, repo) = worktree_fixture();
    git_in(&repo, &["branch", "feat/bare"]);
    let out = rk_scrubbed()
        .args([
            "worktree",
            "add",
            "feat/bare",
            "--apply",
            "--json",
            "--target",
        ])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["source"], "adopted");
    assert_eq!(report["created"], "worktree");

    // Idempotent: the standing canonical worktree satisfies.
    let out = rk_scrubbed()
        .args([
            "worktree",
            "add",
            "feat/bare",
            "--apply",
            "--json",
            "--target",
        ])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["created"], "nothing", "{report}");
}

/// The remote-source arm: a forge-minted branch or the bot's release
/// branch is seated from its real remote tip with the upstream set —
/// never silently recreated from the trunk.
#[test]
fn worktree_add_creates_a_tracking_branch_from_a_remote_tip() {
    let (parent, repo) = worktree_fixture();
    // A real remote: a bare sibling carrying feat/x one commit ahead.
    let bare = parent.path().join("origin.git");
    git_in(
        parent.path(),
        &["init", "-q", "--bare", bare.to_str().expect("utf-8")],
    );
    git_in(
        &repo,
        &["remote", "set-url", "origin", bare.to_str().expect("utf-8")],
    );
    git_in(&repo, &["push", "-q", "origin", "master"]);
    git_in(&repo, &["branch", "feat/x"]);
    git_in(&repo, &["checkout", "-q", "feat/x"]);
    std::fs::write(repo.join("ahead"), "y\n").expect("the extra file writes");
    git_in(&repo, &["add", "ahead"]);
    git_in(&repo, &["commit", "-qm", "chore: advance"]);
    let remote_tip = tip_of(&repo, "feat/x");
    git_in(&repo, &["push", "-q", "origin", "feat/x"]);
    git_in(&repo, &["checkout", "-q", "master"]);
    git_in(&repo, &["branch", "-D", "feat/x"]);
    git_in(&repo, &["fetch", "-q", "origin"]);

    let out = rk_scrubbed()
        .args(["worktree", "add", "feat/x", "--apply", "--json", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["source"], "remote", "{report}");
    assert_eq!(report["upstream"], "origin/feat/x");
    assert_eq!(
        tip_of(&repo, "feat/x"),
        remote_tip,
        "the worktree lands on the remote tip, not the trunk"
    );
    let mut command = std::process::Command::new("git");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let upstream = command
        .args(["rev-parse", "--abbrev-ref"])
        .arg(concat!("feat/x@", "{upstream}"))
        .current_dir(&repo)
        .output()
        .expect("git runs");
    assert_eq!(
        String::from_utf8_lossy(&upstream.stdout).trim(),
        "origin/feat/x",
        "the tracking branch carries its upstream"
    );
}

#[test]
fn worktree_add_refuses_the_trunk_a_bad_grammar_and_a_collision() {
    let (_parent, repo) = worktree_fixture();
    // The trunk fails the grammar before its own refusal arm: the main
    // checkout is its seat either way.
    rk_scrubbed()
        .args(["worktree", "add", "master", "--target"])
        .arg(&repo)
        .assert()
        .code(64)
        .stderr(predicate::str::contains("<type>/<slug>"));
    rk_scrubbed()
        .args(["worktree", "add", "feature/nope", "--target"])
        .arg(&repo)
        .assert()
        .code(64)
        .stderr(predicate::str::contains("release/<line>"));

    // The flattening collision: feat/a-b vs feat-a/b — wait, feat-a is
    // no type; use the honest pair the derivation admits.
    git_in(&repo, &["branch", "feat/a-b"]);
    let seat_ab = seat(&repo, "feat/a-b");
    assert!(seat_ab.is_dir());
    let before = branch_names(&repo);
    rk_scrubbed()
        .args(["worktree", "add", "feat/a/b", "--apply", "--target"])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("feat/a-b"));
    assert_eq!(branch_names(&repo), before, "a refusal mutates nothing");
}

#[test]
fn worktree_add_refuses_a_git_invalid_ref_and_an_unresolvable_base() {
    let (_parent, repo) = worktree_fixture();
    let before = branch_names(&repo);
    rk_scrubbed()
        .args(["worktree", "add", "feat/a..b", "--target"])
        .arg(&repo)
        .assert()
        .code(64)
        .stderr(predicate::str::contains("git refuses"));
    rk_scrubbed()
        .args(["worktree", "add", "feat/x.lock", "--target"])
        .arg(&repo)
        .assert()
        .code(64);
    rk_scrubbed()
        .args([
            "worktree",
            "add",
            "feat/x",
            "--base",
            "no-such-ref",
            "--target",
        ])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("does not resolve"));
    // clap already refuses a bare `--base --force`; the equals form
    // reaches the handler, whose own validation refuses it before any
    // resolution.
    rk_scrubbed()
        .args(["worktree", "add", "feat/x", "--base=--force", "--target"])
        .arg(&repo)
        .assert()
        .code(64)
        .stderr(predicate::str::contains("option-shaped"));
    rk_scrubbed()
        .args(["worktree", "add", "release/1.2", "--target"])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("--base"));
    assert_eq!(branch_names(&repo), before, "every refusal mutates nothing");
}

/// SATISFIES maintenance:a-worktree-path-derives-from-project-and-branch
#[test]
fn worktree_add_refuses_a_branch_checked_out_elsewhere() {
    let (_parent, repo) = worktree_fixture();
    git_in(&repo, &["branch", "feat/held"]);
    let elsewhere = repo.parent().expect("a parent").join("held-elsewhere");
    git_in(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            elsewhere.to_str().expect("utf-8"),
            "feat/held",
        ],
    );
    // The linked seat names the move to the derived path.
    rk_scrubbed()
        .args(["worktree", "add", "feat/held", "--apply", "--target"])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("held-elsewhere"))
        .stderr(predicate::str::contains("git worktree move"))
        .stderr(predicate::str::contains("widget@feat-held"));
    assert!(
        !repo
            .parent()
            .expect("a parent")
            .join("widget@feat-held")
            .exists(),
        "a refusal creates nothing"
    );

    // The main-checkout seat names the switch instead: it is never moved.
    git_in(&repo, &["checkout", "-qb", "feat/mine"]);
    rk_scrubbed()
        .args(["worktree", "add", "feat/mine", "--apply", "--target"])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("git switch master"));
}

#[test]
fn worktree_add_refuses_a_bare_repository() {
    let parent = tempfile::tempdir().expect("a scratch parent exists");
    let bare = parent.path().join("bare.git");
    git_in(
        parent.path(),
        &["init", "-q", "--bare", bare.to_str().expect("utf-8")],
    );
    rk_scrubbed()
        .args(["worktree", "add", "feat/x", "--target"])
        .arg(&bare)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("bare"));
}

/// A seated branch whose upstream is gone: the reportable shape.
fn gone_seat(repo: &Path, branch: &str) -> PathBuf {
    gone_branch(repo, branch);
    seat(repo, branch)
}

/// SATISFIES maintenance:a-dirty-or-locked-worktree-is-never-removed
#[test]
fn worktree_prune_preview_is_offline_and_keeps_the_guarded() {
    let (_parent, repo) = worktree_fixture();
    let dirty = gone_seat(&repo, "feat/dirty");
    std::fs::write(dirty.join("dirt"), "y\n").expect("the dirt writes");
    let locked = gone_seat(&repo, "feat/held");
    git_in(
        &repo,
        &[
            "worktree",
            "lock",
            "--reason",
            "a running agent",
            locked.to_str().expect("utf-8"),
        ],
    );
    gone_branch(&repo, "release/1.2");
    seat(&repo, "release/1.2");
    gone_seat(&repo, "feat/free");
    // A detached probe seat: never a row — no branch, no upstream.
    let probe = repo.parent().expect("a parent").join("widget-probe");
    git_in(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            probe.to_str().expect("utf-8"),
        ],
    );

    let out = rk_scrubbed()
        .args(["worktree", "prune", "--json", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.worktree-prune/1");
    assert_eq!(report["mode"], "preview");
    let rows = report["worktrees"].as_array().expect("rows");
    let named = |branch: &str| {
        rows.iter()
            .find(|row| row["branch"] == branch)
            .unwrap_or_else(|| panic!("a row for {branch}"))
    };
    assert_eq!(named("feat/dirty")["status"], "kept");
    assert!(
        named("feat/dirty")["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("uncommitted")),
        "{report}"
    );
    assert_eq!(named("feat/held")["status"], "kept");
    assert!(
        named("feat/held")["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("a running agent")),
        "{report}"
    );
    assert_eq!(named("release/1.2")["status"], "kept");
    assert_eq!(named("feat/free")["status"], "candidate");
    assert!(
        !rows.iter().any(|row| row["path"]
            .as_str()
            .is_some_and(|path| path.ends_with("widget-probe"))),
        "a detached seat is never a row: {report}"
    );
    assert_eq!(rows.len(), 4, "{report}");
}

/// SATISFIES maintenance:a-prune-report-covers-cleanup-not-inventory
/// A repository with only a main checkout, and one with an active linked
/// worktree mid-work, report nothing — the clean-clone guarantee the
/// reminder hook rests on.
#[test]
fn worktree_prune_reports_nothing_in_a_healthy_repository() {
    let (_parent, repo) = worktree_fixture();
    let assert_clean = |repo: &Path| {
        let out = rk_scrubbed()
            .args(["worktree", "prune", "--json", "--target"])
            .arg(repo)
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
        assert_eq!(report["worktrees"], serde_json::json!([]), "{report}");
        let quiet = rk_scrubbed()
            .args(["worktree", "prune", "--quiet", "--target"])
            .arg(repo)
            .assert()
            .success()
            .get_output()
            .clone();
        assert!(quiet.stdout.is_empty(), "quiet prints nothing when clean");
    };
    assert_clean(&repo);
    // An active linked worktree on a branch with no upstream: healthy.
    git_in(&repo, &["branch", "feat/active"]);
    seat(&repo, "feat/active");
    assert_clean(&repo);
}

/// SATISFIES maintenance:an-unobservable-branch-is-never-a-candidate
#[test]
fn worktree_prune_keeps_a_worktree_whose_branch_observation_is_missing() {
    let (_parent, repo) = worktree_fixture();
    git_in(&repo, &["branch", "feat/ghost"]);
    let ghost = seat(&repo, "feat/ghost");
    let tip = tip_of(&repo, "feat/ghost");
    // Delete the ref under the seated worktree: the inventory still names
    // the branch, and no observation covers it.
    git_in(&repo, &["update-ref", "-d", "refs/heads/feat/ghost", &tip]);
    let out = rk_scrubbed()
        .args(["worktree", "prune", "--json", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let row = report["worktrees"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["branch"] == "feat/ghost"))
        .expect("a kept row");
    assert_eq!(row["status"], "kept");
    assert!(
        row["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("no branch observation")),
        "{report}"
    );
    assert!(ghost.is_dir(), "nothing is removed");

    // The whole inventory failing to parse refuses the run before any
    // judgment: no branch lines at all while worktrees name branches.
    git_in(&repo, &["update-ref", "-d", "refs/heads/master"]);
    rk_scrubbed()
        .args(["worktree", "prune", "--target"])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(predicate::str::contains("branch inventory"));
}

/// The stale sweep's outcome is read per row, never assumed: one record
/// cleared, one surviving what a mid-run lock would do.
#[cfg(unix)]
#[test]
fn worktree_prune_apply_reads_the_stale_sweep_per_row() {
    use std::os::unix::fs::PermissionsExt as _;
    let (_parent, repo) = worktree_fixture();
    git_in(&repo, &["branch", "fix/gone"]);
    let gone = seat(&repo, "fix/gone");
    std::fs::remove_dir_all(&gone).expect("the directory disappears");
    git_in(&repo, &["branch", "fix/gone-too"]);
    let survivor = seat(&repo, "fix/gone-too");
    std::fs::remove_dir_all(&survivor).expect("the directory disappears");
    // A record the sweep cannot clear — its administrative directory made
    // read-only — stands in for a lock arriving mid-run: the per-row
    // re-observation reports it truthfully instead of a blanket claim.
    let record_dir = repo.join(".git/worktrees/widget@fix-gone-too");
    assert!(record_dir.is_dir(), "the record directory exists");
    let readonly = std::fs::Permissions::from_mode(0o555);
    std::fs::set_permissions(&record_dir, readonly).expect("the permissions set");

    let out = rk_scrubbed()
        .args(["worktree", "prune", "--apply", "--json", "--target"])
        .arg(&repo)
        .assert()
        .code(70)
        .get_output()
        .stdout
        .clone();
    std::fs::set_permissions(&record_dir, std::fs::Permissions::from_mode(0o755))
        .expect("the permissions restore");
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let rows = report["worktrees"].as_array().expect("rows");
    let row = |suffix: &str| {
        rows.iter()
            .find(|row| {
                row["path"]
                    .as_str()
                    .is_some_and(|path| path.ends_with(suffix))
            })
            .unwrap_or_else(|| panic!("a row ending {suffix}"))
    };
    assert_eq!(row("widget@fix-gone")["status"], "pruned", "{report}");
    assert_eq!(
        row("widget@fix-gone-too")["status"],
        "remove-failed",
        "{report}"
    );
    assert!(
        row("widget@fix-gone-too")["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("re-run rk worktree prune --apply")),
        "a failure row names its recovery: {report}"
    );
    let mut command = std::process::Command::new("git");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let registered = command
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&repo)
        .output()
        .expect("git runs");
    let listing = String::from_utf8_lossy(&registered.stdout).into_owned();
    assert!(
        !listing.contains("widget@fix-gone\n") && listing.contains("widget@fix-gone-too"),
        "the locked record is intact, the stale one is cleared: {listing}"
    );
}

#[test]
fn worktree_prune_keeps_the_callers_seat_across_targets() {
    let (_parent, repo) = worktree_fixture();
    let seat_a = gone_seat(&repo, "feat/mine");
    let out = rk_scrubbed()
        .current_dir(&seat_a)
        .args(["worktree", "prune", "--json", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let row = report["worktrees"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["branch"] == "feat/mine"))
        .expect("a row");
    assert_eq!(row["status"], "kept", "{report}");
    assert!(
        row["detail"]
            .as_str()
            .is_some_and(|detail| detail.contains("seat in use")),
        "{report}"
    );
}

#[test]
fn worktree_prune_classifies_fresh_and_locked_missing_records() {
    let (_parent, repo) = worktree_fixture();
    gone_branch(&repo, "feat/unlocked");
    let unlocked = seat(&repo, "feat/unlocked");
    std::fs::remove_dir_all(&unlocked).expect("the directory disappears");
    gone_branch(&repo, "feat/locked");
    let locked = seat(&repo, "feat/locked");
    git_in(
        &repo,
        &["worktree", "lock", locked.to_str().expect("utf-8")],
    );
    std::fs::remove_dir_all(&locked).expect("the directory disappears");

    let out = rk_scrubbed()
        .args(["worktree", "prune", "--json", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let rows = report["worktrees"].as_array().expect("rows");
    let by_branch = |branch: &str| {
        rows.iter()
            .find(|row| row["branch"] == branch)
            .unwrap_or_else(|| panic!("a row for {branch}"))
    };
    assert_eq!(by_branch("feat/unlocked")["status"], "stale", "{report}");
    assert_eq!(by_branch("feat/locked")["status"], "kept", "{report}");
}

/// The mock forge confirming one tip through the commits/<tip>/pulls path.
fn confirming_gh(tip: &str) -> String {
    format!(
        "#!/bin/sh\nprintf '[{{\"number\":8,\"merged_at\":\"2026-01-01T00:00:00Z\",\"head\":{{\"sha\":\"{tip}\"}}}}]'\n"
    )
}

/// SATISFIES maintenance:one-merge-proof-authorizes-both-removals
#[test]
fn worktree_prune_verify_confirms_against_the_forge() {
    let (_parent, repo) = worktree_fixture();
    gone_seat(&repo, "feat/merged");
    let tip = tip_of(&repo, "feat/merged");
    let (_mock, gh) = mock_gh(&confirming_gh(&tip));
    let out = rk_scrubbed()
        .args(["worktree", "prune", "--verify", "--json", "--target"])
        .arg(&repo)
        .env("RK_GH_BIN", &gh)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["mode"], "verify");
    let row = &report["worktrees"][0];
    assert_eq!(row["status"], "confirmed");
    assert_eq!(row["request"], "#8");
    assert!(
        repo.parent()
            .expect("a parent")
            .join("widget@feat-merged")
            .is_dir(),
        "verify removes nothing"
    );
}

/// SATISFIES maintenance:a-worktree-is-removed-before-its-branch
/// SATISFIES maintenance:a-branch-deletion-is-compare-and-swap
#[test]
fn worktree_prune_apply_removes_the_tree_then_cas_deletes_the_branch() {
    let (_parent, repo) = worktree_fixture();
    let path = gone_seat(&repo, "feat/merged");
    // Dirt makes git refuse the remove without --force: the ordered
    // apply must then leave the branch and its configuration untouched.
    std::fs::write(path.join("dirt"), "y\n").expect("the dirt writes");
    let tip = tip_of(&repo, "feat/merged");
    let (_mock, gh) = mock_gh(&confirming_gh(&tip));
    let run = |args: &[&str]| {
        let mut cmd = rk_scrubbed();
        cmd.args(["worktree", "prune"])
            .args(args)
            .args(["--json", "--target"])
            .arg(&repo)
            .env("RK_GH_BIN", &gh);
        cmd.assert()
    };
    // The candidate is dirty, so it is kept — clean it first to reach the
    // remove path, then re-dirty between verify and apply below.
    std::fs::remove_file(path.join("dirt")).expect("the dirt clears");
    // Simulate the refused remove instead with a lock git honors.
    git_in(&repo, &["worktree", "lock", path.to_str().expect("utf-8")]);
    let out = run(&["--apply"]).success().get_output().stdout.clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(
        report["worktrees"][0]["status"], "kept",
        "a lock keeps the worktree whole: {report}"
    );
    assert!(branch_names(&repo).contains("feat/merged"));

    git_in(
        &repo,
        &["worktree", "unlock", path.to_str().expect("utf-8")],
    );
    let out = run(&["--apply"]).success().get_output().stdout.clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["worktrees"][0]["status"], "pruned", "{report}");
    assert_eq!(report["worktrees"][0]["request"], "#8");
    assert!(!path.exists(), "the worktree is removed");
    assert!(
        !branch_names(&repo).contains("feat/merged"),
        "the branch follows its worktree"
    );
}

/// Verification authorizes only the state it saw: a tip that moved after
/// the forge confirmed it keeps the worktree in place.
#[test]
fn worktree_prune_apply_keeps_a_tip_that_moved_before_removal() {
    let (_parent, repo) = worktree_fixture();
    let path = gone_seat(&repo, "feat/moved");
    let old_tip = tip_of(&repo, "feat/moved");
    let (_mock, gh) = mock_gh(&confirming_gh(&old_tip));
    // The forge answers for the old tip; the branch then advances in its
    // worktree before the apply acts.
    let advance = path.join("more");
    std::fs::write(&advance, "y\n").expect("the extra file writes");
    git_in(&path, &["add", "more"]);
    git_in(&path, &["commit", "-qm", "chore: advance"]);
    let out = rk_scrubbed()
        .args(["worktree", "prune", "--apply", "--json", "--target"])
        .arg(&repo)
        .env("RK_GH_BIN", &gh)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let row = &report["worktrees"][0];
    assert!(
        row["status"] == "kept" || row["status"] == "unconfirmed",
        "the moved tip is never removed: {report}"
    );
    assert!(path.is_dir(), "nothing was removed");
    assert!(branch_names(&repo).contains("feat/moved"));
}

/// The residual window, exercised at the helper seam: a tip the
/// compare-and-swap no longer matches refuses the deletion, so a branch
/// can outlive its removed worktree — reported truthfully, with
/// `rk worktree add` as the named recovery.
#[test]
fn worktree_prune_apply_reports_a_branch_that_outlived_its_worktree() {
    let (_parent, repo) = worktree_fixture();
    advanced_gone_branch(&repo, "feat/racy");
    let stale_tip = tip_of(&repo, "master");
    assert_ne!(stale_tip, tip_of(&repo, "feat/racy"));
    let refused = release_kit::maintenance::delete_branch(&utf8(&repo), "feat/racy", &stale_tip);
    assert!(
        matches!(refused, release_kit::maintenance::Deletion::Refused { .. }),
        "a moved tip refuses the compare-and-swap: {refused:?}"
    );
    assert!(
        branch_names(&repo).contains("feat/racy"),
        "the branch and its work survive"
    );
    let current = tip_of(&repo, "feat/racy");
    assert!(matches!(
        release_kit::maintenance::delete_branch(&utf8(&repo), "feat/racy", &current),
        release_kit::maintenance::Deletion::Deleted
    ));
}

/// After a pruned row, no branch.<name> configuration survives.
#[test]
fn worktree_prune_apply_drops_the_branch_configuration() {
    let (_parent, repo) = worktree_fixture();
    gone_seat(&repo, "feat/merged");
    let tip = tip_of(&repo, "feat/merged");
    let (_mock, gh) = mock_gh(&confirming_gh(&tip));
    rk_scrubbed()
        .args(["worktree", "prune", "--apply", "--target"])
        .arg(&repo)
        .env("RK_GH_BIN", &gh)
        .assert()
        .success();
    let mut command = std::process::Command::new("git");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let leftover = command
        .args(["config", "--get-regexp", r"^branch\.feat/merged\."])
        .current_dir(&repo)
        .output()
        .expect("git runs");
    assert!(
        leftover.stdout.is_empty(),
        "no stale configuration survives: {}",
        String::from_utf8_lossy(&leftover.stdout)
    );
}

/// SATISFIES maintenance:forge-unavailability-never-authorizes-deletion
#[test]
fn worktree_prune_apply_keeps_unknown() {
    let (_parent, repo) = worktree_fixture();
    let path = gone_seat(&repo, "feat/unknown");
    let (_mock, gh) = mock_gh("#!/bin/sh\necho 'connect: network is unreachable' >&2\nexit 1\n");
    let out = rk_scrubbed()
        .args(["worktree", "prune", "--apply", "--json", "--target"])
        .arg(&repo)
        .env("RK_GH_BIN", &gh)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["worktrees"][0]["status"], "unknown", "{report}");
    assert!(path.is_dir(), "forge unavailability removes nothing");
    assert!(branch_names(&repo).contains("feat/unknown"));
}

#[test]
fn worktree_prune_quiet_prints_nothing_when_clean() {
    let (_parent, repo) = worktree_fixture();
    let out = rk_scrubbed()
        .args(["worktree", "prune", "--quiet", "--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .clone();
    assert!(out.stdout.is_empty());
    rk_scrubbed()
        .args(["worktree", "prune", "--quiet", "--json", "--target"])
        .arg(&repo)
        .assert()
        .code(64);
}

#[test]
fn worktree_json_failure_is_one_diagnostic_line() {
    let output = rk_scrubbed()
        .args(["worktree", "list", "--json", "--target", "/no/such/dir"])
        .assert()
        .code(66)
        .get_output()
        .clone();
    assert!(output.stdout.is_empty());
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&output.stderr).expect("stderr is one JSON diagnostic");
    assert_eq!(diagnostic["schema"], "rk.diagnostic/1");
}

// ---------------------------------------------------------------------------
// Orientation and enforcement: the guard, the reminder, the closing line

/// The `rk-worktree-location` entry's script, extracted from the rendered
/// worktree-mode hook block — the exact bytes a landing writes.
fn guard_script() -> String {
    let block = release_kit::landing::hooks_block(release_kit::landing::Workflow::Worktree);
    let entry = block
        .lines()
        .skip_while(|line| !line.contains("id: rk-worktree-location"))
        .find(|line| line.trim_start().starts_with("entry: sh -c '"))
        .expect("the guard entry line exists");
    entry
        .trim_start()
        .strip_prefix("entry: sh -c '")
        .and_then(|rest| rest.strip_suffix('\''))
        .expect("the entry is one single-quoted script")
        .to_owned()
}

/// Run the guard's script through sh in one directory, as pre-commit
/// would.
fn guard_verdict(dir: &Path) -> bool {
    let mut command = std::process::Command::new("sh");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    command
        .args(["-c", &guard_script()])
        .current_dir(dir)
        .output()
        .expect("sh runs")
        .status
        .success()
}

/// SATISFIES maintenance:worktree-mode-guards-the-main-checkout
/// The six-cell matrix: main+master passes, main+topic and main+detached
/// refuse — the invariant is whole, detached included — and a linked
/// worktree passes in all three states, because topology is tested first.
#[test]
fn the_guard_holds_the_full_matrix() {
    let (_parent, repo) = worktree_fixture();
    git_in(&repo, &["branch", "feat/linked"]);
    let linked = seat(&repo, "feat/linked");

    assert!(guard_verdict(&repo), "main + master passes");
    git_in(&repo, &["checkout", "-qb", "feat/topic"]);
    assert!(!guard_verdict(&repo), "main + topic refuses");
    git_in(&repo, &["checkout", "-q", "--detach"]);
    assert!(!guard_verdict(&repo), "main + detached refuses");
    git_in(&repo, &["checkout", "-q", "master"]);

    assert!(guard_verdict(&linked), "linked + its branch passes");
    git_in(&linked, &["checkout", "-q", "--detach"]);
    assert!(guard_verdict(&linked), "linked + detached passes");
    git_in(&linked, &["checkout", "-qb", "feat/renamed"]);
    assert!(guard_verdict(&linked), "linked + another branch passes");
}

/// Write the reminder body to a script and run it under sh with a
/// controlled PATH; returns (stdout, stderr, success).
fn run_reminder(path_dir: &Path, repo: &Path) -> (String, String, bool) {
    let script = repo.join("reminder.sh");
    std::fs::write(&script, release_kit::setup::branch_reminder::hook_body())
        .expect("the body writes");
    // Absolute sh: the controlled PATH is the test's point, and it must
    // constrain what the hook finds, not what the test can spawn.
    let mut command = std::process::Command::new("/bin/sh");
    for var in GIT_HOOK_VARS {
        command.env_remove(var);
    }
    let out = command
        .arg(script)
        .env("PATH", path_dir)
        .current_dir(repo)
        .output()
        .expect("sh runs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

/// A stub `rk` in its own PATH directory, plus the sh git needs.
fn stub_rk(body: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a stub dir exists");
    let path = dir.path().join("rk");
    std::fs::write(&path, body).expect("the stub writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("the stub is executable");
    }
    dir
}

/// SATISFIES maintenance:the-reminder-is-silent-on-a-binary-that-cannot-prune
/// Behavioral, not a string assertion: a binary that fails every argument
/// — an rk too old for the verbs — keeps the hook silent and exit 0.
#[test]
fn the_reminder_is_silent_on_a_binary_that_cannot_prune() {
    let (_parent, repo) = worktree_fixture();
    let stub = stub_rk("#!/bin/sh\necho 'Usage: rk <COMMAND>' >&2\nexit 64\n");
    let (stdout, stderr, ok) = run_reminder(stub.path(), &repo);
    assert_eq!(stdout, "", "nothing on stdout");
    assert_eq!(
        stderr, "",
        "the probe swallows the incapable binary's usage noise"
    );
    assert!(ok, "the reminder never blocks a pull");
}

/// SATISFIES maintenance:the-reminder-never-blocks-a-pull
/// Command-not-found is the other silent case, and only this exercises
/// it: a PATH with no rk at all.
#[test]
fn the_reminder_is_silent_with_no_rk_on_the_path() {
    let (_parent, repo) = worktree_fixture();
    let empty = tempfile::tempdir().expect("an empty PATH dir exists");
    let (stdout, stderr, ok) = run_reminder(empty.path(), &repo);
    assert_eq!(stdout, "");
    assert_eq!(stderr, "");
    assert!(ok);
}

/// During a transition a binary exists that carries one prune verb and
/// not the other: the probes are per verb, so the half that works runs
/// and the half that does not stays silent.
#[test]
fn the_reminder_probes_each_verb_separately() {
    let (_parent, repo) = worktree_fixture();
    let stub = stub_rk(
        "#!/bin/sh\ncase \"$1 $2\" in\n'branches prune') echo 'BRANCHES RAN';;\n*) echo 'Usage: rk <COMMAND>' >&2; exit 64;;\nesac\n",
    );
    let (stdout, stderr, ok) = run_reminder(stub.path(), &repo);
    assert!(
        stdout.contains("BRANCHES RAN"),
        "the capable half runs: {stdout}"
    );
    assert!(!stdout.contains("worktree"), "{stdout}");
    assert_eq!(stderr, "", "the incapable half stays silent");
    assert!(ok);
}

/// The landed old body observes as Drifted and the designed
/// drift-and-reapply path rewrites it to the probe form.
#[test]
fn branch_reminder_drifted_body_rewrites_on_reapply() {
    let repo = branch_fixture();
    let home = tempfile::tempdir().expect("a scratch home exists");
    let (_mock, gh) = mock_gh("#!/bin/sh\nexit 0\n");
    let hook = repo.path().join(".git/hooks/post-merge");
    std::fs::create_dir_all(hook.parent().expect("a parent")).expect("hooks dir");
    // The 0.2.x body: marker present, `command -v` guard, branches only.
    std::fs::write(
        &hook,
        "#!/bin/sh\n# release-kit branch reminder\nif command -v rk >/dev/null 2>&1; then\n  rk branches prune --quiet || :\nfi\nexit 0\n",
    )
    .expect("the old body writes");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755))
            .expect("the hook is executable");
    }
    rk_scrubbed()
        .env("HOME", home.path())
        .env("XDG_STATE_HOME", home.path())
        .env("RK_GH_BIN", &gh)
        .args(["setup", "step", "branch-reminder", "--apply", "--target"])
        .arg(repo.path())
        .assert()
        .success();
    let written = std::fs::read_to_string(&hook).expect("the hook reads");
    assert_eq!(
        written.as_bytes(),
        release_kit::setup::branch_reminder::hook_body(),
        "the drifted body is rewritten to this binary's"
    );
    assert!(written.contains("rk worktree prune --help"), "{written}");
}

/// The branches report routes a worktree-bound row to the verb that owns
/// its cleanup — and only then.
#[test]
fn branches_prune_next_names_the_worktree_verb_only_when_bound_rows_exist() {
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/plain");
    let unbound = rk_scrubbed()
        .args(["branches", "prune", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        !String::from_utf8_lossy(&unbound).contains("rk worktree prune"),
        "no bound row, no worktree line"
    );

    gone_branch(repo.path(), "feat/seated");
    let elsewhere = repo.path().join("seated-worktree");
    git_in(
        repo.path(),
        &[
            "worktree",
            "add",
            "-q",
            elsewhere.to_str().expect("utf-8"),
            "feat/seated",
        ],
    );
    let bound = rk_scrubbed()
        .args(["branches", "prune", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&bound)
            .contains("rk worktree prune --verify confirms the worktree-bound branches"),
        "a bound row routes to the worktree verb: {}",
        String::from_utf8_lossy(&bound)
    );
}

/// The closing operator line rides what is still owed, never the mode:
/// for both verbs, a preview, a verify, and an apply that kept something
/// keep it; an apply that finished everything it named and an empty
/// report drop it, the empty report keeping its Next block.
#[test]
fn a_report_that_owes_nothing_drops_the_operator_line() {
    // branches: an apply that deletes its one candidate closes silent.
    let repo = branch_fixture();
    gone_branch(repo.path(), "feat/merged");
    let tip = tip_of(repo.path(), "feat/merged");
    let (_mock, gh) = mock_gh(&confirming_gh(&tip));
    let preview = rk_scrubbed()
        .args(["branches", "prune", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&preview).contains("Deleting a branch is the operator's action"),
        "a candidate still owes"
    );
    let applied = rk_scrubbed()
        .args(["branches", "prune", "--apply", "--target"])
        .arg(repo.path())
        .env("RK_GH_BIN", &gh)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&applied);
    assert!(text.contains("deleted"), "{text}");
    assert!(
        !text.contains("operator's action"),
        "an apply that finished everything it named closes without the line: {text}"
    );
    let empty = rk_scrubbed()
        .args(["branches", "prune", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&empty);
    assert!(
        !text.contains("operator's action"),
        "an empty report owes nothing: {text}"
    );
    assert!(
        text.contains("Next:"),
        "the empty report keeps Next: {text}"
    );

    // worktree: same discipline through the shared predicate.
    let (_parent, wt_repo) = worktree_fixture();
    gone_seat(&wt_repo, "feat/merged");
    let tip = tip_of(&wt_repo, "feat/merged");
    let (_mock2, gh2) = mock_gh(&confirming_gh(&tip));
    let verify = rk_scrubbed()
        .args(["worktree", "prune", "--verify", "--target"])
        .arg(&wt_repo)
        .env("RK_GH_BIN", &gh2)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&verify)
            .contains("Removing a worktree and deleting its branch are the operator's action"),
        "a confirmed row still owes the apply"
    );
    let applied = rk_scrubbed()
        .args(["worktree", "prune", "--apply", "--target"])
        .arg(&wt_repo)
        .env("RK_GH_BIN", &gh2)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&applied);
    assert!(text.contains("pruned"), "{text}");
    assert!(
        !text.contains("operator's action"),
        "a finished worktree apply closes without the line: {text}"
    );
}

/// A registered branch whose canonical directory was deleted by hand is
/// a stale record, not a standing seat: add refuses naming the prune
/// recovery instead of reporting satisfied over a path that is gone.
#[test]
fn worktree_add_refuses_a_stale_canonical_record() {
    let (_parent, repo) = worktree_fixture();
    git_in(&repo, &["branch", "feat/x"]);
    let path = seat(&repo, "feat/x");
    std::fs::remove_dir_all(&path).expect("the directory disappears");
    rk_scrubbed()
        .args(["worktree", "add", "feat/x", "--apply", "--target"])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(
            predicate::str::contains("directory is missing")
                .and(predicate::str::contains("rk worktree prune --apply")),
        );

    // A locked missing seat takes a different recovery: prune keeps a
    // locked record unconditionally, so the refusal names the unlock or
    // the repair, never a prune that would loop back here.
    git_in(&repo, &["branch", "feat/held"]);
    let held = seat(&repo, "feat/held");
    git_in(&repo, &["worktree", "lock", held.to_str().expect("utf-8")]);
    std::fs::remove_dir_all(&held).expect("the directory disappears");
    rk_scrubbed()
        .args(["worktree", "add", "feat/held", "--apply", "--target"])
        .arg(&repo)
        .assert()
        .code(73)
        .stderr(
            predicate::str::contains("git worktree unlock")
                .and(predicate::str::contains("git worktree repair")),
        );
}

/// A behavior-defining flag the preview was run with rides into the
/// emitted follow-up command, for both verbs that preview a decision:
/// following the Next line applies what was previewed, never a default.
#[test]
fn a_preview_follow_up_keeps_the_previewed_decision() {
    let (_parent, repo) = worktree_fixture();
    let tag_tip = tip_of(&repo, "master");
    let out = rk_scrubbed()
        .args(["worktree", "add", "release/1.2", "--base"])
        .arg(&tag_tip)
        .args(["--target"])
        .arg(&repo)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains(&format!("--base {tag_tip}")),
        "the add follow-up carries the base: {}",
        String::from_utf8_lossy(&out)
    );

    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    let out = rk()
        .args(["upgrade", "--workflow", "branches", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("rk upgrade --workflow branches --target"),
        "the upgrade follow-up carries the mode change: {}",
        String::from_utf8_lossy(&out)
    );
}

/// A scratch repository ignoring `.draft/`, for the message content guard.
fn message_fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch repo exists");
    git_in(dir.path(), &["init", "-q", "-b", "master"]);
    std::fs::write(dir.path().join(".gitignore"), ".draft/\n").expect("the ignore writes");
    dir
}

/// An agent attribution trailer is a finding, and `--check` refuses it.
#[test]
fn a_message_with_attribution_fails_the_check() {
    let dir = message_fixture();
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .write_stdin("feat(api): add\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("attribution:3"))
        .stderr(predicate::str::contains("2 findings"));
}

/// A reference to a git-ignored path is a finding at its line.
#[test]
fn a_message_naming_an_ignored_path_fails_the_check() {
    let dir = message_fixture();
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .write_stdin("feat(api): add\n\nSee .draft/plan.md for the plan.\n")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains(
            "internal-path:3 .draft/plan.md is git-ignored",
        ));
}

/// The release bot's request is exempt from the attribution class by its
/// title — the real release-plz body shape passes — while the
/// ignored-path class still runs.
#[test]
fn the_bot_request_is_exempt_from_attribution_alone() {
    let dir = message_fixture();
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .write_stdin(
            "chore: release v0.2.6\n\n## 🤖 New release\n\
             \n* `release-kit`: 0.2.5 -> 0.2.6\n\
             \n---\nThis PR was generated with [release-plz](https://github.com/release-plz/release-plz/).\n\
             Co-authored-by: gubasso-release-kit-bot[bot] <231623272+gubasso-release-kit-bot[bot]@users.noreply.github.com>\n",
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("exempt: the release bot's request"));
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .write_stdin("chore: release v0.2.6\n\nStill names .draft/notes.md though.\n")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("internal-path:3"));
}

/// A clean message passes, from a file and from stdin, and a plain run
/// reports without failing.
#[test]
fn a_clean_message_passes_and_a_plain_run_reports() {
    let dir = message_fixture();
    let file = dir.path().join("COMMIT_EDITMSG");
    std::fs::write(&file, "feat(api): add the endpoint\n\nA plain body.\n")
        .expect("the message writes");
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("clean commit"));
    rk().args(["message", "--target"])
        .arg(dir.path())
        .write_stdin("docs(readme): tidy\n\nCo-Authored-By: Claude\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("attribution:3"));
}

/// `--json` emits one object carrying the schema, kind, exemption, and
/// findings, also when the check fails.
#[test]
fn the_message_json_is_one_object_with_the_schema() {
    let dir = message_fixture();
    let out = rk()
        .args(["message", "--json", "--check", "--target"])
        .arg(dir.path())
        .write_stdin("feat(api): add\n\nSee .draft/plan.md here.\n")
        .assert()
        .failure()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value =
        serde_json::from_slice(&out).expect("one JSON object on stdout");
    assert_eq!(report["schema"], "rk.message/1");
    assert_eq!(report["kind"], "commit");
    assert_eq!(report["exempt"], false);
    assert_eq!(report["findings"][0]["class"], "internal-path");
    assert_eq!(report["findings"][0]["line"], 3);
}

/// Outside a repository the ignored-path class degrades to the fixed
/// `.draft/` pattern, honestly noted, and still finds the leak.
#[test]
fn a_non_repo_target_degrades_to_the_fixed_pattern() {
    let dir = tempfile::tempdir().expect("a plain directory exists");
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .write_stdin("feat(api): add\n\nSee .draft/plan.md here.\n")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains(
            "references the internal .draft/ tree",
        ))
        .stderr(predicate::str::contains("not a git repository"));
}

/// A body is judged with its request title supplying the exemption.
#[test]
fn a_body_is_judged_under_its_request_title() {
    let dir = message_fixture();
    rk().args([
        "message",
        "--check",
        "--kind",
        "body",
        "--title",
        "chore: release v0.3.0",
        "--target",
    ])
    .arg(dir.path())
    .write_stdin("Generated with [Claude Code](https://claude.com/claude-code)\n")
    .assert()
    .success()
    .stdout(predicate::str::contains("exempt"));
    rk().args(["message", "--check", "--kind", "body", "--target"])
        .arg(dir.path())
        .write_stdin("Generated with [Claude Code](https://claude.com/claude-code)\n")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("attribution:1"));
}

/// A decorated reference — no clean whitespace token — still answers in
/// a repository, through the fixed pattern, and a non-ASCII ignored path
/// comes back verbatim through check-ignore's NUL-delimited form.
#[test]
fn decorated_and_non_ascii_references_still_answer() {
    let dir = message_fixture();
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .write_stdin("feat(api): add\n\nkept at path=.draft/plan.md today\n")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains(
            "internal-path:3 .draft/plan.md references the internal .draft/ tree",
        ));
    rk().args(["message", "--check", "--target"])
        .arg(dir.path())
        .write_stdin("feat(api): add\n\nSee .draft/r\u{e9}sum\u{e9}.md here\n")
        .assert()
        .failure()
        .code(1)
        .stdout(predicate::str::contains("is git-ignored"));
}

/// A nested ignored path is one finding, not a git-ignored one plus a
/// fixed-pattern duplicate.
#[test]
fn a_nested_draft_reference_is_reported_once() {
    let dir = message_fixture();
    std::fs::create_dir(dir.path().join("nested")).expect("the nested dir exists");
    let out = rk()
        .args(["message", "--json", "--target"])
        .arg(dir.path())
        .write_stdin("feat(api): add\n\nSee nested/.draft/plan.md here\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    let findings = report["findings"].as_array().expect("findings is an array");
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(
        findings[0]["detail"]
            .as_str()
            .expect("detail is a string")
            .contains("is git-ignored"),
        "the repository judgment wins"
    );
}

/// A drifted squash message source is proven drift on an otherwise clean
/// trunk: the title source is owned, the body source is not, and the
/// check names the setting.
#[test]
fn check_reports_a_drifted_squash_message_source() {
    let fixture = ForgeFixture::new();
    fixture.seed("default_branch", "master");
    fixture.seed("rulesets.index", "master-protection\n");
    fixture.seed(
        "ruleset_master-protection",
        r#"{
  "name": "master-protection",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": {
    "ref_name": { "include": ["refs/heads/master"], "exclude": [] }
  },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "pull_request",
      "parameters": { "required_approving_review_count": 0, "allowed_merge_methods": ["squash"] }
    },
    {
      "type": "required_status_checks",
      "parameters": { "required_status_checks": [{ "context": "test" }, { "context": "pr-title" }] }
    }
  ]
}"#,
    );
    fixture.seed("squash_merge_commit_title", "PR_TITLE");
    fixture.seed("squash_merge_commit_message", "BLANK");
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("the squash message source is \"BLANK\" where the setup owns PR_BODY"),
        "the drifted body source is named: {text}"
    );
    assert!(
        !text.contains("the squash title source is"),
        "the owned title source is not faulted: {text}"
    );
}

/// The forge body gate greps a duplicated copy of the guard patterns —
/// it cannot read `blocks/message-guards` at run time — so every
/// non-comment guard line must appear verbatim, with its class, in the
/// embedded pr-title workflow bytes.
#[test]
fn the_forge_body_gate_carries_every_guard_pattern_verbatim() {
    let workflow = release_kit::embedded::SNIPPETS
        .get_file("_shared/github/.github/workflows/pr-title.yml")
        .expect("the shared workflow is embedded")
        .contents_utf8()
        .expect("the workflow is UTF-8");
    let patterns = release_kit::commands::message::guard_patterns();
    assert!(!patterns.is_empty(), "the guard file declares patterns");
    for (class, pattern) in patterns {
        assert!(
            workflow.contains(&format!("{class}|{pattern}")),
            "the body gate must carry {class}|{pattern} verbatim"
        );
    }
}

/// The second run block of the embedded shared pr-title workflow — the
/// body gate — dedented to a runnable script.
fn body_gate_script() -> String {
    let workflow = release_kit::embedded::SNIPPETS
        .get_file("_shared/github/.github/workflows/pr-title.yml")
        .expect("the shared workflow is embedded")
        .contents_utf8()
        .expect("the workflow is UTF-8");
    let mut blocks = workflow.split("run: |");
    let _ = blocks.next();
    let _ = blocks.next();
    let body = blocks
        .next()
        .expect("the workflow carries a second run block");
    body.lines()
        .skip(1)
        .take_while(|line| line.is_empty() || line.starts_with("          "))
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .fold(String::new(), |mut script, line| {
            script.push_str(line);
            script.push('\n');
            script
        })
}

/// Run the body gate as the forge would: bash, TITLE and BODY through the
/// environment, in a scratch directory.
fn run_body_gate(dir: &Path, title: &str, body: &str) -> (bool, String) {
    let script = body_gate_script();
    let out = std::process::Command::new("bash")
        .args(["-c", &script])
        .env("TITLE", title)
        .env("BODY", body)
        .current_dir(dir)
        .output()
        .expect("bash runs the gate");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// The forge body gate behaves: a clean body passes, attribution and an
/// internal path are refused naming their class, and the bot's request
/// passes whole with the very body an operator could not merge.
#[test]
fn the_forge_body_gate_refuses_the_guarded_content() {
    let dir = tempfile::tempdir().expect("a scratch dir exists");
    let (ok, _) = run_body_gate(
        dir.path(),
        "feat(api): add",
        "a clean body about src/main.rs",
    );
    assert!(ok, "a clean body passes");
    let (ok, out) = run_body_gate(
        dir.path(),
        "feat(api): add",
        "Generated with [Claude Code](https://claude.com/claude-code)",
    );
    assert!(!ok, "attribution is refused");
    assert!(out.contains("attribution"), "{out}");
    let (ok, out) = run_body_gate(dir.path(), "feat(api): add", "see .draft/plan.md");
    assert!(!ok, "an internal path is refused");
    assert!(out.contains("internal-path"), "{out}");
    // The same body proves the exemption: guarded content under a normal
    // title fails, and only the bot's title lets it pass whole.
    let attributed =
        "\u{1f916} Generated with release-plz\nsee .draft/release-notes.md\nCo-authored-by: Claude";
    let (ok, _) = run_body_gate(dir.path(), "feat(api): add", attributed);
    assert!(!ok, "the guarded body fails under an operator's title");
    let (ok, _) = run_body_gate(dir.path(), "chore: release v0.3.0", attributed);
    assert!(ok, "the bot's request passes whole");
}

/// Attacker-controlled title and body reach the gate through the
/// environment alone: shell metacharacters execute nothing.
#[test]
fn the_forge_body_gate_expands_no_body_content() {
    let dir = tempfile::tempdir().expect("a scratch dir exists");
    let (ok, _) = run_body_gate(
        dir.path(),
        "feat(api): $(touch title-pwned) `touch title-tick`",
        "$(touch body-pwned) `touch body-tick` && touch chained",
    );
    assert!(ok, "metacharacters are content, not commands");
    for probe in [
        "title-pwned",
        "title-tick",
        "body-pwned",
        "body-tick",
        "chained",
    ] {
        assert!(
            !dir.path().join(probe).exists(),
            "{probe}: body content was executed"
        );
    }
}

/// The gate's here-doc and `blocks/message-guards` hold the same set, in
/// both directions: no guard missing from the gate, no extra guard the
/// file does not declare.
#[test]
fn the_gate_and_the_guard_file_hold_the_same_set() {
    let script = body_gate_script();
    let heredoc: Vec<(&str, &str)> = script
        .lines()
        .skip_while(|line| !line.ends_with("<<'GUARDS'"))
        .skip(1)
        .take_while(|line| *line != "GUARDS")
        .map(|line| line.split_once('|').expect("a class|pattern line"))
        .collect();
    let declared = release_kit::commands::message::guard_patterns();
    assert_eq!(heredoc, declared, "the two copies must hold the same set");
}

/// SATISFIES forge-setup:the-setup-permits-a-request-to-merge-itself: the
/// forge with no project-level switch reports the pipeline requirement as
/// the stand-in, never a bare pass.
#[test]
fn a_gitlab_check_reports_the_auto_merge_limitation() {
    let fixture = ForgeFixture::new();
    fixture.seed("pipeline_required", "true");
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "gitlab"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("ok auto-merge") && text.contains("no project-level auto-merge switch"),
        "the check must name the stand-in for the missing switch: {text}"
    );
}

/// SATISFIES landing:the-arming-identity-is-the-bot: every release workflow
/// that arms the request gates it on the rendered style, authenticates the
/// arm as the bot — never the forge's default CI token — and re-arms in the
/// job that maintains the request, so a forge that replaces the request
/// re-arms the replacement.
#[test]
fn every_arming_step_authenticates_as_the_bot() {
    for path in [
        "snippets/rust/github/.github/workflows/release-plz.yml",
        "snippets/python/github/.github/workflows/release-please.yml",
        "snippets/bash/github/.github/workflows/release.yml",
    ] {
        let text = std::fs::read_to_string(repo_path(path)).expect("the snippet reads");
        assert!(
            text.contains("RELEASE_STYLE: RK_STYLE"),
            "{path}: the rendered style is the arm's gate"
        );
        assert!(
            text.contains("gh pr merge") && text.contains("--auto --squash --delete-branch"),
            "{path}: the arm sets auto-merge with the protected merge method"
        );
        let arm_at = text
            .find("arm the release request")
            .or_else(|| text.find("standing arm"))
            .expect("the arming step exists");
        let arm = &text[arm_at..];
        assert!(
            arm.contains("GH_TOKEN: ${{ steps.app-token.outputs.token }}"),
            "{path}: the arm authenticates as the bot"
        );
        assert!(
            arm.contains("a line's request is never armed"),
            "{path}: the arm must step aside off the trunk"
        );
        assert!(
            !arm.contains("secrets.GITHUB_TOKEN") && !arm.contains("github.token"),
            "{path}: an arm under the default token merges a bump that starts no workflow"
        );
    }
    for path in [
        "snippets/rust/gitlab/.gitlab-ci.yml",
        "snippets/bash/gitlab/.gitlab-ci.yml",
    ] {
        let text = std::fs::read_to_string(repo_path(path)).expect("the snippet reads");
        assert!(
            text.contains("RELEASE_STYLE: RK_STYLE"),
            "{path}: the rendered style is the arm's gate"
        );
        assert!(
            text.contains("auto_merge=true&merge_when_pipeline_succeeds=true"),
            "{path}: the arm sets auto-merge under both parameter names"
        );
        assert!(
            !text.contains("CI_JOB_TOKEN"),
            "{path}: an arm under the job token merges a bump that releases nothing"
        );
        let arm_at = text.find("standing arm").expect("the arming block exists");
        assert!(
            text[arm_at..].contains("PRIVATE-TOKEN: $RELEASE_BOT_TOKEN"),
            "{path}: the arm authenticates as the bot"
        );
        assert!(
            text[arm_at..].contains("a line's request is never armed"),
            "{path}: the arm must step aside off the trunk"
        );
    }
}

/// SATISFIES landing:the-release-style-is-a-landing-parameter: the recorded
/// style renders into the landed release workflow as the one substituted
/// value, so a style change is a one-word reviewed diff and the lines style
/// arms nothing.
#[test]
fn init_renders_the_recorded_style_into_the_release_workflow() {
    let trunk = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(trunk.path()).success();
    let workflow = std::fs::read_to_string(trunk.path().join(".github/workflows/release-plz.yml"))
        .expect("the landed workflow reads");
    assert!(
        workflow.contains("RELEASE_STYLE: trunk"),
        "the default style arms the request: {workflow}"
    );
    assert!(
        !workflow.contains("RK_STYLE"),
        "no style token survives rendering"
    );

    let lines = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "rust", "--forge", "github"])
        .args([
            "--repo",
            "acme/widget",
            "--scopes",
            "api,cli",
            "--style",
            "lines",
            "--target",
        ])
        .arg(lines.path())
        .arg("--apply")
        .assert()
        .success();
    let workflow = std::fs::read_to_string(lines.path().join(".github/workflows/release-plz.yml"))
        .expect("the landed workflow reads");
    assert!(
        workflow.contains("RELEASE_STYLE: lines"),
        "the lines style renders unarmed"
    );
    let manifest = read_manifest(lines.path());
    assert_eq!(manifest["parameters"]["style"], "lines");
}

/// Git against a scratch repository with the hook environment scrubbed:
/// the suite itself sometimes runs under a commit hook, and the exported
/// GIT_* variables would otherwise point every child at the outer
/// repository.
fn scrubbed_git(dir: &Path, args: &[&str]) -> std::process::Output {
    let mut command = std::process::Command::new("git");
    for var in [
        "GIT_COMMON_DIR",
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_WORK_TREE",
    ] {
        command.env_remove(var);
    }
    command
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("git runs")
}

/// One scratch repository with a trunk, a tag, and a release line —
/// the fixture the `rk lines` verbs read and retire.
fn line_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch repo exists");
    let run = |args: &[&str]| {
        let out = scrubbed_git(dir.path(), args);
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["init", "-q", "-b", "master"]);
    run(&["config", "user.name", "t"]);
    run(&["config", "user.email", "t@invalid"]);
    std::fs::write(dir.path().join("f"), "1\n").expect("writes");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "feat(api): one"]);
    run(&["tag", "-a", "v1.1.0", "-m", "v1.1.0"]);
    run(&["branch", "release/1.1", "v1.1.0"]);
    std::fs::write(dir.path().join("f"), "2\n").expect("writes");
    run(&["add", "."]);
    run(&["commit", "-q", "-m", "feat(api): two"]);
    dir
}

/// SATISFIES maintenance:a-line-is-cut-from-an-explicit-base: no base, no
/// line, and the refusal names the flag and the tag form.
#[test]
fn lines_open_refuses_without_a_base_and_adopts_an_existing_line() {
    let repo = line_repo();
    rk().args(["lines", "open", "1.2", "--target"])
        .arg(repo.path())
        .assert()
        .code(64)
        .stderr(predicate::str::contains("--base").and(predicate::str::contains("v<version>")));
    rk().args(["lines", "open", "9.9", "--base", "vnope", "--target"])
        .arg(repo.path())
        .arg("--apply")
        .assert()
        .code(73);
    // An existing line adopts and reports satisfied.
    rk().args(["lines", "open", "1.1", "--base", "v1.1.0", "--target"])
        .arg(repo.path())
        .arg("--apply")
        .assert()
        .success()
        .stdout(predicate::str::contains("satisfied"));
    // A fresh line previews, then lands at its base.
    rk().args(["lines", "open", "1.2", "--base", "v1.1.0", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"));
    rk().args(["lines", "open", "1.2", "--base", "v1.1.0", "--target"])
        .arg(repo.path())
        .arg("--apply")
        .assert()
        .success()
        .stdout(predicate::str::contains("created release/1.2"));
}

/// The inventory reports each line's tags, coverage, and seat, offline.
#[test]
fn lines_list_reports_the_inventory_offline() {
    let repo = line_repo();
    let out = rk()
        .args(["lines", "list", "--json", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.lines-list/1");
    let row = &report["lines"][0];
    assert_eq!(row["branch"], "release/1.1");
    assert_eq!(row["presence"], "local");
    assert_eq!(row["newest_release"], "v1.1.0");
    assert_eq!(row["tag_covered"], true);
}

/// `rc` reads the newest candidate and the next single-use number, and
/// never authors a tag.
#[test]
fn lines_rc_reports_the_newest_candidate_and_the_next_number() {
    let repo = line_repo();
    let git = |args: &[&str]| {
        assert!(scrubbed_git(repo.path(), args).status.success());
    };
    git(&["tag", "-a", "v1.1.1-rc.1", "-m", "rc", "release/1.1"]);
    git(&["tag", "-a", "v1.1.1-rc.2", "-m", "rc", "release/1.1"]);
    let out = rk()
        .args(["lines", "rc", "1.1", "--json", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.lines-rc/1");
    assert_eq!(report["newest_candidate"], "v1.1.1-rc.2");
    assert_eq!(report["next_candidate"], 3);
    assert_eq!(report["newest_release"], "v1.1.0");
}

/// SATISFIES maintenance:a-line-is-never-retired-before-its-tags: an
/// uncovered commit refuses the retirement, tagging it makes the same run
/// pass, and the remote deletion stays the operator's.
#[test]
fn lines_retire_refuses_on_an_untagged_commit_and_needs_apply() {
    let repo = line_repo();
    let git = |args: &[&str]| {
        assert!(scrubbed_git(repo.path(), args).status.success());
    };
    // A line-only commit no tag reaches.
    git(&["checkout", "-q", "release/1.1"]);
    std::fs::write(repo.path().join("g"), "x\n").expect("writes");
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "fix(api): crossed"]);
    git(&["checkout", "-q", "master"]);
    rk().args(["lines", "retire", "1.1", "--target"])
        .arg(repo.path())
        .arg("--apply")
        .assert()
        .code(73)
        .stderr(predicate::str::contains("no tag reaches"));
    // An unrelated tag reaching the tip authorizes nothing: only the
    // line's own v<line>.* tags make a retirement safe.
    git(&["tag", "-a", "v9.9.9", "-m", "unrelated", "release/1.1"]);
    rk().args(["lines", "retire", "1.1", "--target"])
        .arg(repo.path())
        .arg("--apply")
        .assert()
        .code(73)
        .stderr(predicate::str::contains("no tag reaches"));
    // Tagged, the preview reports and writes nothing; the apply deletes.
    git(&["tag", "-a", "v1.1.1", "-m", "v1.1.1", "release/1.1"]);
    rk().args(["lines", "retire", "1.1", "--target"])
        .arg(repo.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("would delete release/1.1"));
    rk().args(["lines", "retire", "1.1", "--target"])
        .arg(repo.path())
        .arg("--apply")
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted release/1.1"))
        .stdout(predicate::str::contains(
            "git push origin --delete release/1.1",
        ));
    let gone = scrubbed_git(
        repo.path(),
        &["show-ref", "--verify", "--quiet", "refs/heads/release/1.1"],
    );
    assert!(!gone.status.success(), "the local branch is gone");
}

/// SATISFIES distribution:machine-output-declares-its-schema for the open
/// verb: one schema on every path, the workflow mode included, with the
/// seat named as the worktree verb's own next action.
#[test]
fn lines_open_json_keeps_its_own_schema() {
    let repo = line_repo();
    let out = rk()
        .args([
            "lines", "open", "1.3", "--base", "v1.1.0", "--json", "--target",
        ])
        .arg(repo.path())
        .arg("--apply")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["schema"], "rk.lines-open/1");
    assert_eq!(report["mode"], "created");
    assert!(
        report["next"][0]
            .as_str()
            .is_some_and(|line| line.contains("rk worktree add release/1.3")),
        "the seat is the worktree verb's job, named as the next action: {report}"
    );
}

/// A style override rides into the replayed apply command, so following
/// the preview's own next line applies the previewed decision.
#[test]
fn upgrade_preview_replays_the_style_override() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    rk().args(["upgrade", "--style", "lines", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("--style lines"));
}

/// A remote-only line adopts the remote tip as a tracking branch instead
/// of being recreated from the supplied base, which could sit behind it.
#[test]
fn lines_open_adopts_a_remote_only_line_at_its_tip() {
    let origin = line_repo();
    let clone = tempfile::tempdir().expect("a scratch clone exists");
    let cloned = scrubbed_git(
        clone.path(),
        &[
            "clone",
            "-q",
            origin.path().to_str().expect("utf-8"),
            "work",
        ],
    );
    assert!(cloned.status.success());
    let work = clone.path().join("work");
    rk().args([
        "lines", "open", "1.1", "--base", "v1.1.0", "--json", "--target",
    ])
    .arg(&work)
    .arg("--apply")
    .assert()
    .success()
    .stdout(predicate::str::contains("rk.lines-open/1"));
    let tracked = scrubbed_git(&work, &["rev-parse", "release/1.1", "origin/release/1.1"]);
    let out = String::from_utf8_lossy(&tracked.stdout);
    let mut shas = out.lines();
    assert_eq!(
        shas.next(),
        shas.next(),
        "the local line tracks the remote tip, not the base"
    );
}

/// The adopt preview's replayed apply command carries every parameter the
/// replay needs, so following it reproduces the previewed candidate.
#[test]
fn adopt_preview_replays_a_complete_apply_command() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    land_rust(target.path()).success();
    std::fs::remove_dir_all(target.path().join(".release-kit")).expect("the record removes");
    rk().args([
        "adopt", "--tech", "rust", "--forge", "github", "--scopes", "api,cli", "--style", "trunk",
    ])
    .args([
        "--workflow",
        "worktree",
        "--repo",
        "acme/widget",
        "--target",
    ])
    .arg(target.path())
    .assert()
    .success()
    .stdout(
        predicate::str::contains("--tech rust")
            .and(predicate::str::contains("--repo acme/widget"))
            .and(predicate::str::contains("--scopes api,cli"))
            .and(predicate::str::contains("--workflow worktree"))
            .and(predicate::str::contains("--style trunk")),
    );
}

/// Every third-party destination the payload names — the two agent skill
/// roots — is justified in `_docs/reference/` by a dated citation to the
/// owning application's documentation, and the recorded matrix matches the
/// roots the code declares, per
/// placement:a-third-party-destination-names-its-source.
#[test]
fn every_third_party_destination_names_its_source() {
    let reference = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("_docs/reference/REFERENCE-skill-scopes-sources.md"),
    )
    .expect("the skill scopes reference reads");
    for root in [
        release_kit::skills::CLAUDE_ROOT,
        release_kit::skills::AGENTS_ROOT,
    ] {
        assert!(
            reference.contains(root),
            "the reference records the destination {root}"
        );
    }
    assert!(
        reference.contains("Verified against the listed sources on 20"),
        "the reference carries a dated verification line"
    );
    for source in [
        "code.claude.com",
        "learn.chatgpt.com",
        "gemini-cli",
        "docs.github.com",
    ] {
        assert!(
            reference.contains(source),
            "the reference cites the owning application's documentation at {source}"
        );
    }
}

/// A single-crate shape the seeded package expression supports: the
/// `[package]` table, the implicit binary, and the committed lock the
/// seed builds from.
fn seed_crate(target: &Path) {
    std::fs::write(
        target.join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n",
    )
    .expect("the crate manifest writes");
    std::fs::create_dir_all(target.join("src")).expect("the src dir exists");
    std::fs::write(target.join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    std::fs::write(target.join("Cargo.lock"), "version = 4\n").expect("the lock writes");
}

/// Land the rust payload with the Nix capability opted in.
fn land_rust_nix(target: &Path) -> assert_cmd::assert::Assert {
    rk().args(["init", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--scopes", "api,cli", "--nix"])
        .arg("--target")
        .arg(target)
        .arg("--apply")
        .assert()
}

/// The opt-in lands the capability with its kinds recorded, the record
/// carries the parameter, and the state file is never compared.
#[test]
fn the_nix_opt_in_lands_the_capability_with_its_kinds() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust_nix(target.path()).success();
    for name in ["nix/package.nix", "flake.nix", "flake.lock"] {
        assert!(target.path().join(name).is_file(), "{name} lands");
    }
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["nix"], true);
    assert_eq!(
        manifest_file(&manifest, "nix/package.nix")["kind"],
        "seeded"
    );
    assert_eq!(manifest_file(&manifest, "flake.nix")["kind"], "seeded");
    assert_eq!(manifest_file(&manifest, "flake.lock")["kind"], "state");
    // The state file is the automation's: editing it is never drift.
    std::fs::write(target.path().join("flake.lock"), "{}\n").expect("the lock rewrites");
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["nix"], true);
    assert_eq!(report["missing"], serde_json::json!([]));
    assert_eq!(report["drift"]["rendered"], 0);
    assert_eq!(
        report["drift"]["seeded"], 0,
        "an edited state file is never compared"
    );
}

/// Off is the default and the record says so explicitly: no Nix file
/// lands, nothing is missing, and the report field is false.
#[test]
fn a_landing_without_nix_lands_none_and_the_record_says_so() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust(target.path()).success();
    for name in ["nix/package.nix", "flake.nix", "flake.lock"] {
        assert!(!target.path().join(name).exists(), "{name} must not land");
    }
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["nix"], false);
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["nix"], false);
    assert_eq!(
        report["missing"],
        serde_json::json!([]),
        "an absent-because-not-wanted file is never missing"
    );
}

/// A target with a flake of its own keeps the pair: the seeded package
/// expression lands, the pair and the workflow are withheld with the
/// reason stated, the preview says the same, and a later upgrade
/// reproduces the decision instead of reporting drift.
#[test]
fn a_target_with_its_own_flake_keeps_it_and_the_pair_is_withheld() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    let own_flake = "{ description = \"the target's own\"; }\n";
    std::fs::write(target.path().join("flake.nix"), own_flake).expect("the flake writes");
    rk().args(["init", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--scopes", "api,cli", "--nix"])
        .arg("--target")
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("withheld flake.lock"));
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains("withheld flake.nix"));
    assert!(target.path().join("nix/package.nix").is_file());
    assert!(!target.path().join("flake.lock").exists());
    assert_eq!(
        std::fs::read_to_string(target.path().join("flake.nix")).expect("the flake reads"),
        own_flake,
        "the target's own flake survives byte-identically"
    );
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["nix"], true);
    assert!(
        manifest["files"]
            .as_array()
            .expect("a file list")
            .iter()
            .all(|file| file["destination"] != "flake.nix" && file["destination"] != "flake.lock"),
        "a withheld destination stays out of the record"
    );
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["missing"], serde_json::json!([]));
    assert_eq!(report["drift"]["rendered"], 0);
    rk().args(["upgrade", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("withheld"));
}

/// The opt-in works after the fact in both directions: `--nix` adds the
/// files and records it, `--no-nix` drops them from the record while the
/// files stay the target's own.
#[test]
fn an_upgrade_moves_the_nix_opt_in_in_both_directions() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust(target.path()).success();
    rk().args(["upgrade", "--nix", "on", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success();
    assert!(target.path().join("flake.nix").is_file());
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["nix"], true);
    assert_eq!(manifest_file(&manifest, "flake.lock")["kind"], "state");
    rk().args(["upgrade", "--nix", "off", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("dropped flake.nix"));
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["nix"], false);
    assert!(
        target.path().join("flake.nix").is_file(),
        "an opt-out leaves the file as the target's own"
    );
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(
        report["missing"],
        serde_json::json!([]),
        "a dropped destination leaves the record whole"
    );
}

/// A record from before the parameter existed reads as opt-out: the
/// upgrade adds no Nix file nobody requested.
#[test]
fn a_pre_nix_record_upgrades_to_nothing_unrequested() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    manifest["schema_version"] = serde_json::json!(3);
    manifest["parameters"]
        .as_object_mut()
        .expect("parameters is an object")
        .remove("nix");
    write_manifest(target.path(), &manifest);
    let out = rk()
        .args(["upgrade", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["nix"], false);
    assert!(
        report["files"]
            .as_array()
            .expect("a file list")
            .iter()
            .all(|file| file["path"] != "flake.nix" && file["path"] != "nix/package.nix"),
        "no Nix destination joins an upgrade nobody opted into"
    );
}

/// A crate shape the seed does not support withholds the whole
/// capability by name, and the landing reports the smaller product.
#[test]
fn a_workspace_root_withholds_the_whole_nix_capability() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"widget\"]\n",
    )
    .expect("the workspace manifest writes");
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains("withheld nix/package.nix"));
    for name in ["nix/package.nix", "flake.nix", "flake.lock"] {
        assert!(!target.path().join(name).exists(), "{name} must not land");
    }
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["parameters"]["nix"], true);
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(
        report["missing"],
        serde_json::json!([]),
        "a withheld capability reports nothing missing"
    );
}

/// A tuned seed survives an upgrade untouched and reports as the
/// target's own drift, never a conflict.
#[test]
fn a_tuned_nix_seed_survives_an_upgrade() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust_nix(target.path()).success();
    let tuned = "# tuned by the target\n{ lib, rustPlatform }: null\n";
    std::fs::write(target.path().join("nix/package.nix"), tuned).expect("the tune writes");
    rk().args(["upgrade", "--apply", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "drift nix/package.nix (seeded, target-owned)",
        ));
    assert_eq!(
        std::fs::read_to_string(target.path().join("nix/package.nix")).expect("the seed reads"),
        tuned,
        "the tune survives the upgrade byte-identically"
    );
}

/// An adoption records a Nix landing: the candidate includes the
/// capability, and a flake pair with no record to vouch for it reads as
/// the target's own — withheld from the candidate exactly as a landing
/// would withhold it, because blessing the disk would launder any pair.
#[test]
fn an_adoption_records_the_nix_parameter() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust_nix(target.path()).success();
    std::fs::remove_file(target.path().join(".release-kit/manifest.json"))
        .expect("the record removes");
    rk().args(["adopt", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--scopes", "api,cli"])
        .args(["--workflow", "worktree", "--style", "trunk", "--nix"])
        .arg("--apply")
        .arg("--target")
        .arg(target.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("withheld flake.nix"));
    let manifest = read_manifest(target.path());
    assert_eq!(manifest["origin"], "adopt");
    assert_eq!(manifest["parameters"]["nix"], true);
    assert_eq!(
        manifest_file(&manifest, "nix/package.nix")["kind"],
        "seeded"
    );
    assert!(
        manifest["files"]
            .as_array()
            .expect("a file list")
            .iter()
            .all(|file| file["destination"] != "flake.nix"),
        "a pair no record vouches for stays the target's own"
    );
}

/// Every action the rendered nix workflow launches is pinned by a full
/// commit that appears exactly once in `versions.toml`, with a discovery
/// ref, a freshness URL, and a checked date beside it. Without this test
/// the pin doctrine holds only as long as whoever edits the workflow
/// remembers it.
#[test]
fn the_nix_workflow_pins_resolve_through_the_registry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow =
        std::fs::read_to_string(root.join("snippets/rust/github/.github/workflows/nix.yml"))
            .expect("the workflow snippet reads");
    let registry = std::fs::read_to_string(root.join("versions.toml")).expect("the registry reads");
    let table: toml::Table = registry.parse().expect("the registry parses");
    let tools = table["tool"].as_array().expect("a tool list");
    let mut pinned = 0;
    for line in workflow.lines() {
        let Some(reference) = line.trim().strip_prefix("- uses: ") else {
            continue;
        };
        pinned += 1;
        let (action, comment) = reference
            .split_once(" # ")
            .expect("a discovery comment follows the pin");
        let (_, commit) = action.split_once('@').expect("an @commit pin");
        assert_eq!(commit.len(), 40, "{action} is not pinned by a full commit");
        assert!(
            commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{action} is not a commit"
        );
        assert!(
            !comment.trim().is_empty(),
            "{action} names no discovery ref"
        );
        let owners: Vec<&toml::Value> = tools
            .iter()
            .filter(|tool| tool.get("commit").and_then(toml::Value::as_str) == Some(commit))
            .collect();
        assert_eq!(
            owners.len(),
            1,
            "{action}: the registry must own this commit exactly once"
        );
        for field in ["action", "check", "checked"] {
            assert!(
                owners[0].get(field).is_some(),
                "{action}: the registry entry carries no {field}"
            );
        }
    }
    assert_eq!(pinned, 3, "the workflow launches three pinned actions");
}

/// The landed capability builds, end to end: a scratch crate opts in and
/// the seed flake compiles `nix/package.nix` through the same named build
/// the rendered workflow runs. Ignored by default — it needs the nix CLI
/// and network access — and run by the build recipe beside the
/// publish-closure proof.
#[test]
#[ignore = "needs the nix CLI and network access; just check runs it"]
fn the_landed_nix_capability_builds_end_to_end() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("the crate manifest writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    let scrubbed = |program: &str| {
        let mut command = std::process::Command::new(program);
        for var in GIT_HOOK_VARS {
            command.env_remove(var);
        }
        command.current_dir(target.path());
        command
    };
    assert!(
        scrubbed("cargo")
            .args(["generate-lockfile", "--offline"])
            .status()
            .expect("cargo runs")
            .success()
    );
    land_rust_nix(target.path()).success();
    assert!(
        scrubbed("git")
            .args(["init", "-q"])
            .status()
            .expect("git runs")
            .success()
    );
    assert!(
        scrubbed("git")
            .args(["add", "-A"])
            .status()
            .expect("git runs")
            .success()
    );
    let build = scrubbed("nix")
        .args(["build", ".#default", "--no-link"])
        .status()
        .expect("the nix CLI runs");
    assert!(
        build.success(),
        "the landed flake must build the seeded package"
    );
}

/// The shape gate holds every structural prerequisite the seed relies
/// on: a lib-only crate and a crate without a committed lock are each
/// withheld with the missing piece named, because the seed would build
/// nothing runnable or throw on first evaluation.
#[test]
fn a_crate_the_seed_cannot_build_withholds_the_capability_by_name() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n",
    )
    .expect("the crate manifest writes");
    std::fs::write(target.path().join("Cargo.lock"), "version = 4\n").expect("the lock writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/lib.rs"), "\n").expect("the lib writes");
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains("declares no binary"));
    assert!(!target.path().join("nix/package.nix").exists());

    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n",
    )
    .expect("the crate manifest writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains("no Cargo.lock"));
    assert!(!target.path().join("nix/package.nix").exists());
}

/// A record whose parameters and file list disagree is drift the check
/// judges: flipping the recorded nix flag without landing a file, and a
/// once-withheld capability whose target grew into the supported shape,
/// both surface instead of reading as clean.
#[test]
fn a_record_whose_parameters_and_files_disagree_is_drift() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    manifest["parameters"]["nix"] = serde_json::json!(true);
    write_manifest(target.path(), &manifest);
    let out = rk()
        .args(["status", "--check", "--json", "--target"])
        .arg(target.path())
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert!(
        report["violations"]
            .as_array()
            .expect("a violation list")
            .iter()
            .any(|violation| {
                violation
                    .as_str()
                    .is_some_and(|line| line.contains("record drift") && line.contains("flake.nix"))
            }),
        "{report}"
    );

    // The other direction: an opted-in workspace grew into a supported
    // crate, so the recorded withhold no longer reproduces.
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"widget\"]\n",
    )
    .expect("the workspace manifest writes");
    land_rust_nix(target.path()).success();
    seed_crate(target.path());
    rk().args(["status", "--check", "--target"])
        .arg(target.path())
        .assert()
        .failure()
        .stdout(predicate::str::contains("DRIFT record"));
}

/// The shape gate refuses a package whose binary is only nominal: an
/// autobins = false library keeping a stray src/main.rs, and a first
/// [[bin]] entry gated behind required-features, each withheld by name.
#[test]
fn a_nominal_binary_does_not_pass_the_shape_gate() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\nautobins = false\n",
    )
    .expect("the crate manifest writes");
    std::fs::write(target.path().join("Cargo.lock"), "version = 4\n").expect("the lock writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains("declares no binary"));
    assert!(!target.path().join("nix/package.nix").exists());

    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n\n[features]\nextra = []\n\n[[bin]]\nname = \"widget\"\npath = \"src/main.rs\"\nrequired-features = [\"extra\"]\n",
    )
    .expect("the crate manifest writes");
    std::fs::write(target.path().join("Cargo.lock"), "version = 4\n").expect("the lock writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains(
            "requires features a default build does not enable",
        ));
    assert!(!target.path().join("nix/package.nix").exists());
}

/// Record drift carries its own count in the report: no file was edited,
/// so the kind counts stay honest while the disagreement is visible.
#[test]
fn record_drift_counts_apart_from_file_drift() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    seed_crate(target.path());
    land_rust(target.path()).success();
    let mut manifest = read_manifest(target.path());
    manifest["parameters"]["nix"] = serde_json::json!(true);
    write_manifest(target.path(), &manifest);
    let out = rk()
        .args(["status", "--json", "--target"])
        .arg(target.path())
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&out).expect("one JSON object");
    assert_eq!(report["drift"]["rendered"], 0, "{report}");
    assert_eq!(report["drift"]["seeded"], 0, "{report}");
    assert!(
        report["record_drift"]
            .as_u64()
            .is_some_and(|count| count > 0),
        "{report}"
    );
}

/// A first [[bin]] entry whose required features the default set enables
/// — directly or through a feature the default implies — passes the
/// shape gate; the gate withholds only what a default build truly skips.
#[test]
fn default_enabled_required_features_pass_the_shape_gate() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n\n[features]\ndefault = [\"full\"]\nfull = [\"cli\"]\ncli = []\n\n[[bin]]\nname = \"widget\"\npath = \"src/main.rs\"\nrequired-features = [\"cli\"]\n",
    )
    .expect("the crate manifest writes");
    std::fs::write(target.path().join("Cargo.lock"), "version = 4\n").expect("the lock writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    land_rust_nix(target.path()).success();
    assert!(target.path().join("nix/package.nix").is_file());
}

/// A strong dependency-feature edge in the default set — `serde/derive`
/// enabling the optional `serde` and its implicit same-named feature —
/// satisfies a binary's requirement, so the gate lands the capability.
#[test]
fn a_strong_dependency_edge_satisfies_a_required_feature() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = { version = \"1\", optional = true }\n\n[features]\ndefault = [\"serde/derive\"]\n\n[[bin]]\nname = \"widget\"\npath = \"src/main.rs\"\nrequired-features = [\"serde\"]\n",
    )
    .expect("the crate manifest writes");
    std::fs::write(target.path().join("Cargo.lock"), "version = 4\n").expect("the lock writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    land_rust_nix(target.path()).success();
    assert!(target.path().join("nix/package.nix").is_file());
}

/// A strong dependency edge on a non-optional dependency enables no
/// local feature: the requirement stays unmet and the gate withholds.
#[test]
fn a_non_optional_dependency_edge_satisfies_nothing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n\n[features]\ndefault = [\"serde/derive\"]\nserde = []\n\n[[bin]]\nname = \"widget\"\npath = \"src/main.rs\"\nrequired-features = [\"serde\"]\n",
    )
    .expect("the crate manifest writes");
    std::fs::write(target.path().join("Cargo.lock"), "version = 4\n").expect("the lock writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains(
            "requires features a default build does not enable",
        ));
    assert!(!target.path().join("nix/package.nix").exists());
}

/// A `dep:` edge anywhere in the feature table suppresses the optional
/// dependency's implicit feature, so the strong edge activates nothing
/// and the gate withholds.
#[test]
fn a_dep_edge_suppresses_the_implicit_feature() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(
        target.path().join("Cargo.toml"),
        "[package]\nname = \"widget\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = { version = \"1\", optional = true }\n\n[features]\ndefault = [\"serde/derive\"]\ninternal = [\"dep:serde\"]\n\n[[bin]]\nname = \"widget\"\npath = \"src/main.rs\"\nrequired-features = [\"serde\"]\n",
    )
    .expect("the crate manifest writes");
    std::fs::write(target.path().join("Cargo.lock"), "version = 4\n").expect("the lock writes");
    std::fs::create_dir_all(target.path().join("src")).expect("the src dir exists");
    std::fs::write(target.path().join("src/main.rs"), "fn main() {}\n").expect("the main writes");
    land_rust_nix(target.path())
        .success()
        .stdout(predicate::str::contains(
            "requires features a default build does not enable",
        ));
    assert!(!target.path().join("nix/package.nix").exists());
}

// ---------------------------------------------------------------------------
// rk devshell

/// The nix stand-in: logs every argv, answers the system probe and the
/// version call, writes a lock naming the pinned tag on a flake update,
/// and fails the verb a seeded file names.
const MOCK_NIX: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/nix-log"
if [[ "$1" == "--version" ]]; then echo "nix (Nix) 2.34.8"; exit 0; fi
if [[ "$*" == "eval --raw --impure --expr builtins.currentSystem" ]]; then printf 'x86_64-linux'; exit 0; fi
if [[ "$1" == "flake" ]]; then
  if [[ -f "$STATE/nix_fail_update" ]]; then echo "error: could not update the lock" >&2; exit 1; fi
  tag=$(sed -n 's/.*release-kit\/\(v[^"]*\)".*/\1/p' flake.nix | head -1)
  printf '{"nodes":{"release-kit":{"locked":{"rev":"rev-of-%s","ref":"refs/tags/%s"}},"root":{}},"version":7}\n' "$tag" "$tag" > flake.lock
  exit 0
fi
if [[ "$1" == "build" ]]; then
  if [[ -f "$STATE/nix_fail_build" ]]; then echo "error: builder failed" >&2; exit 1; fi
  exit 0
fi
exit 0
"#;

/// The curl stand-in for the release redirect: logs argv, fails on
/// demand, and prints the release URL of the seeded latest tag.
const MOCK_RELEASE_CURL: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/curl-log"
if [[ -f "$STATE/curl_fail" ]]; then
  echo "curl: (6) Could not resolve host: github.com" >&2
  exit 6
fi
tag=$(cat "$STATE/latest_tag" 2>/dev/null || echo v0.2.16)
printf 'https://github.com/owner/release-kit/releases/tag/%s' "$tag"
"#;

/// The direnv stand-in: logs argv and answers the version call.
const MOCK_DIRENV: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/direnv-log"
echo "2.32.0"
"#;

/// One devshell fixture: a scratch home that is also the state root, a
/// scratch target under git, and the three mocked tools.
struct DevshellFixture {
    home: tempfile::TempDir,
    target: tempfile::TempDir,
    mock: tempfile::TempDir,
}

impl DevshellFixture {
    fn new() -> Self {
        let fixture = Self {
            home: tempfile::tempdir().expect("a scratch home exists"),
            target: tempfile::tempdir().expect("a scratch target exists"),
            mock: tempfile::tempdir().expect("a scratch mock dir exists"),
        };
        for (name, body) in [
            ("nix", MOCK_NIX),
            ("curl", MOCK_RELEASE_CURL),
            ("direnv", MOCK_DIRENV),
        ] {
            let path = fixture.mock.path().join(name);
            std::fs::write(
                &path,
                body.replace("__STATE__", &fixture.mock.path().to_string_lossy()),
            )
            .expect("the mock writes");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .expect("the mock is executable");
            }
        }
        git_in(fixture.target.path(), &["init", "-q", "-b", "master"]);
        git_in(
            fixture.target.path(),
            &["config", "user.email", "rk@example.invalid"],
        );
        git_in(fixture.target.path(), &["config", "user.name", "rk test"]);
        fixture
    }

    fn target(&self) -> &Path {
        self.target.path()
    }

    fn state(&self, name: &str) -> String {
        std::fs::read_to_string(self.mock.path().join(name)).unwrap_or_default()
    }

    fn nix_log(&self) -> String {
        self.state("nix-log")
    }

    fn curl_log(&self) -> String {
        self.state("curl-log")
    }

    /// A wired flake pinned at `tag`, and a lock naming it.
    fn write_flake(&self, tag: &str) {
        std::fs::write(
            self.target().join("flake.nix"),
            format!(
                "{{\n  inputs = {{\n    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n    release-kit = {{\n      url = \"github:gubasso/release-kit/{tag}\";\n      inputs.nixpkgs.follows = \"nixpkgs\";\n    }};\n  }};\n  outputs = {{ self, nixpkgs, release-kit }}: {{ }};\n}}\n"
            ),
        )
        .expect("the flake writes");
        std::fs::write(
            self.target().join("flake.lock"),
            format!(
                "{{\"nodes\":{{\"release-kit\":{{\"locked\":{{\"rev\":\"rev-of-{tag}\",\"ref\":\"refs/tags/{tag}\"}}}},\"root\":{{}}}},\"version\":7}}\n"
            ),
        )
        .expect("the lock writes");
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.target().join(rel)).expect("the file reads")
    }

    fn seed(&self, name: &str, value: &str) {
        std::fs::write(self.mock.path().join(name), value).expect("the state seeds");
    }

    fn commit_all(&self) {
        git_in(self.target(), &["add", "-A"]);
        git_in(self.target(), &["commit", "-qm", "chore: seed"]);
    }

    /// The per-checkout state directory under the scratch state root.
    fn state_dir(&self) -> PathBuf {
        self.home.path().join("release-kit/devshell")
    }

    fn key(&self) -> String {
        let canonical = std::fs::canonicalize(self.target()).expect("the target canonicalizes");
        release_kit::devshell::state_key(&utf8(&canonical))
    }

    /// A sync run's parsed report, whatever its exit code.
    fn sync_json(&self, args: &[&str]) -> (Option<i32>, serde_json::Value) {
        let out = self.rk(args).arg("--json").output().expect("rk runs");
        let report = serde_json::from_slice(&out.stdout).unwrap_or_else(|_| {
            panic!(
                "one JSON object expected on stdout: {}\n{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        });
        (out.status.code(), report)
    }

    /// The hand-rolled recipe the catalog knows, planted whole: the two
    /// scripts, the two suites, the `.envrc` invocation with its comment,
    /// the switch example, the justfile recipe, the devshell tooling, and
    /// a host install line in the README and in a workflow.
    fn write_predecessor(&self) {
        let target = self.target();
        std::fs::create_dir_all(target.join("scripts")).expect("scripts creates");
        std::fs::create_dir_all(target.join("tests")).expect("tests creates");
        std::fs::create_dir_all(target.join(".github/workflows")).expect("workflows creates");
        std::fs::write(
            target.join("scripts/rk-bump.sh"),
            "#!/usr/bin/env bash\nPIN_PREFIX='github:gubasso/release-kit/'\nflock -n \"$LOCK\" true\n",
        )
        .expect("writes");
        std::fs::write(
            target.join("scripts/rk-autobump.sh"),
            "#!/usr/bin/env bash\nexec \"$(dirname \"$0\")/rk-bump.sh\"\n",
        )
        .expect("writes");
        std::fs::write(
            target.join("tests/rk-bump.bats"),
            "@test \"rk-bump moves the pin\" { true; }\n",
        )
        .expect("writes");
        std::fs::write(
            target.join("tests/rk-autobump.bats"),
            "@test \"rk-autobump stamps the day\" { true; }\n",
        )
        .expect("writes");
        std::fs::write(
            target.join(".envrc"),
            "use flake\n\n# Bump the release-kit pin once a day on entry.\n# scripts/rk-autobump.sh holds the transaction; RK_SKIP_AUTOBUMP=1 skips it.\nscripts/rk-autobump.sh || true\n\nsource_env_if_exists .envrc.local\n",
        )
        .expect("writes");
        std::fs::write(
            target.join(".envrc.local.example"),
            "# export RK_SKIP_AUTOBUMP=1\n",
        )
        .expect("writes");
        std::fs::write(
            target.join("justfile"),
            "# Move the release-kit pin by hand.\nrk-bump tag='':\n    scripts/rk-bump.sh {{tag}}\n\ntest:\n    bats tests\n",
        )
        .expect("writes");
        std::fs::write(
            target.join("flake.nix"),
            "{\n  inputs = {\n    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n    release-kit = {\n      url = \"github:gubasso/release-kit/v0.2.16\";\n      inputs.nixpkgs.follows = \"nixpkgs\";\n    };\n  };\n  outputs = { self, nixpkgs, release-kit }: {\n    devShells.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.mkShell {\n      packages = [\n        release-kit.packages.x86_64-linux.default\n        nixpkgs.legacyPackages.x86_64-linux.flock # for scripts/rk-bump.sh\n        nixpkgs.legacyPackages.x86_64-linux.bats\n      ];\n    };\n  };\n}\n",
        )
        .expect("writes");
        std::fs::write(
            target.join("README.md"),
            "# widget\n\nInstall the tool: `cargo install release-kit`.\n",
        )
        .expect("writes");
        std::fs::write(
            target.join(".github/workflows/ci.yml"),
            "jobs:\n  test:\n    steps:\n      - run: cargo binstall release-kit\n",
        )
        .expect("writes");
    }

    fn rk(&self, args: &[&str]) -> Command {
        let mut command = rk_scrubbed();
        command
            .env("HOME", self.home.path())
            .env("XDG_STATE_HOME", self.home.path())
            .env("RK_NIX_BIN", self.mock.path().join("nix"))
            .env("RK_CURL_BIN", self.mock.path().join("curl"))
            .env("RK_DIRENV_BIN", self.mock.path().join("direnv"))
            .env_remove("RK_DEVSHELL_SYNC");
        for var in [
            "CI",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "BUILDKITE",
            "CIRCLECI",
            "TF_BUILD",
        ] {
            command.env_remove(var);
        }
        command.args(args).arg("--target").arg(self.target());
        command
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        let out = self.rk(args).arg("--json").output().expect("rk runs");
        assert!(
            out.status.success(),
            "rk {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("one JSON object")
    }
}

#[test]
fn devshell_status_reports_a_target_with_no_flake() {
    let fixture = DevshellFixture::new();
    fixture
        .rk(&["devshell", "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("state no-flake")
                .and(predicate::str::contains("flake absent, lock absent"))
                .and(predicate::str::contains("rk devshell add")),
        );
    let report = fixture.json(&["devshell", "status"]);
    assert_eq!(report["schema"], "rk.devshell-status/1");
    assert_eq!(report["state"], "no-flake");
    assert_eq!(report["input"], "absent");
    assert!(
        report.get("pin_tag").is_none(),
        "an unknown value is omitted"
    );
    assert_eq!(report["pending"], false);
}

#[test]
fn devshell_status_reports_a_wired_target_offline() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.16");
    std::fs::write(
        fixture.target().join(".envrc"),
        "use flake\nrk devshell sync --apply || true\n",
    )
    .expect(".envrc writes");
    let report = fixture.json(&["devshell", "status"]);
    assert_eq!(report["state"], "ready");
    assert_eq!(report["input"], "pinned");
    assert_eq!(report["pin_tag"], "v0.2.16");
    assert_eq!(report["pin_lines"], 1);
    assert_eq!(report["locked_rev"], "rev-of-v0.2.16");
    assert_eq!(report["locked_ref"], "refs/tags/v0.2.16");
    assert_eq!(report["envrc"], "present");
    assert_eq!(report["envrc_sync"], true);
    assert_eq!(report["host"]["nix"], "ok");
    assert_eq!(report["host"]["direnv"], "ok");
    assert!(fixture.curl_log().is_empty(), "status fetches nothing");
    assert!(
        !fixture.nix_log().contains("flake") && !fixture.nix_log().contains("build"),
        "status spawns no nix beyond the probe: {}",
        fixture.nix_log()
    );
}

#[test]
fn devshell_status_names_an_ambiguous_pin_with_its_count() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.16");
    let flake = fixture.read("flake.nix");
    std::fs::write(
        fixture.target().join("flake.nix"),
        format!("{flake}  url = \"github:gubasso/release-kit/v0.2.15\";\n"),
    )
    .expect("the flake writes");
    fixture
        .rk(&["devshell", "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("state ambiguous-pin")
                .and(predicate::str::contains("2 lines name it")),
        );
    let report = fixture.json(&["devshell", "status"]);
    assert_eq!(report["input"], "ambiguous");
    assert_eq!(report["pin_lines"], 2);
    assert!(report.get("pin_tag").is_none());
}

#[test]
fn devshell_status_reports_the_two_host_probes() {
    let fixture = DevshellFixture::new();
    let report = fixture.json(&["devshell", "status"]);
    assert_eq!(report["host"]["nix"], "ok");
    let failed = {
        let out = fixture
            .rk(&["devshell", "status"])
            .arg("--json")
            .env("RK_NIX_BIN", "/no/such/nix")
            .env("RK_DIRENV_BIN", "/no/such/direnv")
            .output()
            .expect("rk runs");
        assert!(out.status.success(), "a failed probe is a result");
        serde_json::from_slice::<serde_json::Value>(&out.stdout).expect("one JSON object")
    };
    assert_eq!(failed["host"]["nix"], "failed");
    assert_eq!(failed["host"]["direnv"], "failed");
    // The doctor carries the same two probes, Soft, in catalog order after curl.
    let doctor = {
        let out = rk_scrubbed()
            .args(["doctor", "--json"])
            .env("HOME", fixture.home.path())
            .env("XDG_STATE_HOME", fixture.home.path())
            .env("RK_NIX_BIN", fixture.mock.path().join("nix"))
            .env("RK_DIRENV_BIN", fixture.mock.path().join("direnv"))
            .output()
            .expect("rk runs");
        serde_json::from_slice::<serde_json::Value>(&out.stdout).expect("one JSON object")
    };
    let ids: Vec<&str> = doctor["probes"]
        .as_array()
        .expect("probes")
        .iter()
        .map(|probe| probe["id"].as_str().expect("an id"))
        .collect();
    let curl = ids
        .iter()
        .position(|id| *id == "curl")
        .expect("curl probed");
    assert_eq!(ids[curl + 1], "nix");
    assert_eq!(ids[curl + 2], "direnv");
    for probe in doctor["probes"].as_array().expect("probes") {
        if probe["id"] == "nix" || probe["id"] == "direnv" {
            assert_eq!(probe["class"], "soft", "{}", probe["id"]);
        }
    }
}

/// Nix stays out of the Hard tool registry — and therefore out of the
/// package wrapper — because wrapping it would put Nix inside the Nix
/// package's own closure on every host.
#[test]
fn nix_is_not_a_hard_runtime_tool() {
    assert!(
        release_kit::probes::HARD_RUNTIME_TOOLS
            .iter()
            .all(|(executable, package)| *executable != "nix" && *package != "nix"),
        "nix must stay a Soft probe"
    );
    let nix =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("nix/package.nix"))
            .expect("nix/package.nix reads");
    let marker = "makeBinPath [";
    let start = nix.find(marker).expect("the wrapper names a package list");
    let rest = &nix[start + marker.len()..];
    let end = rest.find(']').expect("the package list closes");
    assert!(
        !rest[..end]
            .split_whitespace()
            .any(|package| package == "nix"),
        "the wrapper must not carry nix"
    );
}

#[test]
fn devshell_add_previews_four_fragments_and_writes_nothing() {
    let fixture = DevshellFixture::new();
    fixture.rk(&["devshell", "add"]).assert().success().stdout(
        predicate::str::contains("DRY RUN")
            .and(predicate::str::contains("--- flake-input into flake.nix"))
            .and(predicate::str::contains(
                "--- outputs-argument into flake.nix",
            ))
            .and(predicate::str::contains(
                "--- devshell-package into flake.nix",
            ))
            .and(predicate::str::contains("--- envrc-sync into .envrc"))
            .and(predicate::str::contains(format!(
                "github:gubasso/release-kit/v{}",
                env!("CARGO_PKG_VERSION")
            )))
            .and(predicate::str::contains("rk devshell sync --apply || true")),
    );
    assert!(!fixture.target().join("flake.nix").exists());
    assert!(!fixture.target().join(".envrc").exists());
    assert!(fixture.curl_log().is_empty(), "add fetches nothing");
    assert!(fixture.nix_log().is_empty(), "add spawns no nix");
}

#[test]
fn devshell_add_json_carries_each_fragment_with_its_anchor_and_placement() {
    let fixture = DevshellFixture::new();
    let report = fixture.json(&["devshell", "add", "--tag", "0.2.15"]);
    assert_eq!(report["schema"], "rk.devshell-add/1");
    assert_eq!(report["mode"], "preview");
    assert_eq!(report["tag"], "v0.2.15");
    assert_eq!(report["tag_source"], "argument");
    assert_eq!(report["written"], serde_json::json!([]));
    let fragments = report["fragments"].as_array().expect("fragments");
    let ids: Vec<&str> = fragments
        .iter()
        .map(|f| f["id"].as_str().expect("an id"))
        .collect();
    assert_eq!(
        ids,
        [
            "flake-input",
            "outputs-argument",
            "devshell-package",
            "envrc-sync"
        ]
    );
    let placements: Vec<&str> = fragments
        .iter()
        .map(|f| f["placement"].as_str().expect("a placement"))
        .collect();
    assert_eq!(
        placements,
        [
            "insert-into-attrset",
            "add-to-function-head",
            "append-to-list",
            "append-line"
        ]
    );
    assert_eq!(fragments[0]["anchor"]["kind"], "attrset");
    assert_eq!(fragments[0]["anchor"]["path"], "inputs");
    assert!(
        fragments[0]["text"]
            .as_str()
            .expect("text")
            .contains("github:gubasso/release-kit/v0.2.15")
    );
    assert_eq!(
        fragments[2]["text"],
        "release-kit.packages.${system}.default"
    );
    assert_eq!(fragments[3]["text"], "rk devshell sync --apply || true");
    // No flake and no .envrc: every fragment is known to be missing.
    for fragment in fragments {
        assert_eq!(fragment["present"], false, "{}", fragment["id"]);
    }
}

#[test]
fn devshell_add_marks_the_fragments_a_half_wired_flake_already_has() {
    let fixture = DevshellFixture::new();
    std::fs::write(
        fixture.target().join("flake.nix"),
        "{\n  inputs = {\n    nixpkgs.url = \"github:NixOS/nixpkgs/nixos-unstable\";\n    release-kit = {\n      url = \"github:gubasso/release-kit/v0.2.16\";\n      inputs.nixpkgs.follows = \"nixpkgs\";\n    };\n  };\n  outputs = { self, nixpkgs }: {\n    devShells.x86_64-linux.default = nixpkgs.legacyPackages.x86_64-linux.mkShell { packages = [ ]; };\n  };\n}\n",
    )
    .expect("the flake writes");
    std::fs::write(fixture.target().join(".envrc"), "use flake\n").expect(".envrc writes");
    let report = fixture.json(&["devshell", "add"]);
    let by_id = |id: &str| {
        report["fragments"]
            .as_array()
            .expect("fragments")
            .iter()
            .find(|f| f["id"] == id)
            .expect("the fragment")
            .clone()
    };
    assert_eq!(by_id("flake-input")["present"], true);
    assert_eq!(by_id("flake-input")["anchor"]["needle"], "inputs = {");
    assert_eq!(by_id("outputs-argument")["present"], false);
    assert_eq!(by_id("outputs-argument")["anchor"]["needle"], "outputs =");
    assert_eq!(by_id("devshell-package")["present"], false);
    assert_eq!(
        by_id("devshell-package")["anchor"]["needle"],
        "packages = ["
    );
    assert_eq!(by_id("envrc-sync")["present"], false);
    assert!(by_id("envrc-sync").get("needle").is_none());
    // An ellipsis in the head cannot be judged: the field is omitted.
    std::fs::write(
        fixture.target().join("flake.nix"),
        "{\n  inputs = { };\n  outputs = { self, ... }: { };\n}\n",
    )
    .expect("the flake writes");
    let report = fixture.json(&["devshell", "add"]);
    let head = report["fragments"]
        .as_array()
        .expect("fragments")
        .iter()
        .find(|f| f["id"] == "outputs-argument")
        .expect("the fragment")
        .clone();
    assert!(head.get("present").is_none(), "{head}");
}

#[test]
fn devshell_add_apply_seeds_a_flake_and_envrc_where_there_is_none() {
    let fixture = DevshellFixture::new();
    let report = fixture.json(&["devshell", "add", "--apply", "--tag", "v0.2.15"]);
    assert_eq!(report["mode"], "apply");
    assert_eq!(
        report["written"],
        serde_json::json!(["flake.nix", ".envrc"])
    );
    assert!(report.get("refusal").is_none());
    let flake = fixture.read("flake.nix");
    assert!(flake.contains("url = \"github:gubasso/release-kit/v0.2.15\";"));
    assert!(flake.contains("release-kit.packages.${system}.default"));
    assert!(flake.contains("inputs.nixpkgs.follows = \"nixpkgs\";"));
    assert_eq!(
        fixture.read(".envrc"),
        "use flake\nrk devshell sync --apply || true\n"
    );
    let status = fixture.json(&["devshell", "status"]);
    assert_eq!(status["state"], "ready");
    assert_eq!(status["pin_tag"], "v0.2.15");
    assert_eq!(status["envrc_sync"], true);
    assert_eq!(
        status["lock"], "absent",
        "the seed carries no lock; sync writes it"
    );
}

#[test]
fn devshell_add_apply_refuses_a_flake_the_target_owns() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    let before = fixture.read("flake.nix");
    let out = fixture
        .rk(&["devshell", "add", "--apply"])
        .output()
        .expect("rk runs");
    assert_eq!(out.status.code(), Some(73));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("--- flake-input into flake.nix"),
        "the fragments still print: {stdout}"
    );
    assert!(
        stdout.contains("wrote .envrc"),
        "the absent file is seeded: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("never edits a file the target owns"),
        "the refusal names the reason: {stderr}"
    );
    assert_eq!(
        fixture.read("flake.nix"),
        before,
        "the owned flake is byte-identical"
    );
    assert!(fixture.target().join(".envrc").exists());
    // The JSON form carries the refusal in the report and the diagnostic on stderr.
    let out = fixture
        .rk(&["devshell", "add", "--apply", "--json"])
        .output()
        .expect("rk runs");
    assert_eq!(out.status.code(), Some(73));
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    assert!(
        report["refusal"]
            .as_str()
            .expect("a refusal")
            .contains("flake.nix and .envrc")
    );
    assert_eq!(report["written"], serde_json::json!([]));
    let diagnostic: serde_json::Value = serde_json::from_slice(&out.stderr).expect("a diagnostic");
    assert_eq!(diagnostic["reason"], "destructive-refusal");
}

#[test]
fn devshell_add_apply_creates_only_absent_files() {
    let fixture = DevshellFixture::new();
    std::fs::write(fixture.target().join(".envrc"), "use flake\nexport FOO=1\n")
        .expect(".envrc writes");
    let out = fixture
        .rk(&["devshell", "add", "--apply", "--json"])
        .output()
        .expect("rk runs");
    assert_eq!(out.status.code(), Some(73), "the owned .envrc is refused");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    assert_eq!(report["written"], serde_json::json!(["flake.nix"]));
    assert_eq!(fixture.read(".envrc"), "use flake\nexport FOO=1\n");
    assert!(fixture.read("flake.nix").contains("release-kit"));
    let envrc = report["fragments"]
        .as_array()
        .expect("fragments")
        .iter()
        .find(|f| f["id"] == "envrc-sync")
        .expect("the fragment")
        .clone();
    assert_eq!(
        envrc["present"], false,
        "the owned .envrc still lacks the line"
    );
}

#[test]
fn devshell_add_refuses_a_tag_that_is_not_a_release() {
    let fixture = DevshellFixture::new();
    fixture
        .rk(&["devshell", "add", "--tag", "latest"])
        .assert()
        .code(64)
        .stderr(predicate::str::contains("is not a release tag"));
}

#[test]
fn devshell_status_names_a_predecessor_mechanism_beside_a_wired_pin() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    fixture
        .rk(&["devshell", "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("state superseded")
                .and(predicate::str::contains(
                    "leftover remove-file scripts/rk-bump.sh",
                ))
                .and(predicate::str::contains("leftover replace-line .envrc:5"))
                .and(predicate::str::contains(
                    "leftover manual justfile:2 rk-bump tag='':",
                ))
                .and(predicate::str::contains("rk devshell clean")),
        );
    let report = fixture.json(&["devshell", "status"]);
    assert_eq!(report["state"], "superseded");
    assert_eq!(report["pin_tag"], "v0.2.16");
    let leftovers = report["leftovers"].as_array().expect("leftovers");
    let rows: Vec<(String, String, String)> = leftovers
        .iter()
        .map(|l| {
            (
                l["id"].as_str().unwrap().to_owned(),
                l["file"].as_str().unwrap().to_owned(),
                l["action"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    let expect = |id: &str, file: &str, action: &str| {
        assert!(
            rows.contains(&(id.to_owned(), file.to_owned(), action.to_owned())),
            "{id} at {file} as {action} is missing from {rows:?}"
        );
    };
    expect("bump-script", "scripts/rk-bump.sh", "remove-file");
    expect("autobump-script", "scripts/rk-autobump.sh", "remove-file");
    expect("bump-suite", "tests/rk-bump.bats", "remove-file");
    expect("autobump-suite", "tests/rk-autobump.bats", "remove-file");
    expect("envrc-invocation", ".envrc", "replace-line");
    expect("envrc-switch", ".envrc.local.example", "manual");
    expect("just-recipe", "justfile", "manual");
    expect("devshell-tooling", "flake.nix", "manual");
    expect("host-install", "README.md", "manual");
    expect("host-install", ".github/workflows/ci.yml", "manual");
    // The three .envrc lines naming the scripts are three replace-line rows.
    assert_eq!(
        rows.iter()
            .filter(|(id, _, _)| id == "envrc-invocation")
            .count(),
        2
    );
    let switch = leftovers
        .iter()
        .find(|l| l["id"] == "envrc-switch")
        .expect("the switch row");
    assert_eq!(switch["line"], 1);
    assert_eq!(switch["text"], "# export RK_SKIP_AUTOBUMP=1");
}

#[test]
fn devshell_status_names_leftovers_in_an_unwired_target() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    std::fs::write(
        fixture.target().join("flake.nix"),
        "{\n  inputs = { };\n  outputs = { self }: { };\n}\n",
    )
    .expect("the flake writes");
    let report = fixture.json(&["devshell", "status"]);
    assert_eq!(
        report["state"], "not-wired",
        "the rollup keeps its first-match order"
    );
    assert!(
        !report["leftovers"]
            .as_array()
            .expect("leftovers")
            .is_empty(),
        "the leftovers are reported whatever the state is"
    );
    let add = fixture.json(&["devshell", "add"]);
    assert!(
        add["next"]
            .as_array()
            .expect("next")
            .iter()
            .any(|line| line.as_str().unwrap().contains("rk devshell clean")),
        "add routes to the cleanup first: {}",
        add["next"]
    );
}

#[test]
fn devshell_clean_previews_every_leftover_and_removes_nothing() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    let envrc = fixture.read(".envrc");
    fixture
        .rk(&["devshell", "clean"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("DRY RUN")
                .and(predicate::str::contains(
                    "leftover remove-file scripts/rk-bump.sh",
                ))
                .and(predicate::str::contains("leftover replace-line .envrc"))
                .and(predicate::str::contains("leftover manual flake.nix"))
                .and(predicate::str::contains("rk devshell clean"))
                .and(predicate::str::contains("--apply")),
        );
    let report = fixture.json(&["devshell", "clean"]);
    assert_eq!(report["schema"], "rk.devshell-clean/1");
    assert_eq!(report["mode"], "preview");
    assert_eq!(report["removed"], serde_json::json!([]));
    assert_eq!(report["rewritten"], serde_json::json!([]));
    assert_eq!(report["manual"], serde_json::json!([]));
    assert!(!report["leftovers"].as_array().unwrap().is_empty());
    assert!(fixture.target().join("scripts/rk-bump.sh").exists());
    assert_eq!(fixture.read(".envrc"), envrc);
}

#[test]
fn devshell_clean_apply_removes_the_scripts_and_the_suites() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    let report = fixture.json(&["devshell", "clean", "--apply"]);
    assert_eq!(report["mode"], "apply");
    assert_eq!(
        report["removed"],
        serde_json::json!([
            "scripts/rk-bump.sh",
            "scripts/rk-autobump.sh",
            "tests/rk-bump.bats",
            "tests/rk-autobump.bats"
        ])
    );
    for rel in [
        "scripts/rk-bump.sh",
        "scripts/rk-autobump.sh",
        "tests/rk-bump.bats",
        "tests/rk-autobump.bats",
    ] {
        assert!(!fixture.target().join(rel).exists(), "{rel} is removed");
    }
}

#[test]
fn devshell_clean_apply_swaps_the_envrc_invocation_for_the_sync_line() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    let report = fixture.json(&["devshell", "clean", "--apply"]);
    assert_eq!(report["rewritten"], serde_json::json!([".envrc"]));
    assert_eq!(
        fixture.read(".envrc"),
        "use flake\n\n# Bump the release-kit pin once a day on entry.\nrk devshell sync --apply || true\n\nsource_env_if_exists .envrc.local\n",
        "the sync line takes the first removed position and every other line is byte-identical"
    );
}

#[test]
fn devshell_clean_apply_leaves_the_justfile_and_the_flake_and_names_them() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    let justfile = fixture.read("justfile");
    let flake = fixture.read("flake.nix");
    let readme = fixture.read("README.md");
    let report = fixture.json(&["devshell", "clean", "--apply"]);
    assert_eq!(fixture.read("justfile"), justfile);
    assert_eq!(fixture.read("flake.nix"), flake);
    assert_eq!(fixture.read("README.md"), readme);
    let manual = report["manual"].as_array().expect("manual");
    let files: Vec<&str> = manual
        .iter()
        .map(|entry| entry["file"].as_str().unwrap())
        .collect();
    assert!(files.contains(&"justfile"), "{files:?}");
    assert_eq!(
        files.iter().filter(|f| **f == "flake.nix").count(),
        2,
        "flock and bats are two entries: {files:?}"
    );
    let recipe = manual
        .iter()
        .find(|entry| entry["id"] == "just-recipe")
        .expect("the recipe row");
    assert_eq!(recipe["line"], 2);
    assert_eq!(recipe["text"], "rk-bump tag='':");
    assert!(
        recipe["reason"]
            .as_str()
            .unwrap()
            .contains("a line scan cannot judge")
    );
    assert!(
        report["next"]
            .as_array()
            .unwrap()
            .iter()
            .any(|line| line.as_str().unwrap().contains("edit by hand")),
        "{}",
        report["next"]
    );
}

#[test]
fn devshell_clean_names_a_host_install_line_in_ci() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    let report = fixture.json(&["devshell", "clean", "--apply"]);
    let ci = report["manual"]
        .as_array()
        .expect("manual")
        .iter()
        .find(|entry| entry["file"] == ".github/workflows/ci.yml")
        .expect("the CI row")
        .clone();
    assert_eq!(ci["id"], "host-install");
    assert_eq!(ci["line"], 4);
    assert_eq!(ci["text"], "- run: cargo binstall release-kit");
    assert_eq!(
        fixture.read(".github/workflows/ci.yml"),
        "jobs:\n  test:\n    steps:\n      - run: cargo binstall release-kit\n"
    );
}

#[test]
fn devshell_clean_also_refuses_a_path_outside_the_target() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    let outside = fixture.home.path().join("elsewhere.sh");
    std::fs::write(&outside, "x\n").expect("writes");
    fixture
        .rk(&[
            "devshell",
            "clean",
            "--apply",
            "--also",
            outside.to_str().expect("utf-8"),
        ])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("outside the target"));
    assert!(outside.exists());
    assert!(
        fixture.target().join("scripts/rk-bump.sh").exists(),
        "a refused --also writes nothing at all"
    );
    fixture
        .rk(&["devshell", "clean", "--apply", "--also", "scripts"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("a directory"));
    fixture
        .rk(&["devshell", "clean", "--apply", "--also", "no-such-file"])
        .assert()
        .code(73);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            fixture.target().join("README.md"),
            fixture.target().join("link.md"),
        )
        .expect("the symlink creates");
        fixture
            .rk(&["devshell", "clean", "--apply", "--also", "link.md"])
            .assert()
            .code(73)
            .stderr(predicate::str::contains("a symlink"));
        assert!(fixture.target().join("README.md").exists());
    }
    // A regular file inside the target is removed beside the catalog.
    std::fs::write(fixture.target().join("scripts/old-bump.sh"), "x\n").expect("writes");
    let report = fixture.json(&[
        "devshell",
        "clean",
        "--apply",
        "--also",
        "scripts/old-bump.sh",
    ]);
    assert!(
        report["removed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "scripts/old-bump.sh")
    );
    assert!(!fixture.target().join("scripts/old-bump.sh").exists());
}

#[test]
fn a_clean_target_reports_ready_and_an_empty_manual_list() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    fixture.json(&["devshell", "clean", "--apply"]);
    // The operator finishes the manual entries by hand.
    std::fs::write(fixture.target().join("justfile"), "test:\n    cargo test\n").expect("writes");
    std::fs::write(
        fixture.target().join("flake.nix"),
        "{\n  inputs = {\n    release-kit = {\n      url = \"github:gubasso/release-kit/v0.2.16\";\n      inputs.nixpkgs.follows = \"nixpkgs\";\n    };\n  };\n  outputs = { self, nixpkgs, release-kit }: { };\n}\n",
    )
    .expect("writes");
    std::fs::write(fixture.target().join("README.md"), "# widget\n").expect("writes");
    std::fs::write(
        fixture.target().join(".github/workflows/ci.yml"),
        "jobs: {}\n",
    )
    .expect("writes");
    std::fs::write(
        fixture.target().join(".envrc.local.example"),
        "# export RK_DEVSHELL_SYNC=0\n",
    )
    .expect("writes");
    let status = fixture.json(&["devshell", "status"]);
    assert_eq!(status["state"], "ready");
    assert_eq!(status["leftovers"], serde_json::json!([]));
    let clean = fixture.json(&["devshell", "clean", "--apply"]);
    assert_eq!(clean["manual"], serde_json::json!([]));
    assert_eq!(clean["removed"], serde_json::json!([]));
}

#[test]
fn devshell_clean_is_idempotent() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    let first = fixture.json(&["devshell", "clean", "--apply"]);
    assert_eq!(first["removed"].as_array().unwrap().len(), 4);
    let envrc = fixture.read(".envrc");
    let second = fixture.json(&["devshell", "clean", "--apply"]);
    assert_eq!(second["removed"], serde_json::json!([]));
    assert_eq!(second["rewritten"], serde_json::json!([]));
    assert_eq!(fixture.read(".envrc"), envrc);
    assert_eq!(
        second["manual"].as_array().unwrap().len(),
        first["manual"].as_array().unwrap().len(),
        "the manual entries are reported again until the operator edits them"
    );
    fixture
        .rk(&["devshell", "clean", "--apply"])
        .assert()
        .success();
}

#[test]
fn devshell_sync_preview_reports_the_bump_and_spawns_no_nix() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    let flake = fixture.read("flake.nix");
    let lock = fixture.read("flake.lock");
    fixture
        .rk(&["devshell", "sync", "--caller", "operator"])
        .assert()
        .success()
        .stdout(predicate::str::contains("would-bump v0.2.15 -> v0.2.16"));
    let (code, report) = fixture.sync_json(&["devshell", "sync"]);
    assert_eq!(code, Some(0));
    assert_eq!(report["schema"], "rk.devshell-sync/1");
    assert_eq!(report["mode"], "preview");
    assert_eq!(report["caller"], "envrc");
    assert_eq!(report["outcome"], "would-bump");
    assert_eq!(report["from"], "v0.2.15");
    assert_eq!(report["to"], "v0.2.16");
    assert!(report.get("steps").is_none());
    assert!(fixture.nix_log().is_empty(), "a preview spawns no nix");
    assert_eq!(fixture.read("flake.nix"), flake);
    assert_eq!(fixture.read("flake.lock"), lock);
}

#[test]
fn devshell_sync_apply_rewrites_the_pin_updates_the_lock_and_builds() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["outcome"], "bumped");
    assert_eq!(report["from"], "v0.2.15");
    assert_eq!(report["to"], "v0.2.16");
    let steps: Vec<(&str, &str)> = report["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .map(|s| (s["step"].as_str().unwrap(), s["status"].as_str().unwrap()))
        .collect();
    assert_eq!(
        steps,
        [
            ("rewrite-pin", "ok"),
            ("flake-update", "ok"),
            ("build", "ok")
        ]
    );
    assert!(report.get("restored").is_none());
    assert!(
        fixture
            .read("flake.nix")
            .contains("url = \"github:gubasso/release-kit/v0.2.16\";")
    );
    assert!(fixture.read("flake.lock").contains("rev-of-v0.2.16"));
    let nix = fixture.nix_log();
    assert!(nix.contains("flake update release-kit"), "{nix}");
    assert!(
        nix.contains("eval --raw --impure --expr builtins.currentSystem"),
        "{nix}"
    );
    assert!(
        nix.contains("build --no-link .#devShells.x86_64-linux.default"),
        "{nix}"
    );
    assert!(
        !fixture.target().join("result").exists(),
        "--no-link drops no result symlink"
    );
    assert!(
        !fixture.state_dir().join(fixture.key()).exists(),
        "a committed transaction leaves no backup"
    );
    let status = fixture.json(&["devshell", "status"]);
    assert_eq!(status["pin_tag"], "v0.2.16");
    assert_eq!(status["locked_rev"], "rev-of-v0.2.16");
}

#[test]
fn a_same_version_sync_writes_nothing_and_spawns_no_nix() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.16");
    fixture.commit_all();
    let flake = fixture.read("flake.nix");
    let lock = fixture.read("flake.lock");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(0));
    assert_eq!(report["outcome"], "current");
    assert_eq!(fixture.read("flake.nix"), flake);
    assert_eq!(fixture.read("flake.lock"), lock);
    assert!(fixture.nix_log().is_empty(), "nothing to do spawns no nix");
    // The same run from .envrc is silent on stdout.
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn a_pin_ahead_of_the_latest_release_is_reported_and_never_rewritten() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.3.0");
    fixture.commit_all();
    let flake = fixture.read("flake.nix");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(0));
    assert_eq!(report["outcome"], "ahead");
    assert_eq!(report["from"], "v0.3.0");
    assert_eq!(report["to"], "v0.2.16");
    assert_eq!(fixture.read("flake.nix"), flake);
    assert!(fixture.nix_log().is_empty());
}

#[test]
fn a_failed_flake_update_restores_both_files() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    fixture.seed("nix_fail_update", "1");
    let flake = fixture.read("flake.nix");
    let lock = fixture.read("flake.lock");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(70), "{report}");
    assert_eq!(report["outcome"], "update-failed");
    assert_eq!(
        report["restored"],
        serde_json::json!(["flake.nix", "flake.lock"])
    );
    let failed = report["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["status"] == "failed")
        .expect("a failed step");
    assert_eq!(failed["step"], "flake-update");
    assert_eq!(failed["detail"], "error: could not update the lock");
    assert_eq!(fixture.read("flake.nix"), flake, "the pin is back");
    assert_eq!(fixture.read("flake.lock"), lock, "the lock is back");
    assert!(!fixture.nix_log().contains("build"), "the build never ran");
    assert!(!fixture.state_dir().join(fixture.key()).exists());
    let out = fixture
        .rk(&[
            "devshell", "sync", "--apply", "--caller", "operator", "--json",
        ])
        .output()
        .expect("rk runs");
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&out.stderr).expect("a diagnostic on stderr");
    assert_eq!(diagnostic["reason"], "subprocess-failed");
    assert_eq!(diagnostic["step"], "flake-update");
    assert_eq!(diagnostic["target_state"], "both files restored");
}

#[test]
fn a_failed_devshell_build_restores_both_files() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    fixture.seed("nix_fail_build", "1");
    let flake = fixture.read("flake.nix");
    let lock = fixture.read("flake.lock");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(70), "{report}");
    assert_eq!(report["outcome"], "build-failed");
    assert_eq!(
        report["restored"],
        serde_json::json!(["flake.nix", "flake.lock"])
    );
    assert_eq!(fixture.read("flake.nix"), flake);
    assert_eq!(
        fixture.read("flake.lock"),
        lock,
        "the lock the update wrote is rolled back"
    );
    assert!(fixture.nix_log().contains("flake update release-kit"));
    // From .envrc the same failure is reported and exits 0.
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("build-failed v0.2.15 -> v0.2.16"));
    assert_eq!(fixture.read("flake.nix"), flake);
}

#[test]
fn devshell_sync_refuses_a_flake_with_more_than_one_pin_line() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    let flake = fixture.read("flake.nix");
    std::fs::write(
        fixture.target().join("flake.nix"),
        format!("{flake}  url = \"github:gubasso/release-kit/v0.2.14\";\n"),
    )
    .expect("the flake writes");
    fixture.commit_all();
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(73));
    assert_eq!(report["outcome"], "ambiguous-pin");
    assert!(report["detail"].as_str().unwrap().contains("2 lines"));
    assert!(
        fixture.curl_log().is_empty(),
        "an ambiguous pin fetches nothing"
    );
    assert!(fixture.nix_log().is_empty());
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ambiguous-pin"));
}

#[test]
fn an_explicit_tag_makes_no_network_request() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    let (code, report) = fixture.sync_json(&[
        "devshell", "sync", "--apply", "--caller", "operator", "--tag", "0.9.0",
    ]);
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["outcome"], "bumped");
    assert_eq!(report["to"], "v0.9.0");
    assert!(
        fixture.curl_log().is_empty(),
        "an explicit tag fetches nothing"
    );
    assert!(fixture.read("flake.nix").contains("release-kit/v0.9.0\""));
    assert!(fixture.read("flake.lock").contains("rev-of-v0.9.0"));
}

#[test]
fn each_tag_shape_folds_to_one_tag() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    for shape in [
        "v0.9.0",
        "0.9.0",
        "https://github.com/gubasso/release-kit/releases/tag/v0.9.0",
    ] {
        let (code, report) = fixture.sync_json(&["devshell", "sync", "--tag", shape]);
        assert_eq!(code, Some(0), "{shape}: {report}");
        assert_eq!(report["outcome"], "would-bump", "{shape}");
        assert_eq!(report["to"], "v0.9.0", "{shape}");
    }
    fixture
        .rk(&["devshell", "sync", "--tag", "latest"])
        .assert()
        .code(64);
}

#[test]
fn discovery_reads_the_redirect_and_spends_no_api_call() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    fixture.seed("latest_tag", "v0.2.17");
    let (_, report) = fixture.sync_json(&["devshell", "sync"]);
    assert_eq!(report["to"], "v0.2.17");
    let curl = fixture.curl_log();
    let lines: Vec<&str> = curl.lines().collect();
    assert_eq!(lines.len(), 1, "one curl call: {curl}");
    let line = lines[0];
    for needle in [
        "-o /dev/null",
        "-w %{url_effective}",
        "--max-time",
        "releases/latest",
    ] {
        assert!(line.contains(needle), "{line} lacks {needle}");
    }
    assert!(!line.contains("api.github.com"), "no API host: {line}");
    // A network failure is a reported outcome that writes nothing.
    fixture.seed("curl_fail", "1");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(70));
    assert_eq!(report["outcome"], "unreachable");
    assert!(
        report["detail"]
            .as_str()
            .unwrap()
            .contains("Could not resolve host")
    );
    assert!(fixture.nix_log().is_empty());
    assert!(fixture.read("flake.nix").contains("v0.2.15"));
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("unreachable"));
}

#[test]
fn an_interrupted_transaction_is_recovered_on_the_next_run() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    let flake = fixture.read("flake.nix");
    let lock = fixture.read("flake.lock");
    // Plant what a killed run leaves: the backups, the marker with a
    // dead owner, and the two files half-moved.
    let backup = fixture.state_dir().join(fixture.key()).join("backup");
    std::fs::create_dir_all(&backup).expect("the backup dir creates");
    std::fs::write(backup.join("flake.nix"), &flake).expect("writes");
    std::fs::write(backup.join("flake.lock"), &lock).expect("writes");
    std::fs::write(
        fixture.state_dir().join(fixture.key()).join("pending.json"),
        r#"{"target":"x","pid":4294967295}"#,
    )
    .expect("the marker writes");
    std::fs::write(
        fixture.target().join("flake.nix"),
        flake.replace("v0.2.15", "v0.2.16"),
    )
    .expect("writes");
    std::fs::remove_file(fixture.target().join("flake.lock")).expect("the lock goes");
    let status = fixture.json(&["devshell", "status"]);
    assert_eq!(status["state"], "pending-recovery");
    assert_eq!(status["pending"], true);
    // A preview names the pending run and touches nothing.
    let (code, report) = fixture.sync_json(&["devshell", "sync", "--caller", "operator"]);
    assert_eq!(code, Some(0));
    assert_eq!(report["outcome"], "pending-recovery");
    assert!(fixture.read("flake.nix").contains("v0.2.16"));
    // The apply recovers first, then continues to the bump.
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(
        report["recovered"],
        serde_json::json!(["flake.nix", "flake.lock"])
    );
    assert_eq!(report["outcome"], "bumped");
    assert_eq!(
        report["from"], "v0.2.15",
        "the recovered pin is what the bump starts from"
    );
    assert!(
        !fixture
            .state_dir()
            .join(fixture.key())
            .join("pending.json")
            .exists()
    );
    assert!(fixture.read("flake.lock").contains("rev-of-v0.2.16"));
}

/// One arrangement of a devshell fixture, for the outcome tables.
type Arrange = Box<dyn Fn(&DevshellFixture)>;

/// Today as the binary stamps it: the first ten characters of UTC now.
fn today_utc() -> String {
    release_kit::applog::now_utc()[..10].to_owned()
}

#[test]
fn devshell_sync_refuses_uncommitted_edits_to_the_two_files() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    std::fs::write(
        fixture.target().join("flake.lock"),
        "{\"nodes\":{},\"version\":7}\n",
    )
    .expect("the lock edits");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(73), "{report}");
    assert_eq!(report["outcome"], "refused-dirty");
    assert_eq!(report["from"], "v0.2.15");
    assert!(
        fixture.curl_log().is_empty(),
        "a dirty tree fetches nothing"
    );
    assert!(fixture.nix_log().is_empty());
    assert!(fixture.read("flake.nix").contains("v0.2.15"));
    // A preview reports the same refusal, so the operator learns it before an apply.
    let (code, report) = fixture.sync_json(&["devshell", "sync", "--caller", "operator"]);
    assert_eq!(code, Some(73));
    assert_eq!(report["outcome"], "refused-dirty");
    // An untracked file, as a fresh seed leaves, counts too.
    let fresh = DevshellFixture::new();
    fresh.json(&["devshell", "add", "--apply", "--tag", "v0.2.15"]);
    let (_, report) = fresh.sync_json(&["devshell", "sync", "--apply"]);
    assert_eq!(report["outcome"], "refused-dirty");
}

#[test]
fn devshell_sync_judges_the_target_repo_from_inside_a_git_hook() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    std::fs::write(fixture.target().join("flake.lock"), "{}\n").expect("the lock edits");
    // A clean decoy repository whose hook variables a running hook would export.
    let decoy = branch_fixture();
    let out = fixture
        .rk(&[
            "devshell", "sync", "--apply", "--caller", "operator", "--json",
        ])
        .env("GIT_DIR", decoy.path().join(".git"))
        .env("GIT_WORK_TREE", decoy.path())
        .env("GIT_INDEX_FILE", decoy.path().join(".git/index"))
        .output()
        .expect("rk runs");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    assert_eq!(
        report["outcome"], "refused-dirty",
        "the target's own dirt is judged, not the decoy's cleanliness: {report}"
    );
}

#[test]
fn a_second_sync_skips_quietly_while_the_first_holds_the_lock() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    std::fs::create_dir_all(fixture.state_dir()).expect("the state dir creates");
    std::fs::write(
        fixture.state_dir().join(format!("{}.lock", fixture.key())),
        format!(
            "pid={}\nstarted={}\n",
            std::process::id(),
            release_kit::applog::now_utc()
        ),
    )
    .expect("the lock plants");
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::is_empty());
    assert!(fixture.curl_log().is_empty(), "contention fetches nothing");
    assert!(fixture.nix_log().is_empty());
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(0), "contention is normal, even for the operator");
    assert_eq!(report["outcome"], "skipped-locked");
    assert!(
        fixture
            .state_dir()
            .join(format!("{}.lock", fixture.key()))
            .exists(),
        "the live lock is left alone"
    );
}

#[test]
fn a_lock_that_cannot_be_taken_at_all_is_reported_loudly() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    // A regular file where the state directory must be: nothing can be created under it.
    std::fs::create_dir_all(fixture.home.path().join("release-kit")).expect("creates");
    std::fs::write(fixture.state_dir(), "not a directory\n").expect("the blocker writes");
    let out = fixture
        .rk(&["devshell", "sync", "--apply"])
        .output()
        .expect("rk runs");
    assert_eq!(
        out.status.code(),
        Some(0),
        "the .envrc caller still exits 0"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("warning: rk devshell sync: the lock cannot be taken"),
        "unavailability is loud on stderr: {stderr}"
    );
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(74), "{report}");
    assert_eq!(report["outcome"], "lock-unavailable");
    assert!(fixture.curl_log().is_empty());
}

#[test]
fn a_stale_lock_whose_owner_is_gone_is_taken() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    std::fs::create_dir_all(fixture.state_dir()).expect("the state dir creates");
    let lock = fixture.state_dir().join(format!("{}.lock", fixture.key()));
    std::fs::write(&lock, "pid=4294967295\nstarted=2020-01-01T00:00:00Z\n").expect("plants");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["outcome"], "bumped", "the stale lock was taken over");
    assert!(!lock.exists(), "the lock is released after the run");
}

#[test]
fn the_daily_stamp_is_written_before_a_failing_attempt() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    fixture.seed("nix_fail_build", "1");
    let stamp = fixture.state_dir().join(format!("{}.stamp", fixture.key()));
    assert!(!stamp.exists());
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::contains("build-failed"));
    assert_eq!(
        std::fs::read_to_string(&stamp)
            .expect("the stamp exists")
            .trim(),
        today_utc(),
        "a failing attempt still costs one fetch and one build a day"
    );
    // The next entry skips without a fetch.
    fixture.seed("curl-log", "");
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert!(fixture.curl_log().is_empty());
}

#[test]
fn a_stamped_day_skips_the_next_directory_entry() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    std::fs::create_dir_all(fixture.state_dir()).expect("the state dir creates");
    std::fs::write(
        fixture.state_dir().join(format!("{}.stamp", fixture.key())),
        format!("{}\n", today_utc()),
    )
    .expect("the stamp plants");
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert!(
        fixture.curl_log().is_empty(),
        "a stamped day fetches nothing"
    );
    assert!(fixture.nix_log().is_empty());
    let (_, report) = fixture.sync_json(&["devshell", "sync", "--apply"]);
    assert_eq!(report["outcome"], "skipped-stamped");
    assert_eq!(report["stamp"], today_utc());
    // An older stamp lets the attempt run and is rewritten to today.
    std::fs::write(
        fixture.state_dir().join(format!("{}.stamp", fixture.key())),
        "2020-01-01\n",
    )
    .expect("the stamp plants");
    let (_, report) = fixture.sync_json(&["devshell", "sync", "--apply"]);
    assert_eq!(report["outcome"], "bumped");
    assert_eq!(report["stamp"], today_utc());
}

#[test]
fn an_operator_run_ignores_the_daily_stamp() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    std::fs::create_dir_all(fixture.state_dir()).expect("the state dir creates");
    let stamp = fixture.state_dir().join(format!("{}.stamp", fixture.key()));
    std::fs::write(&stamp, format!("{}\n", today_utc())).expect("the stamp plants");
    let (code, report) =
        fixture.sync_json(&["devshell", "sync", "--apply", "--caller", "operator"]);
    assert_eq!(code, Some(0), "{report}");
    assert_eq!(report["outcome"], "bumped");
    assert_eq!(
        report["stamp"],
        today_utc(),
        "the stamp is reported, not consulted"
    );
    assert!(!fixture.curl_log().is_empty(), "the operator run fetched");
}

/// Every reported failure outcome from the `.envrc` caller exits 0: the
/// outcome lives in the report, never in the exit code.
#[test]
fn every_envrc_path_exits_zero() {
    let cases: Vec<(&str, Arrange)> = vec![
        ("no-flake", Box::new(|_| {})),
        (
            "not-wired",
            Box::new(|f| {
                std::fs::write(f.target().join("flake.nix"), "{ }\n").expect("writes");
                f.commit_all();
            }),
        ),
        (
            "ambiguous-pin",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                let flake = f.read("flake.nix");
                std::fs::write(
                    f.target().join("flake.nix"),
                    format!("{flake}  url = \"github:gubasso/release-kit/v0.2.14\";\n"),
                )
                .expect("writes");
                f.commit_all();
            }),
        ),
        (
            "refused-dirty",
            Box::new(|f| {
                f.write_flake("v0.2.15");
            }),
        ),
        (
            "unreachable",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                f.seed("curl_fail", "1");
            }),
        ),
        (
            "update-failed",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                f.seed("nix_fail_update", "1");
            }),
        ),
        (
            "build-failed",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                f.seed("nix_fail_build", "1");
            }),
        ),
        (
            "lock-unavailable",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                std::fs::create_dir_all(f.home.path().join("release-kit")).expect("creates");
                std::fs::write(f.state_dir(), "blocker\n").expect("writes");
            }),
        ),
    ];
    for (expected, arrange) in cases {
        let fixture = DevshellFixture::new();
        arrange(&fixture);
        let (code, report) = fixture.sync_json(&["devshell", "sync", "--apply"]);
        assert_eq!(report["outcome"], expected, "{report}");
        assert_eq!(code, Some(0), "{expected} must exit 0 from .envrc");
    }
}

/// The same outcomes under `--caller operator` take the exit-code matrix.
#[test]
fn an_operator_run_fails_loudly_where_the_envrc_run_reports() {
    let cases: Vec<(&str, i32, &str, Arrange)> = vec![
        (
            "ambiguous-pin",
            73,
            "state-drift",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                let flake = f.read("flake.nix");
                std::fs::write(
                    f.target().join("flake.nix"),
                    format!("{flake}  url = \"github:gubasso/release-kit/v0.2.14\";\n"),
                )
                .expect("writes");
                f.commit_all();
            }),
        ),
        (
            "refused-dirty",
            73,
            "state-drift",
            Box::new(|f| {
                f.write_flake("v0.2.15");
            }),
        ),
        (
            "unreachable",
            70,
            "forge-temporary",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                f.seed("curl_fail", "1");
            }),
        ),
        (
            "update-failed",
            70,
            "subprocess-failed",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                f.seed("nix_fail_update", "1");
            }),
        ),
        (
            "build-failed",
            70,
            "subprocess-failed",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                f.seed("nix_fail_build", "1");
            }),
        ),
        (
            "lock-unavailable",
            74,
            "io",
            Box::new(|f| {
                f.write_flake("v0.2.15");
                f.commit_all();
                std::fs::create_dir_all(f.home.path().join("release-kit")).expect("creates");
                std::fs::write(f.state_dir(), "blocker\n").expect("writes");
            }),
        ),
    ];
    for (expected, code, reason, arrange) in cases {
        let fixture = DevshellFixture::new();
        arrange(&fixture);
        let out = fixture
            .rk(&[
                "devshell", "sync", "--apply", "--caller", "operator", "--json",
            ])
            .output()
            .expect("rk runs");
        let report: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("one JSON object");
        assert_eq!(report["outcome"], expected, "{report}");
        assert_eq!(out.status.code(), Some(code), "{expected}");
        // The diagnostic is the last stderr line; a warning may precede it.
        let stderr = String::from_utf8_lossy(&out.stderr);
        let last = stderr.lines().last().unwrap_or_default();
        let diagnostic: serde_json::Value = serde_json::from_str(last)
            .unwrap_or_else(|_| panic!("{expected}: a diagnostic on stderr: {stderr}"));
        assert_eq!(diagnostic["reason"], reason, "{expected}");
        if expected == "unreachable" {
            assert_eq!(diagnostic["retry"], true);
        }
        if expected == "update-failed" || expected == "build-failed" {
            assert_eq!(diagnostic["target_state"], "both files restored");
        }
    }
}

#[test]
fn ci_never_bumps() {
    for var in [
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "BUILDKITE",
        "CIRCLECI",
        "TF_BUILD",
    ] {
        let fixture = DevshellFixture::new();
        fixture.write_flake("v0.2.15");
        fixture.commit_all();
        fixture
            .rk(&["devshell", "sync", "--apply"])
            .env(var, "true")
            .assert()
            .success()
            .stdout(predicate::str::is_empty());
        assert!(fixture.curl_log().is_empty(), "{var}: CI fetches nothing");
        assert!(fixture.nix_log().is_empty(), "{var}: CI spawns nothing");
        let out = fixture
            .rk(&[
                "devshell", "sync", "--apply", "--caller", "operator", "--json",
            ])
            .env(var, "1")
            .output()
            .expect("rk runs");
        let report: serde_json::Value =
            serde_json::from_slice(&out.stdout).expect("one JSON object");
        assert_eq!(report["outcome"], "skipped-ci", "{var}");
        assert_eq!(out.status.code(), Some(0));
    }
    // An explicit no is not CI.
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    let out = fixture
        .rk(&["devshell", "sync", "--json"])
        .env("CI", "false")
        .output()
        .expect("rk runs");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    assert_eq!(report["outcome"], "would-bump");
}

#[test]
fn rk_devshell_sync_is_disabled_by_its_env_switch() {
    let fixture = DevshellFixture::new();
    fixture.write_flake("v0.2.15");
    fixture.commit_all();
    fixture
        .rk(&["devshell", "sync", "--apply"])
        .env("RK_DEVSHELL_SYNC", "0")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert!(fixture.curl_log().is_empty());
    assert!(fixture.nix_log().is_empty());
    let out = fixture
        .rk(&[
            "devshell", "sync", "--apply", "--caller", "operator", "--json",
        ])
        .env("RK_DEVSHELL_SYNC", "0")
        .output()
        .expect("rk runs");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    assert_eq!(report["outcome"], "skipped-disabled");
    assert!(
        !fixture
            .state_dir()
            .join(format!("{}.stamp", fixture.key()))
            .exists(),
        "a switched-off run stamps nothing"
    );
}

/// Every devshell action answers `--json` with exactly one object on
/// stdout, in every mode.
#[test]
fn every_devshell_action_emits_one_json_object() {
    let fixture = DevshellFixture::new();
    fixture.write_predecessor();
    fixture.commit_all();
    for args in [
        vec!["devshell", "status"],
        vec!["devshell", "add"],
        vec!["devshell", "add", "--apply"],
        vec!["devshell", "clean"],
        vec!["devshell", "clean", "--apply"],
        vec!["devshell", "sync"],
        vec!["devshell", "sync", "--apply"],
        vec!["devshell", "sync", "--caller", "operator"],
    ] {
        let out = fixture.rk(&args).arg("--json").output().expect("rk runs");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let value: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|_| panic!("{args:?}: one JSON object on stdout: {stdout}"));
        assert!(
            value["schema"]
                .as_str()
                .is_some_and(|schema| schema.starts_with("rk.devshell-")),
            "{args:?}: {value}"
        );
    }
}
