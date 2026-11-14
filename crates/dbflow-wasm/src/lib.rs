//! WebAssembly bindings: the browser build and the test suite run the same Rust engine as the
//! desktop app. Every function exchanges JSON text, which the page parses once.

use dbflow_core::analysis::Progress;
use dbflow_core::pipeline::InputFile;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

fn wrap(r: Result<Value, String>) -> String {
    match r {
        Ok(v) => json!({ "ok": v }).to_string(),
        Err(e) => json!({ "err": e }).to_string(),
    }
}

fn parse(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or(Value::Null)
}

/// Call any engine method with JSON arguments; returns `{"ok": ...}` or `{"err": "..."}`.
#[wasm_bindgen]
pub fn call(method: &str, args_json: &str) -> String {
    wrap(dbflow_core::call(method, parse(args_json)))
}

fn progress_fn(cb: &js_sys::Function) -> impl Fn(Progress) + '_ {
    move |p: Progress| {
        if let Ok(s) = serde_json::to_string(&p) {
            let _ = cb.call1(&JsValue::NULL, &JsValue::from_str(&s));
        }
    }
}

fn files_from(names_json: &str, contents: &js_sys::Array) -> Vec<InputFile> {
    let names: Vec<String> = serde_json::from_str(names_json).unwrap_or_default();
    names
        .into_iter()
        .enumerate()
        .map(|(i, name)| InputFile {
            name,
            bytes: js_sys::Uint8Array::new(&contents.get(i as u32)).to_vec(),
        })
        .collect()
}

/// Analyse files (phase 1). Returns `{"session": id, "naming": [{key, columns}]}`.
#[wasm_bindgen]
pub fn prepare(
    names_json: &str,
    contents: &js_sys::Array,
    ops_json: &str,
    options_json: &str,
    progress: &js_sys::Function,
) -> String {
    let files = files_from(names_json, contents);
    let ops: Vec<String> = serde_json::from_str(ops_json).unwrap_or_default();
    let cb = progress_fn(progress);
    let never = || false;
    let (id, naming) = dbflow_core::session::prepare(files, ops, parse(options_json), &cb, &never);
    json!({ "session": id, "naming": naming }).to_string()
}

/// Build the outputs of a prepared run (phase 2). `answers_json` is `{key: text}` or empty.
#[wasm_bindgen]
pub fn finish(session: f64, answers_json: &str, namer_error: &str, progress: &js_sys::Function) -> String {
    let answers = serde_json::from_str(answers_json).ok();
    let err = if namer_error.is_empty() {
        None
    } else {
        Some(namer_error.to_string())
    };
    let cb = progress_fn(progress);
    match dbflow_core::session::finish(session as u64, answers, err, &cb) {
        Ok(result) => format!("{{\"ok\":{result}}}"),
        Err(e) => json!({ "err": e }).to_string(),
    }
}

#[wasm_bindgen]
pub fn discard(session: f64) {
    dbflow_core::session::discard(session as u64);
}

/// Decode a file's bytes (BOM / UTF-16 / UTF-8 / Windows-1252) and parse it as CSV.
#[wasm_bindgen]
pub fn parse_csv_bytes(bytes: &[u8], options_json: &str) -> String {
    let (text, _) = dbflow_core::encoding::decode_bytes(bytes);
    wrap(dbflow_core::call("csv.parse", json!([text, parse(options_json)])))
}

#[wasm_bindgen]
pub fn decode_bytes(bytes: &[u8]) -> String {
    let (text, enc) = dbflow_core::encoding::decode_bytes(bytes);
    json!({ "text": text, "encoding": enc.label() }).to_string()
}

#[wasm_bindgen]
pub fn detect_encoding(bytes: &[u8]) -> String {
    dbflow_core::encoding::detect_encoding(bytes).label().to_string()
}

/// Workbook to one CSV per visible sheet: `{"ok": {"files": [{name, text}], "notes": [...]}}`.
#[wasm_bindgen]
pub fn spreadsheet_to_csv(name: &str, bytes: &[u8]) -> String {
    wrap(dbflow_core::spreadsheet::spreadsheet_to_csv(name, bytes).map(|c| {
        json!({
            "files": c.files.into_iter().map(|(n, b)| json!({"name": n, "text": String::from_utf8_lossy(&b)})).collect::<Vec<_>>(),
            "notes": c.notes,
        })
    }))
}

#[wasm_bindgen]
pub fn is_spreadsheet(name: &str, bytes: &[u8]) -> bool {
    dbflow_core::spreadsheet::is_spreadsheet(name, bytes)
}
