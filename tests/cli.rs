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

/// A skill has one owner, so a landing projects none into the target: a
/// second copy under one name is a second entry offering the same skill.
#[test]
fn init_lands_no_skill_into_the_target() {
    let target = tempfile::tempdir().expect("a scratch dir exists");
    rk().args(["init", "--tech", "rust", "--target"])
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
