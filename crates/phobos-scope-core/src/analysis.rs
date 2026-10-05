use crate::*;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct OperationStats {
    pub metric: usize,
    pub name: String,
    pub category: String,
    pub basis: &'static str,
    pub calls: u64,
    pub incomplete: u64,
    pub total_ms: f64,
    pub mean_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub calls_per_second: Option<f64>,
    /// Format 3: time spent in this operation outside its own measured child operations.
    pub self_ms: Option<f64>,
    pub mean_self_ms: Option<f64>,
    pub retained_calls: usize,
}

/// One counter's totals. Format 2 totals are complete; a format 1 capture only has its retained samples, and says so.
#[derive(Debug, Clone, Serialize)]
pub struct CounterStats {
    pub metric: usize,
    pub name: String,
    pub category: String,
    pub unit: String,
    pub kind: Kind,
    pub basis: &'static str,
    pub samples: u64,
    pub sum: Option<f64>,
    pub mean: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub last: Option<f64>,
    /// Increments only: the summed change per real second of capture.
    pub per_second: Option<f64>,
}

pub fn counter_statistics(capture: &ValidatedCapture) -> Vec<CounterStats> {
    let c = &capture.data;
    let seconds = (c.end_tick > 0).then(|| c.end_tick as f64 / c.clock_frequency_hz as f64);
    c.definitions
        .iter()
        .filter(|d| matches!(d.kind, Kind::Gauge | Kind::Cumulative | Kind::Increment))
        .map(|d| {
            let (basis, samples, sum, min, max, last) =
                match c.counter_aggregates.iter().find(|a| a.metric == d.id) {
                    Some(a) => ("complete_total", a.samples, a.sum, a.min, a.max, a.last),
                    None => {
                        let values: Vec<f64> = c
                            .counters
                            .iter()
                            .filter(|s| s.metric == d.id)
                            .map(|s| s.value)
                            .collect();
                        (
                            "retained_samples",
                            values.len() as u64,
                            values.iter().sum(),
                            values.iter().copied().fold(f64::INFINITY, f64::min),
                            values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                            values.last().copied().unwrap_or(0.0),
                        )
                    }
                };
            let any = samples > 0;
            CounterStats {
                metric: d.id,
                name: d.name.clone(),
                category: d.category.clone(),
                unit: d.unit.clone(),
                kind: d.kind.clone(),
                basis,
                samples,
                sum: any.then_some(sum),
                mean: any.then(|| sum / samples as f64),
                min: any.then_some(min),
                max: any.then_some(max),
                last: any.then_some(last),
                per_second: (d.kind == Kind::Increment)
                    .then_some(())
                    .and(seconds)
                    .filter(|_| any)
                    .map(|s| sum / s),
            }
        })
        .collect()
}

pub fn milliseconds(ticks: u64, hz: u64) -> f64 {
    ticks as f64 * 1_000.0 / hz as f64
}

pub fn statistics(capture: &ValidatedCapture) -> Vec<OperationStats> {
    let c = &capture.data;
    c.aggregates
        .iter()
        .map(|a| OperationStats {
            metric: a.metric,
            name: c.definitions[a.metric].name.clone(),
            category: c.definitions[a.metric].category.clone(),
            basis: "completed_scopes_inclusive",
            calls: a.calls,
            incomplete: a.incomplete,
            total_ms: milliseconds(a.total_ticks, c.clock_frequency_hz),
            mean_ms: (a.calls > 0)
                .then(|| milliseconds(a.total_ticks, c.clock_frequency_hz) / a.calls as f64),
            max_ms: (a.calls > 0).then(|| milliseconds(a.max_ticks, c.clock_frequency_hz)),
            calls_per_second: (c.end_tick > 0)
                .then(|| a.calls as f64 * c.clock_frequency_hz as f64 / c.end_tick as f64),
            self_ms: a.self_ticks.map(|t| milliseconds(t, c.clock_frequency_hz)),
            mean_self_ms: a
                .self_ticks
                .filter(|_| a.calls > 0)
                .map(|t| milliseconds(t, c.clock_frequency_hz) / a.calls as f64),
            retained_calls: c.events.iter().filter(|e| e.metric == a.metric).count(),
        })
        .collect()
}

/// Format 3: the share of the capture's real time spent inside measured operations on the recording thread. Self times
/// never overlap there, so they add up; this is elapsed time, not CPU use, and unmeasured work is not in it.
pub fn measured_share(capture: &ValidatedCapture) -> Option<f64> {
    let c = &capture.data;
    if c.end_tick == 0 || c.aggregates.iter().any(|a| a.self_ticks.is_none()) {
        return None;
    }
    let total: u128 = c
        .aggregates
        .iter()
        .map(|a| a.self_ticks.unwrap_or(0) as u128)
        .sum();
    Some(total as f64 / c.end_tick as f64)
}

#[derive(Debug, Serialize)]
pub struct Window {
    pub start_tick: u64,
    pub end_tick: u64,
    pub metric: usize,
    pub name: String,
    pub basis: &'static str,
    pub calls_started: u64,
    pub inclusive_overlap_ms: f64,
}

/// Sparse windows: omitted rows mean no retained event overlap, not proof of no work.
pub fn time_windows(capture: &ValidatedCapture, width_ticks: u64) -> Result<Vec<Window>, Error> {
    let c = &capture.data;
    if c.mode != Mode::Detailed {
        return Err(Error::new(
            "analysis.unavailable",
            "Time windows require a detailed capture.",
        ));
    }
    if width_ticks == 0 || c.end_tick.div_ceil(width_ticks) > 10_000 {
        return Err(Error::new(
            "analysis.windows",
            "Use a positive width with at most 10,000 windows.",
        ));
    }
    let mut bins = BTreeMap::<(u64, usize), (u64, u128)>::new();
    // Limit work even for deeply nested long events at very narrow window widths.
    let mut contributions = 0_u64;
    for e in &c.events {
        let end = e.start_tick + e.duration_ticks;
        let first = e.start_tick / width_ticks;
        let last = if e.duration_ticks == 0 {
            first
        } else {
            (end - 1) / width_ticks
        };
        for index in first..=last {
            contributions += 1;
            if contributions > 2_000_000 {
                return Err(Error::new(
                    "analysis.windows",
                    "Too many event/window intersections; increase window width.",
                ));
            }
            let start = index * width_ticks;
            let stop = start.saturating_add(width_ticks).min(c.end_tick);
            let row = bins.entry((start, e.metric)).or_default();
            if index == first {
                row.0 += 1;
            }
            row.1 += stop.min(end).saturating_sub(start.max(e.start_tick)) as u128;
        }
    }
    Ok(bins
        .into_iter()
        .map(|((start, metric), (calls, ticks))| Window {
            start_tick: start,
            end_tick: start.saturating_add(width_ticks).min(c.end_tick),
            metric,
            name: c.definitions[metric].name.clone(),
            basis: "retained_events_inclusive",
            calls_started: calls,
            inclusive_overlap_ms: ticks as f64 * 1_000.0 / c.clock_frequency_hz as f64,
        })
        .collect())
}
