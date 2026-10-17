//! A lockfile group the manifest no longer declares fails `check` until a
//! plain `update` prunes it, so split or renamed groups leave no stale hashes.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

fn bin() -> Command {
    Command::cargo_bin("outdatty").expect("binary builds")
}

fn write(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write file");
}

const PAIR: &str = "  - name: pair\n    source: [code.rs]\n    dependents: [doc.md]\n";

/// Records `pair` and `gone`, then drops `gone` from the manifest.
fn lock_with_orphan() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    write(
        dir.path(),
        "outdatty.yaml",
        &format!("groups:\n{PAIR}  - name: gone\n    source: [code.rs]\n"),
    );
    bin().current_dir(&dir).arg("update").assert().success();
    write(dir.path(), "outdatty.yaml", &format!("groups:\n{PAIR}"));
    dir
}

#[test]
fn check_fails_naming_the_orphaned_group() {
    let dir = lock_with_orphan();
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(
            contains("orphaned group:    gone")
                .and(contains("outdatty update"))
                .and(contains("1 orphaned lock group")),
        );
}

#[test]
fn json_and_quiet_carry_the_orphan() {
    let dir = lock_with_orphan();
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "json"])
        .assert()
        .code(1)
        .stdout(contains("\"orphaned\": [\n    \"gone\"\n  ]").and(contains("\"failed\": true")));
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "quiet"])
        .assert()
        .code(1)
        .stderr(contains("orphaned group:    gone"));
}

#[test]
fn status_lists_the_orphan_without_failing() {
    let dir = lock_with_orphan();
    bin()
        .current_dir(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("orphaned group:    gone"));
}

#[test]
fn scoped_check_ignores_the_orphan() {
    let dir = lock_with_orphan();
    bin()
        .current_dir(&dir)
        .args(["check", "--group", "pair"])
        .assert()
        .success();
}

#[test]
fn plain_update_prunes_the_orphan() {
    let dir = lock_with_orphan();
    bin()
        .current_dir(&dir)
        .arg("update")
        .assert()
        .success()
        .stdout(contains("removed  gone"));
    bin().current_dir(&dir).arg("check").assert().success();
}
