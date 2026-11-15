//! Cross-table foreign-key detection and rule-based column classification.

use crate::js;
use crate::table::{Cell, Table};
use crate::types::{CrossLink, EntityTag};
use indexmap::{IndexMap, IndexSet};
use std::collections::HashSet;

pub struct DetectorTable<'a> {
    pub table_name: String,
    pub headers: Vec<String>,
    pub rows: &'a Table,
    pub primary_key: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ValueClass {
    Integer,
    Number,
    Date,
    Text,
}

struct ColumnInfo {
    table: String,
    column: String,
    /// Trimmed text of each distinct value; shares the cell's string when no trimming was needed.
    distinct: IndexSet<crate::table::Text>,
    non_null: usize,
    total: usize,
    unique: bool,
    class: ValueClass,
    int_min: f64,
    int_max: f64,
    dense: bool,
    tokens: Vec<String>,
    key: String,
}

pub fn tokenize_name(name: &str) -> Vec<String> {
    let s = re!(r"([a-z0-9])([A-Z])").replace_all(name, "$1 $2");
    let s = re!(r"[^A-Za-z0-9]+").replace_all(&s, " ");
    s.to_lowercase()
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

pub fn singularize(word: &str) -> String {
    let w = word.to_lowercase();
    if w.len() > 3 && w.ends_with("ies") {
        return format!("{}y", &w[..w.len() - 3]);
    }
    if w.len() > 3 && re!(r"(ches|shes|sses|xes|zes)$").is_match(&w) {
        return w[..w.len() - 2].to_string();
    }
    if w.len() > 2 && w.ends_with('s') && !w.ends_with("ss") && !w.ends_with("us") {
        return w[..w.len() - 1].to_string();
    }
    w
}

fn classify(values: &[&str]) -> ValueClass {
    let n = values.len();
    if n == 0 {
        return ValueClass::Text;
    }
    let ints = values.iter().filter(|v| re!(r"^-?[0-9]+$").is_match(v)).count();
    let nums = values
        .iter()
        .filter(|v| re!(r"^-?[0-9]+(\.[0-9]+)?$").is_match(v))
        .count();
    let dates = values
        .iter()
        .filter(|v| re!(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}").is_match(v))
        .count();
    if ints == n {
        ValueClass::Integer
    } else if nums == n {
        ValueClass::Number
    } else if dates == n {
        ValueClass::Date
    } else {
        ValueClass::Text
    }
}

fn profile(t: &DetectorTable, column: &str) -> ColumnInfo {
    let mut distinct = IndexSet::new();
    let mut non_null = 0;
    let col = t.rows.col(column);
    for r in 0..t.rows.len() {
        let v = t.rows.get_idx(r, col);
        if v.is_missing() {
            continue;
        }
        non_null += 1;
        let text: crate::table::Text = match v {
            Cell::Str(s) if js::trim(s).len() == s.len() => s.clone(),
            other => crate::table::Text::new(js::trim(&other.js_string())),
        };
        distinct.insert(text);
    }
    let sample: Vec<&str> = distinct.iter().take(500).map(|v| &**v).collect();
    let class = classify(&sample);
    let (mut min, mut max, mut dense) = (f64::INFINITY, f64::NEG_INFINITY, false);
    if class == ValueClass::Integer && !distinct.is_empty() {
        for v in &distinct {
            let n = js::number(v);
            min = min.min(n);
            max = max.max(n);
        }
        dense = distinct.len() >= 3 && max - min + 1.0 == distinct.len() as f64 && (0.0..=1.0).contains(&min);
    }
    let tokens = tokenize_name(column);
    ColumnInfo {
        table: t.table_name.clone(),
        column: column.to_string(),
        unique: non_null > 0 && distinct.len() == non_null,
        distinct,
        non_null,
        total: t.rows.len(),
        class,
        int_min: min,
        int_max: max,
        dense,
        key: tokens.join(""),
        tokens,
    }
}

fn compatible(a: &ColumnInfo, b: &ColumnInfo) -> bool {
    let numeric = |c: ValueClass| c == ValueClass::Integer || c == ValueClass::Number;
    a.class == b.class || (numeric(a.class) && numeric(b.class))
}

fn name_evidence(src: &ColumnInfo, tgt: &ColumnInfo) -> f64 {
    let table_tokens: Vec<String> = tokenize_name(&tgt.table).iter().map(|t| singularize(t)).collect();
    let table_key = table_tokens.join("");
    let generic = |k: &str| k == "id" || k == "key" || k == "code" || k == "no";
    if src.key == format!("{table_key}{}", tgt.key) || src.key == format!("{table_key}id") {
        return 1.0;
    }
    if src.key == tgt.key && !generic(&src.key) {
        return 0.9;
    }
    if src.key == tgt.key {
        return 0.15;
    }
    let src_tokens: Vec<String> = src.tokens.iter().map(|t| singularize(t)).collect();
    let tgt_tokens: Vec<String> = table_tokens
        .iter()
        .cloned()
        .chain(tgt.tokens.iter().map(|t| singularize(t)))
        .collect();
    let shared = src_tokens
        .iter()
        .filter(|t| tgt_tokens.contains(t) && !generic(t))
        .count();
    if shared > 0 {
        let union: HashSet<&String> = src_tokens.iter().chain(tgt_tokens.iter()).collect();
        return (0.35 + (shared as f64 / union.len() as f64) * 0.9).min(0.85);
    }
    0.0
}

struct Candidate<'a> {
    src: &'a ColumnInfo,
    tgt: &'a ColumnInfo,
    containment: f64,
    coverage: f64,
    name_score: f64,
    score: f64,
}

pub fn detect_foreign_keys(tables: &[DetectorTable], min_containment: f64, min_score: f64) -> Vec<CrossLink> {
    if tables.len() < 2 {
        return vec![];
    }
    // Columns are profiled, and source columns matched, in parallel (native build).
    let profiles: Vec<Vec<ColumnInfo>> = tables
        .iter()
        .map(|t| crate::par_map(&t.headers, |h| profile(t, h)))
        .collect();
    let sources: Vec<(usize, &ColumnInfo)> = profiles
        .iter()
        .enumerate()
        .flat_map(|(si, cols)| cols.iter().map(move |c| (si, c)))
        .collect();
    let per_source: Vec<Vec<Candidate>> = crate::par_map(&sources, |&(si, src)| {
        let mut candidates: Vec<Candidate> = Vec::new();
        {
            if src.non_null == 0 || src.distinct.len() < 2 {
                return candidates;
            }
            for (ti, tcols) in profiles.iter().enumerate() {
                if tables[ti].table_name == tables[si].table_name {
                    continue;
                }
                for tgt in tcols {
                    if !tgt.unique || !compatible(src, tgt) || src.distinct.len() > tgt.distinct.len() {
                        continue;
                    }
                    if src.class == ValueClass::Integer
                        && tgt.class == ValueClass::Integer
                        && (src.int_min < tgt.int_min || src.int_max > tgt.int_max)
                    {
                        continue;
                    }
                    // Stop counting once too many values are missing for the containment threshold.
                    let allowed_misses = ((1.0 - min_containment) * src.distinct.len() as f64).floor() as usize;
                    let mut hits = 0usize;
                    let mut misses = 0usize;
                    for v in &src.distinct {
                        if tgt.distinct.contains(v) {
                            hits += 1;
                        } else {
                            misses += 1;
                            if misses > allowed_misses {
                                break;
                            }
                        }
                    }
                    if misses > allowed_misses {
                        continue;
                    }
                    let containment = hits as f64 / src.distinct.len() as f64;
                    if containment < min_containment {
                        continue;
                    }
                    let coverage = hits as f64 / tgt.distinct.len() as f64;
                    let name_score = name_evidence(src, tgt);
                    if tgt.dense && name_score < 0.5 {
                        continue;
                    }
                    let score = 0.45 * containment + 0.15 * coverage + 0.3 * name_score + 0.1;
                    if score + 1e-9 < min_score {
                        continue;
                    }
                    candidates.push(Candidate {
                        src,
                        tgt,
                        containment,
                        coverage,
                        name_score,
                        score,
                    });
                }
            }
        }
        candidates
    });
    let candidates: Vec<Candidate> = per_source.into_iter().flatten().collect();
    let mut best: IndexMap<String, usize> = IndexMap::new();
    for (i, c) in candidates.iter().enumerate() {
        let k = format!("{}.{}", c.src.table, c.src.column);
        match best.get(&k) {
            Some(&j) if c.score <= candidates[j].score + 1e-9 => {}
            _ => {
                best.insert(k, i);
            }
        }
    }
    let pk_of = |t: &str| {
        tables
            .iter()
            .find(|x| x.table_name == t)
            .map(|x| x.primary_key.clone())
            .unwrap_or_default()
    };
    let mut kept: IndexMap<String, usize> = IndexMap::new();
    for &i in best.values() {
        let c = &candidates[i];
        kept.insert(
            format!("{}.{}->{}.{}", c.src.table, c.src.column, c.tgt.table, c.tgt.column),
            i,
        );
    }
    let mut chosen: Vec<usize> = Vec::new();
    for &i in kept.values() {
        let c = &candidates[i];
        if let Some(&ri) = kept.get(&format!(
            "{}.{}->{}.{}",
            c.tgt.table, c.tgt.column, c.src.table, c.src.column
        )) {
            let r = &candidates[ri];
            let has = |t: &str, col: &str| pk_of(t).iter().any(|k| k == col) as i64;
            let mine = has(&c.tgt.table, &c.tgt.column) - has(&c.src.table, &c.src.column);
            let theirs = has(&r.tgt.table, &r.tgt.column) - has(&r.src.table, &r.src.column);
            if theirs > mine {
                continue;
            }
            if theirs == mine && r.score > c.score + 1e-9 {
                continue;
            }
            if theirs == mine
                && (r.score - c.score).abs() <= 1e-9
                && js::code_unit_cmp(&c.src.table, &c.tgt.table).is_gt()
            {
                continue;
            }
        }
        chosen.push(i);
    }
    let ref_non_unique: HashSet<&String> = chosen
        .iter()
        .filter(|&&i| !candidates[i].src.unique)
        .map(|&i| &candidates[i].tgt.table)
        .collect();
    let mut oriented: Vec<usize> = chosen
        .into_iter()
        .filter(|&i| !(candidates[i].src.unique && ref_non_unique.contains(&candidates[i].src.table)))
        .collect();
    oriented.sort_by(|&a, &b| {
        let (ca, cb) = (&candidates[a], &candidates[b]);
        cb.score
            .partial_cmp(&ca.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| js::locale_compare(&ca.src.table, &cb.src.table))
    });
    let single: Vec<CrossLink> = oriented
        .iter()
        .map(|&i| {
            let c = &candidates[i];
            CrossLink {
                source_table: c.src.table.clone(),
                source_column: c.src.column.clone(),
                target_table: c.tgt.table.clone(),
                target_column: c.tgt.column.clone(),
                source_columns: None,
                target_columns: None,
                cardinality: if c.src.unique { "1:1" } else { "1:N" }.into(),
                confidence: c.score.min(0.99),
                match_ratio: c.containment,
                detected_by: if c.name_score >= 0.5 {
                    "HEURISTIC"
                } else {
                    "VALUE_OVERLAP"
                }
                .into(),
                optional: Some(c.src.non_null < c.src.total),
                coverage: Some(c.coverage),
                name_score: Some(c.name_score),
            }
        })
        .collect();
    let composite = composite_keys(tables, &profiles, min_containment);
    let comp_keys: HashSet<String> = composite
        .iter()
        .map(|c| format!("{}->{}", c.source_table, c.target_table))
        .collect();
    let mut out = composite;
    out.extend(
        single
            .into_iter()
            .filter(|s| !comp_keys.contains(&format!("{}->{}", s.source_table, s.target_table))),
    );
    out
}

const TUPLE_SEP: &str = "\u{1}";

fn composite_keys(tables: &[DetectorTable], profiles: &[Vec<ColumnInfo>], min_containment: f64) -> Vec<CrossLink> {
    let mut out = Vec::new();
    for (ti, tgt_table) in tables.iter().enumerate() {
        let key = &tgt_table.primary_key;
        if key.len() < 2 || key.len() > 4 {
            continue;
        }
        let infos: Option<Vec<&ColumnInfo>> = key
            .iter()
            .map(|k| profiles[ti].iter().find(|c| &c.column == k))
            .collect();
        let Some(infos) = infos else { continue };
        let mut target_tuples = HashSet::new();
        for r in 0..tgt_table.rows.len() {
            if key.iter().any(|k| tgt_table.rows.get(r, k).is_missing()) {
                continue;
            }
            target_tuples.insert(
                key.iter()
                    .map(|k| js::trim(&tgt_table.rows.get(r, k).js_string()).to_string())
                    .collect::<Vec<_>>()
                    .join(TUPLE_SEP),
            );
        }
        if target_tuples.is_empty() {
            continue;
        }
        for (si, src_table) in tables.iter().enumerate() {
            if src_table.table_name == tgt_table.table_name {
                continue;
            }
            let options: Vec<Vec<&ColumnInfo>> = infos
                .iter()
                .map(|k| {
                    profiles[si]
                        .iter()
                        .filter(|c| {
                            c.non_null > 0 && compatible(c, k) && (c.key == k.key || name_evidence(c, k) >= 0.5)
                        })
                        .collect()
                })
                .collect();
            if options.iter().any(|o| o.is_empty()) {
                continue;
            }
            let mut combos: Vec<Vec<&ColumnInfo>> = vec![vec![]];
            for opts in &options {
                let mut next = Vec::new();
                for prefix in &combos {
                    for o in opts {
                        if !prefix.iter().any(|p| std::ptr::eq(*p, *o)) {
                            let mut v = prefix.clone();
                            v.push(*o);
                            next.push(v);
                        }
                    }
                }
                next.truncate(50);
                combos = next;
            }
            let mut best: Option<(Vec<&ColumnInfo>, f64, f64, usize, usize)> = None;
            for cols in &combos {
                let mut seen = HashSet::new();
                let mut nn = 0;
                for r in 0..src_table.rows.len() {
                    if cols.iter().any(|c| src_table.rows.get(r, &c.column).is_missing()) {
                        continue;
                    }
                    nn += 1;
                    seen.insert(
                        cols.iter()
                            .map(|c| js::trim(&src_table.rows.get(r, &c.column).js_string()).to_string())
                            .collect::<Vec<_>>()
                            .join(TUPLE_SEP),
                    );
                }
                if seen.len() < 2 {
                    continue;
                }
                let hits = seen.iter().filter(|t| target_tuples.contains(*t)).count();
                let containment = hits as f64 / seen.len() as f64;
                if containment < min_containment {
                    continue;
                }
                if best.as_ref().map(|b| containment > b.1).unwrap_or(true) {
                    best = Some((
                        cols.clone(),
                        containment,
                        hits as f64 / target_tuples.len() as f64,
                        seen.len(),
                        nn,
                    ));
                }
            }
            let Some((cols, containment, coverage, distinct, nn)) = best else {
                continue;
            };
            out.push(CrossLink {
                source_table: src_table.table_name.clone(),
                source_column: cols[0].column.clone(),
                target_table: tgt_table.table_name.clone(),
                target_column: key[0].clone(),
                source_columns: Some(cols.iter().map(|c| c.column.clone()).collect()),
                target_columns: Some(key.clone()),
                cardinality: if distinct == nn { "1:1" } else { "1:N" }.into(),
                confidence: (0.55 + 0.4 * containment).min(0.99),
                match_ratio: containment,
                detected_by: "HEURISTIC".into(),
                optional: Some(nn < src_table.rows.len()),
                coverage: Some(coverage),
                name_score: Some(1.0),
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Semantic column profiler
// ---------------------------------------------------------------------------

pub fn profiler_tokenize(name: &str) -> Vec<String> {
    let s = re!(r"([a-z0-9])([A-Z])").replace_all(name, "$1 $2");
    re!(r"[\s_\-.]+")
        .split(&s)
        .map(|t| js::trim(&t.to_lowercase()).to_string())
        .filter(|t| !t.is_empty())
        .collect()
}

pub fn profiler_singularize(word: &str) -> String {
    let l = word.to_lowercase();
    if l.len() > 3 && l.ends_with('s') && !l.ends_with("ss") && !l.ends_with("us") && !l.ends_with("is") {
        return l[..l.len() - 1].to_string();
    }
    l
}

struct NameRule {
    kind: &'static str,
    confidence: f64,
    tokens: &'static [&'static str],
    suffixes: &'static [&'static str],
    prefixes: &'static [&'static str],
    description: &'static str,
}

const NAME_RULES: &[NameRule] = &[
    NameRule {
        kind: "EMAIL",
        confidence: 0.95,
        tokens: &["email", "mail", "e_mail"],
        suffixes: &[],
        prefixes: &[],
        description: "E-mail address",
    },
    NameRule {
        kind: "URL",
        confidence: 0.9,
        tokens: &["url", "link", "website", "homepage"],
        suffixes: &[],
        prefixes: &[],
        description: "URL / web address",
    },
    NameRule {
        kind: "UUID",
        confidence: 0.95,
        tokens: &["uuid", "guid"],
        suffixes: &[],
        prefixes: &[],
        description: "UUID identifier",
    },
    NameRule {
        kind: "PHONE_NUMBER",
        confidence: 0.92,
        tokens: &["phone", "mobile", "tel", "telephone", "fax"],
        suffixes: &[],
        prefixes: &[],
        description: "Phone number",
    },
    NameRule {
        kind: "PERSON_NAME",
        confidence: 0.85,
        tokens: &[
            "first_name",
            "last_name",
            "firstname",
            "lastname",
            "fullname",
            "full_name",
            "fname",
            "lname",
            "name",
        ],
        suffixes: &[],
        prefixes: &[],
        description: "Person name",
    },
    NameRule {
        kind: "DATE_TIMESTAMP",
        confidence: 0.9,
        tokens: &[
            "date",
            "time",
            "created",
            "updated",
            "deleted",
            "timestamp",
            "dob",
            "birthday",
            "expires",
            "expiry",
        ],
        suffixes: &["_at", "_on", "_date", "_time", "_until", "_since"],
        prefixes: &[],
        description: "Date or timestamp",
    },
    NameRule {
        kind: "CURRENCY_PRICE",
        confidence: 0.88,
        tokens: &[
            "price", "amount", "cost", "total", "subtotal", "revenue", "salary", "fee", "payment", "discount",
            "balance", "budget",
        ],
        suffixes: &[],
        prefixes: &[],
        description: "Currency / monetary amount",
    },
    NameRule {
        kind: "LOCATION_ADDRESS",
        confidence: 0.85,
        tokens: &[
            "city",
            "state",
            "country",
            "address",
            "street",
            "zip",
            "zipcode",
            "postal",
            "location",
            "latitude",
            "longitude",
            "region",
        ],
        suffixes: &[],
        prefixes: &[],
        description: "Location or address component",
    },
    NameRule {
        kind: "ORGANIZATION_COMPANY",
        confidence: 0.85,
        tokens: &[
            "company",
            "org",
            "organization",
            "vendor",
            "supplier",
            "brand",
            "manufacturer",
            "publisher",
            "department",
            "studio",
        ],
        suffixes: &[],
        prefixes: &[],
        description: "Organization / company",
    },
    NameRule {
        kind: "PRODUCT_ITEM",
        confidence: 0.8,
        tokens: &["product", "item", "title", "sku", "book", "movie", "song", "article"],
        suffixes: &[],
        prefixes: &[],
        description: "Product or item title",
    },
    NameRule {
        kind: "CATEGORY_TYPE",
        confidence: 0.78,
        tokens: &[
            "type", "category", "status", "kind", "genre", "priority", "level", "tier", "stage", "severity",
        ],
        suffixes: &[],
        prefixes: &[],
        description: "Categorical label (low cardinality)",
    },
    NameRule {
        kind: "BOOLEAN_FLAG",
        confidence: 0.7,
        tokens: &["active", "enabled", "verified", "flag"],
        suffixes: &[],
        prefixes: &["is_", "has_"],
        description: "Boolean-like flag",
    },
    NameRule {
        kind: "NUMERIC_METRIC",
        confidence: 0.8,
        tokens: &[
            "count",
            "qty",
            "quantity",
            "stock",
            "score",
            "rating",
            "age",
            "year",
            "views",
            "likes",
            "downloads",
        ],
        suffixes: &[],
        prefixes: &[],
        description: "Numeric measure",
    },
];

const PERSON_EXCLUDES: [&str; 7] = ["product", "item", "company", "file", "table", "column", "field"];

fn numeric_pat(v: &str) -> bool {
    re!(r"^[+-]?[0-9]*\.?[0-9]+([eE][+-]?[0-9]+)?$").is_match(v)
}

pub fn looks_like_date(v: &str) -> bool {
    re!(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}([T ][0-9]{2}:[0-9]{2}(:[0-9]{2})?)?(Z|[+-][0-9]{2}:?[0-9]{2})?$").is_match(v)
        || re!(r"^[0-9]{1,2}[/.][0-9]{1,2}[/.][0-9]{2,4}$").is_match(v)
        || re!(r"^[A-Za-z]{3,9}\s+[0-9]{1,2},?\s+[0-9]{4}$").is_match(v)
        || (js::date_parse_valid(v) && re!(r"[0-9]{4}").is_match(v) && js::len16(v) > 5 && !numeric_pat(v))
}

fn value_evidence(samples: &[String], filled: usize, distinct: usize) -> Vec<(&'static str, f64)> {
    if samples.is_empty() || filled == 0 {
        return vec![];
    }
    let n = samples.len() as f64;
    let count = |f: &dyn Fn(&str) -> bool| samples.iter().filter(|v| f(v)).count() as f64;
    let mut out = Vec::new();
    let email = count(&|v| re!(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").is_match(v));
    if email > 0.0 {
        out.push(("EMAIL", email / n));
    }
    let url = count(&|v| re!(r"(?i)^(https?://|www\.)\S+$").is_match(v));
    if url > 0.0 {
        out.push(("URL", url / n));
    }
    let uuid = count(&|v| re!(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").is_match(v));
    if uuid > 0.0 {
        out.push(("UUID", uuid / n));
    }
    let date = count(&|v| looks_like_date(v));
    if date > 0.0 {
        out.push(("DATE_TIMESTAMP", date / n));
    }
    let b = count(&|v| re!(r"(?i)^(true|false|yes|no|y|n|0|1)$").is_match(js::trim(v)));
    if b > 0.0 {
        out.push(("BOOLEAN_FLAG", b / n));
    }
    let num = count(&|v| numeric_pat(&re!(r"[$,\s]").replace_all(v, "")));
    if num > 0.0 {
        out.push(("NUMERIC_METRIC", num / n));
    }
    if distinct > 0 && distinct <= 32 && distinct < filled.max(1) {
        out.push(("CATEGORY_TYPE", 0.85));
    }
    let ints = count(&|v| re!(r"^[+-]?[0-9]+$").is_match(v));
    if ints / n >= 0.95 && distinct as f64 > filled as f64 * 0.85 {
        out.push(("FOREIGN_KEY_REF", 0.55));
    }
    out
}

fn name_class(table: &str, column: &str) -> (&'static str, f64, String) {
    let tokens = profiler_tokenize(column);
    if tokens.is_empty() {
        return ("GENERAL_TEXT", 0.3, "Text attribute".into());
    }
    let joined = tokens.join("_");
    let ts = profiler_singularize(table);
    if joined == "id" {
        return ("PRIMARY_KEY_ID", 0.95, "Primary key identifier".into());
    }
    let last = tokens.last().unwrap();
    if last == "id" {
        if joined == format!("{ts}_id") || (tokens.len() == 2 && tokens[0] == ts) {
            return ("PRIMARY_KEY_ID", 0.9, "Primary key identifier".into());
        }
        return ("FOREIGN_KEY_REF", 0.9, "Foreign key reference".into());
    }
    if ["key", "code", "no", "num", "number"].contains(&last.as_str()) {
        return ("FOREIGN_KEY_REF", 0.75, "Foreign key reference".into());
    }
    for rule in NAME_RULES {
        let token_hit = rule.tokens.iter().any(|t| {
            if t.contains('_') {
                joined == *t || joined.ends_with(&format!("_{t}"))
            } else {
                tokens.iter().any(|x| x == t)
            }
        });
        let suffix_hit = rule.suffixes.iter().any(|s| joined.ends_with(s));
        let prefix_hit = rule.prefixes.iter().any(|p| joined.starts_with(p));
        if token_hit || suffix_hit || prefix_hit {
            if rule.kind == "PERSON_NAME" && tokens.iter().any(|t| PERSON_EXCLUDES.contains(&t.as_str())) {
                return ("PRODUCT_ITEM", 0.8, "Product or item title".into());
            }
            return (rule.kind, rule.confidence, rule.description.into());
        }
    }
    ("GENERAL_TEXT", 0.4, "Text attribute".into())
}

fn can_override(name_type: &str, value_type: &str) -> bool {
    const SUBSUMED: [&str; 7] = [
        "PRIMARY_KEY_ID",
        "FOREIGN_KEY_REF",
        "CURRENCY_PRICE",
        "NUMERIC_METRIC",
        "DATE_TIMESTAMP",
        "UUID",
        "BOOLEAN_FLAG",
    ];
    if value_type == "NUMERIC_METRIC" {
        return !SUBSUMED.contains(&name_type);
    }
    if value_type == "CATEGORY_TYPE" {
        return name_type == "GENERAL_TEXT";
    }
    if name_type == "CATEGORY_TYPE" || name_type == "BOOLEAN_FLAG" {
        return false;
    }
    true
}

pub struct ColumnStats {
    pub samples: Vec<String>,
    pub filled: usize,
    pub distinct: usize,
}

pub fn classify_column(table_name: &str, column_name: &str, column_index: i64, stats: &ColumnStats) -> EntityTag {
    let (mut kind, mut conf0, mut desc) = name_class(table_name, column_name);
    let mut confidence = conf0;
    for (vt, ratio) in value_evidence(&stats.samples, stats.filled, stats.distinct) {
        if vt == kind {
            confidence = (confidence + 0.35).min(0.99);
            continue;
        }
        if !can_override(kind, vt) {
            continue;
        }
        let fused = conf0 * 0.6 + ratio * 0.4;
        let strong = ratio >= 0.9 && conf0 < 0.8;
        if fused > confidence || strong {
            kind = vt;
            conf0 = fused.max(ratio.min(0.97));
            desc = NAME_RULES
                .iter()
                .find(|r| r.kind == vt)
                .map(|r| r.description.to_string())
                .unwrap_or_else(|| vt.to_string());
            confidence = conf0;
        }
    }
    let high_card = stats.distinct > 0 && stats.distinct as f64 >= (stats.filled.max(1)) as f64 * 0.95;
    if column_index == 0
        && high_card
        && ["FOREIGN_KEY_REF", "NUMERIC_METRIC", "GENERAL_TEXT", "CATEGORY_TYPE"].contains(&kind)
    {
        kind = "PRIMARY_KEY_ID";
        confidence = confidence.max(0.85);
        desc = "Primary key identifier".into();
    }
    if kind == "FOREIGN_KEY_REF" && high_card {
        let tokens = profiler_tokenize(column_name);
        let ts = profiler_singularize(table_name);
        if tokens.last().map(|s| s.as_str()) == Some("id") {
            let stem = if tokens.len() >= 2 {
                tokens[tokens.len() - 2].clone()
            } else {
                String::new()
            };
            if stem == ts || tokens.len() == 1 {
                kind = "PRIMARY_KEY_ID";
                confidence = confidence.max(0.88);
                desc = "Primary key identifier".into();
            }
        }
    }
    EntityTag {
        column_name: column_name.to_string(),
        table_name: table_name.to_string(),
        entity_type: kind.to_string(),
        confidence: js::number(&js::to_fixed(confidence.clamp(0.05, 0.99), 3)),
        sample_values: stats.samples.iter().take(5).cloned().collect(),
        description: desc,
    }
}

/// `RelationDetectorService.performNER` for one table.
pub fn classify_table(table_name: &str, headers: &[String], rows: &Table) -> Vec<EntityTag> {
    let lower = table_name.to_lowercase();
    headers
        .iter()
        .enumerate()
        .map(|(ci, h)| {
            let mut samples = Vec::new();
            let mut seen = HashSet::new();
            let mut filled = 0;
            let limit = rows.len().min(2000);
            let col = rows.col(h);
            for r in 0..limit {
                if samples.len() >= 24 {
                    break;
                }
                let raw = rows.get_idx(r, col);
                if raw.is_nullish() || js::trim(&raw.js_string()).is_empty() {
                    continue;
                }
                filled += 1;
                let v = js::trim(&raw.js_string()).to_string();
                if seen.insert(v.clone()) {
                    samples.push(v);
                }
            }
            let distinct = if filled > 0 && seen.len() as f64 / filled as f64 > 0.9 {
                rows.len()
            } else {
                seen.len()
            };
            classify_column(
                &lower,
                h,
                ci as i64,
                &ColumnStats {
                    samples,
                    filled,
                    distinct,
                },
            )
        })
        .collect()
}

/// `classifyColumnEntity`.
pub fn classify_entity(table_name: &str, column: &str, values: &[crate::table::Cell]) -> EntityTag {
    let samples: Vec<String> = values
        .iter()
        .filter(|v| !v.is_nullish() && !js::trim(&v.js_string()).is_empty())
        .map(|v| js::trim(&v.js_string()).to_string())
        .collect();
    let distinct = samples.iter().collect::<HashSet<_>>().len();
    let filled = samples.len();
    classify_column(
        &table_name.to_lowercase(),
        column,
        -1,
        &ColumnStats {
            samples,
            filled,
            distinct,
        },
    )
}
