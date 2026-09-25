use crate::*;
use std::{fmt::Write as _, io::Write};

// All capture-owned strings enter HTML only through this text/attribute escape.
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn number(value: Option<f64>) -> String {
    value
        .map(|v| {
            if v != 0.0 && v.abs() < 0.001 {
                format!("{v:.3e}")
            } else {
                format!("{v:.3}")
            }
        })
        .unwrap_or_else(|| "—".into())
}
fn begin(title: &str, subtitle: &str) -> String {
    let mut html = String::from(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; base-uri 'none'; form-action 'none'"><title>"##,
    );
    html.push_str(&escape(title));
    html.push_str(r##"</title><style>"##);
    html.push_str(include_str!("report.css"));
    html.push_str(r##"</style></head><body><main><header><div class="eyebrow">PHOBOS SCOPE · OFFLINE REPORT</div>"##);
    write!(
        html,
        "<h1>{}</h1><p>{}</p></header>",
        escape(title),
        escape(subtitle)
    )
    .unwrap();
    html
}
fn finish(mut html: String, writer: impl Write) -> Result<(), Error> {
    write!(html,"<footer>Phobos Scope {} · Self-contained report. No scripts, remote fonts or network requests. Source captures remain unchanged.</footer></main></body></html>",env!("CARGO_PKG_VERSION")).unwrap();
    let mut writer = writer;
    writer
        .write_all(html.as_bytes())
        .map_err(|e| Error::new("export.html", e.to_string()))
}
fn warnings(html: &mut String, messages: &[String]) {
    html.push_str("<div class=\"warning\"><strong>Read before interpreting</strong><p>Inclusive real elapsed time includes nested calls and waits. Parent and child totals overlap; these measurements are not exclusive CPU usage. — means unavailable, never an assumed zero.</p>");
    if messages.is_empty() {
        html.push_str("<p>No recorded quality warnings. This does not establish instrumentation coverage or comparable workloads.</p>");
    } else {
        html.push_str("<ul>");
        for message in messages {
            let readable = if let Some((side, code)) = message.split_once(": ") {
                format!("{side}: {}", quality_message(code))
            } else {
                quality_message(message).to_owned()
            };
            write!(html, "<li>{}</li>", escape(&readable)).unwrap();
        }
        html.push_str("</ul>");
    }
    html.push_str("</div>");
}
fn evidence(html: &mut String, c: &CaptureOverview) {
    write!(html,"<p>Capture <code>{}</code><br>Recorder {} · {:?} · stop: {}<br>{} ms · {} dropped · {} rejected · {} incomplete</p>",escape(&c.capture_id),escape(&c.recorder_version),c.mode,escape(&c.stop_reason),number(Some(c.duration_ms)),c.dropped_records,c.rejected_measurements,c.incomplete_scopes).unwrap();
    html.push_str("<details><summary>Build and workload metadata</summary><table><tbody>");
    for (key, value) in &c.metadata {
        write!(
            html,
            "<tr><td>{}</td><td class=\"text\">{}</td></tr>",
            escape(key),
            escape(value)
        )
        .unwrap();
    }
    if c.metadata.is_empty() {
        html.push_str("<tr><td>No metadata recorded</td></tr>");
    }
    html.push_str("</tbody></table></details><details><summary>Observed workload context</summary><p>Values are observed changes, not continuous monitoring. Context is unknown before the first observation. Record loss can hide later changes.</p>");
    if c.contexts.is_empty() {
        html.push_str("<p>No context recorded.</p>");
    }
    for (name, ctx) in &c.contexts {
        write!(html,"<details><summary>{} · {} observations</summary><div class=\"scroll\"><table><thead><tr><th>Value</th><th>Real time (ms)</th></tr></thead><tbody>",escape(name),ctx.observations).unwrap();
        for p in &ctx.points {
            write!(
                html,
                "<tr><td>{}</td><td>{}</td></tr>",
                escape(&p.value),
                number(Some(p.time_ms))
            )
            .unwrap();
        }
        html.push_str("</tbody></table></div></details>");
    }
    html.push_str("</details>");
}

pub fn report_html(capture: &ValidatedCapture, writer: impl Write) -> Result<(), Error> {
    let c = capture.data();
    let info = overview(capture);
    let mut html = begin(
        "Capture report",
        "Explore operation cost, call frequency and the conditions recorded alongside them.",
    );
    write!(html,"<div class=\"cards\"><div class=\"card\"><strong>{}</strong><span>real seconds</span></div><div class=\"card\"><strong>{}</strong><span>operation definitions</span></div><div class=\"card\"><strong>{}</strong><span>retained duration events</span></div><div class=\"card\"><strong>{}</strong><span>dropped records</span></div></div>",number(Some(info.duration_ms/1000.0)),c.aggregates.len(),c.events.len(),c.dropped_records).unwrap();
    let quality: Vec<_> = info
        .warnings
        .iter()
        .map(|w| quality_message(w).to_owned())
        .collect();
    warnings(&mut html, &quality);
    html.push_str("<h2>Operation cost</h2><p>Ordered by total inclusive time. Bars compare operations, not CPU percentages. Calls and durations use all completed accepted scopes, even when retained events were dropped.</p><div class=\"scroll\"><table><thead><tr><th>Operation / category</th><th>Calls</th><th>Calls/s</th><th>Total ms</th><th>Mean ms/call</th><th>Max ms</th><th>Incomplete</th><th>Retained calls</th></tr></thead><tbody>");
    let mut stats = statistics(capture);
    stats.sort_by(|a, b| b.total_ms.total_cmp(&a.total_ms).then(a.name.cmp(&b.name)));
    let largest = stats.first().map(|s| s.total_ms).unwrap_or(0.0);
    for s in stats {
        let width = if largest > 0.0 {
            s.total_ms / largest * 100.0
        } else {
            0.0
        };
        write!(html,"<tr><td>{}<small>{}</small><div class=\"bar\" style=\"width:{width:.3}%\"></div></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",escape(&s.name),escape(&s.category),s.calls,number(s.calls_per_second),number(Some(s.total_ms)),number(s.mean_ms),number(s.max_ms),s.incomplete,s.retained_calls).unwrap();
    }
    html.push_str("</tbody></table></div><h2>Retained timeline</h2>");
    if c.mode == Mode::Summary {
        html.push_str("<p>Unavailable: summary mode does not record duration events. No timeline is inferred from aggregates.</p>");
    } else if c.events.is_empty() {
        html.push_str("<p>No duration events retained. This is not proof of no work.</p>");
    } else {
        timeline(&mut html, capture);
    }
    html.push_str("<h2>Capture conditions</h2>");
    evidence(&mut html, &info);
    html.push_str("<details><summary>Retained counter observations</summary><p>Increments are individual changes; gauges are levels; cumulative values are running totals. No interpolation or missing-as-zero conversion is applied.</p><div class=\"scroll\"><table><thead><tr><th>Metric</th><th>Time ms</th><th>Kind</th><th>Value</th><th>Unit</th></tr></thead><tbody>");
    for s in &c.counters {
        let d = &c.definitions[s.metric];
        write!(
            html,
            "<tr><td>{}</td><td>{}</td><td>{:?}</td><td>{}</td><td>{}</td></tr>",
            escape(&d.name),
            number(Some(milliseconds(s.tick, c.clock_frequency_hz))),
            d.kind,
            number(Some(s.value)),
            escape(&d.unit)
        )
        .unwrap();
    }
    html.push_str("</tbody></table></div></details>");
    finish(html, writer)
}

fn quality_message(key: &str) -> &str {
    match key {
        "capture.dropped_records" => {
            "Records were dropped. Completed-operation aggregates survive, but the timeline, counters and context are partial."
        }
        "capture.incomplete_scopes" => {
            "Some scopes did not complete. Their durations are unavailable and excluded from completed totals."
        }
        "capture.rejected_measurements" => {
            "Some measurements were rejected. Recorded aggregates do not cover those attempted measurements."
        }
        "capture.non_manual_stop" => {
            "The capture stopped automatically or at a lifecycle boundary. Check its stop reason."
        }
        _ => key,
    }
}

fn timeline(html: &mut String, capture: &ValidatedCapture) {
    let c = capture.data();
    let mut events: Vec<_> = c.events.iter().collect();
    events.sort_by_key(|e| (e.start_tick, std::cmp::Reverse(e.duration_ticks)));
    let mut stack = Vec::new();
    let mut placed = Vec::new();
    let mut max_depth = 0;
    for e in events {
        while stack.last().is_some_and(|end| *end <= e.start_tick) {
            stack.pop();
        }
        let depth = stack.len();
        max_depth = max_depth.max(depth);
        placed.push((e, depth));
        if e.duration_ticks > 0 {
            stack.push(e.start_tick + e.duration_ticks);
        }
    }
    html.push_str("<p>Real time since capture start. Rows show nesting among retained events, not OS threads or exclusive work. Missing parents can change apparent depth. Hover a bar for exact timing; very short/zero durations use a one-pixel marker. For zooming, open trace.json in Perfetto.</p><div class=\"scroll\">");
    let height = (max_depth + 1) * 25 + 45;
    write!(html,"<svg xmlns=\"http://www.w3.org/2000/svg\" role=\"img\" aria-label=\"Retained inclusive operation timeline\" viewBox=\"0 0 1040 {height}\"><title>Retained inclusive operation timeline</title>").unwrap();
    let denominator = c.end_tick.max(1) as f64;
    for i in 0..=4 {
        let x = 20 + i * 250;
        write!(
            html,
            "<text x=\"{x}\" y=\"18\" text-anchor=\"{}\">{} ms</text>",
            if i == 4 { "end" } else { "start" },
            number(Some(
                milliseconds(c.end_tick, c.clock_frequency_hz) * i as f64 / 4.0
            ))
        )
        .unwrap();
    }
    const COLORS: [&str; 6] = [
        "#71d8bd", "#7bb9ed", "#d7b1f0", "#efc276", "#f09696", "#adc96e",
    ];
    for (e, depth) in placed {
        let x = 20.0 + e.start_tick as f64 / denominator * 1000.0;
        let width = (e.duration_ticks as f64 / denominator * 1000.0).max(1.0);
        write!(html,"<rect x=\"{x:.4}\" y=\"{}\" width=\"{width:.4}\" height=\"20\" rx=\"2\" fill=\"{}\"><title>{} · start {} ms · duration {} ms</title></rect>",30+depth*25,COLORS[e.metric%COLORS.len()],escape(&c.definitions[e.metric].name),number(Some(milliseconds(e.start_tick,c.clock_frequency_hz))),number(Some(milliseconds(e.duration_ticks,c.clock_frequency_hz)))).unwrap();
    }
    html.push_str("</svg></div><details><summary>Timeline legend and accessible event list</summary><div class=\"scroll\"><table><thead><tr><th>Operation</th><th>Start ms</th><th>Duration ms</th></tr></thead><tbody>");
    for e in &c.events {
        write!(
            html,
            "<tr><td><span style=\"color:{}\">●</span> {}</td><td>{}</td><td>{}</td></tr>",
            COLORS[e.metric % COLORS.len()],
            escape(&c.definitions[e.metric].name),
            number(Some(milliseconds(e.start_tick, c.clock_frequency_hz))),
            number(Some(milliseconds(e.duration_ticks, c.clock_frequency_hz)))
        )
        .unwrap();
    }
    html.push_str("</tbody></table></div></details>");
}

pub fn comparison_html(report: &Comparison, writer: impl Write) -> Result<(), Error> {
    let mut html = begin(
        "Before / after",
        "Separate changes in call frequency from changes in per-call cost. All differences are after minus before.",
    );
    warnings(&mut html, &report.warnings);
    html.push_str("<h2>Operation changes</h2><p>Compare calls/s for workload frequency and mean ms/call for cost. Inclusive ms/s describes elapsed work per real second, not CPU utilization. Raw totals cover each capture's own duration. Percent change is unavailable when the before value is zero. No statistical regression verdict is inferred.</p>");
    for op in &report.operations {
        write!(
            html,
            "<details open><summary>{} · {}</summary>",
            escape(&op.name),
            escape(op.status)
        )
        .unwrap();
        if op.status != "matched" {
            html.push_str("<p>Deltas unavailable: missing or incompatible definitions are not zero activity.</p>");
            for (label, side) in [("Before", &op.before), ("After", &op.after)] {
                if let Some(s) = side {
                    write!(
                        html,
                        "<p>{label}: {} calls · {} total ms · {} mean ms/call · {} calls/s</p>",
                        s.calls,
                        number(Some(s.total_ms)),
                        number(s.mean_ms),
                        number(s.calls_per_second)
                    )
                    .unwrap();
                } else {
                    write!(html, "<p>{label}: operation unavailable.</p>").unwrap();
                }
            }
        } else {
            html.push_str("<div class=\"scroll\"><table><thead><tr><th>Measure</th><th>Before</th><th>After</th><th>Difference</th><th>Change %</th></tr></thead><tbody>");
            for (label, change) in [
                ("Frequency · calls/s", &op.calls_per_second),
                ("Mean cost · ms/call", &op.mean_ms),
                ("Inclusive elapsed · ms/s", &op.inclusive_ms_per_second),
                ("Raw total · ms", &op.total_ms),
            ] {
                if let Some(c) = change {
                    write!(
                        html,
                        "<tr><td>{label}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                        number(c.before),
                        number(c.after),
                        number(c.difference),
                        number(c.percent)
                    )
                    .unwrap();
                }
            }
            html.push_str("</tbody></table></div>");
            let a = op.before.as_ref().unwrap();
            let b = op.after.as_ref().unwrap();
            write!(
                html,
                "<p>Completed calls: {} → {} · Incomplete: {} → {} · Maximum ms: {} → {}</p>",
                a.calls,
                b.calls,
                a.incomplete,
                b.incomplete,
                number(a.max_ms),
                number(b.max_ms)
            )
            .unwrap();
        }
        html.push_str("</details>");
    }
    html.push_str("<h2>Comparison evidence</h2><div class=\"pair\"><section><h3>Before</h3>");
    evidence(&mut html, &report.before);
    html.push_str("</section><section><h3>After</h3>");
    evidence(&mut html, &report.after);
    html.push_str("</section></div><p>");
    html.push_str(&escape(report.semantics));
    html.push_str("</p>");
    finish(html, writer)
}
