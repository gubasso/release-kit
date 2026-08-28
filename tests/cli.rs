//! End-to-end tests over the built binary: every subcommand, the landing
//! round-trip, and the payload's presence in the embedded form.

// Integration tests: assertion style is the point, so the production
// restrictions on unwrap/expect/panic do not apply here.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use assert_cmd::Command;
use predicates::prelude::*;

fn rk() -> Command {
    Command::cargo_bin("rk").expect("the rk binary builds")
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

#[test]
fn snippet_lists_every_landable_file() {
    rk().args(["snippet", "--list"]).assert().success().stdout(
        predicate::str::contains("rust/release-plz.toml")
            .and(predicate::str::contains(
                "rust/.github/workflows/release-plz.yml",
            ))
            .and(predicate::str::contains(
                "python/.github/workflows/release-please.yml",
            ))
            .and(predicate::str::contains("bash/VERSION"))
            .and(predicate::str::contains("bash/cliff.toml")),
    );
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

#[test]
fn init_dry_runs_by_default_and_writes_nothing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "rust", "--target"])
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
    rk().args(["init", "--tech", "rust", "--target"])
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
    rk().args(["init", "--tech", "rust", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .success()
        .stdout(predicate::str::contains("unchanged release-plz.toml"));
}

#[test]
fn init_refuses_a_conflicting_target_and_writes_nothing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    std::fs::write(target.path().join("release-plz.toml"), "something local\n")
        .expect("the conflict file writes");
    rk().args(["init", "--tech", "rust", "--target"])
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
    rk().args(["init", "--tech", "fortran", "--target"])
        .arg(target.path())
        .assert()
        .code(64)
        .stderr(predicate::str::contains("unknown tech"));
}

#[test]
fn init_refuses_a_missing_target() {
    rk().args(["init", "--tech", "rust", "--target", "/no/such/dir"])
        .assert()
        .code(73);
}

#[test]
fn skill_install_previews_into_a_scratch_home() {
    let home = tempfile::tempdir().expect("a scratch home exists");
    rk().env("HOME", home.path())
        .args(["skill", "install"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DRY RUN"));
    assert!(!home.path().join(".claude").exists());
}

#[test]
fn skill_install_apply_and_uninstall_round_trip() {
    let home = tempfile::tempdir().expect("a scratch home exists");
    rk().env("HOME", home.path())
        .args(["skill", "install", "--apply"])
        .assert()
        .success();
    for scope in [".claude/skills", ".agents/skills"] {
        for name in ["rk-setup", "rk-release"] {
            assert!(
                home.path()
                    .join(scope)
                    .join(name)
                    .join("SKILL.md")
                    .is_file()
            );
        }
    }
    rk().env("HOME", home.path())
        .args(["skill", "uninstall", "--apply"])
        .assert()
        .success();
    assert!(!home.path().join(".claude/skills/rk-setup").exists());
}

#[test]
fn skill_install_refuses_a_differing_destination_without_force() {
    let home = tempfile::tempdir().expect("a scratch home exists");
    let dir = home.path().join(".claude/skills/rk-release");
    std::fs::create_dir_all(&dir).expect("the skill dir creates");
    // Non-UTF-8 bytes: the existing file must still count as a conflict.
    std::fs::write(dir.join("SKILL.md"), [0xff, 0xfe, 0x00]).expect("the conflict file writes");
    rk().env("HOME", home.path())
        .args(["skill", "install", "--apply"])
        .assert()
        .code(73)
        .stderr(predicate::str::contains("--force"));
    assert_eq!(
        std::fs::read(dir.join("SKILL.md")).expect("the conflict file survives"),
        [0xff, 0xfe, 0x00],
        "a refused install must not overwrite"
    );
    rk().env("HOME", home.path())
        .args(["skill", "install", "--apply", "--force"])
        .assert()
        .success();
    assert!(
        std::fs::read_to_string(dir.join("SKILL.md"))
            .expect("the forced install wrote the payload")
            .contains("name: rk-release")
    );
}

#[test]
fn init_propagates_an_unreadable_destination_and_writes_nothing() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    // A directory where a file should land fails the pre-write read pass.
    std::fs::create_dir(target.path().join("release-plz.toml")).expect("the blocking dir creates");
    rk().args(["init", "--tech", "rust", "--target"])
        .arg(target.path())
        .arg("--apply")
        .assert()
        .code(74);
    assert!(
        !target.path().join("dist-workspace.toml").exists(),
        "a failed pre-write pass must write nothing"
    );
}
