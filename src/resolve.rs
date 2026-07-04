//! Expansion of manifest patterns into concrete artifact paths.

use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use ignore::{DirEntry, Walk, WalkBuilder};

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
/// hard error). When `gitignore` is true, glob matches ignored by git — the
/// repository's `.gitignore` files (root and nested), the global excludes, and
/// `.git/info/exclude` — are dropped; explicitly listed literals are always
/// kept. The `.git` directory itself is never traversed. Symlinks are not
/// followed during glob expansion. Paths are normalized to use forward slashes.
///
/// # Errors
///
/// Returns [`Error::Pattern`] if a glob pattern is invalid.
pub fn expand(patterns: &[String], base: &Path, gitignore: bool) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut globs: Vec<&str> = Vec::new();
    for pattern in patterns {
        if is_glob(pattern) {
            globs.push(pattern);
        } else {
            expand_literal(pattern, base, &mut out);
        }
    }
    if !globs.is_empty() {
        expand_globs(&globs, base, gitignore, &mut out)?;
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Walks `base` and returns every file path, relative to `base` and
/// slash-normalized, honouring the gitignore chain when `gitignore` is set. Used
/// to enumerate the project's files for coverage checks. Directories, symlinks,
/// and the `.git` directory are never returned.
#[must_use]
pub fn all_files(base: &Path, gitignore: bool) -> Vec<String> {
    let mut out = Vec::new();
    for entry in build_walker(base, gitignore) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(path);
        out.push(normalize(&rel.to_string_lossy()));
    }
    out.sort();
    out.dedup();
    out
}

fn expand_literal(pattern: &str, base: &Path, out: &mut Vec<String>) {
    if base.join(pattern).is_file() {
        out.push(normalize(pattern));
    } else {
        log::warn!("literal path `{pattern}` is missing; treating as removed");
    }
}

/// Compiles `globs` into a matcher, then walks `base` once (honouring the
/// gitignore chain when `gitignore` is set) and collects every file that
/// matches at least one pattern. Patterns matching nothing are warned about.
fn expand_globs(globs: &[&str], base: &Path, gitignore: bool, out: &mut Vec<String>) -> Result<()> {
    let set = build_globset(globs)?;
    let mut matched = vec![false; globs.len()];
    for entry in build_walker(base, gitignore) {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(base).unwrap_or(path);
        let candidate = normalize(&rel.to_string_lossy());
        let hits = set.matches(&candidate);
        if hits.is_empty() {
            continue;
        }
        for index in hits {
            if let Some(flag) = matched.get_mut(index) {
                *flag = true;
            }
        }
        out.push(candidate);
    }
    for (index, pattern) in globs.iter().enumerate() {
        if matched.get(index) == Some(&false) {
            log::warn!("pattern `{pattern}` matched no files");
        }
    }
    Ok(())
}

/// Builds a [`GlobSet`] from `globs`, treating `/` as a literal separator so
/// `*` does not cross directories and `**` does (matching common glob tools).
fn build_globset(globs: &[&str]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in globs {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|source| Error::Pattern {
                pattern: (*pattern).to_owned(),
                source,
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|source| Error::Pattern {
        pattern: globs.join(", "),
        source,
    })
}

/// Builds a file walker rooted at `base`. Hidden files are included (only
/// ignore rules filter, never the dotfile heuristic) and the `.git` directory
/// is always pruned. When `gitignore` is set, the full gitignore chain applies;
/// `.gitignore` is also added as a custom ignore file so nested manifests are
/// honoured even outside a git repository.
fn build_walker(base: &Path, gitignore: bool) -> Walk {
    let mut builder = WalkBuilder::new(base);
    builder
        .hidden(false)
        .follow_links(false)
        .ignore(false)
        .git_ignore(gitignore)
        .git_global(gitignore)
        .git_exclude(gitignore)
        .parents(gitignore)
        .filter_entry(skip_git_dir);
    if gitignore {
        builder.add_custom_ignore_filename(".gitignore");
    }
    builder.build()
}

fn skip_git_dir(entry: &DirEntry) -> bool {
    entry.file_name() != ".git"
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{all_files, expand, is_glob};

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
    fn star_does_not_cross_directories_but_globstar_does() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("sub")).expect("mkdir");
        touch(dir.path(), "top.rs");
        std::fs::write(dir.path().join("sub/nested.rs"), b"x").expect("write");

        let shallow = expand(&["*.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(shallow, vec!["top.rs".to_owned()], "single star stays flat");

        let deep = expand(&["**/*.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(deep, vec!["sub/nested.rs".to_owned(), "top.rs".to_owned()]);
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

    #[test]
    fn all_files_lists_every_non_ignored_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join(".gitignore"), "/target\n").expect("gitignore");
        std::fs::create_dir(dir.path().join("target")).expect("mkdir");
        std::fs::create_dir(dir.path().join("src")).expect("mkdir");
        touch(dir.path(), "top.rs");
        std::fs::write(dir.path().join("src/nested.rs"), b"x").expect("write");
        std::fs::write(dir.path().join("target/built.rs"), b"x").expect("write");

        let on = all_files(dir.path(), true);
        assert_eq!(
            on,
            vec![
                ".gitignore".to_owned(),
                "src/nested.rs".to_owned(),
                "top.rs".to_owned()
            ],
            "ignored target/ is dropped; nested files are included"
        );
        let off = all_files(dir.path(), false);
        assert!(
            off.contains(&"target/built.rs".to_owned()),
            "gitignore off lists all"
        );
    }

    #[test]
    fn nested_gitignore_is_honoured() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir(dir.path().join("pkg")).expect("mkdir");
        std::fs::write(dir.path().join("pkg/.gitignore"), "generated.rs\n").expect("nested");
        std::fs::write(dir.path().join("pkg/kept.rs"), b"x").expect("write");
        std::fs::write(dir.path().join("pkg/generated.rs"), b"x").expect("write");

        let resolved = expand(&["**/*.rs".to_owned()], dir.path(), true).expect("resolve");
        assert_eq!(
            resolved,
            vec!["pkg/kept.rs".to_owned()],
            "nested .gitignore drops generated.rs"
        );
    }
}
