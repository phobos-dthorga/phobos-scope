# Verification and measurement record

## Repeatable checks

Run `pwsh -File scripts/verify.ps1`. It formats/checks the Rust workspace, applies
Clippy with warnings as errors, runs Rust behaviour tests, builds Release outputs,
runs deterministic C# lifecycle checks and analyses every generated C# fixture
through the real Rust CLI. It also checks known statistics, summary-mode export
limits, CSV escaping, loss accounting and CLI failure/retry boundaries.

Run with `-TraceProcessor <path-to-trace_processor_shell>` to add an actual Perfetto
import check. The local official v58.2 importer accepts the sample's 240 duration
slices and 240 counter observations, with matching durations, scan nesting at
depth 1 and no parser errors. This verifies the viewer's import/analysis engine;
interactive layout still has the owner-run walkthrough in `getting-started.md`.

GitHub Actions repeats the standalone suite on Windows and Linux. The optional
Perfetto import check uses a separately supplied executable and is not downloaded
implicitly by CI. No test opens a game or touches saved games.

## Initial local evidence (2026-09-25)

- Rust 1.97.0, Windows x64; Rustfmt and Clippy with warnings as errors.
- .NET SDK 10.0.400; recorder targets .NET Standard 2.1. Sample/tests run on .NET
  10.0.12. No game runtime was loaded.
- Deterministic recorder checks cover disabled clocks/allocations, source clocks
  above 2^53, nesting, summary mode, record/depth/duration limits, open scopes,
  restart isolation, exception propagation, invalid samples, wrong threads,
  non-LIFO disposal, clock failures, Unicode and retryable exports.
- Rust checks cover known statistics, unavailable observations, malformed/versioned
  input, duplicate keys, inconsistent loss accounting, bounds, nesting, split
  windows, summary restrictions, high-frequency conversion, CSV and write failures.

## Recorder overhead microbenchmark

Run `pwsh -File scripts/demo.ps1 -Benchmark` to produce `overhead.json`, or run the
sample with `--benchmark NEW_REPORT.json`. Results include runtime, OS version,
architecture, timer frequency, iteration counts, range and median.

The first local run used a Release build, 32 integer mixing steps per operation,
10,000 operations per repetition, three warm-up repetitions and seven measured
repetitions per mode. Timer frequency: 10,000,000 Hz. Recording setup, Stop and
serialization are excluded; enabled recording work and event allocations are
included. Windows version was 10.0.26200, process architecture x64.

| Mode | Median ns/operation (work included) | Min–max ns/operation | Median allocated bytes/operation |
|---|---:|---:|---:|
| Baseline workload | 106.03 | 97.91–121.14 | 0 |
| Disabled recorder | 100.73 | 96.97–134.30 | 0 |
| Summary recording | 210.39 | 165.21–215.85 | 0 |
| Detailed recording | 231.43 | 205.42–301.61 | 66.2424 |

The small negative disabled-versus-baseline difference is noise, not a speedup
claim. Tiering, scheduling and timer cost affect these microbenchmarks. Independent
deterministic checks confirm that the disabled path reads no clocks and allocates
no managed memory after warm-up; they do not prove literally zero overhead.
Detailed allocations include stored event objects and list growth. These numbers
are not representative game overhead, a target budget, or a percentage guarantee.

Ostranauts integration, loader compatibility and in-game behaviour/performance
remain unverified. Automated run comparisons, percentile estimates and graphical
report templates are later work.
