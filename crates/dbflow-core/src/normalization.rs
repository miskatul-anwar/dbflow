//! Keys, closures, canonical cover, normal-form tests, 2NF/3NF/BCNF decomposition,
//! chase, dependency preservation and cardinality analysis.

use crate::fd_miner::{self, MineOptions, MineResult};
use crate::js;
use crate::naming::new_relation_id;
use crate::table::{Cell, Table};
use crate::types::{dedup, sorted, Fd, ForeignKey, Metadata, OriginalFd, Relation, Rhs, SFd};
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Dependency helpers
// ---------------------------------------------------------------------------

/// Every (lhs, rhs) pair of `fds` with one right-hand attribute, in order.
pub fn singletons(fds: &[Fd]) -> Vec<SFd> {
    let mut out = Vec::new();
    for fd in fds {
        for r in fd.rhs.list() {
            out.push(SFd {
                lhs: fd.lhs.clone(),
                rhs: r,
            });
        }
    }
    out
}

/// Non-trivial singleton dependencies whose attributes all lie in `attrs` (when given).
fn valid_within(fds: &[Fd], attrs: Option<&[String]>, require_rhs_in: bool) -> Vec<SFd> {
    let mut out = Vec::new();
    for fd in fds {
        if fd.lhs.is_empty() {
            continue;
        }
        if let Some(a) = attrs {
            if !fd.lhs.iter().all(|x| a.contains(x)) {
                continue;
            }
        }
        for r in fd.rhs.list() {
            if r.is_empty() || fd.lhs.contains(&r) {
                continue;
            }
            if require_rhs_in {
                if let Some(a) = attrs {
                    if !a.contains(&r) {
                        continue;
                    }
                }
            }
            out.push(SFd {
                lhs: fd.lhs.clone(),
                rhs: r,
            });
        }
    }
    out
}

/// Attribute closure X+ under `fds`.
pub fn closure(attrs: &[String], fds: &[SFd]) -> IndexSet<String> {
    let mut c: IndexSet<String> = attrs.iter().cloned().collect();
    let mut changed = true;
    while changed {
        changed = false;
        for fd in fds {
            if fd.lhs.iter().all(|a| c.contains(a)) && !fd.rhs.is_empty() && !c.contains(&fd.rhs) {
                c.insert(fd.rhs.clone());
                changed = true;
            }
        }
    }
    c
}

pub fn exact_fds(fds: &[Fd]) -> Vec<Fd> {
    fds.iter()
        .filter(|f| f.violations == Some(0.0) && f.confidence.unwrap_or(0.0) >= 1.0)
        .cloned()
        .collect()
}

fn subsets(arr: &[String], size: usize) -> Vec<Vec<String>> {
    if size == 0 || size > arr.len() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut idx: Vec<usize> = (0..size).collect();
    loop {
        out.push(idx.iter().map(|&i| arr[i].clone()).collect());
        let mut i = size as isize - 1;
        while i >= 0 && idx[i as usize] == arr.len() - size + i as usize {
            i -= 1;
        }
        if i < 0 {
            return out;
        }
        idx[i as usize] += 1;
        for j in i as usize + 1..size {
            idx[j] = idx[j - 1] + 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

/// All minimal candidate keys (L/R/B/N classification plus lattice search on B).
pub fn find_candidate_keys(attributes: &[String], fds: &[Fd]) -> Vec<Vec<String>> {
    let all = sorted(dedup(attributes.iter().cloned()));
    if all.is_empty() {
        return vec![];
    }
    if all.len() == 1 {
        return vec![vec![all[0].clone()]];
    }
    let valid = valid_within(fds, Some(&all), true);
    let mut lhs_attrs = HashSet::new();
    let mut rhs_attrs = HashSet::new();
    for fd in &valid {
        for a in &fd.lhs {
            lhs_attrs.insert(a.clone());
        }
        rhs_attrs.insert(fd.rhs.clone());
    }
    let l: Vec<String> = all
        .iter()
        .filter(|a| lhs_attrs.contains(*a) && !rhs_attrs.contains(*a))
        .cloned()
        .collect();
    let n: Vec<String> = all
        .iter()
        .filter(|a| !lhs_attrs.contains(*a) && !rhs_attrs.contains(*a))
        .cloned()
        .collect();
    let b: Vec<String> = all
        .iter()
        .filter(|a| lhs_attrs.contains(*a) && rhs_attrs.contains(*a))
        .cloned()
        .collect();
    let core = sorted(dedup(l.into_iter().chain(n)));
    if closure(&core, &valid).len() == all.len() {
        return vec![if core.is_empty() { all.clone() } else { core }];
    }
    let mut keys: Vec<Vec<String>> = Vec::new();
    let blist = sorted(b);
    for size in 1..=blist.len() {
        for subset in subsets(&blist, size) {
            let cand = sorted(dedup(core.iter().cloned().chain(subset)));
            if keys.iter().any(|k| k.iter().all(|x| cand.contains(x))) {
                continue;
            }
            if closure(&cand, &valid).len() == all.len() {
                keys.push(cand);
            }
        }
    }
    if keys.is_empty() {
        keys.push(all.clone());
    }
    keys.sort_by(|a, b| {
        a.len()
            .cmp(&b.len())
            .then_with(|| js::locale_compare(&a.join(","), &b.join(",")))
    });
    keys
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct KeyPhase {
    pub name: String,
    pub evaluated: usize,
    pub keys_found: usize,
    pub duration: f64,
    pub success: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchMetrics {
    pub strategy: String,
    pub phases: Vec<KeyPhase>,
    pub total_evaluated: usize,
    pub total_duration: f64,
    pub final_strategy: String,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct KeyResult {
    pub candidate_keys: Vec<Vec<String>>,
    pub superkeys: Vec<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_metrics: Option<SearchMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_key: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_choice: Option<crate::keys::KeyChoice>,
}

pub fn find_keys(attributes: &[String], fds: &[Fd]) -> KeyResult {
    let start = js::now_ms();
    let keys = find_candidate_keys(attributes, fds);
    let duration = (js::now_ms() - start).round();
    let name = "L/R/B/N Attribute Partitioning & Lattice Search".to_string();
    let success = !keys.is_empty();
    KeyResult {
        superkeys: keys.clone(),
        search_metrics: Some(SearchMetrics {
            strategy: "LRBN-lattice-closure".into(),
            phases: vec![KeyPhase {
                name: name.clone(),
                evaluated: keys.len(),
                keys_found: keys.len(),
                duration,
                success,
            }],
            total_evaluated: keys.len(),
            total_duration: duration,
            final_strategy: if success { name } else { "none".into() },
        }),
        candidate_keys: keys,
        primary_key: None,
        key_choice: None,
    }
}

/// Minimal keys out of a list of superkeys.
pub fn extract_candidate_keys(superkeys: &[Vec<String>]) -> Vec<Vec<String>> {
    if superkeys.is_empty() {
        return vec![];
    }
    let mut unique: Vec<Vec<String>> = dedup(superkeys.iter().map(|k| sorted(k.clone()).join("|")))
        .into_iter()
        .map(|s| s.split('|').map(String::from).collect())
        .collect();
    unique.sort_by_key(|k: &Vec<String>| k.len());
    let mut out: Vec<Vec<String>> = Vec::new();
    for key in unique {
        if !out
            .iter()
            .any(|c| c.iter().all(|a| key.contains(a)) && c.len() < key.len())
        {
            out.push(key);
        }
    }
    out
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GroupedFd {
    pub lhs: Vec<String>,
    pub rhs: Vec<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CanonicalCover {
    pub singleton_cover: Vec<Fd>,
    pub grouped_cover: Vec<GroupedFd>,
    pub original_count: usize,
    pub canonical_count: usize,
    pub extraneous_attributes_removed: usize,
    #[serde(rename = "redundantFDsRemoved")]
    pub redundant_fds_removed: usize,
}

fn lc_join(a: &[String]) -> String {
    a.join(",")
}

/// Canonical (minimal) cover.
pub fn canonical_cover(fds: &[Fd]) -> CanonicalCover {
    if fds.is_empty() {
        return CanonicalCover {
            singleton_cover: vec![],
            grouped_cover: vec![],
            original_count: 0,
            canonical_count: 0,
            extraneous_attributes_removed: 0,
            redundant_fds_removed: 0,
        };
    }
    let mut extraneous = 0;
    let mut redundant = 0;
    let mut working: Vec<SFd> = Vec::new();
    for fd in fds {
        let lhs = sorted(dedup(fd.lhs.iter().cloned()));
        for r in fd.rhs.list() {
            if !r.is_empty() && !lhs.contains(&r) && !lhs.is_empty() {
                working.push(SFd {
                    lhs: lhs.clone(),
                    rhs: r,
                });
            }
        }
    }
    let mut seen = HashSet::new();
    working.retain(|f| seen.insert(format!("{}|{}", f.lhs.join(","), f.rhs)));
    working.sort_by(|a, b| {
        js::locale_compare(&lc_join(&a.lhs), &lc_join(&b.lhs)).then_with(|| js::locale_compare(&a.rhs, &b.rhs))
    });

    for i in 0..working.len() {
        if working[i].lhs.len() <= 1 {
            continue;
        }
        let mut reduced = true;
        while reduced && working[i].lhs.len() > 1 {
            reduced = false;
            let lhs = working[i].lhs.clone();
            for attr in &lhs {
                let test: Vec<String> = working[i].lhs.iter().filter(|a| *a != attr).cloned().collect();
                if closure(&test, &working).contains(&working[i].rhs) {
                    working[i].lhs = test;
                    extraneous += 1;
                    reduced = true;
                    break;
                }
            }
        }
    }
    let mut seen = HashSet::new();
    working.retain(|f| {
        if f.lhs.contains(&f.rhs) {
            return false;
        }
        seen.insert(format!("{}|{}", sorted(f.lhs.clone()).join(","), f.rhs))
    });
    let mut i = 0;
    while i < working.len() {
        let without: Vec<SFd> = working
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, f)| f.clone())
            .collect();
        if closure(&working[i].lhs, &without).contains(&working[i].rhs) {
            working.remove(i);
            redundant += 1;
        } else {
            i += 1;
        }
    }
    let mut singleton: Vec<Fd> = working
        .iter()
        .map(|f| Fd::new(sorted(f.lhs.clone()), f.rhs.clone(), 1.0, 1.0, 0.0, 1.0))
        .collect();
    singleton.sort_by(|a, b| {
        js::locale_compare(&lc_join(&a.lhs), &lc_join(&b.lhs))
            .then_with(|| js::locale_compare(&a.rhs_name(), &b.rhs_name()))
    });
    let mut grouped: IndexMap<String, (Vec<String>, IndexSet<String>)> = IndexMap::new();
    for f in &working {
        let lhs = sorted(f.lhs.clone());
        let key = lhs.join(",");
        grouped
            .entry(key)
            .or_insert_with(|| (lhs, IndexSet::new()))
            .1
            .insert(f.rhs.clone());
    }
    let mut grouped_cover: Vec<GroupedFd> = grouped
        .into_values()
        .map(|(lhs, rhs)| GroupedFd {
            lhs,
            rhs: sorted(rhs.into_iter().collect()),
        })
        .collect();
    grouped_cover.sort_by(|a, b| js::locale_compare(&lc_join(&a.lhs), &lc_join(&b.lhs)));
    CanonicalCover {
        canonical_count: singleton.len(),
        singleton_cover: singleton,
        grouped_cover,
        original_count: fds.len(),
        extraneous_attributes_removed: extraneous,
        redundant_fds_removed: redundant,
    }
}

pub fn classify_prime(attributes: &[String], keys: &[Vec<String>]) -> (Vec<String>, Vec<String>, HashSet<String>) {
    let prime: IndexSet<String> = keys.iter().flatten().cloned().collect();
    let set: HashSet<String> = prime.iter().cloned().collect();
    let prime_attrs = sorted(prime.into_iter().collect());
    let non_prime = sorted(attributes.iter().filter(|a| !set.contains(*a)).cloned().collect());
    (prime_attrs, non_prime, set)
}

// ---------------------------------------------------------------------------
// Normal-form tests
// ---------------------------------------------------------------------------

fn violation(desc: &str, props: Value) -> Value {
    let mut m = match props {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    m.insert("description".into(), Value::String(desc.into()));
    m.insert("__string".into(), Value::Bool(true));
    Value::Object(m)
}

pub fn is_1nf(rows: &Table, attributes: Option<&[String]>) -> Value {
    let attrs: Vec<String> = match attributes {
        Some(a) if !a.is_empty() => a.to_vec(),
        _ => rows.keys(),
    };
    if rows.is_empty() || attrs.is_empty() {
        return json!({"nf":"1NF","isNormalized":true,"is1NF":true,"violations":[],"violationDetails":[],
            "totalAttributes":attrs.len(),"atomicAttributes":attrs,"nonAtomicAttributes":[]});
    }
    let limit = rows.len().min(500);
    let mut non_atomic: IndexSet<String> = IndexSet::new();
    let mut violations = Vec::new();
    let mut details = Vec::new();
    for attr in &attrs {
        for r in 0..limit {
            let v = rows.get(r, attr);
            if v.is_empty_like() {
                continue;
            }
            let desc = match v {
                Cell::Json(Value::Array(_)) => Some(format!("Column '{attr}' contains nested array data.")),
                Cell::Json(_) => Some(format!("Column '{attr}' contains complex nested object data.")),
                Cell::Str(s) => {
                    let t = js::trim(s);
                    if (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('{') && t.ends_with('}')) {
                        match serde_json::from_str::<Value>(t) {
                            Ok(Value::Array(_)) | Ok(Value::Object(_)) => {
                                Some(format!("Column '{attr}' contains serialized JSON arrays/objects."))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(desc) = desc {
                non_atomic.insert(attr.clone());
                let d = json!({"type":"1NF_NON_ATOMIC","culprit":attr,"description":desc,"sampleValue":v.to_json()});
                violations.push(violation(&desc, d.clone()));
                details.push(d);
                break;
            }
        }
    }
    let ok = details.is_empty();
    let non = sorted(non_atomic.iter().cloned().collect());
    let atomic = sorted(attrs.iter().filter(|a| !non_atomic.contains(*a)).cloned().collect());
    json!({"nf":"1NF","isNormalized":ok,"is1NF":ok,"violations":violations,"violationDetails":details,
        "totalAttributes":attrs.len(),"atomicAttributes":atomic,"nonAtomicAttributes":non})
}

pub fn is_2nf(attributes: &[String], fds: &[Fd], keys: Option<&[Vec<String>]>) -> Value {
    let keys: Vec<Vec<String>> = keys
        .map(|k| k.to_vec())
        .unwrap_or_else(|| find_candidate_keys(attributes, fds));
    let (prime, non_prime, is_prime) = classify_prime(attributes, &keys);
    let composite = keys.iter().any(|k| k.len() > 1);
    if non_prime.is_empty() || !composite {
        return json!({"nf":"2NF","isNormalized":true,"is2NF":true,"violations":[],"violationDetails":[],
            "partialDependencies":[],"primeAttributes":prime,"candidateKeys":keys});
    }
    let valid = valid_within(fds, Some(attributes), true);
    let (mut violations, mut details, mut partial) = (vec![], vec![], vec![]);
    for fd in &valid {
        if is_prime.contains(&fd.rhs) {
            continue;
        }
        for key in &keys {
            if !fd.lhs.is_empty() && fd.lhs.len() < key.len() && fd.lhs.iter().all(|a| key.contains(a)) {
                let desc = format!(
                    "Partial dependency: Non-prime attribute '{}' depends on {{{}}}, which is a proper subset of candidate key {{{}}}.",
                    fd.rhs,
                    fd.lhs.join(", "),
                    key.join(", ")
                );
                violations.push(violation(
                    &desc,
                    json!({"lhs":fd.lhs,"rhs":fd.rhs,"candidateKey":key,"type":"2NF_PARTIAL_DEPENDENCY"}),
                ));
                details.push(json!({"type":"2NF_PARTIAL_DEPENDENCY","culprit":format!("{{{}}} -> {}", fd.lhs.join(", "), fd.rhs),
                    "description":desc,"culpritFD":{"lhs":fd.lhs,"rhs":fd.rhs},"candidateKey":key,"attributes":[fd.rhs]}));
                partial.push(json!({"lhs":fd.lhs,"rhs":fd.rhs,"candidateKey":key,"description":desc}));
                break;
            }
        }
    }
    let ok = details.is_empty();
    json!({"nf":"2NF","isNormalized":ok,"is2NF":ok,"violations":violations,"violationDetails":details,
        "partialDependencies":partial,"primeAttributes":prime,"candidateKeys":keys})
}

pub fn is_3nf(attributes: &[String], fds: &[Fd], keys: Option<&[Vec<String>]>) -> Value {
    let keys: Vec<Vec<String>> = keys
        .map(|k| k.to_vec())
        .unwrap_or_else(|| find_candidate_keys(attributes, fds));
    let (prime, _, is_prime) = classify_prime(attributes, &keys);
    let r2 = is_2nf(attributes, fds, Some(&keys));
    if r2["isNormalized"] == Value::Bool(false) {
        let mut v = vec![Value::String("Relation violates 2NF.".into())];
        v.extend(r2["violations"].as_array().cloned().unwrap_or_default());
        return json!({"nf":"3NF","isNormalized":false,"is3NF":false,"violations":v,"violationDetails":r2["violationDetails"],
            "transitiveDependencies":r2["partialDependencies"],"primeAttributes":prime,"candidateKeys":keys});
    }
    let valid = valid_within(fds, Some(attributes), true);
    let (mut violations, mut details, mut trans) = (vec![], vec![], vec![]);
    for fd in &valid {
        if closure(&fd.lhs, &valid).len() == attributes.len() || is_prime.contains(&fd.rhs) {
            continue;
        }
        let desc = format!(
            "Transitive dependency: {{{}}} -> {} violates 3NF (determinant is not a superkey and '{}' is non-prime).",
            fd.lhs.join(", "),
            fd.rhs,
            fd.rhs
        );
        violations.push(violation(
            &desc,
            json!({"lhs":fd.lhs,"rhs":fd.rhs,"type":"3NF_TRANSITIVE_DEPENDENCY"}),
        ));
        details.push(
            json!({"type":"3NF_TRANSITIVE_DEPENDENCY","culprit":format!("{{{}}} -> {}", fd.lhs.join(", "), fd.rhs),
            "description":desc,"culpritFD":{"lhs":fd.lhs,"rhs":fd.rhs},"attributes":[fd.rhs]}),
        );
        trans.push(json!({"lhs":fd.lhs,"rhs":fd.rhs,"description":desc}));
    }
    let ok = details.is_empty();
    json!({"nf":"3NF","isNormalized":ok,"is3NF":ok,"violations":violations,"violationDetails":details,
        "transitiveDependencies":trans,"primeAttributes":prime,"candidateKeys":keys})
}

pub fn is_bcnf(attributes: &[String], fds: &[Fd], keys: Option<&[Vec<String>]>) -> Value {
    let keys: Vec<Vec<String>> = keys
        .map(|k| k.to_vec())
        .unwrap_or_else(|| find_candidate_keys(attributes, fds));
    let (prime, _, _) = classify_prime(attributes, &keys);
    let valid = valid_within(fds, Some(attributes), true);
    let (mut violations, mut details) = (vec![], vec![]);
    for fd in &valid {
        if closure(&fd.lhs, &valid).len() != attributes.len() {
            let desc = format!(
                "BCNF violation: {{{}}} -> {} determinant is not a superkey.",
                fd.lhs.join(", "),
                fd.rhs
            );
            violations.push(violation(
                &desc,
                json!({"lhs":fd.lhs,"rhs":fd.rhs,"type":"BCNF_NON_SUPERKEY_DETERMINANT"}),
            ));
            details.push(json!({"type":"BCNF_NON_SUPERKEY_DETERMINANT","culprit":format!("{{{}}} -> {}", fd.lhs.join(", "), fd.rhs),
                "description":desc,"culpritFD":{"lhs":fd.lhs,"rhs":fd.rhs},"attributes":[fd.rhs]}));
        }
    }
    let ok = details.is_empty();
    json!({"nf":"BCNF","isNormalized":ok,"isBCNF":ok,"violations":violations,"violationDetails":details,
        "primeAttributes":prime,"candidateKeys":keys})
}

pub fn is_bcnf_ok(attributes: &[String], fds: &[Fd]) -> bool {
    let valid = valid_within(fds, Some(attributes), true);
    valid
        .iter()
        .all(|fd| closure(&fd.lhs, &valid).len() == attributes.len())
}

pub fn determine_normal_form(attributes: &[String], fds: &[Fd], rows: &Table, keys: Option<&[Vec<String>]>) -> Value {
    let r1 = is_1nf(rows, Some(attributes));
    let r2 = is_2nf(attributes, fds, keys);
    let r3 = is_3nf(attributes, fds, keys);
    let rb = is_bcnf(attributes, fds, keys);
    let ok = |v: &Value| v["isNormalized"] == Value::Bool(true);
    let mut highest = "UNNORMALIZED";
    if ok(&r1) {
        highest = "1NF";
        if ok(&r2) {
            highest = "2NF";
            if ok(&r3) {
                highest = "3NF";
                if ok(&rb) {
                    highest = "BCNF";
                }
            }
        }
    }
    json!({"highestNormalForm":highest,"details":{"1NF":r1,"2NF":r2,"3NF":r3,"BCNF":rb}})
}

pub fn has_atomic_violations(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    if re!(r"^\s*\[.*\]\s*$").is_match(value) || re!(r"^\s*\{.*\}\s*$").is_match(value) {
        if let Ok(v) = serde_json::from_str::<Value>(value) {
            if v.is_array() || v.is_object() || v.is_null() {
                return true;
            }
        }
    }
    re!(r";\s*[^;]").is_match(value) || re!(r"\|\s*[^|]").is_match(value)
}

// ---------------------------------------------------------------------------
// Relation data helpers
// ---------------------------------------------------------------------------

/// Distinct projection of `data` onto `attributes` (`row[attr] ?? ''`).
pub fn extract_relation_data(data: &Table, attributes: &[String]) -> Table {
    let cols: Vec<Option<usize>> = attributes.iter().map(|a| data.col(a)).collect();
    let mut out = Table::with_columns(attributes);
    let out_idx: Vec<usize> = attributes.iter().map(|a| out.col(a).unwrap_or(0)).collect();
    let mut seen: HashSet<Vec<std::borrow::Cow<'_, str>>> = HashSet::new();
    for r in 0..data.len() {
        let key: Vec<std::borrow::Cow<'_, str>> = cols.iter().map(|&c| data.get_idx(r, c).text_or_empty()).collect();
        if seen.insert(key) {
            let mut cells = vec![Cell::Absent; out.columns_raw().len()];
            for (&c, &o) in cols.iter().zip(&out_idx) {
                let v = data.get_idx(r, c);
                cells[o] = if v.is_nullish() { Cell::str("") } else { v.clone() };
            }
            out.push_row(cells);
        }
    }
    out
}

/// Legacy per-column type guess used for relation metadata.
pub fn legacy_infer_data_type(values: &[&Cell]) -> String {
    if values.is_empty() {
        return "string".into();
    }
    let sample = &values[..values.len().min(20)];
    let text: Vec<String> = sample.iter().map(|v| v.js_string()).collect();
    if text
        .iter()
        .all(|t| re!(r"(?i)^(true|false|yes|no|y|n|0|1|on|off)$").is_match(t))
    {
        return "boolean".into();
    }
    let numeric = sample.iter().zip(&text).all(|(v, t)| match v {
        Cell::Num(n) => !n.is_nan(),
        Cell::Bool(_) => false,
        _ => !js::is_nan_str(t) && !js::parse_float(t).is_nan(),
    });
    if numeric {
        return if text.iter().any(|t| t.contains('.')) {
            "decimal"
        } else {
            "integer"
        }
        .into();
    }
    let date_re = re!(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}|[0-9]{2}/[0-9]{2}/[0-9]{4}|[0-9]{2}-[0-9]{2}-[0-9]{4}");
    if text.iter().all(|t| date_re.is_match(t) || js::date_parse_valid(t)) {
        return "date".into();
    }
    "string".into()
}

pub fn infer_attribute_types(data: &Table, attributes: &[String]) -> IndexMap<String, String> {
    let n = data.len().min(50);
    let mut out = IndexMap::new();
    for a in attributes {
        let c = data.col(a);
        let vals: Vec<&Cell> = (0..n).map(|r| data.get_idx(r, c)).filter(|v| !v.is_nullish()).collect();
        out.insert(a.clone(), legacy_infer_data_type(&vals));
    }
    out
}

pub fn make_relation(name: String, attrs: Vec<String>, pk: Vec<String>, rows: Table, source: &str) -> Relation {
    let types = infer_attribute_types(&rows, &attrs);
    Relation {
        name,
        metadata: Metadata {
            row_count: rows.len() as f64,
            attribute_types: types,
        },
        attributes: attrs,
        primary_key: pk,
        rows,
        source_type: Some(source.into()),
        ..Default::default()
    }
}

fn analyze_cardinality(source_rows: &Table, target_pk: &[String], target_has_rows: bool) -> String {
    if source_rows.is_empty() || !target_has_rows {
        return "1:N".into();
    }
    let cols: Vec<Option<usize>> = target_pk.iter().map(|a| source_rows.col(a)).collect();
    let mut counts: HashMap<Vec<std::borrow::Cow<'_, str>>, usize> = HashMap::new();
    for r in 0..source_rows.len() {
        let key: Vec<std::borrow::Cow<'_, str>> = cols
            .iter()
            .map(|&c| {
                let v = source_rows.get_idx(r, c);
                if v.truthy() {
                    v.text()
                } else {
                    std::borrow::Cow::Borrowed("")
                }
            })
            .collect();
        // `parts.join('||').trim() !== ''`
        if key.iter().all(|k| js::trim(k).is_empty()) && key.len() <= 1 {
            continue;
        }
        *counts.entry(key).or_insert(0) += 1;
    }
    if counts.is_empty() {
        return "1:N".into();
    }
    let max = counts.values().copied().max().unwrap_or(0);
    let avg = counts.values().sum::<usize>() as f64 / counts.len() as f64;
    if max == 1 && avg == 1.0 {
        "1:1".into()
    } else if max > 1 {
        "1:N".into()
    } else {
        "N:M".into()
    }
}

/// Foreign keys between relations of one decomposition: R references S when S's key lies in R.
pub fn establish_foreign_keys(relations: &mut [Relation]) {
    let mut all_fks: Vec<Vec<ForeignKey>> = Vec::with_capacity(relations.len());
    for (i, rel) in relations.iter().enumerate() {
        let mut fks = Vec::new();
        for (j, other) in relations.iter().enumerate() {
            if i == j {
                continue;
            }
            let pk_in = other.primary_key.iter().all(|a| rel.attributes.contains(a));
            let same_pk = rel.primary_key.len() == other.primary_key.len()
                && rel.primary_key.iter().all(|a| other.primary_key.contains(a));
            if pk_in && !same_pk {
                fks.push(ForeignKey {
                    attributes: other.primary_key.clone(),
                    references: other.name.clone(),
                    referenced_key: other.primary_key.clone(),
                    cardinality: Some(analyze_cardinality(
                        &rel.rows,
                        &other.primary_key,
                        !other.rows.is_empty(),
                    )),
                });
            }
        }
        all_fks.push(fks);
    }
    for (rel, fks) in relations.iter_mut().zip(all_fks) {
        rel.foreign_keys = fks;
    }
}

// ---------------------------------------------------------------------------
// Decompositions
// ---------------------------------------------------------------------------

/// 1NF: non-destructive copy, or unnest of repeating groups.
pub fn apply_1nf(data: &Table, unnest: Option<(&[String], Option<&str>)>) -> Table {
    if data.is_empty() {
        return Table::new();
    }
    if let Some((cols, delimiter)) = unnest {
        if !cols.is_empty() {
            let mut current = data.clone();
            for col in cols {
                let mut next = Table::new();
                for c in current.columns_raw() {
                    next.col_index_or_add(c);
                }
                let ci = next.col_index_or_add(col);
                for r in 0..current.len() {
                    let v = current.get(r, col).clone();
                    let items: Vec<Cell> = match &v {
                        Cell::Json(Value::Array(a)) => a.iter().cloned().map(Cell::from_json).collect(),
                        Cell::Str(s) => match serde_json::from_str::<Value>(s) {
                            Ok(Value::Array(a)) => a.into_iter().map(Cell::from_json).collect(),
                            Ok(_) => vec![],
                            Err(_) => {
                                let parts: Vec<&str> = match delimiter {
                                    Some(d) => s.split(d).collect(),
                                    None => re!(r"[;|]").split(s).collect(),
                                };
                                parts
                                    .into_iter()
                                    .map(js::trim)
                                    .filter(|x| !x.is_empty())
                                    .map(Cell::str)
                                    .collect()
                            }
                        },
                        _ => vec![],
                    };
                    let mut base = current.rows[r].clone();
                    base.resize(next.columns_raw().len(), Cell::Absent);
                    if items.len() > 1 {
                        for it in items {
                            let mut row = base.clone();
                            row[ci] = it;
                            next.push_row(row);
                        }
                    } else {
                        next.push_row(base);
                    }
                }
                current = next;
            }
            return current;
        }
    }
    data.clone()
}

pub fn apply_2nf(data: &Table, fds: &[Fd]) -> Vec<Relation> {
    if data.is_empty() {
        return vec![];
    }
    let attributes = data.keys();
    let keys = find_keys(&attributes, fds).candidate_keys;
    if keys.is_empty() {
        return vec![make_relation(
            new_relation_id(),
            attributes.clone(),
            attributes[..1.min(attributes.len())].to_vec(),
            data.clone(),
            "2NF_Fallback",
        )];
    }
    let pk = keys[0].clone();
    let (prime, _, _) = classify_prime(&attributes, &keys);
    let partial: Vec<&Fd> = fds
        .iter()
        .filter(|fd| {
            let proper = keys
                .iter()
                .any(|k| !fd.lhs.is_empty() && fd.lhs.len() < k.len() && fd.lhs.iter().all(|a| k.contains(a)));
            proper && !prime.contains(&fd.rhs_name())
        })
        .collect();
    if partial.is_empty() {
        return vec![make_relation(new_relation_id(), attributes, pk, data.clone(), "2NF")];
    }
    let mut rels = Vec::new();
    for fd in &partial {
        let attrs = dedup(fd.lhs.iter().cloned().chain([fd.rhs_name()]));
        let rows = extract_relation_data(data, &attrs);
        let mut r = make_relation(new_relation_id(), attrs, fd.lhs.clone(), rows, "2NF_PartialDependency");
        r.original_fd = Some(OriginalFd {
            lhs: fd.lhs.clone(),
            rhs: fd.rhs_name(),
            confidence: fd.conf(),
        });
        rels.push(r);
    }
    let used: HashSet<String> = partial
        .iter()
        .flat_map(|fd| fd.lhs.iter().cloned().chain([fd.rhs_name()]))
        .collect();
    let remaining: Vec<String> = attributes
        .iter()
        .filter(|a| !used.contains(*a) || pk.contains(a))
        .cloned()
        .collect();
    if !remaining.is_empty() {
        let rows = extract_relation_data(data, &remaining);
        rels.push(make_relation(new_relation_id(), remaining, pk, rows, "2NF_Main"));
    }
    rels
}

pub fn apply_3nf(relations: &[Relation], fds: &[Fd]) -> Vec<Relation> {
    let mut out = Vec::new();
    for rel in relations {
        let attrs = rel.attributes.clone();
        let rel_fds: Vec<Fd> = fds
            .iter()
            .filter(|fd| {
                !fd.lhs.is_empty() && fd.lhs.iter().all(|a| attrs.contains(a)) && attrs.contains(&fd.rhs_name())
            })
            .cloned()
            .collect();
        let mut keys = find_candidate_keys(&attrs, &rel_fds);
        if keys.is_empty() && !rel.primary_key.is_empty() {
            keys = vec![rel.primary_key.clone()];
        }
        let (_, _, is_prime) = classify_prime(&attrs, &keys);
        let rel_s = singletons(&rel_fds);
        let mut violations: IndexMap<String, (Vec<String>, IndexSet<String>)> = IndexMap::new();
        for fd in &rel_fds {
            let rhs = fd.rhs_name();
            if fd.lhs.contains(&rhs) || closure(&fd.lhs, &rel_s).len() == attrs.len() || is_prime.contains(&rhs) {
                continue;
            }
            let lhs = sorted(fd.lhs.clone());
            violations
                .entry(lhs.join(","))
                .or_insert_with(|| (lhs, IndexSet::new()))
                .1
                .insert(rhs);
        }
        if violations.is_empty() {
            out.push(rel.clone());
            continue;
        }
        let mut decomposed = Vec::new();
        let mut all_rhs: HashSet<String> = HashSet::new();
        for (lhs, rhs) in violations.values() {
            all_rhs.extend(rhs.iter().cloned());
            let dattrs = sorted(dedup(lhs.iter().cloned().chain(rhs.iter().cloned())));
            let rows = extract_relation_data(&rel.rows, &dattrs);
            decomposed.push(make_relation(
                new_relation_id(),
                dattrs,
                lhs.clone(),
                rows,
                "3NF_Decomposed",
            ));
        }
        let residual: Vec<String> = attrs.iter().filter(|a| !all_rhs.contains(*a)).cloned().collect();
        let rpk: Vec<String> = rel
            .primary_key
            .iter()
            .filter(|a| residual.contains(a))
            .cloned()
            .collect();
        let eff = if rpk.is_empty() { residual.clone() } else { rpk };
        let rrows = extract_relation_data(&rel.rows, &residual);
        let mut fks = Vec::new();
        for d in &decomposed {
            if d.primary_key.iter().all(|a| residual.contains(a)) {
                fks.push(ForeignKey {
                    attributes: d.primary_key.clone(),
                    references: d.name.clone(),
                    referenced_key: d.primary_key.clone(),
                    cardinality: Some(analyze_cardinality(&rrows, &d.primary_key, !d.rows.is_empty())),
                });
            }
        }
        let mut r = make_relation(new_relation_id(), residual, eff, rrows, "3NF_Residual");
        r.foreign_keys = fks;
        out.extend(decomposed);
        out.push(r);
    }
    out
}

/// Bernstein 3NF synthesis.
pub fn synthesize_3nf(
    fds: &[Fd],
    candidate_keys: &[Vec<String>],
    data: &Table,
    all_attributes: Option<&[String]>,
) -> Vec<Relation> {
    let attributes: Vec<String> = match all_attributes {
        Some(a) if !a.is_empty() => a.to_vec(),
        _ if !data.is_empty() => data.keys(),
        _ => dedup(
            fds.iter()
                .flat_map(|f| f.lhs.iter().cloned().chain([f.rhs_name()]))
                .chain(candidate_keys.iter().flatten().cloned()),
        ),
    };
    if attributes.is_empty() {
        return vec![];
    }
    if fds.is_empty() {
        let pk = candidate_keys.first().cloned().unwrap_or_else(|| attributes.clone());
        let rows = extract_relation_data(data, &attributes);
        let types = infer_attribute_types(data, &attributes);
        return vec![Relation {
            name: new_relation_id(),
            attributes: attributes.clone(),
            primary_key: pk,
            rows,
            source_type: Some("3NF_Universal".into()),
            metadata: Metadata {
                row_count: data.len() as f64,
                attribute_types: types,
            },
            ..Default::default()
        }];
    }
    let cc = canonical_cover(fds);
    let mut keys: Vec<Vec<String>> = if !candidate_keys.is_empty() {
        candidate_keys.to_vec()
    } else {
        find_candidate_keys(&attributes, &cc.singleton_cover)
    };
    if keys.is_empty() {
        keys = vec![attributes.clone()];
    }
    let mut synth: Vec<Relation> = Vec::new();
    let group_attrs: Vec<Vec<String>> = cc
        .grouped_cover
        .iter()
        .map(|g| sorted(dedup(g.lhs.iter().cloned().chain(g.rhs.iter().cloned()))))
        .collect();
    let group_rows: Vec<Table> = crate::par_map(&group_attrs, |attrs| extract_relation_data(data, attrs));
    for ((g, attrs), rows) in cc.grouped_cover.iter().zip(group_attrs).zip(group_rows) {
        let mut r = make_relation(new_relation_id(), attrs, sorted(g.lhs.clone()), rows, "3NF_Synthesized");
        r.original_fd = Some(OriginalFd {
            lhs: g.lhs.clone(),
            rhs: g.rhs.join(", "),
            confidence: 1.0,
        });
        synth.push(r);
    }
    let rank = |r: &Relation| -> usize {
        keys.iter()
            .position(|k| k.len() == r.primary_key.len() && k.iter().all(|a| r.primary_key.contains(a)))
            .unwrap_or(usize::MAX)
    };
    let mut keep = Vec::new();
    for i in 0..synth.len() {
        let r1 = &synth[i];
        let redundant = synth.iter().enumerate().any(|(j, r2)| {
            if i == j || !r1.attributes.iter().all(|a| r2.attributes.contains(a)) {
                return false;
            }
            if r2.attributes.len() > r1.attributes.len() {
                return true;
            }
            if r2.attributes.len() == r1.attributes.len() {
                let (a, b) = (rank(r2), rank(r1));
                if a != b {
                    return a < b;
                }
                return j < i;
            }
            false
        });
        if !redundant {
            keep.push(i);
        }
    }
    let mut non: Vec<Relation> = keep.into_iter().map(|i| synth[i].clone()).collect();
    for rel in &mut non {
        let current = keys
            .iter()
            .position(|k| k.len() == rel.primary_key.len() && k.iter().all(|a| rel.primary_key.contains(a)));
        let Some(current) = current else { continue };
        if current == 0 {
            continue;
        }
        if let Some(better) = keys.iter().position(|k| k.iter().all(|a| rel.attributes.contains(a))) {
            if better < current {
                rel.primary_key = sorted(keys[better].clone());
            }
        }
    }
    let has_key = keys
        .iter()
        .any(|k| non.iter().any(|r| k.iter().all(|a| r.attributes.contains(a))));
    if !has_key {
        let mut key = keys[0].clone();
        for k in &keys {
            if k.len() < key.len() {
                key = k.clone();
            }
        }
        let rows = extract_relation_data(data, &key);
        let types = infer_attribute_types(&rows, &key);
        non.push(Relation {
            name: new_relation_id(),
            attributes: sorted(key.clone()),
            primary_key: sorted(key),
            metadata: Metadata {
                row_count: rows.len() as f64,
                attribute_types: types,
            },
            rows,
            source_type: Some("3NF_CandidateKey_Lossless".into()),
            ..Default::default()
        });
    }
    let covered: HashSet<String> = non.iter().flat_map(|r| r.attributes.iter().cloned()).collect();
    let missing: Vec<String> = attributes.iter().filter(|a| !covered.contains(*a)).cloned().collect();
    if !missing.is_empty() {
        let rows = extract_relation_data(data, &missing);
        let types = infer_attribute_types(&rows, &missing);
        non.push(Relation {
            name: new_relation_id(),
            attributes: sorted(missing.clone()),
            primary_key: sorted(missing),
            metadata: Metadata {
                row_count: rows.len() as f64,
                attribute_types: types,
            },
            rows,
            source_type: Some("3NF_IndependentAttributes".into()),
            ..Default::default()
        });
    }
    establish_foreign_keys(&mut non);
    non
}

fn build_bcnf_relation(name: String, attributes: &[String], fds: &[Fd], rows: &Table) -> Relation {
    let keys = find_candidate_keys(attributes, fds);
    let pk = keys
        .first()
        .cloned()
        .unwrap_or_else(|| attributes.first().cloned().into_iter().collect());
    let projected = extract_relation_data(rows, attributes);
    let types = infer_attribute_types(&projected, attributes);
    Relation {
        name,
        attributes: sorted(attributes.to_vec()),
        primary_key: sorted(pk),
        metadata: Metadata {
            row_count: projected.len() as f64,
            attribute_types: types,
        },
        rows: projected,
        source_type: Some("BCNF_Decomposed".into()),
        ..Default::default()
    }
}

fn prune_redundant(relations: Vec<Relation>) -> Vec<Relation> {
    let mut sorted_rels = relations;
    sorted_rels.sort_by_key(|r| std::cmp::Reverse(r.attributes.len()));
    let mut retained: Vec<Relation> = Vec::new();
    for rel in sorted_rels {
        if !retained
            .iter()
            .any(|o| rel.attributes.iter().all(|a| o.attributes.contains(a)))
        {
            retained.push(rel);
        }
    }
    retained.reverse();
    retained
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BcnfStep {
    pub source_relation: String,
    #[serde(rename = "violatingFD")]
    pub violating_fd: GroupedFd,
    pub decomposed_relations: (String, String),
}

pub struct BcnfResult {
    pub relations: Vec<Relation>,
    pub is_lossless: bool,
    pub preservation: Preservation,
    pub steps: Vec<BcnfStep>,
}

/// BCNF decomposition of one relation (or attribute list with data).
pub fn decompose_bcnf_detailed(attributes: &[String], rows: &Table, base_name: &str, fds: &[Fd]) -> BcnfResult {
    let valid: Vec<SFd> = {
        let mut v = Vec::new();
        for fd in fds {
            if fd.lhs.is_empty() {
                continue;
            }
            for r in fd.rhs.list() {
                if !r.is_empty() && !fd.lhs.contains(&r) {
                    v.push(SFd {
                        lhs: fd.lhs.clone(),
                        rhs: r,
                    });
                }
            }
        }
        v
    };
    let valid_fds: Vec<Fd> = valid
        .iter()
        .map(|s| Fd {
            lhs: s.lhs.clone(),
            rhs: Rhs::One(s.rhs.clone()),
            ..Default::default()
        })
        .collect();
    let mut queue: std::collections::VecDeque<(String, Vec<String>, Table)> = std::collections::VecDeque::new();
    queue.push_back((
        base_name.to_string(),
        sorted(dedup(attributes.iter().cloned())),
        rows.clone(),
    ));
    let mut done = Vec::new();
    let mut steps = Vec::new();
    while let Some((name, cur, crow)) = queue.pop_front() {
        if cur.len() <= 2 {
            done.push(build_bcnf_relation(name, &cur, &valid_fds, &crow));
            continue;
        }
        let mut violation: Option<(Vec<String>, Vec<String>)> = None;
        for fd in &valid {
            if fd.lhs.iter().all(|a| cur.contains(a)) && cur.contains(&fd.rhs) {
                let det: Vec<String> = closure(&fd.lhs, &valid)
                    .into_iter()
                    .filter(|a| cur.contains(a) && !fd.lhs.contains(a))
                    .collect();
                if !det.is_empty() && fd.lhs.len() + det.len() < cur.len() {
                    violation = Some((fd.lhs.clone(), det));
                    break;
                }
            }
        }
        if violation.is_none() {
            let max = (cur.len() - 1).min(3);
            'outer: for size in 1..=max {
                for subset in subsets(&cur, size) {
                    let det: Vec<String> = closure(&subset, &valid)
                        .into_iter()
                        .filter(|a| cur.contains(a) && !subset.contains(a))
                        .collect();
                    if !det.is_empty() && subset.len() + det.len() < cur.len() {
                        violation = Some((subset, det));
                        break 'outer;
                    }
                }
            }
        }
        match violation {
            None => done.push(build_bcnf_relation(name, &cur, &valid_fds, &crow)),
            Some((x, y)) => {
                let r1 = sorted(dedup(x.iter().cloned().chain(y.iter().cloned())));
                let r2 = sorted(cur.iter().filter(|a| !y.contains(a)).cloned().collect());
                let (n1, n2) = (new_relation_id(), new_relation_id());
                steps.push(BcnfStep {
                    source_relation: name.clone(),
                    violating_fd: GroupedFd { lhs: x, rhs: y },
                    decomposed_relations: (n1.clone(), n2.clone()),
                });
                let rows1 = extract_relation_data(&crow, &r1);
                let rows2 = extract_relation_data(&crow, &r2);
                queue.push_back((n1, r1, rows1));
                queue.push_back((n2, r2, rows2));
            }
        }
    }
    let mut fin = prune_redundant(done);
    establish_foreign_keys(&mut fin);
    let is_lossless = chase(&fin, fds).is_lossless;
    let preservation = dependency_preservation(&fin, fds);
    BcnfResult {
        relations: fin,
        is_lossless,
        preservation,
        steps,
    }
}

pub fn apply_bcnf(relations: &[Relation], fds: &[Fd]) -> Vec<Relation> {
    if relations.is_empty() {
        return vec![];
    }
    let mut out = Vec::new();
    for rel in relations {
        if is_bcnf_ok(&rel.attributes, fds) {
            out.push(rel.clone());
        } else {
            out.extend(decompose_bcnf_detailed(&rel.attributes, &rel.rows, &rel.name, fds).relations);
        }
    }
    let mut uniq = prune_redundant(out);
    establish_foreign_keys(&mut uniq);
    uniq
}

// ---------------------------------------------------------------------------
// Chase, projected closure, dependency preservation
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChaseStep {
    pub iteration: usize,
    pub fd: Value,
    pub equated_symbol: String,
    pub replaced_symbol: String,
    pub affected_rows: Vec<usize>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChaseResult {
    pub is_lossless: bool,
    pub tableau: Vec<Vec<String>>,
    pub attributes: Vec<String>,
    pub iterations: usize,
    pub fully_distinguished_row_index: i64,
    pub steps: Vec<ChaseStep>,
}

pub fn initialize_tableau(relations: &[Relation], all: &[String]) -> Vec<Vec<String>> {
    relations
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (0..all.len())
                .map(|j| {
                    if r.attributes.contains(&all[j]) {
                        format!("a_{j}")
                    } else {
                        format!("b_{i}_{j}")
                    }
                })
                .collect()
        })
        .collect()
}

fn fully_distinguished(t: &[Vec<String>], cols: usize) -> i64 {
    for (i, row) in t.iter().enumerate() {
        if (0..cols).all(|j| row.get(j).map(|s| s == &format!("a_{j}")).unwrap_or(false)) {
            return i as i64;
        }
    }
    -1
}

pub fn has_all_equal_row(t: &[Vec<String>]) -> bool {
    !t.is_empty() && fully_distinguished(t, t[0].len()) != -1
}

pub fn chase(relations: &[Relation], fds: &[Fd]) -> ChaseResult {
    if relations.is_empty() {
        return ChaseResult {
            is_lossless: false,
            tableau: vec![],
            attributes: vec![],
            iterations: 0,
            fully_distinguished_row_index: -1,
            steps: vec![],
        };
    }
    let all = sorted(dedup(relations.iter().flat_map(|r| r.attributes.iter().cloned())));
    if relations.len() == 1 {
        return ChaseResult {
            is_lossless: true,
            tableau: vec![(0..all.len()).map(|j| format!("a_{j}")).collect()],
            attributes: all,
            iterations: 0,
            fully_distinguished_row_index: 0,
            steps: vec![],
        };
    }
    let mut t = initialize_tableau(relations, &all);
    let fdr = fully_distinguished(&t, all.len());
    if fdr != -1 {
        return ChaseResult {
            is_lossless: true,
            tableau: t,
            attributes: all,
            iterations: 0,
            fully_distinguished_row_index: fdr,
            steps: vec![],
        };
    }
    let mut valid = Vec::new();
    for fd in fds {
        if fd.lhs.is_empty() || !fd.lhs.iter().all(|a| all.contains(a)) {
            continue;
        }
        for r in fd.rhs.list() {
            if !r.is_empty() && all.contains(&r) && !fd.lhs.contains(&r) {
                valid.push(SFd {
                    lhs: fd.lhs.clone(),
                    rhs: r,
                });
            }
        }
    }
    let pos = |a: &str| all.iter().position(|x| x == a);
    let max_iter = (valid.len() * all.len() * relations.len()).max(100);
    let mut changed = true;
    let mut iterations = 0;
    let mut steps = Vec::new();
    while changed && iterations < max_iter {
        changed = false;
        iterations += 1;
        for fd in &valid {
            let lhs_idx: Vec<usize> = fd.lhs.iter().filter_map(|a| pos(a)).collect();
            let Some(rhs_idx) = pos(&fd.rhs) else {
                continue;
            };
            let mut groups: IndexMap<String, Vec<usize>> = IndexMap::new();
            for (i, row) in t.iter().enumerate() {
                let key = lhs_idx.iter().map(|&c| row[c].as_str()).collect::<Vec<_>>().join("||");
                groups.entry(key).or_default().push(i);
            }
            for rows in groups.values() {
                if rows.len() <= 1 {
                    continue;
                }
                let symbols: IndexSet<String> = rows.iter().map(|&r| t[r][rhs_idx].clone()).collect();
                if symbols.len() <= 1 {
                    continue;
                }
                let dist = format!("a_{rhs_idx}");
                let target = if symbols.contains(&dist) {
                    dist
                } else {
                    let mut s: Vec<String> = symbols.iter().cloned().collect();
                    js::sort_strings(&mut s);
                    s[0].clone()
                };
                for old in &symbols {
                    if *old == target {
                        continue;
                    }
                    let mut affected = Vec::new();
                    for (r, row) in t.iter_mut().enumerate() {
                        if row[rhs_idx] == *old {
                            row[rhs_idx] = target.clone();
                            affected.push(r);
                            changed = true;
                        }
                    }
                    steps.push(ChaseStep {
                        iteration: iterations,
                        fd: json!({"lhs": fd.lhs, "rhs": fd.rhs}),
                        equated_symbol: target.clone(),
                        replaced_symbol: old.clone(),
                        affected_rows: affected,
                    });
                }
                let fdr = fully_distinguished(&t, all.len());
                if fdr != -1 {
                    return ChaseResult {
                        is_lossless: true,
                        tableau: t,
                        attributes: all,
                        iterations,
                        fully_distinguished_row_index: fdr,
                        steps,
                    };
                }
            }
        }
    }
    let fdr = fully_distinguished(&t, all.len());
    ChaseResult {
        is_lossless: fdr != -1,
        tableau: t,
        attributes: all,
        iterations,
        fully_distinguished_row_index: fdr,
        steps,
    }
}

pub fn projected_closure(x: &[String], relations: &[Relation], all_fds: &[Fd]) -> IndexSet<String> {
    let mut result: IndexSet<String> = x.iter().cloned().collect();
    let normalized: Vec<SFd> = {
        let mut v = Vec::new();
        for fd in all_fds {
            if fd.lhs.is_empty() {
                continue;
            }
            for r in fd.rhs.list() {
                v.push(SFd {
                    lhs: fd.lhs.clone(),
                    rhs: r,
                });
            }
        }
        v
    };
    let mut changed = true;
    while changed {
        changed = false;
        for rel in relations {
            let common: Vec<String> = rel.attributes.iter().filter(|a| result.contains(*a)).cloned().collect();
            if common.is_empty() {
                continue;
            }
            let cc = closure(&common, &normalized);
            for a in &rel.attributes {
                if cc.contains(a) && !result.contains(a) {
                    result.insert(a.clone());
                    changed = true;
                }
            }
        }
    }
    result
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreservationDetail {
    pub fd: Fd,
    pub is_preserved: bool,
    pub reason: String,
    pub derived_closure: Vec<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Preservation {
    pub is_preserved: bool,
    pub preservation_ratio: f64,
    pub preserved: Vec<Fd>,
    pub lost: Vec<Fd>,
    pub total_dependencies: usize,
    pub details: Vec<PreservationDetail>,
}

pub fn dependency_preservation(relations: &[Relation], fds: &[Fd]) -> Preservation {
    if fds.is_empty() {
        return Preservation {
            is_preserved: true,
            preservation_ratio: 1.0,
            ..Default::default()
        };
    }
    let mut p = Preservation::default();
    for fd in fds {
        let rhs = fd.rhs.list();
        let single = relations
            .iter()
            .any(|r| fd.lhs.iter().all(|a| r.attributes.contains(a)) && rhs.iter().all(|a| r.attributes.contains(a)));
        if single {
            p.preserved.push(fd.clone());
            p.details.push(PreservationDetail {
                fd: fd.clone(),
                is_preserved: true,
                reason: "single_relation".into(),
                derived_closure: dedup(fd.lhs.iter().cloned().chain(rhs.iter().cloned())),
            });
            continue;
        }
        let pc = projected_closure(&fd.lhs, relations, fds);
        let ok = rhs.iter().all(|r| pc.contains(r));
        let detail = PreservationDetail {
            fd: fd.clone(),
            is_preserved: ok,
            reason: if ok { "projected_closure" } else { "not_preserved" }.into(),
            derived_closure: pc.into_iter().collect(),
        };
        if ok {
            p.preserved.push(fd.clone());
        } else {
            p.lost.push(fd.clone());
        }
        p.details.push(detail);
    }
    p.total_dependencies = fds.len();
    p.preservation_ratio = p.preserved.len() as f64 / fds.len() as f64;
    p.is_preserved = p.lost.is_empty();
    p
}

// ---------------------------------------------------------------------------
// Cardinalities and misc exported helpers
// ---------------------------------------------------------------------------

pub fn analyze_cardinalities(data: &Table) -> IndexMap<String, Value> {
    let mut out = IndexMap::new();
    if data.is_empty() {
        return out;
    }
    let keys = data.keys();
    let results: Vec<Value> = crate::par_map(&keys, |attr| {
        let values: Vec<&Cell> = data.column(attr).into_iter().filter(|v| !v.is_nullish()).collect();
        let unique: HashSet<std::borrow::Cow<'_, str>> = values.iter().map(|v| v.text()).collect();
        // Most frequent value (a Map keyed by value: equal primitives of the same type share a key).
        let mut freq: IndexMap<(u8, std::borrow::Cow<'_, str>), (usize, usize)> = IndexMap::new();
        for (i, v) in values.iter().enumerate() {
            let tag = match v {
                Cell::Str(_) => 0,
                Cell::Num(_) => 1,
                Cell::Bool(_) => 2,
                _ => 3,
            };
            let key = if tag == 3 {
                std::borrow::Cow::Owned(format!("{i}"))
            } else {
                v.text()
            };
            freq.entry((tag, key)).or_insert((i, 0)).1 += 1;
        }
        let mut best: (Value, usize) = (Value::Null, 0);
        for (_, (i, f)) in freq {
            if f > best.1 {
                best = (values[i].to_json(), f);
            }
        }
        let n = values.len() as f64;
        json!({
                "totalValues": values.len(),
                "uniqueValues": unique.len(),
                "cardinality": if n > 0.0 { unique.len() as f64 / n } else { 0.0 },
                "duplicateRatio": if n > 0.0 { (n - unique.len() as f64) / n } else { 0.0 },
                "mostFrequent": { "value": best.0, "frequency": best.1 },
                "dataType": legacy_infer_data_type(&values[..values.len()]),
        })
    });
    for (k, v) in keys.into_iter().zip(results) {
        out.insert(k, v);
    }
    out
}

pub fn ensure_candidate_key_coverage(relations: &mut Vec<Relation>, keys: &[Vec<String>], data: &Table) {
    if keys.is_empty() {
        return;
    }
    let covered = keys
        .iter()
        .any(|k| relations.iter().any(|r| k.iter().all(|a| r.attributes.contains(a))));
    if !covered {
        let k = keys[0].clone();
        let rows = extract_relation_data(data, &k);
        relations.push(make_relation(
            new_relation_id(),
            k.clone(),
            k,
            rows,
            "CandidateKey_LosslessJoin",
        ));
    }
}

pub fn remove_redundant_preserving_lossless(relations: &[Relation], keys: &[Vec<String>]) -> Vec<Relation> {
    let mut remove: Vec<usize> = Vec::new();
    for (i, r) in relations.iter().enumerate() {
        if keys.iter().any(|k| k.iter().all(|a| r.attributes.contains(a))) {
            continue;
        }
        let redundant = relations.iter().enumerate().any(|(j, o)| {
            i != j
                && !remove.contains(&j)
                && r.attributes.iter().all(|a| o.attributes.contains(a))
                && o.attributes.len() > r.attributes.len()
        });
        if redundant {
            remove.push(i);
        }
    }
    relations
        .iter()
        .enumerate()
        .filter(|(i, _)| !remove.contains(i))
        .map(|(_, r)| r.clone())
        .collect()
}

/// `mineFunctionalDependenciesDetailed`.
pub fn mine_fds_detailed(data: &Table, max_lhs: usize, opts: MineOptions) -> MineResult {
    if data.is_empty() || data.keys().len() < 2 {
        return MineResult {
            level_reached: 0,
            ..Default::default()
        };
    }
    let opts = MineOptions {
        max_lhs_size: Some(max_lhs),
        min_confidence: Some(opts.min_confidence.unwrap_or(0.85)),
        ..opts
    };
    fd_miner::mine_sampled(data, &opts)
}
