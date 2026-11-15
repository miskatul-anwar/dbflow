#![allow(clippy::regex_creation_in_loops)] // `re!` caches each compiled pattern in a OnceLock
//! DBFlow analysis engine.
//!
//! Everything the app computes lives here: CSV and spreadsheet parsing, cleaning and profiling,
//! functional-dependency mining, keys, normal forms (1NF/2NF/3NF/BCNF), the chase, dependency
//! preservation, cross-file foreign keys, schema assembly, ERD/SQL/report generation.
//! The crate compiles natively (Tauri desktop app) and to WebAssembly (browser/dev and tests).

/// A lazily compiled, cached regular expression.
#[macro_export]
macro_rules! re {
    ($p:literal) => {{
        static R: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        R.get_or_init(|| regex::Regex::new($p).expect("valid regex"))
    }};
}

/// Map over items in parallel in the native build (rayon), sequentially in WebAssembly.
pub(crate) fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync + Send) -> Vec<R> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        items.par_iter().map(f).collect()
    }
    #[cfg(target_arch = "wasm32")]
    {
        items.iter().map(f).collect()
    }
}

/// Upper bound on the threads the engine uses (0 = one per CPU core). Set once at start-up.
static THREAD_LIMIT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Threads the engine may use: the configured limit, otherwise one per CPU core.
pub fn engine_threads() -> usize {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    match THREAD_LIMIT.load(std::sync::atomic::Ordering::Relaxed) {
        0 => cores,
        n => n.min(cores),
    }
}

/// Leave `reserved` CPU cores to the rest of the application (the desktop app keeps one for its window, so
/// the interface stays responsive while a large run uses every other core). Call once, before any run.
pub fn reserve_cores(reserved: usize) {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let n = cores.saturating_sub(reserved).max(1);
    THREAD_LIMIT.store(n, std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(target_arch = "wasm32"))]
    {
        // Fails only if the pool already exists; the limit above still bounds the per-file threads.
        let _ = rayon::ThreadPoolBuilder::new().num_threads(n).build_global();
    }
}

/// Memory currently available to new allocations (Linux `MemAvailable`); `None` where it is unknown.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn available_memory_bytes() -> Option<u64> {
    std::fs::read_to_string("/proc/meminfo").ok().and_then(|s| {
        s.lines()
            .find(|l| l.starts_with("MemAvailable:"))
            .and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok())
            .map(|kb| kb * 1024)
    })
}

pub mod analysis;
pub mod api;
pub mod csv;
pub mod data_processor;
pub mod data_utils;
pub mod ddl;
pub mod encoding;
pub mod erd_dot;
pub mod fd_miner;
pub mod identifiers;
pub mod input_check;
pub mod js;
pub mod keys;
pub mod legacy;
pub mod naming;
pub mod normalization;
pub mod pipeline;
pub mod relations;
pub mod reports;
pub mod schema;
pub mod spreadsheet;
pub mod sql_types;
pub mod table;
pub mod types;

pub use api::call;

/// Two-phase runs kept between `prepare` and `finish` (the UI names relations in between).
pub mod session {
    use crate::analysis::Progress;
    use crate::pipeline::{self, InputFile, NamingItem, Prepared};
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    fn store() -> &'static Mutex<HashMap<u64, Prepared>> {
        static S: OnceLock<Mutex<HashMap<u64, Prepared>>> = OnceLock::new();
        S.get_or_init(|| Mutex::new(HashMap::new()))
    }

    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

    /// Analyse the files; returns the session id and the naming questions.
    pub fn prepare(
        files: Vec<InputFile>,
        ops: Vec<String>,
        options: Value,
        progress: &dyn Fn(Progress),
        cancelled: &dyn Fn() -> bool,
    ) -> (u64, Vec<NamingItem>) {
        let p = pipeline::prepare(files, ops, options, progress, cancelled);
        let items = p.naming_requests();
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        store().lock().map(|mut s| s.insert(id, p)).ok();
        (id, items)
    }

    /// Finish a prepared run with the model's answers (or the reason it was unavailable).
    pub fn finish(
        id: u64,
        answers: Option<HashMap<String, String>>,
        namer_error: Option<String>,
        progress: &dyn Fn(Progress),
    ) -> Result<String, String> {
        let p = store()
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&id)
            .ok_or("unknown or finished engine session")?;
        Ok(pipeline::finish(p, answers, namer_error, progress))
    }

    pub fn discard(id: u64) {
        if let Ok(mut s) = store().lock() {
            s.remove(&id);
        }
    }
}
