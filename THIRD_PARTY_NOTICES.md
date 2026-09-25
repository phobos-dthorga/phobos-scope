# Third-party provenance

The repository layout and contributor conventions are adapted from
[Phobos Ostranauts](https://github.com/phobos-dthorga/phobos-ostranauts), by
Phobos A. D'thorga. No game-derived source, binaries, artwork, saved games or
upstream navigation adaptations are included. Phobos Scope's project licence
remains undecided; see [licensing status](docs/licensing.md).

## Rust dependencies

The following versions and declared licences were read from the resolved Cargo
package metadata on 2026-09-25. `Cargo.lock` fixes the complete dependency graph.
Dependencies are fetched from crates.io, not vendored or relicensed. Preserve
their actual licence notices when preparing a future distributable binary.

| Crate | Version | Declared licence | Upstream |
|---|---|---|---|
| csv | 1.4.0 | Unlicense/MIT | [rust-csv](https://github.com/BurntSushi/rust-csv) |
| csv-core | 0.1.13 | Unlicense/MIT | [rust-csv](https://github.com/BurntSushi/rust-csv) |
| itoa | 1.0.18 | MIT OR Apache-2.0 | [itoa](https://github.com/dtolnay/itoa) |
| memchr | 2.8.3 | Unlicense OR MIT | [memchr](https://github.com/BurntSushi/memchr) |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | [proc-macro2](https://github.com/dtolnay/proc-macro2) |
| quote | 1.0.47 | MIT OR Apache-2.0 | [quote](https://github.com/dtolnay/quote) |
| ryu | 1.0.23 | Apache-2.0 OR BSL-1.0 | [ryu](https://github.com/dtolnay/ryu) |
| serde | 1.0.229 | MIT OR Apache-2.0 | [serde](https://github.com/serde-rs/serde) |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | [serde](https://github.com/serde-rs/serde) |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | [serde](https://github.com/serde-rs/serde) |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | [serde_json](https://github.com/serde-rs/json) |
| syn | 3.0.6 | MIT OR Apache-2.0 | [syn](https://github.com/dtolnay/syn) |
| unicode-ident | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | [unicode-ident](https://github.com/dtolnay/unicode-ident) |
| zmij | 1.0.23 | MIT | [zmij](https://github.com/dtolnay/zmij) |

The C# recorder uses the .NET Standard 2.1 base libraries, including
`DataContractJsonSerializer`, and has no third-party NuGet dependencies. The sample
and test runner require .NET 10, without bundling the runtime.

## External inspection tools

[Perfetto](https://github.com/google/perfetto) is an external Apache-2.0 project.
Its v58.2 Windows trace processor was downloaded from its official GitHub release
to ignored `.tools/` for local import verification; it is not distributed here.
Phobos Scope generates the documented Chrome Trace Event JSON format.
LibreOffice is an optional external CSV/chart viewer and is not bundled.

GitHub Actions use official `actions/checkout` and `actions/setup-dotnet` at
recorded commit hashes. They are CI tooling, not application dependencies.
