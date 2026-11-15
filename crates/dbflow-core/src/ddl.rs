//! Dialect-aware DDL for decomposed relations, plus CSV formatting helpers.

use crate::js;
use crate::table::{Cell, Table};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Dialect {
    Postgresql,
    #[default]
    Mysql,
    Sqlite,
    Ansi,
}

impl Dialect {
    pub fn parse(s: &str) -> Option<Dialect> {
        match s {
            "postgresql" => Some(Dialect::Postgresql),
            "mysql" => Some(Dialect::Mysql),
            "sqlite" => Some(Dialect::Sqlite),
            "ansi" => Some(Dialect::Ansi),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Dialect::Postgresql => "postgresql",
            Dialect::Mysql => "mysql",
            Dialect::Sqlite => "sqlite",
            Dialect::Ansi => "ansi",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecomposedFk {
    pub attributes: Vec<String>,
    pub references: String,
    pub referenced_key: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DecomposedRelation {
    pub name: String,
    #[serde(default)]
    pub attributes: Vec<String>,
    #[serde(default)]
    pub primary_key: Vec<String>,
    #[serde(default)]
    pub foreign_keys: Vec<DecomposedFk>,
    #[serde(default)]
    pub column_types: IndexMap<String, String>,
    #[serde(default)]
    pub not_null: IndexMap<String, bool>,
    #[serde(default)]
    pub unique: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    #[serde(default)]
    pub column_sources: IndexMap<String, String>,
}

pub fn quote_identifier(ident: &str, dialect: Dialect) -> String {
    if ident.is_empty() {
        return "\"\"".into();
    }
    let mut s: String = ident
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if s.as_bytes()[0].is_ascii_digit() {
        s.insert(0, '_');
    }
    match dialect {
        Dialect::Mysql => format!("`{}`", s.replace('`', "``")),
        _ => format!("\"{}\"", s.replace('"', "\"\"")),
    }
}

pub fn map_data_type(inferred: &str, dialect: Dialect) -> String {
    let upper = if inferred.is_empty() {
        "VARCHAR(255)".to_string()
    } else {
        inferred.to_uppercase()
    };
    use Dialect::*;
    if upper.contains("INT") {
        if matches!(dialect, Postgresql | Ansi | Sqlite) {
            if upper.contains("BIGINT") {
                return "BIGINT".into();
            }
            if upper.contains("SMALLINT") {
                return "SMALLINT".into();
            }
            return "INTEGER".into();
        }
        return "INT".into();
    }
    if upper.contains("BOOL") {
        return match dialect {
            Sqlite => "INTEGER",
            Mysql => "TINYINT(1)",
            _ => "BOOLEAN",
        }
        .into();
    }
    if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
        return upper;
    }
    if upper.contains("FLOAT") || upper.contains("DOUBLE") || upper.contains("REAL") {
        return match dialect {
            Sqlite => "REAL",
            Postgresql | Ansi => "DOUBLE PRECISION",
            _ => "DOUBLE",
        }
        .into();
    }
    if upper.contains("DATE") && !upper.contains("TIME") {
        return if dialect == Sqlite { "TEXT" } else { "DATE" }.into();
    }
    if upper.contains("TIME") || upper.contains("DATETIME") {
        return match dialect {
            Sqlite => "TEXT",
            Postgresql | Ansi => "TIMESTAMP",
            _ => "DATETIME",
        }
        .into();
    }
    if upper.contains("JSON") {
        return match dialect {
            Postgresql => "JSONB",
            Mysql => "JSON",
            _ => "TEXT",
        }
        .into();
    }
    if upper.contains("TEXT") || upper.contains("LONGTEXT") || upper.contains("CLOB") {
        if dialect == Mysql {
            return if upper.contains("LONG") { "LONGTEXT" } else { "TEXT" }.into();
        }
        return "TEXT".into();
    }
    if upper.starts_with("VARCHAR") {
        return if dialect == Sqlite { "TEXT".into() } else { upper };
    }
    if dialect == Sqlite { "TEXT" } else { "VARCHAR(255)" }.into()
}

pub fn generate_table_ddl(r: &DecomposedRelation, dialect: Dialect) -> String {
    let table = quote_identifier(&r.name, dialect);
    let mut defs = Vec::new();
    let composite = r.primary_key.len() > 1;
    for attr in &r.attributes {
        let col = quote_identifier(attr, dialect);
        let raw = r
            .column_types
            .get(attr)
            .filter(|t| !t.is_empty())
            .map(|s| s.as_str())
            .unwrap_or("VARCHAR(255)");
        let ty = map_data_type(raw, dialect);
        let is_pk = r.primary_key.contains(attr);
        if is_pk && !composite {
            defs.push(format!("  {col} {ty} PRIMARY KEY"));
        } else if is_pk {
            defs.push(format!("  {col} {ty} NOT NULL"));
        } else {
            let nn = if r.not_null.get(attr).copied().unwrap_or(false) {
                " NOT NULL"
            } else {
                ""
            };
            let uq = if r.unique.contains(attr) { " UNIQUE" } else { "" };
            defs.push(format!("  {col} {ty}{nn}{uq}"));
        }
    }
    if composite {
        let cols: Vec<String> = r.primary_key.iter().map(|k| quote_identifier(k, dialect)).collect();
        defs.push(format!("  PRIMARY KEY ({})", cols.join(", ")));
    }
    for fk in &r.foreign_keys {
        let src: Vec<String> = fk.attributes.iter().map(|a| quote_identifier(a, dialect)).collect();
        let tgt: Vec<String> = fk.referenced_key.iter().map(|a| quote_identifier(a, dialect)).collect();
        let cname = js::substring16(&format!("fk_{}_{}", r.name, fk.attributes.join("_")), 64);
        defs.push(format!(
            "  CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE CASCADE ON UPDATE CASCADE",
            quote_identifier(&cname, dialect),
            src.join(", "),
            quote_identifier(&fk.references, dialect),
            tgt.join(", ")
        ));
    }
    let mut sql = format!("CREATE TABLE {table} (\n{}\n)", defs.join(",\n"));
    if dialect == Dialect::Mysql {
        sql += " ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";
    }
    sql += ";\n\n";
    sql
}

pub fn generate_ddl(relations: &[DecomposedRelation], dialect: Dialect) -> String {
    let mut out = String::from("-- DBFlow Relational Schema DDL\n");
    out += &format!("-- Dialect: {}\n", dialect.name().to_uppercase());
    out += &format!("-- Generated: {}\n\n", js::iso_now());
    out += match dialect {
        Dialect::Mysql => {
            "SET FOREIGN_KEY_CHECKS = 0;\nSET UNIQUE_CHECKS = 0;\nSET AUTOCOMMIT = 0;\nSTART TRANSACTION;\n\n"
        }
        Dialect::Postgresql => "BEGIN;\n\n",
        Dialect::Sqlite => "PRAGMA foreign_keys = OFF;\nBEGIN TRANSACTION;\n\n",
        Dialect::Ansi => "",
    };
    for r in relations {
        out += &generate_table_ddl(r, dialect);
    }
    out += match dialect {
        Dialect::Mysql => "COMMIT;\nSET FOREIGN_KEY_CHECKS = 1;\nSET UNIQUE_CHECKS = 1;\nSET AUTOCOMMIT = 1;\n",
        Dialect::Postgresql => "COMMIT;\n",
        Dialect::Sqlite => "COMMIT;\nPRAGMA foreign_keys = ON;\n",
        Dialect::Ansi => "COMMIT;\n",
    };
    out
}

fn needs_quotes(s: &str) -> bool {
    s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r')
}

/// RFC 4180 cell formatting.
pub fn format_csv_cell(v: &Value) -> String {
    let s = match v {
        Value::Null => return String::new(),
        Value::String(s) => s.clone(),
        Value::Array(_) | Value::Object(_) => serde_json::to_string(v).unwrap_or_default(),
        other => crate::table::json_to_js_string(other),
    };
    if needs_quotes(&s) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s
    }
}

pub fn format_csv_header(h: &str) -> String {
    if needs_quotes(h) {
        format!("\"{}\"", h.replace('"', "\"\""))
    } else {
        h.to_string()
    }
}

fn push_csv(out: &mut String, s: &str) {
    if needs_quotes(s) {
        out.push('"');
        out.push_str(&s.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(s);
    }
}

/// Rows as CSV (header from the first row's keys).
pub fn rows_to_csv(rows: &Table) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let headers = rows.keys();
    if headers.is_empty() {
        return String::new();
    }
    let cols: Vec<Option<usize>> = headers.iter().map(|h| rows.col(h)).collect();
    let mut out = String::with_capacity(rows.len() * headers.len() * 8);
    for (i, h) in headers.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_csv(&mut out, h);
    }
    for r in 0..rows.len() {
        out.push('\n');
        for (i, &c) in cols.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            match rows.get_idx(r, c) {
                Cell::Absent | Cell::Null => {}
                Cell::Str(s) => push_csv(&mut out, s),
                Cell::Json(v) if matches!(**v, Value::Array(_) | Value::Object(_)) => {
                    push_csv(&mut out, &serde_json::to_string(v).unwrap_or_default())
                }
                other => push_csv(&mut out, &other.js_string()),
            }
        }
    }
    out
}
