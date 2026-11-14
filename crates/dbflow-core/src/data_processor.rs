//! Data cleaning, atomicity (multi-valued column) detection and type profiling.

use crate::data_utils::{clean_value, infer_data_type};
use crate::js;
use crate::table::{Cell, Table};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Remove empty rows and clean every value (`DataProcessor.clean`).
pub fn clean(data: &Table) -> Table {
    let mut out = Table::new();
    // Rows that have every column (the usual case) share one key order and one output layout.
    let all_cols: Vec<String> = data.columns_raw().to_vec();
    let ordered: Vec<usize> = crate::table::js_key_order(&all_cols);
    let full_layout: Vec<(usize, String)> = ordered
        .iter()
        .map(|&c| (c, js::trim(&all_cols[c]).to_string()))
        .collect();
    let mut full_targets: Option<Vec<Option<usize>>> = None;
    for r in 0..data.len() {
        let row = &data.rows[r];
        let complete = row.iter().all(|c| !matches!(c, Cell::Absent));
        let layout: std::borrow::Cow<'_, [(usize, String)]> = if complete {
            std::borrow::Cow::Borrowed(&full_layout)
        } else {
            let keys = data.keys_of(r);
            std::borrow::Cow::Owned(
                keys.iter()
                    .map(|k| (data.col(k).unwrap_or(0), js::trim(k).to_string()))
                    .collect(),
            )
        };
        let values: Vec<String> = layout.iter().map(|(c, _)| clean_value(&row[*c])).collect();
        if values.iter().all(|v| v.is_empty()) || layout.iter().all(|(_, k)| k.is_empty()) {
            continue;
        }
        if layout
            .iter()
            .zip(&values)
            .filter(|((_, k), _)| !k.is_empty())
            .all(|(_, v)| v.is_empty())
        {
            continue;
        }
        // Later duplicates of a trimmed key overwrite earlier ones (object assignment order).
        let targets: Vec<Option<usize>> = if complete && full_targets.is_some() {
            full_targets.clone().unwrap_or_default()
        } else {
            let t: Vec<Option<usize>> = layout
                .iter()
                .map(|(_, k)| {
                    if k.is_empty() {
                        None
                    } else {
                        Some(out.col_index_or_add(k))
                    }
                })
                .collect();
            if complete {
                full_targets = Some(t.clone());
            }
            t
        };
        let mut cells = vec![Cell::Absent; out.columns_raw().len()];
        for (t, v) in targets.iter().zip(values) {
            if let Some(i) = t {
                cells[*i] = Cell::Str(v);
            }
        }
        out.push_row(cells);
    }
    out
}

/// Split `text` into list items if it looks like a list, else `None`.
pub fn list_tokens(text: &str) -> Option<Vec<String>> {
    if re!(r"^\[.*\]$").is_match(text) {
        if let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(text) {
            return Some(items.iter().map(crate::table::json_to_js_string).collect());
        }
    }
    for d in [";", "|", ",", " / "] {
        if !text.contains(d) {
            continue;
        }
        let tokens: Vec<String> = text
            .split(d)
            .map(|t| js::trim(t).to_string())
            .filter(|t| !t.is_empty())
            .collect();
        if tokens.len() >= 2 && tokens.iter().all(|t| re!(r"\s+").split(t).count() <= 3) {
            return Some(tokens);
        }
    }
    None
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Violation {
    pub row: usize,
    pub value: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AtomicityResult {
    pub atomicity: IndexMap<String, bool>,
    pub violations: IndexMap<String, Vec<Violation>>,
}

fn free_text_column(name: &str) -> bool {
    re!(r"(?i)(^|[_\s-])(name|address|street|city|location|desc|description|comment|note|notes|title|text|remark|remarks|phone|email|url|summary|bio)($|[_\s-])")
        .is_match(name)
}

pub fn check_atomicity(data: &Table) -> AtomicityResult {
    let mut res = AtomicityResult::default();
    if data.is_empty() {
        return res;
    }
    for attr in data.keys() {
        let mut list_like = Vec::new();
        let mut non_empty = 0usize;
        let mut token_words = 0usize;
        let mut token_count = 0usize;
        let col = data.col(&attr);
        for r in 0..data.len() {
            let v = data.get_idx(r, col);
            if v.is_nullish() {
                continue;
            }
            let text = js::trim(&v.js_string()).to_string();
            if text.is_empty() {
                continue;
            }
            non_empty += 1;
            if let Some(tokens) = list_tokens(&text) {
                list_like.push(Violation {
                    row: r + 1,
                    value: js::substring16(&text, 100),
                });
                for t in tokens {
                    token_count += 1;
                    token_words += re!(r"\s+").split(&t).count();
                }
            }
        }
        let share = if non_empty > 0 {
            list_like.len() as f64 / non_empty as f64
        } else {
            0.0
        };
        let avg_words = if token_count > 0 {
            token_words as f64 / token_count as f64
        } else {
            0.0
        };
        let free_text = free_text_column(&attr) || avg_words > 2.0;
        let atomic = !(share >= 0.3 && !free_text);
        res.atomicity.insert(attr.clone(), atomic);
        if !atomic {
            list_like.truncate(5);
            res.violations.insert(attr, list_like);
        }
    }
    res
}

pub fn profile_data_types(data: &Table) -> IndexMap<String, String> {
    let mut types = IndexMap::new();
    if data.is_empty() {
        return types;
    }
    for attr in data.keys() {
        let vals: Vec<&Cell> = data
            .column(&attr)
            .into_iter()
            .filter(|v| !v.is_nullish() && !js::trim(&v.js_string()).is_empty())
            .collect();
        types.insert(attr, infer_data_type(&vals));
    }
    types
}

/// Evenly spaced sample of at most `target` rows (`getProgressiveSample`).
pub fn progressive_sample(data: &Table, target: usize) -> Table {
    if data.len() <= target {
        return data.clone();
    }
    let step = data.len() / target;
    let mut idx = Vec::new();
    let mut i = 0;
    while i < data.len() && idx.len() < target {
        idx.push(i);
        i += step;
    }
    data.select_rows(&idx)
}
