//! Evaluation of manifest groups against the lockfile.

use std::collections::BTreeMap;
use std::path::Path;

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::Serialize;

use crate::error::Result;
use crate::lock::{GroupSnapshot, Lockfile};
use crate::manifest::{Group, Manifest};
use crate::{hashing, lock, resolve};

/// Selection of groups to operate on.
#[derive(Debug, Clone)]
pub enum Filter {
    /// Every group in the manifest.
    All,
    /// Only the groups whose identifier appears in this list.
    Only(Vec<String>),
}

impl Filter {
    fn selects(&self, id: &str) -> bool {
        match self {
            Filter::All => true,
            Filter::Only(ids) => ids.iter().any(|wanted| wanted == id),
        }
    }
}

/// Synchronisation status of a single group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Every source matches the confirmed snapshot. A directed group is also
    /// `Ok` when only its dependents changed; that is allowed, and `check`
    /// passes. The differing dependents are still reported in `changed_dependents`
    /// for machine consumers.
    Ok,
    /// A source changed; dependents must be re-confirmed.
    Stale,
    /// The group has no confirmed snapshot yet.
    New,
}

impl Status {
    /// Returns true if this status should fail a `check`.
    #[must_use]
    pub fn is_failure(self) -> bool {
        matches!(self, Status::Stale | Status::New)
    }
}

/// Result of evaluating one group.
#[derive(Debug, Clone, Serialize)]
pub struct GroupReport {
    /// Group identifier.
    pub id: String,
    /// Synchronisation status.
    pub status: Status,
    /// Source paths whose content differs from the snapshot.
    pub changed_sources: Vec<String>,
    /// Dependent paths whose content differs from the snapshot.
    pub changed_dependents: Vec<String>,
    /// The group's full declared dependents; review targets for a human when
    /// the group fails.
    pub dependents: Vec<String>,
}

/// Aggregate evaluation result.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Per-group results in manifest order.
    pub groups: Vec<GroupReport>,
    /// Files `require_tracked` demands but no group covers. Populated only for a
    /// whole-manifest check; a non-empty list fails a `check`.
    pub untracked: Vec<String>,
}

impl Report {
    /// Returns true if any group is failing or any required file is untracked.
    #[must_use]
    pub fn has_failure(&self) -> bool {
        self.groups.iter().any(|group| group.status.is_failure()) || !self.untracked.is_empty()
    }
}

/// Returns the identifiers of every group in `manifest`, in order.
#[must_use]
pub fn ids(manifest: &Manifest) -> Vec<String> {
    manifest
        .groups
        .iter()
        .map(|group| group.name.clone())
        .collect()
}

fn hash_patterns(
    patterns: &[String],
    base: &Path,
    gitignore: bool,
) -> Result<BTreeMap<String, String>> {
    let paths = resolve::expand(patterns, base, gitignore)?;
    // Hash files in parallel; the BTreeMap collect restores deterministic order.
    paths
        .into_par_iter()
        .map(|path| {
            let hash = hashing::hash_file(&base.join(&path))?;
            Ok((path, hash))
        })
        .collect()
}

fn snapshot_group(group: &Group, base: &Path, gitignore: bool) -> Result<GroupSnapshot> {
    Ok(GroupSnapshot {
        source: hash_patterns(&group.source, base, gitignore)?,
        dependents: hash_patterns(&group.dependents, base, gitignore)?,
    })
}

/// Returns the sorted keys whose values differ between `current` and `locked`,
/// including keys present in only one of the maps.
fn diff(current: &BTreeMap<String, String>, locked: &BTreeMap<String, String>) -> Vec<String> {
    let mut changed = Vec::new();
    for (key, value) in current {
        if locked.get(key) != Some(value) {
            changed.push(key.clone());
        }
    }
    for key in locked.keys() {
        if !current.contains_key(key) {
            changed.push(key.clone());
        }
    }
    changed.sort();
    changed.dedup();
    changed
}

fn classify(bidirectional: bool, source_changed: bool, dependent_changed: bool) -> Status {
    if source_changed || (bidirectional && dependent_changed) {
        Status::Stale
    } else {
        Status::Ok
    }
}

fn evaluate_group(
    group: &Group,
    id: String,
    base: &Path,
    locked: Option<&GroupSnapshot>,
    gitignore: bool,
) -> Result<GroupReport> {
    let current = snapshot_group(group, base, gitignore)?;
    let Some(locked) = locked else {
        return Ok(GroupReport {
            id,
            status: Status::New,
            changed_sources: Vec::new(),
            changed_dependents: Vec::new(),
            dependents: group.dependents.clone(),
        });
    };
    let changed_sources = diff(&current.source, &locked.source);
    let changed_dependents = diff(&current.dependents, &locked.dependents);
    let status = classify(
        group.bidirectional,
        !changed_sources.is_empty(),
        !changed_dependents.is_empty(),
    );
    Ok(GroupReport {
        id,
        status,
        changed_sources,
        changed_dependents,
        dependents: group.dependents.clone(),
    })
}

/// Evaluates the selected manifest groups against `lock`.
///
/// # Errors
///
/// Returns an error if any artifact cannot be resolved or hashed.
pub fn evaluate(
    manifest: &Manifest,
    lock: &Lockfile,
    base: &Path,
    filter: &Filter,
) -> Result<Report> {
    let mut groups = Vec::new();
    for group in &manifest.groups {
        let id = group.name.clone();
        if !filter.selects(&id) {
            continue;
        }
        let locked = lock.groups.get(&id);
        groups.push(evaluate_group(group, id, base, locked, manifest.gitignore)?);
    }
    // Coverage is a whole-manifest concern computed by the caller (it needs the
    // manifest and lockfile paths to exempt); an unfiltered evaluation reports no
    // untracked files here.
    Ok(Report {
        groups,
        untracked: Vec::new(),
    })
}

/// Action taken on a group during an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateAction {
    /// A new snapshot was recorded.
    Added,
    /// An existing snapshot changed.
    Updated,
    /// The snapshot was already current.
    Unchanged,
    /// A lockfile entry with no matching manifest group was pruned.
    Removed,
}

/// Result of updating one group.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateEntry {
    /// Group identifier.
    pub id: String,
    /// What happened to the group's snapshot.
    pub action: UpdateAction,
}

/// Outcome of an update operation.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateReport {
    /// Per-group actions, in manifest order.
    pub entries: Vec<UpdateEntry>,
}

fn action_for(previous: Option<&GroupSnapshot>, next: &GroupSnapshot) -> UpdateAction {
    match previous {
        None => UpdateAction::Added,
        Some(prev) if prev == next => UpdateAction::Unchanged,
        Some(_) => UpdateAction::Updated,
    }
}

/// Rebuilds `lock` for the selected groups, returning the new lock and a report
/// of what changed.
///
/// When `filter` is [`Filter::All`], group entries absent from the manifest are
/// pruned from the lockfile.
///
/// # Errors
///
/// Returns an error if any artifact cannot be resolved or hashed.
pub fn build(
    manifest: &Manifest,
    lock: &Lockfile,
    base: &Path,
    filter: &Filter,
) -> Result<(Lockfile, UpdateReport)> {
    let mut next = lock.clone();
    next.version = lock::VERSION;
    next.algorithm = hashing::ALGORITHM.to_owned();
    let mut entries = Vec::new();
    let mut keep = std::collections::BTreeSet::new();
    for group in &manifest.groups {
        let id = group.name.clone();
        keep.insert(id.clone());
        if !filter.selects(&id) {
            continue;
        }
        let snapshot = snapshot_group(group, base, manifest.gitignore)?;
        let action = action_for(lock.groups.get(&id), &snapshot);
        next.groups.insert(id.clone(), snapshot);
        entries.push(UpdateEntry { id, action });
    }
    if matches!(filter, Filter::All) {
        for id in next.groups.keys() {
            if !keep.contains(id) {
                entries.push(UpdateEntry {
                    id: id.clone(),
                    action: UpdateAction::Removed,
                });
            }
        }
        next.groups.retain(|id, _| keep.contains(id));
    }
    Ok((next, UpdateReport { entries }))
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
mod tests {
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
}
