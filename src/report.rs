//! Human- and machine-readable rendering of engine reports.

use clap::ValueEnum;
use serde::Serialize;

use crate::engine::{GroupReport, Report, Status, UpdateAction, UpdateReport};
use crate::error::Result;

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
}

/// Machine-readable view of an evaluation report, carrying an explicit failure
/// signal and counts so consumers need not parse the process exit code.
#[derive(Serialize)]
struct ReportView<'a> {
    version: u32,
    failed: bool,
    total: usize,
    out_of_date: usize,
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
            failed: out_of_date > 0,
            total: report.groups.len(),
            out_of_date,
            groups: &report.groups,
        }
    }
}

/// Renders an evaluation [`Report`] in the requested `format`.
///
/// # Errors
///
/// Returns [`crate::error::Error::Json`] if JSON serialization fails.
pub fn render_report(report: &Report, format: Format) -> Result<String> {
    match format {
        Format::Quiet => Ok(String::new()),
        Format::Json => Ok(to_json(&ReportView::new(report))?),
        Format::Plain => Ok(render_report_plain(report)),
    }
}

/// Serializes `value` as pretty JSON with a trailing newline.
fn to_json<T: Serialize>(value: &T) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

fn render_report_plain(report: &Report) -> String {
    if report.groups.is_empty() {
        return "no groups defined\n".to_owned();
    }
    let mut out = String::new();
    for group in &report.groups {
        push_group_line(&mut out, group);
    }
    let total = report.groups.len();
    let failures = report
        .groups
        .iter()
        .filter(|group| group.status.is_failure())
        .count();
    if failures == 0 {
        out.push_str(&format!("\n{total} group(s) checked, none out of date\n"));
    } else {
        out.push_str(&format!(
            "\n{failures} of {total} group(s) out of date; review and run `outdatty update`\n"
        ));
    }
    out
}

fn push_group_line(out: &mut String, group: &GroupReport) {
    out.push_str(&format!("{}  {}\n", status_label(group.status), group.id));
    for path in &group.changed_sources {
        out.push_str(&format!("    source changed:    {path}\n"));
    }
    for path in &group.changed_dependents {
        out.push_str(&format!("    dependent changed: {path}\n"));
    }
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Ok => "[  ok   ]",
        Status::DependentDrift => "[ drift ]",
        Status::Stale => "[ stale ]",
        Status::New => "[  new  ]",
    }
}

/// Renders an [`UpdateReport`] in the requested `format`.
///
/// # Errors
///
/// Returns [`crate::error::Error::Json`] if JSON serialization fails.
pub fn render_update(report: &UpdateReport, format: Format) -> Result<String> {
    match format {
        Format::Quiet => Ok(String::new()),
        Format::Json => to_json(report),
        Format::Plain => Ok(render_update_plain(report)),
    }
}

fn render_update_plain(report: &UpdateReport) -> String {
    if report.entries.is_empty() {
        return "no groups updated\n".to_owned();
    }
    let mut out = String::new();
    for entry in &report.entries {
        out.push_str(&format!("{}  {}\n", action_label(entry.action), entry.id));
    }
    out
}

fn action_label(action: UpdateAction) -> &'static str {
    match action {
        UpdateAction::Added => "added  ",
        UpdateAction::Updated => "updated",
        UpdateAction::Unchanged => "current",
        UpdateAction::Removed => "removed",
    }
}

#[cfg(test)]
mod tests {
    use super::{Format, render_report, render_update};
    use crate::engine::{GroupReport, Report, Status, UpdateAction, UpdateEntry, UpdateReport};

    fn sample_report() -> Report {
        Report {
            groups: vec![
                GroupReport {
                    id: "ok-one".to_owned(),
                    status: Status::Ok,
                    changed_sources: Vec::new(),
                    changed_dependents: Vec::new(),
                },
                GroupReport {
                    id: "stale-one".to_owned(),
                    status: Status::Stale,
                    changed_sources: vec!["code.rs".to_owned()],
                    changed_dependents: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn plain_lists_groups_and_summary() {
        let text = render_report(&sample_report(), Format::Plain).expect("render");
        assert!(text.contains("stale-one"));
        assert!(text.contains("source changed:    code.rs"));
        assert!(text.contains("out of date"));
    }

    #[test]
    fn quiet_is_empty() {
        let text = render_report(&sample_report(), Format::Quiet).expect("render");
        assert!(text.is_empty());
    }

    #[test]
    fn json_is_machine_readable() {
        let text = render_report(&sample_report(), Format::Json).expect("render");
        assert!(text.contains("\"status\": \"stale\""));
        assert!(text.contains("\"changed_sources\""));
        assert!(text.contains("\"failed\": true"), "carries failure signal");
        assert!(text.contains("\"out_of_date\": 1"));
        assert!(text.ends_with("\n"), "json ends with newline");
    }

    #[test]
    fn empty_report_is_reported() {
        let report = Report { groups: Vec::new() };
        let text = render_report(&report, Format::Plain).expect("render");
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
        let text = render_update(&report, Format::Plain).expect("render");
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
        let json = render_update(&report, Format::Json).expect("render");
        assert!(json.contains("\"action\": \"updated\""));
        assert!(
            render_update(&report, Format::Quiet)
                .expect("render")
                .is_empty()
        );
    }

    #[test]
    fn empty_update_is_reported() {
        let report = UpdateReport {
            entries: Vec::new(),
        };
        let text = render_update(&report, Format::Plain).expect("render");
        assert!(text.contains("no groups updated"));
    }
}
