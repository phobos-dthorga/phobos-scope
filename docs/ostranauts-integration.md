# Ostranauts integration boundary

**Planning only in this repository.** The first slice runs standalone. No
`phobosframework perf` commands or game patches have been added, and no saves,
installed mods or game files have been changed.

The reference checkout inspected on 2026-09-25 targets .NET Standard 2.1 for
Phobos Framework, Auto Nav and Shipbreaker. The recorder targets the same API
surface and has no game dependencies. This establishes a build-time fit, not
verified loader/runtime compatibility. Recheck installed Ostranauts/BepInEx and
the latest Framework source before integrating.

## Ownership

Framework owns one recorder on its main thread, registered operation handles,
the capture lifecycle and workload context. Content mods register coarse
operations and use those handles. Generic recording stays here; validation and
report generation stay in Rust. The game needs only the recorder assembly.

Extend FrameworkConsole's parser deliberately to support the proposed hierarchy:

```text
phobosframework perf start
phobosframework perf status
phobosframework perf stop
phobosframework perf export
```

Start should select bounded mode/duration/record limits. Status should call Poll
and report active/stopped state, stop reason and quality counts. Export should use
the stopped snapshot and a new filename in a local captures directory. On export
failure, retain data and give one actionable, localized console message for retry.
Keep machine command/metric keys stable. Starting performance recording must not
enable verbose flight logging.

Poll once per owner-thread update. Stop at save load/new game with WorldChange and
on orderly exit with ApplicationExit. Do not automatically join worlds or resume
capturing after reload. Leave profiling out of persistence and simulation state.
No async scopes or background thread measurements in v1; rejected calls are visible
in the report. Use disposable scopes so gameplay exceptions retain their identity.

## First instrumentation candidates

Choose a few coarse operations after inspecting current services: Auto Nav
guidance/contact checks, Framework observation reads and Shipbreaker routing or
processing passes. Capture queue/connection/item counts and cache-hit increments
with explicit units. Record game speed and panel visibility initially and when
they change. Record relevant build versions as small metadata values.

Prepare owner-run instructions for ordinary play, fast-forward and an open
industrial console under comparable workloads. Verify loaded assembly versions,
disabled behaviour, world-change stopping, bounds, export retry and usable Perfetto
tracks. Do not infer flight behaviour, resource use or per-mod CPU attribution from
standalone test success. Coordinate changes with the other repository's current
working tree and avoid unrelated edits or commits.
