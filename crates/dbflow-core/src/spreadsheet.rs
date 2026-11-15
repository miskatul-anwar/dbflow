//! Excel / OpenDocument workbooks: every visible, non-empty sheet becomes one CSV table.

use crate::input_check;
use calamine::{open_workbook_auto_from_rs, Data, Reader, SheetVisible};
use std::io::Cursor;

pub const SPREADSHEET_EXTENSIONS: [&str; 13] = [
    "xlsx", "xlsm", "xlsb", "xltx", "xltm", "xlam", "xls", "xlt", "xla", "xlw", "ods", "ots", "fods",
];
const MAX_BYTES: usize = 100 * 1024 * 1024;
const MAX_ROWS_PER_SHEET: usize = 1_000_000;

pub fn ext_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((_, e)) if !e.contains('/') && !e.contains('\\') && !e.is_empty() => e.to_lowercase(),
        _ => String::new(),
    }
}

pub fn is_spreadsheet_name(name: &str) -> bool {
    SPREADSHEET_EXTENSIONS.contains(&ext_of(name).as_str())
}

pub fn is_spreadsheet_xml(name: &str, bytes: &[u8]) -> bool {
    if ext_of(name) != "xml" {
        return false;
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    head.contains("urn:schemas-microsoft-com:office:spreadsheet")
}

pub fn is_spreadsheet(name: &str, bytes: &[u8]) -> bool {
    is_spreadsheet_name(name) || is_spreadsheet_xml(name, bytes)
}

#[derive(PartialEq)]
enum Kind {
    Binary,
    Xml,
    Text,
    Unknown,
}

fn sniff(b: &[u8]) -> Kind {
    if b.len() >= 4 && b[0] == 0x50 && b[1] == 0x4b && matches!(b[2], 0x03 | 0x05 | 0x07) {
        return Kind::Binary;
    }
    if b.len() >= 8 && b[..4] == [0xd0, 0xcf, 0x11, 0xe0] {
        return Kind::Binary;
    }
    if b.len() >= 2 && b[0] == 0x09 && matches!(b[1], 0x00 | 0x02 | 0x04 | 0x08) {
        return Kind::Binary;
    }
    let head = &b[..b.len().min(4096)];
    if head.contains(&0) {
        return Kind::Unknown;
    }
    let mut i = if head.starts_with(&[0xef, 0xbb, 0xbf]) { 3 } else { 0 };
    while i < head.len() && matches!(head[i], 0x20 | 0x0a | 0x0d | 0x09) {
        i += 1;
    }
    if head.get(i) == Some(&b'<') {
        Kind::Xml
    } else {
        Kind::Text
    }
}

fn pad(n: i64, w: usize) -> String {
    format!("{n:0w$}")
}

fn format_serial(serial: f64) -> String {
    let total = (serial * 86_400.0).round() as i64;
    let days = total.div_euclid(86_400);
    let secs = total.rem_euclid(86_400);
    // Excel's epoch is 1899-12-30.
    let epoch = crate::js::days_from_civil(1899, 12, 30);
    let ms = (epoch + days) as f64 * 86_400_000.0;
    let iso = crate::js::iso_from_ms(ms);
    let y: i64 = iso[..4].parse().unwrap_or(0);
    let time = format!(
        "{}:{}:{}",
        pad(secs / 3600, 2),
        pad((secs / 60) % 60, 2),
        pad(secs % 60, 2)
    );
    if y < 1900 {
        return time;
    }
    let date = &iso[..10];
    if time == "00:00:00" {
        date.to_string()
    } else {
        format!("{date} {time}")
    }
}

fn number_text(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    if v.fract() == 0.0 {
        return crate::js::num_to_string(v);
    }
    let p: f64 = format!("{v:.14e}").parse().unwrap_or(v);
    crate::js::num_to_string(p)
}

fn cell_text(d: &Data) -> String {
    match d {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => number_text(*f),
        Data::Bool(b) => if *b { "true" } else { "false" }.into(),
        Data::DateTime(dt) => {
            if dt.is_datetime() {
                format_serial(dt.as_f64())
            } else {
                number_text(dt.as_f64())
            }
        }
        Data::DateTimeIso(s) => iso_text(s),
        Data::DurationIso(s) => s.clone(),
    }
}

/// ISO 8601 cell text in the same shape as serial dates: `2024-03-05`, `2024-03-05 14:30:00` or `14:30:00`.
fn iso_text(s: &str) -> String {
    let c = crate::re!(r"^(?:([0-9]{4}-[0-9]{2}-[0-9]{2}))?(?:[T ]?([0-9]{2}:[0-9]{2}(?::[0-9]{2})?))?(?:\.[0-9]+)?(?:Z|[+-][0-9]{2}:?[0-9]{2})?$").captures(s);
    let Some(c) = c else { return s.to_string() };
    let date = c.get(1).map(|m| m.as_str());
    let time = c.get(2).map(|m| {
        if m.as_str().len() == 5 {
            format!("{}:00", m.as_str())
        } else {
            m.as_str().to_string()
        }
    });
    match (date, time) {
        (Some(d), Some(t)) if t != "00:00:00" => format!("{d} {t}"),
        (Some(d), _) => d.to_string(),
        (None, Some(t)) => t,
        (None, None) => s.to_string(),
    }
}

pub(crate) fn csv_field(s: &str) -> String {
    if s.contains('"') || s.contains(',') || s.contains('\r') || s.contains('\n') || s != crate::js::trim(s) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn safe(s: &str) -> String {
    let mut out = String::new();
    let mut bad = false;
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' || c == '.' || c == '-' {
            out.push(c);
            bad = false;
        } else if !bad {
            out.push('_');
            bad = true;
        }
    }
    let t = out.trim_matches('_').to_string();
    if t.is_empty() {
        "sheet".into()
    } else {
        t
    }
}

fn strip_ext(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((b, e)) if !e.contains('/') && !e.contains('\\') => b,
        _ => name,
    }
}

pub struct Converted {
    /// (file name, CSV text).
    pub files: Vec<(String, Vec<u8>)>,
    pub notes: Vec<String>,
}

/// Checks that run before any reader opens a workbook (this one, or the interface's fallback reader):
/// the size limit, files that are clearly something else (a PDF, an image, a program), and how far a
/// ZIP-based workbook expands when unzipped.
pub fn check_workbook(name: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_BYTES {
        return Err(format!("{name}: file exceeds the 100MB limit."));
    }
    input_check::reject_known_non_table(name, bytes)?;
    if bytes.starts_with(b"PK") {
        let summary = input_check::check_zip_expansion(name, bytes)?;
        if !summary.entries.is_empty() {
            if let Some(kind) = input_check::describe_non_spreadsheet_zip(&summary.entries, bytes) {
                return Err(format!("{name} is {kind}, not a spreadsheet."));
            }
        }
    }
    Ok(())
}

/// XML the interface's fallback reader can turn into a table: Excel 2003 SpreadsheetML, flat
/// OpenDocument spreadsheets and HTML tables (Excel's "web page" export, often saved as .xls).
fn is_convertible_xml(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_lowercase();
    head.contains("urn:schemas-microsoft-com:office:spreadsheet")
        || head.contains("opendocument.spreadsheet")
        || head.contains("<html")
        || head.contains("<table")
}

/// Convert a workbook into one CSV per visible, non-empty sheet.
pub fn spreadsheet_to_csv(name: &str, bytes: &[u8]) -> Result<Converted, String> {
    check_workbook(name, bytes)?;
    match sniff(bytes) {
        Kind::Unknown => {
            return Err(format!(
                "{name}: not a valid spreadsheet (the file is damaged or in an unsupported format)."
            ))
        }
        Kind::Text => {
            input_check::check_text_table(name, bytes)?;
            return Ok(Converted {
                files: vec![(format!("{}.csv", strip_ext(name)), bytes.to_vec())],
                notes: vec![format!("{name}: contains delimited text, read as CSV.")],
            });
        }
        Kind::Xml if !is_convertible_xml(bytes) => {
            return Err(format!(
                "{name} contains XML that is not a spreadsheet. Save it as CSV or as a spreadsheet first."
            ));
        }
        Kind::Xml => {
            return Err(format!(
                "{name}: XML spreadsheets are converted by the interface before analysis."
            ));
        }
        Kind::Binary => {}
    }
    let mut wb = open_workbook_auto_from_rs(Cursor::new(bytes.to_vec())).map_err(|e| {
        let msg = e.to_string();
        if msg.to_lowercase().contains("password") || msg.to_lowercase().contains("encrypt") {
            format!("{name}: the workbook is password-protected or encrypted. Remove the password and try again.")
        } else {
            format!("{name}: could not read the spreadsheet ({msg}).")
        }
    })?;
    let base = strip_ext(name).to_string();
    let mut notes = Vec::new();
    let mut tables: Vec<(String, String)> = Vec::new();
    let sheets = wb.sheets_metadata().to_vec();
    for sheet in sheets {
        if sheet.visible != SheetVisible::Visible {
            notes.push(format!("{name}: hidden sheet \"{}\" skipped.", sheet.name));
            continue;
        }
        let Ok(range) = wb.worksheet_range(&sheet.name) else {
            continue;
        };
        let grid: Vec<Vec<String>> = range
            .rows()
            .map(|r| r.iter().map(cell_text).collect::<Vec<_>>())
            .filter(|r| r.iter().any(|c| !c.is_empty()))
            .collect();
        if grid.is_empty() {
            notes.push(format!("{name}: sheet \"{}\" is empty and was skipped.", sheet.name));
            continue;
        }
        let width = grid.iter().map(|r| r.len()).max().unwrap_or(0);
        let used: Vec<usize> = (0..width)
            .filter(|&c| grid.iter().any(|r| r.get(c).is_some_and(|v| !v.is_empty())))
            .collect();
        let mut lines: Vec<String> = grid
            .iter()
            .map(|r| {
                used.iter()
                    .map(|&c| csv_field(r.get(c).map(|s| s.as_str()).unwrap_or("")))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .collect();
        if lines.len() > MAX_ROWS_PER_SHEET + 1 {
            notes.push(format!(
                "{name}: sheet \"{}\" has more than {} rows; only the first {} are analysed.",
                sheet.name,
                crate::js::locale_usize(MAX_ROWS_PER_SHEET),
                crate::js::locale_usize(MAX_ROWS_PER_SHEET)
            ));
            lines.truncate(MAX_ROWS_PER_SHEET + 1);
        }
        tables.push((sheet.name.clone(), lines.join("\n")));
    }
    if tables.is_empty() {
        return Err(format!("{name}: the workbook has no visible sheets with data."));
    }
    let mut used = std::collections::HashSet::new();
    let single = tables.len() == 1;
    let files = tables
        .into_iter()
        .map(|(sheet, csv)| {
            let mut fname = if single {
                format!("{}.csv", safe(&base))
            } else {
                format!("{}_{}.csv", safe(&base), safe(&sheet))
            };
            let mut n = 2;
            while used.contains(&fname.to_lowercase()) {
                let stem = crate::re!(r"(_[0-9]+)?\.csv$").replace(&fname, "").to_string();
                fname = format!("{stem}_{n}.csv");
                n += 1;
            }
            used.insert(fname.to_lowercase());
            (fname, csv.into_bytes())
        })
        .collect();
    Ok(Converted { files, notes })
}
