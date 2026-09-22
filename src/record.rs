//! Recording chosen dependents without re-recording their whole group.
//!
//! A recorded dependent hash is a review watermark. Re-recording a group claims
//! review of every dependent in it, including ones edited by someone else and
//! still pending. [`dependents`] amends only the named paths and leaves the
//! group's sources and other dependents as the lockfile has them, so a stale
//! group stays stale.

use std::path::Path;

use crate::engine::{UpdateAction, UpdateEntry, UpdateReport};
use crate::error::{Error, Result};
use crate::lock::{self, Lockfile};
use crate::manifest::Manifest;
use crate::{hashing, resolve};

/// Records the current hash of each of `paths` in every group named in
/// `groups`, returning the new lock and a report of what changed.
///
/// A path the group still records but no longer resolves is dropped from its
/// snapshot, confirming the removal.
///
/// # Errors
///
/// Returns [`Error::UnrecordedGroup`] if a named group has no snapshot to amend,
/// [`Error::UnknownDependent`] if a named group neither resolves nor records one
/// of `paths`, or an error if an artifact cannot be resolved or hashed. On any
/// error nothing is recorded.
pub fn dependents(
    manifest: &Manifest,
    lock: &Lockfile,
    base: &Path,
    groups: &[String],
    paths: &[String],
) -> Result<(Lockfile, UpdateReport)> {
    let mut next = lock.clone();
    next.version = lock::VERSION;
    next.algorithm = hashing::ALGORITHM.to_owned();
    let mut entries = Vec::new();
    for group in &manifest.groups {
        let id = &group.name;
        if !groups.contains(id) {
            continue;
        }
        let locked = lock
            .groups
            .get(id)
            .ok_or_else(|| Error::UnrecordedGroup(id.clone()))?;
        let declared = resolve::expand(&group.dependents, base, manifest.gitignore)?;
        let mut snapshot = locked.clone();
        for path in paths {
            let path = path.replace('\\', "/");
            if declared.contains(&path) {
                let hash = hashing::hash_file(&base.join(&path))?;
                snapshot.dependents.insert(path, hash);
            } else if snapshot.dependents.remove(&path).is_none() {
                return Err(Error::UnknownDependent {
                    group: id.clone(),
                    path,
                });
            }
        }
        let action = if &snapshot == locked {
            UpdateAction::Unchanged
        } else {
            UpdateAction::Updated
        };
        next.groups.insert(id.clone(), snapshot);
        entries.push(UpdateEntry {
            id: id.clone(),
            action,
        });
    }
    Ok((next, UpdateReport { entries }))
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

    /// A group `g` of `code.rs` -> `a.md`, `b.md`, recorded and then drifted:
    /// both dependents and the source edited since.
    fn drifted(base: &Path) -> (Manifest, Lockfile) {
        write(base, "code.rs", "src");
        write(base, "a.md", "a");
        write(base, "b.md", "b");
        let manifest = Manifest {
            groups: vec![Group {
                name: "g".to_owned(),
                source: names(&["code.rs"]),
                dependents: names(&["a.md", "b.md"]),
                bidirectional: false,
            }],
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
            matches!(err, Error::UnknownDependent { ref group, ref path } if group == "g" && path == "code.rs")
        );
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
