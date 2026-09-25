# Contributing

Read [AGENTS.md](AGENTS.md), the [project direction](docs/project-direction.md)
and the [capture contract](docs/capture-format.md). Keep changes focused on a
useful recording or analysis workflow. Run `pwsh -File scripts/verify.ps1`.

Use synthetic data for tests. Report actual recorder overhead measurements with
their environment and workload; do not equate elapsed timing with CPU attribution.
Record in-game evidence separately from standalone tests.

This private project uses ordinary commits and direct pushes to `main` for
requested checkpoints. Public releases, package publication, licensing and
repository visibility remain separate owner decisions.
