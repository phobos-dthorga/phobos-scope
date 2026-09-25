use phobos_scope_core::*;
use std::{
    env,
    fs::{self, File},
    path::Path,
    process::ExitCode,
};

const HELP: &str = "Phobos Scope\n\n  phobos-scope validate CAPTURE.json\n  phobos-scope analyse CAPTURE.json NEW_OUTPUT_DIRECTORY [WINDOW_MS]\n  phobos-scope compare BEFORE.json AFTER.json NEW_OUTPUT_DIRECTORY\n\nAnalysis includes report.html. Comparison includes comparison.html, JSON and CSV.\nWINDOW_MS defaults to 1000. Existing output directories are never overwritten.\nTimings are inclusive elapsed time, not exclusive CPU usage.\n";

fn publish_directory(staging: &Path, output: &Path) -> std::io::Result<()> {
    const RETRY_LIMIT: usize = 20;
    for attempt in 0..=RETRY_LIMIT {
        if output.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Output already exists.",
            ));
        }
        match fs::rename(staging, output) {
            Ok(()) => return Ok(()),
            Err(error)
                if cfg!(windows)
                    && attempt < RETRY_LIMIT
                    && matches!(error.raw_os_error(), Some(5 | 32 | 33)) =>
            {
                // Indexers/sync clients may briefly hold a newly written Windows directory.
                // Retry the same rename only; never replace another output or change permissions.
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(error) => {
                return Err(std::io::Error::new(
                    error.kind(),
                    format!("Could not publish report directory: {error}"),
                ));
            }
        }
    }
    unreachable!()
}

fn write_reports(
    output: &Path,
    write: impl FnOnce(&Path) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    if output.exists() {
        return Err("Output already exists; select a new directory.".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".phobos-scope-{}", std::process::id()));
    fs::create_dir(&staging)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        write(&staging)?;
        publish_directory(&staging, output)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result
}

fn analyse(
    c: &ValidatedCapture,
    output: &Path,
    window_ms: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let width_ticks =
        (window_ms as u128 * c.data().clock_frequency_hz as u128 / 1_000).try_into()?;
    if c.data().mode == Mode::Detailed {
        time_windows(c, width_ticks)?;
    }
    write_reports(output, |staging| {
        summary_csv(c, File::create(staging.join("summary.csv"))?)?;
        report_html(c, File::create(staging.join("report.html"))?)?;
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
        Ok(())
    })
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
        || (args[0] == "analyse" && (args.len() == 3 || args.len() == 4))
        || (args[0] == "compare" && args.len() == 4);
    if !valid {
        return Err(format!("Invalid arguments.\n{HELP}").into());
    }
    let c = read_capture(File::open(&args[1])?)?;
    for warning in c.warnings() {
        eprintln!("Warning: {warning}");
    }
    if args[0] == "compare" {
        let after = read_capture(File::open(&args[2])?)?;
        let comparison = compare(&c, &after);
        for warning in &comparison.warnings {
            eprintln!("Warning: {warning}");
        }
        write_reports(Path::new(&args[3]), |staging| {
            comparison_html(&comparison, File::create(staging.join("comparison.html"))?)?;
            comparison_csv(&comparison, File::create(staging.join("comparison.csv"))?)?;
            serde_json::to_writer_pretty(
                File::create(staging.join("comparison.json"))?,
                &comparison,
            )?;
            Ok(())
        })?;
        println!("Comparison written to {}", Path::new(&args[3]).display());
    } else if args[0] == "analyse" {
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
