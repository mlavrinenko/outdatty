//! Content hashing of artifacts using `blake3`.

use std::fs::File;
use std::path::Path;

use crate::error::Result;

/// Name of the hash algorithm recorded in lockfiles.
pub const ALGORITHM: &str = "blake3";

/// Computes the `blake3` hash of a file, returned as a lowercase hex string.
///
/// The file is streamed, so arbitrarily large and binary artifacts (for
/// example `docx`) are handled without loading them fully into memory.
///
/// # Errors
///
/// Returns [`crate::error::Error::Io`] if the file cannot be opened or read.
pub fn hash_file(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(file)?;
    Ok(hasher.finalize().to_hex().as_str().to_owned())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::hash_file;

    #[test]
    fn hashes_are_stable_and_content_dependent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("a.txt");
        let mut file = std::fs::File::create(&path).expect("create");
        file.write_all(b"hello").expect("write");
        drop(file);

        let first = hash_file(&path).expect("hash");
        let second = hash_file(&path).expect("hash");
        assert_eq!(first, second, "hashing is deterministic");
        assert_eq!(first.len(), 64, "blake3 hex is 64 chars");

        std::fs::write(&path, b"world").expect("rewrite");
        let third = hash_file(&path).expect("hash");
        assert_ne!(first, third, "different content yields different hash");
    }

    #[test]
    fn missing_file_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = hash_file(&dir.path().join("nope"));
        assert!(result.is_err(), "missing file is an error");
    }
}
