use std::collections::BTreeMap;

use super::{Contested, UNMATCHED, merge, merge_files};
use crate::lock::{GroupSnapshot, Lockfile};

/// A lockfile from `(group, source path, hash)` triples.
fn lock(entries: &[(&str, &str, &str)]) -> Lockfile {
    let mut lock = Lockfile::default();
    for (group, path, hash) in entries {
        lock.groups
            .entry((*group).to_owned())
            .or_default()
            .source
            .insert((*path).to_owned(), (*hash).to_owned());
    }
    lock
}

fn source(lock: &Lockfile, group: &str) -> BTreeMap<String, String> {
    lock.groups
        .get(group)
        .map(|snapshot| snapshot.source.clone())
        .unwrap_or_default()
}

fn hashes(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, hash)| ((*path).to_owned(), (*hash).to_owned()))
        .collect()
}

#[test]
fn each_side_keeps_the_hashes_it_changed() {
    let base = lock(&[("g", "a", "a0"), ("g", "b", "b0"), ("h", "c", "c0")]);
    let ours = lock(&[("g", "a", "a1"), ("g", "b", "b0"), ("h", "c", "c0")]);
    let theirs = lock(&[("g", "a", "a0"), ("g", "b", "b1"), ("h", "c", "c1")]);
    let (merged, contested) = merge(&base, &ours, &theirs);
    assert!(contested.is_empty());
    assert_eq!(source(&merged, "g"), hashes(&[("a", "a1"), ("b", "b1")]));
    assert_eq!(source(&merged, "h"), hashes(&[("c", "c1")]));
}

#[test]
fn the_same_change_on_both_sides_is_no_contest() {
    let base = lock(&[("g", "a", "a0")]);
    let both = lock(&[("g", "a", "a1")]);
    let (merged, contested) = merge(&base, &both, &both);
    assert!(contested.is_empty());
    assert_eq!(source(&merged, "g"), hashes(&[("a", "a1")]));
}

#[test]
fn a_hash_both_sides_changed_is_unmatched() {
    let base = lock(&[("g", "a", "a0"), ("g", "b", "b0")]);
    let ours = lock(&[("g", "a", "a1"), ("g", "b", "b1")]);
    let theirs = lock(&[("g", "a", "a2")]);
    let (merged, contested) = merge(&base, &ours, &theirs);
    assert_eq!(
        source(&merged, "g"),
        hashes(&[("a", UNMATCHED), ("b", UNMATCHED)])
    );
    let paths: Vec<&str> = contested.iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, ["a", "b"], "a changed twice, b changed and dropped");
}

#[test]
fn a_contested_dependent_is_flagged_as_one() {
    let mut base = Lockfile::default();
    let mut snapshot = GroupSnapshot::default();
    snapshot.dependents.insert("d".to_owned(), "d0".to_owned());
    base.groups.insert("g".to_owned(), snapshot);
    let mut ours = base.clone();
    let mut theirs = base.clone();
    for (lock, hash) in [(&mut ours, "d1"), (&mut theirs, "d2")] {
        if let Some(group) = lock.groups.get_mut("g") {
            group.dependents.insert("d".to_owned(), hash.to_owned());
        }
    }
    let (_, contested) = merge(&base, &ours, &theirs);
    assert_eq!(
        contested,
        [Contested {
            group: "g".to_owned(),
            dependent: true,
            path: "d".to_owned(),
        }]
    );
}

#[test]
fn a_hash_one_side_dropped_stays_dropped() {
    let base = lock(&[("g", "a", "a0"), ("g", "b", "b0")]);
    let ours = lock(&[("g", "a", "a0")]);
    let (merged, contested) = merge(&base, &ours, &base);
    assert!(contested.is_empty());
    assert_eq!(source(&merged, "g"), hashes(&[("a", "a0")]));
}

#[test]
fn a_group_one_side_dropped_stays_dropped_though_the_other_changed_it() {
    let base = lock(&[("old", "a", "a0"), ("keep", "k", "k0")]);
    let ours = lock(&[("new", "a", "a0"), ("keep", "k", "k0")]);
    let theirs = lock(&[("old", "a", "a1"), ("keep", "k", "k0")]);
    let (merged, contested) = merge(&base, &ours, &theirs);
    assert!(contested.is_empty());
    let ids: Vec<&String> = merged.groups.keys().collect();
    assert_eq!(ids, ["keep", "new"]);
}

#[test]
fn a_group_both_sides_added_merges_against_nothing() {
    let base = Lockfile::default();
    let ours = lock(&[("g", "a", "a1"), ("g", "b", "b1")]);
    let theirs = lock(&[("g", "a", "a2"), ("g", "c", "c1")]);
    let (merged, contested) = merge(&base, &ours, &theirs);
    assert_eq!(
        source(&merged, "g"),
        hashes(&[("a", UNMATCHED), ("b", "b1"), ("c", "c1")])
    );
    assert_eq!(contested.len(), 1);
}

#[test]
fn the_driver_writes_ours_and_reads_an_empty_side_as_no_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let [base, ours, theirs] = ["base", "ours", "theirs"].map(|name| dir.path().join(name));
    std::fs::write(&base, "").expect("write base");
    lock(&[("g", "a", "a1")]).save(&ours).expect("save ours");
    lock(&[("g", "a", "a2")])
        .save(&theirs)
        .expect("save theirs");
    let notes = merge_files(&base, &ours, &theirs).expect("merges");
    assert!(
        notes.contains("group g, source a") && notes.contains("outdatty update --group g"),
        "{notes}"
    );
    let merged = Lockfile::load(&ours).expect("ours is a lock");
    assert_eq!(source(&merged, "g"), hashes(&[("a", UNMATCHED)]));
}
