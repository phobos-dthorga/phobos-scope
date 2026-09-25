use phobos_scope_core::*;
use std::{
    env,
    fs::{self, File},
    path::Path,
    process::ExitCode,
};

const HELP: &str = "Phobos Scope 0.1.0\n\n  phobos-scope validate CAPTURE.json\n  phobos-scope analyse CAPTURE.json NEW_OUTPUT_DIRECTORY [WINDOW_MS]\n\nWINDOW_MS defaults to 1000. Existing output directories are never overwritten.\nTimings are inclusive elapsed time, not exclusive CPU usage.\n";

fn analyse(
    c: &ValidatedCapture,
    output: &Path,
    window_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() {
        return Err("Output already exists; select a new directory.".into());
    }
    let width_ticks =
        (window_ms as u128 * c.data().clock_frequency_hz as u128 / 1_000).try_into()?;
    if c.data().mode == Mode::Detailed {
        time_windows(c, width_ticks)?;
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".phobos-scope-{}", std::process::id()));
    fs::create_dir(&staging)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        summary_csv(c, File::create(staging.join("summary.csv"))?)?;
        counters_csv(c, File::create(staging.join("counters.csv"))?)?;
        contexts_csv(c, File::create(staging.join("context.csv"))?)?;
        if c.data().mode == Mode::Detailed {
            windows_csv(
                c,
                width_ticks,
                File::create(staging.join("time-series.csv"))?,
            )?;
            trace_json(c, File::create(staging.join("trace.json"))?)?;
        }
        let manifest = serde_json::json!({
            "analyser_version":env!("CARGO_PKG_VERSION"), "capture_id":c.data().capture_id,
            "mode":c.data().mode, "stop_reason":c.data().stop_reason, "metadata":c.data().metadata,
            "duration_ms":milliseconds(c.data().end_tick,c.data().clock_frequency_hz),
            "clock_frequency_hz":c.data().clock_frequency_hz, "limits":c.data().limits,
            "warnings":c.warnings(), "dropped_records":c.data().dropped_records,
            "rejected_measurements":c.data().rejected_measurements,
            "statistics":statistics(c), "window_ms":window_ms,
            "semantics":"Inclusive elapsed time. Nested totals overlap. Empty cells are unavailable. Time-series rows use retained events only; omitted rows are not evidence of no work."
        });
        serde_json::to_writer_pretty(File::create(staging.join("report.json"))?, &manifest)?;
        if output.exists() {
            return Err("Output appeared during analysis; select a new directory.".into());
        }
        fs::rename(&staging, output)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.is_empty() || (args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        print!("{HELP}");
        return Ok(());
    }
    if args.len() == 1 && args[0] == "--version" {
        println!("phobos-scope {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let valid = (args[0] == "validate" && args.len() == 2)
        || (args[0] == "analyse" && (args.len() == 3 || args.len() == 4));
    if !valid {
        return Err(format!("Invalid arguments.\n{HELP}").into());
    }
    let c = read_capture(File::open(&args[1])?)?;
    for warning in c.warnings() {
        eprintln!("Warning: {warning}");
    }
    if args[0] == "analyse" {
        let window_ms = if args.len() == 4 {
            args[3]
                .to_str()
                .ok_or("WINDOW_MS must be an integer.")?
                .parse::<u64>()?
        } else {
            1_000
        };
        if window_ms == 0 {
            return Err("WINDOW_MS must be positive.".into());
        }
        analyse(&c, Path::new(&args[2]), window_ms)?;
        println!("Reports written to {}", Path::new(&args[2]).display());
    } else {
        println!(
            "Valid capture: {} ({:?})",
            c.data().capture_id,
            c.data().mode
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
