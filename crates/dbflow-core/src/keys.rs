//! Primary-key selection, 1NF child tables and instance-level lossless verification.

use crate::data_processor::list_tokens;
use crate::js;
use crate::naming::new_relation_id;
use crate::table::{Cell, Table};
use crate::types::{Metadata, Relation};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const SURROGATE_KEY_COLUMN: &str = "row_id";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyChoice {
    pub primary_key: Vec<String>,
    pub rule: String,
    pub reason: String,
    pub clean_keys: Vec<Vec<String>>,
    pub surrogate_added: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surrogate_column: Option<String>,
}

/// True when `cols` identifies every row and no key cell is missing.
pub fn is_unique_and_complete(rows: &Table, cols: &[String]) -> bool {
    if rows.is_empty() || cols.is_empty() {
        return false;
    }
    let idx: Vec<Option<usize>> = cols.iter().map(|c| rows.col(c)).collect();
    let mut seen = HashSet::with_capacity(rows.len());
    for r in 0..rows.len() {
        let mut parts = Vec::with_capacity(cols.len());
        for &c in &idx {
            let v = rows.get_idx(r, c);
            if v.is_missing() {
                return false;
            }
            parts.push(v.js_string());
        }
        if !seen.insert(parts.join("\u{1}")) {
            return false;
        }
    }
    true
}

fn looks_like_identifier(name: &str) -> u8 {
    let n = name.to_lowercase();
    if n == "id" || n == "uuid" || n == "guid" {
        return 3;
    }
    if re!(r"(^|[_\s-])id$").is_match(&n) || re!(r"[a-z0-9](ID|Id)$").is_match(name) {
        return 2;
    }
    if re!(r"(^|[_\s-])(no|num|number|code|key)$").is_match(&n) {
        return 1;
    }
    0
}

/// Choose a primary key; adds a surrogate column to `rows`/`attributes` when nothing usable exists.
pub fn choose_key(
    attributes: &mut Vec<String>,
    rows: &mut Table,
    candidate_keys: &[Vec<String>],
    table_name: &str,
) -> KeyChoice {
    let unique: Vec<&Vec<String>> = candidate_keys
        .iter()
        .filter(|k| !k.is_empty() && k.iter().all(|c| attributes.contains(c)))
        .collect();
    let clean: Vec<Vec<String>> = unique
        .into_iter()
        .filter(|k| is_unique_and_complete(rows, k))
        .cloned()
        .collect();
    let mut clean_keys = clean.clone();

    let mut single: indexmap::IndexSet<String> = clean.iter().filter(|k| k.len() == 1).map(|k| k[0].clone()).collect();
    for col in attributes.iter() {
        if !single.contains(col)
            && looks_like_identifier(col) > 0
            && is_unique_and_complete(rows, std::slice::from_ref(col))
        {
            single.insert(col.clone());
            clean_keys.push(vec![col.clone()]);
        }
    }
    let pos = |c: &str| attributes.iter().position(|a| a == c).map(|p| p as i64).unwrap_or(-1);
    let mut id_cols: Vec<(String, u8)> = single
        .iter()
        .map(|c| (c.clone(), looks_like_identifier(c)))
        .filter(|(_, s)| *s > 0)
        .collect();
    id_cols.sort_by(|a, b| b.1.cmp(&a.1).then(pos(&a.0).cmp(&pos(&b.0))));
    if let Some((c, _)) = id_cols.first() {
        return KeyChoice {
            primary_key: vec![c.clone()],
            rule: "identifier-column".into(),
            reason: format!("\"{c}\" is unique, has no missing values and is named like an identifier."),
            clean_keys,
            surrogate_added: false,
            surrogate_column: None,
        };
    }
    if !clean.is_empty() {
        let mut sorted = clean.clone();
        let min_pos = |k: &Vec<String>| k.iter().map(|c| pos(c)).min().unwrap_or(i64::MAX);
        sorted.sort_by(|a, b| a.len().cmp(&b.len()).then(min_pos(a).cmp(&min_pos(b))));
        let best = sorted[0].clone();
        return KeyChoice {
            primary_key: best.clone(),
            rule: "candidate-key".into(),
            reason: format!(
                "Smallest candidate key that is unique and complete: {{{}}}.",
                best.join(", ")
            ),
            clean_keys,
            surrogate_added: false,
            surrogate_column: None,
        };
    }
    let mut column = SURROGATE_KEY_COLUMN.to_string();
    let mut n = 1;
    while attributes.contains(&column) {
        n += 1;
        column = format!("{SURROGATE_KEY_COLUMN}_{n}");
    }
    for r in 0..rows.len() {
        rows.set(r, &column, Cell::Num((r + 1) as f64));
    }
    attributes.insert(0, column.clone());
    KeyChoice {
        primary_key: vec![column.clone()],
        rule: "surrogate".into(),
        reason: format!(
            "No candidate key of \"{table_name}\" is both unique and free of missing values (duplicate rows or incomplete key columns), so a surrogate key \"{column}\" was added."
        ),
        clean_keys: vec![vec![column.clone()]],
        surrogate_added: true,
        surrogate_column: Some(column),
    }
}

pub struct AtomicSplit {
    pub parent_rows: Table,
    pub parent_attributes: Vec<String>,
    pub key: Vec<String>,
    pub children: Vec<Relation>,
    pub moved: Vec<String>,
}

fn values_of(cell: &Cell) -> Vec<String> {
    if cell.is_missing() {
        return vec![];
    }
    let text = js::trim(&cell.js_string()).to_string();
    list_tokens(&text).unwrap_or_else(|| vec![text])
}

/// Move multi-valued columns into child relations `(key…, column)`.
pub fn split_multi_valued_columns(rows: &Table, multi_valued: &[String], table_name: &str) -> Option<AtomicSplit> {
    if rows.is_empty() {
        return None;
    }
    let all = rows.keys();
    let moved: Vec<String> = multi_valued.iter().filter(|c| all.contains(c)).cloned().collect();
    let remaining: Vec<String> = all.iter().filter(|c| !moved.contains(c)).cloned().collect();
    if moved.is_empty() || remaining.is_empty() {
        return None;
    }
    let mut parent_rows = rows.project(&remaining, |c| c.clone());
    let mut parent_attributes = remaining.clone();
    let unique_singles: Vec<Vec<String>> = remaining
        .iter()
        .filter(|c| {
            let set: HashSet<String> = (0..parent_rows.len())
                .map(|r| parent_rows.get(r, c).or_empty())
                .collect();
            !parent_rows.is_empty() && set.len() == parent_rows.len()
        })
        .map(|c| vec![c.clone()])
        .collect();
    let choice = choose_key(&mut parent_attributes, &mut parent_rows, &unique_singles, table_name);
    let key = choice.primary_key.clone();

    let mut children = Vec::new();
    for col in &moved {
        let mut child_cols = key.clone();
        child_cols.push(col.clone());
        let mut child = Table::with_columns(&child_cols);
        let mut seen = HashSet::new();
        for r in 0..rows.len() {
            let key_vals: Vec<Cell> = key.iter().map(|k| parent_rows.get(r, k).clone()).collect();
            for value in values_of(rows.get(r, col)) {
                let mut sig: Vec<serde_json::Value> = key_vals.iter().map(|c| c.to_json()).collect();
                sig.push(serde_json::Value::String(value.clone()));
                if !seen.insert(serde_json::to_string(&sig).unwrap_or_default()) {
                    continue;
                }
                let mut cells = key_vals.clone();
                cells.push(Cell::str(value));
                let mut ordered = vec![Cell::Absent; child.columns_raw().len()];
                for (name, v) in child_cols.iter().zip(cells) {
                    let i = child.col(name).unwrap_or(0);
                    ordered[i] = v;
                }
                child.push_row(ordered);
            }
        }
        let count = child.len();
        children.push(Relation {
            name: new_relation_id(),
            attributes: child_cols.clone(),
            primary_key: child_cols,
            rows: child,
            source_type: Some("1NF".into()),
            metadata: Metadata {
                row_count: count as f64,
                attribute_types: Default::default(),
            },
            ..Default::default()
        });
    }
    Some(AtomicSplit {
        parent_rows,
        parent_attributes,
        key,
        children,
        moved,
    })
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceLossless {
    pub lossless: Option<bool>,
    pub input_rows: usize,
    pub joined_rows: usize,
    pub spurious_rows: usize,
    pub missing_rows: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Natural-join the projections of `rows` and compare with the distinct input rows.
///
/// Values are compared as text (`String(v)`, missing = empty); each column is integer-coded once so
/// the join and the set comparisons work on small integer tuples.
pub fn verify_lossless_on_data(rows: &Table, relation_attrs: &[Vec<String>], max_join_rows: usize) -> InstanceLossless {
    let empty = InstanceLossless {
        lossless: None,
        input_rows: rows.len(),
        ..Default::default()
    };
    if rows.is_empty() || relation_attrs.is_empty() {
        return InstanceLossless {
            lossless: if rows.is_empty() { Some(true) } else { None },
            note: Some("No data to verify.".into()),
            ..empty
        };
    }
    let all_attrs = rows.keys();
    let covered: HashSet<&String> = relation_attrs.iter().flatten().collect();
    if !all_attrs.iter().all(|a| covered.contains(a)) {
        return InstanceLossless {
            lossless: Some(false),
            note: Some("Some attributes are not in any relation.".into()),
            ..empty
        };
    }
    // Integer codes per attribute (same text, same code), one column per task.
    let names: Vec<&String> = crate::types::dedup(all_attrs.iter().chain(relation_attrs.iter().flatten()).cloned())
        .into_iter()
        .filter_map(|n| {
            all_attrs
                .iter()
                .chain(relation_attrs.iter().flatten())
                .find(|x| **x == n)
        })
        .collect();
    let encoded: Vec<Vec<u32>> = crate::par_map(&names, |a| {
        let col = rows.col(a);
        let mut dict: std::collections::HashMap<std::borrow::Cow<'_, str>, u32> = std::collections::HashMap::new();
        (0..rows.len())
            .map(|r| {
                let n = dict.len() as u32;
                *dict.entry(rows.get_idx(r, col).text_or_empty()).or_insert(n)
            })
            .collect()
    });
    let codes: std::collections::HashMap<&str, Vec<u32>> = names.iter().map(|n| n.as_str()).zip(encoded).collect();
    let tuple = |r: usize, attrs: &[String]| -> Vec<u32> { attrs.iter().map(|a| codes[a.as_str()][r]).collect() };
    let input_set: HashSet<Vec<u32>> = (0..rows.len()).map(|r| tuple(r, &all_attrs)).collect();

    let mut projections: Vec<(Vec<String>, Vec<Vec<u32>>)> = relation_attrs
        .iter()
        .map(|attrs| {
            let mut seen = HashSet::new();
            let mut out = Vec::new();
            for r in 0..rows.len() {
                let t = tuple(r, attrs);
                if seen.insert(t.clone()) {
                    out.push(t);
                }
            }
            (attrs.clone(), out)
        })
        .collect();
    projections.sort_by_key(|p| p.1.len());
    let (mut current_attrs, mut current) = projections.remove(0);
    let mut remaining = projections;
    while !remaining.is_empty() {
        let idx = remaining
            .iter()
            .position(|p| p.0.iter().any(|a| current_attrs.contains(a)))
            .unwrap_or(0);
        let (next_attrs, next_rows) = remaining.remove(idx);
        let shared: Vec<&String> = next_attrs.iter().filter(|a| current_attrs.contains(a)).collect();
        let shared_next: Vec<usize> = shared
            .iter()
            .map(|a| next_attrs.iter().position(|x| x == *a).unwrap_or(0))
            .collect();
        let shared_cur: Vec<usize> = shared
            .iter()
            .map(|a| current_attrs.iter().position(|x| x == *a).unwrap_or(0))
            .collect();
        let mut index: std::collections::HashMap<Vec<u32>, Vec<usize>> = std::collections::HashMap::new();
        for (i, r) in next_rows.iter().enumerate() {
            index
                .entry(shared_next.iter().map(|&c| r[c]).collect())
                .or_default()
                .push(i);
        }
        let added: Vec<usize> = next_attrs
            .iter()
            .enumerate()
            .filter(|(_, a)| !current_attrs.contains(a))
            .map(|(i, _)| i)
            .collect();
        let mut joined: Vec<Vec<u32>> = Vec::new();
        for left in &current {
            let k: Vec<u32> = shared_cur.iter().map(|&c| left[c]).collect();
            let Some(bucket) = index.get(&k) else { continue };
            for &ri in bucket {
                let mut row = left.clone();
                row.extend(added.iter().map(|&i| next_rows[ri][i]));
                joined.push(row);
                if joined.len() > max_join_rows {
                    return InstanceLossless {
                        lossless: None,
                        joined_rows: joined.len(),
                        note: Some(format!(
                            "Join exceeded {} rows; the decomposition is very likely lossy.",
                            js::locale_usize(max_join_rows)
                        )),
                        ..empty
                    };
                }
            }
        }
        current = joined;
        current_attrs.extend(added.iter().map(|&i| next_attrs[i].clone()));
    }
    let pos: Vec<Option<usize>> = all_attrs
        .iter()
        .map(|a| current_attrs.iter().position(|x| x == a))
        .collect();
    let empty_code = |a: &str| codes.get(a).and_then(|c| c.first().copied());
    let joined_set: HashSet<Vec<u32>> = current
        .iter()
        .map(|r| {
            pos.iter()
                .zip(&all_attrs)
                .map(|(p, a)| p.map(|i| r[i]).unwrap_or_else(|| empty_code(a).unwrap_or(u32::MAX)))
                .collect()
        })
        .collect();
    let spurious = joined_set.iter().filter(|k| !input_set.contains(*k)).count();
    let missing = input_set.iter().filter(|k| !joined_set.contains(*k)).count();
    InstanceLossless {
        lossless: Some(spurious == 0 && missing == 0),
        input_rows: input_set.len(),
        joined_rows: joined_set.len(),
        spurious_rows: spurious,
        missing_rows: missing,
        note: None,
    }
}
