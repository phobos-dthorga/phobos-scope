# Project direction

Phobos Scope measures explicitly instrumented mod operations and analyses their
cost outside the game. DLL size is not a runtime performance measurement.

The shared Rust library owns validation, analysis and report generation. The CLI
is a file/argument wrapper. Recorders collect bounded timing/counter/context data
in each host language. Game adapters own lifecycle, commands and metadata; mods
choose useful operations. None of the generic components reference game APIs.

The first standalone slice implements JSON v1, aggregate and detailed recording,
basic statistics, time-window aggregation, CSV and Perfetto exports. It establishes
the shared contract before game coupling. `docs/handoff.md` preserves the original
proposal; the README and format document describe the actual implementation.

## Next useful slice

The .NET Standard 2.1 recorder is integrated through Phobos Framework 0.15.0 in
the separate Ostranauts repository. Coarse guidance, docking, contact, observation,
routing, processing and panel operations are instrumented. Next, verify the
loader and lifecycle during owner-run gameplay and collect comparable captures.
Follow `ostranauts-integration.md`.

Comparison automation, spike ranking, sample-supported percentiles, reusable Calc
charts and a self-contained HTML report should follow actual capture questions.
Lua adapters wait for a concrete consumer. Cross-thread recording needs a deliberate
contract extension. There is no native Rust game DLL, background service, upload,
live streaming, custom timeline viewer or package publication in this slice.

## Trustworthy measurement

Use monotonic real elapsed time. Keep game speed and other changing context in
timestamped observations. Never treat unavailable data as zero, infer percentile
distributions from totals, or claim native waits are exclusive mod CPU cost.
Nested totals overlap; cross-thread totals could exceed wall time in a future
multi-thread contract. Process memory is not automatically attributable to a mod.

Profiling must not change simulation, sensors, emissions, material transfers,
saved state or unrelated diagnostic settings. Use explicit capture commands,
bounded memory, explicit exports and recoverable stopped snapshots. Register stable
names instead of creating labels for individual game objects.

Keep metadata purposeful: application/mod/build versions and workload settings,
without names, saves, arbitrary game objects or personal paths. Compare runs only
under documented comparable workloads. Standalone tests are not game compatibility
or in-game performance evidence.
