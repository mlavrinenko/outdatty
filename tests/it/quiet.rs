//! `--format quiet` is what a memoized gate runs: silent on a pass, but a
//! failing `check` names what failed on stderr so the red arm explains itself.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_empty};

fn bin() -> Command {
    Command::cargo_bin("outdatty").expect("binary builds")
}

fn write(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write file");
}

/// A recorded group coupling `code.rs` to `doc.md`.
fn recorded() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "outdatty.yaml",
        "groups:\n  - name: pair\n    source: [code.rs]\n    dependents: [doc.md]\n",
    );
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin().current_dir(&dir).arg("update").assert().success();
    dir
}

#[test]
fn quiet_check_prints_nothing_on_a_clean_tree() {
    let dir = recorded();
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "quiet"])
        .assert()
        .success()
        .stdout(is_empty())
        .stderr(is_empty());
}

#[test]
fn quiet_check_names_the_stale_group_on_stderr() {
    let dir = recorded();
    write(dir.path(), "code.rs", "changed");
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "quiet"])
        .assert()
        .code(1)
        .stdout(is_empty())
        .stderr(
            contains("[ stale ]  pair")
                .and(contains("source changed:    code.rs"))
                .and(contains("review dependent:  doc.md"))
                .and(contains("confirm with:      outdatty update --group pair"))
                .and(contains("outdatty: 1 of 1 group out of date")),
        );
}

#[test]
fn quiet_status_and_update_stay_silent() {
    let dir = recorded();
    write(dir.path(), "code.rs", "changed");
    for command in ["status", "update"] {
        bin()
            .current_dir(&dir)
            .args([command, "--format", "quiet"])
            .assert()
            .success()
            .stdout(is_empty())
            .stderr(is_empty());
    }
}
