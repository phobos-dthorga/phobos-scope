use crate::*;
use std::collections::HashSet;

/// Constructed only after validation; exporters cannot accidentally accept unchecked data.
#[derive(Debug)]
pub struct ValidatedCapture {
    pub(crate) data: Capture,
}
impl ValidatedCapture {
    pub fn data(&self) -> &Capture {
        &self.data
    }
    pub fn warnings(&self) -> Vec<&'static str> {
        let mut warnings = Vec::new();
        if self.data.dropped_records > 0 {
            warnings.push("capture.dropped_records");
        }
        if self.data.rejected_measurements > 0 {
            warnings.push("capture.rejected_measurements");
        }
        if self.data.aggregates.iter().any(|a| a.incomplete > 0) {
            warnings.push("capture.incomplete_scopes");
        }
        if self.data.stop_reason != "manual" {
            warnings.push("capture.non_manual_stop");
        }
        warnings
    }
}

fn is_counter(kind: &Kind) -> bool {
    matches!(kind, Kind::Gauge | Kind::Cumulative | Kind::Increment)
}

/// Format 2 counter totals: exactly one per counter, internally consistent, and never smaller than the samples
/// retained beside them. Format 1 has none, and summary mode in format 2 retains no samples.
fn counter_totals<'a>(
    c: &'a Capture,
    definition: impl Fn(usize) -> Result<&'a Definition, Error>,
) -> Result<(), Error> {
    if c.format_version == 1 {
        return require(
            c.counter_aggregates.is_empty(),
            "Format 1 captures carry no counter_aggregates.",
        );
    }
    require(
        c.mode != Mode::Summary || c.counters.is_empty(),
        "Format 2 summary captures keep counter totals, not samples.",
    )?;
    let mut seen = std::collections::HashSet::new();
    for a in &c.counter_aggregates {
        let kind = &definition(a.metric)?.kind;
        require(
            is_counter(kind) && seen.insert(a.metric),
            "Exactly one counter aggregate is required per counter.",
        )?;
        require(
            [a.sum, a.min, a.max, a.last].iter().all(|v| v.is_finite()),
            "Counter totals must be finite.",
        )?;
        if a.samples == 0 {
            require(
                a.sum == 0.0 && a.min == 0.0 && a.max == 0.0 && a.last == 0.0,
                "A counter with no samples reports zeros, not a level.",
            )?;
        } else {
            require(
                a.min <= a.max && a.min <= a.last && a.last <= a.max,
                "Counter minimum, maximum and last value disagree.",
            )?;
        }
        if *kind == Kind::Cumulative && a.samples > 0 {
            require(
                a.min >= 0.0 && a.last == a.max,
                "A cumulative counter ends at its largest value.",
            )?;
        }
        let retained: Vec<f64> = c
            .counters
            .iter()
            .filter(|s| s.metric == a.metric)
            .map(|s| s.value)
            .collect();
        require(
            retained.len() as u64 <= a.samples
                && retained.iter().all(|v| *v >= a.min && *v <= a.max),
            "Retained counter samples exceed their complete total.",
        )?;
        if c.dropped_records == 0 && c.mode == Mode::Detailed {
            let sum: f64 = retained.iter().sum();
            require(
                retained.len() as u64 == a.samples
                    && retained.last().is_none_or(|v| *v == a.last)
                    && (sum - a.sum).abs() <= 1e-9 * a.sum.abs().max(1.0),
                "Detailed counter samples and totals disagree without dropped records.",
            )?;
        }
    }
    require(
        c.definitions.iter().filter(|d| is_counter(&d.kind)).count() == c.counter_aggregates.len(),
        "Missing counter aggregate.",
    )
}

fn require(ok: bool, detail: impl Into<String>) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::new("capture.invalid", detail))
    }
}
fn text(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_TEXT_BYTES && !value.contains('\0')
}

pub fn validate(c: Capture) -> Result<ValidatedCapture, Error> {
    require(
        SUPPORTED_FORMAT_VERSIONS.contains(&c.format_version),
        "Unsupported format_version.",
    )?;
    require(
        text(&c.capture_id) && text(&c.recorder_version),
        "Capture identity/version must be bounded nonempty text.",
    )?;
    require(
        matches!(
            c.stop_reason.as_str(),
            "manual"
                | "duration_limit"
                | "world_change"
                | "application_exit"
                | "nesting_error"
                | "clock_error"
                | "overflow"
        ),
        "Unknown stop_reason.",
    )?;
    require(
        c.clock_frequency_hz > 0 && c.clock_frequency_hz <= MAX_CLOCK_HZ,
        "Clock frequency must be 1..1,000,000,000 Hz.",
    )?;
    require(
        c.limits.max_records > 0
            && c.limits.max_records <= MAX_RECORDS
            && c.limits.max_depth > 0
            && c.limits.max_depth <= 256,
        "Recording limits exceed format bounds.",
    )?;
    require(
        c.limits.max_duration_ticks > 0
            && c.limits.max_duration_ticks <= c.clock_frequency_hz * MAX_CAPTURE_SECONDS
            && c.end_tick <= c.limits.max_duration_ticks,
        "Capture duration exceeds its declared limit.",
    )?;
    require(
        c.definitions.len() <= MAX_DEFINITIONS && c.metadata.len() <= 32,
        "Too many definitions or metadata entries.",
    )?;
    let mut names = HashSet::new();
    for (id, d) in c.definitions.iter().enumerate() {
        require(
            d.id == id && text(&d.name) && text(&d.category) && text(&d.unit),
            "Definition IDs must be contiguous; names, categories and units must be bounded text.",
        )?;
        require(names.insert(&d.name), "Metric names must be unique.")?;
        require(
            d.kind != Kind::Operation || d.unit == "ticks",
            "Operations use the ticks unit.",
        )?;
    }
    let mut keys = HashSet::new();
    for m in &c.metadata {
        require(
            text(&m.key) && text(&m.value) && keys.insert(&m.key),
            "Metadata keys must be unique and values bounded.",
        )?;
    }
    let definition = |id: usize| {
        c.definitions
            .get(id)
            .ok_or_else(|| Error::new("capture.metric", format!("Unknown metric ID {id}.")))
    };
    require(
        c.events.len() + c.counters.len() + c.contexts.len() <= c.limits.max_records,
        "Retained records exceed max_records.",
    )?;
    require(
        c.mode != Mode::Summary || c.events.is_empty(),
        "Summary captures cannot contain duration events.",
    )?;
    let mut aggregate_ids = HashSet::new();
    for a in &c.aggregates {
        require(
            definition(a.metric)?.kind == Kind::Operation && aggregate_ids.insert(a.metric),
            "Exactly one aggregate is required per operation.",
        )?;
        require(
            a.max_ticks <= c.end_tick && a.total_ticks >= a.max_ticks,
            "Invalid aggregate duration bounds.",
        )?;
        require(
            (a.calls > 0 || (a.total_ticks == 0 && a.max_ticks == 0))
                && (a.total_ticks as u128) <= (a.calls as u128) * (a.max_ticks as u128),
            "Aggregate count/total/maximum disagree.",
        )?;
    }
    require(
        c.definitions
            .iter()
            .filter(|d| d.kind == Kind::Operation)
            .count()
            == c.aggregates.len(),
        "Missing operation aggregate.",
    )?;
    let mut retained = vec![(0_u64, 0_u128, 0_u64); c.definitions.len()];
    let mut ordered: Vec<_> = c.events.iter().collect();
    ordered.sort_by_key(|e| (e.start_tick, std::cmp::Reverse(e.duration_ticks)));
    let mut stack = Vec::new();
    for e in ordered {
        require(
            definition(e.metric)?.kind == Kind::Operation,
            "Duration event must reference an operation.",
        )?;
        let end = e
            .start_tick
            .checked_add(e.duration_ticks)
            .ok_or_else(|| Error::new("capture.overflow", "Event end overflows."))?;
        require(end <= c.end_tick, "Event lies outside capture boundaries.")?;
        while stack.last().is_some_and(|end| *end <= e.start_tick) {
            stack.pop();
        }
        require(
            stack.last().is_none_or(|parent_end| end <= *parent_end),
            "Duration events overlap without nesting on the single recording thread.",
        )?;
        if e.duration_ticks > 0 {
            stack.push(end);
        }
        require(
            stack.len() <= c.limits.max_depth,
            "Event nesting exceeds max_depth.",
        )?;
        let r = &mut retained[e.metric];
        r.0 += 1;
        r.1 += e.duration_ticks as u128;
        r.2 = r.2.max(e.duration_ticks);
    }
    let mut missing_events = 0_u128;
    for a in &c.aggregates {
        let r = retained[a.metric];
        require(
            r.0 <= a.calls && r.1 <= a.total_ticks as u128 && r.2 <= a.max_ticks,
            "Retained events exceed complete aggregates.",
        )?;
        if c.mode == Mode::Detailed {
            missing_events += (a.calls - r.0) as u128;
        }
        if c.mode == Mode::Detailed && (c.dropped_records == 0 || a.calls == r.0) {
            require(
                r == (a.calls, a.total_ticks as u128, a.max_ticks),
                "Detailed events and aggregates disagree without missing duration events.",
            )?;
        }
    }
    require(
        missing_events <= c.dropped_records as u128,
        "Missing duration events exceed the declared dropped-record count.",
    )?;
    let mut previous = vec![None; c.definitions.len()];
    let mut last_tick = 0;
    for s in &c.counters {
        let kind = &definition(s.metric)?.kind;
        require(
            matches!(kind, Kind::Gauge | Kind::Cumulative | Kind::Increment)
                && s.value.is_finite()
                && s.tick <= c.end_tick
                && s.tick >= last_tick,
            "Counter kind, value or timestamp is invalid.",
        )?;
        if *kind == Kind::Cumulative {
            require(
                s.value >= 0.0 && previous[s.metric].is_none_or(|p| s.value >= p),
                "Cumulative counters cannot decrease or be negative; use a new capture for resets.",
            )?;
            previous[s.metric] = Some(s.value);
        }
        last_tick = s.tick;
    }
    counter_totals(&c, definition)?;
    last_tick = 0;
    for s in &c.contexts {
        require(
            definition(s.metric)?.kind == Kind::Context
                && s.tick <= c.end_tick
                && s.tick >= last_tick
                && text(&s.value),
            "Context kind, value or timestamp is invalid.",
        )?;
        last_tick = s.tick;
    }
    Ok(ValidatedCapture { data: c })
}
