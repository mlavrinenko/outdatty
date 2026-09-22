//! Recording chosen dependents without re-recording their whole group.
//!
//! A recorded dependent hash is a review watermark. Re-recording a group claims
//! review of every dependent in it, including ones edited by someone else and
//! still pending. [`dependents`] amends only the named paths and leaves the
//! group's sources and other dependents as the lockfile has them, so a stale
//! group stays stale.

use std::collections::BTreeSet;
use std::path::Path;

use crate::engine::{UpdateAction, UpdateEntry, UpdateReport};
use crate::error::{Error, Result};
use crate::lock::{self, GroupSnapshot, Lockfile};
use crate::manifest::{Group, Manifest};
use crate::{hashing, resolve};

/// Records the current hash of each of `paths` in whichever of the named
/// `groups` declare it, returning the new lock and a report of what changed.
/// A named group declaring none of `paths` is left out of the report.
///
/// A path the group still records but no longer resolves is dropped from its
/// snapshot, confirming the removal.
///
/// # Errors
///
/// Returns [`Error::UnknownDependent`] if no named group declares one of
/// `paths`, [`Error::UnrecordedGroup`] if a group declaring one has no snapshot
/// to amend, or an error if an artifact cannot be resolved or hashed. On any
/// error nothing is recorded.
pub fn dependents(
    manifest: &Manifest,
    lock: &Lockfile,
    base: &Path,
    groups: &[String],
    paths: &[String],
) -> Result<(Lockfile, UpdateReport)> {
    let paths: Vec<String> = paths.iter().map(|path| path.replace('\\', "/")).collect();
    let mut next = lock.clone();
    next.version = lock::VERSION;
    next.algorithm = hashing::ALGORITHM.to_owned();
    let mut entries = Vec::new();
    let mut claimed = BTreeSet::new();
    for group in manifest
        .groups
        .iter()
        .filter(|group| groups.contains(&group.name))
    {
        let locked = lock.groups.get(&group.name);
        let Some((snapshot, recorded)) = amend(group, locked, base, manifest.gitignore, &paths)?
        else {
            continue;
        };
        let action = if Some(&snapshot) == locked {
            UpdateAction::Unchanged
        } else {
            UpdateAction::Updated
        };
        claimed.extend(recorded.iter().cloned());
        next.groups.insert(group.name.clone(), snapshot);
        entries.push(UpdateEntry {
            id: group.name.clone(),
            action,
            recorded,
        });
    }
    if let Some(path) = paths.iter().find(|path| !claimed.contains(*path)) {
        return Err(Error::UnknownDependent {
            groups: groups.to_vec(),
            path: path.clone(),
        });
    }
    Ok((next, UpdateReport { entries }))
}

/// Amends `group`'s snapshot with those of `paths` it declares, returning the
/// snapshot and the paths recorded, or `None` if it declares none of them.
fn amend(
    group: &Group,
    locked: Option<&GroupSnapshot>,
    base: &Path,
    gitignore: bool,
    paths: &[String],
) -> Result<Option<(GroupSnapshot, Vec<String>)>> {
    let resolved = resolve::expand(&group.dependents, base, gitignore)?;
    let recorded: Vec<String> = paths
        .iter()
        .filter(|path| {
            resolved.contains(path)
                || locked.is_some_and(|snap| snap.dependents.contains_key(*path))
        })
        .cloned()
        .collect();
    if recorded.is_empty() {
        return Ok(None);
    }
    let mut snapshot = locked
        .cloned()
        .ok_or_else(|| Error::UnrecordedGroup(group.name.clone()))?;
    for path in &recorded {
        if resolved.contains(path) {
            let hash = hashing::hash_file(&base.join(path))?;
            snapshot.dependents.insert(path.clone(), hash);
        } else {
            snapshot.dependents.remove(path);
        }
    }
    Ok(Some((snapshot, recorded)))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::dependents;
    use crate::engine::{Filter, UpdateAction, build};
    use crate::error::Error;
    use crate::lock::{GroupSnapshot, Lockfile};
    use crate::manifest::{Group, Manifest};

    fn write(base: &Path, name: &str, body: &str) {
        std::fs::write(base.join(name), body).expect("write");
    }

    fn names(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| (*item).to_owned()).collect()
    }

    /// Groups `g` (`code.rs` -> `a.md`, `b.md`) and `h` (`code.rs` -> `b.md`),
    /// recorded and then drifted: both dependents and the source edited since.
    fn drifted(base: &Path) -> (Manifest, Lockfile) {
        write(base, "code.rs", "src");
        write(base, "a.md", "a");
        write(base, "b.md", "b");
        let manifest = Manifest {
            groups: vec![
                Group {
                    name: "g".to_owned(),
                    source: names(&["code.rs"]),
                    dependents: names(&["a.md", "b.md"]),
                    bidirectional: false,
                },
                Group {
                    name: "h".to_owned(),
                    source: names(&["code.rs"]),
                    dependents: names(&["b.md"]),
                    bidirectional: false,
                },
            ],
            ..Manifest::default()
        };
        let (lock, _) = build(&manifest, &Lockfile::default(), base, &Filter::All).expect("build");
        write(base, "code.rs", "src edited");
        write(base, "a.md", "a reviewed");
        write(base, "b.md", "b pending");
        (manifest, lock)
    }

    #[test]
    fn records_the_named_dependent_and_nothing_else() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (manifest, lock) = drifted(dir.path());
        let (next, report) = dependents(
            &manifest,
            &lock,
            dir.path(),
            &names(&["g"]),
            &names(&["a.md"]),
        )
        .expect("record");
        let was = lock.groups.get("g").expect("locked");
        let now = next.groups.get("g").expect("recorded");
        let hash = |snapshot: &GroupSnapshot, path: &str| snapshot.dependents.get(path).cloned();
        assert_ne!(hash(was, "a.md"), hash(now, "a.md"), "a moved");
        assert_eq!(hash(was, "b.md"), hash(now, "b.md"), "b did not");
        assert_eq!(was.source, now.source, "sources stay as recorded");
        assert_eq!(
            report.entries.first().expect("entry").action,
            UpdateAction::Updated
        );
    }

    #[test]
    fn a_current_dependent_is_unchanged() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (manifest, lock) = drifted(dir.path());
        write(dir.path(), "a.md", "a");
        let (_, report) = dependents(
            &manifest,
            &lock,
            dir.path(),
            &names(&["g"]),
            &names(&["a.md"]),
        )
        .expect("record");
        assert_eq!(
            report.entries.first().expect("entry").action,
            UpdateAction::Unchanged
        );
    }

    #[test]
    fn a_deleted_dependent_is_dropped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (manifest, lock) = drifted(dir.path());
        std::fs::remove_file(dir.path().join("a.md")).expect("remove");
        let (next, _) = dependents(
            &manifest,
            &lock,
            dir.path(),
            &names(&["g"]),
            &names(&["a.md"]),
        )
        .expect("record");
        let now = next.groups.get("g").expect("recorded");
        assert!(!now.dependents.contains_key("a.md"));
    }

    #[test]
    fn refuses_an_undeclared_path_by_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (manifest, lock) = drifted(dir.path());
        let err = dependents(
            &manifest,
            &lock,
            dir.path(),
            &names(&["g"]),
            &names(&["a.md", "code.rs"]),
        )
        .expect_err("a source is not a dependent");
        assert!(
            matches!(err, Error::UnknownDependent { ref groups, ref path } if groups == &names(&["g"]) && path == "code.rs")
        );
    }

    #[test]
    fn records_each_path_in_the_named_groups_that_declare_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (manifest, lock) = drifted(dir.path());
        let (_, report) = dependents(
            &manifest,
            &lock,
            dir.path(),
            &names(&["g", "h"]),
            &names(&["a.md"]),
        )
        .expect("record");
        let ids: Vec<&str> = report
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .collect();
        assert_eq!(ids, vec!["g"], "h declares no a.md and is left alone");

        let (next, report) = dependents(
            &manifest,
            &lock,
            dir.path(),
            &names(&["g", "h"]),
            &names(&["a.md", "b.md"]),
        )
        .expect("record");
        let recorded: Vec<&[String]> = report
            .entries
            .iter()
            .map(|entry| entry.recorded.as_slice())
            .collect();
        assert_eq!(recorded, vec![names(&["a.md", "b.md"]), names(&["b.md"])]);
        let now = next.groups.get("h").expect("recorded");
        assert_eq!(now.source, lock.groups.get("h").expect("locked").source);
    }

    #[test]
    fn refuses_a_group_never_recorded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (manifest, _) = drifted(dir.path());
        let err = dependents(
            &manifest,
            &Lockfile::default(),
            dir.path(),
            &names(&["g"]),
            &names(&["a.md"]),
        )
        .expect_err("nothing to amend");
        assert!(matches!(err, Error::UnrecordedGroup(ref id) if id == "g"));
    }
}
