//! JSON dispatcher: one entry point (`call(method, args)`) for every engine function, used by
//! the WebAssembly module and the Tauri commands alike.

use crate::ddl::{self, DecomposedRelation, Dialect};
use crate::fd_miner::{self, MineOptions, MinedFd, NullSemantics};
use crate::identifiers;
use crate::naming;
use crate::normalization as nf;
use crate::schema::{self, BuiltSchema, Junction, TableSchemaInput, Unplaced};
use crate::table::{Cell, Table};
use crate::types::{CrossLink, Fd, Relation};
use crate::{csv, data_processor as dp, data_utils as du, erd_dot, js, keys, legacy, relations, sql_types};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

struct Args(Vec<Value>);

impl Args {
    fn raw(&self, i: usize) -> Value {
        self.0.get(i).cloned().unwrap_or(Value::Null)
    }
    fn get<T: DeserializeOwned>(&self, i: usize) -> Result<T, String> {
        serde_json::from_value(self.raw(i)).map_err(|e| format!("argument {i}: {e}"))
    }
    fn opt<T: DeserializeOwned>(&self, i: usize) -> Option<T> {
        match self.raw(i) {
            Value::Null => None,
            v => serde_json::from_value(v).ok(),
        }
    }
    fn str(&self, i: usize) -> String {
        match self.raw(i) {
            Value::String(s) => s,
            Value::Null => String::new(),
            v => crate::table::json_to_js_string(&v),
        }
    }
    fn table(&self, i: usize) -> Table {
        match self.raw(i) {
            Value::Array(rows) => Table::from_json_rows(rows),
            _ => Table::new(),
        }
    }
    fn cells(&self, i: usize) -> Vec<Cell> {
        match self.raw(i) {
            Value::Array(v) => v.into_iter().map(Cell::from_json).collect(),
            _ => vec![],
        }
    }
    fn cell(&self, i: usize) -> Cell {
        match self.0.get(i) {
            None => Cell::Absent,
            Some(v) => Cell::from_json(v.clone()),
        }
    }
    fn fds(&self, i: usize) -> Vec<Fd> {
        self.opt(i).unwrap_or_default()
    }
    fn rels(&self, i: usize) -> Vec<Relation> {
        self.opt(i).unwrap_or_default()
    }
    fn strings(&self, i: usize) -> Vec<String> {
        self.opt(i).unwrap_or_default()
    }
    fn dialect(&self, i: usize, default: Dialect) -> Dialect {
        Dialect::parse(&self.str(i)).unwrap_or(default)
    }
}

fn to<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn set_to_vec(s: indexmap::IndexSet<String>) -> Value {
    Value::Array(s.into_iter().map(Value::String).collect())
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct MineOpts {
    max_lhs_size: Option<usize>,
    min_confidence: Option<f64>,
    null_semantics: Option<NullSemantics>,
    near_key_ratio: Option<f64>,
    complete_determinants_only: Option<bool>,
    near_key_min_rows: Option<usize>,
    min_rows_for_non_key_determinants: Option<usize>,
    time_budget_ms: Option<f64>,
    sample_size: Option<usize>,
    max_rounds: Option<usize>,
    seed: Option<u32>,
}

impl MineOpts {
    fn into_options(self) -> MineOptions<'static> {
        MineOptions {
            max_lhs_size: self.max_lhs_size,
            min_confidence: self.min_confidence,
            null_semantics: self.null_semantics.unwrap_or_default(),
            near_key_ratio: self.near_key_ratio,
            complete_determinants_only: self.complete_determinants_only.unwrap_or(false),
            near_key_min_rows: self.near_key_min_rows,
            min_rows_for_non_key_determinants: self.min_rows_for_non_key_determinants,
            aborted: None,
            time_budget_ms: self.time_budget_ms,
            sample_size: self.sample_size,
            max_rounds: self.max_rounds,
            seed: self.seed,
            full_distinct_ratio: None,
            full_key_check: None,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SchemaTableIn {
    table_name: String,
    #[serde(default)]
    headers: Vec<String>,
    #[serde(default)]
    rows: Table,
    #[serde(default)]
    primary_key: Vec<String>,
    #[serde(default)]
    relations: Vec<Relation>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct SchemaIn {
    relations: Vec<Relation>,
    optional_fks: Vec<String>,
    referenced_unique: Vec<String>,
    unplaced: Vec<Value>,
    junctions: Vec<Value>,
}

fn schema_from(v: Value) -> BuiltSchema {
    let s: SchemaIn = serde_json::from_value(v).unwrap_or_default();
    BuiltSchema {
        junctions: if s.junctions.is_empty() {
            schema::find_junction_tables(&s.relations)
        } else {
            s.junctions
                .into_iter()
                .filter_map(|j| {
                    Some(Junction {
                        relation: j["relation"].as_str()?.to_string(),
                        between: serde_json::from_value(j["between"].clone()).ok()?,
                    })
                })
                .collect()
        },
        inferred_types: Default::default(),
        relations: s.relations,
        optional_fks: s.optional_fks.into_iter().collect(),
        referenced_unique: s.referenced_unique.into_iter().collect(),
        unplaced: s
            .unplaced
            .into_iter()
            .filter_map(|u| {
                Some(Unplaced {
                    link: serde_json::from_value(u["link"].clone()).ok()?,
                    reason: u["reason"].as_str()?.to_string(),
                })
            })
            .collect(),
    }
}

fn schema_json(s: &BuiltSchema) -> Value {
    let mut opt: Vec<&String> = s.optional_fks.iter().collect();
    opt.sort();
    let mut uq: Vec<&String> = s.referenced_unique.iter().collect();
    uq.sort();
    json!({"relations": s.relations, "optionalFks": opt, "referencedUnique": uq, "unplaced": s.unplaced, "junctions": s.junctions})
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DetIn {
    table_name: String,
    #[serde(default)]
    headers: Vec<String>,
    #[serde(default)]
    rows: Table,
    #[serde(default)]
    primary_key: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NerIn {
    table_name: String,
    #[serde(default)]
    headers: Vec<String>,
    #[serde(default)]
    rows: Table,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NameTableIn {
    table_name: String,
    #[serde(default)]
    primary_key: Vec<String>,
    #[serde(rename = "final", default)]
    final_set: Vec<Relation>,
    #[serde(default)]
    others: Vec<Vec<Relation>>,
}

/// Dispatch one call. `args` is the JSON array of positional arguments.
pub fn call(method: &str, args: Value) -> Result<Value, String> {
    let a = Args(match args {
        Value::Array(v) => v,
        Value::Null => vec![],
        other => vec![other],
    });
    match method {
        // ---- CSV ---------------------------------------------------------
        "csv.detectDelimiter" => to(csv::detect_delimiter(&a.str(0)).to_string()),
        "csv.sanitizeHeaders" => to(csv::sanitize_headers(&a.strings(0))),
        "csv.parseCells" => {
            let d = a.str(1).chars().next().unwrap_or(',');
            let (rows, multiline) = csv::parse_cells(&a.str(0), d);
            to(json!({"rows": rows, "hasMultiline": multiline}))
        }
        "csv.parse" => {
            #[derive(Deserialize, Default)]
            #[serde(rename_all = "camelCase", default)]
            struct O {
                delimiter: Option<String>,
                max_rows: Option<usize>,
                preserve_index_columns: Option<bool>,
                file_name: Option<String>,
            }
            let o: O = a.opt(1).unwrap_or_default();
            let opts = csv::CsvOptions {
                delimiter: o.delimiter.and_then(|d| d.chars().next()),
                max_rows: o.max_rows,
                preserve_index_columns: o.preserve_index_columns,
                file_name: o.file_name,
            };
            to(csv::parse(&a.str(0), &opts)?)
        }
        "csv.parseStreaming" => {
            let max = a.opt::<usize>(1).unwrap_or(usize::MAX);
            let delim = a.opt::<String>(2).and_then(|d| d.chars().next());
            let (rows, d, truncated) = csv::parse_streaming(&a.str(0), max, delim);
            to(json!({"rows": rows, "headers": rows.keys(), "delimiter": d.to_string(), "truncated": truncated}))
        }
        // ---- DataUtils -----------------------------------------------------
        "dataUtils.normalizeName" => to(du::normalize_name(&a.str(0))),
        "dataUtils.isIndexColumn" => {
            let values = a
                .opt::<Vec<Value>>(1)
                .map(|v| v.into_iter().map(Cell::from_json).collect::<Vec<_>>());
            let allow = a.raw(2)["allowDiscardPrimaryKey"].as_bool().unwrap_or(false);
            let refs: Option<Vec<&Cell>> = values.as_ref().map(|v| v.iter().collect());
            let name = match a.raw(0) {
                Value::String(s) => s,
                _ => String::new(),
            };
            to(du::is_index_column(&name, refs.as_deref(), allow))
        }
        "dataUtils.inferDataType" => {
            let c = a.cells(0);
            to(du::infer_data_type(&c.iter().collect::<Vec<_>>()))
        }
        "dataUtils.cleanValue" => to(du::clean_value(&a.cell(0))),
        "dataUtils.isAtomic" => to(du::is_atomic(&a.cell(0))),
        "dataUtils.generateRowHash" => {
            let t = Table::from_json_rows(vec![a.raw(0)]);
            to(du::generate_row_hash(&t, 0, &a.strings(1)))
        }
        "dataUtils.removeDuplicates" => to(du::remove_duplicates(&a.table(0), &a.strings(1))),
        "dataUtils.isValidAttributeName" => to(match a.raw(0) {
            Value::String(s) => du::is_valid_attribute_name(&s),
            _ => false,
        }),
        "dataUtils.suggestValidAttributeName" => to(match a.raw(0) {
            Value::String(s) => du::suggest_valid_attribute_name(&s),
            _ => "attribute".into(),
        }),
        "dataUtils.areValuesEqual" => to(du::are_values_equal(&a.cell(0), &a.cell(1))),
        "dataUtils.calculateNumericStats" => {
            let c = a.cells(0);
            to(du::calculate_numeric_stats(&c.iter().collect::<Vec<_>>()))
        }
        "dataUtils.formatValueForDisplay" => to(du::format_value_for_display(&a.cell(0), a.opt(1).unwrap_or(50))),
        "dataUtils.parseDelimitedValue" => {
            let d = if a.raw(1).is_null() { ",".to_string() } else { a.str(1) };
            to(du::parse_delimited_value(&a.str(0), &d))
        }
        "dataUtils.getUniqueValues" => {
            let mut seen = indexmap::IndexSet::new();
            for c in a.cells(0) {
                seen.insert(du::clean_value(&c));
            }
            to(seen.into_iter().collect::<Vec<_>>())
        }
        "dataUtils.getValueFrequency" => {
            let mut freq: indexmap::IndexMap<String, usize> = indexmap::IndexMap::new();
            for c in a.cells(0) {
                *freq.entry(du::clean_value(&c)).or_insert(0) += 1;
            }
            to(freq.into_iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>())
        }
        "dataUtils.analyzeDataQuality" => {
            let vals = a.cells(0);
            let cleaned: Vec<String> = vals.iter().map(du::clean_value).collect();
            let null_count = cleaned.iter().filter(|v| v.is_empty()).count();
            let non: Vec<&String> = cleaned.iter().filter(|v| !v.is_empty()).collect();
            let mut freq: indexmap::IndexMap<&String, usize> = indexmap::IndexMap::new();
            for v in &non {
                *freq.entry(*v).or_insert(0) += 1;
            }
            let uniq = freq.len();
            let mut best: (Value, usize) = (Value::Null, 0);
            for (v, c) in &freq {
                if *c > best.1 {
                    best = (Value::String((*v).clone()), *c);
                }
            }
            let total = vals.len();
            to(json!({
                "totalCount": total, "nullCount": null_count, "uniqueCount": uniq,
                "completeness": if total > 0 { (total - null_count) as f64 / total as f64 } else { 0.0 },
                "uniqueness": if !non.is_empty() { uniq as f64 / non.len() as f64 } else { 0.0 },
                "mostFrequent": {"value": best.0, "count": best.1}
            }))
        }
        // ---- DataProcessor ---------------------------------------------------
        "dataProcessor.clean" => to(dp::clean(&a.table(0))),
        "dataProcessor.checkAtomicity" => to(dp::check_atomicity(&a.table(0))),
        "dataProcessor.listTokens" => to(dp::list_tokens(&a.str(0))),
        "dataProcessor.profileDataTypes" => to(dp::profile_data_types(&a.table(0))),
        "dataProcessor.progressiveSample" => to(dp::progressive_sample(&a.table(0), a.opt(1).unwrap_or(0))),
        // ---- SQL types / identifiers -----------------------------------------
        "sqlTypes.inferColumnSqlType" => {
            let c = a.cells(0);
            to(sql_types::infer_column_sql_type(&c.iter().collect::<Vec<_>>()))
        }
        "sqlTypes.inferColumnTypes" => to(sql_types::infer_column_types(&a.table(0), &a.strings(1))),
        "identifiers.sanitize" => to(identifiers::sanitize_identifier(&a.str(0))),
        "identifiers.mdEscape" => to(identifiers::md_escape(&a.str(0))),
        // ---- FD mining -------------------------------------------------------
        "fd.mine" => to(fd_miner::mine(
            &a.table(0),
            &a.opt::<MineOpts>(1).unwrap_or_default().into_options(),
        )),
        "fd.mineSampled" => to(fd_miner::mine_sampled(
            &a.table(0),
            &a.opt::<MineOpts>(1).unwrap_or_default().into_options(),
        )),
        "fd.validate" => {
            let fds: Vec<MinedFd> = a.opt(1).unwrap_or_default();
            let ns = a.opt(2).unwrap_or_default();
            let v = fd_miner::validate(&a.table(0), &fds, ns);
            to(v.into_iter()
                .map(|x| json!({"fd": x.fd, "witness": x.witness}))
                .collect::<Vec<_>>())
        }
        "fd.reservoirSampleIndices" => to(fd_miner::reservoir_sample_indices(
            a.opt(0).unwrap_or(0),
            a.opt(1).unwrap_or(0),
            a.opt(2).unwrap_or(42),
        )),
        // ---- Keys, 1NF split, instance verification -------------------------------
        "keys.isUniqueAndComplete" => to(keys::is_unique_and_complete(&a.table(0), &a.strings(1))),
        "keys.chooseKey" => {
            let mut attrs = a.strings(0);
            let mut rows = a.table(1);
            let ck: Vec<Vec<String>> = a.opt(2).unwrap_or_default();
            let name = if a.raw(3).is_null() {
                "table".to_string()
            } else {
                a.str(3)
            };
            let choice = keys::choose_key(&mut attrs, &mut rows, &ck, &name);
            to(json!({"choice": choice, "attributes": attrs, "rows": rows}))
        }
        "atomicSplit.split" => {
            let r = keys::split_multi_valued_columns(&a.table(0), &a.strings(1), &a.str(2));
            to(r.map(|s| json!({"parentRows": s.parent_rows, "parentAttributes": s.parent_attributes, "key": s.key, "children": s.children, "moved": s.moved})))
        }
        "instance.verifyLosslessOnData" => {
            let attrs: Vec<Vec<String>> = a.opt(1).unwrap_or_default();
            to(keys::verify_lossless_on_data(
                &a.table(0),
                &attrs,
                a.opt(2).unwrap_or(250_000),
            ))
        }
        // ---- Normalization engine ---------------------------------------------
        "nf.exactFDs" => to(nf::exact_fds(&a.fds(0))),
        "nf.mineDetailed" => {
            let mut o = a.opt::<MineOpts>(2).unwrap_or_default().into_options();
            o.sample_size = o.sample_size.or(Some(10_000));
            to(nf::mine_fds_detailed(&a.table(0), a.opt(1).unwrap_or(4), o))
        }
        "nf.findKeys" => to(nf::find_keys(&a.strings(0), &a.fds(1))),
        "nf.findCandidateKeys" => to(nf::find_candidate_keys(&a.strings(0), &a.fds(1))),
        "nf.computeClosure" => to(set_to_vec(nf::closure(&a.strings(0), &nf::singletons(&a.fds(1))))),
        "nf.extractCandidateKeys" => to(nf::extract_candidate_keys(
            &a.opt::<Vec<Vec<String>>>(0).unwrap_or_default(),
        )),
        "nf.computeCanonicalCover" => to(nf::canonical_cover(&a.fds(0))),
        "nf.classifyPrimeAttributes" => {
            let (p, n, _) = nf::classify_prime(&a.strings(0), &a.opt::<Vec<Vec<String>>>(1).unwrap_or_default());
            to(json!({"primeAttributes": p, "nonPrimeAttributes": n}))
        }
        "nf.is1NF" => {
            let attrs = a.opt::<Vec<String>>(1);
            to(nf::is_1nf(&a.table(0), attrs.as_deref()))
        }
        "nf.is2NF" | "nf.is3NF" | "nf.isBCNF" => {
            let keys = a.opt::<Vec<Vec<String>>>(2);
            let (attrs, fds) = (a.strings(0), a.fds(1));
            Ok(match method {
                "nf.is2NF" => nf::is_2nf(&attrs, &fds, keys.as_deref()),
                "nf.is3NF" => nf::is_3nf(&attrs, &fds, keys.as_deref()),
                _ => nf::is_bcnf(&attrs, &fds, keys.as_deref()),
            })
        }
        "nf.determineNormalForm" => {
            let keys = a.opt::<Vec<Vec<String>>>(3);
            Ok(nf::determine_normal_form(
                &a.strings(0),
                &a.fds(1),
                &a.table(2),
                keys.as_deref(),
            ))
        }
        "nf.apply1NF" => {
            let o = a.raw(1);
            let cols: Vec<String> = serde_json::from_value(o["multiValuedColumns"].clone()).unwrap_or_default();
            let delim = o["delimiter"].as_str().map(String::from);
            let unnest = if o["mode"].as_str() == Some("unnest_repeating_groups") {
                Some((cols.as_slice(), delim.as_deref()))
            } else {
                None
            };
            to(nf::apply_1nf(&a.table(0), unnest))
        }
        "nf.hasAtomicViolations" => to(nf::has_atomic_violations(&a.str(0))),
        "nf.apply2NF" => to(nf::apply_2nf(&a.table(0), &a.fds(1))),
        "nf.apply3NF" => to(nf::apply_3nf(&a.rels(0), &a.fds(1))),
        "nf.synthesize3NF" => {
            let attrs = a.opt::<Vec<String>>(3);
            to(nf::synthesize_3nf(
                &a.fds(0),
                &a.opt::<Vec<Vec<String>>>(1).unwrap_or_default(),
                &a.table(2),
                attrs.as_deref(),
            ))
        }
        "nf.decomposeBCNFDetailed" => {
            let input = a.raw(0);
            let (attrs, rows, name) = if input.is_array() {
                (a.strings(0), a.table(2), "Rel_BCNF".to_string())
            } else {
                let r: Relation = serde_json::from_value(input).unwrap_or_default();
                let rows = if r.rows.is_empty() { a.table(2) } else { r.rows };
                (r.attributes, rows, r.name)
            };
            let res = nf::decompose_bcnf_detailed(&attrs, &rows, &name, &a.fds(1));
            to(json!({
                "relations": res.relations,
                "isLossless": res.is_lossless,
                "isDependencyPreserving": res.preservation.is_preserved,
                "preservationDetails": res.preservation,
                "decompositionSteps": res.steps,
            }))
        }
        "nf.applyBCNF" => to(nf::apply_bcnf(&a.rels(0), &a.fds(1))),
        "nf.chaseAlgorithm" => to(nf::chase(&a.rels(0), &a.fds(1))),
        "nf.computeProjectedClosure" => to(set_to_vec(nf::projected_closure(&a.strings(0), &a.rels(1), &a.fds(2)))),
        "nf.checkDependencyPreservationDetails" => to(nf::dependency_preservation(&a.rels(0), &a.fds(1))),
        "nf.analyzeCardinalities" => to(nf::analyze_cardinalities(&a.table(0))),
        "nf.extractRelationData" => to(nf::extract_relation_data(&a.table(0), &a.strings(1))),
        "nf.ensureCandidateKeyCoverage" => {
            let mut rels = a.rels(0);
            nf::ensure_candidate_key_coverage(
                &mut rels,
                &a.opt::<Vec<Vec<String>>>(1).unwrap_or_default(),
                &a.table(2),
            );
            to(rels)
        }
        "nf.removeRedundantRelationsPreservingLossless" => to(nf::remove_redundant_preserving_lossless(
            &a.rels(0),
            &a.opt::<Vec<Vec<String>>>(1).unwrap_or_default(),
        )),
        "nf.initializeTableau" => to(nf::initialize_tableau(&a.rels(0), &a.strings(1))),
        "nf.hasAllEqualRow" => to(nf::has_all_equal_row(&a.opt::<Vec<Vec<String>>>(0).unwrap_or_default())),
        "nf.generateRelationSchemas" => to(legacy::generate_relation_schemas(&a.rels(0))),
        "nf.generateERD" => {
            let mut rels = a.rels(0);
            let (content, metadata) = legacy::generate_erd(&mut rels);
            let order: Vec<String> = rels.iter().map(|r| r.name.clone()).collect();
            to(json!({"content": content, "metadata": metadata, "order": order}))
        }
        "nf.generateSQLDDL" => to(legacy::generate_sql_ddl(&a.rels(0))),
        // ---- DDL ----------------------------------------------------------------
        "ddl.quoteIdentifier" => to(ddl::quote_identifier(&a.str(0), a.dialect(1, Dialect::Ansi))),
        "ddl.mapDataTypeToDialect" => to(ddl::map_data_type(&a.str(0), a.dialect(1, Dialect::Ansi))),
        "ddl.generateTableDDL" => {
            let r: DecomposedRelation = a.get(0)?;
            to(ddl::generate_table_ddl(&r, a.dialect(1, Dialect::Mysql)))
        }
        "ddl.generateDDL" => {
            let r: Vec<DecomposedRelation> = a.opt(0).unwrap_or_default();
            to(ddl::generate_ddl(&r, a.dialect(1, Dialect::Mysql)))
        }
        "ddl.formatCSVCell" => to(match a.0.first() {
            None => String::new(),
            Some(v) => ddl::format_csv_cell(v),
        }),
        "ddl.formatCSVHeader" => to(ddl::format_csv_header(&match a.raw(0) {
            Value::Null => String::new(),
            v => crate::table::json_to_js_string(&v),
        })),
        "ddl.rowsToCsv" => to(ddl::rows_to_csv(&a.table(0))),
        // ---- Schema graph -------------------------------------------------------
        "schema.build" => {
            let tables: Vec<SchemaTableIn> = a.opt(0).unwrap_or_default();
            let cross: Vec<CrossLink> = a.opt(1).unwrap_or_default();
            let s = schema::build_schema(
                tables
                    .into_iter()
                    .map(|t| TableSchemaInput {
                        table_name: t.table_name,
                        headers: t.headers,
                        rows: t.rows,
                        primary_key: t.primary_key,
                        relations: t.relations,
                    })
                    .collect(),
                &cross,
            );
            Ok(schema_json(&s))
        }
        "schema.emitMermaidErd" => to(schema::emit_mermaid_erd(&schema_from(a.raw(0)))),
        "schema.toDecomposedRelations" => to(schema::to_decomposed_relations(&schema_from(a.raw(0)))),
        "schema.exportNormalizedData" => to(schema::export_normalized_data(
            &schema_from(a.raw(0)),
            a.dialect(1, Dialect::Sqlite),
        )),
        "schema.orphanChecks" => {
            let r: Vec<DecomposedRelation> = a.opt(0).unwrap_or_default();
            to(schema::orphan_checks(&r, a.dialect(1, Dialect::Sqlite)))
        }
        "schema.findJunctionTables" => to(schema::find_junction_tables(&a.rels(0))),
        "schema.csvCell" => to(schema::csv_cell(&a.cell(0))),
        // ---- Relations / classification ------------------------------------------
        "fk.detect" => {
            let t: Vec<DetIn> = a.opt(0).unwrap_or_default();
            let o = a.raw(1);
            let det: Vec<relations::DetectorTable> = t
                .iter()
                .map(|x| relations::DetectorTable {
                    table_name: x.table_name.clone(),
                    headers: x.headers.clone(),
                    rows: &x.rows,
                    primary_key: x.primary_key.clone(),
                })
                .collect();
            to(relations::detect_foreign_keys(
                &det,
                o["minContainment"].as_f64().unwrap_or(0.95),
                o["minScore"].as_f64().unwrap_or(0.7),
            ))
        }
        "fk.tokenizeName" => to(relations::tokenize_name(&a.str(0))),
        "fk.singularize" => to(relations::singularize(&a.str(0))),
        "semantic.tokenizeName" => to(relations::profiler_tokenize(&a.str(0))),
        "semantic.singularize" => to(relations::profiler_singularize(&a.str(0))),
        "semantic.looksLikeDate" => to(relations::looks_like_date(&a.str(0))),
        "semantic.classifyColumn" => {
            let i = a.raw(0);
            let stats = relations::ColumnStats {
                samples: serde_json::from_value(i["stats"]["samples"].clone()).unwrap_or_default(),
                filled: i["stats"]["filled"].as_u64().unwrap_or(0) as usize,
                distinct: i["stats"]["distinct"].as_u64().unwrap_or(0) as usize,
            };
            to(relations::classify_column(
                i["tableName"].as_str().unwrap_or(""),
                i["columnName"].as_str().unwrap_or(""),
                i["columnIndex"].as_i64().unwrap_or(-1),
                &stats,
            ))
        }
        "semantic.performNER" => {
            let t: Vec<NerIn> = a.opt(0).unwrap_or_default();
            let mut out = serde_json::Map::new();
            for x in t {
                out.insert(
                    x.table_name.clone(),
                    to(relations::classify_table(&x.table_name, &x.headers, &x.rows))?,
                );
            }
            Ok(Value::Object(out))
        }
        "semantic.classifyColumnEntity" => to(relations::classify_entity(&a.str(0), &a.str(1), &a.cells(2))),
        // ---- Naming -------------------------------------------------------------
        "naming.relationSignature" => {
            let r = a.raw(1);
            let attrs: Vec<String> = serde_json::from_value(r["attributes"].clone()).unwrap_or_default();
            let pk: Vec<String> = serde_json::from_value(r["primaryKey"].clone()).unwrap_or_default();
            to(naming::relation_signature(&a.str(0), &attrs, &pk))
        }
        "naming.cleanModelName" => to(naming::clean_model_name(&a.str(0), &a.strings(1))),
        "naming.mainOf" => to(naming::main_of(&a.rels(0), &a.strings(1))),
        "naming.nameRelationSet" => {
            let mut set = a.rels(0);
            let o = a.raw(1);
            let pk: Vec<String> = serde_json::from_value(o["primaryKey"].clone()).unwrap_or_default();
            let chosen: HashMap<String, Value> = serde_json::from_value(o["chosen"].clone()).unwrap_or_default();
            let chosen: HashMap<String, naming::ChosenName> = chosen
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        naming::ChosenName {
                            text: v["text"].as_str().unwrap_or("").into(),
                            source: v["source"].as_str().unwrap_or("").into(),
                        },
                    )
                })
                .collect();
            let mut reg = identifiers::IdentifierRegistry::new();
            naming::name_relation_set(
                &mut set,
                o["tableName"].as_str().unwrap_or(""),
                &pk,
                Some(&chosen),
                &mut reg,
            );
            to(set)
        }
        "naming.plan" => {
            let t: Vec<NameTableIn> = a.opt(0).unwrap_or_default();
            let user: HashMap<String, String> = a.opt(1).unwrap_or_default();
            let namable: Vec<naming::NamableTable> = t
                .iter()
                .map(|x| naming::NamableTable {
                    table_name: x.table_name.clone(),
                    primary_key: x.primary_key.clone(),
                    final_set: Some(&x.final_set),
                    others: x.others.iter().map(|o| o.as_slice()).collect(),
                })
                .collect();
            let (chosen, ask) = naming::plan_names(&namable, &user);
            let chosen: serde_json::Map<String, Value> = chosen
                .into_iter()
                .map(|(k, c)| (k, json!({"text": c.text, "source": c.source})))
                .collect();
            to(
                json!({"chosen": chosen, "toAsk": ask.into_iter().map(|(k, c)| json!({"key": k, "columns": c})).collect::<Vec<_>>()}),
            )
        }
        "naming.apply" => {
            let mut t: Vec<NameTableIn> = a.opt(0).unwrap_or_default();
            let chosen_in: HashMap<String, Value> = a.opt(1).unwrap_or_default();
            let mut chosen: HashMap<String, naming::ChosenName> = chosen_in
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        naming::ChosenName {
                            text: v["text"].as_str().unwrap_or("").into(),
                            source: v["source"].as_str().unwrap_or("").into(),
                        },
                    )
                })
                .collect();
            let ask: Vec<Value> = a.opt(2).unwrap_or_default();
            let ask: indexmap::IndexMap<String, Vec<String>> = ask
                .into_iter()
                .map(|v| {
                    (
                        v["key"].as_str().unwrap_or("").to_string(),
                        serde_json::from_value(v["columns"].clone()).unwrap_or_default(),
                    )
                })
                .collect();
            if let Some(answers) = a.opt::<HashMap<String, String>>(3) {
                naming::accept_model_names(&mut chosen, &ask, &answers);
            }
            let mut reg = identifiers::IdentifierRegistry::new();
            for x in t.iter_mut() {
                naming::name_relation_set(&mut x.final_set, &x.table_name, &x.primary_key, Some(&chosen), &mut reg);
            }
            for x in t.iter_mut() {
                for o in x.others.iter_mut() {
                    naming::name_relation_set(
                        o,
                        &x.table_name,
                        &x.primary_key,
                        Some(&chosen),
                        &mut identifiers::IdentifierRegistry::new(),
                    );
                }
            }
            to(t.into_iter()
                .map(|x| json!({"final": x.final_set, "others": x.others}))
                .collect::<Vec<_>>())
        }
        // ---- ERD (Graphviz) -------------------------------------------------------
        "erd.chenDot" => {
            let o = a.raw(1);
            to(erd_dot::build_chen_dot(
                &a.rels(0),
                o["attributes"].as_bool().unwrap_or(true),
                o["tree"].as_bool().unwrap_or(false),
            ))
        }
        "erd.barkerDot" => to(erd_dot::build_barker_dot(
            &a.rels(0),
            a.raw(1)["attributes"].as_bool().unwrap_or(true),
        )),
        "erd.umlDot" => to(erd_dot::build_uml_dot(
            &a.rels(0),
            a.raw(1)["attributes"].as_bool().unwrap_or(true),
        )),
        "erd.attributeCount" => to(erd_dot::attribute_count(&a.rels(0))),
        "erd.isJunction" => to(erd_dot::is_junction(&a.get::<Relation>(0).unwrap_or_default())),
        "erd.isWeak" => to(erd_dot::is_weak(&a.get::<Relation>(0).unwrap_or_default())),
        "erd.optionalColumns" => {
            let mut v: Vec<String> = erd_dot::optional_columns(&a.get::<Relation>(0).unwrap_or_default())
                .into_iter()
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            v.sort();
            to(v)
        }
        // ---- JS primitives (exposed for tests) ------------------------------------
        "js.localeCompare" => to(js::locale_compare(&a.str(0), &a.str(1)) as i32),
        "js.dateParseValid" => to(js::date_parse_valid(&a.str(0))),
        "spreadsheet.isSpreadsheetName" => to(crate::spreadsheet::is_spreadsheet_name(&a.str(0))),
        "engine.version" => to(env!("CARGO_PKG_VERSION")),
        other => Err(format!("unknown engine method: {other}")),
    }
}
