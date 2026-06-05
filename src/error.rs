//! Error types for the `outdatty` library.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced while loading manifests, lockfiles, resolving patterns, or
/// hashing artifacts.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O operation failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// The manifest could not be located.
    #[error("manifest not found: {0} (run `outdatty init` to create one)")]
    ManifestNotFound(PathBuf),

    /// A manifest already exists and `--force` was not given.
    #[error("manifest already exists: {0} (use --force to overwrite)")]
    ManifestExists(PathBuf),

    /// The manifest failed to parse.
    #[error("failed to parse manifest {path}: {source}")]
    ManifestParse {
        /// Path of the offending manifest.
        path: PathBuf,
        /// Underlying parser error.
        source: Box<serde_yaml_ng::Error>,
    },

    /// Two groups share the same identifier.
    #[error("duplicate group name: {0} (group names must be unique)")]
    DuplicateGroup(String),

    /// A group declares no source artifacts.
    #[error("group {0} has an empty `source`; a group must declare at least one source")]
    EmptyGroupSource(String),

    /// The lockfile could not be located.
    #[error("lockfile not found: {0} (run `outdatty update` to create it)")]
    LockNotFound(PathBuf),

    /// The lockfile was written by an incompatible (newer) version.
    #[error(
        "lockfile {path} has unsupported version {found} (this build supports up to {supported}); upgrade outdatty"
    )]
    LockVersion {
        /// Path of the offending lockfile.
        path: PathBuf,
        /// Version recorded in the lockfile.
        found: u32,
        /// Highest version this build understands.
        supported: u32,
    },

    /// The lockfile records a hash algorithm this build cannot reproduce.
    #[error(
        "lockfile {path} uses hash algorithm `{found}`, but this build computes `{expected}`; run `outdatty update`"
    )]
    LockAlgorithm {
        /// Path of the offending lockfile.
        path: PathBuf,
        /// Algorithm recorded in the lockfile.
        found: String,
        /// Algorithm this build computes.
        expected: String,
    },

    /// The lockfile failed to parse.
    #[error("failed to parse lockfile {path}: {source}")]
    LockParse {
        /// Path of the offending lockfile.
        path: PathBuf,
        /// Underlying parser error.
        source: Box<serde_yaml_ng::Error>,
    },

    /// The lockfile could not be serialized.
    #[error("failed to serialize lockfile: {0}")]
    LockSerialize(Box<serde_yaml_ng::Error>),

    /// A glob pattern was invalid.
    #[error("invalid pattern `{pattern}`: {source}")]
    Pattern {
        /// The offending pattern.
        pattern: String,
        /// Underlying glob error.
        source: globset::Error,
    },

    /// A requested group does not exist in the manifest.
    #[error("no such group: {0}")]
    UnknownGroup(String),

    /// Rendering a report to JSON failed.
    #[error("failed to render json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias for fallible operations in this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn messages_are_actionable() {
        let unknown = Error::UnknownGroup("ghost".to_owned());
        assert_eq!(unknown.to_string(), "no such group: ghost");
        let dup = Error::DuplicateGroup("pair".to_owned());
        assert!(dup.to_string().contains("duplicate group name"));
        let empty = Error::EmptyGroupSource("pair".to_owned());
        assert!(empty.to_string().contains("empty `source`"));
    }
}
