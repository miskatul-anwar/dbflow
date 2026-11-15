//! CSV parsing: delimiter sniffing, quoted fields, header repair, null tokens.

use crate::data_utils::{clean_str, index_column_needs_values, is_index_column};
use crate::js;
use crate::table::{Cell, Table};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub const SUPPORTED_DELIMITERS: [char; 4] = [',', ';', '\t', '|'];
const NULL_VALUES: [&str; 6] = ["null", "undefined", "n/a", "nil", "nan", "\\n"];

/// Repair raw header names: blanks become `column_N`, duplicates get `_N` suffixes.
pub fn sanitize_headers(raw: &[String]) -> Vec<String> {
    let mut allocated: HashSet<String> = HashSet::new();
    let mut base_counts: HashMap<String, usize> = HashMap::new();
    raw.iter()
        .enumerate()
        .map(|(idx, r)| {
            let mut base = js::trim(r).to_string();
            if base.is_empty() {
                base = format!("column_{}", idx + 1);
            }
            let mut candidate = base.clone();
            if allocated.contains(&candidate) {
                let mut count = *base_counts.get(&base).unwrap_or(&1);
                while allocated.contains(&format!("{base}_{count}")) {
                    count += 1;
                }
                candidate = format!("{base}_{count}");
                base_counts.insert(base.clone(), count + 1);
            } else {
                base_counts.insert(base.clone(), 1);
            }
            allocated.insert(candidate.clone());
            candidate
        })
        .collect()
}

fn strip_bom(s: &str) -> &str {
    s.strip_prefix('\u{FEFF}').unwrap_or(s)
}

/// Pick the delimiter whose per-line count is most consistent.
pub fn detect_delimiter(sample: &str) -> char {
    let clean = strip_bom(sample);
    let window: Vec<char> = clean.encode_utf16().take(32768).collect::<Vec<u16>>().pipe(|u| {
        char::decode_utf16(u.iter().copied())
            .map(|r| r.unwrap_or('\u{FFFD}'))
            .collect()
    });
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut i = 0;
    while i < window.len() && lines.len() < 20 {
        let c = window[i];
        if c == '"' {
            if in_quotes && window.get(i + 1) == Some(&'"') {
                current.push_str("\"\"");
                i += 2;
                continue;
            }
            in_quotes = !in_quotes;
            current.push(c);
        } else if (c == '\n' || c == '\r') && !in_quotes {
            if c == '\r' && window.get(i + 1) == Some(&'\n') {
                i += 1;
            }
            if !js::trim(&current).is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            current.clear();
        } else {
            current.push(c);
        }
        i += 1;
    }
    if !js::trim(&current).is_empty() && lines.len() < 20 {
        lines.push(current);
    }
    if lines.is_empty() {
        return ',';
    }
    let weight = |d: char| match d {
        ',' => 0.4,
        ';' => 0.3,
        '\t' => 0.2,
        _ => 0.1,
    };
    let mut best = ',';
    let mut best_score = -1.0;
    for d in SUPPORTED_DELIMITERS {
        let mut counts = Vec::new();
        for line in &lines {
            let ch: Vec<char> = line.chars().collect();
            let mut count = 0;
            let mut q = false;
            let mut j = 0;
            while j < ch.len() {
                let c = ch[j];
                if c == '"' {
                    if q && ch.get(j + 1) == Some(&'"') {
                        j += 1;
                    } else {
                        q = !q;
                    }
                } else if c == d && !q {
                    count += 1;
                }
                j += 1;
            }
            counts.push(count);
        }
        let valid: Vec<usize> = counts.into_iter().filter(|&c| c > 0).collect();
        if valid.is_empty() {
            continue;
        }
        let mut freq: HashMap<usize, usize> = HashMap::new();
        let (mut mode_count, mut mode_freq) = (0usize, 0usize);
        for c in valid {
            let f = freq.entry(c).or_insert(0);
            *f += 1;
            if *f > mode_freq {
                mode_freq = *f;
                mode_count = c;
            }
        }
        let consistency = mode_freq as f64 / lines.len() as f64;
        let score = consistency * 100.0 + mode_count as f64 * 2.0 + weight(d);
        if score > best_score {
            best_score = score;
            best = d;
        }
    }
    best
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}

#[derive(Clone, Debug, Serialize)]
pub struct RawCell {
    pub value: String,
    #[serde(rename = "isQuoted")]
    pub is_quoted: bool,
}

/// Split text into rows of cells, honouring quotes (`""` and `\"` escapes).
pub fn parse_cells(text: &str, delimiter: char) -> (Vec<Vec<RawCell>>, bool) {
    let text = strip_bom(text);
    let mut rows = Vec::new();
    let mut row: Vec<RawCell> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut quoted = false;
    let mut multiline = false;
    let mut it = text.chars().peekable();
    let emit = |field: &mut String, quoted: &mut bool, row: &mut Vec<RawCell>| {
        let value = if *quoted {
            std::mem::take(field)
        } else {
            js::trim(&std::mem::take(field)).to_string()
        };
        row.push(RawCell {
            value,
            is_quoted: *quoted,
        });
        *quoted = false;
    };
    while let Some(c) = it.next() {
        if in_quotes {
            if c == '"' {
                if it.peek() == Some(&'"') {
                    it.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else if c == '\\' && it.peek() == Some(&'"') {
                it.next();
                field.push('"');
            } else {
                if c == '\n' || c == '\r' {
                    multiline = true;
                }
                field.push(c);
            }
        } else if c == '"' {
            if js::trim(&field).is_empty() {
                in_quotes = true;
                quoted = true;
                field.clear();
            } else {
                field.push(c);
            }
        } else if c == delimiter {
            emit(&mut field, &mut quoted, &mut row);
        } else if c == '\r' || c == '\n' {
            if c == '\r' && it.peek() == Some(&'\n') {
                it.next();
            }
            emit(&mut field, &mut quoted, &mut row);
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if in_quotes || quoted || !field.is_empty() || !row.is_empty() {
        emit(&mut field, &mut quoted, &mut row);
        rows.push(row);
    }
    (rows, multiline)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvMetadata {
    pub original_rows: usize,
    pub final_rows: usize,
    pub original_columns: usize,
    pub final_columns: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    pub delimiter: String,
    pub has_multiline_fields: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CsvParseResult {
    pub data: Table,
    pub headers: Vec<String>,
    pub metadata: CsvMetadata,
}

#[derive(Clone, Debug, Default)]
pub struct CsvOptions {
    pub delimiter: Option<char>,
    pub max_rows: Option<usize>,
    pub preserve_index_columns: Option<bool>,
    pub file_name: Option<String>,
}

fn is_null_token(v: &str) -> bool {
    v.len() <= 9 && NULL_VALUES.iter().any(|n| v.eq_ignore_ascii_case(n))
}

fn cell_value(cell: Option<&RawCell>) -> Cell {
    match cell {
        Some(c) if c.is_quoted => {
            if is_null_token(&c.value) {
                Cell::str("")
            } else {
                Cell::str(c.value.clone())
            }
        }
        Some(c) => Cell::str(clean_str(&c.value)),
        None => Cell::str(""),
    }
}

/// Like `cell_value`, taking ownership so quoted text is not copied.
fn cell_value_owned(cell: Option<&mut RawCell>) -> Cell {
    match cell {
        Some(c) if c.is_quoted => {
            if is_null_token(&c.value) {
                Cell::str("")
            } else {
                Cell::str(std::mem::take(&mut c.value))
            }
        }
        Some(c) => Cell::str(clean_str(&c.value)),
        None => Cell::str(""),
    }
}

/// Parse CSV text into rows (`CSVProcessor.parse`).
pub fn parse(text: &str, options: &CsvOptions) -> Result<CsvParseResult, String> {
    if js::trim(text).is_empty() {
        return Err("Empty CSV file".into());
    }
    let delimiter = options.delimiter.unwrap_or_else(|| detect_delimiter(text));
    let (cell_rows, multiline) = parse_cells(text, delimiter);
    let rows: Vec<Vec<RawCell>> = cell_rows
        .into_iter()
        .filter(|r| !r.is_empty() && r.iter().any(|c| !c.value.is_empty()))
        .collect();
    if rows.is_empty() {
        return Err("No valid data found in CSV file".into());
    }
    let raw_headers: Vec<String> = rows[0].iter().map(|c| c.value.clone()).collect();
    let headers = sanitize_headers(&raw_headers);
    let data_rows = &rows[1..];
    let allow = options.preserve_index_columns == Some(false);
    let mut active = Vec::new();
    for (ci, h) in headers.iter().enumerate() {
        let index = if index_column_needs_values(h, allow) {
            let vals: Vec<Cell> = data_rows
                .iter()
                .map(|r| r.get(ci).map(|c| Cell::str(c.value.clone())).unwrap_or(Cell::Absent))
                .collect();
            let refs: Vec<&Cell> = vals.iter().collect();
            is_index_column(h, Some(&refs), allow)
        } else {
            is_index_column(h, None, allow)
        };
        if !index {
            active.push(ci);
        }
    }
    let final_headers: Vec<String> = active.iter().map(|&i| headers[i].clone()).collect();
    let original_rows = data_rows.len();
    let max_rows = options.max_rows.unwrap_or(original_rows);
    let mut table = Table::with_columns(&final_headers);
    let targets: Vec<(usize, usize)> = active
        .iter()
        .map(|&ci| (ci, table.col(&headers[ci]).unwrap_or(0)))
        .collect();
    table.rows.reserve(original_rows.min(max_rows));
    for mut r in rows.into_iter().skip(1).take(max_rows) {
        let mut cells = vec![Cell::Absent; final_headers.len()];
        for &(ci, col) in &targets {
            cells[col] = cell_value_owned(r.get_mut(ci));
        }
        table.push_row(cells);
    }
    Ok(CsvParseResult {
        metadata: CsvMetadata {
            original_rows,
            final_rows: table.len(),
            original_columns: headers.len(),
            final_columns: final_headers.len(),
            file_name: options.file_name.clone(),
            delimiter: delimiter.to_string(),
            has_multiline_fields: multiline,
        },
        data: table,
        headers: final_headers,
    })
}

/// The large-file reader (`StreamingCSVParser`): index columns are judged by name only and at most
/// `max_rows` data rows are kept. Returns the rows, the delimiter and whether the limit was hit.
pub fn parse_streaming(text: &str, max_rows: usize, delimiter: Option<char>) -> (Table, char, bool) {
    let text = strip_bom(text);
    let delimiter = delimiter.unwrap_or_else(|| if text.is_empty() { ',' } else { detect_delimiter(text) });
    let mut headers: Option<Vec<String>> = None;
    let mut active: Vec<usize> = Vec::new();
    let mut table = Table::new();
    let mut truncated = false;

    let mut row: Vec<RawCell> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut quoted = false;
    let mut chars = text.chars().peekable();

    let emit_field = |field: &mut String, quoted: &mut bool, row: &mut Vec<RawCell>| {
        let value = if *quoted {
            std::mem::take(field)
        } else {
            js::trim(&std::mem::take(field)).to_string()
        };
        row.push(RawCell {
            value,
            is_quoted: *quoted,
        });
        *quoted = false;
    };
    let mut emit_row = |row: &mut Vec<RawCell>, headers: &mut Option<Vec<String>>, table: &mut Table| {
        let r = std::mem::take(row);
        if r.is_empty() || r.iter().all(|c| c.value.is_empty()) {
            return;
        }
        match headers {
            None => {
                let h = sanitize_headers(&r.iter().map(|c| c.value.clone()).collect::<Vec<_>>());
                active = (0..h.len()).filter(|&i| !is_index_column(&h[i], None, false)).collect();
                let names: Vec<String> = active.iter().map(|&i| h[i].clone()).collect();
                *table = Table::with_columns(&names);
                *headers = Some(h);
            }
            Some(h) => {
                let mut cells = vec![Cell::Absent; active.len()];
                for &ci in &active {
                    let col = table.col(&h[ci]).unwrap_or(0);
                    cells[col] = cell_value(r.get(ci));
                }
                table.push_row(cells);
            }
        }
    };

    while let Some(c) = chars.next() {
        if headers.is_some() && table.len() >= max_rows {
            truncated = true;
            break;
        }
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else if c == '\\' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    field.push('\\');
                }
            } else {
                field.push(c);
            }
        } else if c == '"' {
            if js::trim(&field).is_empty() {
                in_quotes = true;
                quoted = true;
                field.clear();
            } else {
                field.push(c);
            }
        } else if c == delimiter {
            emit_field(&mut field, &mut quoted, &mut row);
        } else if c == '\r' || c == '\n' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            emit_field(&mut field, &mut quoted, &mut row);
            emit_row(&mut row, &mut headers, &mut table);
        } else {
            field.push(c);
        }
    }
    if !truncated && (in_quotes || quoted || !field.is_empty() || !row.is_empty()) {
        emit_field(&mut field, &mut quoted, &mut row);
        emit_row(&mut row, &mut headers, &mut table);
    }
    if headers.is_some() && table.len() >= max_rows && !truncated && chars.peek().is_some() {
        truncated = true;
    }
    (table, delimiter, truncated)
}
