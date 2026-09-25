# Standalone walkthrough

## Build, record and analyse

From the repository root, run `pwsh -File scripts/demo.ps1 -Benchmark`. Each run
uses a fresh ignored `artifacts/demo-*` directory. The sample records 120 updates,
nested scan operations, queue levels, processed-item totals, cache-hit increments
and two changes in simulated workload context. These are synthetic values, not
measurements from Ostranauts.

| File | What it contains |
|---|---|
| `capture.json` | Original recorder output, retained for reanalysis |
| `report/summary.csv` | Complete counts and inclusive timings for completed scopes |
| `report/time-series.csv` | Sparse per-operation retained-event overlap in time windows |
| `report/counters.csv` | Retained counter observations with explicit kinds and units |
| `report/context.csv` | Timestamped context observations |
| `report/trace.json` | Retained durations, counters and context in Chrome Trace JSON |
| `report/report.json` | Capture identity, metadata, limits, warnings and statistics |
| `overhead.json` | Optional microbenchmark measurements and limitations |

Summary captures deliberately omit `trace.json` and `time-series.csv`. No detailed
event history can be recovered from aggregates. Empty CSV statistic cells mean
unavailable. The operation total is inclusive; do not sum parent and child rows
into a CPU-use percentage.

## Inspect in Perfetto

Open [Perfetto](https://ui.perfetto.dev), select **Open trace file**, and select
`report/trace.json`. Expand **Phobos Scope / Recorder owner thread**. Updates contain
scan slices. Queue/processed counters appear as counter tracks; cache increments
and context changes appear as point events. Zoom into a few updates and select a
slice to inspect its name and duration. Read `report.json` for dropped/rejected
observations and incomplete-scope warnings before drawing conclusions.

Perfetto documents Chrome JSON duration, counter and metadata import, including
its requirement for properly nested duration events:
[external trace formats](https://perfetto.dev/docs/getting-started/other-formats).
No upload or sharing action is required by Phobos Scope.

For an automated import check, obtain the official
[Perfetto trace processor](https://perfetto.dev/docs/analysis/trace-processor), then
run `scripts/verify-perfetto.ps1` with its executable and a generated report
directory. It checks imported timing slices, nesting, counters and parser errors.

## Inspect in a spreadsheet

Open the CSV using UTF-8, comma delimiters and decimal points. Chart operation
names against `mean_ms` or `max_ms` from `summary.csv`. For a trend, select one
operation in `time-series.csv` and chart `window_start_ms` against
`inclusive_overlap_ms`. Windows split duration at their boundaries; they are not
exclusive utilisation. Omitted windows mean no retained overlap, which is not
proof of no work when records were lost.

Counter `gauge` values are levels, `cumulative` values are totals since the producer's
documented origin, and `increment` values are individual changes. Do not sum gauge
levels or assume a missing sample is zero. Import formula-looking labels as text;
the exporter prefixes risky text cells with an apostrophe for spreadsheet safety.

## Use the C# recorder

```csharp
var recorder = new Recorder();
var update = recorder.RegisterOperation("navigation.guidance_update", "navigation");
// Register names once, outside the hot path; retain the handles.
recorder.Start(new CaptureOptions { MaxDuration = TimeSpan.FromSeconds(30) });
using (recorder.Measure(update))
{
    GuidanceUpdate(); // Existing workload, unchanged; no await inside the scope.
}
recorder.Poll(); // Owner-thread update loop, also when no operation ran.
var stopped = recorder.Stop();
stopped!.Export("captures/new-capture.json");
```

Explicit setup/export errors belong at the console-command boundary. Catch an
export error there and offer retry against the same snapshot. Do not log on every
frame. Metric names and command keys remain stable; human display messages can be
localized by the adapter. Recording calls reject ordinary misuse without throwing
into the workload. Resource exhaustion is not a guaranteed recoverable condition.

The creating thread owns registration, Start and Stop. Scopes must start/finish
there, synchronously and in reverse order. Late scopes from old sessions and
double disposal cannot increment a new session. On save load/new game, stop with
`StopReason.WorldChange`; on orderly exit use `StopReason.ApplicationExit`.
There is no implicit export, no background timer and no save-file integration.
