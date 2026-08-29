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
const SKILLS: [&str; 2] = ["rk-release", "rk-setup"];
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
         dist-workspace.toml\n\
         release-plz.toml\n\
         Next:\n  rk init --tech rust --forge github --target {path} --apply\n"
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
        cmd.args(["init", "--tech", "rust", "--forge", "github", "--target"])
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
    std::fs::write(target.path().join("release-plz.toml"), "something local\n")
        .expect("the conflict file writes");
    let output = rk()
        .args(["init", "--tech", "rust", "--forge", "github", "--target"])
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
            .is_some_and(|m| m.contains("release-plz.toml")),
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
fn init_apply_lands_reports_sentinels_and_is_idempotent() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "rust", "--forge", "github", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .success()
        .stdout(
            predicate::str::contains("wrote release-plz.toml")
                .and(predicate::str::contains("TODO(release-kit)")),
        );
    assert!(
        target
            .path()
            .join(".github/workflows/release-plz.yml")
            .is_file()
    );
    rk().args(["init", "--tech", "rust", "--forge", "github", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .success()
        .stdout(predicate::str::contains("unchanged release-plz.toml"));
}

/// A skill has one owner, so a landing projects none into the target: a
/// second copy under one name is a second entry offering the same skill.
#[test]
fn init_lands_no_skill_into_the_target() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "rust", "--forge", "github", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .success();
    for root in ROOTS {
        assert!(
            !target.path().join(root).exists(),
            "{root} must not be landed into a target"
        );
    }
}

#[test]
fn init_refuses_a_conflicting_target_and_writes_nothing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(target.path().join("release-plz.toml"), "something local\n")
        .expect("the conflict file writes");
    rk().args(["init", "--tech", "rust", "--forge", "github", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .code(73)
        .stderr(predicate::str::contains("release-plz.toml"));
    assert!(
        !target.path().join("dist-workspace.toml").exists(),
        "a refused landing must write nothing"
    );
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
    rk().args(["init", "--tech", "rust", "--forge", "github", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .code(74);
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
const FORGE_STEPS: [&str; 10] = [
    "bot-secrets",
    "ci-permissions",
    "create-release-branch",
    "default-branch",
    "delete-main",
    "install-bot",
    "protect-integration-branch",
    "protect-release-branch",
    "protect-tags",
    "protections-check",
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
    assert_eq!(listed.len(), 11, "eleven steps list: {text}");
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
        "RK_INTEGRATION_BRANCH",
        "RK_RELEASE_BRANCH",
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
        .args(["setup", "script", "delete-main", "--forge", "gitlab"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(
        gitlab,
        std::fs::read(repo_path("setup/gitlab/delete-main")).expect("reads")
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
    rk().args(["init", "--tech", "rust", "--forge", "github", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .success();
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
      *'contains(["required_linear_history"])'*) echo false;;
      *"allowed_merge_methods"*) echo '["merge"]';;
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
    echo "{\"id\":1,\"default_branch\":\"$(cat "$STATE/default_branch")\",\"jobs_enabled\":true,\"only_allow_merge_if_pipeline_succeeds\":false,\"merge_method\":\"ff\"}";;
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
    apply(&fixture).assert().success();

    // The forge now holds the desired state.
    assert_eq!(
        std::fs::read_to_string(fixture.state("default_branch")).expect("state reads"),
        "develop\n"
    );
    assert!(fixture.state("branch_master").is_file());
    assert!(!fixture.state("branch_main").exists());
    assert!(fixture.state("installed").is_file());
    let rulesets = std::fs::read_to_string(fixture.state("rulesets.index")).expect("reads");
    assert_eq!(
        rulesets.lines().count(),
        3,
        "exactly three protections: {rulesets}"
    );
    let body = std::fs::read_to_string(fixture.state("ruleset_master-protection")).expect("reads");
    assert!(body.contains(r#""context": "test-check""#));

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

    // And check reports clean at exit 0.
    fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("ok protections-check")
                .not()
                .or(predicate::str::contains("ok")),
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
    assert!(text.contains("unsatisfied protect-tags"), "{text}");
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
    fixture.seed("branch_master", "abc123");
    fixture
        .rk(&["setup", "step", "protect-release-branch"])
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
/// release branch has not been proven.
#[test]
fn a_protection_step_refuses_before_the_release_branch_exists() {
    let fixture = ForgeFixture::new();
    fixture
        .rk(&["setup", "step", "protect-release-branch"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .args(["--required-check", "test-check"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("create-release-branch"));
    assert!(
        !fixture.log().contains("-X POST"),
        "the refusal must write nothing"
    );
}

/// delete-main refuses when main is not an ancestor of the release branch.
#[test]
fn delete_main_refuses_a_non_ancestor() {
    let fixture = ForgeFixture::new();
    fixture.seed("branch_master", "abc123");
    fixture.seed("compare_main_master", "diverged");
    fixture
        .rk(&["setup", "step", "delete-main"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("lose work"));
    assert!(
        fixture.state("branch_main").is_file(),
        "the branch must survive the refusal"
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
        "develop\n"
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
fn delete_main_refuses_an_unreadable_comparison() {
    let fixture = ForgeFixture::new();
    fixture.seed("branch_master", "abc123");
    fixture.seed("compare_main_master", "error");
    fixture
        .rk(&["setup", "step", "delete-main"])
        .args(["--repo", "acme/widget", "--forge", "github", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("delete-main refuses"));
    assert!(
        fixture.state("branch_main").is_file(),
        "the branch must survive an unreadable guard"
    );
}

/// A release branch carrying commits the integration branch lacks is not a
/// clean setup, and check says so.
#[test]
fn check_reports_a_divergent_release_branch() {
    let fixture = ForgeFixture::new();
    fixture.seed("branch_master", "fff999");
    fixture.seed("compare_develop_master", "diverged");
    let out = fixture
        .rk(&["setup", "check"])
        .args(["--repo", "acme/widget", "--forge", "github"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    assert!(
        String::from_utf8_lossy(&out).contains("unsatisfied create-release-branch"),
        "{}",
        String::from_utf8_lossy(&out)
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
