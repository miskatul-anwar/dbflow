//! `cargo run --release -p dbflow-core --example bench -- file.csv [more.csv]`: times a full run.
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
        "CSV Data Loader",
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
            "{:>8.1} ms  (+{:>7.1})  {}",
            (now - start).as_secs_f64() * 1e3,
            (now - *last.borrow()).as_secs_f64() * 1e3,
            p.message
        );
        *last.borrow_mut() = now;
    };
    let never = || false;
    let p = prepare(
        files,
        ops.iter().map(|s| s.to_string()).collect(),
        serde_json::json!({}),
        &progress,
        &never,
    );
    let t1 = start.elapsed();
    let json = finish(p, None, None, &progress);
    let t2 = start.elapsed();
    eprintln!(
        "prepare {:?}  finish {:?}  ({} MB of JSON)  success={}",
        t1,
        t2 - t1,
        json.len() / 1_000_000,
        json.starts_with("{\"success\":true")
    );
}
