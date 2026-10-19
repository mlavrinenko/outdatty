//! A manifest's `root` key moves where its paths and lock keys resolve from,
//! so the pair can live in `.config/` with entries written from the repo root.

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
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(path, body).expect("write file");
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).expect("read file")
}

const GROUPS: &str = "groups:\n  - name: pair\n    source: [code.rs]\n    dependents: [doc.md]\n";

/// The lock `update` writes for [`GROUPS`] over `code.rs` = `a`, `doc.md` = `b`.
const LOCK: &str = "version: 1
algorithm: blake3
groups:
  pair:
    source:
      code.rs: 17762fddd969a453925d65717ac3eea21320b66b54342fde15128d6caf21215f
    dependents:
      doc.md: 10e5cf3d3c8a4f9f3468c8cc58eea84892a22fdadbc1acb22410190044c1d553
";

/// A repo with `code.rs` and `doc.md` at its top and the manifest in `.config/`.
fn config_dir_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    write(
        dir.path(),
        ".config/outdatty.yaml",
        &format!("root: ..\n{GROUPS}"),
    );
    dir
}

const CONFIG_FLAGS: [&str; 4] = [
    "--manifest",
    ".config/outdatty.yaml",
    "--lock",
    ".config/outdatty.lock",
];

#[test]
fn without_root_paths_resolve_from_the_manifest_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "sub/code.rs", "a");
    write(dir.path(), "sub/doc.md", "b");
    write(dir.path(), "sub/outdatty.yaml", GROUPS);
    let flags = ["--manifest", "sub/outdatty.yaml"];
    bin()
        .current_dir(&dir)
        .args(flags)
        .arg("update")
        .assert()
        .success();
    assert_eq!(read(dir.path(), "sub/outdatty.lock"), LOCK);

    bin()
        .current_dir(&dir)
        .args(flags)
        .arg("update")
        .assert()
        .success();
    assert_eq!(
        read(dir.path(), "sub/outdatty.lock"),
        LOCK,
        "a second update rewrites the lock byte for byte"
    );
    bin()
        .current_dir(&dir)
        .args(flags)
        .arg("check")
        .assert()
        .success()
        .stdout("outdatty: 1 group up to date\n");
}

#[test]
fn root_keys_the_lock_from_the_root_not_the_manifest() {
    let dir = config_dir_project();
    bin()
        .current_dir(&dir)
        .args(CONFIG_FLAGS)
        .arg("update")
        .assert()
        .success();
    assert_eq!(
        read(dir.path(), ".config/outdatty.lock"),
        LOCK,
        "the same lock a root-level pair writes, with no `../` keys"
    );
    bin()
        .current_dir(&dir)
        .args(CONFIG_FLAGS)
        .arg("check")
        .assert()
        .success()
        .stdout("outdatty: 1 group up to date\n");

    write(dir.path(), "code.rs", "changed");
    bin()
        .current_dir(&dir)
        .args(CONFIG_FLAGS)
        .arg("check")
        .assert()
        .code(1)
        .stdout(contains("source changed:    code.rs"));
}

#[test]
fn a_root_level_lock_moved_into_config_stays_green() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    write(dir.path(), "outdatty.yaml", GROUPS);
    bin().current_dir(&dir).arg("update").assert().success();

    let lock = read(dir.path(), "outdatty.lock");
    std::fs::remove_file(dir.path().join("outdatty.yaml")).expect("remove manifest");
    std::fs::remove_file(dir.path().join("outdatty.lock")).expect("remove lock");
    write(
        dir.path(),
        ".config/outdatty.yaml",
        &format!("root: ..\n{GROUPS}"),
    );
    write(dir.path(), ".config/outdatty.lock", &lock);
    bin()
        .current_dir(&dir)
        .args(CONFIG_FLAGS)
        .arg("check")
        .assert()
        .success()
        .stdout("outdatty: 1 group up to date\n");
}

#[test]
fn root_resolves_the_same_from_any_working_directory() {
    let dir = config_dir_project();
    std::fs::create_dir(dir.path().join("deep")).expect("mkdir");
    bin()
        .current_dir(&dir)
        .args(CONFIG_FLAGS)
        .arg("update")
        .assert()
        .success();

    bin()
        .current_dir(dir.path().join("deep"))
        .args(["--manifest", "../.config/outdatty.yaml", "check"])
        .assert()
        .success()
        .stdout("outdatty: 1 group up to date\n");
    let absolute = dir.path().join(".config/outdatty.yaml");
    bin()
        .current_dir(dir.path().join("deep"))
        .arg("--manifest")
        .arg(&absolute)
        .arg("check")
        .assert()
        .success()
        .stdout("outdatty: 1 group up to date\n");
}

#[test]
fn discovery_finds_the_config_dir_manifest() {
    let dir = config_dir_project();
    bin().current_dir(&dir).arg("update").assert().success();
    assert_eq!(read(dir.path(), ".config/outdatty.lock"), LOCK);
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .success()
        .stdout("outdatty: 1 group up to date\n");
}

#[test]
fn a_root_level_manifest_wins_discovery() {
    let dir = config_dir_project();
    write(
        dir.path(),
        "outdatty.yaml",
        "groups:\n  - name: top\n    source: [code.rs]\n",
    );
    bin()
        .current_dir(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(contains("top").and(contains("pair").not()));
}

#[test]
fn a_missing_root_is_an_operational_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(
        dir.path(),
        "outdatty.yaml",
        &format!("root: nope\n{GROUPS}"),
    );
    bin()
        .current_dir(&dir)
        .arg("check")
        .assert()
        .code(2)
        .stderr(contains("root `nope`"));
}
