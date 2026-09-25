use phobos_scope_core::*;

fn fixture() -> Capture {
    serde_json::from_str(include_str!("../../../fixtures/known-detailed.json")).unwrap()
}
fn summary() -> Capture {
    let mut c = fixture();
    c.mode = Mode::Summary;
    c.events.clear();
    c
}
fn html(c: Capture) -> String {
    let mut bytes = Vec::new();
    report_html(&validate(c).unwrap(), &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn comparison_separates_frequency_from_mean_and_normalizes_wall_duration() {
    let before = validate(summary()).unwrap();
    let mut after = summary();
    after.capture_id = "after".into();
    after.end_tick = 40;
    after.aggregates[0].calls = 4;
    after.aggregates[0].total_ticks = 60;
    after.aggregates[0].max_ticks = 15;
    let report = compare(&before, &validate(after).unwrap());
    let outer = report
        .operations
        .iter()
        .find(|o| o.name == "fixture.outer")
        .unwrap();
    let frequency = outer.calls_per_second.as_ref().unwrap();
    assert_eq!(
        (frequency.before, frequency.after, frequency.percent),
        (Some(50.0), Some(100.0), Some(100.0))
    );
    assert_eq!(outer.mean_ms.as_ref().unwrap().percent, Some(50.0));
    assert_eq!(outer.total_ms.as_ref().unwrap().percent, Some(500.0));
    assert_eq!(
        outer.inclusive_ms_per_second.as_ref().unwrap().percent,
        Some(200.0)
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.contains("durations differ"))
    );
}

#[test]
fn metric_ids_and_clock_frequencies_do_not_define_comparison_identity() {
    let before = validate(fixture()).unwrap();
    let mut after = fixture();
    after.definitions.swap(0, 1);
    after.definitions[0].id = 0;
    after.definitions[1].id = 1;
    for a in &mut after.aggregates {
        a.metric = 1 - a.metric;
        a.total_ticks *= 1000;
        a.max_ticks *= 1000;
    }
    for e in &mut after.events {
        e.metric = 1 - e.metric;
        e.start_tick *= 1000;
        e.duration_ticks *= 1000;
    }
    for s in &mut after.counters {
        s.tick *= 1000;
    }
    for s in &mut after.contexts {
        s.tick *= 1000;
    }
    after.clock_frequency_hz *= 1000;
    after.end_tick *= 1000;
    after.limits.max_duration_ticks *= 1000;
    let report = compare(&before, &validate(after).unwrap());
    assert!(
        report
            .operations
            .iter()
            .all(|op| op.mean_ms.as_ref().unwrap().difference == Some(0.0)
                && op.calls_per_second.as_ref().unwrap().difference == Some(0.0))
    );
}

#[test]
fn missing_and_incompatible_definitions_never_receive_deltas() {
    let before = validate(summary()).unwrap();
    let mut after = summary();
    after.definitions[0].name = "new.operation".into();
    after.definitions[1].category = "changed".into();
    let report = compare(&before, &validate(after).unwrap());
    assert_eq!(
        report
            .operations
            .iter()
            .map(|o| (o.name.as_str(), o.status))
            .collect::<Vec<_>>(),
        vec![
            ("fixture.inner", "incompatible_definition"),
            ("fixture.outer", "removed"),
            ("new.operation", "added")
        ]
    );
    assert!(
        report
            .operations
            .iter()
            .all(|o| o.mean_ms.is_none() && o.total_ms.is_none())
    );
    let mut after = summary();
    after.definitions[0].kind = Kind::Gauge;
    after.definitions[0].unit = "items".into();
    after.aggregates.remove(0);
    let report = compare(&before, &validate(after).unwrap());
    assert_eq!(
        report
            .operations
            .iter()
            .find(|o| o.name == "fixture.outer")
            .unwrap()
            .status,
        "incompatible_definition"
    );
}

#[test]
fn zero_baselines_and_no_calls_are_not_infinite_or_fabricated() {
    let mut empty = summary();
    for a in &mut empty.aggregates {
        a.calls = 0;
        a.total_ticks = 0;
        a.max_ticks = 0;
    }
    let report = compare(
        &validate(empty.clone()).unwrap(),
        &validate(summary()).unwrap(),
    );
    let row = &report.operations[0];
    assert_eq!(row.calls_per_second.as_ref().unwrap().percent, None);
    assert_eq!(row.mean_ms.as_ref().unwrap().difference, None);
    assert_eq!(row.total_ms.as_ref().unwrap().difference, Some(3.0));
    empty.end_tick = 0;
    empty.contexts.clear();
    empty.counters.clear();
    let report = compare(&validate(empty).unwrap(), &validate(summary()).unwrap());
    assert_eq!(
        report.operations[0]
            .calls_per_second
            .as_ref()
            .unwrap()
            .difference,
        None
    );
    assert!(report.warnings.iter().any(|w| w.contains("zero duration")));
    assert!(!serde_json::to_string(&report).unwrap().contains("NaN"));
}

#[test]
fn context_timing_versions_and_loss_are_visible_without_discarding_aggregates() {
    let before = validate(fixture()).unwrap();
    let mut after = fixture();
    after.contexts[1].tick = 10;
    after.metadata[0].value = "different workload".into();
    after.recorder_version = "new/2".into();
    after.events.remove(0);
    after.dropped_records = 1;
    after.rejected_measurements = 2;
    after.aggregates[0].incomplete = 1;
    let report = compare(&before, &validate(after).unwrap());
    for term in [
        "Recorder versions",
        "Metadata differs",
        "Observed context differs",
        "dropped_records",
        "rejected_measurements",
        "incomplete_scopes",
        "changed",
    ] {
        assert!(report.warnings.iter().any(|w| w.contains(term)), "{term}");
    }
    assert_eq!(
        report.operations[0].mean_ms.as_ref().unwrap().difference,
        Some(0.0)
    );
    assert_eq!(
        report.after.contexts["fixture.speed"].points[1].time_ms,
        10.0
    );
}

#[test]
fn html_is_self_contained_escaped_and_preserves_summary_and_loss_semantics() {
    let mut c = fixture();
    let hostile = "</title><script>alert('x')</script>&\"";
    c.capture_id = hostile.into();
    c.definitions[0].name = hostile.into();
    c.metadata[0].value = hostile.into();
    c.contexts[0].value = hostile.into();
    let page = html(c.clone());
    assert!(!page.contains("<script"));
    assert!(!page.contains(hostile));
    assert!(page.contains("&lt;script&gt;"));
    assert!(page.contains("Content-Security-Policy"));
    assert!(page.contains("<svg"));
    assert!(page.contains("nested calls and waits"));
    c.mode = Mode::Summary;
    c.events.clear();
    let page = html(c.clone());
    assert!(!page.contains("<svg"));
    assert!(page.contains("summary mode does not record"));
    let capture = validate(c).unwrap();
    let mut out = Vec::new();
    comparison_html(&compare(&capture, &capture), &mut out).unwrap();
    assert!(!String::from_utf8(out).unwrap().contains(hostile));
    let mut lost = fixture();
    lost.events.remove(0);
    lost.dropped_records = 1;
    assert!(html(lost).contains("timeline, counters and context are partial"));
}

#[test]
fn comparison_csv_escapes_labels_and_preserves_unavailable_values() {
    let mut c = summary();
    c.definitions[0].name = "=formula,\"quoted\"\nline".into();
    c.aggregates[0].calls = 0;
    c.aggregates[0].total_ticks = 0;
    c.aggregates[0].max_ticks = 0;
    let c = validate(c).unwrap();
    let mut out = Vec::new();
    comparison_csv(&compare(&c, &c), &mut out).unwrap();
    let rows: Vec<_> = csv::Reader::from_reader(out.as_slice())
        .records()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(&rows[0][0], "'=formula,\"quoted\"\nline");
    assert_eq!(&rows[0][7], "");
    assert_eq!(&rows[1][4], "");
    assert_eq!(&rows[1][6], "");
}

#[test]
fn report_writers_propagate_io_errors() {
    struct Fails;
    impl std::io::Write for Fails {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("full"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let c = validate(fixture()).unwrap();
    let report = compare(&c, &c);
    assert!(report_html(&c, Fails).is_err());
    assert!(comparison_html(&report, Fails).is_err());
    assert!(comparison_csv(&report, Fails).is_err());
}
