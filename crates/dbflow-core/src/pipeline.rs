//! Whole-run orchestration (`processFiles`): read files, analyse each table, name relations,
//! join tables into one schema and produce every output.
//!
//! A run has two phases so the UI can name relations with its on-device model in between:
//! [`prepare`] does all the analysis and returns the naming questions; [`finish`] takes the
//! answers and builds the outputs.

use crate::analysis::{self, analyze_rows, suggest_primary_key, Ctx, Options, Progress, TableAnalysis};
use crate::csv;
use crate::ddl::{generate_ddl, rows_to_csv, Dialect};
use crate::encoding::{decode_bytes, Encoding};
use crate::identifiers::IdentifierRegistry;
use crate::input_check::{self, InputChecks};
use crate::js;
use crate::legacy;
use crate::naming::{
    accept_model_names, accept_rule_names, name_relation_set, plan_names, reset_relation_ids, ChosenName, NamableTable,
};
use crate::relations::{detect_foreign_keys, DetectorTable};
use crate::reports;
use crate::schema::{
    build_schema, emit_mermaid_erd, export_normalized_data_with, to_decomposed_relations, TableSchemaInput,
};
use crate::spreadsheet;
use crate::table::Table;
use crate::types::{CrossLink, Relation};
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::value::RawValue;
use serde_json::{json, Value};
use std::collections::HashMap;

const HARD_ROW_LIMIT: usize = 1_000_000;
const STREAMING_THRESHOLD: usize = 10 * 1024 * 1024;

pub struct InputFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Serialize)]
pub struct NamingItem {
    pub key: String,
    pub columns: Vec<String>,
}

struct Logger {
    entries: Vec<(f64, &'static str, String, Option<Value>)>,
}

impl Logger {
    fn new() -> Logger {
        Logger { entries: vec![] }
    }
    fn add(&mut self, level: &'static str, msg: impl Into<String>, data: Option<Value>) {
        self.entries.push((js::now_ms(), level, msg.into(), data));
    }
    fn info(&mut self, m: impl Into<String>) {
        self.add("INFO", m, None)
    }
    fn success(&mut self, m: impl Into<String>) {
        self.add("SUCCESS", m, None)
    }
    fn warn(&mut self, m: impl Into<String>) {
        self.add("WARN", m, None)
    }
    fn lines(&self, tz: f64) -> Vec<String> {
        self.entries
            .iter()
            .map(|(t, l, m, d)| {
                let data = match d {
                    Some(v) if !v.is_null() => {
                        format!(" - {}", serde_json::to_string(v).unwrap_or_default())
                    }
                    _ => String::new(),
                };
                format!("[{}] [{l}] {m}{data}", js::locale_time(*t, tz))
            })
            .collect()
    }
}

/// Current name of an operation; earlier names still work (saved pipelines, scripts).
fn canonical(op: &str) -> String {
    match op {
        "NER Classifier" => "Column Classifier".into(),
        "CSV Data Loader" => "Data Loader".into(),
        _ => op.to_string(),
    }
}

fn has(ops: &[String], name: &str) -> bool {
    ops.iter().any(|o| o == name)
}

fn duration(start: f64) -> String {
    format!("{}s", js::to_fixed((js::now_ms() - start) / 1000.0, 2))
}

fn strip_extension(name: &str) -> String {
    re!(r"\.[^/.]+$").replace(name, "").to_string()
}

fn footprint_mb(rows: &Table) -> f64 {
    if rows.is_empty() {
        return 0.0;
    }
    let step = (rows.len() / 200).max(1);
    let (mut bytes, mut n) = (0f64, 0f64);
    let mut i = 0;
    while i < rows.len() {
        for k in rows.keys_of(i) {
            bytes += 16.0
                + match rows.get(i, &k) {
                    crate::table::Cell::Str(s) => js::len16(s) as f64 * 2.0,
                    _ => 8.0,
                };
        }
        n += 1.0;
        i += step;
    }
    bytes / n * rows.len() as f64 / 1024.0 / 1024.0
}

struct Loaded {
    rows: Table,
    size: usize,
}

/// `processCSVWithStreaming`: large files use the streaming reader, smaller ones the full parser.
fn load_csv(file: &InputFile, log: &mut Logger) -> Result<Loaded, String> {
    if file.bytes.len() > STREAMING_THRESHOLD {
        log.info("Large file detected, using streaming CSV processing");
        let enc = crate::encoding::detect_encoding(&file.bytes[..file.bytes.len().min(65_536)]);
        if enc != Encoding::Utf8 {
            log.info(format!("{}: detected {} encoding", file.name, enc.label()));
        }
        let (text, _) = decode_bytes(&file.bytes);
        let (rows, _, truncated) = csv::parse_streaming(&text, HARD_ROW_LIMIT, None);
        if truncated {
            log.warn(format!(
                "{}: more than {} rows — only the first {} are analysed.",
                file.name,
                js::locale_usize(rows.len()),
                js::locale_usize(rows.len())
            ));
        }
        return Ok(Loaded {
            rows,
            size: file.bytes.len(),
        });
    }
    let (text, _) = decode_bytes(&file.bytes);
    let parsed = csv::parse(
        &text,
        &csv::CsvOptions {
            file_name: Some(file.name.clone()),
            ..Default::default()
        },
    )?;
    Ok(Loaded {
        rows: parsed.data,
        size: file.bytes.len(),
    })
}

fn quality(data: &Table) -> Value {
    if data.is_empty() {
        return json!({"score": 0, "issues": ["No data available"], "recommendations": ["Upload a valid CSV file with data"]});
    }
    let attrs = data.keys();
    let n = data.len().min(1000);
    let mut missing = 0.0;
    for r in 0..n {
        let m = attrs.iter().filter(|a| data.get(r, a).is_empty_like()).count();
        missing += m as f64 / attrs.len() as f64;
    }
    let ratio = missing / n as f64;
    let mut score: i64 = 100;
    let (mut issues, mut recs) = (vec![], vec![]);
    if ratio > 0.1 {
        score -= 20;
        issues.push(format!("High missing value ratio: {}%", js::to_fixed(ratio * 100.0, 1)));
        recs.push("Consider data cleaning to handle missing values".to_string());
    }
    let uniq: std::collections::HashSet<String> = (0..n)
        .map(|r| serde_json::to_string(&Value::Object(data.row_json(r))).unwrap_or_default())
        .collect();
    let dup = 1.0 - uniq.len() as f64 / n as f64;
    if dup > 0.05 {
        score -= 15;
        issues.push(format!("Duplicate rows detected: {}%", js::to_fixed(dup * 100.0, 1)));
        recs.push("Remove duplicate rows to improve analysis quality".to_string());
    }
    json!({"score": score.max(0), "issues": issues, "recommendations": recs})
}

type Raw = Box<RawValue>;

/// Serialize once into a raw JSON fragment (embedded later without re-encoding).
fn raw<T: Serialize + ?Sized>(v: &T) -> Raw {
    serde_json::value::to_raw_value(v).unwrap_or_else(|_| RawValue::from_string("null".into()).expect("valid JSON"))
}

/// A JSON object built from raw fragments, in field order.
fn object(fields: Vec<(&str, Raw)>) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&serde_json::to_string(k).unwrap_or_default());
        out.push(':');
        out.push_str(v.get());
    }
    out.push('}');
    out
}

/// `JSON.stringify(v).length` without building the string (counts UTF-16 code units).
fn json_len(v: &impl Serialize) -> usize {
    struct Counter(usize);
    impl std::io::Write for Counter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            for &b in buf {
                if b & 0xC0 != 0x80 {
                    self.0 += 1;
                }
                if b >= 0xF0 {
                    self.0 += 1;
                }
            }
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut c = Counter(0);
    let _ = serde_json::to_writer(&mut c, v);
    c.0
}

#[derive(Serialize)]
struct OutFile<'a> {
    name: &'a str,
    #[serde(rename = "type")]
    kind: &'a str,
    description: &'a str,
    content: Raw,
    size: usize,
    category: &'a str,
}

fn out_file(name: &str, kind: &str, description: &str, content: Raw, size: usize, category: &str) -> Raw {
    raw(&OutFile {
        name,
        kind,
        description,
        content,
        size,
        category,
    })
}

fn text(s: impl Into<String>) -> Raw {
    raw(&s.into())
}

/// Relations without their rows (`{ ...r, rows: undefined }`).
fn rels_without_rows(rels: &[Relation]) -> Raw {
    raw(&rels.iter().map(|r| r.without_rows()).collect::<Vec<_>>())
}

/// Inputs above this many cells (rows × columns) get a compact result: relations carry `rowCount`
/// instead of their rows, and each relation's data is exported once, as CSV, instead of several times.
pub const COMPACT_RESULT_CELLS: usize = 999_999;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CompactRelation<'a> {
    #[serde(flatten)]
    relation: crate::types::RelationNoRows<'a>,
    rows: [u8; 0],
    row_count: usize,
}

/// Relations for the result JSON: with their rows (small inputs) or with `rowCount` only (large inputs).
fn rels(set: &[Relation], compact: bool) -> Raw {
    if !compact {
        return raw(set);
    }
    raw(&set
        .iter()
        .map(|r| CompactRelation {
            relation: r.without_rows(),
            rows: [],
            row_count: r.row_count(),
        })
        .collect::<Vec<_>>())
}

/// Which normal-form sets a table names: the final set and the distinct others.
#[derive(Clone, Copy, PartialEq)]
enum SetId {
    One,
    Two,
    Three,
    Bcnf,
    Four,
}

fn set_ref(a: &TableAnalysis, id: SetId) -> &[Relation] {
    match id {
        SetId::One => &a.relations_1nf,
        SetId::Two => &a.relations_2nf,
        SetId::Three => &a.relations_3nf,
        SetId::Bcnf => &a.relations_bcnf,
        SetId::Four => &a.relations_4nf,
    }
}

fn set_mut(a: &mut TableAnalysis, id: SetId) -> &mut Vec<Relation> {
    match id {
        SetId::One => &mut a.relations_1nf,
        SetId::Two => &mut a.relations_2nf,
        SetId::Three => &mut a.relations_3nf,
        SetId::Bcnf => &mut a.relations_bcnf,
        SetId::Four => &mut a.relations_4nf,
    }
}

fn naming_sets(a: &TableAnalysis, multi: bool) -> (SetId, Vec<SetId>) {
    let fin = if !multi || !a.relations_3nf.is_empty() {
        SetId::Three
    } else {
        SetId::Two
    };
    let mut others = Vec::new();
    for id in [SetId::One, SetId::Two, SetId::Three, SetId::Bcnf, SetId::Four] {
        if id == fin {
            continue;
        }
        // A set that is the final set itself (held in the 3NF slot) is named with it, not on its own.
        let same_as_three = fin == SetId::Three || others.contains(&SetId::Three);
        if id == SetId::Bcnf && a.bcnf_is_final && same_as_three {
            continue;
        }
        if id == SetId::Four && a.fourth_is_final && same_as_three {
            continue;
        }
        others.push(id);
    }
    (fin, others)
}

struct NamedTable {
    name: String,
    pk: Vec<String>,
    analysis: TableAnalysis,
}

fn plan(
    tables: &[NamedTable],
    multi: bool,
    user: &HashMap<String, String>,
) -> (HashMap<String, ChosenName>, IndexMap<String, Vec<String>>) {
    let namable: Vec<NamableTable> = tables
        .iter()
        .map(|t| {
            let (fin, others) = naming_sets(&t.analysis, multi);
            NamableTable {
                table_name: t.name.clone(),
                primary_key: t.pk.clone(),
                final_set: Some(set_ref(&t.analysis, fin)),
                others: others.into_iter().map(|o| set_ref(&t.analysis, o)).collect(),
            }
        })
        .collect();
    plan_names(&namable, user)
}

fn apply(tables: &mut [NamedTable], multi: bool, chosen: &HashMap<String, ChosenName>) {
    let mut registry = IdentifierRegistry::new();
    for t in tables.iter_mut() {
        let (fin, _) = naming_sets(&t.analysis, multi);
        name_relation_set(
            set_mut(&mut t.analysis, fin),
            &t.name,
            &t.pk,
            Some(chosen),
            &mut registry,
        );
    }
    for t in tables.iter_mut() {
        let (_, others) = naming_sets(&t.analysis, multi);
        for o in others {
            name_relation_set(
                set_mut(&mut t.analysis, o),
                &t.name,
                &t.pk,
                Some(chosen),
                &mut IdentifierRegistry::new(),
            );
        }
        if t.analysis.bcnf_is_final {
            t.analysis.relations_bcnf = t.analysis.relations_3nf.clone();
        }
        if t.analysis.fourth_is_final {
            t.analysis.relations_4nf = t.analysis.relations_3nf.clone();
        }
        label_mvd_relations(&mut t.analysis);
    }
}

/// MVDs record the relation they were found in by its id; once relations are named, report the name people see
/// (the BCNF relation with the same attributes), or the attribute list for a relation split along the way.
fn label_mvd_relations(a: &mut TableAnalysis) {
    let names: Vec<(std::collections::BTreeSet<String>, String)> = a
        .relations_bcnf
        .iter()
        .map(|r| {
            (
                r.attributes.iter().cloned().collect(),
                r.display_name.clone().unwrap_or_else(|| r.name.clone()),
            )
        })
        .collect();
    for m in &mut a.mvds {
        let attrs: std::collections::BTreeSet<String> = m.lhs.iter().chain(&m.rhs).chain(&m.rest).cloned().collect();
        m.relation = match names.iter().find(|(set, _)| *set == attrs) {
            Some((_, name)) => name.clone(),
            None => format!(
                "({})",
                m.lhs
                    .iter()
                    .chain(&m.rhs)
                    .chain(&m.rest)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
    }
}

struct SingleRun {
    log: Logger,
    checkpoints: Vec<(String, f64, f64)>,
    file_size: usize,
    original_rows: usize,
    quality: Value,
    memory_mb: f64,
}

enum Mode {
    Failed(Value),
    Single(SingleRun, Vec<NamedTable>),
    Multi(Vec<String>, Vec<(String, NamedTable)>),
}

pub struct Prepared {
    mode: Mode,
    ops: Vec<String>,
    options: Options,
    raw_options: Value,
    start: f64,
    chosen: HashMap<String, ChosenName>,
    to_ask: IndexMap<String, Vec<String>>,
    pre_notes: Vec<String>,
    input_checks: InputChecks,
}

impl Prepared {
    pub fn naming_requests(&self) -> Vec<NamingItem> {
        self.to_ask
            .iter()
            .map(|(k, c)| NamingItem {
                key: k.clone(),
                columns: c.clone(),
            })
            .collect()
    }
}

fn multi_failure(msg: &str, start: f64) -> Value {
    let d = duration(start);
    json!({"success": false, "error": msg, "outputFiles": [], "logs": [format!("[ERROR] {msg}")], "duration": d, "stats": {"processDuration": d}})
}

/// What remains of the uploaded files after the input checks, ready for analysis.
struct Admitted {
    files: Vec<InputFile>,
    /// How each file is named in messages (repeated names get "(file k)").
    labels: Vec<String>,
    notes: Vec<String>,
    checks: InputChecks,
}

/// Run the input checks: reject files that are not tables (whatever their extension), expand
/// workbooks into one CSV per sheet, set aside exact duplicates, and find (or merge) files that
/// overlap. A bad file is skipped with its reason; the run continues with the others.
fn admit_inputs(files: Vec<InputFile>, options: &Options) -> Admitted {
    let mut notes = Vec::new();
    let mut checks = InputChecks {
        skipped: options.skipped_inputs.clone().unwrap_or_default(),
        ..Default::default()
    };
    let names: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
    let upload_labels = input_check::labels(&names);
    let mut kept = Vec::new();
    for (f, label) in files.into_iter().zip(upload_labels) {
        let admitted = (|| -> Result<Vec<InputFile>, String> {
            input_check::reject_known_non_table(&label, &f.bytes)?;
            let is_book = spreadsheet::is_spreadsheet(&f.name, &f.bytes);
            if is_book || input_check::is_container(&f.bytes) {
                let conv = spreadsheet::spreadsheet_to_csv(&f.name, &f.bytes).map_err(|e| {
                    if is_book {
                        e
                    } else {
                        format!("{label} is not a table: {e}")
                    }
                })?;
                if !is_book {
                    notes.push(format!("{label} is really a workbook; it was read as one."));
                }
                notes.extend(conv.notes);
                return Ok(conv
                    .files
                    .into_iter()
                    .map(|(name, bytes)| InputFile { name, bytes })
                    .collect());
            }
            input_check::check_text_table(&label, &f.bytes)?;
            Ok(vec![f])
        })();
        match admitted {
            Ok(fs) => kept.extend(fs),
            Err(reason) => checks.skipped.push(input_check::Skipped { file: label, reason }),
        }
    }
    // Exact duplicates: the first copy is analysed, later copies are set aside.
    let names: Vec<String> = kept.iter().map(|f| f.name.clone()).collect();
    let mut labels = input_check::labels(&names);
    let mut first_of: HashMap<(usize, u64), String> = HashMap::new();
    let mut files = Vec::with_capacity(kept.len());
    let mut unique_labels = Vec::with_capacity(kept.len());
    for (f, label) in kept.into_iter().zip(labels.drain(..)) {
        let fp = input_check::fingerprint(&f.bytes);
        if let Some(original) = first_of.get(&fp) {
            checks.duplicates.push(input_check::Duplicate {
                file: label,
                same_as: original.clone(),
            });
            continue;
        }
        first_of.insert(fp, label.clone());
        files.push(f);
        unique_labels.push(label);
    }
    let mut labels = unique_labels;
    if files.len() > 1 {
        checks.overlaps = input_check::find_overlaps(&files, &labels);
        if options.merge_overlapping_files.unwrap_or(false) && !checks.overlaps.is_empty() {
            let (f, l, merged) = input_check::merge_overlapping(files, labels, &checks.overlaps);
            files = f;
            labels = l;
            checks.merged = merged;
        }
    }
    for s in &checks.skipped {
        notes.push(format!("Skipped: {}", s.reason));
    }
    for d in &checks.duplicates {
        notes.push(format!(
            "{} has the same contents as {}; it was analysed once.",
            d.file, d.same_as
        ));
    }
    for o in &checks.overlaps {
        let mut msg = format!(
            "{} and {} have the same columns and share {} of their rows",
            o.files[0], o.files[1], o.shared_rows
        );
        if let Some(k) = &o.key {
            msg.push_str(&format!(
                "; {} records match on {k}, {} of them with different values",
                o.shared_keys, o.conflicts
            ));
        }
        msg.push_str(if checks.merged.is_empty() {
            ". They are analysed as separate tables; merge them to analyse one."
        } else {
            "."
        });
        notes.push(msg);
    }
    for m in &checks.merged {
        let mut msg = format!("{} merged into {}: {} distinct rows", m.from.join(", "), m.into, m.rows);
        if m.conflicts_resolved > 0 {
            msg.push_str(&format!(
                "; for {} conflicting records the version in {} was kept",
                m.conflicts_resolved, m.into
            ));
        }
        msg.push('.');
        notes.push(msg);
    }
    Admitted {
        files,
        labels,
        notes,
        checks,
    }
}

/// Why nothing could be analysed: the single file's reason, or every file's.
fn nothing_left(checks: &InputChecks) -> String {
    match checks.skipped.as_slice() {
        [] => "None of the uploaded files contained data rows.".into(),
        [one] => one.reason.clone(),
        many => format!(
            "None of the uploaded files could be analysed. {}",
            many.iter().map(|s| s.reason.as_str()).collect::<Vec<_>>().join(" ")
        ),
    }
}

/// Add the input-check report to a result (JSON object text).
fn with_input_checks(result: String, checks: &InputChecks) -> String {
    if checks.is_empty() {
        return result;
    }
    let Some(end) = result.rfind('}') else {
        return result;
    };
    let report = serde_json::to_string(checks).unwrap_or_else(|_| "null".into());
    let sep = if result[..end].trim_end().ends_with('{') {
        ""
    } else {
        ","
    };
    format!("{}{sep}\"inputChecks\":{report}}}", &result[..end])
}

pub fn prepare(
    files: Vec<InputFile>,
    ops: Vec<String>,
    raw_options: Value,
    progress: &dyn Fn(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Prepared {
    let ops: Vec<String> = ops.iter().map(|o| canonical(o)).collect();
    let options: Options = serde_json::from_value(raw_options.clone()).unwrap_or_default();
    let start = js::now_ms();
    reset_relation_ids();
    let mut prepared = Prepared {
        mode: Mode::Failed(Value::Null),
        ops: ops.clone(),
        options,
        raw_options,
        start,
        chosen: HashMap::new(),
        to_ask: IndexMap::new(),
        pre_notes: vec![],
        input_checks: InputChecks::default(),
    };
    let skipped_by_interface = prepared.options.skipped_inputs.as_ref().is_some_and(|s| !s.is_empty());
    if files.is_empty() && !skipped_by_interface {
        prepared.mode = Mode::Failed(multi_failure("No files uploaded for processing.", start));
        return prepared;
    }
    let Admitted {
        files,
        labels,
        notes,
        checks,
    } = admit_inputs(files, &prepared.options);
    prepared.input_checks = checks;
    let mut all_notes = prepared.options.input_notes.clone().unwrap_or_default();
    all_notes.extend(notes);
    prepared.pre_notes = all_notes;
    if files.is_empty() {
        let msg = nothing_left(&prepared.input_checks);
        prepared.mode = Mode::Failed(multi_failure(&msg, start));
        return prepared;
    }
    let user = prepared.options.user_names.clone().unwrap_or_default();
    if files.len() == 1 {
        let file = files.into_iter().next().unwrap();
        match prepare_single(&file, &prepared, progress, cancelled) {
            Ok((run, tables)) => {
                let (c, a) = plan(&tables, false, &user);
                prepared.chosen = c;
                prepared.to_ask = a;
                prepared.mode = Mode::Single(run, tables);
            }
            Err(v) => prepared.mode = Mode::Failed(v),
        }
        return prepared;
    }
    match prepare_multi(files, labels, &prepared, progress, cancelled) {
        Ok((logs, tables, skipped)) => {
            prepared.input_checks.skipped.extend(skipped);
            let refs: Vec<&NamedTable> = tables.iter().map(|(_, t)| t).collect();
            let namable: Vec<NamableTable> = refs
                .iter()
                .map(|t| {
                    let (fin, others) = naming_sets(&t.analysis, true);
                    NamableTable {
                        table_name: t.name.clone(),
                        primary_key: t.pk.clone(),
                        final_set: Some(set_ref(&t.analysis, fin)),
                        others: others.into_iter().map(|o| set_ref(&t.analysis, o)).collect(),
                    }
                })
                .collect();
            let (c, a) = plan_names(&namable, &user);
            prepared.chosen = c;
            prepared.to_ask = a;
            prepared.mode = Mode::Multi(logs, tables);
        }
        Err(e) => {
            prepared.mode = Mode::Failed(multi_failure(&e, start));
        }
    }
    prepared
}

fn prepare_single(
    file: &InputFile,
    p: &Prepared,
    progress: &dyn Fn(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Result<(SingleRun, Vec<NamedTable>), Value> {
    let mut run = SingleRun {
        log: Logger::new(),
        checkpoints: vec![],
        file_size: file.bytes.len(),
        original_rows: 0,
        quality: Value::Null,
        memory_mb: 0.0,
    };
    for n in &p.pre_notes {
        run.log.warn(n.clone());
    }
    let start = p.start;
    let mut opts = p.raw_options.clone();
    if let Value::Object(m) = &mut opts {
        m.entry("enableParallelProcessing").or_insert(Value::Bool(true));
        m.entry("adaptiveThresholds").or_insert(Value::Bool(true));
        let mb = file.bytes.len() as f64 / (1024.0 * 1024.0);
        let size = (10_000f64).max(10_000.0 * mb.log10()).min(50_000.0);
        let size = if size.is_nan() { 10_000.0 } else { size };
        m.entry("maxSampleSize").or_insert(crate::table::num_json(size));
        m.remove("tzOffsetMinutes");
        m.remove("inputNotes");
    }
    run.log.add(
        "INFO",
        "Starting optimized database normalization process...",
        Some(json!({"selectedOperations": p.ops, "options": opts})),
    );
    let checkpoint = |run: &mut SingleRun, name: &str| {
        let t = js::now_ms() - start;
        run.checkpoints.push((name.to_string(), t.round(), run.memory_mb));
    };
    checkpoint(&mut run, "Phase 1 Start: Data Preparation");
    run.log.info("Phase 1: Optimized Data Preparation");
    progress(Progress {
        phase: "PREP".into(),
        operation: "csv-loader".into(),
        progress: 5.0,
        message: "Loading CSV data with streaming...".into(),
        memory_usage: Some(0.0),
        ..Default::default()
    });
    let fail = |run: &mut SingleRun, msg: String| -> Value {
        run.log.add(
            "ERROR",
            "Optimized database normalization process failed",
            Some(json!({"name": "Error", "message": msg})),
        );
        let d = duration(start);
        let cps: Vec<Value> = run
            .checkpoints
            .iter()
            .map(|(n, t, m)| json!({"name": n, "time": t, "memory": m}))
            .collect();
        json!({
            "success": false, "error": msg,
            "stats": {"processDuration": d, "memoryUsage": format!("{} MB", js::to_fixed(run.memory_mb, 2)),
                "performanceMetrics": {"totalDuration": d, "checkpoints": cps, "errorOccurred": true}},
            "outputFiles": [], "logs": run.log.lines(p.options.tz_offset_minutes.unwrap_or(0.0)),
            "duration": d, "selectedOperations": p.ops,
        })
    };
    let loaded = match load_csv(file, &mut run.log) {
        Ok(l) => l,
        Err(e) => return Err(fail(&mut run, format!("Enhanced CSV processing failed: {e}"))),
    };
    let rows = loaded.rows;
    run.file_size = loaded.size;
    run.original_rows = rows.len();
    run.memory_mb = footprint_mb(&rows);
    checkpoint(&mut run, "CSV Loading Complete");
    run.log.success(format!(
        "CSV loaded with streaming: {} rows, {} columns",
        rows.len(),
        rows.keys().len()
    ));
    run.quality = quality(&rows);
    let score = run.quality["score"].as_i64().unwrap_or(0);
    if score < 70 {
        run.log.warn(format!("Data quality issues detected (score: {score})"));
    }
    let tn = re!(r"[^a-zA-Z0-9_]")
        .replace_all(&strip_extension(&file.name), "_")
        .to_lowercase();
    let table_name = if tn.is_empty() { "table".to_string() } else { tn };
    let ctx = Ctx {
        progress,
        cancelled,
        start,
        memory_mb: run.memory_mb,
    };
    let analysis = match analyze_rows(rows, &p.ops, &p.options, &table_name, &ctx, &|x| x) {
        Ok(a) => a,
        Err(e) => return Err(fail(&mut run, e)),
    };
    for (level, msg) in &analysis.log {
        run.log.add(level, msg.clone(), None);
    }
    for n in &analysis.notes {
        run.log.warn(n.clone());
    }
    let pk = analysis
        .key_choice
        .as_ref()
        .map(|k| k.primary_key.clone())
        .or_else(|| analysis.key_result.primary_key.clone())
        .unwrap_or_default();
    Ok((
        run,
        vec![NamedTable {
            name: table_name,
            pk,
            analysis,
        }],
    ))
}

/// Log lines, the analysed tables (with their file labels), and files skipped during analysis.
type MultiTables = (Vec<String>, Vec<(String, NamedTable)>, Vec<input_check::Skipped>);

/// In a multi-file run only the final relation set's rows are used after analysis (for the data
/// export); the result reports the other sets by row count. Large tables release those rows right
/// away, so memory holds one copy of each table's data instead of four or five.
fn release_intermediate_rows(a: &mut TableAnalysis, options: &Options) {
    if options.keep_intermediate_rows.unwrap_or(false)
        || a.rows.len() * a.attributes.len().max(1) <= COMPACT_RESULT_CELLS
    {
        return;
    }
    for r in a
        .relations_1nf
        .iter_mut()
        .chain(a.relations_2nf.iter_mut())
        .chain(a.relations_bcnf.iter_mut())
        .chain(a.relations_4nf.iter_mut())
    {
        r.release_rows();
    }
}

fn prepare_multi(
    files: Vec<InputFile>,
    labels: Vec<String>,
    p: &Prepared,
    progress: &dyn Fn(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Result<MultiTables, String> {
    let mut logs = vec![format!(
        "[INFO] Multi-CSV Pipeline initiated for {} dataset files.",
        files.len()
    )];
    for n in &p.pre_notes {
        logs.push(format!("[WARN] {n}"));
    }
    // Table names are assigned up front, in file order, so they do not depend on scheduling.
    let mut names = IdentifierRegistry::new();
    let jobs: Vec<(usize, &InputFile, String)> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (i, f, names.fresh(&strip_extension(&f.name).to_lowercase())))
        .collect();
    let workers = parallel_file_workers(&p.options, jobs.len());
    if workers > 1 {
        logs.push(format!(
            "[INFO] Analysing {} files in parallel on {workers} threads.",
            jobs.len()
        ));
    }
    let results = analyze_files(&jobs, p, workers, progress, cancelled);
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for ((i, _, table_name), result) in jobs.iter().zip(results) {
        let label = &labels[*i];
        let analysis = match result? {
            FileOutcome::Analysed(a) => *a,
            FileOutcome::Skipped(reason) => {
                logs.push(format!("[WARN] Skipped: {label} {reason}"));
                skipped.push(input_check::Skipped {
                    file: label.clone(),
                    reason: format!("{label} {reason}"),
                });
                continue;
            }
        };
        for note in &analysis.notes {
            logs.push(format!("[WARN] {note}"));
        }
        logs.push(format!(
            "[SUCCESS] Table '{}' ({}): {} rows, {} columns, {} exact FDs.",
            table_name,
            label,
            analysis.rows.len(),
            analysis.attributes.len(),
            analysis.fd_search.as_ref().map(|s| s.exact).unwrap_or(0)
        ));
        let pk = analysis
            .key_choice
            .as_ref()
            .map(|k| k.primary_key.clone())
            .or_else(|| analysis.key_result.primary_key.clone())
            .unwrap_or_else(|| suggest_primary_key(&analysis.rows, &analysis.attributes));
        out.push((
            label.clone(),
            NamedTable {
                name: table_name.clone(),
                pk,
                analysis,
            },
        ));
    }
    if out.is_empty() {
        let mut all = p.input_checks.clone();
        all.skipped.extend(skipped);
        return Err(nothing_left(&all));
    }
    Ok((logs, out, skipped))
}

/// Relation ids of file `i` start at `i × RELATION_ID_BLOCK`, so files analysed concurrently never
/// share an id and every run numbers relations the same way.
const RELATION_ID_BLOCK: u64 = 100_000;

/// How many files to analyse at once: `maxParallelFiles` when given, otherwise one per CPU core (native)
/// or one (WebAssembly, which has no threads here).
fn parallel_file_workers(options: &Options, files: usize) -> usize {
    if cfg!(target_arch = "wasm32") || !options.enable_parallel_processing.unwrap_or(true) {
        return 1;
    }
    let cores = crate::engine_threads();
    let wanted = options.max_parallel_files.map(|n| n.max(1.0) as usize).unwrap_or(cores);
    wanted.min(files).max(1)
}

/// Rough peak memory for analysing one file: parsed cells, cleaned and normalised copies, encodings.
/// Measured at about 20 bytes of memory per byte of CSV on wide numeric tables.
#[cfg(not(target_arch = "wasm32"))]
fn estimated_peak_bytes(file: &InputFile) -> u64 {
    (file.bytes.len() as u64).saturating_mul(20).max(16 << 20)
}

/// Memory the parallel analysis may plan for: `memoryBudgetMb` when given, otherwise half of the
/// memory currently available (Linux) or 4 GB elsewhere.
#[cfg(not(target_arch = "wasm32"))]
fn memory_budget_bytes(options: &Options) -> u64 {
    if let Some(mb) = options.memory_budget_mb {
        return (mb.max(64.0) as u64) << 20;
    }
    match crate::available_memory_bytes() {
        Some(bytes) => bytes / 2,
        None => 4 << 30,
    }
}

/// One file's analysis, or why the file was skipped (it could not be read, or has no data rows).
enum FileOutcome {
    Analysed(Box<TableAnalysis>),
    Skipped(String),
}

/// Load and analyse one file. An unreadable file is skipped, not fatal; `Err` stops the run
/// (cancellation, or a failure inside the analysis).
fn analyze_file(
    file: &InputFile,
    table_name: &str,
    p: &Prepared,
    ctx: &Ctx,
    pct: &dyn Fn(f64) -> f64,
) -> Result<FileOutcome, String> {
    ctx.check()?;
    let mut scratch = Logger::new();
    let rows = match load_csv(file, &mut scratch) {
        Ok(l) => l.rows,
        Err(e) => return Ok(FileOutcome::Skipped(format!("could not be read ({e})."))),
    };
    if rows.is_empty() {
        return Ok(FileOutcome::Skipped("has no data rows.".into()));
    }
    let ctx = Ctx {
        memory_mb: footprint_mb(&rows),
        ..*ctx
    };
    let mut analysis = analyze_rows(rows, &p.ops, &p.options, table_name, &ctx, pct)?;
    release_intermediate_rows(&mut analysis, &p.options);
    Ok(FileOutcome::Analysed(Box::new(analysis)))
}

type FileResult = Result<FileOutcome, String>;

/// Analyse every file, one after another or on `workers` threads. Results are in file order.
fn analyze_files(
    jobs: &[(usize, &InputFile, String)],
    p: &Prepared,
    workers: usize,
    progress: &dyn Fn(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Vec<FileResult> {
    let n = jobs.len();
    let span = |idx: usize| {
        (
            3.0 + (idx as f64 / n as f64) * 67.0,
            3.0 + ((idx + 1) as f64 / n as f64) * 67.0,
        )
    };
    #[cfg(not(target_arch = "wasm32"))]
    if workers > 1 {
        return analyze_files_parallel(jobs, p, workers, progress, cancelled);
    }
    let _ = workers;
    let mut results = Vec::with_capacity(n);
    for (idx, file, table_name) in jobs {
        let (lo, hi) = span(*idx);
        progress(Progress {
            phase: "PREP".into(),
            operation: "csv-loader".into(),
            progress: lo.round(),
            message: format!("Loading table {}/{}: {}…", idx + 1, n, file.name),
            ..Default::default()
        });
        let ctx = Ctx {
            progress,
            cancelled,
            start: p.start,
            memory_mb: 0.0,
        };
        // Same id blocks as a parallel run, so the result does not depend on the number of threads.
        crate::naming::set_relation_id_base(*idx as u64 * RELATION_ID_BLOCK);
        let r = analyze_file(file, table_name, p, &ctx, &|pct| lo + (pct / 100.0) * (hi - lo));
        let failed = r.is_err();
        results.push(r);
        if failed {
            break;
        }
    }
    results
}

/// Each file gets its own thread (bounded by `workers`); the operations inside a file still use the
/// shared rayon pool. Progress and cancellation stay on the calling thread: workers send progress
/// over a channel, and a cancel request is passed on through a shared flag.
#[cfg(not(target_arch = "wasm32"))]
fn analyze_files_parallel(
    jobs: &[(usize, &InputFile, String)],
    p: &Prepared,
    workers: usize,
    progress: &dyn Fn(Progress),
    cancelled: &dyn Fn() -> bool,
) -> Vec<FileResult> {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{mpsc, Mutex};
    use std::time::Duration;

    let n = jobs.len();
    // Largest files first, so the longest analyses start early (results are still kept in file order).
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(jobs[i].1.bytes.len()));
    // A file starts only when its estimated peak memory fits the budget next to the files in progress
    // (a file always starts when nothing else runs), so wide inputs do not exhaust memory.
    let budget = memory_budget_bytes(&p.options);
    let in_use = Mutex::new(0u64);
    let freed = std::sync::Condvar::new();
    let next = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let results: Mutex<Vec<Option<FileResult>>> = Mutex::new((0..n).map(|_| None).collect());
    let (tx, rx) = mpsc::channel::<(usize, Progress)>();
    let mut fraction = vec![0.0f64; n];

    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for w in 0..workers {
            let tx = tx.clone();
            let (next, stop, results, order) = (&next, &stop, &results, &order);
            let (in_use, freed) = (&in_use, &freed);
            let spawned = std::thread::Builder::new()
                .name(format!("dbflow-file-{w}"))
                .stack_size(16 * 1024 * 1024)
                .spawn_scoped(scope, move || loop {
                    let k = next.fetch_add(1, Ordering::SeqCst);
                    if k >= n {
                        break;
                    }
                    let (idx, file, table_name) = &jobs[order[k]];
                    let need = estimated_peak_bytes(file);
                    if let Ok(mut used) = in_use.lock() {
                        while *used > 0 && *used + need > budget && !stop.load(Ordering::Relaxed) {
                            used = match freed.wait_timeout(used, Duration::from_millis(200)) {
                                Ok((g, _)) => g,
                                Err(e) => e.into_inner().0,
                            };
                        }
                        *used += need;
                    }
                    crate::naming::set_relation_id_base(*idx as u64 * RELATION_ID_BLOCK);
                    let send = |pr: Progress| {
                        let _ = tx.send((*idx, pr));
                    };
                    let is_cancelled = || stop.load(Ordering::Relaxed);
                    send(Progress {
                        phase: "PREP".into(),
                        operation: "csv-loader".into(),
                        progress: 0.0,
                        message: format!("Loading table {}/{}: {}…", idx + 1, n, file.name),
                        ..Default::default()
                    });
                    let ctx = Ctx {
                        progress: &send,
                        cancelled: &is_cancelled,
                        start: p.start,
                        memory_mb: 0.0,
                    };
                    let r = analyze_file(file, table_name, p, &ctx, &|pct| pct);
                    if let Ok(mut used) = in_use.lock() {
                        *used = used.saturating_sub(need);
                    }
                    freed.notify_all();
                    if r.is_err() {
                        stop.store(true, Ordering::Relaxed);
                    }
                    if let Ok(mut all) = results.lock() {
                        all[*idx] = Some(r);
                    }
                });
            match spawned {
                Ok(h) => handles.push(h),
                Err(_) => break,
            }
        }
        drop(tx);
        // Forward progress as one overall percentage and pass a cancel request on to the workers.
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok((idx, mut pr)) => {
                    fraction[idx] = (pr.progress / 100.0).clamp(fraction[idx], 1.0);
                    pr.progress = (3.0 + 67.0 * fraction.iter().sum::<f64>() / n as f64).round();
                    pr.estimated_time_remaining = None;
                    progress(pr);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if cancelled() {
                stop.store(true, Ordering::Relaxed);
            }
        }
        for h in handles {
            let _ = h.join();
        }
    });
    let mut collected = results.into_inner().unwrap_or_default();
    // Report the first failure in file order; files after it are not used.
    let mut out = Vec::with_capacity(n);
    for r in collected.drain(..) {
        let r = r.unwrap_or_else(|| Err("Processing was cancelled.".into()));
        let failed = r.is_err();
        out.push(r);
        if failed {
            break;
        }
    }
    out
}

/// Apply names (model answers or the reason the model was unavailable) and build all outputs.
/// Returns the result as JSON text (large parts are serialized once, never copied into a tree).
pub fn finish(
    p: Prepared,
    answers: Option<HashMap<String, String>>,
    namer_error: Option<String>,
    progress: &dyn Fn(Progress),
) -> String {
    let Prepared {
        mode,
        ops,
        options,
        start,
        mut chosen,
        to_ask,
        input_checks,
        ..
    } = p;
    if let Some(a) = &answers {
        accept_model_names(&mut chosen, &to_ask, a);
    }
    let by_rule = options.naming_mode.as_deref() == Some("rules");
    if by_rule {
        accept_rule_names(&mut chosen, &to_ask);
    }
    let result = match mode {
        Mode::Failed(v) => v.to_string(),
        Mode::Single(mut run, mut tables) => {
            if let Some(e) = &namer_error {
                run.log.warn(format!(
                    "Relation naming model unavailable ({e}); relations are left unnamed."
                ));
            }
            apply(&mut tables, false, &chosen);
            let t = tables.pop().expect("one table");
            finish_single(run, t, &ops, &options, start, progress)
        }
        Mode::Multi(logs, tables) => {
            let (files, mut nts): (Vec<String>, Vec<NamedTable>) = tables.into_iter().unzip();
            apply(&mut nts, true, &chosen);
            finish_multi(logs, nts, files, &ops, &options, start, progress)
        }
    };
    with_input_checks(result, &input_checks)
}

fn finish_single(
    mut run: SingleRun,
    t: NamedTable,
    ops: &[String],
    options: &Options,
    start: f64,
    progress: &dyn Fn(Progress),
) -> String {
    let NamedTable {
        name: table_name,
        analysis: mut a,
        ..
    } = t;
    let tz = options.tz_offset_minutes.unwrap_or(0.0);
    let checkpoint = |run: &mut SingleRun, name: &str| {
        let t = js::now_ms() - start;
        run.checkpoints.push((name.to_string(), t.round(), run.memory_mb));
    };
    checkpoint(&mut run, "Analysis Complete");
    checkpoint(&mut run, "Phase 5 Start: Optimized Output Generation");
    run.log.info("Phase 5: Optimized Output Generation");
    let eta = |frac: f64| {
        let e = js::now_ms() - start;
        (e / frac - e).max(0.0)
    };
    let mut erd = String::new();
    let mut schemas = String::new();
    let mut sql = String::new();
    let steps: [(&str, &str, f64, &str, &str); 3] = [
        (
            "ERD Generator",
            "erd-generator",
            90.0,
            "Generating enhanced Entity Relationship Diagram...",
            "ERD generation",
        ),
        (
            "Relation Generator",
            "relation-generator",
            95.0,
            "Generating optimized relation schemas...",
            "Relation schema generation",
        ),
        (
            "SQL Generator",
            "sql-generator",
            98.0,
            "Generating enhanced SQL DDL...",
            "SQL DDL generation",
        ),
    ];
    for (op, id, pct, msg, label) in steps {
        if !has(ops, op) {
            continue;
        }
        progress(Progress {
            phase: "OUTPUT".into(),
            operation: id.into(),
            progress: pct,
            message: msg.into(),
            estimated_time_remaining: Some(eta(pct / 100.0)),
            memory_usage: Some(run.memory_mb),
        });
        match op {
            "ERD Generator" => {
                erd = legacy::generate_erd(&mut a.relations_3nf).0;
                if a.bcnf_is_final {
                    a.relations_bcnf = a.relations_3nf.clone();
                }
                if a.fourth_is_final {
                    a.relations_4nf = a.relations_3nf.clone();
                }
            }
            "Relation Generator" => schemas = legacy::generate_relation_schemas(&a.relations_3nf),
            _ => sql = legacy::generate_sql_ddl(&a.relations_3nf),
        }
        checkpoint(&mut run, &format!("{label} Complete"));
        run.log.success(format!("{label} completed"));
    }
    if let Some(d) = options.sql_dialect.as_deref().and_then(Dialect::parse) {
        if has(ops, "SQL Generator") && !a.relations_3nf.is_empty() {
            let schema = build_schema(
                vec![TableSchemaInput {
                    table_name: table_name.clone(),
                    headers: a.attributes.clone(),
                    rows: a.rows.clone(),
                    primary_key: a.key_result.primary_key.clone().unwrap_or_default(),
                    relations: a.relations_3nf.clone(),
                }],
                &[],
            );
            sql = generate_ddl(&to_decomposed_relations(&schema), d);
        }
    }
    progress(Progress {
        phase: "OUTPUT".into(),
        operation: "report-generator".into(),
        progress: 100.0,
        message: "Finalizing optimized reports...".into(),
        memory_usage: Some(run.memory_mb),
        ..Default::default()
    });
    let dur = duration(start);
    checkpoint(&mut run, "Process Complete");
    let retention = if run.original_rows > 0 {
        format!(
            "{}%",
            js::to_fixed(a.rows.len() as f64 / run.original_rows as f64 * 100.0, 1)
        )
    } else {
        "100%".into()
    };
    let peak = run.checkpoints.iter().map(|c| c.2).fold(0.0, f64::max);
    let total = a.cache_hits + a.cache_misses;
    let mut optimizations = vec![];
    if options.enable_parallel_processing.unwrap_or(true) {
        optimizations.push("Parallel Processing");
    }
    if options.adaptive_thresholds.unwrap_or(true) {
        optimizations.push("Adaptive Thresholds");
    }
    optimizations.extend([
        "Sampling Strategy",
        "Streaming CSV",
        "Incremental Processing",
        "Memory Optimization",
        "Rust Engine",
    ]);
    let cps: Vec<Value> = run
        .checkpoints
        .iter()
        .map(|(n, t, m)| json!({"name": n, "time": t, "memory": m}))
        .collect();
    let perf = json!({
        "totalDuration": dur,
        "checkpoints": cps,
        "peakMemoryUsage": peak,
        "memorySource": "estimate",
        "optimizationsApplied": optimizations,
        "cacheHitRate": if total > 0 { json!(a.cache_hits as f64 / total as f64) } else { Value::Null },
        "cacheHits": a.cache_hits,
        "cacheMisses": a.cache_misses,
    });
    let compact = a.compact || a.rows.len() * a.attributes.len().max(1) > COMPACT_RESULT_CELLS;
    if compact {
        run.log.info(format!(
            "Large input ({} rows × {} columns): each relation's data is exported once as CSV; row data is not repeated in the JSON result.",
            a.rows.len(),
            a.attributes.len()
        ));
    }
    let files = output_files(&a, ops, &erd, &schemas, &sql, &perf, compact);
    run.log.success(format!(
        "Optimized database normalization completed successfully in {dur}"
    ));
    let r3 = &a.relations_3nf;
    let relationships: Vec<Value> = r3
        .iter()
        .flat_map(|r| {
            r.foreign_keys
                .iter()
                .map(move |fk| json!({"source": r.name, "target": fk.references, "cardinality": fk.cardinality}))
        })
        .collect();
    let stats = json!({
        "originalRows": run.original_rows,
        "cleanedRows": a.rows.len(),
        "dataRetention": retention,
        "attributes": a.attributes.len(),
        "functionalDependencies": a.functional_dependencies.len(),
        "candidateKeys": a.key_result.candidate_keys.len(),
        "superkeys": a.key_result.superkeys.len(),
        "relations1NF": a.relations_1nf.len(),
        "relations2NF": a.relations_2nf.len(),
        "relations3NF": r3.len(),
        "relationsBCNF": a.relations_bcnf.len(),
        "relations4NF": a.relations_4nf.len(),
        "multivaluedDependencies": a.mvds.len(),
        "connectedEntities": r3.iter().filter(|r| !r.foreign_keys.is_empty()).count(),
        "relationships": r3.iter().map(|r| r.foreign_keys.len()).sum::<usize>(),
        "cardinalityMappings": a.cardinality.len(),
        "losslessJoin": a.lossless_check["isLossless"],
        "dependencyPreservation": a.dependency_preservation["isPreserved"],
        "processDuration": dur,
        "memoryUsage": format!("{} MB", js::to_fixed(peak, 2)),
        "performanceMetrics": perf,
    });
    let relations_json = rels(r3, compact);
    let bcnf_json = if a.bcnf_is_final {
        relations_json.clone()
    } else {
        rels(&a.relations_bcnf, compact)
    };
    let four_json = if a.fourth_is_final {
        relations_json.clone()
    } else {
        rels(&a.relations_4nf, compact)
    };
    object(vec![
        ("success", raw(&true)),
        ("stats", raw(&stats)),
        ("outputFiles", raw(&files)),
        ("logs", raw(&run.log.lines(tz))),
        ("relations", relations_json),
        ("relations1NF", rels(&a.relations_1nf, compact)),
        ("relations2NF", rels(&a.relations_2nf, compact)),
        ("bcnf", bcnf_json),
        ("relations4NF", four_json),
        ("multivaluedDependencies", raw(&a.mvds)),
        ("relationships", raw(&relationships)),
        ("erdContent", raw(&erd)),
        ("cardinalityAnalysis", raw(&a.cardinality)),
        ("losslessCheck", raw(&a.lossless_check)),
        ("dependencyPreservation", raw(&a.dependency_preservation)),
        ("duration", raw(&dur)),
        ("selectedOperations", raw(ops)),
        ("qualityAssessment", raw(&run.quality)),
        ("engine", raw(engine_name())),
    ])
}

fn engine_name() -> &'static str {
    if cfg!(target_arch = "wasm32") {
        "rust-wasm"
    } else {
        "rust-native"
    }
}

fn output_files(
    a: &TableAnalysis,
    ops: &[String],
    erd: &str,
    schemas: &str,
    sql: &str,
    perf: &Value,
    compact: bool,
) -> Vec<Raw> {
    let mut f = Vec::new();
    f.push(out_file(
        "performance_metrics.json",
        "json",
        "Detailed performance metrics and optimizations applied",
        raw(perf),
        json_len(perf),
        "Performance Analytics",
    ));
    f.push(out_file(
        "performance_report.md",
        "markdown",
        "Performance analysis and optimization report",
        text(reports::performance_markdown(perf, "estimate")),
        0,
        "Performance Analytics",
    ));
    let data = &a.rows;
    if has(ops, "Data Cleaner") && !data.is_empty() {
        f.push(out_file(
            "cleaned_data.json",
            "json",
            "Cleaned and processed dataset",
            raw(&data.slice(0, 100)),
            json_len(data),
            "Data Preparation",
        ));
        let csv = rows_to_csv(data);
        let n = js::len16(&csv);
        f.push(out_file(
            "cleaned_data.csv",
            "csv",
            "Cleaned dataset in CSV format",
            text(csv),
            n,
            "Data Preparation",
        ));
    }
    if has(ops, "Atomicity Checker") {
        let j = a.atomicity_json();
        f.push(out_file(
            "atomicity_analysis.json",
            "json",
            "First Normal Form atomicity analysis",
            raw(&j),
            json_len(&j),
            "Data Preparation",
        ));
        let md = reports::atomicity_markdown(&a.atomicity_result.clone().unwrap_or_default());
        f.push(out_file(
            "atomicity_report.md",
            "markdown",
            "Atomicity analysis report",
            text(md),
            0,
            "Data Preparation",
        ));
    }
    if has(ops, "Data Type Profiler") {
        f.push(out_file(
            "data_profile.json",
            "json",
            "Data type and quality analysis",
            raw(&a.data_profile),
            json_len(&a.data_profile),
            "Data Preparation",
        ));
        f.push(out_file(
            "data_profile_report.md",
            "markdown",
            "Data profiling report",
            text(reports::profile_markdown(&a.data_profile)),
            0,
            "Data Preparation",
        ));
    }
    if has(ops, "1NF") && !a.relations_1nf.is_empty() {
        f.push(out_file(
            "relations_1nf.json",
            "json",
            "First Normal Form relations",
            rels_without_rows(&a.relations_1nf),
            0,
            "First Normal Form",
        ));
        f.push(out_file(
            "relations_1nf_report.md",
            "markdown",
            "1NF analysis and atomicity report",
            text(reports::nf1_markdown(&a.relations_1nf)),
            0,
            "First Normal Form",
        ));
    }
    if has(ops, "Cardinality Analyzer") {
        f.push(out_file(
            "cardinality_analysis.json",
            "json",
            "Relationship cardinality analysis",
            raw(&a.cardinality),
            json_len(&a.cardinality),
            "Analysis",
        ));
        f.push(out_file(
            "cardinality_report.md",
            "markdown",
            "Cardinality analysis report",
            text(reports::cardinality_markdown(&a.cardinality)),
            0,
            "Analysis",
        ));
    }
    let fds = &a.functional_dependencies;
    if has(ops, "FD Miner") && !fds.is_empty() {
        f.push(out_file(
            "functional_dependencies.md",
            "markdown",
            "Comprehensive functional dependency analysis",
            text(reports::fd_markdown(fds)),
            0,
            "Functional Dependencies",
        ));
        f.push(out_file(
            "functional_dependencies.json",
            "json",
            "Functional dependencies raw data",
            raw(fds),
            json_len(fds),
            "Functional Dependencies",
        ));
        let svg = reports::fd_svg(fds);
        let n = js::len16(&svg);
        f.push(out_file(
            "functional_dependencies_graph.svg",
            "svg",
            "Functional dependencies visualization",
            text(svg),
            n,
            "Functional Dependencies",
        ));
    }
    if has(ops, "Key Discovery") {
        f.push(out_file(
            "keys_analysis.md",
            "markdown",
            "Candidate and super key analysis",
            text(reports::key_markdown(&a.key_result)),
            0,
            "Key Analysis",
        ));
        f.push(out_file(
            "keys_analysis.json",
            "json",
            "Key analysis raw data",
            raw(&a.key_result),
            json_len(&a.key_result),
            "Key Analysis",
        ));
    }
    if has(ops, "2NF") && !a.relations_2nf.is_empty() {
        f.push(out_file(
            "relations_2nf.json",
            "json",
            "Second Normal Form relations",
            rels_without_rows(&a.relations_2nf),
            0,
            "Second Normal Form",
        ));
        f.push(out_file(
            "relations_2nf_report.md",
            "markdown",
            "2NF analysis and decomposition report",
            text(reports::nf2_markdown(&a.relations_2nf)),
            0,
            "Second Normal Form",
        ));
        for r in a.relations_2nf.iter().filter(|_| !compact) {
            if !r.rows.is_empty() {
                let csv = rows_to_csv(&r.rows);
                let n = js::len16(&csv);
                f.push(out_file(
                    &format!("relation_2nf_{}.csv", r.name),
                    "csv",
                    &format!("2NF data for relation: {}", r.name),
                    text(csv),
                    n,
                    "Second Normal Form",
                ));
            }
        }
    }
    let r3 = &a.relations_3nf;
    // Each relation's CSV (and JSON rows) is used by several output files; build them once, in parallel.
    let r3_csv: Vec<String> = if (has(ops, "3NF") || has(ops, "BCNF") || has(ops, "4NF")) && !r3.is_empty() {
        crate::par_map(r3, |r| {
            if r.rows.is_empty() {
                String::new()
            } else {
                rows_to_csv(&r.rows)
            }
        })
    } else {
        vec![]
    };
    if has(ops, "3NF") && !r3.is_empty() {
        f.push(out_file(
            "relations_3nf.json",
            "json",
            "Third Normal Form relations",
            rels_without_rows(r3),
            0,
            "Third Normal Form",
        ));
        if !schemas.is_empty() {
            f.push(out_file(
                "relation_schemas.md",
                "markdown",
                "Detailed relation schema documentation",
                text(schemas),
                js::len16(schemas),
                "Third Normal Form",
            ));
        }
        // Large inputs: the CSV is the data export; a JSON copy of every row would double it.
        let r3_json: Vec<Option<(Raw, usize)>> =
            crate::par_map(r3, |r| (!compact).then(|| (raw(&r.rows), json_len(&r.rows))));
        for ((r, csv), rows) in r3.iter().zip(&r3_csv).zip(r3_json) {
            if !r.rows.is_empty() {
                let csv = csv.clone();
                let n = js::len16(&csv);
                f.push(out_file(
                    &format!("relation_3nf_{}.csv", r.name),
                    "csv",
                    &format!("3NF data for relation: {}", r.name),
                    text(csv),
                    n,
                    "Third Normal Form",
                ));
                if let Some((rows_json, rows_len)) = rows {
                    f.push(out_file(
                        &format!("relation_3nf_{}.json", r.name),
                        "json",
                        &format!("3NF JSON data for relation: {}", r.name),
                        rows_json,
                        rows_len,
                        "Third Normal Form",
                    ));
                }
            }
        }
    }
    if has(ops, "BCNF") && !r3.is_empty() {
        // BCNF is the final set unless 4NF ran after it; then it has its own relations.
        let bcnf: &[Relation] = if a.bcnf_is_final { r3 } else { &a.relations_bcnf };
        f.push(out_file(
            "relations_bcnf.json",
            "json",
            "Boyce-Codd Normal Form relations",
            rels_without_rows(bcnf),
            0,
            "Boyce-Codd Normal Form",
        ));
        // The BCNF files repeat the 3NF ones (same relations); large inputs export them once.
        let own_csv: Vec<String>;
        let bcnf_csv: &[String] = if !a.bcnf_is_final {
            own_csv = crate::par_map(bcnf, |r| {
                if r.rows.is_empty() {
                    String::new()
                } else {
                    rows_to_csv(&r.rows)
                }
            });
            &own_csv
        } else if compact && has(ops, "3NF") {
            &r3_csv[..0]
        } else {
            &r3_csv[..]
        };
        for (r, csv) in bcnf.iter().zip(bcnf_csv) {
            if !r.rows.is_empty() {
                let csv = csv.clone();
                let n = js::len16(&csv);
                f.push(out_file(
                    &format!("relation_bcnf_{}.csv", r.name),
                    "csv",
                    &format!("BCNF data for relation: {}", r.name),
                    text(csv),
                    n,
                    "Boyce-Codd Normal Form",
                ));
            }
        }
    }
    if has(ops, "4NF") && !r3.is_empty() {
        f.push(out_file(
            "relations_4nf.json",
            "json",
            "Fourth Normal Form relations",
            rels_without_rows(r3),
            0,
            "Fourth Normal Form",
        ));
        f.push(out_file(
            "multivalued_dependencies.json",
            "json",
            "Multivalued dependencies the 4NF step decomposed on",
            raw(&a.mvds),
            json_len(&a.mvds),
            "Fourth Normal Form",
        ));
        // The 4NF relations are the final set, so their data is already in the 3NF files when 3NF ran.
        let four_csv = if has(ops, "3NF") { &r3_csv[..0] } else { &r3_csv[..] };
        for (r, csv) in r3.iter().zip(four_csv) {
            if !r.rows.is_empty() && !csv.is_empty() {
                let csv = csv.clone();
                let n = js::len16(&csv);
                f.push(out_file(
                    &format!("relation_4nf_{}.csv", r.name),
                    "csv",
                    &format!("4NF data for relation: {}", r.name),
                    text(csv),
                    n,
                    "Fourth Normal Form",
                ));
            }
        }
    }
    if has(ops, "Lossless Check") {
        f.push(out_file(
            "lossless_check.json",
            "json",
            "Lossless join decomposition analysis",
            raw(&a.lossless_check),
            json_len(&a.lossless_check),
            "Quality Assurance",
        ));
        f.push(out_file(
            "lossless_check_report.md",
            "markdown",
            "Lossless join analysis report",
            text(reports::lossless_markdown(&a.lossless_check)),
            0,
            "Quality Assurance",
        ));
    }
    if has(ops, "Dependency Preservation") {
        f.push(out_file(
            "dependency_preservation.json",
            "json",
            "Dependency preservation analysis",
            raw(&a.dependency_preservation),
            json_len(&a.dependency_preservation),
            "Quality Assurance",
        ));
        f.push(out_file(
            "dependency_preservation_report.md",
            "markdown",
            "Dependency preservation analysis report",
            text(reports::preservation_markdown(&a.dependency_preservation)),
            0,
            "Quality Assurance",
        ));
    }
    if has(ops, "ERD Generator") && !erd.is_empty() {
        f.push(out_file(
            "erd_diagram.mmd",
            "mermaid",
            "Entity Relationship Diagram in Mermaid format",
            text(erd),
            js::len16(erd),
            "Entity Relationship Diagram",
        ));
    }
    if has(ops, "SQL Generator") && !sql.is_empty() {
        f.push(out_file(
            "database_schema.sql",
            "sql",
            "Complete SQL Data Definition Language",
            text(sql),
            js::len16(sql),
            "SQL Generation",
        ));
    }
    if has(ops, "Report Generator") {
        let report = reports::process_report(&reports::ReportInput {
            ops,
            fds,
            keys: &a.key_result,
            r1: &a.relations_1nf,
            r2: &a.relations_2nf,
            r3,
            row_count: data.len(),
            columns: &data.keys(),
            cardinality: &a.cardinality,
            lossless: &a.lossless_check,
            preservation: &a.dependency_preservation,
            atomicity: &a.atomicity_result.clone().unwrap_or_default(),
            profile: &a.data_profile,
            performance: perf,
            mvds: &a.mvds,
        });
        let n = js::len16(&report);
        f.push(out_file(
            "process_report.md",
            "markdown",
            "Comprehensive normalization analysis report",
            text(report),
            n,
            "Process Report",
        ));
    }
    f
}

fn table_json(name: &str, file: &str, t: &NamedTable, row_count: usize, compact: bool) -> Raw {
    let a = &t.analysis;
    let keys: Vec<Value> = a
        .key_result
        .candidate_keys
        .iter()
        .map(|k| json!({"attributes": k, "isMinimal": true}))
        .collect();
    RawValue::from_string(object(vec![
        ("tableName", raw(name)),
        ("fileName", raw(file)),
        ("headers", raw(&a.attributes)),
        ("rows", if compact { raw(&[0u8; 0]) } else { raw(&a.rows) }),
        ("rowCount", raw(&row_count)),
        ("nerTags", raw(&a.ner_tags)),
        ("functionalDependencies", raw(&a.functional_dependencies)),
        ("candidateKeys", raw(&keys)),
        ("primaryKey", raw(&t.pk)),
        ("firstNormalForm", rels(&a.relations_1nf, compact)),
        ("secondNormalForm", rels(&a.relations_2nf, compact)),
        ("thirdNormalForm", rels(&a.relations_3nf, compact)),
        ("bcnf", rels(&a.relations_bcnf, compact)),
        ("fourthNormalForm", rels(&a.relations_4nf, compact)),
        ("multivaluedDependencies", raw(&a.mvds)),
        ("keyReason", raw(&a.key_choice.as_ref().map(|k| k.reason.clone()))),
        ("fdSearch", raw(&a.fd_search)),
        ("losslessCheck", raw(&a.lossless_check)),
        ("dependencyPreservation", raw(&a.dependency_preservation)),
        ("notes", raw(&a.notes)),
    ]))
    .expect("valid JSON")
}

fn finish_multi(
    mut logs: Vec<String>,
    mut tables: Vec<NamedTable>,
    files: Vec<String>,
    ops: &[String],
    options: &Options,
    start: f64,
    progress: &dyn Fn(Progress),
) -> String {
    let total_cells: usize = tables
        .iter()
        .map(|t| t.analysis.rows.len() * t.analysis.attributes.len().max(1))
        .sum();
    let compact = total_cells > COMPACT_RESULT_CELLS;
    let mut ner: IndexMap<&str, &Vec<crate::types::EntityTag>> = IndexMap::new();
    for t in &tables {
        ner.insert(&t.name, &t.analysis.ner_tags);
    }
    let ner = raw(&ner);
    let mut links: Vec<CrossLink> = Vec::new();
    if has(ops, "Relation Detector") {
        progress(Progress {
            phase: "ANALYZE".into(),
            operation: "relation-detector".into(),
            progress: 72.0,
            message: "Detecting foreign keys across files…".into(),
            ..Default::default()
        });
        let det: Vec<DetectorTable> = tables
            .iter()
            .map(|t| DetectorTable {
                table_name: t.name.clone(),
                headers: t.analysis.attributes.clone(),
                rows: &t.analysis.rows,
                primary_key: t.pk.clone(),
            })
            .collect();
        links = detect_foreign_keys(&det, 0.95, 0.7);
        logs.push(format!(
            "[SUCCESS] Relation Detector identified {} foreign key relationship(s).",
            links.len()
        ));
    }
    let row_counts: Vec<usize> = tables.iter().map(|t| t.analysis.rows.len()).collect();
    // Large runs move each table's rows into the schema instead of copying them (the result then reports
    // row counts only); small runs keep both, as the result includes the rows.
    let schema_inputs: Vec<TableSchemaInput> = tables
        .iter_mut()
        .map(|t| {
            let (rows, relations) = if compact {
                let a = &mut t.analysis;
                let fin = if !a.relations_3nf.is_empty() {
                    &mut a.relations_3nf
                } else {
                    &mut a.relations_2nf
                };
                let moved: Vec<Relation> = fin
                    .iter_mut()
                    .map(|r| {
                        let rows = std::mem::take(&mut r.rows);
                        r.released_rows = Some(rows.len());
                        let mut copy = r.clone();
                        copy.rows = rows;
                        copy.released_rows = None;
                        copy
                    })
                    .collect();
                (std::mem::take(&mut a.rows), moved)
            } else {
                (t.analysis.rows.clone(), t.analysis.final_set().to_vec())
            };
            TableSchemaInput {
                table_name: t.name.clone(),
                headers: t.analysis.attributes.clone(),
                rows,
                primary_key: t.pk.clone(),
                relations,
            }
        })
        .collect();
    let schema = build_schema(schema_inputs, &links);
    for u in &schema.unplaced {
        logs.push(format!(
            "[WARN] Foreign key {}.{} → {}.{} skipped: {}.",
            u.link.source_table, u.link.source_column, u.link.target_table, u.link.target_column, u.reason
        ));
    }
    let dialect = options
        .sql_dialect
        .as_deref()
        .and_then(Dialect::parse)
        .unwrap_or(Dialect::Sqlite);
    let mut erd = String::new();
    if has(ops, "ERD Generator") {
        progress(Progress {
            phase: "OUTPUT".into(),
            operation: "erd-generator".into(),
            progress: 85.0,
            message: "Generating ER diagram…".into(),
            ..Default::default()
        });
        erd = emit_mermaid_erd(&schema);
    }
    let mut sql = String::new();
    if has(ops, "SQL Generator") {
        progress(Progress {
            phase: "OUTPUT".into(),
            operation: "sql-generator".into(),
            progress: 92.0,
            message: "Generating SQL DDL…".into(),
            ..Default::default()
        });
        sql = generate_ddl(&to_decomposed_relations(&schema), dialect);
    }
    progress(Progress {
        phase: "OUTPUT".into(),
        operation: "report-generator".into(),
        progress: 97.0,
        message: "Writing report…".into(),
        ..Default::default()
    });
    let entries: Vec<reports::MultiTableEntry> = tables
        .iter()
        .zip(&files)
        .zip(&row_counts)
        .map(|((t, f), &row_count)| reports::MultiTableEntry {
            table_name: &t.name,
            file_name: f,
            row_count,
            analysis: &t.analysis,
        })
        .collect();
    let report = reports::multi_table_report(&entries, &links, &schema, dialect.name());
    let mut out = Vec::new();
    if !erd.is_empty() {
        out.push(out_file(
            "multi_table_erd.mermaid",
            "mermaid",
            "ER diagram of the normalized relations with foreign keys",
            text(erd.clone()),
            js::len16(&erd),
            "ERD",
        ));
    }
    if !sql.is_empty() {
        out.push(out_file(
            "consolidated_schema.sql",
            "sql",
            &format!("Consolidated {} DDL for the normalized relations", dialect.name()),
            text(sql.clone()),
            js::len16(&sql),
            "SQL",
        ));
        if compact {
            logs.push(format!(
                "[INFO] Large input ({total_cells} cells): normalized data is exported as one CSV per relation; the INSERT script and repeated row data are left out."
            ));
        }
        for f in export_normalized_data_with(&schema, dialect, !compact) {
            let desc = if f.kind == "csv" {
                "Normalized rows for one relation (duplicates removed)"
            } else if f.name.starts_with("verify") {
                "Queries that count orphan rows per foreign key (expect 0)"
            } else {
                "INSERT statements in dependency order"
            };
            let n = js::len16(&f.content);
            let cat = if f.kind == "csv" { "Data" } else { "SQL" };
            out.push(out_file(&f.name, &f.kind, desc, text(f.content), n, cat));
        }
    }
    out.push(out_file(
        "multi_csv_normalization_report.md",
        "markdown",
        "Per-table analysis, relationships and final schema",
        text(report.clone()),
        js::len16(&report),
        "Report",
    ));
    let dur = duration(start);
    progress(Progress {
        phase: "OUTPUT".into(),
        operation: "report-generator".into(),
        progress: 100.0,
        message: "Multi-CSV pipeline completed successfully!".into(),
        ..Default::default()
    });
    let all_fds: usize = tables.iter().map(|t| t.analysis.functional_dependencies.len()).sum();
    let rows: usize = row_counts.iter().sum();
    let table_values: Vec<Raw> = tables
        .iter()
        .zip(&files)
        .zip(&row_counts)
        .map(|((t, f), &n)| table_json(&t.name, f, t, n, compact))
        .collect();
    let relationships: Vec<Value> = schema
        .relations
        .iter()
        .flat_map(|r| {
            r.foreign_keys
                .iter()
                .map(move |fk| json!({"source": r.name, "target": fk.references, "cardinality": fk.cardinality}))
        })
        .collect();
    let stats = json!({
        "originalRows": rows,
        "cleanedRows": rows,
        "attributes": tables.iter().map(|t| t.analysis.attributes.len()).sum::<usize>(),
        "functionalDependencies": all_fds,
        "candidateKeys": tables.iter().map(|t| t.analysis.key_result.candidate_keys.len()).sum::<usize>(),
        "relationships": links.len(),
        "relations3NF": schema.relations.len(),
        "processDuration": dur,
    });
    let relations = rels(&schema.relations, compact);
    object(vec![
        ("success", raw(&true)),
        ("isMultiTable", raw(&true)),
        ("tables", raw(&table_values)),
        ("crossTableRelationships", raw(&links)),
        ("nerResults", raw(&ner)),
        ("relations", relations.clone()),
        ("relations3NF", relations),
        ("relationships", raw(&relationships)),
        ("stats", raw(&stats)),
        ("outputFiles", raw(&out)),
        ("logs", raw(&logs)),
        ("erdDiagram", raw(&erd)),
        ("erdContent", raw(&erd)),
        ("sqlStatements", raw(&sql)),
        ("reportMarkdown", raw(&report)),
        ("duration", raw(&dur)),
        ("selectedOperations", raw(ops)),
        ("engine", raw(engine_name())),
    ])
}

/// One-shot run without a naming model.
pub fn process(files: Vec<InputFile>, ops: Vec<String>, options: Value) -> Value {
    let noop = |_: Progress| {};
    let never = || false;
    let p = prepare(files, ops, options, &noop, &never);
    serde_json::from_str(&finish(p, None, None, &noop)).unwrap_or(Value::Null)
}

pub use analysis::Options as ProcessingOptions;
