//! Whole-project coverage: files `require_tracked` demands belong to a group.
//!
//! A glob source already flags a *changed* file. Coverage answers the other
//! half — a brand-new file that no group lists at all. The manifest's
//! `require_tracked` patterns name the files that must appear in some group's
//! `source` or `dependents`; any required file that does not is reported so a
//! `check` fails until it is wired in (or excluded).

use std::collections::BTreeSet;
use std::path::Path;

use globset::{GlobBuilder, GlobMatcher};

use crate::error::{Error, Result};
use crate::manifest::Manifest;
use crate::resolve;
use crate::style::Styler;

/// Compiled `require_tracked` patterns with last-match-wins semantics: patterns
/// are tried in order and the last to match a path decides membership; a
/// `!`-prefixed pattern negates it. A path matched by no pattern is not required.
struct Requirement {
    globs: Vec<(GlobMatcher, bool)>,
}

impl Requirement {
    fn compile(patterns: &[String]) -> Result<Self> {
        let mut globs = Vec::with_capacity(patterns.len());
        for raw in patterns {
            let (negated, body) = raw
                .strip_prefix('!')
                .map_or((false, raw.as_str()), |rest| (true, rest));
            let glob = GlobBuilder::new(body)
                .literal_separator(true)
                .build()
                .map_err(|source| Error::Pattern {
                    pattern: raw.clone(),
                    source,
                })?;
            globs.push((glob.compile_matcher(), negated));
        }
        Ok(Self { globs })
    }

    fn requires(&self, path: &str) -> bool {
        let mut required = false;
        for (glob, negated) in &self.globs {
            if glob.is_match(path) {
                required = !negated;
            }
        }
        required
    }
}

/// Files that `require_tracked` demands but no group covers, sorted.
///
/// The universe is every file under `base` git does not ignore (when
/// `manifest.gitignore` is set); `exempt` (the manifest and lockfile, as
/// base-relative slash paths) is always excluded. A file is covered when it
/// appears in the expansion of any group's `source` or `dependents`.
///
/// # Errors
///
/// Returns [`Error::Pattern`] if a `require_tracked` or group pattern is invalid.
pub fn untracked(manifest: &Manifest, base: &Path, exempt: &[String]) -> Result<Vec<String>> {
    let requirement = Requirement::compile(&manifest.require_tracked)?;
    let covered = covered_set(manifest, base)?;
    let exempt: BTreeSet<&str> = exempt.iter().map(String::as_str).collect();
    let mut out: Vec<String> = resolve::all_files(base, manifest.gitignore)
        .into_iter()
        .filter(|path| requirement.requires(path))
        .filter(|path| !exempt.contains(path.as_str()))
        .filter(|path| !covered.contains(path))
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

fn covered_set(manifest: &Manifest, base: &Path) -> Result<BTreeSet<String>> {
    let mut covered = BTreeSet::new();
    for group in &manifest.groups {
        for path in resolve::expand(&group.source, base, manifest.gitignore)? {
            covered.insert(path);
        }
        for path in resolve::expand(&group.dependents, base, manifest.gitignore)? {
            covered.insert(path);
        }
    }
    Ok(covered)
}

/// Renders the untracked block appended to a plain report, or an empty string
/// when nothing is untracked.
#[must_use]
pub fn render_plain(untracked: &[String], styler: Styler) -> String {
    if untracked.is_empty() {
        return String::new();
    }
    let mut out = styler.red(&format!(
        "[untracked]  {} file(s) covered by no group",
        untracked.len()
    ));
    out.push('\n');
    for path in untracked {
        out.push_str(&styler.dim(&format!("    untracked file:    {path}")));
        out.push('\n');
    }
    out.push_str(
        &styler.dim("    add each to a group, or exclude via `require_tracked` in the manifest"),
    );
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Requirement, render_plain, untracked};
    use crate::manifest::{Group, Manifest};
    use crate::style::Styler;

    fn write(base: &Path, name: &str, body: &str) {
        if let Some(parent) = Path::new(name).parent() {
            std::fs::create_dir_all(base.join(parent)).expect("mkdir");
        }
        std::fs::write(base.join(name), body).expect("write");
    }

    #[test]
    fn require_globstar_matches_every_depth() {
        let req = Requirement::compile(&["**".to_owned()]).expect("compile");
        assert!(req.requires("top.rs"));
        assert!(req.requires("src/a/b.rs"));
    }

    #[test]
    fn require_last_match_wins_with_negation() {
        let req = Requirement::compile(&[
            "**".to_owned(),
            "!tasks/**".to_owned(),
            "!LICENSE".to_owned(),
        ])
        .expect("compile");
        assert!(req.requires("src/main.rs"));
        assert!(!req.requires("tasks/todo.typ"), "negated subtree excluded");
        assert!(!req.requires("LICENSE"), "negated literal excluded");
    }

    #[test]
    fn require_allowlist_needs_no_leading_negation() {
        let req = Requirement::compile(&["src/**".to_owned()]).expect("compile");
        assert!(req.requires("src/a.rs"));
        assert!(!req.requires("README.md"), "unmatched path not required");
    }

    fn manifest_with(group: Group, require: &[&str]) -> Manifest {
        Manifest {
            groups: vec![group],
            require_tracked: require.iter().map(|s| (*s).to_owned()).collect(),
            ..Manifest::default()
        }
    }

    #[test]
    fn reports_brand_new_file_missing_from_every_group() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "src/a.rs", "a");
        write(dir.path(), "src/b.rs", "b"); // brand-new, in no group
        write(dir.path(), "README.md", "docs");
        let group = Group {
            name: "g".to_owned(),
            source: vec!["src/a.rs".to_owned()],
            dependents: vec!["README.md".to_owned()],
            ..Group::default()
        };
        let manifest = manifest_with(group, &["**"]);
        let missing =
            untracked(&manifest, dir.path(), &["outdatty.yaml".to_owned()]).expect("coverage");
        assert_eq!(missing, vec!["src/b.rs".to_owned()]);
    }

    #[test]
    fn glob_source_covers_new_files_and_exempt_and_negation_are_honoured() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "src/a.rs", "a");
        write(dir.path(), "src/b.rs", "b");
        write(dir.path(), "tasks/todo.typ", "t"); // excluded by negation
        write(dir.path(), "outdatty.yaml", "manifest"); // exempt
        let group = Group {
            name: "code".to_owned(),
            source: vec!["src/**/*.rs".to_owned()],
            ..Group::default()
        };
        let manifest = manifest_with(group, &["**", "!tasks/**"]);
        let missing =
            untracked(&manifest, dir.path(), &["outdatty.yaml".to_owned()]).expect("coverage");
        assert!(
            missing.is_empty(),
            "glob source covers both rs files: {missing:?}"
        );
    }

    #[test]
    fn render_plain_is_empty_when_covered_and_lists_when_not() {
        assert!(render_plain(&[], Styler::new(false)).is_empty());
        let text = render_plain(&["src/x.rs".to_owned()], Styler::new(false));
        assert!(text.contains("untracked file:    src/x.rs"));
        assert!(text.contains("require_tracked"));
    }
}
