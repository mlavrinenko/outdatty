//! Command implementations, independent of argument parsing.
//!
//! Each function takes a [`Config`] (resolved paths and output format) plus any
//! command-specific arguments, performs the work, and returns the text to print.
//! Keeping this layer free of `clap` makes it directly unit-testable.

use std::path::{Path, PathBuf};

use crate::engine::{self, Filter, Report};
use crate::error::{Error, Result};
use crate::lock::{self, Lockfile};
use crate::manifest::{self, Manifest};
use crate::report::{self, Format};

/// Starter manifest written by [`init`].
const TEMPLATE: &str = r"# yaml-language-server: $schema=https://raw.githubusercontent.com/mlavrinenko/outdatty/main/schema/outdatty.schema.json

# Declare which dependents must be re-confirmed when a source changes.
# Confirm a group with `outdatty update`; gate it in CI with `outdatty check`.
groups:
  - name: example
    source:
      - src/feature.rs
    dependents:
      - docs/feature.mdx
      - tests/feature.rs
    # bidirectional: true   # also flag the group when a dependent changes
";

/// Resolved configuration shared by every command.
pub struct Config {
    /// Explicit manifest path, or `None` to discover the default.
    pub manifest: Option<PathBuf>,
    /// Explicit lockfile path, or `None` to derive it from the manifest.
    pub lock: Option<PathBuf>,
    /// Output format for rendered results.
    pub format: Format,
}

/// Loaded manifest plus the paths derived for an operation.
struct Context {
    manifest: Manifest,
    base: PathBuf,
    lock_path: PathBuf,
}

impl Config {
    fn manifest_path(&self) -> Result<PathBuf> {
        if let Some(path) = &self.manifest {
            return Ok(path.clone());
        }
        Manifest::discover(Path::new("."))
            .ok_or_else(|| Error::ManifestNotFound(PathBuf::from("outdatty.yaml")))
    }

    fn lock_path(&self, manifest_path: &Path) -> PathBuf {
        match &self.lock {
            Some(path) => path.clone(),
            None => base_dir(manifest_path).join(lock::DEFAULT_NAME),
        }
    }

    fn context(&self) -> Result<Context> {
        let manifest_path = self.manifest_path()?;
        let manifest = Manifest::load(&manifest_path)?;
        let base = base_dir(&manifest_path);
        let lock_path = self.lock_path(&manifest_path);
        Ok(Context {
            manifest,
            base,
            lock_path,
        })
    }
}

/// Outcome of a check: rendered output and whether any group is failing.
pub struct Outcome {
    /// Text to print, honouring the configured format.
    pub output: String,
    /// True if a group is stale or uninitialised.
    pub failed: bool,
}

/// Writes a starter manifest, returning the message to print.
///
/// # Errors
///
/// Returns [`Error::ManifestExists`] if the target exists and `force` is false,
/// or [`Error::Io`] if the file cannot be written.
pub fn init(config: &Config, force: bool) -> Result<String> {
    let path = config
        .manifest
        .clone()
        .unwrap_or_else(|| PathBuf::from("outdatty.yaml"));
    if path.exists() && !force {
        return Err(Error::ManifestExists(path));
    }
    std::fs::write(&path, TEMPLATE)?;
    if config.format == Format::Quiet {
        Ok(String::new())
    } else {
        Ok(format!("wrote {}\n", path.display()))
    }
}

fn evaluate(config: &Config, groups: &[String]) -> Result<Report> {
    let ctx = config.context()?;
    let lock = Lockfile::load_or_default(&ctx.lock_path)?;
    let filter = make_filter(&ctx.manifest, groups)?;
    engine::evaluate(&ctx.manifest, &lock, &ctx.base, &filter)
}

/// Evaluates the selected groups and reports whether any is out of date.
///
/// # Errors
///
/// Returns an error if the manifest cannot be loaded or an artifact cannot be
/// resolved, hashed, or rendered.
pub fn check(config: &Config, groups: &[String]) -> Result<Outcome> {
    let report = evaluate(config, groups)?;
    let output = report::render_report(&report, config.format)?;
    Ok(Outcome {
        output,
        failed: report.has_failure(),
    })
}

/// Renders the status of the selected groups without signalling failure.
///
/// # Errors
///
/// Returns an error if the manifest cannot be loaded or an artifact cannot be
/// resolved, hashed, or rendered.
pub fn status(config: &Config, groups: &[String]) -> Result<String> {
    let report = evaluate(config, groups)?;
    report::render_report(&report, config.format)
}

/// Refreshes the lockfile for the selected groups, returning the message to
/// print.
///
/// # Errors
///
/// Returns an error if the manifest cannot be loaded, an artifact cannot be
/// resolved or hashed, or the lockfile cannot be written.
pub fn update(config: &Config, groups: &[String]) -> Result<String> {
    let ctx = config.context()?;
    let lock = Lockfile::load_or_default(&ctx.lock_path)?;
    let filter = make_filter(&ctx.manifest, groups)?;
    let (next, report) = engine::build(&ctx.manifest, &lock, &ctx.base, &filter)?;
    next.save(&ctx.lock_path)?;
    report::render_update(&report, config.format)
}

/// Returns the JSON schema for the manifest.
///
/// # Errors
///
/// Returns [`Error::Json`] if serialization fails.
pub fn schema() -> Result<String> {
    manifest::schema_json()
}

fn base_dir(manifest_path: &Path) -> PathBuf {
    match manifest_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    }
}

fn make_filter(manifest: &Manifest, groups: &[String]) -> Result<Filter> {
    if groups.is_empty() {
        return Ok(Filter::All);
    }
    let known = engine::ids(manifest);
    for wanted in groups {
        if !known.iter().any(|id| id == wanted) {
            return Err(Error::UnknownGroup(wanted.clone()));
        }
    }
    let mut wanted = groups.to_vec();
    wanted.sort();
    wanted.dedup();
    Ok(Filter::Only(wanted))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Config, base_dir, check, init, make_filter, status, update};
    use crate::engine::Filter;
    use crate::manifest::{Group, Manifest};
    use crate::report::Format;

    fn write(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).expect("write");
    }

    fn project(dir: &Path) -> Config {
        write(
            dir,
            "outdatty.yaml",
            "groups:\n  - name: pair\n    source: [code.rs]\n    dependents: [doc.md]\n",
        );
        write(dir, "code.rs", "a");
        write(dir, "doc.md", "b");
        Config {
            manifest: Some(dir.join("outdatty.yaml")),
            lock: Some(dir.join("outdatty.lock")),
            format: Format::Plain,
        }
    }

    #[test]
    fn base_dir_falls_back_to_current_dir() {
        assert_eq!(base_dir(Path::new("outdatty.yaml")), PathBuf::from("."));
        assert_eq!(
            base_dir(Path::new("a/b/outdatty.yaml")),
            PathBuf::from("a/b")
        );
    }

    #[test]
    fn make_filter_validates_group_names() {
        let manifest = Manifest {
            groups: vec![Group {
                name: Some("known".to_owned()),
                ..Group::default()
            }],
            ..Manifest::default()
        };
        assert!(matches!(
            make_filter(&manifest, &[]).expect("all"),
            Filter::All
        ));
        assert!(make_filter(&manifest, &["known".to_owned()]).is_ok());
        assert!(make_filter(&manifest, &["ghost".to_owned()]).is_err());
    }

    #[test]
    fn init_writes_then_refuses_without_force() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = Config {
            manifest: Some(dir.path().join("outdatty.yaml")),
            lock: None,
            format: Format::Plain,
        };
        let message = init(&config, false).expect("init");
        assert!(message.contains("wrote"));
        assert!(init(&config, false).is_err(), "refuses to clobber");
        assert!(init(&config, true).is_ok(), "force overwrites");
    }

    #[test]
    fn init_is_silent_when_quiet() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = Config {
            manifest: Some(dir.path().join("outdatty.yaml")),
            lock: None,
            format: Format::Quiet,
        };
        assert!(init(&config, false).expect("init").is_empty());
    }

    #[test]
    fn check_then_update_then_check_round_trip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = project(dir.path());

        assert!(
            check(&config, &[]).expect("check").failed,
            "new group fails"
        );
        update(&config, &[]).expect("update");
        assert!(
            !check(&config, &[]).expect("check").failed,
            "synced after update"
        );

        write(dir.path(), "code.rs", "changed");
        let stale = check(&config, &[]).expect("check");
        assert!(stale.failed);
        assert!(stale.output.contains("source changed"));

        update(&config, &["pair".to_owned()]).expect("scoped update");
        assert!(!check(&config, &[]).expect("check").failed);
    }

    #[test]
    fn status_never_marks_failure() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = project(dir.path());
        update(&config, &[]).expect("update");
        write(dir.path(), "code.rs", "changed");
        let text = status(&config, &[]).expect("status");
        assert!(text.contains("stale"), "status still reports drift");
    }

    #[test]
    fn missing_manifest_is_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = Config {
            manifest: Some(dir.path().join("absent.yaml")),
            lock: None,
            format: Format::Plain,
        };
        assert!(check(&config, &[]).is_err());
    }
}
