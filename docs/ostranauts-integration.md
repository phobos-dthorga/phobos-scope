# Ostranauts integration

The adapter is implemented in the separate
[Phobos Ostranauts repository](https://github.com/phobos-dthorga/phobos-ostranauts):
Framework **0.15.0**, Auto Nav **0.10.1**, Shipbreaker **0.11.1**. Framework builds
the recorder from a pinned Scope submodule and packages one shared recorder DLL.
The full [capture guide](https://github.com/phobos-dthorga/phobos-ostranauts/blob/main/docs/performance-captures.md)
documents installation, metric semantics and owner-run gameplay checks.

The adapter builds against the locally installed game. Synthetic adapter captures
are accepted by the pinned Rust analyser, including incomplete scopes at world
changes and bounded summary data. These checks do not establish Unity/Mono loader
compatibility or in-game performance. No game launch, save access or automated
gameplay is part of verification.

## Ownership and commands

Framework owns one recorder on its main thread, registered operation handles,
capture lifecycle and workload context. Content mods register coarse operations
and use opaque handles. Generic recording stays here; validation and report
generation stay in Rust. The game needs only the recorder assembly.

```text
phobosframework perf start [detailed|summary] [seconds] [records]
phobosframework perf status
phobosframework perf stop
phobosframework perf export
```

Defaults are detailed mode, 60 seconds and 20,000 retained records. Limits are
1–3,600 seconds and 1–20,000 records. Recording starts only in a loaded world.
Exports use unique filenames under `BepInEx/captures/PhobosScope`; a stopped
capture must be exported before the next start. Failed exports retain the
snapshot for retry. Captures remain in memory until explicitly exported; there
is no automatic save on exit.

Framework polls once per update. Load/new-game transitions stop with
`world_change`; orderly exit stops with `application_exit`. Open scopes become
incomplete. Diagnostic failures stop recording, count a rejected measurement and
log once. Human messages use Framework's translation catalog. Recording does not
enable verbose flight logging or modify simulation and persistence state.

## Instrumentation

The first operations cover Auto Nav guidance, docking and contact reads;
Framework room-alarm observations; Shipbreaker processing and routing checks and
advances; and industrial-panel refreshes. A routing increment counts the candidate
collection size offered to selection, not the number of predicates executed.
Speed multiplier, pause and console visibility are sampled initially and when
changed, while recording. Small metadata values identify the game and loaded
mod versions. No per-object names or save data are included.

Scopes measure inclusive real elapsed time. Nested totals overlap; native waits
are not exclusive mod CPU use. Synchronous main-thread scopes are required.

## Verification boundary

The consumer repository's `scripts/verify-performance.ps1` exercises the real
adapter service and console parser against small game-boundary doubles, then
analyses exported captures with the pinned Rust CLI. Its normal mod build scripts
also run adapter checks. Installer fixtures check missing and duplicate recorder
DLLs and dependency versions. These complement Scope's recorder/contract checks.

Owner-run checks must still verify loaded assembly versions, disabled behavior,
ordinary play, fast-forward, open consoles, load/new-game stopping and usable
exports under comparable workloads. Never infer flight behavior, resource use or
per-mod CPU attribution from standalone test success.
