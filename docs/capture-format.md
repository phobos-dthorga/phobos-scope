# Phobos Scope capture format v3

Status: implemented standalone contract. Recorder 0.3.0 writes format 3; analyser
0.4.0 reads formats 1, 2 and 3. Format 2 added `counter_aggregates` and stopped
summary mode from retaining counter samples (see [Format 2 changes](#format-2-changes)).
Format 3 adds `self_ticks` to every operation aggregate (see
[Format 3 changes](#format-3-changes)). Everything else is unchanged from format 1. Encoding: UTF-8 JSON, one complete object.
See [`fixtures/known-detailed.json`](../fixtures/known-detailed.json). The public
Rust model and validator are the executable schema. Unknown fields, unsupported
versions, malformed/truncated JSON and inconsistent data are errors. A stopped
capture with declared drops/incomplete scopes is valid but carries quality warnings.

## Envelope

| Key | Meaning |
|---|---|
| `format_version` | Integer `3` (or `1` and `2` for older captures); incompatible changes require another version |
| `recorder_version` | Bounded nonempty recorder identity/version |
| `capture_id` | Unique bounded identity; C# uses a random GUID, with no personal path |
| `mode` | `summary` or `detailed` |
| `clock_frequency_hz` | Integer ticks per real second, 1 through 1,000,000,000 |
| `end_tick` | Capture end relative to start tick **zero** |
| `stop_reason` | `manual`, `duration_limit`, `world_change`, `application_exit`, `nesting_error`, `clock_error`, or `overflow` |
| `limits` | `max_records`, `max_depth`, `max_duration_ticks` |
| `definitions` | Stable metric definitions, indexed by contiguous integer `id` |
| `aggregates` | Exactly one complete aggregate for every operation definition |
| `events` | Retained completed durations; empty in summary mode |
| `counters` | Retained timestamped numeric samples; detailed mode only in format 2 |
| `counter_aggregates` | Format 2: exactly one complete total for every counter definition |
| `contexts` | Retained timestamped string observations |
| `metadata` | At most 32 `{key, value}` pairs; unique keys |
| `dropped_records` | Records rejected because the retained-record buffer was full |
| `rejected_measurements` | Unsupported/invalid instrumentation calls and depth-limit rejections |

Integers are nonnegative. Clock source values are subtracted **as integers in the
recorder**, before conversion or serialization. Absolute monotonic timestamps and
simulation time are not in the contract. Thus a source clock above 2^53 can still
yield exact short relative durations. The analyser converts relative ticks to
floating-point milliseconds/microseconds only for reporting.

The maximum recording duration is 3,600 seconds; maximum retained records 20,000;
maximum stack depth and metric definitions 256 each. Default C# limits are 60
seconds, 20,000 records and depth 64. All labels/metadata/context values are
nonempty UTF-8 text up to 512 bytes, excluding NUL. The analyser reads at most
64 MiB per file and bounds time-window expansion separately. Duration events,
counters and context share the same record limit. Operation aggregates and the
scope stack are bounded separately by definition and depth limits.

## Metric definitions and observations

Each definition has `id`, unique `name`, `category`, `unit`, `kind`. Names are stable
machine keys, not localized strings or per-object labels. Kinds:

- `operation`: unit `ticks`. Aggregate fields: `metric`, `calls`, `total_ticks`,
  `max_ticks`, `incomplete`. Calls are completed scopes only; incomplete scopes
  contribute neither a fabricated duration nor a completed call.
- `gauge`: a current level, e.g. items in a queue. Samples have `metric`, `tick`,
  `value`, and the definition declares units.
- `cumulative`: a nonnegative running total. Values cannot decrease within a
  capture. Reset by starting a new capture; document the producer's origin.
- `increment`: a numeric individual change. It is not a level or an inferred
  cumulative total. Negative increments are permitted.
- `context`: observations have `metric`, `tick`, `value` (bounded string). The
  recorder uses unit `text`. Emit an initial observation and subsequent changes;
  context before its first retained observation is unknown.

Counter values must be finite. Counter and context lists are ordered by
nondecreasing time; each observation is inside `[0, end_tick]`. There is no
implicit interpolation, reset detection, rate or missing-as-zero conversion.

Duration records have `metric`, `start_tick`, `duration_ticks`. Their end lies
within the capture. They are inclusive elapsed times on one logical track: the
recorder's creating thread. Positive durations must nest or be disjoint; crossing
intervals are invalid. Events may be stored in completion order. Export sorts by
start time and then descending duration to put parents first at equal starts.
Zero-tick events are valid at the resolution of the recorder's clock.

## Format 2 changes

Format 1 summary captures retained every counter sample, so one busy counter could
fill the record limit within seconds and push out every other counter and context
observation for the rest of the window. Format 2 keeps a complete total per counter
instead, whatever the mode or the record limit.

Each `counter_aggregates` entry has `metric`, `samples`, `sum`, `min`, `max` and
`last`. Every accepted sample enters its counter's total, including samples that
detailed mode could not retain. A counter with no samples reports all zeros, which
means unobserved, not a level of zero; the analyser shows those figures as
unavailable. The validator requires `min <= last <= max`; for a cumulative counter,
`min >= 0` and `last == max`. A sample that would overflow the sum is rejected and
counted in `rejected_measurements`.

Read the figures by kind:

- `gauge`: the mean is the mean of samples, not a time-weighted average. It is
  only as representative as the producer's sampling.
- `increment`: the sum is the total change; the analyser also reports it per real
  second of capture.
- `cumulative`: read `last`.

Summary mode retains no counter samples in format 2; it still retains context.
Detailed mode retains samples while capacity remains, and with no dropped records
its retained samples must agree with the totals: the same count and last value,
and a sum equal within a relative 1e-9.

Format 1 captures carry no totals. The analyser reports their counters from
retained samples, labelled `retained_samples`, which are partial when records were
dropped.

## Format 3 changes

Each operation aggregate gains `self_ticks`: the part of `total_ticks` spent outside
the operation's own completed child scopes. The recorder subtracts each completed
child's duration from its parent as the child ends. Children lie inside their
parent on the single recording thread, so self time is never negative, and self
times of different scopes never overlap: unlike inclusive totals, they add up. A
nested call rejected at the depth limit was never on the stack, so its time stays in
its parent's self time. Self time is still elapsed time on that thread, not CPU use,
and it includes waits and any unmeasured work done inside the scope.

The validator requires `self_ticks <= total_ticks`, requires it in format 3 and
refuses it earlier. In a detailed capture with no dropped records it recomputes self
time from the retained events (each event's duration less its direct children's)
and requires an exact match. Events with equal start and duration are ordered by
completion: the one that completed later is the parent, as the recorder saw them.

The analyser reports self ms, mean self ms per call, and the share of the capture's
real time spent inside measured operations (self times added up). The comparison
reports self ms per real second where both captures have it.

## Recording series

`phobos-scope series NEW_OUTPUT_DIRECTORY CAPTURE.json...` joins the windows of one
recording into a timeline (`series.html`, `series.json`, `series.csv`). Windows are
ordered by two metadata keys an adapter may write: `recording_id` (one value per
recording) and `window` (1, 2, 3... within it). When any capture lacks them, the
given order is kept and the report says so. Mixed recordings, and window numbers
that skip or repeat, are reported. The time axis is recorded time, the sum of
earlier windows' durations: gaps between windows (a world load) are not on it.

Each window keeps its own complete figures. Gauges show each window's minimum,
mean, maximum and last reading; increments and operations are per real second of
that window. For each gauge the report gives the least-squares slope of the
window minimums against recorded time, per hour, from three windows up. For the
managed heap after a collection that slope is the plainest leak sign. It is a
description, not a statistical test, and nothing is interpolated between windows.

## Completeness and lifecycle

Summary mode retains complete operation aggregates and counter totals, plus bounded
context samples (format 1 summary also retained counter samples). It never retains duration events. Detailed mode also retains each
completed duration while capacity remains. Once capacity is full, further records
increment `dropped_records`; completed-operation aggregates continue. No sampling
strategy or statistical representativeness is claimed.

The validator checks retained event counts/totals/maxima against aggregates. With
no drops in detailed mode they must agree exactly. With drops, retained values
must not exceed the full completed aggregates. Rejected measurements are outside
that aggregate population; reports carry the rejection count.

Start creates a new identity and generation. Scopes from a prior generation never
enter the new capture. Stop turns all open accepted scopes into incomplete counts.
Duration limits are checked on instrumentation calls, Poll and Stop, and clamp
the end boundary to the configured limit. A scope crossing the limit remains
incomplete. The host must Poll on its update loop; there is no timer thread.

Wrong-thread measurement calls are ignored and counted while recording is active.
An adapter can call `StopAfterDiagnosticFailure` to stop explicitly with a rejected
measurement marker; this preserves v1 stop reasons while making failure visible
in the analyser's capture-quality warnings.
Start/Stop/registration on the wrong thread are setup errors. Non-LIFO disposal
stops the capture with `nesting_error`; open scopes become incomplete. Clock
regression/failure stops with `clock_error`. Stop on an already stopped recorder
returns its latest snapshot. Start does not destroy that snapshot; the next stop
replaces `LastCapture`, so retain snapshots the caller still needs.

Export is explicit from stopped immutable data, through a sibling temporary file
to a new destination. Failures preserve the snapshot and do not overwrite existing
files. No per-event I/O or automatic application-exit persistence occurs. Sudden
process termination can lose the in-memory capture. Future adapters stop at world
changes and never write profiling data into saves.

## Derived output

`total_ms = total_ticks * 1000 / clock_frequency_hz`; mean divides that total by
completed calls. Calls per second divides completed calls by capture wall time.
Mean/maximum are unavailable when calls are zero; rate is unavailable at zero
wall duration. Incomplete counts are reported separately. No percentile is
derivable from count/total/maximum alone.

Time windows are sparse, aligned to tick zero. Each duration contributes its
intersection with each window, and counts as a call start in exactly one window.
Durations that straddle windows are split. Parent/child overlap remains inclusive.
Zero-duration calls at capture end may produce a final zero-width row. Maximum
window count is 10,000 and event/window intersections are capped at 2,000,000.

CSV uses stable English machine headers, invariant numbers, explicit units and
proper quoting. Formula-like text cells receive an apostrophe. `report.json`
preserves source labels and includes quality warnings. Chrome Trace JSON uses
microsecond `ts`/`dur`, process/thread metadata, `X` durations, `C` gauge/cumulative
samples, and thread-scoped point events for increments/context. The single track
is logical, not a claim about OS thread IDs. Retained-event loss is disclosed.

The CLI emits no timeline/time-series file for summary mode. All derived files
are staged in a fresh directory and published together; existing output is
refused. Input captures are never rewritten.
