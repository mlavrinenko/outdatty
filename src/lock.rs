//! The lockfile recording confirmed artifact hashes (`outdatty.lock`).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::hashing;

/// Current lockfile format version.
pub const VERSION: u32 = 1;

/// Default lockfile name placed next to the manifest.
pub const DEFAULT_NAME: &str = "outdatty.lock";

/// A snapshot of confirmed hashes for every declared group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile format version.
    pub version: u32,
    /// Hash algorithm used for every entry.
    pub algorithm: String,
    /// Per-group snapshots keyed by group identifier.
    #[serde(default)]
    pub groups: BTreeMap<String, GroupSnapshot>,
}

/// Confirmed hashes for a single group, keyed by artifact path.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSnapshot {
    /// Source artifact paths mapped to their confirmed hash.
    #[serde(default)]
    pub source: BTreeMap<String, String>,
    /// Dependent artifact paths mapped to their confirmed hash.
    #[serde(default)]
    pub dependents: BTreeMap<String, String>,
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: VERSION,
            algorithm: hashing::ALGORITHM.to_owned(),
            groups: BTreeMap::new(),
        }
    }
}

impl Lockfile {
    /// Loads a lockfile from `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LockNotFound`] if `path` is missing, or
    /// [`Error::LockParse`] if it cannot be parsed.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(Error::LockNotFound(path.to_path_buf()));
        }
        let text = fs::read_to_string(path)?;
        let lock = serde_yaml_ng::from_str(&text).map_err(|source| Error::LockParse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;
        Ok(lock)
    }

    /// Loads a lockfile from `path`, returning an empty lockfile if absent.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LockParse`] if an existing file cannot be parsed.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::default())
        }
    }

    /// Writes the lockfile to `path` as YAML with a trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LockSerialize`] if serialization fails or
    /// [`Error::Io`] if the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<()> {
        let mut text =
            serde_yaml_ng::to_string(self).map_err(|err| Error::LockSerialize(Box::new(err)))?;
        if !text.ends_with('\n') {
            text.push('\n');
        }
        fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{GroupSnapshot, Lockfile, VERSION};

    #[test]
    fn default_carries_version_and_algorithm() {
        let lock = Lockfile::default();
        assert_eq!(lock.version, VERSION);
        assert_eq!(lock.algorithm, "blake3");
        assert!(lock.groups.is_empty());
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("outdatty.lock");

        let mut lock = Lockfile::default();
        let mut snapshot = GroupSnapshot::default();
        snapshot
            .source
            .insert("a.rs".to_owned(), "hash-a".to_owned());
        snapshot
            .dependents
            .insert("a.md".to_owned(), "hash-b".to_owned());
        lock.groups.insert("g".to_owned(), snapshot);
        lock.save(&path).expect("save");

        let loaded = Lockfile::load(&path).expect("load");
        assert_eq!(loaded.groups, lock.groups);
    }

    #[test]
    fn load_or_default_handles_absent_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let lock = Lockfile::load_or_default(&dir.path().join("nope.lock")).expect("default");
        assert!(lock.groups.is_empty());
    }

    #[test]
    fn missing_file_is_an_error_for_load() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(Lockfile::load(&dir.path().join("nope.lock")).is_err());
    }
}
