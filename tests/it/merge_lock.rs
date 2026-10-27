//! `outdatty merge-lock` as git's merge driver for `outdatty.lock`: each test
//! builds a throwaway repo that registers the driver, records the lock on two
//! branches and merges them with a real `git merge`.

use std::path::Path;
use std::process::Output;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

/// The hash a key both sides changed gets: blake3's shape, and no file's hash.
const UNMATCHED: &str = "0000000000000000000000000000000000000000000000000000000000000000";

const MANIFEST: &str = "require_tracked: [\"!**\"]
groups:
  - name: alpha
    source: [a1.rs, a2.rs]
    dependents: [a.md]
  - name: beta
    source: [b.rs]
    dependents: [b.md]
";

/// A five-line file, so an edit to its first line and one to its last merge
/// as text without a conflict.
const FIVE: &str = "one\ntwo\nthree\nfour\nfive\n";

fn bin() -> Command {
    Command::cargo_bin("outdatty").expect("binary builds")
}

fn write(dir: &Path, name: &str, body: &str) {
    std::fs::write(dir.join(name), body).expect("write file");
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

/// Runs `git <args>` in `dir`, isolated from any repo or hook the test runs
/// under, and returns its output whatever the exit code.
fn git_output(dir: &Path, args: &[&str]) -> Output {
    std::process::Command::new("git")
        .current_dir(dir)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("git runs")
}

fn git(dir: &Path, args: &[&str]) {
    let out = git_output(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn update(dir: &Path, args: &[&str]) {
    bin()
        .current_dir(dir)
        .arg("update")
        .args(args)
        .assert()
        .success();
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

/// A repo on `main` whose lock records every group, with the driver
/// registered as the README says.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path();
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.name", "test"]);
    git(path, &["config", "user.email", "test@example.invalid"]);
    git(path, &["config", "commit.gpgsign", "false"]);
    let driver = format!(
        "'{}' merge-lock %O %A %B",
        assert_cmd::cargo::cargo_bin("outdatty").display()
    );
    git(path, &["config", "merge.outdatty.driver", &driver]);
    write(path, ".gitattributes", "outdatty.lock merge=outdatty\n");
    write(path, "outdatty.yaml", MANIFEST);
    for name in ["a1.rs", "a2.rs", "a.md", "b.rs", "b.md"] {
        write(path, name, FIVE);
    }
    update(path, &[]);
    commit_all(path, "base");
    dir
}

/// On a `feat` branch from main, runs `feat`, then back on main runs `main`,
/// and merges `feat` into main.
fn merge_after(dir: &Path, feat: impl Fn(&Path), main: impl Fn(&Path)) -> Output {
    git(dir, &["checkout", "-q", "-b", "feat"]);
    feat(dir);
    commit_all(dir, "feat");
    git(dir, &["checkout", "-q", "main"]);
    main(dir);
    commit_all(dir, "main");
    git_output(dir, &["merge", "-q", "--no-edit", "feat"])
}

#[test]
fn hashes_each_side_recorded_merge_without_a_conflict() {
    let dir = repo();
    let path = dir.path();
    // The two source hashes sit on adjacent lines, which git's text merge
    // reports as a conflict.
    let out = merge_after(
        path,
        |dir| {
            write(dir, "a1.rs", "feat\n");
            update(dir, &["--group", "alpha"]);
        },
        |dir| {
            write(dir, "a2.rs", "main\n");
            update(dir, &["--group", "alpha"]);
        },
    );
    assert!(
        out.status.success(),
        "merge failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin().current_dir(path).arg("check").assert().success();
}

#[test]
fn a_hash_both_sides_changed_fails_check_until_reviewed() {
    let dir = repo();
    let path = dir.path();
    let out = merge_after(
        path,
        |dir| {
            write(dir, "a1.rs", &FIVE.replace("five", "FIVE"));
            update(dir, &["--group", "alpha"]);
        },
        |dir| {
            write(dir, "a1.rs", &FIVE.replace("one", "ONE"));
            update(dir, &["--group", "alpha"]);
        },
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "merge failed: {stderr}");
    assert!(
        stderr.contains("alpha") && stderr.contains("a1.rs"),
        "names the group and the path: {stderr}"
    );
    assert!(read(path, "outdatty.lock").contains(UNMATCHED));
    bin()
        .current_dir(path)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains("alpha").and(contains("a1.rs")));
}

#[test]
fn a_group_one_side_renamed_does_not_come_back() {
    let dir = repo();
    let path = dir.path();
    let out = merge_after(
        path,
        |dir| {
            write(dir, "b.md", "reviewed\n");
            update(dir, &["--group", "beta", "--dependent", "b.md"]);
        },
        |dir| {
            write(dir, "outdatty.yaml", &MANIFEST.replace("beta", "gamma"));
            update(dir, &[]);
        },
    );
    assert!(
        out.status.success(),
        "merge failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lock = read(path, "outdatty.lock");
    assert!(!lock.contains("beta"), "beta is gone: {lock}");
    bin().current_dir(path).arg("check").assert().success();
}

#[test]
fn a_side_that_is_no_lock_fails_and_leaves_ours() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path();
    let lock = "version: 1\nalgorithm: blake3\ngroups: {}\n";
    write(path, "base", lock);
    write(path, "ours", lock);
    write(path, "theirs", "not: [a lock");
    bin()
        .current_dir(path)
        .args(["merge-lock", "base", "ours", "theirs"])
        .assert()
        .code(2)
        .stderr(contains("theirs"));
    assert_eq!(read(path, "ours"), lock);
}
