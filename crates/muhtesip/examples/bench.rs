//! Time the library in process over a corpus directory.
//!
//! The CLI's wall time includes process start and file reading, and a caller that embeds the
//! library pays neither, so this reports the other half: how long
//! [`lint_str`] itself takes over the same bytes. Both numbers are reported; neither is derived
//! from the other, and start-up is never silently subtracted.
//!
//! Usage: `cargo run --release --example bench -- <directory> [passes] [json-out]`

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use muhtesip::{Registry, lint_str};

/// Passes timed by default. More than the CLI's, because an in-process pass is cheap.
const DEFAULT_PASSES: usize = 20;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(directory) = args.first() else {
        return Err("usage: bench <directory> [passes] [json-out]".into());
    };
    let passes: usize = match args.get(1) {
        Some(value) => value.parse()?,
        None => DEFAULT_PASSES,
    };

    let sources = read_corpus(Path::new(directory))?;
    if sources.is_empty() {
        return Err(format!("no YAML files under {directory}").into());
    }
    let bytes: usize = sources.iter().map(|(_, text)| text.len()).sum();
    let registry = Registry::all();

    // A corpus of real files contains documents this parser rejects. That is a fact to report, not
    // a reason to stop: aborting would hide it, and a benchmark that hides a rejected file is
    // claiming coverage it does not have.
    let mut rejected = 0usize;
    let mut first_rejection = String::new();
    for (path, text) in &sources {
        if let Err(error) = lint_str(text, &registry) {
            rejected += 1;
            if first_rejection.is_empty() {
                first_rejection = format!("{}: {error}", path.display());
            }
        }
    }

    // One warm pass: the first read of a file pays for the page cache, and a cold number would be
    // a number about the filesystem rather than about the linter.
    for (_, text) in &sources {
        let _ = lint_str(text, &registry);
    }

    let mut pass_times = Vec::with_capacity(passes);
    let mut per_file: Vec<Duration> = Vec::with_capacity(sources.len() * passes);
    for _ in 0..passes {
        let started = Instant::now();
        for (_, text) in &sources {
            let file_started = Instant::now();
            let _ = lint_str(text, &registry);
            per_file.push(file_started.elapsed());
        }
        pass_times.push(started.elapsed());
    }

    per_file.sort_unstable();
    let median_pass = median(&mut pass_times.clone());
    let seconds = median_pass.as_secs_f64();
    let summary = format!(
        "in-process: {} files, {bytes} bytes, {passes} passes, {rejected} rejected by the parser\n  \
         median pass      {:.3} ms\n  \
         throughput       {:.1} MB/s   {:.0} files/s\n  \
         per file         p50 {:.3} ms  p95 {:.3} ms  p99 {:.3} ms  max {:.3} ms\n",
        sources.len(),
        median_pass.as_secs_f64() * 1e3,
        bytes as f64 / 1e6 / seconds,
        sources.len() as f64 / seconds,
        percentile(&per_file, 50.0).as_secs_f64() * 1e3,
        percentile(&per_file, 95.0).as_secs_f64() * 1e3,
        percentile(&per_file, 99.0).as_secs_f64() * 1e3,
        per_file.last().copied().unwrap_or_default().as_secs_f64() * 1e3,
    );
    print!("{summary}");
    if rejected > 0 {
        println!("  first rejection  {first_rejection}");
    }

    if let Some(path) = args.get(2) {
        let json = format!(
            "{{\n  \"kind\": \"in-process\",\n  \"files\": {},\n  \"bytes\": {},\n  \"passes\": {},\n  \
             \"rejected_files\": {rejected},\n  \"first_rejection\": \"{}\",\n  \
             \"median_pass_ms\": {:.4},\n  \"megabytes_per_second\": {:.2},\n  \"files_per_second\": {:.1},\n  \
             \"per_file_p50_ms\": {:.4},\n  \"per_file_p95_ms\": {:.4},\n  \"per_file_p99_ms\": {:.4}\n}}\n",
            sources.len(),
            bytes,
            passes,
            first_rejection.replace('"', "'"),
            median_pass.as_secs_f64() * 1e3,
            bytes as f64 / 1e6 / seconds,
            sources.len() as f64 / seconds,
            percentile(&per_file, 50.0).as_secs_f64() * 1e3,
            percentile(&per_file, 95.0).as_secs_f64() * 1e3,
            percentile(&per_file, 99.0).as_secs_f64() * 1e3,
        );
        std::fs::write(path, json)?;
    }

    Ok(())
}

/// Every YAML document under `directory`, in path order.
fn read_corpus(directory: &Path) -> Result<Vec<(PathBuf, String)>, Box<dyn std::error::Error>> {
    let mut paths = Vec::new();
    collect(directory, &mut paths)?;
    paths.sort();

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        sources.push((path.clone(), std::fs::read_to_string(&path)?));
    }
    Ok(sources)
}

/// Collect the YAML files under `directory`, recursively.
fn collect(directory: &Path, into: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, into)?;
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yml" | "yaml")
        ) {
            into.push(path);
        }
    }
    Ok(())
}

/// The middle value of a set of timings.
fn median(times: &mut [Duration]) -> Duration {
    times.sort_unstable();
    times[times.len() / 2]
}

/// The value at `percentile` (0-100) of an already sorted slice.
fn percentile(sorted: &[Duration], percentile: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let rank = (percentile / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}
