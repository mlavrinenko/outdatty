//! Three-way merge of lockfiles, run by git as a merge driver.
//!
//! Register it once per clone and assign it to the lockfile:
//!
//! ```text
//! git config merge.outdatty.driver 'outdatty merge-lock %O %A %B'
//! echo 'outdatty.lock merge=outdatty' >> .gitattributes
//! ```
//!
//! Two branches that each record a different group change hash lines that can
//! sit next to each other, so git's text merge conflicts where neither branch's
//! review touched the other's. The lockfile is a map of maps, so it merges key
//! by key instead: every hash (group, `source` or `dependents`, path) takes the
//! side that changed it.
//!
//! A hash both sides changed to different values gets [`UNMATCHED`], since no
//! side's review covers the file the merge produces. For a source that makes
//! `check` fail the group until someone reviews it and runs `update`. The merge
//! itself never stops on the lockfile.
//!
//! A group one side dropped stays dropped, whatever the other side recorded in
//! it: only a plain `update` drops a group, and only once the manifest no longer
//! declares it, so a rename or a split on one branch leaves no orphan behind.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use crate::error::Result;
use crate::lock::{GroupSnapshot, Lockfile};

/// The hash a key both sides changed gets: blake3's shape, and no file's hash.
pub const UNMATCHED: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A hash both sides changed, which the merge left as [`UNMATCHED`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contested {
    /// The group holding the hash.
    pub group: String,
    /// True for a dependent's hash, false for a source's.
    pub dependent: bool,
    /// The artifact path the hash belongs to.
    pub path: String,
}

/// Merges `ours` and `theirs`, both descended from `base`, returning the merged
/// lockfile and the hashes it left as [`UNMATCHED`].
#[must_use]
pub fn merge(base: &Lockfile, ours: &Lockfile, theirs: &Lockfile) -> (Lockfile, Vec<Contested>) {
    let mut merged = Lockfile::default();
    let mut contested = Vec::new();
    let ids: BTreeSet<&String> = base
        .groups
        .keys()
        .chain(ours.groups.keys())
        .chain(theirs.groups.keys())
        .collect();
    let empty = GroupSnapshot::default();
    for id in ids {
        let was = base.groups.get(id);
        let (left, right) = (ours.groups.get(id), theirs.groups.get(id));
        if was.is_some() && (left.is_none() || right.is_none()) {
            continue;
        }
        let [was, left, right] = [was, left, right].map(|group| group.unwrap_or(&empty));
        let mut contest = |dependent: bool, path: &str| {
            contested.push(Contested {
                group: id.clone(),
                dependent,
                path: path.to_owned(),
            });
        };
        let source = merge_hashes(&was.source, &left.source, &right.source, &mut |path| {
            contest(false, path);
        });
        let dependents = merge_hashes(
            &was.dependents,
            &left.dependents,
            &right.dependents,
            &mut |path| contest(true, path),
        );
        merged
            .groups
            .insert(id.clone(), GroupSnapshot { source, dependents });
    }
    (merged, contested)
}

/// Merges one map of path to hash, calling `on_contested` for each path both
/// sides changed to different values, which gets [`UNMATCHED`].
fn merge_hashes(
    base: &BTreeMap<String, String>,
    ours: &BTreeMap<String, String>,
    theirs: &BTreeMap<String, String>,
    on_contested: &mut dyn FnMut(&str),
) -> BTreeMap<String, String> {
    let paths: BTreeSet<&String> = base
        .keys()
        .chain(ours.keys())
        .chain(theirs.keys())
        .collect();
    let mut merged = BTreeMap::new();
    for path in paths {
        let (was, left, right) = (base.get(path), ours.get(path), theirs.get(path));
        let value = if left == right || was == right {
            left.cloned()
        } else if was == left {
            right.cloned()
        } else {
            on_contested(path);
            Some(UNMATCHED.to_owned())
        };
        if let Some(value) = value {
            merged.insert(path.clone(), value);
        }
    }
    merged
}

/// Runs the merge driver: merges the lockfiles at `base`, `ours` and `theirs`
/// into `ours` and returns one line per hash left as [`UNMATCHED`].
///
/// # Errors
///
/// Returns an error if a side cannot be read or parsed as a lockfile, or the
/// result cannot be written. `ours` is then left as it was.
pub fn merge_files(base: &Path, ours: &Path, theirs: &Path) -> Result<String> {
    let (merged, contested) = merge(
        &Lockfile::load_merge_side(base)?,
        &Lockfile::load_merge_side(ours)?,
        &Lockfile::load_merge_side(theirs)?,
    );
    merged.save(ours)?;
    Ok(render(&contested))
}

/// One line per contested hash, saying what records it once it is reviewed.
fn render(contested: &[Contested]) -> String {
    let mut out = String::new();
    for entry in contested {
        let (group, path) = (&entry.group, &entry.path);
        let _ = if entry.dependent {
            writeln!(
                out,
                "outdatty merge-lock: group {group}, dependent {path}: both sides changed its hash; \
                 once reviewed, record it: outdatty update --group {group} --dependent {path}"
            )
        } else {
            writeln!(
                out,
                "outdatty merge-lock: group {group}, source {path}: both sides changed its hash, \
                 so `outdatty check` fails the group until it is reviewed: outdatty update --group {group}"
            )
        };
    }
    out
}

#[cfg(test)]
#[path = "merge_tests.rs"]
mod tests;
