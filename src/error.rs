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

    /// The lockfile could not be located.
    #[error("lockfile not found: {0} (run `outdatty update` to create it)")]
    LockNotFound(PathBuf),

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

    /// A directly referenced file is missing from disk.
    #[error("referenced file is missing: {0}")]
    MissingFile(String),

    /// A glob pattern was invalid.
    #[error("invalid pattern `{pattern}`: {source}")]
    Pattern {
        /// The offending pattern.
        pattern: String,
        /// Underlying glob error.
        source: glob::PatternError,
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
        let missing = Error::MissingFile("code.rs".to_owned());
        assert!(missing.to_string().contains("referenced file is missing"));
    }
}
