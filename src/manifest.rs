//! The dependency-graph manifest (`outdatty.yaml`).

use std::fs;
use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Default manifest file names, tried in order during discovery.
const DEFAULT_NAMES: [&str; 2] = ["outdatty.yaml", "outdatty.yml"];

/// Default for [`Manifest::gitignore`].
const fn default_gitignore() -> bool {
    true
}

/// A declared dependency graph between artifacts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Dependency groups. Each group couples one or more source artifacts to
    /// the dependents that must be re-confirmed when a source changes.
    #[serde(default)]
    pub groups: Vec<Group>,

    /// When true (the default), glob expansion skips paths ignored by git —
    /// the `.gitignore` files (root and nested), the global excludes, and
    /// `.git/info/exclude` — so build artifacts and other generated files never
    /// enter a group. Set to false to match every file on disk.
    #[serde(default = "default_gitignore")]
    pub gitignore: bool,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            gitignore: default_gitignore(),
        }
    }
}

/// A single dependency group.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// Stable identifier used in reports and for `--group` targeting. Required
    /// and unique: lockfile entries are keyed by it, so it must not depend on a
    /// group's position in the manifest.
    pub name: String,

    /// Source artifacts. A change to any source marks the group out of date.
    /// Entries may be literal paths or glob patterns such as `src/**/*.rs`.
    pub source: Vec<String>,

    /// Dependent artifacts that must be re-confirmed when a source changes.
    pub dependents: Vec<String>,

    /// When true, a change to any dependent also marks the group out of date,
    /// modelling a bidirectional coupling.
    #[serde(default)]
    pub bidirectional: bool,
}

impl Manifest {
    /// Loads a manifest from `path`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ManifestNotFound`] if `path` does not exist, or
    /// [`Error::ManifestParse`] if it cannot be parsed.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(Error::ManifestNotFound(path.to_path_buf()));
        }
        let text = fs::read_to_string(path)?;
        let manifest: Self =
            serde_yaml_ng::from_str(&text).map_err(|source| Error::ManifestParse {
                path: path.to_path_buf(),
                source: Box::new(source),
            })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Checks structural invariants the schema cannot express: every group must
    /// carry a non-empty, unique `name` and declare at least one source.
    ///
    /// # Errors
    ///
    /// Returns [`Error::EmptyGroupName`], [`Error::DuplicateGroup`], or
    /// [`Error::EmptyGroupSource`].
    pub fn validate(&self) -> Result<()> {
        let mut seen: Vec<&str> = Vec::new();
        for (index, group) in self.groups.iter().enumerate() {
            if group.name.trim().is_empty() {
                return Err(Error::EmptyGroupName(index + 1));
            }
            if group.source.is_empty() {
                return Err(Error::EmptyGroupSource(group.name.clone()));
            }
            if seen.contains(&group.name.as_str()) {
                return Err(Error::DuplicateGroup(group.name.clone()));
            }
            seen.push(group.name.as_str());
        }
        Ok(())
    }

    /// Finds the default manifest in `dir`, returning the first existing name.
    #[must_use]
    pub fn discover(dir: &Path) -> Option<PathBuf> {
        for name in DEFAULT_NAMES {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }
}

/// Returns the JSON schema for the manifest as a pretty-printed string.
///
/// # Errors
///
/// Returns [`Error::Json`] if serialization fails.
pub fn schema_json() -> Result<String> {
    let schema = schemars::schema_for!(Manifest);
    let mut text = serde_json::to_string_pretty(&schema)?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::{Group, Manifest, schema_json};

    #[test]
    fn parses_a_minimal_manifest() {
        let text = "groups:\n  - name: g\n    source: [a.rs]\n    dependents: [a.md]\n";
        let manifest: Manifest = serde_yaml_ng::from_str(text).expect("parse");
        assert_eq!(manifest.groups.len(), 1);
        let group = manifest.groups.first().expect("group");
        assert_eq!(group.name, "g");
        assert_eq!(group.source, vec!["a.rs".to_owned()]);
        assert!(!group.bidirectional);
    }

    #[test]
    fn name_is_required() {
        let text = "groups:\n  - source: [a.rs]\n    dependents: [a.md]\n";
        let parsed: Result<Manifest, _> = serde_yaml_ng::from_str(text);
        assert!(parsed.is_err(), "a group without a name is rejected");
    }

    #[test]
    fn validate_rejects_blank_name() {
        let manifest = Manifest {
            groups: vec![Group {
                name: "   ".to_owned(),
                source: vec!["a".to_owned()],
                dependents: vec!["b".to_owned()],
                ..Group::default()
            }],
            ..Manifest::default()
        };
        assert!(manifest.validate().is_err(), "blank name rejected");
    }

    #[test]
    fn gitignore_defaults_to_true() {
        let text = "groups:\n  - name: g\n    source: [a.rs]\n    dependents: [a.md]\n";
        let manifest: Manifest = serde_yaml_ng::from_str(text).expect("parse");
        assert!(manifest.gitignore, "gitignore defaults on");
        assert!(Manifest::default().gitignore);
    }

    #[test]
    fn validate_rejects_duplicate_names_and_empty_source() {
        let dup = Manifest {
            groups: vec![
                Group {
                    name: "g".to_owned(),
                    source: vec!["a".to_owned()],
                    ..Group::default()
                },
                Group {
                    name: "g".to_owned(),
                    source: vec!["b".to_owned()],
                    ..Group::default()
                },
            ],
            ..Manifest::default()
        };
        assert!(dup.validate().is_err(), "duplicate names rejected");

        let empty = Manifest {
            groups: vec![Group {
                name: "g".to_owned(),
                source: Vec::new(),
                dependents: vec!["b".to_owned()],
                ..Group::default()
            }],
            ..Manifest::default()
        };
        assert!(empty.validate().is_err(), "empty source rejected");
    }

    #[test]
    fn load_rejects_duplicate_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("outdatty.yaml");
        std::fs::write(
            &path,
            "groups:\n  - name: g\n    source: [a]\n    dependents: [b]\n  - name: g\n    source: [c]\n    dependents: [d]\n",
        )
        .expect("write");
        assert!(Manifest::load(&path).is_err(), "load validates");
    }

    #[test]
    fn rejects_unknown_fields() {
        let text = "groups:\n  - name: g\n    source: [a]\n    dependents: [b]\n    bogus: 1\n";
        let parsed: Result<Manifest, _> = serde_yaml_ng::from_str(text);
        assert!(parsed.is_err(), "unknown fields are rejected");
    }

    #[test]
    fn schema_is_generatable() {
        let schema = schema_json().expect("schema");
        assert!(schema.contains("\"groups\""), "schema mentions groups");
        assert!(
            schema.contains("bidirectional"),
            "schema mentions bidirectional"
        );
    }

    #[test]
    fn committed_schema_is_current() {
        let generated = schema_json().expect("schema");
        let committed = include_str!("../schema/outdatty.schema.json");
        assert_eq!(
            generated, committed,
            "schema drifted; run `just gen-schema`"
        );
    }

    #[test]
    fn loads_and_discovers_from_disk() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("outdatty.yaml");
        std::fs::write(
            &path,
            "groups:\n  - name: g\n    source: [a]\n    dependents: [b]\n",
        )
        .expect("write");

        let manifest = Manifest::load(&path).expect("load");
        assert_eq!(manifest.groups.len(), 1);
        assert_eq!(Manifest::discover(dir.path()), Some(path));
        assert!(Manifest::discover(&dir.path().join("missing-subdir")).is_none());
    }

    #[test]
    fn load_missing_manifest_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(Manifest::load(&dir.path().join("nope.yaml")).is_err());
    }
}
