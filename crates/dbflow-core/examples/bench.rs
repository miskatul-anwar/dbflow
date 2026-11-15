//! `cargo run --release -p dbflow-core --example bench -- file.csv [more.csv]`: times a full run and
//! reports peak memory (Linux). Set `DBFLOW_BENCH_OUT=result.json` to keep the result.
use dbflow_core::analysis::Progress;
use dbflow_core::pipeline::{finish, prepare, InputFile};
use std::time::Instant;

fn main() {
    let files: Vec<InputFile> = std::env::args()
        .skip(1)
        .map(|p| InputFile {
            name: p.rsplit('/').next().unwrap_or(&p).to_string(),
            bytes: std::fs::read(&p).expect("read file"),
        })
        .collect();
    let ops = [
        "Data Loader",
        "Data Cleaner",
        "Atomicity Checker",
        "Data Type Profiler",
        "Column Classifier",
        "1NF",
        "FD Miner",
        "Key Discovery",
        "Cardinality Analyzer",
        "2NF",
        "3NF",
        "BCNF",
        "Lossless Check",
        "Dependency Preservation",
        "Relation Detector",
        "ERD Generator",
        "Relation Generator",
        "SQL Generator",
        "Report Generator",
    ];
    let last = std::cell::RefCell::new(Instant::now());
    let start = Instant::now();
    let progress = |p: Progress| {
        let now = Instant::now();
        eprintln!(
            "{:>8.1} ms  (+{:>7.1})  {:>6} MB  {}",
            (now - start).as_secs_f64() * 1e3,
            (now - *last.borrow()).as_secs_f64() * 1e3,
            mem_mb("VmRSS:"),
            p.message
        );
        *last.borrow_mut() = now;
    };
    let never = || false;
    let p = prepare(
        files,
        ops.iter().map(|s| s.to_string()).collect(),
        // DBFLOW_BENCH_OPTIONS='{"maxParallelFiles":1}' passes engine options.
        std::env::var("DBFLOW_BENCH_OPTIONS")
            .ok()
            .and_then(|o| serde_json::from_str(&o).ok())
            .unwrap_or_else(|| serde_json::json!({})),
        &progress,
        &never,
    );
    let t1 = start.elapsed();
    eprintln!("peak memory after prepare: {} MB", peak_mb());
    let json = finish(p, None, None, &progress);
    let t2 = start.elapsed();
    eprintln!(
        "prepare {:?}  finish {:?}  ({} MB of JSON)  success={}",
        t1,
        t2 - t1,
        json.len() / 1_000_000,
        json.starts_with("{\"success\":true")
    );
    eprintln!("peak memory: {} MB", peak_mb());
    // DBFLOW_BENCH_OUT=result.json keeps the result for inspection.
    if let Ok(path) = std::env::var("DBFLOW_BENCH_OUT") {
        std::fs::write(&path, &json).expect("write result");
    }
}

/// Peak resident memory of this process, in MB (Linux; 0 elsewhere).
fn peak_mb() -> u64 {
    mem_mb("VmHWM:")
}

/// A memory figure of this process from /proc/self/status, in MB (Linux; 0 elsewhere).
fn mem_mb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with(field))
                .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
        })
        .map(|kb| kb / 1024)
        .unwrap_or(0)
}
