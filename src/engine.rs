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
    /// A directed group whose dependents glob expands to a file the lockfile
    /// holds no hash for, so nothing watches it until it is recorded.
    Unrecorded,
}

impl Status {
    /// Returns true if this status should fail a `check`.
    #[must_use]
    pub fn is_failure(self) -> bool {
        matches!(self, Status::Stale | Status::New | Status::Unrecorded)
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
    /// Dependent paths the group declares today that the lockfile has no hash
    /// for; each needs `update --dependent` to be recorded.
    pub unrecorded_dependents: Vec<String>,
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

/// Returns the sorted keys of `current` that `locked` holds no hash for.
fn unrecorded(
    current: &BTreeMap<String, String>,
    locked: &BTreeMap<String, String>,
) -> Vec<String> {
    current
        .keys()
        .filter(|key| !locked.contains_key(*key))
        .cloned()
        .collect()
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
            unrecorded_dependents: Vec::new(),
            dependents: group.dependents.clone(),
        });
    };
    let changed_sources = diff(&current.source, &locked.source);
    let changed_dependents = diff(&current.dependents, &locked.dependents);
    let unrecorded_dependents = unrecorded(&current.dependents, &locked.dependents);
    let mut status = classify(
        group.bidirectional,
        !changed_sources.is_empty(),
        !changed_dependents.is_empty(),
    );
    if status == Status::Ok && !unrecorded_dependents.is_empty() {
        status = Status::Unrecorded;
    }
    Ok(GroupReport {
        id,
        status,
        changed_sources,
        changed_dependents,
        unrecorded_dependents,
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
    /// Dependents recorded by a `--dependent` update; empty when the whole
    /// group was recorded.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recorded: Vec<String>,
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
        entries.push(UpdateEntry {
            id,
            action,
            recorded: Vec::new(),
        });
    }
    if matches!(filter, Filter::All) {
        for id in next.groups.keys() {
            if !keep.contains(id) {
                entries.push(UpdateEntry {
                    id: id.clone(),
                    action: UpdateAction::Removed,
                    recorded: Vec::new(),
                });
            }
        }
        next.groups.retain(|id, _| keep.contains(id));
    }
    Ok((next, UpdateReport { entries }))
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
