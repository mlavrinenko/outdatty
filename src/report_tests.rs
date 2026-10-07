use super::{Format, Listing, render_report, render_update};
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
            unrecorded_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        },
        GroupReport {
            id: "stale-one".to_owned(),
            status: Status::Stale,
            changed_sources: vec!["code.rs".to_owned()],
            changed_dependents: Vec::new(),
            unrecorded_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        },
    ])
}

#[test]
fn plain_lists_groups_and_summary() {
    let text = render_report(&sample_report(), Format::Plain, false, Listing::All).expect("render");
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
        unrecorded_dependents: Vec::new(),
        dependents: vec!["doc.md".to_owned()],
    }]);
    let text = render_report(&report, Format::Plain, false, Listing::All).expect("render");
    assert!(
        !text.contains("review dependent:"),
        "ok groups stay terse; no review targets are listed"
    );
}

#[test]
fn plain_is_uncolored_when_color_off_and_styled_when_on() {
    let plain =
        render_report(&sample_report(), Format::Plain, false, Listing::All).expect("render");
    assert!(!plain.contains('\u{1b}'), "no escapes without color");

    let colored =
        render_report(&sample_report(), Format::Plain, true, Listing::All).expect("render");
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
        unrecorded_dependents: Vec::new(),
        dependents: vec!["doc.md".to_owned()],
    }]);
    let text = render_report(&report, Format::Plain, false, Listing::All).expect("render");
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
    let text = render_report(&sample_report(), Format::Quiet, false, Listing::All).expect("render");
    assert!(text.is_empty());
}

#[test]
fn json_is_machine_readable() {
    let text = render_report(&sample_report(), Format::Json, false, Listing::All).expect("render");
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
    let text =
        render_report(&report(Vec::new()), Format::Plain, false, Listing::All).expect("render");
    assert!(text.contains("no groups"));
}

#[test]
fn update_plain_lists_actions() {
    let report = UpdateReport {
        entries: vec![UpdateEntry {
            id: "g".to_owned(),
            action: UpdateAction::Added,
            recorded: Vec::new(),
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
            recorded: Vec::new(),
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
            unrecorded_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        },
        GroupReport {
            id: "a-group".to_owned(),
            status: Status::Stale,
            changed_sources: vec!["src/a.rs".to_owned()],
            changed_dependents: Vec::new(),
            unrecorded_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        },
    ]);
    let text = render_report(&report, Format::Paths, false, Listing::All).expect("render");
    assert_eq!(text, "src/a.rs\nsrc/z.rs\n");
}

#[test]
fn paths_is_empty_for_clean_report() {
    let text = render_report(&sample_report(), Format::Paths, false, Listing::All)
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
        unrecorded_dependents: Vec::new(),
        dependents: vec!["doc.md".to_owned()],
    }]);
    let text = render_report(&clean, Format::Paths, false, Listing::All).expect("render");
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
            unrecorded_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        },
        GroupReport {
            id: "a-group".to_owned(),
            status: Status::Stale,
            changed_sources: vec!["src/a.rs".to_owned()],
            changed_dependents: Vec::new(),
            unrecorded_dependents: Vec::new(),
            dependents: vec!["doc.md".to_owned()],
        },
    ]);
    let text = render_report(&report, Format::Paths0, false, Listing::All).expect("render");
    assert_eq!(text, "src/a.rs\0src/z.rs\0");
    assert!(!text.contains('\n'), "no newline characters in paths0");
}

#[test]
fn update_paths_is_empty() {
    let report = UpdateReport {
        entries: vec![UpdateEntry {
            id: "g".to_owned(),
            action: UpdateAction::Added,
            recorded: Vec::new(),
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

#[test]
fn update_plain_lists_recorded_dependents() {
    let report = UpdateReport {
        entries: vec![UpdateEntry {
            id: "g".to_owned(),
            action: UpdateAction::Updated,
            recorded: vec!["a.md".to_owned()],
        }],
    };
    let text = render_update(&report, Format::Plain, false).expect("render");
    assert_eq!(text, "updated  g\n    recorded dependent:  a.md\n");
    let json = render_update(&report, Format::Json, false).expect("render");
    assert!(json.contains("\"recorded\""));
}

#[test]
fn failing_listing_hides_passing_groups() {
    let text =
        render_report(&sample_report(), Format::Plain, false, Listing::Failing).expect("render");
    assert!(!text.contains("ok-one"), "passing group is not listed");
    assert!(text.contains("[ stale ]  stale-one"));
    assert!(text.ends_with("\noutdatty: 1 of 2 groups out of date\n"));
}

#[test]
fn summary_counts_untracked_files_with_or_without_failing_groups() {
    let mut clean = report(vec![GroupReport {
        id: "ok-one".to_owned(),
        status: Status::Ok,
        changed_sources: Vec::new(),
        changed_dependents: Vec::new(),
        unrecorded_dependents: Vec::new(),
        dependents: Vec::new(),
    }]);
    clean.untracked = vec!["a.rs".to_owned()];
    let text = render_report(&clean, Format::Plain, false, Listing::Failing).expect("render");
    assert!(text.contains("untracked file:    a.rs"));
    assert!(text.ends_with("\noutdatty: 1 untracked file\n"));

    let mut both = sample_report();
    both.untracked = vec!["a.rs".to_owned(), "b.rs".to_owned()];
    let text = render_report(&both, Format::Plain, false, Listing::Failing).expect("render");
    assert!(text.ends_with("\noutdatty: 1 of 2 groups out of date, 2 untracked files\n"));
}

#[test]
fn passing_check_is_one_line() {
    let clean = report(vec![GroupReport {
        id: "ok-one".to_owned(),
        status: Status::Ok,
        changed_sources: Vec::new(),
        changed_dependents: Vec::new(),
        unrecorded_dependents: Vec::new(),
        dependents: Vec::new(),
    }]);
    let text = render_report(&clean, Format::Plain, false, Listing::Failing).expect("render");
    assert_eq!(text, "outdatty: 1 group up to date\n");
}
