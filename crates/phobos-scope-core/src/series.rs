//! A series joins the consecutive windows of one recording into a single timeline, so slow trends such as memory
//! growth over an hour become visible. Each window keeps its own complete aggregates; nothing is interpolated between
//! windows and time between windows (a world load, say) is not on the axis.
use crate::*;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

/// Adapter metadata that orders windows: one id per recording, and the window's number within it (1, 2, 3...).
pub const RECORDING_ID_KEY: &str = "recording_id";
pub const WINDOW_KEY: &str = "window";
/// At least this many windows with readings before a trend per hour is reported.
pub const MIN_TREND_WINDOWS: usize = 3;
pub const MAX_SERIES_CAPTURES: usize = 1_000;

#[derive(Debug, Serialize)]
pub struct SeriesWindow {
    pub position: usize,
    pub capture_id: String,
    pub recording_id: Option<String>,
    pub window: Option<u64>,
    /// Recorded seconds before this window began: the sum of earlier windows' durations, not wall-clock time.
    pub start_s: f64,
    pub duration_s: f64,
    pub stop_reason: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GaugePoint {
    pub position: usize,
    pub mid_s: f64,
    pub samples: u64,
    pub min: Option<f64>,
    pub mean: Option<f64>,
    pub max: Option<f64>,
    pub last: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct GaugeTrend {
    pub name: String,
    pub category: String,
    pub unit: String,
    pub points: Vec<GaugePoint>,
    /// Least-squares slope of each window's lowest reading against recorded time, per hour. For the heap after
    /// collection this is the plainest leak sign. Descriptive only; unavailable below the minimum window count.
    pub floor_change_per_hour: Option<f64>,
    pub first_floor: Option<f64>,
    pub last_floor: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct RatePoint {
    pub position: usize,
    pub mid_s: f64,
    pub per_second: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct IncrementTrend {
    pub name: String,
    pub category: String,
    pub unit: String,
    pub points: Vec<RatePoint>,
}

#[derive(Debug, Serialize)]
pub struct OperationPoint {
    pub position: usize,
    pub mid_s: f64,
    pub calls_per_second: Option<f64>,
    pub inclusive_ms_per_second: Option<f64>,
    pub self_ms_per_second: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct OperationTrend {
    pub name: String,
    pub category: String,
    pub points: Vec<OperationPoint>,
    /// The mean of the per-window self (or, before format 3, inclusive) ms per second, for ranking.
    pub mean_ms_per_second: f64,
    pub basis: &'static str,
}

#[derive(Debug, Serialize)]
pub struct Series {
    pub report_version: u32,
    pub analyser_version: &'static str,
    pub windows: Vec<SeriesWindow>,
    pub recorded_s: f64,
    pub gauges: Vec<GaugeTrend>,
    pub increments: Vec<IncrementTrend>,
    pub operations: Vec<OperationTrend>,
    pub warnings: Vec<String>,
    pub semantics: &'static str,
}

fn meta<'a>(c: &'a Capture, key: &str) -> Option<&'a str> {
    c.metadata
        .iter()
        .find(|m| m.key == key)
        .map(|m| m.value.as_str())
}

/// Orders windows by recording and window number when every capture carries them; otherwise keeps the given order and
/// says so. Mixed recordings are allowed but warned about: their windows are not one continuous session.
pub fn series(captures: &[ValidatedCapture]) -> Result<Series, Error> {
    if captures.is_empty() || captures.len() > MAX_SERIES_CAPTURES {
        return Err(Error::new(
            "series.count",
            format!("A series needs 1 to {MAX_SERIES_CAPTURES} captures."),
        ));
    }
    let mut warnings = vec!["Each window keeps its own complete figures. The time axis is recorded time: gaps between windows, such as world loads, are not on it. Trends are descriptive, not a verdict.".to_owned()];
    let keyed: Vec<Option<(String, u64)>> = captures
        .iter()
        .map(|c| {
            let d = c.data();
            Some((
                meta(d, RECORDING_ID_KEY)?.to_owned(),
                meta(d, WINDOW_KEY)?.parse().ok()?,
            ))
        })
        .collect();
    let mut order: Vec<usize> = (0..captures.len()).collect();
    if keyed.iter().all(Option::is_some) {
        order.sort_by(|&x, &y| keyed[x].cmp(&keyed[y]));
        let recordings: BTreeSet<_> = keyed.iter().flatten().map(|(r, _)| r.clone()).collect();
        if recordings.len() > 1 {
            warnings.push(format!("The captures come from {} recordings; windows from different recordings are not one continuous session.", recordings.len()));
        }
        for pair in order.windows(2) {
            let (a, b) = (
                keyed[pair[0]].as_ref().unwrap(),
                keyed[pair[1]].as_ref().unwrap(),
            );
            if a.0 == b.0 && b.1 != a.1 + 1 {
                warnings.push(format!(
                    "Recording {}: window {} follows window {}; windows are missing or repeated.",
                    a.0, b.1, a.1
                ));
            }
        }
    } else {
        warnings.push("Some captures carry no recording id or window number; they are kept in the order given.".into());
    }
    let mut windows = Vec::new();
    let mut start = 0.0;
    for (position, &index) in order.iter().enumerate() {
        let c = captures[index].data();
        let duration = c.end_tick as f64 / c.clock_frequency_hz as f64;
        windows.push(SeriesWindow {
            position,
            capture_id: c.capture_id.clone(),
            recording_id: keyed[index].as_ref().map(|k| k.0.clone()),
            window: keyed[index].as_ref().map(|k| k.1),
            start_s: start,
            duration_s: duration,
            stop_reason: c.stop_reason.clone(),
            warnings: captures[index]
                .warnings()
                .iter()
                .map(|w| w.to_string())
                .collect(),
        });
        start += duration;
    }
    // Every name seen in any window; a window that lacks one reports it unavailable, never zero.
    let mut gauges: BTreeMap<String, GaugeTrend> = BTreeMap::new();
    let mut increments: BTreeMap<String, IncrementTrend> = BTreeMap::new();
    let mut operations: BTreeMap<String, OperationTrend> = BTreeMap::new();
    for (position, &index) in order.iter().enumerate() {
        let capture = &captures[index];
        let w = &windows[position];
        let mid = w.start_s + w.duration_s / 2.0;
        let seconds = (w.duration_s > 0.0).then_some(w.duration_s);
        for s in counter_statistics(capture) {
            match s.kind {
                Kind::Gauge | Kind::Cumulative => {
                    let trend = gauges.entry(s.name.clone()).or_insert_with(|| GaugeTrend {
                        name: s.name.clone(),
                        category: s.category.clone(),
                        unit: s.unit.clone(),
                        points: Vec::new(),
                        floor_change_per_hour: None,
                        first_floor: None,
                        last_floor: None,
                    });
                    trend.points.push(GaugePoint {
                        position,
                        mid_s: mid,
                        samples: s.samples,
                        min: s.min,
                        mean: s.mean,
                        max: s.max,
                        last: s.last,
                    });
                }
                _ => {
                    let trend =
                        increments
                            .entry(s.name.clone())
                            .or_insert_with(|| IncrementTrend {
                                name: s.name.clone(),
                                category: s.category.clone(),
                                unit: s.unit.clone(),
                                points: Vec::new(),
                            });
                    trend.points.push(RatePoint {
                        position,
                        mid_s: mid,
                        per_second: s.sum.zip(seconds).map(|(v, t)| v / t),
                    });
                }
            }
        }
        for s in statistics(capture) {
            let trend = operations
                .entry(s.name.clone())
                .or_insert_with(|| OperationTrend {
                    name: s.name.clone(),
                    category: s.category.clone(),
                    points: Vec::new(),
                    mean_ms_per_second: 0.0,
                    basis: "self",
                });
            trend.points.push(OperationPoint {
                position,
                mid_s: mid,
                calls_per_second: s.calls_per_second,
                inclusive_ms_per_second: seconds.map(|t| s.total_ms / t),
                self_ms_per_second: s.self_ms.zip(seconds).map(|(v, t)| v / t),
            });
        }
    }
    for trend in gauges.values_mut() {
        let floors: Vec<(f64, f64)> = trend
            .points
            .iter()
            .filter_map(|p| p.min.map(|m| (p.mid_s, m)))
            .collect();
        trend.first_floor = floors.first().map(|p| p.1);
        trend.last_floor = floors.last().map(|p| p.1);
        if floors.len() >= MIN_TREND_WINDOWS {
            trend.floor_change_per_hour = slope(&floors).map(|per_second| per_second * 3_600.0);
        }
    }
    for trend in operations.values_mut() {
        let self_known = trend.points.iter().all(|p| p.self_ms_per_second.is_some());
        trend.basis = if self_known { "self" } else { "inclusive" };
        let values: Vec<f64> = trend
            .points
            .iter()
            .filter_map(|p| {
                if self_known {
                    p.self_ms_per_second
                } else {
                    p.inclusive_ms_per_second
                }
            })
            .collect();
        trend.mean_ms_per_second = if values.is_empty() {
            0.0
        } else {
            values.iter().sum::<f64>() / values.len() as f64
        };
    }
    let mut operations: Vec<_> = operations.into_values().collect();
    operations.sort_by(|a, b| {
        b.mean_ms_per_second
            .total_cmp(&a.mean_ms_per_second)
            .then(a.name.cmp(&b.name))
    });
    Ok(Series {
        report_version: 1,
        analyser_version: env!("CARGO_PKG_VERSION"),
        recorded_s: start,
        windows,
        gauges: gauges.into_values().collect(),
        increments: increments.into_values().collect(),
        operations,
        warnings,
        semantics: "One row per window, in recording order. Gauge minimum, mean, maximum and last reading are the window's own; a gauge mean is a mean of samples, not time-weighted. Increments and operations are per real second of that window. Self ms/s needs format 3. Missing values are unavailable, not zero.",
    })
}

/// Ordinary least-squares slope; none when every x is the same.
fn slope(points: &[(f64, f64)]) -> Option<f64> {
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return None;
    }
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    Some(sxy / sxx)
}

pub fn series_csv(report: &Series, writer: impl Write) -> Result<(), Error> {
    let mut w = csv::Writer::from_writer(writer);
    let err = |e: csv::Error| Error::new("export.csv", e.to_string());
    w.write_record([
        "position",
        "recording_id",
        "window",
        "start_s",
        "duration_s",
        "kind",
        "name",
        "statistic",
        "value",
        "unit",
    ])
    .map_err(err)?;
    let text = |v: Option<f64>| v.map(|v| v.to_string()).unwrap_or_default();
    let mut row = |position: usize,
                   kind: &str,
                   name: &str,
                   statistic: &str,
                   value: Option<f64>,
                   unit: &str| {
        let window = &report.windows[position];
        w.write_record([
            position.to_string(),
            window
                .recording_id
                .as_deref()
                .map(crate::export::cell)
                .unwrap_or_default(),
            window.window.map(|v| v.to_string()).unwrap_or_default(),
            window.start_s.to_string(),
            window.duration_s.to_string(),
            kind.to_owned(),
            crate::export::cell(name),
            statistic.to_owned(),
            text(value),
            crate::export::cell(unit),
        ])
    };
    for g in &report.gauges {
        for p in &g.points {
            for (statistic, value) in [
                ("min", p.min),
                ("mean", p.mean),
                ("max", p.max),
                ("last", p.last),
            ] {
                row(p.position, "gauge", &g.name, statistic, value, &g.unit).map_err(err)?;
            }
        }
    }
    for i in &report.increments {
        for p in &i.points {
            row(
                p.position,
                "increment",
                &i.name,
                "per_second",
                p.per_second,
                &format!("{}/s", i.unit),
            )
            .map_err(err)?;
        }
    }
    for o in &report.operations {
        for p in &o.points {
            row(
                p.position,
                "operation",
                &o.name,
                "calls_per_second",
                p.calls_per_second,
                "calls/s",
            )
            .map_err(err)?;
            row(
                p.position,
                "operation",
                &o.name,
                "inclusive_ms_per_second",
                p.inclusive_ms_per_second,
                "ms/s",
            )
            .map_err(err)?;
            row(
                p.position,
                "operation",
                &o.name,
                "self_ms_per_second",
                p.self_ms_per_second,
                "ms/s",
            )
            .map_err(err)?;
        }
    }
    w.flush()
        .map_err(|e| Error::new("export.csv", e.to_string()))
}
