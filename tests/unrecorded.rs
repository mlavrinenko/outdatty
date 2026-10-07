//! A file a glob newly expands to, with no hash in the lockfile, must fail
//! `check` until it is recorded.

use std::path::Path;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::{contains, is_empty};

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

/// A recorded directed group whose dependents glob later gains `docs/new.md`.
fn recorded_with_new_page(bidirectional: bool) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let flag = if bidirectional {
        "    bidirectional: true\n"
    } else {
        ""
    };
    write(
        dir.path(),
        "outdatty.yaml",
        &format!(
            "require_tracked: []\ngroups:\n  - name: pages\n    source: [code.rs]\n    dependents: ['docs/*.md']\n{flag}"
        ),
    );
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "docs/old.md", "b");
    bin().current_dir(&dir).arg("update").assert().success();
    write(dir.path(), "docs/new.md", "c");
    dir
}

const RECORD_IT: &str = "outdatty update --group pages --dependent docs/new.md";

#[test]
fn plain_check_names_the_file_and_the_recording_command() {
    let dir = recorded_with_new_page(false);
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(
            contains("unrecorded: docs/new.md")
                .and(contains(format!("record it: {RECORD_IT}")))
                .and(contains("outdatty: 1 of 1 group out of date")),
        );
}

#[test]
fn status_shows_it_without_failing() {
    let dir = recorded_with_new_page(false);
    bin()
        .current_dir(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("unrecorded: docs/new.md"));
}

#[test]
fn json_check_lists_unrecorded_dependents() {
    let dir = recorded_with_new_page(false);
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "json"])
        .assert()
        .code(1)
        .stdout(
            contains("\"status\": \"unrecorded\"")
                .and(contains(
                    "\"unrecorded_dependents\": [\n        \"docs/new.md\"\n      ]",
                ))
                .and(contains("\"failed\": true")),
        );
}

#[test]
fn quiet_check_fails_silently() {
    let dir = recorded_with_new_page(false);
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "quiet"])
        .assert()
        .code(1)
        .stdout(is_empty());
}

#[test]
fn paths_formats_list_the_unrecorded_file() {
    let dir = recorded_with_new_page(false);
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "paths"])
        .assert()
        .code(1)
        .stdout("docs/new.md\n");
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "paths0"])
        .assert()
        .code(1)
        .stdout("docs/new.md\0");
}

#[test]
fn recording_the_named_dependent_clears_the_failure() {
    let dir = recorded_with_new_page(false);
    bin()
        .current_dir(&dir)
        .args(["update", "--group", "pages", "--dependent", "docs/new.md"])
        .assert()
        .success();
    bin().current_dir(&dir).arg("check").assert().success();
}

#[test]
fn bidirectional_group_also_names_the_command() {
    let dir = recorded_with_new_page(true);
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains(RECORD_IT));
}

#[test]
fn a_new_source_file_already_fails_as_a_changed_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "outdatty.yaml",
        "require_tracked: []\ngroups:\n  - name: pages\n    source: ['src/*.rs']\n    dependents: [doc.md]\n",
    );
    write(dir.path(), "src/a.rs", "a");
    write(dir.path(), "doc.md", "b");
    bin().current_dir(&dir).arg("update").assert().success();
    write(dir.path(), "src/b.rs", "c");
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains("source changed:    src/b.rs"));
}

#[test]
fn fresh_manifest_without_lock_stays_new_not_unrecorded() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "outdatty.yaml",
        "require_tracked: []\ngroups:\n  - name: pages\n    source: [code.rs]\n    dependents: ['docs/*.md']\n",
    );
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "docs/new.md", "c");
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains("[  new  ]").and(contains("unrecorded").not()));
    bin()
        .current_dir(&dir)
        .args(["check", "--format", "json"])
        .assert()
        .code(1)
        .stdout(contains("\"status\": \"new\"").and(contains("\"unrecorded_dependents\": []")));
}
