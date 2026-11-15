//! DBFlow — Tauri application entry point.
//!
//! All analysis (CSV and spreadsheet parsing, functional-dependency mining, normalization,
//! foreign-key detection, ERD/SQL/report generation) runs in the Rust engine
//! (`crates/dbflow-core`), linked natively here. The webview only renders the UI.
//!
//! A run is driven by four commands:
//!   `engine_upload`   one file's raw bytes (binary IPC body, name in the `x-file-name` header, run in `x-run-id`);
//!   `engine_prepare`  parse and analyse the uploads on a background thread, streaming progress;
//!   `engine_finish`   apply relation names from the on-device model and build every output;
//!   `engine_cancel`   stop a running `engine_prepare`.
//! `engine_call` exposes any single engine function (JSON in, JSON out).

use dbflow_core::analysis::Progress;
use dbflow_core::pipeline::InputFile;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request, Response};
use tauri::State;

#[derive(Default)]
struct Engine {
    /// Uploaded files by id, with the run they belong to (so a cancelled run's files are dropped).
    uploads: Mutex<HashMap<u64, (String, InputFile)>>,
    cancels: Mutex<HashMap<String, Arc<AtomicBool>>>,
    next_upload: AtomicU64,
}

fn decode_name(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&raw[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Async so the copy of a large file never runs on the main thread (Tauri runs plain commands there, and the
/// window cannot repaint or take input while one is busy).
#[tauri::command]
async fn engine_upload(request: Request<'_>, engine: State<'_, Engine>) -> Result<u64, String> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the file bytes as the request body".into());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(decode_name)
        .unwrap_or_else(|| "data.csv".into());
    let run = request
        .headers()
        .get("x-run-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let id = engine.next_upload.fetch_add(1, Ordering::Relaxed) + 1;
    engine.uploads.lock().map_err(|e| e.to_string())?.insert(
        id,
        (
            run,
            InputFile {
                name,
                bytes: bytes.clone(),
            },
        ),
    );
    Ok(id)
}

#[tauri::command]
async fn engine_prepare(
    run_id: String,
    uploads: Vec<u64>,
    operations: Vec<String>,
    options: Value,
    on_progress: Channel<Progress>,
    engine: State<'_, Engine>,
) -> Result<Value, String> {
    let files: Vec<InputFile> = {
        let mut store = engine.uploads.lock().map_err(|e| e.to_string())?;
        uploads
            .iter()
            .filter_map(|id| store.remove(id).map(|(_, f)| f))
            .collect()
    };
    let flag = Arc::new(AtomicBool::new(false));
    engine
        .cancels
        .lock()
        .map_err(|e| e.to_string())?
        .insert(run_id.clone(), flag.clone());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let progress = |p: Progress| {
            let _ = on_progress.send(p);
        };
        let cancelled = || flag.load(Ordering::Relaxed);
        let (session, naming) = dbflow_core::session::prepare(files, operations, options, &progress, &cancelled);
        json!({ "session": session, "naming": naming })
    })
    .await
    .map_err(|e| e.to_string());
    engine.cancels.lock().map_err(|e| e.to_string())?.remove(&run_id);
    result
}

#[tauri::command]
async fn engine_finish(
    session: u64,
    answers: Option<HashMap<String, String>>,
    namer_error: Option<String>,
    on_progress: Channel<Progress>,
) -> Result<Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let progress = |p: Progress| {
            let _ = on_progress.send(p);
        };
        // The result is already JSON text; hand it to the webview without re-encoding.
        dbflow_core::session::finish(session, answers, namer_error, &progress)
            .map(|json| Response::new(InvokeResponseBody::Json(json)))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn engine_cancel(run_id: String, engine: State<'_, Engine>) {
    if let Ok(c) = engine.cancels.lock() {
        if let Some(flag) = c.get(&run_id) {
            flag.store(true, Ordering::Relaxed);
        }
    }
    // Files uploaded for a run that was cancelled before it started would otherwise stay in memory.
    if let Ok(mut store) = engine.uploads.lock() {
        store.retain(|_, (run, _)| *run != run_id);
    }
}

#[tauri::command]
async fn engine_call(method: String, args: Value) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || dbflow_core::call(&method, args))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // One core stays free for the window, so tabs, dialogs and other workspaces respond during a large run.
    dbflow_core::reserve_cores(1);
    tauri::Builder::default()
        .manage(Engine::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            engine_upload,
            engine_prepare,
            engine_finish,
            engine_cancel,
            engine_call
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_encoded_names() {
        assert_eq!(decode_name("caf%C3%A9%20orders.csv"), "café orders.csv");
        assert_eq!(decode_name("plain.csv"), "plain.csv");
    }

    #[test]
    fn native_pipeline_runs() {
        let csv = "order_id,customer,city\n1,Ann,Paris\n2,Bob,Rome\n3,Ann,Paris\n";
        let result = dbflow_core::pipeline::process(
            vec![InputFile {
                name: "orders.csv".into(),
                bytes: csv.as_bytes().to_vec(),
            }],
            vec![
                "Data Loader".into(),
                "FD Miner".into(),
                "Key Discovery".into(),
                "3NF".into(),
                "SQL Generator".into(),
            ],
            json!({}),
        );
        assert_eq!(result["success"], json!(true), "{result}");
        assert_eq!(result["engine"], json!("rust-native"));
    }
}
