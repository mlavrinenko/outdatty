//! Human- and machine-readable rendering of engine reports.

use clap::ValueEnum;
use serde::Serialize;

use crate::coverage;
use crate::engine::{GroupReport, Report, Status, UpdateAction, UpdateEntry, UpdateReport};
use crate::error::Result;
use crate::style::Styler;

/// Schema version of the JSON report payload. Bump on incompatible changes.
const JSON_VERSION: u32 = 1;

/// Output format for command results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Format {
    /// Human-readable text (default).
    Plain,
    /// Machine-readable JSON.
    Json,
    /// No output; rely on the exit code.
    Quiet,
    /// Newline-delimited changed source paths, for piping into a diff tool.
    Paths,
    /// NUL-delimited changed source paths, safe for paths with spaces or newlines.
    Paths0,
}

/// Machine-readable view of an evaluation report, carrying an explicit failure
/// signal and counts so consumers need not parse the process exit code.
#[derive(Serialize)]
struct ReportView<'a> {
    version: u32,
    failed: bool,
    total: usize,
    out_of_date: usize,
    untracked: &'a [String],
    groups: &'a [GroupReport],
}

impl<'a> ReportView<'a> {
    fn new(report: &'a Report) -> Self {
        let out_of_date = report
            .groups
            .iter()
            .filter(|group| group.status.is_failure())
            .count();
        Self {
            version: JSON_VERSION,
            failed: out_of_date > 0 || !report.untracked.is_empty(),
            total: report.groups.len(),
            out_of_date,
            untracked: &report.untracked,
            groups: &report.groups,
        }
    }
}

/// Machine-readable view of an update report, carrying the same `version` and
/// `total` envelope as [`ReportView`] so consumers see a consistent shape.
#[derive(Serialize)]
struct UpdateView<'a> {
    version: u32,
    total: usize,
    entries: &'a [UpdateEntry],
}

impl<'a> UpdateView<'a> {
    fn new(report: &'a UpdateReport) -> Self {
        Self {
            version: JSON_VERSION,
            total: report.entries.len(),
            entries: &report.entries,
        }
    }
}

/// Renders an evaluation [`Report`] in the requested `format`.
///
/// `color` only affects [`Format::Plain`]; JSON and quiet output are never
/// styled so they stay machine-parseable.
///
/// # Errors
///
/// Returns [`crate::error::Error::Json`] if JSON serialization fails.
pub fn render_report(report: &Report, format: Format, color: bool) -> Result<String> {
    match format {
        Format::Quiet => Ok(String::new()),
        Format::Json => Ok(to_json(&ReportView::new(report))?),
        Format::Plain => Ok(render_report_plain(report, Styler::new(color))),
        Format::Paths => Ok(render_paths(report, '\n')),
        Format::Paths0 => Ok(render_paths(report, '\0')),
    }
}

/// Renders the deduped, sorted set of changed source paths across every
/// group, each followed by `delimiter` (including after the last path).
/// Never colored and carries no status labels or summary line, so it pipes
/// directly into an external diff tool.
fn render_paths(report: &Report, delimiter: char) -> String {
    let mut paths: Vec<&str> = report
        .groups
        .iter()
        .flat_map(|group| group.changed_sources.iter())
        .map(String::as_str)
        .collect();
    paths.sort_unstable();
    paths.dedup();
    let mut out = String::new();
    for path in paths {
        out.push_str(path);
        out.push(delimiter);
    }
    out
}

/// Serializes `value` as pretty JSON with a trailing newline.
fn to_json<T: Serialize>(value: &T) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

fn render_report_plain(report: &Report, styler: Styler) -> String {
    if report.groups.is_empty() && report.untracked.is_empty() {
        return "no groups defined\n".to_owned();
    }
    let mut out = String::new();
    for group in &report.groups {
        push_group_line(&mut out, group, styler);
    }
    out.push_str(&coverage::render_plain(&report.untracked, styler));
    let total = report.groups.len();
    let failures = report
        .groups
        .iter()
        .filter(|group| group.status.is_failure())
        .count();
    out.push('\n');
    out.push_str(&summary_line(
        styler,
        total,
        failures,
        report.untracked.len(),
    ));
    out.push('\n');
    out
}

fn summary_line(styler: Styler, total: usize, failures: usize, untracked: usize) -> String {
    if failures == 0 && untracked == 0 {
        return styler.green(&format!("{total} group(s) checked, none out of date"));
    }
    let mut parts = Vec::new();
    if failures > 0 {
        parts.push(format!("{failures} of {total} group(s) out of date"));
    }
    if untracked > 0 {
        parts.push(format!("{untracked} untracked file(s)"));
    }
    styler.red(&format!(
        "{}; review and run `outdatty update`",
        parts.join("; ")
    ))
}

fn push_group_line(out: &mut String, group: &GroupReport, styler: Styler) {
    out.push_str(&format!(
        "{}  {}\n",
        status_label(styler, group.status),
        group.id
    ));
    for path in &group.changed_sources {
        out.push_str(&styler.dim(&format!("    source changed:    {path}")));
        out.push('\n');
    }
    if group.status.is_failure() {
        for path in &group.dependents {
            out.push_str(&styler.dim(&format!("    review dependent:  {path}")));
            out.push('\n');
        }
        out.push_str(&styler.dim(&format!(
            "    confirm with:      outdatty update --group {}",
            group.id
        )));
        out.push('\n');
    }
}

fn status_label(styler: Styler, status: Status) -> String {
    match status {
        Status::Ok => styler.green("[  ok   ]"),
        Status::Stale => styler.red("[ stale ]"),
        Status::New => styler.red("[  new  ]"),
    }
}

/// Renders an [`UpdateReport`] in the requested `format`.
///
/// `color` only affects [`Format::Plain`].
///
/// # Errors
///
/// Returns [`crate::error::Error::Json`] if JSON serialization fails.
pub fn render_update(report: &UpdateReport, format: Format, color: bool) -> Result<String> {
    match format {
        Format::Json => to_json(&UpdateView::new(report)),
        Format::Plain => Ok(render_update_plain(report, Styler::new(color))),
        // Paths/Paths0 surface changed *sources* from a Report; UpdateReport
        // carries no such list, so there is nothing to emit (same as Quiet).
        Format::Quiet | Format::Paths | Format::Paths0 => Ok(String::new()),
    }
}

fn render_update_plain(report: &UpdateReport, styler: Styler) -> String {
    if report.entries.is_empty() {
        return "no groups updated\n".to_owned();
    }
    let mut out = String::new();
    for entry in &report.entries {
        out.push_str(&format!(
            "{}  {}\n",
            action_label(styler, entry.action),
            entry.id
        ));
    }
    out
}

fn action_label(styler: Styler, action: UpdateAction) -> String {
    match action {
        UpdateAction::Added => styler.green("added  "),
        UpdateAction::Updated => styler.yellow("updated"),
        UpdateAction::Unchanged => styler.dim("current"),
        UpdateAction::Removed => styler.red("removed"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Format, render_report, render_update};
    use crate::engine::{GroupReport, Report, Status, UpdateAction, UpdateEntry, UpdateReport};

    fn report(groups: Vec<GroupReport>) -> Report {
        Report {
            groups,
            untracked: Vec::new(),
        }
    }

    fn sample_report() -> Report {
        report(vec![
            GroupReport {
                id: "ok-one".to_owned(),
                status: Status::Ok,
                changed_sources: Vec::new(),
                changed_dependents: Vec::new(),
                dependents: vec!["doc.md".to_owned()],
            },
            GroupReport {
                id: "stale-one".to_owned(),
                status: Status::Stale,
                changed_sources: vec!["code.rs".to_owned()],
                changed_dependents: Vec::new(),
                dependents: vec!["doc.md".to_owned()],
            },
        ])
    }

    #[test]
    fn plain_lists_groups_and_summary() {
        let text = render_report(&sample_report(), Format::Plain, false).expect("render");
        assert!(text.contains("stale-one"));
        assert!(text.contains("source changed:    code.rs"));
        assert!(text.contains("out of date"));
        assert!(
            text.contains("confirm with:      outdatty update --group stale-one"),
            "failing group suggests the scoped command"
        );
        assert!(
            !text.contains("update --group ok-one"),
            "healthy group gets no suggestion"
        );
        assert!(
            text.contains("review dependent:  doc.md"),
            "failing group lists its declared dependents as review targets"
        );
    }

    #[test]
    fn plain_omits_review_dependent_for_ok_groups() {
        let report = report(vec![GroupReport {
            id: "ok-one".to_owned(),
            status: Status::Ok,
            changed_sources: Vec::new(),
            changed_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        }]);
        let text = render_report(&report, Format::Plain, false).expect("render");
        assert!(
            !text.contains("review dependent:"),
            "ok groups stay terse; no review targets are listed"
        );
    }

    #[test]
    fn plain_is_uncolored_when_color_off_and_styled_when_on() {
        let plain = render_report(&sample_report(), Format::Plain, false).expect("render");
        assert!(!plain.contains('\u{1b}'), "no escapes without color");

        let colored = render_report(&sample_report(), Format::Plain, true).expect("render");
        assert!(colored.contains('\u{1b}'), "escapes present with color");
        assert!(
            colored.contains("source changed:    code.rs"),
            "payload text survives styling"
        );
    }

    #[test]
    fn plain_omits_dependent_only_changes() {
        let report = report(vec![GroupReport {
            id: "directed".to_owned(),
            status: Status::Ok,
            changed_sources: Vec::new(),
            changed_dependents: vec!["doc.md".to_owned()],
            dependents: vec!["doc.md".to_owned()],
        }]);
        let text = render_report(&report, Format::Plain, false).expect("render");
        assert!(
            !text.contains("dependent changed"),
            "dependent-only edits are not surfaced in plain output"
        );
        assert!(
            text.contains("[  ok   ]"),
            "directed dependent edit reads as ok"
        );
    }

    #[test]
    fn quiet_is_empty() {
        let text = render_report(&sample_report(), Format::Quiet, false).expect("render");
        assert!(text.is_empty());
    }

    #[test]
    fn json_is_machine_readable() {
        let text = render_report(&sample_report(), Format::Json, false).expect("render");
        assert!(text.contains("\"status\": \"stale\""));
        assert!(text.contains("\"changed_sources\""));
        assert!(
            text.contains("\"dependents\""),
            "json carries review targets"
        );
        assert!(text.contains("\"failed\": true"), "carries failure signal");
        assert!(text.contains("\"out_of_date\": 1"));
        assert!(text.ends_with("\n"), "json ends with newline");
    }

    #[test]
    fn empty_report_is_reported() {
        let text = render_report(&report(Vec::new()), Format::Plain, false).expect("render");
        assert!(text.contains("no groups"));
    }

    #[test]
    fn update_plain_lists_actions() {
        let report = UpdateReport {
            entries: vec![UpdateEntry {
                id: "g".to_owned(),
                action: UpdateAction::Added,
            }],
        };
        let text = render_update(&report, Format::Plain, false).expect("render");
        assert!(text.contains("added"));
        assert!(text.contains('g'));
    }

    #[test]
    fn update_json_and_quiet() {
        let report = UpdateReport {
            entries: vec![UpdateEntry {
                id: "g".to_owned(),
                action: UpdateAction::Updated,
            }],
        };
        let json = render_update(&report, Format::Json, false).expect("render");
        assert!(json.contains("\"action\": \"updated\""));
        assert!(
            json.contains("\"version\": 1"),
            "update json carries version"
        );
        assert!(json.contains("\"total\": 1"), "update json carries total");
        assert!(
            render_update(&report, Format::Quiet, false)
                .expect("render")
                .is_empty()
        );
    }

    #[test]
    fn paths_lists_changed_sources_sorted_and_deduped() {
        let report = report(vec![
            GroupReport {
                id: "b-group".to_owned(),
                status: Status::Stale,
                changed_sources: vec!["src/z.rs".to_owned(), "src/a.rs".to_owned()],
                changed_dependents: Vec::new(),
                dependents: vec!["doc.md".to_owned()],
            },
            GroupReport {
                id: "a-group".to_owned(),
                status: Status::Stale,
                changed_sources: vec!["src/a.rs".to_owned()],
                changed_dependents: Vec::new(),
                dependents: vec!["doc.md".to_owned()],
            },
        ]);
        let text = render_report(&report, Format::Paths, false).expect("render");
        assert_eq!(text, "src/a.rs\nsrc/z.rs\n");
    }

    #[test]
    fn paths_is_empty_for_clean_report() {
        let text = render_report(&sample_report(), Format::Paths, false)
            .expect("render")
            .lines()
            .count();
        // sample_report has exactly one changed source across all groups.
        assert_eq!(text, 1);

        let clean = report(vec![GroupReport {
            id: "ok-one".to_owned(),
            status: Status::Ok,
            changed_sources: Vec::new(),
            changed_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        }]);
        let text = render_report(&clean, Format::Paths, false).expect("render");
        assert!(text.is_empty());
    }

    #[test]
    fn paths0_is_nul_separated_without_trailing_newline() {
        let report = report(vec![
            GroupReport {
                id: "b-group".to_owned(),
                status: Status::Stale,
                changed_sources: vec!["src/z.rs".to_owned()],
                changed_dependents: Vec::new(),
                dependents: vec!["doc.md".to_owned()],
            },
            GroupReport {
                id: "a-group".to_owned(),
                status: Status::Stale,
                changed_sources: vec!["src/a.rs".to_owned()],
                changed_dependents: Vec::new(),
                dependents: vec!["doc.md".to_owned()],
            },
        ]);
        let text = render_report(&report, Format::Paths0, false).expect("render");
        assert_eq!(text, "src/a.rs\0src/z.rs\0");
        assert!(!text.contains('\n'), "no newline characters in paths0");
    }

    #[test]
    fn update_paths_is_empty() {
        let report = UpdateReport {
            entries: vec![UpdateEntry {
                id: "g".to_owned(),
                action: UpdateAction::Added,
            }],
        };
        assert!(
            render_update(&report, Format::Paths, false)
                .expect("render")
                .is_empty()
        );
        assert!(
            render_update(&report, Format::Paths0, false)
                .expect("render")
                .is_empty()
        );
    }

    #[test]
    fn empty_update_is_reported() {
        let report = UpdateReport {
            entries: Vec::new(),
        };
        let text = render_update(&report, Format::Plain, false).expect("render");
        assert!(text.contains("no groups updated"));
    }
}
