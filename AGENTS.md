# Phobos Scope contributor instructions

## Working conventions

- Author: Phobos A. D'thorga (phobosgekko).
- Keep this repository private. Requested checkpoints use ordinary commits and
  direct pushes to `main`; no PR workflow unless the owner asks. Never force-push,
  change visibility or publish packages implicitly.
- Conventions are adapted from `phobos-dthorga/phobos-ostranauts`. Do not copy its
  gameplay policies, game-derived material or unrelated implementation.
- Prefer practical working slices, small reusable scripts and meaningful tests.
  Explain verified behaviour and remaining limits without invented time estimates.
- Keep local paths, credentials, game binaries/assets, decompiled source, saves
  and real captures out of Git. Synthetic fixtures are welcome.
- Do not control the owner's mouse or keyboard. Gameplay and save testing belong
  to the owner. Never describe a build or synthetic workload as an in-game test.
- Private-key generation, conversion and credential entry remain owner-run.
- The project licence has not yet been selected. Preserve third-party notices;
  do not silently import another repository's licence or relicense dependencies.

## Architecture and measurements

- Read `docs/project-direction.md` and `docs/capture-format.md` before changing
  the contract. Version incompatible changes explicitly.
- Keep validation, statistics and exports in the reusable Rust core. The CLI
  handles arguments and files. Recorders measure and bound data; adapters connect
  lifecycle and context. No Unity, BepInEx, Harmony or game references in core or
  recorder projects. File exchange is the initial integration boundary.
- Profiling is opt-in. The disabled hot path must avoid clocks, allocations,
  formatting and I/O. Register bounded stable names before recording.
- Preserve monotonic elapsed-time semantics, explicit units and counter kinds.
  Incomplete, missing or dropped observations are not zero. Inclusive elapsed
  time is not exclusive CPU time; never add nested totals as a utilisation score.
- Never fabricate percentiles from aggregates or timelines from summary mode.
- Keep UI and human messages separate from computation. Use stable machine keys,
  complete localizable messages and named limits/units. Extract duplicated logic
  when there is a concrete shared need.
- Preserve stopped captures after export failure. Instrumentation must not alter
  gameplay exceptions or state; handle diagnostic faults without per-frame spam.
- First recorder contract is synchronous, single-owner-thread recording. Do not
  silently broaden it to async/cross-thread scopes without tests and a new policy.
- Run `scripts/verify.ps1` for contract or implementation changes. Include
  cross-language round trips and targeted lifecycle/precision/export checks.
- Keep Ostranauts integration in its own repository and inspect its current
  working tree before any future integration. Do not commit unrelated work.
