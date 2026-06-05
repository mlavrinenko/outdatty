//! Expansion of manifest patterns into concrete artifact paths.

use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

use crate::error::{Error, Result};

/// Returns true if `pattern` contains glob metacharacters.
#[must_use]
pub fn is_glob(pattern: &str) -> bool {
    pattern.chars().any(|ch| matches!(ch, '*' | '?' | '['))
}

/// Expands `patterns` (interpreted relative to `base`) into a sorted,
/// de-duplicated list of artifact paths relative to `base`.
///
/// Glob patterns may match zero files. A literal path that is absent on disk is
/// skipped with a warning (so a deleted source surfaces as drift rather than a
/// hard error). When `gitignore` is true, glob matches ignored by the root
/// `.gitignore` are dropped; explicitly listed literals are always kept. Paths
/// are normalized to use forward slashes.
///
/// # Errors
///
/// Returns [`Error::Pattern`] if a glob pattern is invalid.
pub fn expand(patterns: &[String], base: &Path, gitignore: bool) -> Result<Vec<String>> {
    let ignorer = gitignore.then(|| build_gitignore(base));
    let mut out: Vec<String> = Vec::new();
    for pattern in patterns {
        if is_glob(pattern) {
            expand_glob(pattern, base, ignorer.as_ref(), &mut out)?;
        } else {
            expand_literal(pattern, base, &mut out);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Builds a matcher from the root `.gitignore`; an empty matcher (ignores
/// nothing) when the file is absent or unreadable.
fn build_gitignore(base: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(base);
    builder.add(base.join(".gitignore"));
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

fn expand_literal(pattern: &str, base: &Path, out: &mut Vec<String>) {
    if base.join(pattern).is_file() {
        out.push(normalize(pattern));
    } else {
        log::warn!("literal path `{pattern}` is missing; treating as removed");
    }
}

fn expand_glob(
    pattern: &str,
    base: &Path,
    ignorer: Option<&Gitignore>,
    out: &mut Vec<String>,
) -> Result<()> {
    let joined = base.join(pattern);
    let full = joined.to_string_lossy();
    let entries = glob::glob(full.as_ref()).map_err(|source| Error::Pattern {
        pattern: pattern.to_owned(),
        source,
    })?;
    let mut matched = false;
    for entry in entries.flatten() {
        if !entry.is_file() {
            continue;
        }
        let rel = entry.strip_prefix(base).unwrap_or(&entry);
        if let Some(ignorer) = ignorer {
            if ignorer.matched_path_or_any_parents(rel, false).is_ignore() {
                continue;
            }
        }
        matched = true;
        out.push(normalize(rel.to_string_lossy().as_ref()));
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
    fn present_literal_resolves_missing_is_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "present.txt");
        let ok = expand(&["present.txt".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(ok, vec!["present.txt".to_owned()]);

        let gone = expand(&["absent.txt".to_owned()], dir.path(), true).expect("resolve");
        assert!(gone.is_empty(), "missing literal is skipped, not an error");
    }

    #[test]
    fn globs_expand_sorted_and_unique() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "b.txt");
        touch(dir.path(), "a.txt");
        let resolved =
            expand(&["*.txt".to_owned(), "a.txt".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(resolved, vec!["a.txt".to_owned(), "b.txt".to_owned()]);
    }

    #[test]
    fn empty_glob_is_allowed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let resolved = expand(&["*.none".to_owned()], dir.path(), true).expect("resolve");
        assert!(resolved.is_empty(), "zero matches is not an error");
    }

    #[test]
    fn gitignore_filters_globs_but_not_literals() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".gitignore"), "/target\n").expect("gitignore");
        std::fs::create_dir(dir.path().join("target")).expect("mkdir");
        touch(dir.path(), "kept.rs");
        std::fs::write(dir.path().join("target/built.rs"), b"x").expect("write");

        let on = expand(&["**/*.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(on, vec!["kept.rs".to_owned()], "ignored path dropped");

        let off = expand(&["**/*.rs".to_owned()], dir.path(), false).expect("resolve");
        assert_eq!(
            off,
            vec!["kept.rs".to_owned(), "target/built.rs".to_owned()]
        );

        let literal = expand(&["target/built.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(
            literal,
            vec!["target/built.rs".to_owned()],
            "explicit literal overrides gitignore"
        );
    }
}
