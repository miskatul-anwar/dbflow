//! Row storage.
//!
//! Rows are kept column-oriented by name with a shared column list, which is
//! far more compact than one map per row. A cell can be `Absent` (the key is
//! not on that row), which keeps JavaScript object semantics for rows that do
//! not all have the same keys.

use crate::js;
use serde::de::{Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Cell {
    #[default]
    Absent,
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Json(Value),
}

impl Cell {
    pub fn str(s: impl Into<String>) -> Cell {
        Cell::Str(s.into())
    }

    pub fn from_json(v: Value) -> Cell {
        match v {
            Value::Null => Cell::Null,
            Value::Bool(b) => Cell::Bool(b),
            Value::Number(n) => Cell::Num(n.as_f64().unwrap_or(f64::NAN)),
            Value::String(s) => Cell::Str(s),
            other => Cell::Json(other),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Cell::Absent | Cell::Null => Value::Null,
            Cell::Bool(b) => Value::Bool(*b),
            Cell::Num(n) => num_json(*n),
            Cell::Str(s) => Value::String(s.clone()),
            Cell::Json(v) => v.clone(),
        }
    }

    /// `v === null || v === undefined`.
    pub fn is_nullish(&self) -> bool {
        matches!(self, Cell::Absent | Cell::Null)
    }

    /// null, undefined or a blank string.
    pub fn is_missing(&self) -> bool {
        match self {
            Cell::Absent | Cell::Null => true,
            Cell::Str(s) => js::trim(s).is_empty(),
            _ => false,
        }
    }

    /// `v === null || v === undefined || v === ''`.
    pub fn is_empty_like(&self) -> bool {
        match self {
            Cell::Absent | Cell::Null => true,
            Cell::Str(s) => s.is_empty(),
            _ => false,
        }
    }

    /// JavaScript truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            Cell::Absent | Cell::Null => false,
            Cell::Bool(b) => *b,
            Cell::Num(n) => *n != 0.0 && !n.is_nan(),
            Cell::Str(s) => !s.is_empty(),
            Cell::Json(_) => true,
        }
    }

    /// `String(v)`.
    pub fn js_string(&self) -> String {
        match self {
            Cell::Absent => "undefined".into(),
            Cell::Null => "null".into(),
            Cell::Bool(b) => b.to_string(),
            Cell::Num(n) => js::num_to_string(*n),
            Cell::Str(s) => s.clone(),
            Cell::Json(v) => json_to_js_string(v),
        }
    }

    /// `String(v)` without copying string cells.
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        match self {
            Cell::Str(s) => std::borrow::Cow::Borrowed(s.as_str()),
            other => std::borrow::Cow::Owned(other.js_string()),
        }
    }

    /// `(v ?? '').toString()` without copying string cells.
    pub fn text_or_empty(&self) -> std::borrow::Cow<'_, str> {
        if self.is_nullish() {
            std::borrow::Cow::Borrowed("")
        } else {
            self.text()
        }
    }

    /// `(v ?? '').toString()`.
    pub fn or_empty(&self) -> String {
        if self.is_nullish() {
            String::new()
        } else {
            self.js_string()
        }
    }

    /// `(v || '').toString()`.
    pub fn or_empty_falsy(&self) -> String {
        if self.truthy() {
            self.js_string()
        } else {
            String::new()
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Cell::Str(s) => Some(s),
            _ => None,
        }
    }

    /// `JSON.stringify(v)` for use as a map key.
    pub fn json_key(&self) -> String {
        match self {
            Cell::Absent => "undefined".into(),
            other => serde_json::to_string(&other.to_json()).unwrap_or_default(),
        }
    }
}

pub fn num_json(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        Value::from(n as i64)
    } else {
        serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// `String(value)` for an arbitrary JSON value.
pub fn json_to_js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => js::num_to_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        Value::Array(a) => a
            .iter()
            .map(|x| {
                if x.is_null() {
                    String::new()
                } else {
                    json_to_js_string(x)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// True for a canonical array-index property name ("0", "17", not "01").
pub fn is_index_key(k: &str) -> bool {
    if k.is_empty() || k.len() > 10 || !k.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    if k.len() > 1 && k.starts_with('0') {
        return false;
    }
    k.parse::<u64>().map(|v| v < 4_294_967_295).unwrap_or(false)
}

/// Orders property names the way JavaScript enumerates object keys.
pub fn js_key_order<S: AsRef<str>>(keys: &[S]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..keys.len()).filter(|&i| is_index_key(keys[i].as_ref())).collect();
    idx.sort_by_key(|&i| keys[i].as_ref().parse::<u64>().unwrap_or(0));
    idx.extend((0..keys.len()).filter(|&i| !is_index_key(keys[i].as_ref())));
    idx
}

#[derive(Clone, Debug, Default)]
pub struct Table {
    cols: Vec<String>,
    index: HashMap<String, usize>,
    pub rows: Vec<Vec<Cell>>,
}

static ABSENT: Cell = Cell::Absent;

impl Table {
    pub fn new() -> Table {
        Table::default()
    }

    pub fn with_columns<S: AsRef<str>>(cols: &[S]) -> Table {
        let mut t = Table::new();
        for c in cols {
            t.col_index_or_add(c.as_ref());
        }
        t
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn col(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    pub fn col_index_or_add(&mut self, name: &str) -> usize {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.cols.len();
        self.cols.push(name.to_string());
        self.index.insert(name.to_string(), i);
        for r in &mut self.rows {
            r.push(Cell::Absent);
        }
        i
    }

    pub fn columns_raw(&self) -> &[String] {
        &self.cols
    }

    pub fn get(&self, row: usize, name: &str) -> &Cell {
        match self.index.get(name) {
            Some(&c) => self.rows[row].get(c).unwrap_or(&ABSENT),
            None => &ABSENT,
        }
    }

    pub fn get_idx(&self, row: usize, col: Option<usize>) -> &Cell {
        match col {
            Some(c) => self.rows[row].get(c).unwrap_or(&ABSENT),
            None => &ABSENT,
        }
    }

    pub fn set(&mut self, row: usize, name: &str, v: Cell) {
        let c = self.col_index_or_add(name);
        self.rows[row][c] = v;
    }

    pub fn push_row(&mut self, cells: Vec<Cell>) {
        let mut cells = cells;
        cells.resize(self.cols.len(), Cell::Absent);
        self.rows.push(cells);
    }

    /// `Object.keys(rows[i])`.
    pub fn keys_of(&self, row: usize) -> Vec<String> {
        let present: Vec<usize> = (0..self.cols.len())
            .filter(|&c| !matches!(self.rows[row][c], Cell::Absent))
            .collect();
        let names: Vec<&str> = present.iter().map(|&c| self.cols[c].as_str()).collect();
        js_key_order(&names).into_iter().map(|i| names[i].to_string()).collect()
    }

    /// `Object.keys(data[0] || {})`.
    pub fn keys(&self) -> Vec<String> {
        if self.rows.is_empty() {
            Vec::new()
        } else {
            self.keys_of(0)
        }
    }

    /// Values of one column (`rows.map(r => r[name])`).
    pub fn column(&self, name: &str) -> Vec<&Cell> {
        let c = self.col(name);
        (0..self.rows.len()).map(|r| self.get_idx(r, c)).collect()
    }

    /// Rows `start..end` as a new table.
    pub fn slice(&self, start: usize, end: usize) -> Table {
        let end = end.min(self.rows.len());
        let start = start.min(end);
        Table {
            cols: self.cols.clone(),
            index: self.index.clone(),
            rows: self.rows[start..end].to_vec(),
        }
    }

    pub fn select_rows(&self, idx: &[usize]) -> Table {
        Table {
            cols: self.cols.clone(),
            index: self.index.clone(),
            rows: idx.iter().map(|&i| self.rows[i].clone()).collect(),
        }
    }

    /// Copy of the rows with only `attrs` (each attribute set even when absent: `row[a]`).
    pub fn project(&self, attrs: &[String], fill: impl Fn(&Cell) -> Cell) -> Table {
        let mut t = Table::with_columns(attrs);
        let cols: Vec<Option<usize>> = attrs.iter().map(|a| self.col(a)).collect();
        for r in 0..self.rows.len() {
            let mut cells: Vec<Cell> = vec![Cell::Absent; t.cols.len()];
            for (a, c) in attrs.iter().zip(&cols) {
                let ti = t.index[a];
                cells[ti] = fill(self.get_idx(r, *c));
            }
            t.rows.push(cells);
        }
        t
    }

    pub fn row_json(&self, row: usize) -> serde_json::Map<String, Value> {
        let mut m = serde_json::Map::new();
        for k in self.keys_of(row) {
            m.insert(k.clone(), self.get(row, &k).to_json());
        }
        m
    }

    pub fn to_json(&self) -> Value {
        Value::Array((0..self.rows.len()).map(|r| Value::Object(self.row_json(r))).collect())
    }

    pub fn from_json_rows(rows: Vec<Value>) -> Table {
        let mut t = Table::new();
        for row in rows {
            let mut cells = vec![Cell::Absent; t.cols.len()];
            if let Value::Object(m) = row {
                for (k, v) in m {
                    let c = t.col_index_or_add(&k);
                    if cells.len() <= c {
                        cells.resize(c + 1, Cell::Absent);
                    }
                    cells[c] = Cell::from_json(v);
                }
            }
            cells.resize(t.cols.len(), Cell::Absent);
            t.rows.push(cells);
        }
        let width = t.cols.len();
        for r in &mut t.rows {
            r.resize(width, Cell::Absent);
        }
        t
    }
}

impl Table {
    /// Column indices of a complete row in JavaScript key order (shared by every complete row).
    pub fn full_order(&self) -> Vec<usize> {
        js_key_order(&self.cols)
    }

    /// Column indices present on `row`, in JavaScript key order.
    pub fn order_of(&self, row: usize, full: &[usize]) -> Vec<usize> {
        if self.rows[row].iter().all(|c| !matches!(c, Cell::Absent)) {
            full.to_vec()
        } else {
            full.iter()
                .copied()
                .filter(|&c| !matches!(self.rows[row][c], Cell::Absent))
                .collect()
        }
    }
}

struct CellRef<'a>(&'a Cell);

impl Serialize for CellRef<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Cell::Absent | Cell::Null => s.serialize_unit(),
            Cell::Bool(b) => s.serialize_bool(*b),
            Cell::Num(n) => num_json(*n).serialize(s),
            Cell::Str(v) => s.serialize_str(v),
            Cell::Json(v) => v.serialize(s),
        }
    }
}

impl Serialize for Table {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let full = self.full_order();
        let mut seq = s.serialize_seq(Some(self.rows.len()))?;
        for r in 0..self.rows.len() {
            seq.serialize_element(&RowRef(self, r, &full))?;
        }
        seq.end()
    }
}

struct RowRef<'a>(&'a Table, usize, &'a [usize]);

impl Serialize for RowRef<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let row = &self.0.rows[self.1];
        let complete = row.iter().all(|c| !matches!(c, Cell::Absent));
        let n = if complete {
            self.2.len()
        } else {
            row.iter().filter(|c| !matches!(c, Cell::Absent)).count()
        };
        let mut m = s.serialize_map(Some(n))?;
        for &c in self.2 {
            if complete || !matches!(row[c], Cell::Absent) {
                m.serialize_entry(&self.0.cols[c], &CellRef(&row[c]))?;
            }
        }
        m.end()
    }
}

impl<'de> Deserialize<'de> for Table {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Table, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Table;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an array of row objects")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Table, A::Error> {
                let mut rows = Vec::new();
                while let Some(v) = seq.next_element::<Value>()? {
                    rows.push(v);
                }
                Ok(Table::from_json_rows(rows))
            }
            fn visit_unit<E>(self) -> Result<Table, E> {
                Ok(Table::new())
            }
            fn visit_none<E>(self) -> Result<Table, E> {
                Ok(Table::new())
            }
        }
        d.deserialize_any(V)
    }
}
