//! Expansion of manifest patterns into concrete artifact paths.

use std::path::Path;

use crate::error::{Error, Result};

/// Returns true if `pattern` contains glob metacharacters.
#[must_use]
pub fn is_glob(pattern: &str) -> bool {
    pattern.chars().any(|ch| matches!(ch, '*' | '?' | '['))
}

/// Expands `patterns` (interpreted relative to `base`) into a sorted,
/// de-duplicated list of artifact paths relative to `base`.
///
/// Literal paths (no glob metacharacters) must exist on disk; glob patterns may
/// match zero files. Paths are normalized to use forward slashes.
///
/// # Errors
///
/// Returns [`Error::MissingFile`] if a literal path does not exist, or
/// [`Error::Pattern`] if a glob pattern is invalid.
pub fn expand(patterns: &[String], base: &Path) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for pattern in patterns {
        if is_glob(pattern) {
            expand_glob(pattern, base, &mut out)?;
        } else {
            expand_literal(pattern, base, &mut out)?;
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn expand_literal(pattern: &str, base: &Path, out: &mut Vec<String>) -> Result<()> {
    if base.join(pattern).is_file() {
        out.push(normalize(pattern));
        Ok(())
    } else {
        Err(Error::MissingFile(pattern.to_owned()))
    }
}

fn expand_glob(pattern: &str, base: &Path, out: &mut Vec<String>) -> Result<()> {
    let joined = base.join(pattern);
    let full = joined.to_string_lossy();
    let entries = glob::glob(full.as_ref()).map_err(|source| Error::Pattern {
        pattern: pattern.to_owned(),
        source,
    })?;
    let mut matched = false;
    for entry in entries.flatten() {
        if entry.is_file() {
            matched = true;
            let rel = entry.strip_prefix(base).unwrap_or(&entry);
            out.push(normalize(rel.to_string_lossy().as_ref()));
        }
    }
    if !matched {
        log::warn!("pattern `{pattern}` matched no files");
    }
    Ok(())
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{expand, is_glob};

    fn touch(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"x").expect("write");
    }

    #[test]
    fn detects_glob_patterns() {
        assert!(is_glob("src/*.rs"));
        assert!(is_glob("a?.txt"));
        assert!(!is_glob("plain/path.rs"));
    }

    #[test]
    fn literal_must_exist() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "present.txt");
        let ok = expand(&["present.txt".to_owned()], dir.path()).expect("resolve");
        assert_eq!(ok, vec!["present.txt".to_owned()]);

        let err = expand(&["absent.txt".to_owned()], dir.path());
        assert!(err.is_err(), "missing literal is an error");
    }

    #[test]
    fn globs_expand_sorted_and_unique() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "b.txt");
        touch(dir.path(), "a.txt");
        let resolved =
            expand(&["*.txt".to_owned(), "a.txt".to_owned()], dir.path()).expect("resolve");
        assert_eq!(resolved, vec!["a.txt".to_owned(), "b.txt".to_owned()]);
    }

    #[test]
    fn empty_glob_is_allowed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let resolved = expand(&["*.none".to_owned()], dir.path()).expect("resolve");
        assert!(resolved.is_empty(), "zero matches is not an error");
    }
}
