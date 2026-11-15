//! The shared per-table analysis (`analyzeRows`): every selected operation for one table.

use crate::data_processor::{self, AtomicityResult};
use crate::fd_miner::MineOptions;
use crate::js;
use crate::keys::{choose_key, split_multi_valued_columns, verify_lossless_on_data, KeyChoice};
use crate::naming::new_relation_id;
use crate::normalization::{self as nf, KeyResult};
use crate::relations::classify_table;
use crate::table::{Cell, Table};
use crate::types::{EntityTag, Fd, ForeignKey, Metadata, Relation};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    pub max_lhs_size: Option<f64>,
    pub max_sample_size: Option<f64>,
    pub fd_sample_size: Option<f64>,
    pub near_key_ratio: Option<f64>,
    pub complete_determinants_only: Option<bool>,
    pub min_rows_for_non_key_determinants: Option<f64>,
    pub timeout: Option<f64>,
    pub enable_parallel_processing: Option<bool>,
    pub adaptive_thresholds: Option<bool>,
    #[serde(rename = "acceptApproximateFDs")]
    pub accept_approximate_fds: Option<bool>,
    pub sql_dialect: Option<String>,
    pub user_names: Option<std::collections::HashMap<String, String>>,
    /// Notes from converting inputs before the run (e.g. skipped spreadsheet sheets).
    pub input_notes: Option<Vec<String>>,
    /// Keep the rows of every normal-form set in large multi-file runs (default: only the final set's).
    pub keep_intermediate_rows: Option<bool>,
    /// Files analysed at once in a multi-file run (native build; default: one per CPU core).
    pub max_parallel_files: Option<f64>,
    /// Memory the parallel multi-file analysis may plan for, in MB (default: half of what is available).
    pub memory_budget_mb: Option<f64>,
    /// `"model"` (default): the on-device model suggests relation names; `"rules"`: names are built
    /// from the key columns (fast, no model).
    pub naming_mode: Option<String>,
    /// Merge files that have the same columns and mostly the same rows into one table (default:
    /// analyse them separately and report the overlap).
    pub merge_overlapping_files: Option<bool>,
    /// Files the interface could not convert (reported with the result, after the engine's own checks).
    pub skipped_inputs: Option<Vec<crate::input_check::Skipped>>,
    /// Minutes east of UTC, for log time stamps.
    pub tz_offset_minutes: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: String,
    pub operation: String,
    pub progress: f64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_time_remaining: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_usage: Option<f64>,
}

pub struct Ctx<'a> {
    pub progress: &'a dyn Fn(Progress),
    pub cancelled: &'a dyn Fn() -> bool,
    pub start: f64,
    pub memory_mb: f64,
}

impl Ctx<'_> {
    pub fn check(&self) -> Result<(), String> {
        if (self.cancelled)() {
            Err("Processing was cancelled.".into())
        } else {
            Ok(())
        }
    }
    pub fn eta(&self, fraction: f64) -> f64 {
        let elapsed = js::now_ms() - self.start;
        (elapsed / fraction - elapsed).max(0.0)
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FdSearch {
    pub truncated: bool,
    pub level_reached: usize,
    pub max_lhs_size: usize,
    pub exact: usize,
    pub approximate: usize,
    pub approximate_used: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TableAnalysis {
    pub rows: Table,
    pub attributes: Vec<String>,
    pub atomicity_result: Option<AtomicityResult>,
    pub data_profile: IndexMap<String, String>,
    pub ner_tags: Vec<EntityTag>,
    pub functional_dependencies: Vec<Fd>,
    pub used_fds: Vec<Fd>,
    pub fd_search: Option<FdSearch>,
    pub key_result: KeyResult,
    pub key_choice: Option<KeyChoice>,
    pub relations_1nf: Vec<Relation>,
    pub relations_2nf: Vec<Relation>,
    pub relations_3nf: Vec<Relation>,
    pub relations_bcnf: Vec<Relation>,
    /// BCNF ran and its result is the final relation set (`relations3NF === relationsBCNF`).
    pub bcnf_is_final: bool,
    pub cardinality: IndexMap<String, Value>,
    pub lossless_check: Value,
    pub dependency_preservation: Value,
    pub notes: Vec<String>,
    /// Large input: some relation sets hold no rows (see `Relation::released_rows`) and the result
    /// reports row counts instead of rows.
    pub compact: bool,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub log: Vec<(&'static str, String)>,
}

impl TableAnalysis {
    pub fn atomicity_json(&self) -> Value {
        match &self.atomicity_result {
            Some(a) => serde_json::to_value(a).unwrap_or(Value::Null),
            None => json!({}),
        }
    }
    pub fn final_set(&self) -> &[Relation] {
        if !self.relations_3nf.is_empty() {
            &self.relations_3nf
        } else {
            &self.relations_2nf
        }
    }
}

/// `suggestPrimaryKey`: first column unique on a 1000-row sample.
pub fn suggest_primary_key(data: &Table, attributes: &[String]) -> Vec<String> {
    if data.is_empty() || attributes.is_empty() {
        return attributes.iter().take(1).cloned().collect();
    }
    let n = data.len().min(1000);
    for a in attributes {
        let c = data.col(a);
        let set: HashSet<String> = (0..n)
            .map(|r| {
                let v = data.get_idx(r, c);
                format!(
                    "{}{}",
                    std::mem::discriminant(v) == std::mem::discriminant(&Cell::str("")),
                    v.json_key()
                )
            })
            .collect();
        if set.len() == n {
            return vec![a.clone()];
        }
    }
    attributes.iter().take(1).cloned().collect()
}

fn optimized_type(values: &[&Cell]) -> &'static str {
    if values.is_empty() {
        return "string";
    }
    let sample = &values[..values.len().min(10)];
    let numeric = sample.iter().all(|v| match v {
        Cell::Num(n) => !n.is_nan(),
        Cell::Bool(_) => true,
        other => {
            let s = other.js_string();
            !js::is_nan_str(&s) && !js::parse_float(&s).is_nan()
        }
    });
    if numeric {
        return if sample.iter().any(|v| v.js_string().contains('.')) {
            "decimal"
        } else {
            "integer"
        };
    }
    if sample
        .iter()
        .all(|v| crate::re!(r"(?i)^(true|false|yes|no|y|n|0|1)$").is_match(&v.js_string()))
    {
        return "boolean";
    }
    "string"
}

/// `inferAttributeTypesOptimized`.
pub fn optimized_types(data: &Table, attributes: &[String]) -> IndexMap<String, String> {
    let n = data.len().min(50);
    attributes
        .iter()
        .map(|a| {
            let c = data.col(a);
            let vals: Vec<&Cell> = (0..n)
                .map(|r| data.get_idx(r, c))
                .filter(|v| !v.is_empty_like() && !js::trim(&v.js_string()).is_empty())
                .collect();
            (a.clone(), optimized_type(&vals).to_string())
        })
        .collect()
}

fn has(ops: &[String], name: &str) -> bool {
    ops.iter().any(|o| o == name)
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        one.to_string()
    } else {
        many.to_string()
    }
}

pub fn analyze_rows(
    rows: Table,
    ops: &[String],
    options: &Options,
    table_name: &str,
    ctx: &Ctx,
    map_progress: &dyn Fn(f64) -> f64,
) -> Result<TableAnalysis, String> {
    let report = |phase: &str, op: &str, pct: f64, msg: String| {
        (ctx.progress)(Progress {
            phase: phase.into(),
            operation: op.into(),
            progress: map_progress(pct).round(),
            message: msg,
            estimated_time_remaining: Some(ctx.eta(pct / 100.0)),
            memory_usage: Some(ctx.memory_mb),
        });
    };
    let mut a = TableAnalysis {
        lossless_check: json!({"isLossless": false}),
        dependency_preservation: json!({"isPreserved": false}),
        ..Default::default()
    };
    let mut data = rows;

    if has(ops, "Data Cleaner") {
        ctx.check()?;
        report("PREP", "data-cleaner", 15.0, format!("{table_name}: cleaning data…"));
        data = data_processor::clean(&data);
        a.log
            .push(("SUCCESS", format!("Data cleaned: {} rows remaining", data.len())));
    }
    let mut attributes = data.keys();

    if has(ops, "Atomicity Checker") {
        ctx.check()?;
        report(
            "PREP",
            "atomicity-checker",
            25.0,
            format!("{table_name}: checking atomicity…"),
        );
        let sample = data_processor::progressive_sample(&data, data.len().min(5000));
        let res = data_processor::check_atomicity(&sample);
        let multi: Vec<&String> = res.atomicity.iter().filter(|(_, v)| !**v).map(|(k, _)| k).collect();
        if !multi.is_empty() {
            a.log.push(("WARN", format!(
                "Possible multi-valued column(s): {}. These violate 1NF if each cell holds several values; consider moving them to a child table.",
                multi.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
            )));
        }
        a.atomicity_result = Some(res);
    }
    if has(ops, "Data Type Profiler") {
        ctx.check()?;
        report(
            "PREP",
            "data-profiler",
            35.0,
            format!("{table_name}: profiling data types…"),
        );
        let sample = data_processor::progressive_sample(&data, data.len().min(2000));
        a.data_profile = data_processor::profile_data_types(&sample);
    }
    if has(ops, "Column Classifier") {
        ctx.check()?;
        report(
            "PREP",
            "column-classifier",
            38.0,
            format!("{table_name}: classifying columns…"),
        );
        a.ner_tags = classify_table(table_name, &attributes, &data);
    }

    let mut children: Vec<Relation> = Vec::new();
    if has(ops, "1NF") {
        ctx.check()?;
        report(
            "NORMAL",
            "1nf",
            40.0,
            format!("{table_name}: applying First Normal Form…"),
        );
        let atomicity = match &a.atomicity_result {
            Some(r) if !r.atomicity.is_empty() => r.clone(),
            _ => data_processor::check_atomicity(&data_processor::progressive_sample(&data, 5000)),
        };
        let multi: Vec<String> = atomicity
            .atomicity
            .iter()
            .filter(|(_, v)| !**v)
            .map(|(k, _)| k.clone())
            .collect();
        if !multi.is_empty() {
            if let Some(split) = split_multi_valued_columns(&data, &multi, table_name) {
                data = split.parent_rows;
                attributes = data.keys();
                let names: Vec<String> = split.children.iter().map(|c| c.name.clone()).collect();
                a.notes.push(format!(
                    "{table_name}: multi-valued column{} {} moved to child table{} {} (1NF).",
                    plural(split.moved.len(), "", "s"),
                    split.moved.join(", "),
                    plural(split.moved.len(), "", "s"),
                    names.join(", ")
                ));
                children = split.children;
            }
        }
        let pk = suggest_primary_key(&data, &attributes);
        let mut r1 = vec![Relation {
            name: table_name.to_string(),
            attributes: attributes.clone(),
            primary_key: pk,
            rows: data.slice(0, 100),
            source_type: Some("1NF".into()),
            metadata: Metadata {
                row_count: data.len() as f64,
                attribute_types: optimized_types(&data, &attributes),
            },
            ..Default::default()
        }];
        r1.extend(children.iter().cloned());
        a.relations_1nf = r1;
    }

    if has(ops, "FD Miner") {
        ctx.check()?;
        report(
            "ANALYZE",
            "fd-miner",
            50.0,
            format!("{table_name}: mining functional dependencies…"),
        );
        let max_lhs = options.max_lhs_size.filter(|v| *v != 0.0 && !v.is_nan()).unwrap_or(4.0) as usize;
        let mined = nf::mine_fds_detailed(
            &data,
            max_lhs,
            MineOptions {
                aborted: Some(ctx.cancelled),
                time_budget_ms: Some(options.timeout.unwrap_or(30_000.0)),
                sample_size: Some(options.fd_sample_size.unwrap_or(10_000.0) as usize),
                near_key_ratio: Some(options.near_key_ratio.unwrap_or(0.9)),
                complete_determinants_only: options.complete_determinants_only.unwrap_or(true),
                min_rows_for_non_key_determinants: Some(
                    options.min_rows_for_non_key_determinants.unwrap_or(10.0) as usize
                ),
                ..Default::default()
            },
        );
        ctx.check()?;
        a.functional_dependencies = mined.fds.iter().map(|f| f.to_fd()).collect();
        if let Some(c) = &mined.partition_cache {
            a.cache_hits += c.hits;
            a.cache_misses += c.misses;
        }
        if let Some(s) = &mined.sampling {
            a.notes.push(format!(
                "{table_name}: dependencies were mined on a {}-row sample and then checked against all {} rows{}",
                js::locale_usize(s.sample_size),
                js::locale_usize(s.total_rows),
                if s.refuted > 0 {
                    format!(
                        " ({} candidate{} refuted by the full data).",
                        s.refuted,
                        if s.refuted == 1 { " was" } else { "s were" }
                    )
                } else {
                    ".".into()
                }
            ));
        }
        let exact = nf::exact_fds(&a.functional_dependencies);
        let approx_used = options.accept_approximate_fds.unwrap_or(false);
        a.used_fds = if approx_used {
            a.functional_dependencies.clone()
        } else {
            exact.clone()
        };
        let search = FdSearch {
            truncated: mined.truncated,
            level_reached: mined.level_reached,
            max_lhs_size: max_lhs,
            exact: exact.len(),
            approximate: a.functional_dependencies.len() - exact.len(),
            approximate_used: approx_used,
        };
        if search.approximate > 0 && !search.approximate_used {
            a.notes.push(format!(
                "{table_name}: {} approximate dependenc{} reported but not used for keys or normal forms (they are violated by some rows).",
                search.approximate,
                if search.approximate == 1 { "y was" } else { "ies were" }
            ));
        }
        if mined.truncated {
            a.notes.push(format!(
                "{table_name}: FD search stopped after determinants of size {}; larger determinants were not explored.",
                mined.level_reached
            ));
        }
        a.fd_search = Some(search);
    }

    let run_keys = |a: &mut TableAnalysis, attributes: &mut Vec<String>, data: &mut Table| {
        let mut kr = nf::find_keys(attributes, &a.used_fds);
        let choice = choose_key(attributes, data, &kr.candidate_keys, table_name);
        if choice.surrogate_added {
            let key = choice.primary_key[0].clone();
            let extra: Vec<Fd> = attributes
                .iter()
                .filter(|x| **x != key)
                .map(|x| Fd::new(vec![key.clone()], x.clone(), 1.0, 1.0, 0.0, data.len() as f64))
                .collect();
            a.used_fds.extend(extra.iter().cloned());
            a.functional_dependencies.extend(extra);
            kr.candidate_keys = vec![choice.primary_key.clone()];
            kr.superkeys = vec![choice.primary_key.clone()];
            a.notes.push(format!("{table_name}: {}", choice.reason));
        }
        kr.primary_key = Some(choice.primary_key.clone());
        kr.key_choice = Some(choice.clone());
        a.key_result = kr;
        a.key_choice = Some(choice);
    };

    if has(ops, "Key Discovery") {
        ctx.check()?;
        report(
            "ANALYZE",
            "key-discovery",
            60.0,
            format!("{table_name}: discovering keys…"),
        );
        run_keys(&mut a, &mut attributes, &mut data);
    }
    if has(ops, "Cardinality Analyzer") {
        ctx.check()?;
        report(
            "ANALYZE",
            "cardinality-analyzer",
            65.0,
            format!("{table_name}: analyzing cardinalities…"),
        );
        a.cardinality = nf::analyze_cardinalities(&data);
    }
    if has(ops, "2NF") {
        ctx.check()?;
        report(
            "NORMAL",
            "2nf",
            70.0,
            format!("{table_name}: applying Second Normal Form…"),
        );
        let preferred = a.key_choice.as_ref().map(|k| k.primary_key.clone());
        a.relations_2nf = nf::apply_2nf_preferring(&data, &a.used_fds, preferred.as_deref());
    }
    if has(ops, "3NF") {
        ctx.check()?;
        report(
            "NORMAL",
            "3nf",
            75.0,
            format!("{table_name}: synthesizing Third Normal Form…"),
        );
        // The key chosen for the table goes first, so the main relation keeps it as its primary key.
        let keys = nf::keys_preferring(
            &a.key_result.candidate_keys,
            a.key_choice.as_ref().map(|k| k.primary_key.as_slice()),
        );
        a.relations_3nf = nf::synthesize_3nf(&a.used_fds, &keys, &data, None);
    }
    if has(ops, "BCNF") {
        ctx.check()?;
        report("NORMAL", "bcnf", 78.0, format!("{table_name}: decomposing to BCNF…"));
        // The input set is borrowed, not copied (it holds every row of the table).
        let mut fallback: Vec<Relation> = Vec::new();
        if a.relations_3nf.is_empty() && a.relations_2nf.is_empty() {
            if a.key_choice.is_none() {
                run_keys(&mut a, &mut attributes, &mut data);
            }
            fallback = vec![Relation {
                name: new_relation_id(),
                attributes: attributes.clone(),
                primary_key: a.key_choice.as_ref().map(|k| k.primary_key.clone()).unwrap_or_default(),
                rows: data.clone(),
                metadata: Metadata {
                    row_count: data.len() as f64,
                    attribute_types: Default::default(),
                },
                ..Default::default()
            }];
        }
        let source: &[Relation] = if !a.relations_3nf.is_empty() {
            &a.relations_3nf
        } else if !a.relations_2nf.is_empty() {
            &a.relations_2nf
        } else {
            &fallback
        };
        let bcnf = nf::apply_bcnf(source, &a.used_fds);
        drop(fallback);
        // Large tables keep the rows once: in the final (3NF slot) set; the BCNF set is a row-less copy.
        a.compact = data.len() * data.keys().len().max(1) > crate::pipeline::COMPACT_RESULT_CELLS;
        if a.compact {
            a.relations_bcnf = bcnf.iter().map(Relation::shadow).collect();
            a.relations_3nf = bcnf;
        } else {
            a.relations_3nf = bcnf.clone();
            a.relations_bcnf = bcnf;
        }
        a.bcnf_is_final = true;
    }

    // The checks below read attributes and dependencies only, so the copy leaves the rows out.
    let final_rel: Vec<Relation> = if !a.relations_3nf.is_empty() {
        a.relations_3nf.iter().map(Relation::shadow).collect()
    } else if !a.relations_2nf.is_empty() {
        a.relations_2nf.iter().map(Relation::shadow).collect()
    } else {
        a.relations_1nf.iter().map(Relation::shadow).collect()
    };

    if has(ops, "Lossless Check") {
        ctx.check()?;
        report(
            "NORMAL",
            "lossless-check",
            80.0,
            format!("{table_name}: checking lossless join…"),
        );
        let chase = nf::chase(&final_rel, &a.used_fds).is_lossless;
        let attrs: Vec<Vec<String>> = final_rel.iter().map(|r| r.attributes.clone()).collect();
        let inst = verify_lossless_on_data(&data, &attrs, 250_000);
        a.lossless_check = json!({
            "isLossless": chase && inst.lossless != Some(false),
            "chaseLossless": chase,
            "instanceLossless": inst.lossless,
            "details": {
                "algorithm": "Chase (w.r.t. FDs) + re-join of projections (on data)",
                "relationCount": final_rel.len(),
                "fdCount": a.used_fds.len(),
                "instance": inst,
            }
        });
        if chase && inst.lossless == Some(false) {
            a.notes.push(format!(
                "{table_name}: the chase accepts the decomposition but re-joining the data produced {} spurious and {} missing rows; some dependencies may be coincidental.",
                inst.spurious_rows, inst.missing_rows
            ));
        }
    }
    if has(ops, "Dependency Preservation") {
        ctx.check()?;
        report(
            "NORMAL",
            "dependency-preservation",
            85.0,
            format!("{table_name}: checking dependency preservation…"),
        );
        let d = nf::dependency_preservation(&final_rel, &a.used_fds);
        a.dependency_preservation = json!({
            "isPreserved": d.is_preserved,
            "preserved": d.preserved,
            "lost": d.lost,
            "details": d.details,
            "totalDependencies": d.total_dependencies,
            "preservedCount": d.preserved.len(),
            "lostCount": d.lost.len(),
            "preservationRatio": d.preservation_ratio,
        });
    }

    attributes = data.keys();
    if !children.is_empty() {
        let with_children = |set: &[Relation]| -> Vec<Relation> {
            if set.is_empty() {
                return vec![];
            }
            let mut out = set.to_vec();
            for child in &children {
                let link: Vec<String> = child.primary_key[..child.primary_key.len().saturating_sub(1)].to_vec();
                let parent = set
                    .iter()
                    .find(|r| link.iter().all(|k| r.primary_key.contains(k)) && r.primary_key.len() == link.len())
                    .or_else(|| set.iter().find(|r| link.iter().all(|k| r.attributes.contains(k))));
                let mut c = child.clone();
                c.foreign_keys = parent
                    .map(|p| {
                        vec![ForeignKey {
                            attributes: link.clone(),
                            references: p.name.clone(),
                            referenced_key: link.clone(),
                            cardinality: Some("1:N".into()),
                        }]
                    })
                    .unwrap_or_default();
                out.push(c);
            }
            out
        };
        a.relations_2nf = with_children(&a.relations_2nf);
        a.relations_3nf = with_children(&a.relations_3nf);
        a.relations_bcnf = if a.bcnf_is_final && a.compact {
            a.relations_3nf.iter().map(Relation::shadow).collect()
        } else if a.bcnf_is_final {
            a.relations_3nf.clone()
        } else {
            with_children(&a.relations_bcnf)
        };
    }
    a.rows = data;
    a.attributes = attributes;
    Ok(a)
}
