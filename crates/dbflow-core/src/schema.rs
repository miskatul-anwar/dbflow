//! Multi-table schema assembly, Mermaid ERD, DDL input and normalized-data export.

use crate::ddl::{quote_identifier, DecomposedFk, DecomposedRelation, Dialect};
use crate::identifiers::{sanitize_identifier, IdentifierRegistry};
use crate::js;
use crate::naming::name_relation_set;
use crate::sql_types::infer_column_types;
use crate::table::{Cell, Table};
use crate::types::{same_set, CrossLink, ForeignKey, Metadata, Relation};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

pub struct TableSchemaInput {
    pub table_name: String,
    pub headers: Vec<String>,
    pub rows: Table,
    pub primary_key: Vec<String>,
    pub relations: Vec<Relation>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Junction {
    pub relation: String,
    pub between: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Unplaced {
    pub link: CrossLink,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct BuiltSchema {
    pub relations: Vec<Relation>,
    pub optional_fks: HashSet<String>,
    pub referenced_unique: HashSet<String>,
    pub unplaced: Vec<Unplaced>,
    pub junctions: Vec<Junction>,
}

pub fn build_schema(tables: Vec<TableSchemaInput>, cross: &[CrossLink]) -> BuiltSchema {
    let mut names = IdentifierRegistry::new();
    let mut by_table: Vec<(String, std::ops::Range<usize>)> = Vec::new();
    let mut all: Vec<Relation> = Vec::new();
    for t in tables {
        let mut rels = t.relations;
        if rels.is_empty() {
            rels = vec![Relation {
                name: t.table_name.clone(),
                attributes: t.headers.clone(),
                primary_key: t.primary_key.clone(),
                metadata: Metadata {
                    row_count: t.rows.len() as f64,
                    attribute_types: Default::default(),
                },
                rows: t.rows,
                source_type: Some("Main".into()),
                ..Default::default()
            }];
        }
        let holds: Vec<usize> = (0..rels.len())
            .filter(|&i| t.primary_key.iter().all(|k| rels[i].attributes.contains(k)))
            .collect();
        let mut pool = if holds.is_empty() {
            (0..rels.len()).collect()
        } else {
            holds
        };
        pool.sort_by(|&a, &b| {
            let (ra, rb) = (&rels[a], &rels[b]);
            (same_set(&rb.primary_key, &t.primary_key) as i64 - same_set(&ra.primary_key, &t.primary_key) as i64)
                .cmp(&0)
                .then((rb.attributes.len() as i64 - ra.attributes.len() as i64).cmp(&0))
        });
        let main = pool[0];
        name_relation_set(&mut rels, &t.table_name, &t.primary_key, None, &mut names);
        for r in &mut rels {
            r.source_table = Some(t.table_name.clone());
        }
        let main_name = rels[main].name.clone();
        if !t.primary_key.is_empty()
            && t.primary_key.iter().all(|k| rels[main].attributes.contains(k))
            && !same_set(&rels[main].primary_key, &t.primary_key)
            && !rels
                .iter()
                .enumerate()
                .any(|(i, r)| i != main && r.foreign_keys.iter().any(|fk| fk.references == main_name))
        {
            rels[main].primary_key = t.primary_key.clone();
        }
        let start = all.len();
        all.extend(rels);
        by_table.push((t.table_name.clone(), start..all.len()));
    }
    let range_of = |name: &str| {
        by_table
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r.clone())
            .unwrap_or(0..0)
    };
    let mut optional = HashSet::new();
    let mut ref_unique = HashSet::new();
    let mut unplaced = Vec::new();
    for link in cross {
        if link.match_ratio < 1.0 - 1e-9 {
            unplaced.push(Unplaced {
                link: link.clone(),
                reason: format!(
                    "only {}% of its values are contained in the target, so it is not emitted as a constraint",
                    js::to_fixed(link.match_ratio * 100.0, 1)
                ),
            });
            continue;
        }
        let src_cols = link.src_cols();
        let tgt_cols = link.tgt_cols();
        let label = |t: &str, c: &[String]| format!("{t}.{}", c.join("+"));
        let holding_src: Vec<usize> = range_of(&link.source_table)
            .filter(|&i| src_cols.iter().all(|c| all[i].attributes.contains(c)))
            .collect();
        if holding_src.is_empty() {
            unplaced.push(Unplaced {
                link: link.clone(),
                reason: format!(
                    "column {} is not in any normalized relation",
                    label(&link.source_table, &src_cols)
                ),
            });
            continue;
        }
        let mut hs = holding_src;
        hs.sort_by(|&a, &b| {
            let ka = src_cols.iter().all(|c| all[a].primary_key.contains(c)) as i64;
            let kb = src_cols.iter().all(|c| all[b].primary_key.contains(c)) as i64;
            (ka - kb)
                .cmp(&0)
                .then((all[b].attributes.len() as i64 - all[a].attributes.len() as i64).cmp(&0))
        });
        let src = hs[0];
        let holding_tgt: Vec<usize> = range_of(&link.target_table)
            .filter(|&i| tgt_cols.iter().all(|c| all[i].attributes.contains(c)))
            .collect();
        if holding_tgt.is_empty() {
            unplaced.push(Unplaced {
                link: link.clone(),
                reason: format!(
                    "column {} is not in any normalized relation",
                    label(&link.target_table, &tgt_cols)
                ),
            });
            continue;
        }
        let mut ht = holding_tgt;
        ht.sort_by(|&a, &b| {
            let (ra, rb) = (&all[a], &all[b]);
            (same_set(&rb.primary_key, &tgt_cols) as i64 - same_set(&ra.primary_key, &tgt_cols) as i64)
                .cmp(&0)
                .then(
                    (tgt_cols.iter().all(|c| rb.primary_key.contains(c)) as i64
                        - tgt_cols.iter().all(|c| ra.primary_key.contains(c)) as i64)
                        .cmp(&0),
                )
                .then((ra.attributes.len() as i64 - rb.attributes.len() as i64).cmp(&0))
        });
        let tgt = ht[0];
        let tgt_name = all[tgt].name.clone();
        let tgt_pk = all[tgt].primary_key.clone();
        let dup = all[src]
            .foreign_keys
            .iter()
            .any(|fk| fk.references == tgt_name && same_set(&fk.attributes, &src_cols));
        if !dup {
            all[src].foreign_keys.push(ForeignKey {
                attributes: src_cols.clone(),
                references: tgt_name.clone(),
                referenced_key: tgt_cols.clone(),
                cardinality: Some(link.cardinality.clone()),
            });
        }
        if link.optional == Some(true) {
            for c in &src_cols {
                optional.insert(format!("{}.{c}", all[src].name));
            }
        }
        if !same_set(&tgt_pk, &tgt_cols) {
            for c in &tgt_cols {
                ref_unique.insert(format!("{tgt_name}.{c}"));
            }
        }
    }
    let junctions = find_junction_tables(&all);
    BuiltSchema {
        relations: all,
        optional_fks: optional,
        referenced_unique: ref_unique,
        unplaced,
        junctions,
    }
}

pub fn find_junction_tables(relations: &[Relation]) -> Vec<Junction> {
    let mut out = Vec::new();
    for r in relations {
        if r.primary_key.len() < 2 {
            continue;
        }
        let fks: Vec<&ForeignKey> = r
            .foreign_keys
            .iter()
            .filter(|fk| fk.attributes.iter().all(|a| r.primary_key.contains(a)))
            .collect();
        let covered: HashSet<&String> = fks.iter().flat_map(|fk| fk.attributes.iter()).collect();
        let parents: Vec<String> = crate::types::dedup(fks.iter().map(|fk| fk.references.clone()))
            .into_iter()
            .filter(|p| *p != r.name)
            .collect();
        if parents.len() >= 2 && r.primary_key.iter().all(|a| covered.contains(a)) {
            out.push(Junction {
                relation: r.name.clone(),
                between: parents,
            });
        }
    }
    out
}

fn mermaid_type(sql: &str) -> &'static str {
    let t = sql.to_uppercase();
    if t.starts_with("BIGINT") {
        "bigint"
    } else if t.starts_with("INT") {
        "int"
    } else if t.starts_with("DECIMAL") {
        "decimal"
    } else if t.starts_with("BOOL") {
        "boolean"
    } else if t.starts_with("TIMESTAMP") {
        "timestamp"
    } else if t.starts_with("DATE") {
        "date"
    } else {
        "string"
    }
}

pub fn emit_mermaid_erd(schema: &BuiltSchema) -> String {
    let mut entity_names = IdentifierRegistry::new();
    let mut entity: HashMap<String, String> = HashMap::new();
    for r in &schema.relations {
        let n = entity_names.fresh(&r.name);
        entity.insert(r.name.clone(), n);
    }
    let mut col_regs: HashMap<String, IdentifierRegistry> = HashMap::new();
    let mut col = |rel: &str, name: &str| col_regs.entry(rel.to_string()).or_default().assign(name);
    let mut lines = vec!["erDiagram".to_string()];
    for r in &schema.relations {
        let types = infer_column_types(&r.rows, &r.attributes);
        let fk_cols: HashSet<&String> = r.foreign_keys.iter().flat_map(|fk| fk.attributes.iter()).collect();
        lines.push(format!("    {} {{", entity[&r.name]));
        for a in &r.attributes {
            let mut tags = Vec::new();
            if r.primary_key.contains(a) {
                tags.push("PK");
            }
            if fk_cols.contains(a) {
                tags.push("FK");
            }
            let ty = types.get(a).map(|t| t.sql_type.as_str()).unwrap_or("VARCHAR(255)");
            let tag = if tags.is_empty() {
                String::new()
            } else {
                format!(" {}", tags.join(", "))
            };
            lines.push(format!("        {} {}{tag}", mermaid_type(ty), col(&r.name, a)));
        }
        lines.push("    }".into());
    }
    for r in &schema.relations {
        for fk in &r.foreign_keys {
            let (Some(parent), Some(child)) = (entity.get(&fk.references), entity.get(&r.name)) else {
                continue;
            };
            let opt = fk
                .attributes
                .iter()
                .any(|a| schema.optional_fks.contains(&format!("{}.{a}", r.name)));
            let left = if opt { "|o" } else { "||" };
            let right = if fk.card() == "1:1" { "o|" } else { "o{" };
            let label: Vec<String> = fk.attributes.iter().map(|a| col(&r.name, a)).collect();
            let label = label.join(", ").replace('"', "'");
            lines.push(format!("    {parent} {left}--{right} {child} : \"{label}\""));
        }
    }
    for j in &schema.junctions {
        let names: Vec<String> = j
            .between
            .iter()
            .map(|b| entity.get(b).cloned().unwrap_or_else(|| b.clone()))
            .collect();
        lines.push(format!(
            "    %% {} is a junction table: many-to-many between {}",
            entity.get(&j.relation).cloned().unwrap_or_else(|| j.relation.clone()),
            names.join(" <-> ")
        ));
    }
    format!("{}\n", lines.join("\n"))
}

fn order_parents_first(relations: &[Relation]) -> Vec<usize> {
    let by_name: HashMap<&str, usize> = relations
        .iter()
        .enumerate()
        .map(|(i, r)| (r.name.as_str(), i))
        .collect();
    let mut state: HashMap<String, u8> = HashMap::new();
    let mut out = Vec::new();
    fn visit(
        i: usize,
        rels: &[Relation],
        by: &HashMap<&str, usize>,
        state: &mut HashMap<String, u8>,
        out: &mut Vec<usize>,
    ) {
        if state.get(&rels[i].name).copied().unwrap_or(0) != 0 {
            return;
        }
        state.insert(rels[i].name.clone(), 1);
        for fk in &rels[i].foreign_keys {
            if let Some(&p) = by.get(fk.references.as_str()) {
                if p != i {
                    visit(p, rels, by, state, out);
                }
            }
        }
        state.insert(rels[i].name.clone(), 2);
        out.push(i);
    }
    for i in 0..relations.len() {
        visit(i, relations, &by_name, &mut state, &mut out);
    }
    out
}

pub fn to_decomposed_relations(schema: &BuiltSchema) -> Vec<DecomposedRelation> {
    let mut tnames = IdentifierRegistry::new();
    let mut table_of: HashMap<String, String> = HashMap::new();
    for r in &schema.relations {
        let n = tnames.fresh(&r.name);
        table_of.insert(r.name.clone(), n);
    }
    let mut cols: HashMap<String, IdentifierRegistry> = HashMap::new();
    for r in &schema.relations {
        let mut reg = IdentifierRegistry::new();
        for a in &r.attributes {
            reg.assign(a);
        }
        cols.insert(r.name.clone(), reg);
    }
    let col_of = |rel: &str, a: &str| {
        cols.get(rel)
            .and_then(|r| r.get(a))
            .unwrap_or_else(|| sanitize_identifier(a))
    };
    order_parents_first(&schema.relations)
        .into_iter()
        .map(|i| {
            let r = &schema.relations[i];
            let inferred = infer_column_types(&r.rows, &r.attributes);
            let mut d = DecomposedRelation {
                name: table_of[&r.name].clone(),
                source_name: Some(r.name.clone()),
                ..Default::default()
            };
            for a in &r.attributes {
                let c = col_of(&r.name, a);
                d.column_sources.insert(c.clone(), a.clone());
                d.column_types.insert(c.clone(), inferred[a].sql_type.clone());
                d.not_null.insert(c.clone(), inferred[a].not_null);
                if schema.referenced_unique.contains(&format!("{}.{a}", r.name)) {
                    d.unique.push(c.clone());
                }
            }
            d.attributes = r.attributes.iter().map(|a| col_of(&r.name, a)).collect();
            d.primary_key = r.primary_key.iter().map(|a| col_of(&r.name, a)).collect();
            d.foreign_keys = r
                .foreign_keys
                .iter()
                .map(|fk| DecomposedFk {
                    attributes: fk.attributes.iter().map(|a| col_of(&r.name, a)).collect(),
                    references: table_of
                        .get(&fk.references)
                        .cloned()
                        .unwrap_or_else(|| sanitize_identifier(&fk.references)),
                    referenced_key: fk.referenced_key.iter().map(|a| col_of(&fk.references, a)).collect(),
                    cardinality: fk.cardinality.clone(),
                })
                .collect();
            d
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Normalized data export
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
pub struct ExportedFile {
    pub name: String,
    pub content: String,
    #[serde(rename = "type")]
    pub kind: String,
}

pub fn csv_cell(v: &Cell) -> String {
    if v.is_missing() {
        return String::new();
    }
    let t = v.js_string();
    if re!(r#"[",\r\n]"#).is_match(&t) || re!(r"^\s|\s$").is_match(&t) {
        format!("\"{}\"", t.replace('"', "\"\""))
    } else {
        t
    }
}

fn sql_literal(v: &Cell, sql_type: &str) -> String {
    if v.is_missing() {
        return "NULL".into();
    }
    let text = js::trim(&v.js_string()).to_string();
    let t = sql_type.to_uppercase();
    if re!(r"^(INTEGER|BIGINT|DECIMAL)").is_match(&t) && re!(r"^-?[0-9]+(\.[0-9]+)?$").is_match(&text) {
        return text;
    }
    if t.starts_with("BOOLEAN") {
        return if text.eq_ignore_ascii_case("true") {
            "TRUE"
        } else {
            "FALSE"
        }
        .into();
    }
    format!("'{}'", v.js_string().replace('\'', "''"))
}

pub fn export_normalized_data(schema: &BuiltSchema, dialect: Dialect) -> Vec<ExportedFile> {
    let decomposed = to_decomposed_relations(schema);
    let by_source: HashMap<&str, &Relation> = schema.relations.iter().map(|r| (r.name.as_str(), r)).collect();
    let mut files = Vec::new();
    let mut inserts = vec![
        "-- DBFlow normalized data".to_string(),
        format!("-- Dialect: {}", dialect.name().to_uppercase()),
        "-- Load after the CREATE TABLE statements; tables are ordered so parents come first.".into(),
        String::new(),
    ];
    for rel in &decomposed {
        let Some(source) = by_source.get(rel.source_name.as_deref().unwrap_or(&rel.name)) else {
            continue;
        };
        let originals: Vec<String> = rel
            .attributes
            .iter()
            .map(|a| rel.column_sources.get(a).cloned().unwrap_or_else(|| a.clone()))
            .collect();
        let idx: Vec<Option<usize>> = originals.iter().map(|o| source.rows.col(o)).collect();
        let mut seen = HashSet::new();
        let mut rows: Vec<Vec<Cell>> = Vec::new();
        for r in 0..source.rows.len() {
            let vals: Vec<Cell> = idx.iter().map(|&c| source.rows.get_idx(r, c).clone()).collect();
            let key = serde_json::to_string(&vals.iter().map(|c| c.to_json()).collect::<Vec<_>>()).unwrap_or_default();
            if seen.insert(key) {
                rows.push(vals);
            }
        }
        let header = rel
            .attributes
            .iter()
            .map(|a| csv_cell(&Cell::Str(a.clone())))
            .collect::<Vec<_>>()
            .join(",");
        let mut lines = vec![header];
        lines.extend(
            rows.iter()
                .map(|r| r.iter().map(csv_cell).collect::<Vec<_>>().join(",")),
        );
        files.push(ExportedFile {
            name: format!("data_{}.csv", rel.name),
            kind: "csv".into(),
            content: format!("{}\n", lines.join("\n")),
        });
        if !rows.is_empty() {
            let table = quote_identifier(&rel.name, dialect);
            let cols = rel
                .attributes
                .iter()
                .map(|a| quote_identifier(a, dialect))
                .collect::<Vec<_>>()
                .join(", ");
            inserts.push(format!(
                "-- {}: {} row{}",
                rel.name,
                rows.len(),
                if rows.len() == 1 { "" } else { "s" }
            ));
            for row in &rows {
                let values: Vec<String> = row
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        sql_literal(
                            v,
                            rel.column_types
                                .get(&rel.attributes[i])
                                .map(|s| s.as_str())
                                .unwrap_or("TEXT"),
                        )
                    })
                    .collect();
                inserts.push(format!("INSERT INTO {table} ({cols}) VALUES ({});", values.join(", ")));
            }
            inserts.push(String::new());
        }
    }
    files.push(ExportedFile {
        name: "data_inserts.sql".into(),
        kind: "sql".into(),
        content: format!("{}\n", inserts.join("\n")),
    });
    files.push(ExportedFile {
        name: "verify_foreign_keys.sql".into(),
        kind: "sql".into(),
        content: orphan_checks(&decomposed, dialect),
    });
    files
}

pub fn orphan_checks(relations: &[DecomposedRelation], dialect: Dialect) -> String {
    let q = |s: &str| quote_identifier(s, dialect);
    let mut out = vec![
        "-- Post-load check: every query should return 0.".to_string(),
        String::new(),
    ];
    for r in relations {
        for fk in &r.foreign_keys {
            let on: Vec<String> = fk
                .attributes
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    format!(
                        "p.{} = c.{}",
                        q(fk.referenced_key.get(i).map(|s| s.as_str()).unwrap_or("")),
                        q(a)
                    )
                })
                .collect();
            let nn: Vec<String> = fk
                .attributes
                .iter()
                .map(|a| format!("c.{} IS NOT NULL", q(a)))
                .collect();
            out.push(format!(
                "-- {}({}) -> {}({})",
                r.name,
                fk.attributes.join(", "),
                fk.references,
                fk.referenced_key.join(", ")
            ));
            out.push(format!(
                "SELECT COUNT(*) AS orphans FROM {} c LEFT JOIN {} p ON {} WHERE {} AND p.{} IS NULL;",
                q(&r.name),
                q(&fk.references),
                on.join(" AND "),
                nn.join(" AND "),
                q(fk.referenced_key.first().map(|s| s.as_str()).unwrap_or(""))
            ));
            out.push(String::new());
        }
    }
    format!("{}\n", out.join("\n"))
}
