use crate::*;
use serde_json::{Value, json};
use std::io::Write;

fn csv_error(e: impl std::fmt::Display) -> Error {
    Error::new("export.csv", e.to_string())
}
// Preserve source names in captures/JSON; neutralize formula-like labels for spreadsheets.
pub(crate) fn cell(text: &str) -> String {
    if text.trim_start().starts_with(['=', '+', '-', '@']) || text.starts_with(['\t', '\r', '\n']) {
        format!("'{text}")
    } else {
        text.to_owned()
    }
}

pub fn summary_csv(capture: &ValidatedCapture, writer: impl Write) -> Result<(), Error> {
    let mut w = csv::Writer::from_writer(writer);
    w.write_record([
        "name",
        "category",
        "basis",
        "calls",
        "incomplete",
        "total_ms",
        "mean_ms",
        "max_ms",
        "calls_per_second",
        "retained_calls",
        "dropped_records",
        "rejected_measurements",
    ])
    .map_err(csv_error)?;
    for s in statistics(capture) {
        w.write_record([
            cell(&s.name),
            cell(&s.category),
            s.basis.to_owned(),
            s.calls.to_string(),
            s.incomplete.to_string(),
            s.total_ms.to_string(),
            s.mean_ms.map(|v| v.to_string()).unwrap_or_default(),
            s.max_ms.map(|v| v.to_string()).unwrap_or_default(),
            s.calls_per_second
                .map(|v| v.to_string())
                .unwrap_or_default(),
            s.retained_calls.to_string(),
            capture.data.dropped_records.to_string(),
            capture.data.rejected_measurements.to_string(),
        ])
        .map_err(csv_error)?;
    }
    w.flush().map_err(csv_error)
}

pub fn windows_csv(
    capture: &ValidatedCapture,
    width_ticks: u64,
    writer: impl Write,
) -> Result<(), Error> {
    let rows = time_windows(capture, width_ticks)?;
    let mut w = csv::Writer::from_writer(writer);
    w.write_record([
        "window_start_ms",
        "window_end_ms",
        "name",
        "basis",
        "calls_started",
        "inclusive_overlap_ms",
        "dropped_records",
    ])
    .map_err(csv_error)?;
    for row in rows {
        w.write_record([
            milliseconds(row.start_tick, capture.data.clock_frequency_hz).to_string(),
            milliseconds(row.end_tick, capture.data.clock_frequency_hz).to_string(),
            cell(&row.name),
            row.basis.to_owned(),
            row.calls_started.to_string(),
            row.inclusive_overlap_ms.to_string(),
            capture.data.dropped_records.to_string(),
        ])
        .map_err(csv_error)?;
    }
    w.flush().map_err(csv_error)
}

pub fn counters_csv(capture: &ValidatedCapture, writer: impl Write) -> Result<(), Error> {
    let mut w = csv::Writer::from_writer(writer);
    w.write_record([
        "time_ms",
        "name",
        "kind",
        "unit",
        "value",
        "basis",
        "dropped_records",
    ])
    .map_err(csv_error)?;
    for s in &capture.data.counters {
        let d = &capture.data.definitions[s.metric];
        let kind = match d.kind {
            Kind::Gauge => "gauge",
            Kind::Cumulative => "cumulative",
            _ => "increment",
        };
        w.write_record([
            milliseconds(s.tick, capture.data.clock_frequency_hz).to_string(),
            cell(&d.name),
            kind.to_owned(),
            cell(&d.unit),
            s.value.to_string(),
            "retained_samples".to_owned(),
            capture.data.dropped_records.to_string(),
        ])
        .map_err(csv_error)?;
    }
    w.flush().map_err(csv_error)
}

pub fn contexts_csv(capture: &ValidatedCapture, writer: impl Write) -> Result<(), Error> {
    let mut w = csv::Writer::from_writer(writer);
    w.write_record(["time_ms", "name", "value", "dropped_records"])
        .map_err(csv_error)?;
    for s in &capture.data.contexts {
        w.write_record([
            milliseconds(s.tick, capture.data.clock_frequency_hz).to_string(),
            cell(&capture.data.definitions[s.metric].name),
            cell(&s.value),
            capture.data.dropped_records.to_string(),
        ])
        .map_err(csv_error)?;
    }
    w.flush().map_err(csv_error)
}

pub fn trace_json(capture: &ValidatedCapture, writer: impl Write) -> Result<(), Error> {
    let c = &capture.data;
    if c.mode != Mode::Detailed {
        return Err(Error::new(
            "export.unavailable",
            "Summary captures do not contain a timeline.",
        ));
    }
    let micros = |ticks: u64| ticks as f64 * 1_000_000.0 / c.clock_frequency_hz as f64;
    let mut events: Vec<Value> = vec![
        json!({"ph":"M","name":"process_name","pid":1,"args":{"name":"Phobos Scope"}}),
        json!({"ph":"M","name":"thread_name","pid":1,"tid":1,"args":{"name":"Recorder owner thread"}}),
    ];
    let mut durations: Vec<_> = c.events.iter().collect();
    durations.sort_by_key(|e| (e.start_tick, std::cmp::Reverse(e.duration_ticks)));
    for e in durations {
        let d = &c.definitions[e.metric];
        events.push(json!({"ph":"X","name":d.name,"cat":d.category,"pid":1,"tid":1,"ts":micros(e.start_tick),"dur":micros(e.duration_ticks),"args":{"basis":"retained_inclusive_elapsed"}}));
    }
    for s in &c.counters {
        let d = &c.definitions[s.metric];
        // Increments are point observations, never misrepresented as a gauge level.
        if d.kind == Kind::Increment {
            events.push(json!({"ph":"i","s":"t","name":d.name,"cat":d.category,"pid":1,"tid":1,"ts":micros(s.tick),"args":{"increment":s.value,"unit":d.unit}}));
        } else {
            events.push(json!({"ph":"C","name":format!("{} [{}]",d.name,d.unit),"cat":d.category,"pid":1,"tid":1,"ts":micros(s.tick),"args":{"value":s.value}}));
        }
    }
    for s in &c.contexts {
        let d = &c.definitions[s.metric];
        events.push(json!({"ph":"i","s":"t","name":d.name,"cat":d.category,"pid":1,"tid":1,"ts":micros(s.tick),"args":{"value":s.value}}));
    }
    serde_json::to_writer_pretty(writer, &json!({"traceEvents":events,"displayTimeUnit":"ms","phobos_scope":{"capture_id":c.capture_id,"dropped_records":c.dropped_records,"rejected_measurements":c.rejected_measurements,"warnings":capture.warnings(),"basis":"retained_events_inclusive","end_tick":c.end_tick,"clock_frequency_hz":c.clock_frequency_hz}})).map_err(|e| Error::new("export.json", e.to_string()))
}
