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

fn rk() -> Command {
    Command::cargo_bin("rk").expect("the rk binary builds")
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
        let mut command = rk();
        command.env("HOME", self.path());
        command
    }

    fn destination(&self, root: &str, skill: &str) -> PathBuf {
        self.path().join(root).join(skill).join("SKILL.md")
    }

    fn record(&self) -> PathBuf {
        self.path().join(RECORD_PATH)
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
        .args(["--repo", "acme/widget", "--target"])
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
         .github/workflows/release-plz.yml\n\
         AGENTS.md\n\
         dist-workspace.toml\n\
         release-plz.toml\n\
         Next:\n  rk init --tech rust --forge github --repo <owner/name> --target {path} --apply\n"
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
            .args(["--repo", "acme/widget", "--target"])
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
        assert_eq!(report["schema"], "rk.init/1");
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
        .args(["--repo", "acme/widget", "--target"])
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
    assert_eq!(
        home.load_record().written.len(),
        ROOTS.len() * SKILLS.len(),
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
    assert_eq!(actions.len(), ROOTS.len() * SKILLS.len());
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
        ["sh", "state-root", "git-remote", "gh-auth", "glab-auth"]
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
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(home.path())
            .status()
            .expect("git runs");
        assert!(status.success());
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
        "rk runs list",
        "rk runs show",
        "rk runs prune",
        "rk skill install",
        "rk skill uninstall",
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
const FORGE_STEPS: [&str; 9] = [
    "bot-secrets",
    "ci-permissions",
    "default-branch",
    "install-bot",
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
/// tree, except `package-check`, the one step outside the parity rule.
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
    assert_eq!(listed.len(), 10, "ten steps list: {text}");
    listed.retain(|name| name != "package-check");
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
        "RK_BOT_PRIVATE_KEY",
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

/// The runbooks render the spine: same steps, same order, as the chapters.
#[test]
fn the_runbooks_match_their_method_chapters() {
    let chapter = std::fs::read_to_string(repo_path("method/02-setup.md")).expect("reads");
    let runbook = std::fs::read_to_string(repo_path("runbooks/setup.md")).expect("reads");
    let chapter_steps: Vec<&str> = chapter
        .lines()
        .filter(|line| {
            line.strip_prefix("## ")
                .and_then(|rest| rest.split_once('.'))
                .is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    let runbook_steps: Vec<&str> = runbook
        .lines()
        .filter(|line| {
            line.strip_prefix("## ")
                .and_then(|rest| rest.split_once('.'))
                .is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
        })
        .collect();
    assert_eq!(
        chapter_steps, runbook_steps,
        "the setup runbook's steps drifted from the chapter's"
    );

    let operate = std::fs::read_to_string(repo_path("method/03-operate.md")).expect("reads");
    let sequence = operate
        .lines()
        .filter(|line| {
            line.split_once(". ")
                .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        })
        .count();
    let release = std::fs::read_to_string(repo_path("runbooks/release.md")).expect("reads");
    let rendered = release
        .lines()
        .filter(|line| {
            line.strip_prefix("## ")
                .and_then(|rest| rest.split_once('.'))
                .is_some_and(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
        })
        .count();
    assert_eq!(
        sequence, rendered,
        "the release runbook's step count drifted from the chapter's"
    );
}

#[test]
fn every_runbook_fence_declares_a_language() {
    for name in ["README.md", "release.md", "setup.md"] {
        let text = std::fs::read_to_string(repo_path("runbooks").join(name)).expect("reads");
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
    rk().args(["guide", "--list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("release").and(predicate::str::contains("setup")));
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

const MOCK_GH: &str = r#"#!/usr/bin/env bash
STATE="__STATE__"
printf '%s\n' "$*" >> "$STATE/log"
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
    if [[ "$query" == ".id" ]]; then echo 1; else echo "{\"id\":1,\"default_branch\":\"$(cat "$STATE/default_branch")\"}"; fi;;
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
  "GET user/installations")
    if [[ "$query" == ".total_count" ]]; then echo 1
    elif [[ -n "$query" ]]; then echo 42
    else echo '{"total_count":1,"installations":[{"id":42,"app_slug":"bot"}]}'; fi;;
  "PUT user/installations/"*"/repositories/"*)
    touch "$STATE/installed"; echo '{}';;
  "GET user/installations/"*"/repositories")
    entry=""
    [[ -f "$STATE/installed" ]] && entry='{"full_name":"acme/widget"}'
    if [[ -n "$query" ]]; then
      [[ -n "$entry" ]] && echo "acme/widget"
    else
      echo "{\"repositories\":[$entry]}"
    fi;;
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
  "GET "*"/protected_branches/"*)
    echo "glab: 404 Not Found (HTTP 404)" >&2; exit 1;;
  "PUT projects/"*)
    for f in "${fields[@]}"; do
      case "$f" in default_branch=*) echo "${f#default_branch=}" > "$STATE/default_branch";; esac
    done
    echo '{}';;
  "GET projects/"*)
    echo "{\"id\":1,\"default_branch\":\"$(cat "$STATE/default_branch")\",\"jobs_enabled\":true,\"only_allow_merge_if_pipeline_succeeds\":false,\"merge_method\":\"ff\",\"squash_option\":\"never\"}";;
  *) echo '{}';;
  esac
  exit 0;;
esac
exit 0
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
        for (name, body) in [("gh", MOCK_GH), ("glab", MOCK_GLAB)] {
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
        fixture
    }

    fn seed(&self, name: &str, value: &str) {
        std::fs::write(self.mock.path().join(name), value).expect("the state seeds");
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

    fn runs_root(&self) -> PathBuf {
        self.home.path().join("release-kit/runs")
    }

    fn rk(&self, args: &[&str]) -> Command {
        let mut command = rk();
        command
            .env("HOME", self.home.path())
            .env("XDG_STATE_HOME", self.home.path())
            .env("RK_GH_BIN", self.mock.path().join("gh"))
            .env("RK_GLAB_BIN", self.mock.path().join("glab"))
            .env_remove("RK_BOT_APP_ID")
            .env_remove("RK_BOT_PRIVATE_KEY")
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
    let pem = "-----BEGIN FAKE KEY-----\nsekret-pem-bytes\n-----END FAKE KEY-----\n";
    let apply = |fixture: &ForgeFixture| {
        let mut command = fixture.rk(&["setup"]);
        command
            .args(["--repo", "acme/widget", "--forge", "github"])
            .args(["--apply", "--required-check", "test-check"])
            .env("RK_BOT_APP_ID", "314159")
            .env("RK_BOT_PRIVATE_KEY", pem);
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
    assert!(body.contains(r#""allowed_merge_methods": ["squash"]"#));

    // The secret reached stdin and nothing else.
    assert!(fixture.stdin_log().contains("sekret-pem-bytes"));
    assert!(
        !fixture.log().contains("sekret-pem-bytes"),
        "a secret reached a process argument list"
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
        assert!(
            !text.contains("sekret-pem-bytes"),
            "{file} carries key material"
        );
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
        meta["secrets"]
            .as_array()
            .is_some_and(|list| list.iter().any(|s| s["secret"] == "RK_BOT_PRIVATE_KEY")),
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

    // And check reports clean at exit 0.
    fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ok protections-check"));
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

/// Detection selects the tree from the remote host, an unknown host refuses
/// naming the overrides, and a self-hosted GitLab warns about trusted
/// publishing at the start rather than at the registry step.
#[test]
fn detection_selects_the_tree_and_refuses_an_unknown_host() {
    let fixture = ForgeFixture::new();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(fixture.target.path())
            .status()
            .expect("git runs");
        assert!(status.success());
    };
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
    assert_eq!(manifest["schema_version"], 1);
    assert_eq!(manifest["rk_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["origin"], "init");
    assert_eq!(manifest["tech"], "rust");
    assert_eq!(manifest["forge"], "github");
    assert_eq!(manifest["parameters"]["repo"], "acme/widget");

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
    assert!(agents.trim_end().ends_with("<!-- END release-kit -->"));
    assert_eq!(
        manifest_file(&read_manifest(target.path()), "AGENTS.md")["kind"],
        "rendered"
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
    assert_eq!(report["schema"], "rk.status/1");
    assert_eq!(report["landed"], true);
    assert_eq!(report["tech"], "rust");
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
        cmd.args(["adopt", "--tech", "rust", "--forge", "github"])
            .args(["--repo", "acme/widget", "--target"])
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

    rk().args(["adopt", "--tech", "rust", "--forge", "github"])
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

    rk().args(["adopt", "--tech", "rust", "--forge", "github"])
        .args(["--repo", "acme/widget", "--target"])
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

    rk().args(["adopt", "--tech", "rust", "--forge", "github"])
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

    rk().args(["adopt", "--tech", "rust", "--forge", "github"])
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
    rk().args(["adopt", "--tech", "rust", "--forge", "github"])
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
    assert_eq!(report["schema"], "rk.versions-check/1");
    assert!(
        report["pins"]
            .as_array()
            .is_some_and(|pins| !pins.is_empty()),
        "{report}"
    );
}
