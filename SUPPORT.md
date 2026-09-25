# Help with Phobos Scope

Start with the [walkthrough](docs/getting-started.md). Questions, documentation
fixes and bugs are welcome in [Issues](https://github.com/phobos-dthorga/phobos-scope/issues/new/choose).
Support is best-effort; there is no guaranteed response time.

- Missing compiler/SDK: install the tools in [README](README.md); check the pinned
  `rust-toolchain.toml` and `global.json` versions.
- Existing output directory: choose a new path. Exports deliberately refuse to
  overwrite previous reports.
- Missing timeline: summary captures contain aggregates, not detailed events.
- Missing observations or quality warnings: read `report.json`; missing/dropped
  observations are not zero and cannot be reconstructed.
- Large inclusive totals: nested scopes overlap. Summing them is not CPU usage.
- Ostranauts adapter problems: use [Phobos Ostranauts support](https://github.com/phobos-dthorga/phobos-ostranauts/blob/main/SUPPORT.md).

Reports should include the command, revision, operating system, tool versions,
expected/actual behaviour and a minimal synthetic capture if possible. Review
logs, context labels and captures for private details before posting. Do not
upload credentials, game binaries, saves or unredacted real captures. For
vulnerabilities, use [private reporting](SECURITY.md).
