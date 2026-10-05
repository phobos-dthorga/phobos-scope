//! Format 3 self times, counter comparison and the recording series (recorder 0.3.0, analyser 0.4.0).
use phobos_scope_core::*;

fn fixture() -> Capture {
    serde_json::from_str(include_str!("../../../fixtures/known-detailed.json")).unwrap()
}

/// The known fixture (outer 10 ticks holding inner 3) as a format 3 capture.
fn format_three() -> Capture {
    let mut c = fixture();
    c.format_version = 3;
    c.aggregates[0].self_ticks = Some(7);
    c.aggregates[1].self_ticks = Some(3);
    c.counter_aggregates = vec![CounterAggregate {
        metric: 2,
        samples: 1,
        sum: 7.0,
        min: 7.0,
        max: 7.0,
        last: 7.0,
    }];
    c
}

#[test]
fn self_time_is_checked_against_retained_events() {
    let c = validate(format_three()).unwrap();
    let s = statistics(&c);
    assert_eq!((s[0].self_ms, s[1].self_ms), (Some(7.0), Some(3.0)));
    assert_eq!(s[0].mean_self_ms, Some(7.0));
    // Self times do not overlap: 7 + 3 ticks of a 20-tick capture.
    assert_eq!(measured_share(&c), Some(0.5));
    let mut wrong = format_three();
    wrong.aggregates[0].self_ticks = Some(8);
    assert!(
        validate(wrong).is_err(),
        "self time must match the events when nothing was dropped"
    );
    let mut missing = format_three();
    missing.aggregates[1].self_ticks = None;
    assert!(
        validate(missing).is_err(),
        "format 3 aggregates carry self time"
    );
    let mut early = fixture();
    early.aggregates[0].self_ticks = Some(7);
    assert!(
        validate(early).is_err(),
        "format 1 aggregates carry no self time"
    );
    let mut over = format_three();
    over.mode = Mode::Summary;
    over.events.clear();
    over.counters.clear();
    over.aggregates[1].self_ticks = Some(4);
    assert!(validate(over).is_err(), "self time cannot exceed the total");
    assert!(measured_share(&validate(fixture()).unwrap()).is_none());
}

#[test]
fn equal_spans_follow_completion_order() {
    // Inner completes first and spans exactly what outer spans: the recorder gave inner all the self time.
    let mut c = format_three();
    c.events = vec![
        Event {
            metric: 1,
            start_tick: 0,
            duration_ticks: 5,
        },
        Event {
            metric: 0,
            start_tick: 0,
            duration_ticks: 5,
        },
    ];
    c.end_tick = 5;
    c.aggregates[0].total_ticks = 5;
    c.aggregates[0].max_ticks = 5;
    c.aggregates[0].self_ticks = Some(0);
    c.aggregates[1].total_ticks = 5;
    c.aggregates[1].max_ticks = 5;
    c.aggregates[1].self_ticks = Some(5);
    c.counters.clear();
    c.contexts.clear();
    c.counter_aggregates[0] = CounterAggregate {
        metric: 2,
        samples: 0,
        sum: 0.0,
        min: 0.0,
        max: 0.0,
        last: 0.0,
    };
    validate(c).unwrap();
}

fn summary_with(heap: f64, rate: f64, window: u64, recording: &str) -> ValidatedCapture {
    let mut c = format_three();
    c.mode = Mode::Summary;
    c.events.clear();
    c.counters.clear();
    c.end_tick = 1000;
    c.definitions.push(Definition {
        id: 4,
        name: "memory.managed_heap".into(),
        category: "memory".into(),
        unit: "bytes".into(),
        kind: Kind::Gauge,
    });
    c.definitions.push(Definition {
        id: 5,
        name: "test.hits".into(),
        category: "test".into(),
        unit: "hits".into(),
        kind: Kind::Increment,
    });
    c.counter_aggregates.push(CounterAggregate {
        metric: 4,
        samples: 2,
        sum: heap * 2.0 + 10.0,
        min: heap,
        max: heap + 10.0,
        last: heap + 10.0,
    });
    c.counter_aggregates.push(CounterAggregate {
        metric: 5,
        samples: 4,
        sum: rate,
        min: 0.0,
        max: rate,
        last: 1.0,
    });
    c.metadata.push(Metadata {
        key: RECORDING_ID_KEY.into(),
        value: recording.into(),
    });
    c.metadata.push(Metadata {
        key: WINDOW_KEY.into(),
        value: window.to_string(),
    });
    validate(c).unwrap()
}

#[test]
fn comparison_covers_memory_and_rates() {
    let before = summary_with(100.0, 20.0, 1, "a");
    let after = summary_with(150.0, 10.0, 1, "b");
    let report = compare(&before, &after);
    let heap = report
        .counters
        .iter()
        .find(|c| c.name == "memory.managed_heap")
        .unwrap();
    assert_eq!(heap.status, "matched");
    assert_eq!(heap.max.as_ref().unwrap().difference, Some(50.0));
    assert!(heap.per_second.is_none(), "a level has no rate");
    let hits = report
        .counters
        .iter()
        .find(|c| c.name == "test.hits")
        .unwrap();
    let rate = hits.per_second.as_ref().unwrap();
    assert_eq!(
        (rate.before, rate.after),
        (Some(20.0), Some(10.0)),
        "increments compare per real second"
    );
    let outer = report
        .operations
        .iter()
        .find(|o| o.name == "fixture.outer")
        .unwrap();
    assert!(outer.self_ms_per_second.is_some());
    // A counter only one side has gets no difference.
    let mut lone = format_three();
    lone.definitions[2].unit = "crates".into();
    let changed = compare(&validate(fixture()).unwrap(), &validate(lone).unwrap());
    let queue = changed
        .counters
        .iter()
        .find(|c| c.name == "fixture.queue")
        .unwrap();
    assert_eq!(queue.status, "incompatible_definition");
    assert!(queue.mean.is_none());
    assert!(
        changed
            .warnings
            .iter()
            .any(|w| w.contains("retained samples"))
    );
    let mut csv = Vec::new();
    comparison_csv(&report, &mut csv).unwrap();
    assert!(
        String::from_utf8(csv)
            .unwrap()
            .contains("counter_per_second")
    );
}

#[test]
fn series_orders_windows_and_reports_a_floor_trend() {
    // Given out of order; each window is one recorded second, its heap floor rising 10 bytes a window.
    let captures = vec![
        summary_with(120.0, 4.0, 3, "rec"),
        summary_with(100.0, 4.0, 1, "rec"),
        summary_with(110.0, 4.0, 2, "rec"),
    ];
    let report = series(&captures).unwrap();
    assert_eq!(
        report.windows.iter().map(|w| w.window).collect::<Vec<_>>(),
        vec![Some(1), Some(2), Some(3)]
    );
    assert_eq!(report.windows[2].start_s, 2.0);
    assert_eq!(report.recorded_s, 3.0);
    let heap = report
        .gauges
        .iter()
        .find(|g| g.name == "memory.managed_heap")
        .unwrap();
    assert_eq!(
        (heap.first_floor, heap.last_floor),
        (Some(100.0), Some(120.0))
    );
    let per_hour = heap.floor_change_per_hour.unwrap();
    assert!(
        (per_hour - 36_000.0).abs() < 1e-6,
        "10 bytes a recorded second is 36,000 an hour: {per_hour}"
    );
    let hits = report
        .increments
        .iter()
        .find(|i| i.name == "test.hits")
        .unwrap();
    assert!(hits.points.iter().all(|p| p.per_second == Some(4.0)));
    assert_eq!(report.operations[0].basis, "self");
    assert_eq!(
        report.warnings.len(),
        1,
        "one continuous recording needs only the general note"
    );
    // Two windows are too few for a trend; a gap and a second recording are named.
    let short = series(&captures[..2]).unwrap();
    assert!(
        short
            .gauges
            .iter()
            .all(|g| g.floor_change_per_hour.is_none())
    );
    let mixed = series(&[
        summary_with(100.0, 1.0, 1, "a"),
        summary_with(100.0, 1.0, 3, "a"),
        summary_with(100.0, 1.0, 1, "b"),
    ])
    .unwrap();
    assert!(mixed.warnings.iter().any(|w| w.contains("2 recordings")));
    assert!(
        mixed
            .warnings
            .iter()
            .any(|w| w.contains("missing or repeated"))
    );
    // Without recording metadata the given order stands, and the report says so.
    let plain = series(&[validate(format_three()).unwrap()]).unwrap();
    assert!(plain.warnings.iter().any(|w| w.contains("order given")));
    assert!(series(&[]).is_err());
    let mut html = Vec::new();
    series_html(&report, &mut html).unwrap();
    let html = String::from_utf8(html).unwrap();
    assert!(
        html.contains("<svg") && html.contains("memory.managed_heap") && !html.contains("<script")
    );
    let mut csv = Vec::new();
    series_csv(&report, &mut csv).unwrap();
    assert!(
        String::from_utf8(csv)
            .unwrap()
            .contains("self_ms_per_second")
    );
}
