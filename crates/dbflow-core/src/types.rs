//! Data model shared by every stage (mirrors the TypeScript interfaces the UI consumes).

use crate::table::Table;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Right-hand side of a dependency: one attribute, a comma list, or an array.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Rhs {
    One(String),
    Many(Vec<String>),
}

impl Default for Rhs {
    fn default() -> Self {
        Rhs::One(String::new())
    }
}

impl Rhs {
    /// The attribute list (`"a, b"` is two attributes).
    pub fn list(&self) -> Vec<String> {
        match self {
            Rhs::Many(v) => v.clone(),
            Rhs::One(s) if s.contains(',') => s
                .split(',')
                .map(|x| crate::js::trim(x).to_string())
                .filter(|x| !x.is_empty())
                .collect(),
            Rhs::One(s) => vec![s.clone()],
        }
    }

    /// Display text (`${fd.rhs}`).
    pub fn text(&self) -> String {
        match self {
            Rhs::One(s) => s.clone(),
            Rhs::Many(v) => v.join(","),
        }
    }

    pub fn single(&self) -> String {
        match self {
            Rhs::One(s) => s.clone(),
            Rhs::Many(v) => v.join(","),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fd {
    #[serde(default)]
    pub lhs: Vec<String>,
    #[serde(default)]
    pub rhs: Rhs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub violations: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique_groups: Option<f64>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Fd {
    pub fn new(lhs: Vec<String>, rhs: String, confidence: f64, coverage: f64, violations: f64, groups: f64) -> Fd {
        Fd {
            lhs,
            rhs: Rhs::One(rhs),
            confidence: Some(confidence),
            coverage: Some(coverage),
            violations: Some(violations),
            unique_groups: Some(groups),
            extra: Map::new(),
        }
    }
    pub fn conf(&self) -> f64 {
        self.confidence.unwrap_or(f64::NAN)
    }
    pub fn rhs_name(&self) -> String {
        self.rhs.single()
    }
}

/// A dependency with exactly one right-hand attribute (normalized form used by the algorithms).
#[derive(Clone, Debug, PartialEq)]
pub struct SFd {
    pub lhs: Vec<String>,
    pub rhs: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKey {
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub references: String,
    #[serde(default)]
    pub referenced_key: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality: Option<String>,
}

impl ForeignKey {
    pub fn card(&self) -> &str {
        self.cardinality.as_deref().unwrap_or("")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(default)]
    pub row_count: f64,
    #[serde(default)]
    pub attribute_types: IndexMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct OriginalFd {
    pub lhs: Vec<String>,
    pub rhs: String,
    pub confidence: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Relation {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub primary_key: Vec<String>,
    #[serde(default)]
    pub foreign_keys: Vec<ForeignKey>,
    #[serde(default)]
    pub rows: Table,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(default)]
    pub metadata: Metadata,
    #[serde(default, rename = "originalFD", skip_serializing_if = "Option::is_none")]
    pub original_fd: Option<OriginalFd>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_table: Option<String>,
    /// Rows are not serialized (output files strip them to keep JSON small).
    #[serde(skip)]
    pub omit_rows: bool,
    /// Set when the rows were released to save memory (large multi-file runs): how many there were.
    #[serde(skip)]
    pub released_rows: Option<usize>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Relation {
    /// Number of rows, also after they were released.
    pub fn row_count(&self) -> usize {
        self.released_rows.unwrap_or(self.rows.len())
    }

    /// A copy without the rows (which can be large), remembering how many there were.
    pub fn shadow(&self) -> Relation {
        Relation {
            name: self.name.clone(),
            attributes: self.attributes.clone(),
            primary_key: self.primary_key.clone(),
            foreign_keys: self.foreign_keys.clone(),
            rows: Default::default(),
            source_type: self.source_type.clone(),
            metadata: self.metadata.clone(),
            original_fd: self.original_fd.clone(),
            display_name: self.display_name.clone(),
            name_source: self.name_source.clone(),
            source_table: self.source_table.clone(),
            omit_rows: self.omit_rows,
            released_rows: Some(self.row_count()),
            extra: self.extra.clone(),
        }
    }

    /// Drop the rows but remember how many there were.
    pub fn release_rows(&mut self) {
        if self.released_rows.is_none() {
            self.released_rows = Some(self.rows.len());
        }
        self.rows = Default::default();
    }

    pub fn source(&self) -> &str {
        self.source_type.as_deref().unwrap_or("undefined")
    }

    /// JSON without the rows (`{ ...r, rows: undefined }`).
    pub fn to_json_without_rows(&self) -> Value {
        serde_json::to_value(self.without_rows()).unwrap_or(Value::Null)
    }

    /// A serializable view of the relation without its rows.
    pub fn without_rows(&self) -> RelationNoRows<'_> {
        RelationNoRows {
            name: &self.name,
            attributes: &self.attributes,
            primary_key: &self.primary_key,
            foreign_keys: &self.foreign_keys,
            source_type: &self.source_type,
            metadata: &self.metadata,
            original_fd: &self.original_fd,
            display_name: &self.display_name,
            name_source: &self.name_source,
            source_table: &self.source_table,
            extra: &self.extra,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationNoRows<'a> {
    pub name: &'a String,
    pub attributes: &'a Vec<String>,
    pub primary_key: &'a Vec<String>,
    pub foreign_keys: &'a Vec<ForeignKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_type: &'a Option<String>,
    pub metadata: &'a Metadata,
    #[serde(rename = "originalFD", skip_serializing_if = "Option::is_none")]
    pub original_fd: &'a Option<OriginalFd>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_source: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_table: &'a Option<String>,
    #[serde(flatten)]
    pub extra: &'a Map<String, Value>,
}

/// Multi-file foreign key between two input tables.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CrossLink {
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_columns: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_columns: Option<Vec<String>>,
    pub cardinality: String,
    pub confidence: f64,
    pub match_ratio: f64,
    pub detected_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_score: Option<f64>,
}

impl CrossLink {
    pub fn src_cols(&self) -> Vec<String> {
        self.source_columns
            .clone()
            .unwrap_or_else(|| vec![self.source_column.clone()])
    }
    pub fn tgt_cols(&self) -> Vec<String> {
        self.target_columns
            .clone()
            .unwrap_or_else(|| vec![self.target_column.clone()])
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityTag {
    pub column_name: String,
    pub table_name: String,
    pub entity_type: String,
    pub confidence: f64,
    pub sample_values: Vec<String>,
    pub description: String,
}

/// `a.length === b.length && a.every(x => b.includes(x))`.
pub fn same_set(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().all(|x| b.contains(x))
}

/// `Array.from(new Set(items))`.
pub fn dedup(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = indexmap::IndexSet::new();
    for i in items {
        seen.insert(i);
    }
    seen.into_iter().collect()
}

pub fn sorted(mut v: Vec<String>) -> Vec<String> {
    crate::js::sort_strings(&mut v);
    v
}
