use phobos_scope_core::*;
use serde_json::Value;

fn fixture() -> Capture {
    serde_json::from_str(include_str!("../../../fixtures/known-detailed.json")).unwrap()
}

#[test]
fn known_statistics_are_inclusive_and_rates_use_real_time() {
    let c = validate(fixture()).unwrap();
    let s = statistics(&c);
    assert_eq!(s[0].calls, 1);
    assert_eq!(s[0].total_ms, 10.0);
    assert_eq!(s[0].mean_ms, Some(10.0));
    assert_eq!(s[0].calls_per_second, Some(50.0));
    assert_eq!(s[1].total_ms, 3.0);
    assert_eq!(s[0].basis, "completed_scopes_inclusive");
}

#[test]
fn incomplete_and_no_observations_remain_unavailable() {
    let mut c = fixture();
    c.events.clear();
    c.mode = Mode::Summary;
    c.end_tick = 0;
    c.counters.clear();
    c.contexts.clear();
    for a in &mut c.aggregates {
        a.calls = 0;
        a.total_ticks = 0;
        a.max_ticks = 0;
        a.incomplete = 1;
    }
    let c = validate(c).unwrap();
    let s = statistics(&c);
    assert!(s[0].mean_ms.is_none() && s[0].max_ms.is_none() && s[0].calls_per_second.is_none());
    assert!(c.warnings().contains(&"capture.incomplete_scopes"));
}

#[test]
fn unsupported_and_truncated_input_are_actionable() {
    assert_eq!(
        read_capture(br#"{"format_version":7}"#.as_slice())
            .unwrap_err()
            .code,
        "capture.version"
    );
    assert_eq!(
        read_capture(br#"{"format_version":1,"events":["#.as_slice())
            .unwrap_err()
            .code,
        "capture.json"
    );
    assert_eq!(
        read_capture(br#"{"format_version":1}"#.as_slice())
            .unwrap_err()
            .code,
        "capture.schema"
    );
}

#[test]
fn duplicate_json_keys_and_underreported_drops_are_rejected() {
    let source = include_str!("../../../fixtures/known-detailed.json");
    let duplicate = source.replace("\"end_tick\": 20", "\"end_tick\": 20, \"end_tick\": 20");
    assert_eq!(
        read_capture(duplicate.as_bytes()).unwrap_err().code,
        "capture.schema"
    );
    let mut c = fixture();
    c.events.clear();
    c.dropped_records = 1;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.dropped_records = 1;
    c.aggregates[0].total_ticks = 11;
    c.aggregates[0].max_ticks = 11;
    assert!(validate(c).is_err());
}

#[test]
fn bounds_and_aggregate_inconsistency_are_rejected() {
    let mut c = fixture();
    c.events[0].metric = 999;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.events[0].duration_ticks = u64::MAX;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.aggregates[0].calls = 0;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.limits.max_records = 1;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.clock_frequency_hz = 0;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.definitions[1].name = c.definitions[0].name.clone();
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.aggregates.pop();
    assert!(validate(c).is_err());
}

#[test]
fn dropped_event_totals_are_not_recomputed_from_retained_samples() {
    let mut c = fixture();
    c.events.remove(0);
    c.dropped_records = 1;
    let c = validate(c).unwrap();
    let s = statistics(&c);
    assert_eq!(s[1].total_ms, 3.0);
    assert_eq!(s[1].retained_calls, 0);
    assert!(c.warnings().contains(&"capture.dropped_records"));
    let mut c = fixture();
    c.events.remove(0);
    assert!(validate(c).is_err());
}

#[test]
fn non_nested_intersections_are_rejected_but_touching_events_are_valid() {
    let mut c = fixture();
    c.events[0].start_tick = 9;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.events[0].start_tick = 10;
    assert!(validate(c).is_ok());
}

#[test]
fn windows_split_overlap_and_count_each_call_once() {
    let c = validate(fixture()).unwrap();
    let windows = time_windows(&c, 4).unwrap();
    let outer: Vec<_> = windows.iter().filter(|w| w.metric == 0).collect();
    assert_eq!(
        outer
            .iter()
            .map(|w| w.inclusive_overlap_ms)
            .collect::<Vec<_>>(),
        vec![4.0, 4.0, 2.0]
    );
    assert_eq!(outer.iter().map(|w| w.calls_started).sum::<u64>(), 1);
    assert!(time_windows(&c, 0).is_err());
    assert_eq!(time_windows(&c, u64::MAX).unwrap()[0].end_tick, 20);
}

#[test]
fn summary_cannot_create_an_invented_timeline() {
    let mut c = fixture();
    c.mode = Mode::Summary;
    assert!(validate(c.clone()).is_err());
    c.events.clear();
    let c = validate(c).unwrap();
    assert_eq!(
        trace_json(&c, Vec::new()).unwrap_err().code,
        "export.unavailable"
    );
    assert_eq!(
        time_windows(&c, 10).unwrap_err().code,
        "analysis.unavailable"
    );
}

#[test]
fn high_frequency_clock_exports_correct_microseconds() {
    let mut c = fixture();
    c.clock_frequency_hz = 1_000_000_000;
    c.end_tick = 20_000_000_000;
    c.limits.max_duration_ticks = 30_000_000_000;
    c.events[0].start_tick = 2_000_000_000;
    c.events[0].duration_ticks = 3;
    c.events[1].duration_ticks = 10_000_000_000;
    c.aggregates[0].total_ticks = 10_000_000_000;
    c.aggregates[0].max_ticks = 10_000_000_000;
    let c = validate(c).unwrap();
    let mut out = Vec::new();
    trace_json(&c, &mut out).unwrap();
    let trace: Value = serde_json::from_slice(&out).unwrap();
    let inner = trace["traceEvents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "fixture.inner")
        .unwrap();
    assert_eq!(inner["ts"], 2_000_000.0);
    assert_eq!(inner["dur"], 0.003);
}

#[test]
fn csv_escapes_names_and_protects_spreadsheet_formula_cells() {
    let mut c = fixture();
    c.definitions[0].name = "=formula,\"quoted\"\nline".into();
    let c = validate(c).unwrap();
    let mut out = Vec::new();
    summary_csv(&c, &mut out).unwrap();
    let mut reader = csv::Reader::from_reader(out.as_slice());
    let row = reader.records().next().unwrap().unwrap();
    assert_eq!(&row[0], "'=formula,\"quoted\"\nline");
    assert_eq!(&row[5], "10");
}

#[test]
fn counter_types_and_context_time_are_explicit() {
    let mut c = fixture();
    c.definitions[2].kind = Kind::Cumulative;
    c.counters.push(Sample {
        metric: 2,
        tick: 6,
        value: 6.0,
    });
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.counters[0].value = f64::NAN;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.contexts[1].tick = 21;
    assert!(validate(c).is_err());
    let mut c = fixture();
    c.definitions[2].kind = Kind::Increment;
    let c = validate(c).unwrap();
    let mut out = Vec::new();
    trace_json(&c, &mut out).unwrap();
    let trace: Value = serde_json::from_slice(&out).unwrap();
    let increment = trace["traceEvents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "fixture.queue")
        .unwrap();
    assert_eq!(increment["ph"], "i");
    assert_eq!(increment["args"]["increment"], 7.0);
}

#[test]
fn write_failures_are_reported_without_consuming_validated_data() {
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
    assert!(summary_csv(&c, Fails).is_err());
    assert!(trace_json(&c, Fails).is_err());
    assert!(summary_csv(&c, Vec::new()).is_ok());
}
