//! End-to-end tests driving the compiled `outdatty` binary.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn bin() -> Command {
    Command::cargo_bin("outdatty").expect("binary builds")
}

fn write(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(path, body).expect("write file");
}

/// Writes a minimal manifest coupling `code.rs` to `doc.md`.
fn manifest(dir: &Path) {
    write(
        dir,
        "outdatty.yaml",
        "groups:\n  - name: pair\n    source: [code.rs]\n    dependents: [doc.md]\n",
    );
}

#[test]
fn init_writes_manifest_and_refuses_to_clobber() {
    let dir = tempfile::tempdir().expect("tempdir");
    bin().current_dir(&dir).arg("init").assert().success();
    assert!(
        dir.path().join("outdatty.yaml").exists(),
        "manifest written"
    );
    bin()
        .current_dir(&dir)
        .arg("init")
        .assert()
        .code(2)
        .stderr(contains("already exists"));
    bin()
        .current_dir(&dir)
        .args(["init", "--force"])
        .assert()
        .success();
}

#[test]
fn check_without_lock_reports_new_group() {
    let dir = tempfile::tempdir().expect("tempdir");
    manifest(dir.path());
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains("out of date"));
}

#[test]
fn full_lifecycle_update_check_drift_resync() {
    let dir = tempfile::tempdir().expect("tempdir");
    manifest(dir.path());
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");

    bin()
        .current_dir(&dir)
        .arg("update")
        .assert()
        .success()
        .stdout(contains("added"));
    assert!(
        dir.path().join("outdatty.lock").exists(),
        "lockfile written"
    );
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .success()
        .stdout(contains("none out of date"));

    write(dir.path(), "code.rs", "changed");
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains("stale").and(contains("source changed")));

    bin().current_dir(&dir).arg("update").assert().success();
    bin().current_dir(&dir).arg("check").assert().success();
}

#[test]
fn json_format_emits_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    manifest(dir.path());
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin().current_dir(&dir).arg("update").assert().success();
    write(dir.path(), "code.rs", "changed");
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "json"])
        .assert()
        .code(1)
        .stdout(contains("\"status\": \"stale\""));
}

#[test]
fn deleted_literal_source_is_drift_not_crash() {
    let dir = tempfile::tempdir().expect("tempdir");
    manifest(dir.path());
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin().current_dir(&dir).arg("update").assert().success();

    std::fs::remove_file(dir.path().join("code.rs")).expect("remove source");
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains("stale").and(contains("source changed")));
}

#[test]
fn duplicate_group_name_is_operational_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "outdatty.yaml",
        "groups:\n  - name: pair\n    source: [code.rs]\n    dependents: [doc.md]\n  - name: pair\n    source: [code.rs]\n    dependents: [doc.md]\n",
    );
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(2)
        .stderr(contains("duplicate group name"));
}

#[test]
fn unknown_group_is_operational_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    manifest(dir.path());
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin()
        .current_dir(&dir)
        .args(["status", "--group", "ghost"])
        .assert()
        .code(2)
        .stderr(contains("no such group"));
}

#[test]
fn color_always_emits_ansi_never_is_plain() {
    let dir = tempfile::tempdir().expect("tempdir");
    manifest(dir.path());
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");

    bin()
        .current_dir(&dir)
        .args(["check", "--color", "always"])
        .assert()
        .code(1)
        .stdout(contains("\u{1b}["));
    bin()
        .current_dir(&dir)
        .args(["check", "--color", "never"])
        .assert()
        .code(1)
        .stdout(contains("\u{1b}[").not());
}

#[test]
fn status_reports_drift_without_failing() {
    let dir = tempfile::tempdir().expect("tempdir");
    manifest(dir.path());
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin().current_dir(&dir).arg("update").assert().success();
    write(dir.path(), "code.rs", "changed");
    bin()
        .current_dir(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("stale"));
}
