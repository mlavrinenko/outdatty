#![allow(clippy::too_many_lines)]
use std::path::Path;

use super::{Filter, Status, UpdateAction, build, classify, diff, evaluate, ids};
use crate::lock::Lockfile;
use crate::manifest::{Group, Manifest};

fn write(base: &Path, name: &str, body: &str) {
    std::fs::write(base.join(name), body).expect("write");
}

fn manifest_with(group: Group) -> Manifest {
    Manifest {
        groups: vec![group],
        ..Manifest::default()
    }
}

fn pair_group() -> Group {
    Group {
        name: "pair".to_owned(),
        source: vec!["code.rs".to_owned()],
        dependents: vec!["doc.md".to_owned()],
        bidirectional: false,
    }
}

#[test]
fn diff_reports_added_removed_and_changed() {
    let mut current = std::collections::BTreeMap::new();
    current.insert("same".to_owned(), "h".to_owned());
    current.insert("changed".to_owned(), "new".to_owned());
    current.insert("added".to_owned(), "h".to_owned());
    let mut locked = std::collections::BTreeMap::new();
    locked.insert("same".to_owned(), "h".to_owned());
    locked.insert("changed".to_owned(), "old".to_owned());
    locked.insert("removed".to_owned(), "h".to_owned());
    assert_eq!(diff(&current, &locked), vec!["added", "changed", "removed"]);
}

#[test]
fn classify_covers_the_truth_table() {
    // Directed dependent-only change is allowed; it stays Ok.
    assert_eq!(classify(false, false, false), Status::Ok);
    assert_eq!(classify(false, false, true), Status::Ok);
    assert_eq!(classify(false, true, false), Status::Stale);
    assert_eq!(classify(true, false, true), Status::Stale);
}

#[test]
fn ids_returns_group_names_in_order() {
    let manifest = Manifest {
        groups: vec![
            pair_group(),
            Group {
                name: "second".to_owned(),
                source: vec!["x".to_owned()],
                ..Group::default()
            },
        ],
        ..Manifest::default()
    };
    assert_eq!(ids(&manifest), vec!["pair".to_owned(), "second".to_owned()]);
}

#[test]
fn new_group_without_lock_is_a_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    let manifest = manifest_with(pair_group());
    let report =
        evaluate(&manifest, &Lockfile::default(), dir.path(), &Filter::All).expect("evaluate");
    assert_eq!(report.groups.first().expect("group").status, Status::New);
    assert!(report.has_failure());
}

#[test]
fn in_sync_after_build_then_evaluate() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    let manifest = manifest_with(pair_group());
    let (lock, update) =
        build(&manifest, &Lockfile::default(), dir.path(), &Filter::All).expect("build");
    assert_eq!(
        update.entries.first().expect("entry").action,
        UpdateAction::Added
    );
    let report = evaluate(&manifest, &lock, dir.path(), &Filter::All).expect("evaluate");
    assert_eq!(report.groups.first().expect("group").status, Status::Ok);
    assert!(!report.has_failure());
}

#[test]
fn changed_source_makes_group_stale() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    let manifest = manifest_with(pair_group());
    let (lock, _) =
        build(&manifest, &Lockfile::default(), dir.path(), &Filter::All).expect("build");
    write(dir.path(), "code.rs", "changed");
    let report = evaluate(&manifest, &lock, dir.path(), &Filter::All).expect("evaluate");
    let group = report.groups.first().expect("group");
    assert_eq!(group.status, Status::Stale);
    assert_eq!(group.changed_sources, vec!["code.rs".to_owned()]);
    assert_eq!(
        group.dependents,
        vec!["doc.md".to_owned()],
        "the group's declared dependents are review targets regardless of status"
    );
}

#[test]
fn dependent_change_alone_is_allowed_when_directed() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    let manifest = manifest_with(pair_group());
    let (lock, _) =
        build(&manifest, &Lockfile::default(), dir.path(), &Filter::All).expect("build");
    write(dir.path(), "doc.md", "edited");
    let report = evaluate(&manifest, &lock, dir.path(), &Filter::All).expect("evaluate");
    let group = report.groups.first().expect("group");
    assert_eq!(group.status, Status::Ok, "directed dependent edit stays ok");
    assert_eq!(
        group.changed_dependents,
        vec!["doc.md".to_owned()],
        "the differing dependent is still recorded for machine consumers"
    );
    assert!(
        !report.has_failure(),
        "dependent-only change does not fail check"
    );
}

#[test]
fn dependent_change_fails_when_bidirectional() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    let mut group = pair_group();
    group.bidirectional = true;
    let manifest = manifest_with(group);
    let (lock, _) =
        build(&manifest, &Lockfile::default(), dir.path(), &Filter::All).expect("build");
    write(dir.path(), "doc.md", "edited");
    let report = evaluate(&manifest, &lock, dir.path(), &Filter::All).expect("evaluate");
    assert_eq!(report.groups.first().expect("group").status, Status::Stale);
}

#[test]
fn build_all_prunes_orphans_but_filtered_keeps_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "code.rs", "a");
    write(dir.path(), "doc.md", "b");
    let manifest = manifest_with(pair_group());
    let (mut lock, _) =
        build(&manifest, &Lockfile::default(), dir.path(), &Filter::All).expect("build");
    lock.groups.insert("orphan".to_owned(), Default::default());

    let only = Filter::Only(vec!["pair".to_owned()]);
    let (kept, _) = build(&manifest, &lock, dir.path(), &only).expect("build");
    assert!(
        kept.groups.contains_key("orphan"),
        "filtered build keeps orphans"
    );

    let (pruned, report) = build(&manifest, &lock, dir.path(), &Filter::All).expect("build");
    assert!(
        !pruned.groups.contains_key("orphan"),
        "full build prunes orphans"
    );
    assert!(
        report
            .entries
            .iter()
            .any(|entry| entry.id == "orphan" && entry.action == UpdateAction::Removed),
        "pruned orphan is reported as removed"
    );
}
