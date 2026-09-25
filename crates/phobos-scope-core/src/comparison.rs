use crate::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

#[derive(Debug, Serialize)]
pub struct CaptureOverview {
    pub capture_id: String,
    pub recorder_version: String,
    pub mode: Mode,
    pub duration_ms: f64,
    pub stop_reason: String,
    pub dropped_records: u64,
    pub rejected_measurements: u64,
    pub incomplete_scopes: u128,
    pub warnings: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub contexts: BTreeMap<String, ContextOverview>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ContextOverview {
    pub first_ms: Option<f64>,
    pub last_ms: Option<f64>,
    pub observations: usize,
    pub values: Vec<String>,
    pub points: Vec<ContextPoint>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct ContextPoint {
    pub time_ms: f64,
    pub value: String,
}

pub fn overview(capture: &ValidatedCapture) -> CaptureOverview {
    let c = capture.data();
    let contexts = c
        .definitions
        .iter()
        .filter(|d| d.kind == Kind::Context)
        .map(|d| {
            let observations: Vec<_> = c.contexts.iter().filter(|s| s.metric == d.id).collect();
            let values: BTreeSet<_> = observations.iter().map(|s| s.value.clone()).collect();
            (
                d.name.clone(),
                ContextOverview {
                    first_ms: observations
                        .first()
                        .map(|s| milliseconds(s.tick, c.clock_frequency_hz)),
                    last_ms: observations
                        .last()
                        .map(|s| milliseconds(s.tick, c.clock_frequency_hz)),
                    observations: observations.len(),
                    values: values.into_iter().collect(),
                    points: observations
                        .iter()
                        .map(|s| ContextPoint {
                            time_ms: milliseconds(s.tick, c.clock_frequency_hz),
                            value: s.value.clone(),
                        })
                        .collect(),
                },
            )
        })
        .collect();
    CaptureOverview {
        capture_id: c.capture_id.clone(),
        recorder_version: c.recorder_version.clone(),
        mode: c.mode.clone(),
        duration_ms: milliseconds(c.end_tick, c.clock_frequency_hz),
        stop_reason: c.stop_reason.clone(),
        dropped_records: c.dropped_records,
        rejected_measurements: c.rejected_measurements,
        incomplete_scopes: c.aggregates.iter().map(|a| a.incomplete as u128).sum(),
        warnings: capture.warnings().iter().map(|s| s.to_string()).collect(),
        metadata: c
            .metadata
            .iter()
            .map(|m| (m.key.clone(), m.value.clone()))
            .collect(),
        contexts,
    }
}

#[derive(Debug, Serialize)]
pub struct Change {
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub difference: Option<f64>,
    pub percent: Option<f64>,
}
impl Change {
    fn new(before: Option<f64>, after: Option<f64>) -> Self {
        let difference = before.zip(after).map(|(a, b)| b - a);
        let percent = before
            .zip(difference)
            .filter(|(a, _)| *a != 0.0)
            .map(|(a, d)| d / a.abs() * 100.0);
        Self {
            before,
            after,
            difference,
            percent,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OperationComparison {
    pub name: String,
    /// matched, added, removed or incompatible_definition. Missing is never zero.
    pub status: &'static str,
    pub before: Option<OperationStats>,
    pub after: Option<OperationStats>,
    pub calls_per_second: Option<Change>,
    pub mean_ms: Option<Change>,
    pub total_ms: Option<Change>,
    pub inclusive_ms_per_second: Option<Change>,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub report_version: u32,
    pub analyser_version: &'static str,
    pub before: CaptureOverview,
    pub after: CaptureOverview,
    pub warnings: Vec<String>,
    pub operations: Vec<OperationComparison>,
    pub semantics: &'static str,
}

/// Match stable operation names, never capture-local integer IDs. Comparable definitions
/// permit descriptive deltas, not causal attribution or a statistical regression verdict.
pub fn compare(before: &ValidatedCapture, after: &ValidatedCapture) -> Comparison {
    let a = overview(before);
    let b = overview(after);
    let mut warnings = vec!["Matched measurements do not establish comparable workloads or causation. Check capture conditions before drawing conclusions.".into()];
    if a.capture_id == b.capture_id {
        warnings.push("Both captures have the same identity; this may be the same capture.".into());
    }
    if a.mode != b.mode {
        warnings.push(
            "Recording modes differ; recording overhead and retained timelines may differ.".into(),
        );
    }
    if a.recorder_version != b.recorder_version {
        warnings.push("Recorder versions differ; measurement behavior may differ.".into());
    }
    if a.duration_ms != b.duration_ms {
        warnings.push("Capture durations differ. Compare calls/second and mean cost; raw totals cover different observation periods.".into());
    }
    for (label, side) in [("Before", &a), ("After", &b)] {
        if side.duration_ms == 0.0 {
            warnings.push(format!("{label}: zero duration; rates are unavailable."));
        }
        for warning in &side.warnings {
            warnings.push(format!("{label}: {warning}"));
        }
        if side.contexts.is_empty() {
            warnings.push(format!("{label}: no workload context was recorded."));
        }
        for (name, context) in &side.contexts {
            if context.first_ms != Some(0.0) {
                warnings.push(format!("{label}: context {name} is unknown before its first observation (or entirely missing)."));
            }
            if context.values.len() > 1 {
                warnings.push(format!("{label}: context {name} changed; whole-capture rates combine multiple conditions."));
            }
        }
    }
    for key in a
        .metadata
        .keys()
        .chain(b.metadata.keys())
        .collect::<BTreeSet<_>>()
    {
        if a.metadata.get(key) != b.metadata.get(key) {
            warnings.push(format!("Metadata differs or is missing: {key}."));
        }
    }
    for key in a
        .contexts
        .keys()
        .chain(b.contexts.keys())
        .collect::<BTreeSet<_>>()
    {
        if a.contexts.get(key) != b.contexts.get(key) {
            warnings.push(format!("Observed context differs or is missing: {key}. Inspect values and timing in each report."));
        }
    }
    let aa: BTreeMap<_, _> = statistics(before)
        .into_iter()
        .map(|s| (s.name.clone(), s))
        .collect();
    let bb: BTreeMap<_, _> = statistics(after)
        .into_iter()
        .map(|s| (s.name.clone(), s))
        .collect();
    let mut operations = Vec::new();
    for name in aa.keys().chain(bb.keys()).collect::<BTreeSet<_>>() {
        let left = aa.get(name).cloned();
        let right = bb.get(name).cloned();
        let da = before.data().definitions.iter().find(|d| &d.name == name);
        let db = after.data().definitions.iter().find(|d| &d.name == name);
        let status = match (da, db) {
            (Some(x), Some(y))
                if x.kind != y.kind || x.category != y.category || x.unit != y.unit =>
            {
                "incompatible_definition"
            }
            (Some(_), Some(_)) => "matched",
            (None, _) => "added",
            (_, None) => "removed",
        };
        let mut row = OperationComparison {
            name: name.clone(),
            status,
            before: left,
            after: right,
            calls_per_second: None,
            mean_ms: None,
            total_ms: None,
            inclusive_ms_per_second: None,
        };
        if status == "matched" {
            let x = row.before.as_ref().unwrap();
            let y = row.after.as_ref().unwrap();
            row.calls_per_second = Some(Change::new(x.calls_per_second, y.calls_per_second));
            row.mean_ms = Some(Change::new(x.mean_ms, y.mean_ms));
            row.total_ms = Some(Change::new(Some(x.total_ms), Some(y.total_ms)));
            row.inclusive_ms_per_second = Some(Change::new(
                (a.duration_ms > 0.0).then(|| x.total_ms * 1000.0 / a.duration_ms),
                (b.duration_ms > 0.0).then(|| y.total_ms * 1000.0 / b.duration_ms),
            ));
        } else {
            warnings.push(format!(
                "Operation {name}: {status}; deltas are unavailable."
            ));
        }
        operations.push(row);
    }
    Comparison {
        report_version: 1,
        analyser_version: env!("CARGO_PKG_VERSION"),
        before: a,
        after: b,
        warnings,
        operations,
        semantics: "After minus before. Completed synchronous scopes, inclusive real elapsed time. Nested totals overlap; inclusive ms/second is not CPU utilization. Missing metrics are not zero. Percentage changes from zero are unavailable. No significance or causation is inferred.",
    }
}

pub fn comparison_csv(report: &Comparison, writer: impl Write) -> Result<(), Error> {
    let mut w = csv::Writer::from_writer(writer);
    let err = |e: csv::Error| Error::new("export.csv", e.to_string());
    w.write_record([
        "name",
        "status",
        "measure",
        "unit",
        "before",
        "after",
        "difference",
        "percent",
    ])
    .map_err(err)?;
    for op in &report.operations {
        for (name, unit, change) in [
            ("calls_per_second", "calls/s", &op.calls_per_second),
            ("mean", "ms/call", &op.mean_ms),
            ("total", "ms", &op.total_ms),
            ("inclusive_per_second", "ms/s", &op.inclusive_ms_per_second),
        ] {
            let mut row = vec![
                crate::export::cell(&op.name),
                op.status.into(),
                name.into(),
                unit.into(),
            ];
            for value in [
                change.as_ref().and_then(|c| c.before),
                change.as_ref().and_then(|c| c.after),
                change.as_ref().and_then(|c| c.difference),
                change.as_ref().and_then(|c| c.percent),
            ] {
                row.push(value.map(|v| v.to_string()).unwrap_or_default());
            }
            w.write_record(row).map_err(err)?;
        }
    }
    w.flush()
        .map_err(|e| Error::new("export.csv", e.to_string()))
}
