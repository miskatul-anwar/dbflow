//! Value-level utilities: cleaning, type inference, index-column detection.

use crate::js;
use crate::table::{Cell, Table};
use serde::Serialize;

pub const NULL_TOKENS: [&str; 7] = ["null", "undefined", "n/a", "nil", "nan", "\\n", ""];

/// Normalize attribute names for database compatibility.
pub fn normalize_name(name: &str) -> String {
    let s = re!(r"[\s\-]+").replace_all(name, "_");
    let s = re!(r"[^a-zA-Z0-9_]").replace_all(&s, "");
    let s = re!(r"^_+|_+$").replace_all(&s, "");
    s.to_lowercase()
}

fn is_protected_key(name: &str) -> bool {
    let patterns = [
        re!(r"(?i)^id$"),
        re!(r"(?i)^_id$"),
        re!(r"(?i)^id_$"),
        re!(r"(?i)^pk$"),
        re!(r"(?i)^uuid$"),
        re!(r"(?i)^guid$"),
        re!(r"(?i)^sn$"),
        re!(r"(?i)^s\.?n\.?$"),
        re!(r"(?i)^s_n$"),
        re!(r"(?i)^serial(_?no)?$"),
        re!(r"(?i)^no$"),
        re!(r"(?i)^no\.?$"),
        re!(r"(?i)^num$"),
        re!(r"(?i)^number$"),
        re!(r"(?i)^idx$"),
        re!(r"(?i)^index$"),
        re!(r"(?i)^seq(uence)?$"),
        re!(r"(?i)^row_?id$"),
        re!(r"(?i)^row_?num(ber)?$"),
        re!(r"^[0-9]+$"),
        re!(r"^[#$@%]"),
    ];
    patterns.iter().any(|p| p.is_match(name))
}

/// True when the column is a surrogate index artifact (e.g. pandas `Unnamed: 0`).
pub fn is_index_column(column_name: &str, values: Option<&[&Cell]>, allow_discard_primary_key: bool) -> bool {
    if js::trim(column_name).is_empty() {
        return true;
    }
    let lower = js::trim(column_name).to_lowercase();
    let protected = is_protected_key(&lower);
    if !allow_discard_primary_key && protected {
        return false;
    }
    let artifact = [
        re!(r"(?i)^unnamed:?\s*[0-9]*$"),
        re!(r"(?i)^__index_level_[0-9]+__$"),
        re!(r"(?i)^_c[0-9]+$"),
    ]
    .iter()
    .any(|p| p.is_match(&lower));
    let discardable_pk = allow_discard_primary_key && protected;
    if !artifact && !discardable_pk {
        return false;
    }
    if let Some(values) = values {
        if !values.is_empty() {
            let non_null: Vec<&&Cell> = values
                .iter()
                .filter(|v| !v.is_nullish() && !js::trim(&v.js_string()).is_empty())
                .collect();
            if non_null.is_empty() {
                return artifact;
            }
            let num = |c: &Cell| match c {
                Cell::Num(n) => *n,
                Cell::Bool(b) => *b as u8 as f64,
                other => js::number(&other.js_string()),
            };
            let first = num(non_null[0]);
            if first.fract() != 0.0 || !first.is_finite() || (first != 0.0 && first != 1.0) {
                return false;
            }
            return non_null.iter().enumerate().all(|(i, v)| {
                let n = num(v);
                n.is_finite() && n.fract() == 0.0 && n == first + i as f64
            });
        }
    }
    artifact
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
                29
            } else {
                28
            }
        }
    }
}

fn strict_numeric(s: &str) -> bool {
    re!(r"^[-+]?(?:(?:[0-9]+(?:\.[0-9]*)?)|(?:\.[0-9]+))(?:[eE][-+]?[0-9]+)?$").is_match(s)
}

fn is_date_value(s: &str) -> bool {
    if strict_numeric(s) {
        return false;
    }
    let iso = re!(r"^([0-9]{4})-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])$");
    let iso_dt = re!(
        r"(?i)^([0-9]{4})-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])[T ]([01][0-9]|2[0-3]):([0-5][0-9]):([0-5][0-9])(?:\.[0-9]+)?(?:Z|[+-](?:[01][0-9]|2[0-3]):?[0-5][0-9])?$"
    );
    let slash1 = re!(r"^([0-9]{4})/(0[1-9]|1[0-2])/(0[1-9]|[12][0-9]|3[01])$");
    let slash2 = re!(r"^(0?[1-9]|1[0-2])/(0?[1-9]|[12][0-9]|3[01])/([0-9]{4})$");
    let dash = re!(r"^(0?[1-9]|[12][0-9]|3[01])-(0?[1-9]|1[0-2])-([0-9]{4})$");
    let p = |c: &regex::Captures, i: usize| c.get(i).map(|m| m.as_str().parse::<i64>().unwrap_or(0)).unwrap_or(0);
    let (y, m, d) = if let Some(c) = iso
        .captures(s)
        .or_else(|| slash1.captures(s))
        .or_else(|| iso_dt.captures(s))
    {
        (p(&c, 1), p(&c, 2), p(&c, 3))
    } else if let Some(c) = slash2.captures(s) {
        (p(&c, 3), p(&c, 1), p(&c, 2))
    } else if let Some(c) = dash.captures(s) {
        let (p1, p2) = (p(&c, 1), p(&c, 2));
        if p1 > 12 {
            (p(&c, 3), p2, p1)
        } else {
            (p(&c, 3), p1, p2)
        }
    } else {
        return false;
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return false;
    }
    d as u32 <= days_in_month(y, m as u32)
}

/// Infer a column type from sample values with strict numeric and date validation.
pub fn infer_data_type(values: &[&Cell]) -> String {
    if values.is_empty() {
        return "string".into();
    }
    let null_strings = ["null", "undefined", "nan", "\\n", ""];
    let non_empty: Vec<&Cell> = values
        .iter()
        .copied()
        .filter(|v| {
            if v.is_nullish() {
                return false;
            }
            let s = js::trim(&v.js_string()).to_string();
            !s.is_empty() && !null_strings.contains(&s.to_lowercase().as_str())
        })
        .collect();
    if non_empty.is_empty() {
        return "string".into();
    }
    let sample = &non_empty[..non_empty.len().min(50)];
    let text = |v: &Cell| js::trim(&v.js_string()).to_string();

    if sample
        .iter()
        .all(|v| matches!(v, Cell::Bool(_)) || re!(r"(?i)^(true|false|yes|no)$").is_match(&text(v)))
    {
        return "boolean".into();
    }
    let is_numeric = |v: &Cell| match v {
        Cell::Num(n) => n.is_finite(),
        _ => {
            let s = text(v);
            strict_numeric(&s) && js::number(&s).is_finite()
        }
    };
    if sample.iter().all(|v| is_numeric(v)) {
        let has_decimal = sample.iter().any(|v| {
            let s = text(v);
            s.contains('.') || s.contains('e') || s.contains('E')
        });
        return if has_decimal { "decimal" } else { "integer" }.into();
    }
    if sample.iter().all(|v| is_date_value(&text(v))) {
        return "date".into();
    }
    let email = re!(
        r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)+$"
    );
    if sample.iter().all(|v| email.is_match(&text(v))) {
        return "email".into();
    }
    if sample
        .iter()
        .all(|v| re!(r"(?i)^https?://[^\s/$.?#].[^\s]*$").is_match(&text(v)))
    {
        return "url".into();
    }
    let phone = re!(r"^(?:\+?[0-9]{1,4}[-.\s]?)?(?:\(?[0-9]{2,4}\)?[-.\s]?)?[0-9]{3,4}[-.\s]?[0-9]{3,4}$");
    if sample.iter().all(|v| {
        let s = text(v);
        phone.is_match(&s) && (re!(r"[+\-()]").is_match(&s) || js::len16(&s) >= 7)
    }) {
        return "phone".into();
    }
    "string".into()
}

/// Clean and normalize a value (`null`, `n/a`, ... become empty).
pub fn clean_value(value: &Cell) -> String {
    if value.is_nullish() {
        return String::new();
    }
    clean_str(&value.text())
}

/// Whether `is_index_column` needs the column values for this name (most names are decided by name alone).
pub fn index_column_needs_values(column_name: &str, allow_discard_primary_key: bool) -> bool {
    if js::trim(column_name).is_empty() {
        return false;
    }
    let lower = js::trim(column_name).to_lowercase();
    let protected = is_protected_key(&lower);
    if !allow_discard_primary_key && protected {
        return false;
    }
    let artifact = [
        re!(r"(?i)^unnamed:?\s*[0-9]*$"),
        re!(r"(?i)^__index_level_[0-9]+__$"),
        re!(r"(?i)^_c[0-9]+$"),
    ]
    .iter()
    .any(|p| p.is_match(&lower));
    artifact || (allow_discard_primary_key && protected)
}

/// `cleanValue` for text: trimmed, with null tokens (`null`, `n/a`, ...) mapped to empty.
pub fn clean_str(value: &str) -> String {
    let t = js::trim(value);
    if t.len() <= 9 && NULL_TOKENS.iter().any(|n| t.eq_ignore_ascii_case(n)) {
        String::new()
    } else {
        t.to_string()
    }
}

/// Whether a value is atomic (heuristic used by `isAtomic`).
pub fn is_atomic(value: &Cell) -> bool {
    if !value.truthy() {
        return true;
    }
    let s = js::trim(&value.js_string()).to_string();
    if s.is_empty() {
        return true;
    }
    let pats = [
        re!(r"[,;|&+]"),
        re!(r"(?i)\s+and\s+"),
        re!(r"(?i)\s+or\s+"),
        re!(r"[\[\]{}()]"),
        re!(r"^\{.*\}$"),
        re!(r"^\[.*\]$"),
    ];
    let delims = pats.iter().any(|p| p.is_match(&s));
    let words = re!(r"\s+").split(&s).count();
    !delims && words <= 5
}

pub fn generate_row_hash(rows: &Table, row: usize, attributes: &[String]) -> String {
    attributes
        .iter()
        .map(|a| {
            let c = rows.get(row, a);
            if c.is_nullish() {
                String::new()
            } else {
                clean_value(c)
            }
        })
        .collect::<Vec<_>>()
        .join("||")
}

pub fn remove_duplicates(rows: &Table, attributes: &[String]) -> Table {
    let mut seen = std::collections::HashSet::new();
    let mut keep = Vec::new();
    for r in 0..rows.len() {
        if seen.insert(generate_row_hash(rows, r, attributes)) {
            keep.push(r);
        }
    }
    rows.select_rows(&keep)
}

const RESERVED: [&str; 26] = [
    "select",
    "from",
    "where",
    "insert",
    "update",
    "delete",
    "create",
    "drop",
    "alter",
    "table",
    "index",
    "view",
    "database",
    "schema",
    "primary",
    "foreign",
    "key",
    "constraint",
    "null",
    "not",
    "unique",
    "default",
    "check",
    "references",
    "",
    "",
];

fn reserved(name: &str) -> bool {
    RESERVED[..24].contains(&name)
}

pub fn is_valid_attribute_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    if reserved(js::trim(name).to_lowercase().as_str()) {
        return false;
    }
    re!(r"^[a-zA-Z][a-zA-Z0-9_]*$").is_match(name)
}

pub fn suggest_valid_attribute_name(name: &str) -> String {
    if name.is_empty() {
        return "attribute".into();
    }
    let mut s = normalize_name(name);
    if !re!(r"^[a-zA-Z]").is_match(&s) {
        s = format!("attr_{s}");
    }
    if reserved(&s.to_lowercase()) {
        s.push_str("_attr");
    }
    if s.is_empty() {
        "attribute".into()
    } else {
        s
    }
}

pub fn are_values_equal(a: &Cell, b: &Cell) -> bool {
    clean_value(a).to_lowercase() == clean_value(b).to_lowercase()
}

#[derive(Serialize)]
pub struct NumericStats {
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub median: f64,
    #[serde(rename = "stdDev")]
    pub std_dev: f64,
    pub count: usize,
}

fn round2(x: f64) -> f64 {
    js::number(&js::to_fixed(x, 2))
}

pub fn calculate_numeric_stats(values: &[&Cell]) -> Option<NumericStats> {
    let nums: Vec<f64> = values
        .iter()
        .map(|v| js::parse_float(&clean_value(v)))
        .filter(|v| v.is_finite())
        .collect();
    if nums.is_empty() {
        return None;
    }
    let mut sorted = nums.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let count = nums.len();
    let mean = nums.iter().sum::<f64>() / count as f64;
    let variance = nums.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count as f64;
    let median = if count % 2 == 0 {
        (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
    } else {
        sorted[count / 2]
    };
    Some(NumericStats {
        min: sorted[0],
        max: sorted[count - 1],
        mean: round2(mean),
        median: round2(median),
        std_dev: round2(variance.sqrt()),
        count,
    })
}

pub fn format_value_for_display(value: &Cell, max_length: usize) -> String {
    if value.is_nullish() {
        return "<null>".into();
    }
    let s = value.js_string();
    if js::len16(&s) <= max_length {
        s
    } else {
        format!("{}...", js::substring16(&s, max_length.saturating_sub(3)))
    }
}

pub fn parse_delimited_value(value: &str, delimiter: &str) -> Vec<String> {
    if value.is_empty() {
        return vec![];
    }
    value
        .split(delimiter)
        .map(|v| js::trim(v).to_string())
        .filter(|v| !v.is_empty())
        .collect()
}
