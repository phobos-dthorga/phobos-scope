# Original project handoff

Historical pre-implementation handoff supplied by the owner on 2026-09-25.
The current README and capture-format document describe the implemented slice.

**Phobos Scope — project handoff**

Phobos Scope is a proposed, reusable performance-recording and analysis toolkit for game mods. Its purpose is to help us understand the cost of our code, identify expensive operations, compare changes, and produce useful visual reports.

The owner wants most analytical functionality in a **game-independent Rust library**, with small recording components written in whichever language each game supports. Ostranauts should require only a thin integration through Phobos Framework.

This handoff records the agreed direction and recommended initial implementation. **No Phobos Scope library, executable, recorder, export format or profiling commands have been implemented yet.**

**1. Motivation and intended experience**

The project grew out of a discussion about the compiled sizes of our Ostranauts mods. DLL size provides little information about runtime cost: a small amount of frequently executed code can be more expensive than a larger collection of occasional operations.

The owner requested performance measurements that operate only in an explicitly enabled diagnostic mode, followed by exports that free applications can analyse and display as attractive graphs.

The intended workflow is:

1. Enable a bounded capture through the game’s debug console.
2. Play normally or reproduce a particular workload.
3. Stop recording and export the capture.
4. Process it with Phobos Scope outside the game.
5. Inspect interactive timelines, spreadsheet charts or comparisons between runs.

Ordinary gameplay should require neither a running Rust process nor an installed analysis application.

**2. Names**

| Component | Proposed name |
|---|---|
| Project | **Phobos Scope** |
| Repository | `phobos-scope` |
| Analysis executable | `phobos-scope` |
| Shared Rust library | `phobos-scope-core` |
| Reusable C# recorder | `Phobos.Scope.Recording` |

These are project naming decisions, not claims that repositories, packages or registry names have already been created or reserved.

Suggested project description:

> An opt-in profiling and analysis toolkit for game mods, with lightweight recording adapters, a shared Rust analysis engine, and portable exports for visualization.

**3. Architectural boundary**

Measurements must be captured near the code being measured. The bulk of analysis can run afterwards.

| Layer | Responsibilities |
|---|---|
| Rust core library | Capture validation, statistics, time-window aggregation, spike analysis, run comparisons and export generation. |
| Standalone command-line application | Load captures, call the Rust library, write reports and exports, and explain invalid or incomplete data. |
| Language-specific recorders | Measure explicitly instrumented operations, collect counters and context, and produce bounded captures. |
| Game adapters | Connect recorder lifecycle and commands to the game, and supply relevant game-specific metadata. |
| Individual mods | Identify meaningful operations and counters to instrument. |
| Existing visualization applications | Display timelines and conventional charts. |

The first recorder should be C#, because Ostranauts is the first concrete consumer. Other compatible C# projects should be able to reuse it without referencing Ostranauts.

Lua or other language recorders should be added when a real integration requires them. Sharing a capture contract allows their analysis code to remain entirely in Rust.

Use **file-based exchange initially**. Do not require a native Rust DLL inside the game, continuous inter-process communication, or a background service. Rust supports native interoperability, but that adds deployment and runtime-boundary work without an established need here. [Rust interoperability documentation](https://doc.rust-lang.org/nomicon/ffi.html)

The generic components must not reference Unity, BepInEx, Harmony, Ostranauts classes or game assets.

**4. Recording modes**

Two complementary modes were discussed:

- **Summary capture:** inexpensive aggregates for named operations and counters.
- **Detailed capture:** bounded timestamped events for investigating when work happened and how operations overlapped or nested.

Detailed timelines cannot be reconstructed from aggregate statistics. Exports must clearly identify which information the capture actually contains.

Keep the recorder simple. Small online aggregates may be necessary to avoid retaining every event, but report generation, comparisons and richer analysis belong in Rust.

Profiling is **disabled by default**. The disabled path should avoid timer reads, recording allocations, string formatting and file operations. A small enabled check is acceptable; do not promise literally zero overhead.

Enabling profiling must not automatically enable verbose flight logs or other unrelated diagnostics.

**5. Capture contract**

Design and document a small, versioned, language-independent capture format before coupling it to game code.

JSON or newline-delimited JSON is a reasonable starting candidate. The exact encoding is not yet settled. Prefer simplicity and inspectability over inventing a complex protocol.

The format should represent, where available:

- Format version and recorder version.
- Capture identity and start/end boundaries.
- Game, mod and relevant build versions.
- Capture mode and recording limits.
- Monotonic clock units or frequency.
- Stable operation names and categories.
- Operation start times and durations.
- Thread or track identity where meaningful.
- Numeric counters with explicit units and interpretation.
- Workload context such as game speed and panel visibility.
- Dropped events, truncation and incomplete measurements.

Use monotonic real elapsed time for performance measurements. Keep simulation time separate: pausing, fast-forwarding and loading an earlier save must not make measured durations negative or alter their units.

Context that changes during recording—such as game speed—needs timestamped changes or separate capture segments. One initial setting is insufficient.

Distinguish counters that represent a current quantity, a cumulative total or an increment. The analyser must not infer that distinction from a display name.

Avoid unbounded labels such as a separate metric name for every ship or item. Keep metadata small and purposeful.

Do not export save contents, player names, arbitrary game objects or personal paths by default.

**6. Measurement semantics**

Initial useful measurements include:

- Number of calls.
- Total, mean and maximum elapsed duration.
- Calls per real second.
- Objects examined or processed.
- Cache hits and misses.
- Queue or connection counts.
- Costs grouped into time windows.

Percentiles are useful only when supported by retained samples or a documented approximation. Do not fabricate them from count, total and maximum alone.

Be explicit about limitations:

- An instrumented operation’s elapsed time includes native functions it calls and any waits or scheduling delays.
- It is not automatically exclusive CPU time attributable to that mod.
- Nested durations cannot simply be added together without double-counting.
- Summed durations across threads can exceed the capture’s wall-clock duration.
- Process memory and garbage collection are not inherently attributable to individual mods.
- Named timing scopes do not provide automatic full call-stack profiling.
- Profiling itself introduces overhead that needs to be measured.

Unavailable measurements should remain unavailable, rather than appearing as zero.

If events are dropped, reports must distinguish retained-sample statistics from any complete aggregates recorded separately.

**7. Exports and visualization**

The initial recommended outputs are:

| Output | Intended use |
|---|---|
| Chrome Trace Event JSON | Interactive timelines and counters in Perfetto. |
| Summary CSV | Operation comparisons and charts in LibreOffice Calc or another spreadsheet application. |
| Time-series CSV | Performance over time and relationships between cost and workload. |

Perfetto accepts externally generated Chrome Trace JSON, including duration events, counters and metadata. Use its documented common event types and respect its nesting rules. [Perfetto external trace formats](https://perfetto.dev/docs/getting-started/other-formats)

Perfetto is open source and provides a browser-based local viewer. It should supply the first interactive visualization experience; there is no need to build a competing timeline viewer. [Perfetto overview](https://perfetto.dev/docs/)

LibreOffice Calc can provide conventional, customizable charts from exported data. A reusable chart template could follow once actual captures establish useful layouts. [Calc chart documentation](https://books.libreoffice.org/en/CG262/CG26206-CreatingChartsAndGraphs.html)

CSV should have stable column names, explicit units, consistent number formatting and correct escaping.

A self-contained HTML report is a possible later addition. It is not necessary for the first working slice.

**8. First integration: Ostranauts**

The existing repository is:

[phobos-ostranauts](https://github.com/phobos-dthorga/phobos-ostranauts)

The owner has been developing:

- **Phobos Framework:** shared registration, persistence, controls, material-handling and observation services.
- **Phobos Auto Nav:** navigation guidance, sensor-aware contact handling, RCS/torch control and docking.
- **Phobos Shipbreaker:** industrial processing, material routing and equipment controls.

The inspected projects target **.NET Standard 2.1**. Existing integration research used **Ostranauts 1.0.1.5** and **BepInEx 5.4.23.5**. Verify the actual installation and runtime APIs before choosing recorder dependencies.

Useful integration locations include:

- `FrameworkConsole`, which handles the `phobosframework` command.
- Auto Nav’s existing navigation service and `ShipSitu.TimeAdvance` patch.
- Auto Nav’s persistence update path.
- Framework’s native observation adapters.
- Shipbreaker’s processing, routing and console-observation services.

Auto Nav already has optional verbose diagnostics. Those are not a performance recorder.

Suggested future commands are:

    phobosframework perf start
    phobosframework perf status
    phobosframework perf stop
    phobosframework perf export

These commands do not exist yet. Their options and exact behaviour still need design.

The current Framework command parser accepts only its existing short commands, so supporting this hierarchy requires a deliberate parser extension.

Framework should own the Ostranauts capture session, commands and context. Individual mods should register meaningful operations and use the shared recorder. Avoid duplicating recording infrastructure in each mod.

**9. Initial instrumentation targets**

Start with a few useful, coarse operations:

| Area | Candidate measurements |
|---|---|
| Auto Nav | Guidance updates, native contact checks and docking calculations. |
| Industrial routing | Transfer-pass duration, active connections, blocked destinations and items examined. |
| Processing | Machinery update duration and active job count. |
| Console observations | Discovery, cached reads, source validation and observation count. |
| Panels | Refresh or presentation costs, explicitly separated from underlying service work. |

Examples of questions the toolkit should help answer:

- Does guidance cost change between coasting, braking and docking?
- How does material-routing cost grow with active connections?
- Does opening an industrial console add appreciable work?
- Are repeated scans defeating a cache?
- Did a code change improve performance under comparable conditions?
- Does fast-forward increase invocation frequency, per-call cost, or both?

Do not instrument every method immediately. Expand coverage in response to actual questions.

Developer profiling and gameplay instrumentation are separate concepts. Recording performance must not change sensor emissions, machine operation, flight authority, material transfers or saved state.

**10. Capture safety and lifecycle**

Use bounded memory and duration limits. Report reaching those limits rather than silently losing data.

Avoid disk writes per event. Export explicitly, preferably from a stopped snapshot.

Keep file formatting and analysis outside measured operations. Retain the capture when an export fails so the owner can retry.

Recording scopes should close correctly when the measured code throws, without swallowing or changing the original gameplay exception.

Account for scopes that remain open when recording stops. Mark them incomplete rather than inventing completed durations.

Choose and document behaviour for save loads, new games and application exit. A reasonable initial policy is to stop or segment a capture at world changes. Do not silently join unrelated sessions or store profiling records inside saves.

Diagnostic failures should not break gameplay. Handle them without repeated per-frame error spam.

Cross-thread behaviour should be explicit: support it correctly or document a restricted first implementation.

**11. Recommended first implementation**

Build a vertical slice that proves the shared boundary:

1. Establish the Rust workspace, reusable core and command-line application.
2. Define the capture contract with synthetic examples.
3. Implement validation and basic operation statistics.
4. Add CSV and Perfetto exports.
5. Build the small C# recorder with no game dependencies.
6. Demonstrate C# recording → Rust analysis → usable exports using a standalone sample.
7. Integrate a small number of Ostranauts operations through Framework.
8. Prepare short owner-run capture instructions.

Keep the Rust core callable as a library so future applications can reuse it without invoking the command-line tool.

Exact CLI syntax, capture filenames, package versions and distribution method remain implementation decisions. Inspect the new project’s toolchain before assuming Rust or any particular dependencies are installed.

**12. Verification**

Focus automated checks on meaningful behaviour:

- Known durations and counts produce correct statistics.
- Clock conversions and large timestamps retain appropriate precision.
- Nested measurements are not misreported as exclusive totals.
- Capture limits and dropped-event accounting work.
- Stop/start cycles and exception paths do not mix sessions.
- Invalid, truncated and unsupported-version captures produce actionable results.
- C# captures are accepted by the Rust implementation.
- CSV escaping and trace export structure are correct.
- Failed exports preserve recoverable data.

Use representative standalone workloads to quantify enabled and disabled recorder overhead. Record results rather than inventing an acceptable percentage in advance.

Verify exports in the intended viewer. Valid JSON alone does not prove a useful timeline.

The owner performs in-game testing. Suggested later captures are ordinary play, fast-forward and an open industrial console under comparable workloads.

A successful build or synthetic test must never be described as verified in-game performance.

**13. Working preferences and repository boundaries**

The owner values reusable tools and meaningful shared infrastructure, while preferring practical working slices over speculative generalization.

Carry forward these preferences:

- Keep UI and presentation separate from analysis and state-changing services.
- Use named units, limits and tolerances instead of unexplained numbers.
- Design human-facing messages for localization; keep machine keys and command names stable.
- Reuse established libraries and file formats where they fit.
- Preserve licensing and attribution; no project license has yet been selected for Phobos Scope.
- Keep repositories private unless explicitly instructed otherwise.
- For requested checkpoints, the existing preference is ordinary commits and direct pushes, without PR formalities unless requested.
- Do not force-push, change visibility or publish packages implicitly.
- Do not commit game assemblies, extracted assets, decompiled source, saves, credentials or personal machine paths.
- Use repeatable build and packaging scripts where worthwhile.
- Do not take control of the owner’s mouse or keyboard.
- Leave gameplay execution and save manipulation to the owner.

The Ostranauts working tree contained substantial ongoing changes when this handoff was prepared. Coordinate integration with that project and inspect its current state; do not overwrite, revert or commit unrelated work.

**14. Deliberately deferred**

The first release does not need a native Rust runtime inside games, a resident service, live network streaming, a hosted dashboard, automatic uploads, a complete call-stack profiler, per-mod memory attribution, or adapters for every game.

Those can be considered when a concrete use case justifies their cost.

The immediate objective is:

> Deliver a reusable C# recorder and Rust analyser that capture a small amount of meaningful mod-performance data, produce trustworthy statistics, and export usable Perfetto and CSV files—with only a thin Ostranauts-specific layer in Phobos Framework.