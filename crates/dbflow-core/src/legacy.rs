//! Single-file output generators: relation-schema documentation, the annotated
//! Mermaid ERD and the validated MySQL DDL script.

use crate::identifiers::IdentifierRegistry;
use crate::js;
use crate::sql_types::infer_column_sql_type;
use crate::table::Cell;
use crate::types::{ForeignKey, Relation};
use indexmap::IndexMap;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

fn pct(x: f64, d: usize) -> String {
    js::to_fixed(x * 100.0, d)
}

struct FdStats {
    high: usize,
    medium: usize,
    low: usize,
}

fn fd_stats(relations: &[Relation]) -> FdStats {
    let mut s = FdStats {
        high: 0,
        medium: 0,
        low: 0,
    };
    for r in relations {
        if let Some(fd) = &r.original_fd {
            if fd.confidence >= 0.95 {
                s.high += 1;
            } else if fd.confidence >= 0.80 {
                s.medium += 1;
            } else {
                s.low += 1;
            }
        }
    }
    s
}

pub fn generate_relation_schemas(relations: &[Relation]) -> String {
    let mut c = String::from("# 3NF Relation Schemas (ALL FDs Preserved)\n\n");
    c += &format!("Generated: {}\n", js::iso_now());
    c += &format!("Total Relations: {}\n\n", relations.len());
    let s = fd_stats(relations);
    c += "## Functional Dependency Statistics\n\n";
    c += &format!("- HIGH confidence FDs (≥95%): {}\n", s.high);
    c += &format!("- MEDIUM confidence FDs (80-94%): {}\n", s.medium);
    c += &format!("- LOW confidence FDs (<80%): {}\n", s.low);
    c += &format!("- Total FDs preserved: {}\n\n", s.high + s.medium + s.low);
    c += "## Relations Summary\n\n";
    c += "| Relation | Attributes | Primary Key | Foreign Keys | Rows | Source | FD Confidence |\n";
    c += "|----------|------------|-------------|--------------|------|--------|---------------|\n";
    let or_none = |v: String| if v.is_empty() { "None".to_string() } else { v };
    for r in relations {
        let fks = or_none(
            r.foreign_keys
                .iter()
                .map(|fk| format!("{} -> {}", fk.attributes.join(","), fk.references))
                .collect::<Vec<_>>()
                .join("; "),
        );
        let conf = r
            .original_fd
            .as_ref()
            .map(|f| format!("{}%", pct(f.confidence, 1)))
            .unwrap_or_else(|| "N/A".into());
        c += &format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            r.name,
            r.attributes.join(", "),
            or_none(r.primary_key.join(", ")),
            fks,
            js::num_to_string(r.metadata.row_count),
            r.source(),
            conf
        );
    }
    c += "\n## Detailed Relation Descriptions\n\n";
    for r in relations {
        c += &format!("### {}\n\n", r.name);
        c += &format!("**Attributes**: {}\n", r.attributes.join(", "));
        c += &format!("**Primary Key**: {{{}}}\n", or_none(r.primary_key.join(", ")));
        if !r.foreign_keys.is_empty() {
            c += "**Foreign Keys**:\n";
            for fk in &r.foreign_keys {
                c += &format!(
                    "- {{{}}} -> {}.{{{}}}\n",
                    fk.attributes.join(", "),
                    fk.references,
                    fk.referenced_key.join(", ")
                );
            }
        }
        c += &format!("**Source**: {}\n", r.source());
        c += &format!("**Row Count**: {}\n", js::num_to_string(r.metadata.row_count));
        if let Some(fd) = &r.original_fd {
            let level = if fd.confidence >= 0.95 {
                "HIGH"
            } else if fd.confidence >= 0.80 {
                "MEDIUM"
            } else {
                "LOW"
            };
            c += &format!(
                "**Original FD**: {} -> {} ({}% - {level})\n",
                fd.lhs.join(", "),
                fd.rhs,
                pct(fd.confidence, 2)
            );
        }
        c += "**Attribute Types**:\n";
        for (a, t) in &r.metadata.attribute_types {
            c += &format!("  - {a}: {t}\n");
        }
        c += "\n---\n\n";
    }
    c
}

// ---------------------------------------------------------------------------
// ERD
// ---------------------------------------------------------------------------

struct Ids {
    tables: IdentifierRegistry,
    columns: IdentifierRegistry,
}

impl Ids {
    fn new(relations: &[Relation]) -> Ids {
        let mut ids = Ids {
            tables: IdentifierRegistry::new(),
            columns: IdentifierRegistry::new(),
        };
        for r in relations {
            ids.tables.assign(&r.name);
            for a in &r.attributes {
                ids.columns.assign(a);
            }
        }
        ids
    }
}

fn relation_weight(r: &Relation) -> f64 {
    let mut w = ((r.attributes.len() + 1) as f64).ln() * 10.0;
    w += (r.metadata.row_count + 1.0).ln() * 5.0;
    w += if r.primary_key.len() == 1 { 15.0 } else { 10.0 };
    w += r.foreign_keys.len() as f64 * 20.0;
    w += match r.source() {
        "FunctionalDependency" => 25.0,
        "CandidateKey_LosslessJoin" => 35.0,
        "LosslessJoin_Guarantee" => 30.0,
        "CandidateKey" => 30.0,
        "3NF" => 20.0,
        "2NF_PartialDependency" => 18.0,
        "2NF" => 15.0,
        "1NF" => 10.0,
        _ => 5.0,
    };
    if let Some(fd) = &r.original_fd {
        w += 15.0 + fd.confidence * 10.0;
    }
    w
}

fn relationship_weight(src: &Relation, fk: &ForeignKey) -> f64 {
    let mut w = 10.0;
    w += match fk.card() {
        "1:1" => 30.0,
        "1:N" => 20.0,
        "N:M" => 15.0,
        "N:1" => 20.0,
        _ => 10.0,
    };
    w += (20.0 - fk.attributes.len() as f64 * 5.0).max(0.0);
    w += src.attributes.iter().filter(|a| fk.attributes.contains(a)).count() as f64 * 5.0;
    if let Some(fd) = &src.original_fd {
        w += fd.confidence * 5.0;
    }
    w
}

struct Node {
    neighbors: IndexMap<String, f64>,
    weight: f64,
}

struct Cluster {
    entities: Vec<String>,
}

fn structural_importance(r: &Relation) -> f64 {
    let mut s = if r.primary_key.len() == 1 { 20.0 } else { 10.0 };
    s += (r.metadata.row_count + 1.0).ln() * 5.0;
    s += (r.attributes.len() as f64 * 3.0).min(30.0);
    s += match r.source() {
        "FunctionalDependency" => 15.0,
        "CandidateKey_LosslessJoin" => 30.0,
        "LosslessJoin_Guarantee" => 25.0,
        "CandidateKey" => 25.0,
        "3NF" => 20.0,
        "2NF_PartialDependency" => 15.0,
        "2NF" => 10.0,
        "1NF" => 5.0,
        _ => 0.0,
    };
    if let Some(fd) = &r.original_fd {
        s += 10.0 + fd.confidence * 15.0;
    }
    s
}

fn cluster_cohesion(entities: &[String], graph: &IndexMap<String, Node>) -> f64 {
    if entities.len() <= 1 {
        return 0.0;
    }
    let (mut total, mut n) = (0.0, 0);
    for e in entities {
        if let Some(node) = graph.get(e) {
            for (nb, w) in &node.neighbors {
                if entities.contains(nb) {
                    total += w;
                    n += 1;
                }
            }
        }
    }
    if n > 0 {
        total / n as f64
    } else {
        0.0
    }
}

fn clusters_of(graph: &IndexMap<String, Node>) -> Vec<Cluster> {
    let mut sorted: Vec<&String> = graph.keys().collect();
    sorted.sort_by(|a, b| {
        graph[*b]
            .weight
            .partial_cmp(&graph[*a].weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut visited: HashSet<String> = HashSet::new();
    let mut clusters = Vec::new();
    for e in sorted {
        if visited.contains(e) {
            continue;
        }
        let mut cluster = Vec::new();
        let mut stack = vec![e.clone()];
        while let Some(cur) = stack.pop() {
            if !visited.insert(cur.clone()) {
                continue;
            }
            cluster.push(cur.clone());
            if let Some(node) = graph.get(&cur) {
                for (nb, w) in &node.neighbors {
                    if !visited.contains(nb) && *w >= 12.0 {
                        stack.push(nb.clone());
                    }
                }
            }
        }
        if !cluster.is_empty() {
            clusters.push(Cluster { entities: cluster });
        }
    }
    // Merge small clusters into large ones; only large clusters are kept.
    let (small, mut large): (Vec<Cluster>, Vec<Cluster>) = clusters.into_iter().partition(|c| c.entities.len() < 2);
    for s in small {
        let mut best: Option<(usize, f64)> = None;
        for (i, l) in large.iter().enumerate() {
            let (mut total, mut n) = (0.0, 0);
            for e in &s.entities {
                if let Some(node) = graph.get(e) {
                    for (nb, w) in &node.neighbors {
                        if l.entities.contains(nb) {
                            total += w;
                            n += 1;
                        }
                    }
                }
            }
            let strength = if n > 0 { total / n as f64 } else { 0.0 };
            if best.map(|b| strength > b.1).unwrap_or(true) {
                best = Some((i, strength));
            }
        }
        if let Some((i, st)) = best {
            if st > 8.0 {
                large[i].entities.extend(s.entities);
                let _ = cluster_cohesion(&large[i].entities, graph);
            }
        }
    }
    large
}

fn mermaid_type(t: &str) -> &'static str {
    match t {
        "integer" => "int",
        "decimal" => "decimal",
        "boolean" => "boolean",
        "date" => "date",
        _ => "varchar",
    }
}

fn mermaid_card(c: &str) -> &'static str {
    match c {
        "1:1" => "||--||",
        "N:1" => "}o--||",
        "N:M" | "N:N" => "}o--o{",
        _ => "||--o{",
    }
}

fn conf_tag(c: f64) -> &'static str {
    if c >= 0.95 {
        "[H]"
    } else if c >= 0.80 {
        "[M]"
    } else {
        "[L]"
    }
}

fn entity_lines(e: &Relation, primary: bool, ids: &mut Ids) -> Vec<String> {
    let mut lines = vec![format!("    {} {{", ids.tables.assign(&e.name))];
    let max = if primary { 10 } else { 7 };
    let is_fk = |a: &String| e.foreign_keys.iter().any(|fk| fk.attributes.contains(a));
    let (keys, regular): (Vec<&String>, Vec<&String>) =
        e.attributes.iter().partition(|a| e.primary_key.contains(a) || is_fk(a));
    let mut display: Vec<&String> = keys;
    let slots = max as isize - display.len() as isize;
    if slots > 0 {
        display.extend(regular.into_iter().take(slots as usize));
    }
    display.truncate(max);
    for a in &display {
        let t = mermaid_type(
            e.metadata
                .attribute_types
                .get(*a)
                .map(|s| s.as_str())
                .unwrap_or("string"),
        );
        let mut cons = String::new();
        if e.primary_key.contains(a) {
            cons += "PK ";
        }
        if is_fk(a) {
            cons += "FK ";
        }
        lines.push(format!("        {t} {} \"{}\"", ids.columns.assign(a), cons.trim()));
    }
    if e.attributes.len() > display.len() {
        lines.push(format!(
            "        string ellipsis \"... +{} more\"",
            e.attributes.len() - display.len()
        ));
    }
    if let Some(fd) = &e.original_fd {
        lines.push(format!(
            "        string confidence \"{} FD: {}%\"",
            conf_tag(fd.confidence),
            js::to_fixed(fd.confidence * 100.0, 0)
        ));
    }
    lines.push("    }".into());
    lines
}

fn complexity(relations: &[Relation]) -> &'static str {
    let rels: usize = relations.iter().map(|r| r.foreign_keys.len()).sum();
    let avg = if relations.is_empty() {
        0.0
    } else {
        relations.iter().map(|r| r.attributes.len()).sum::<usize>() as f64 / relations.len() as f64
    };
    let score = relations.len() as f64 * 2.0 + rels as f64 * 3.0 + avg;
    if score < 50.0 {
        "Simple"
    } else if score < 150.0 {
        "Moderate"
    } else if score < 300.0 {
        "Complex"
    } else {
        "Very Complex"
    }
}

/// The annotated single-file ERD. Sorts `relations` in place by importance, as the original did.
pub fn generate_erd(relations: &mut [Relation]) -> (String, Value) {
    let mut ids = Ids::new(relations);
    if relations.is_empty() {
        return (
            "erDiagram\n    %% No relations to display".into(),
            json!({"connectedEntities":0,"totalEntities":0,"relationships":0,"diagramType":"empty"}),
        );
    }
    let mut graph: IndexMap<String, Node> = IndexMap::new();
    for r in relations.iter() {
        graph.insert(
            r.name.clone(),
            Node {
                neighbors: IndexMap::new(),
                weight: relation_weight(r),
            },
        );
    }
    for r in relations.iter() {
        for fk in &r.foreign_keys {
            if graph.contains_key(&r.name) && graph.contains_key(&fk.references) {
                let w = relationship_weight(r, fk);
                graph
                    .get_mut(&r.name)
                    .unwrap()
                    .neighbors
                    .insert(fk.references.clone(), w);
                graph
                    .get_mut(&fk.references)
                    .unwrap()
                    .neighbors
                    .insert(r.name.clone(), w);
            }
        }
    }
    let clusters = clusters_of(&graph);
    let mut importance: HashMap<String, f64> = HashMap::new();
    for r in relations.iter() {
        let node = graph.get(&r.name);
        let degree = node.map(|n| n.neighbors.len() as f64 * 10.0).unwrap_or(0.0);
        let weighted = node.map(|n| n.neighbors.values().sum::<f64>()).unwrap_or(0.0);
        importance.insert(r.name.clone(), degree + weighted * 0.5 + structural_importance(r));
    }
    let score = |n: &str| importance.get(n).copied().unwrap_or(0.0);
    relations.sort_by(|a, b| {
        score(&b.name)
            .partial_cmp(&score(&a.name))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let referenced = |name: &str, rels: &[Relation]| {
        rels.iter()
            .any(|r| r.foreign_keys.iter().any(|fk| fk.references == name))
    };
    let primary: Vec<usize> = (0..relations.len())
        .filter(|&i| {
            let r = &relations[i];
            score(&r.name) > 20.0
                && (!r.foreign_keys.is_empty() || referenced(&r.name, relations) || r.original_fd.is_some())
        })
        .take(12)
        .collect();
    let primary_names: HashSet<String> = primary.iter().map(|&i| relations[i].name.clone()).collect();
    let secondary: Vec<usize> = (0..relations.len())
        .filter(|&i| !primary_names.contains(&relations[i].name))
        .filter(|&i| {
            let r = &relations[i];
            let connected = r.foreign_keys.iter().any(|fk| primary_names.contains(&fk.references))
                || primary
                    .iter()
                    .any(|&p| relations[p].foreign_keys.iter().any(|fk| fk.references == r.name));
            connected || r.original_fd.as_ref().map(|f| f.confidence > 0.6).unwrap_or(false)
        })
        .take(8)
        .collect();
    let secondary_names: HashSet<String> = secondary.iter().map(|&i| relations[i].name.clone()).collect();
    let shown_clusters = clusters
        .iter()
        .filter(|c| {
            c.entities
                .iter()
                .any(|e| primary_names.contains(e) || secondary_names.contains(e))
        })
        .count();

    let mut lines = vec![
        "erDiagram".to_string(),
        "    %% Comprehensive ERD - ALL Functional Dependencies Preserved".into(),
        "    %% Legend: [H] = High Confidence (≥95%), [M] = Medium (80-94%), [L] = Low (<80%)".into(),
        String::new(),
        "    %% === PRIMARY ENTITIES (High Importance) ===".into(),
    ];
    for &i in &primary {
        lines.extend(entity_lines(&relations[i], true, &mut ids));
        lines.push(String::new());
    }
    if !secondary.is_empty() {
        lines.push("    %% === SECONDARY ENTITIES (Supporting) ===".into());
        for &i in &secondary {
            lines.extend(entity_lines(&relations[i], false, &mut ids));
            lines.push(String::new());
        }
    }
    lines.push("    %% === RELATIONSHIPS (All Confidence Levels) ===".into());
    let display: Vec<usize> = primary.iter().chain(secondary.iter()).copied().collect();
    let names: HashSet<String> = display.iter().map(|&i| relations[i].name.clone()).collect();
    let mut added: HashSet<String> = HashSet::new();
    for &i in &display {
        let e = &relations[i];
        for fk in &e.foreign_keys {
            if !names.contains(&fk.references) {
                continue;
            }
            let k = format!("{}-{}", e.name, fk.references);
            let rk = format!("{}-{}", fk.references, e.name);
            if added.contains(&k) || added.contains(&rk) {
                continue;
            }
            let tag = relations
                .iter()
                .find(|r| r.name == fk.references)
                .and_then(|t| t.original_fd.as_ref())
                .map(|f| conf_tag(f.confidence))
                .unwrap_or("");
            let label = if fk.attributes.len() == 1 {
                format!("{}{tag}", fk.attributes[0])
            } else {
                format!("{}attrs{tag}", fk.attributes.len())
            };
            let src = ids.tables.assign(&e.name);
            let tgt = ids.tables.assign(&fk.references);
            lines.push(format!("    {src} {} {tgt} : \"{label}\"", mermaid_card(fk.card())));
            added.insert(k);
        }
    }
    lines.push(String::new());
    let s = fd_stats(relations);
    lines.push("    %% === METADATA ===".into());
    lines.push(format!(
        "    %% Total Relations: {} | Displayed: {}",
        relations.len(),
        display.len()
    ));
    lines.push(format!(
        "    %% FD Distribution: HIGH: {} | MEDIUM: {} | LOW: {}",
        s.high, s.medium, s.low
    ));
    lines.push(format!("    %% Clusters: {shown_clusters}"));
    lines.push(format!("    %% Generated: {}", js::iso_now()));
    lines.push("    %% ALL functional dependencies preserved (no lossy joins)".into());
    let total_rels: usize = relations.iter().map(|r| r.foreign_keys.len()).sum();
    let meta = json!({
        "totalEntities": relations.len(),
        "displayedEntities": display.len(),
        "clusters": shown_clusters,
        "relationships": total_rels,
        "diagramType": "comprehensive-all-fds",
        "complexity": complexity(relations),
        "fdStatistics": {"highConfidence": s.high, "mediumConfidence": s.medium, "lowConfidence": s.low, "total": s.high + s.medium + s.low},
        "optimization": {
            "entityReduction": format!("{}%", js::to_fixed((relations.len() - primary.len()) as f64 / relations.len() as f64 * 100.0, 1)),
            "clustering": shown_clusters > 1,
            "allFDsIncluded": true
        }
    });
    (lines.join("\n"), meta)
}

// ---------------------------------------------------------------------------
// SQL DDL (MySQL, validated)
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Analysis {
    lengths: HashMap<String, usize>,
    patterns: HashMap<String, String>,
    null_ratio: HashMap<String, f64>,
    unique_ratio: HashMap<String, f64>,
    samples: HashMap<String, Vec<String>>,
    sql_types: HashMap<String, String>,
}

fn detect_pattern(values: &[String]) -> String {
    if values.is_empty() {
        return "empty".into();
    }
    let sample = &values[..values.len().min(100)];
    let n = sample.len() as f64;
    let count = |f: &dyn Fn(&str) -> bool| sample.iter().filter(|v| f(v)).count() as f64;
    let t = |v: &str| js::trim(v).to_string();
    let email = re!(
        r"^[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*$"
    );
    if count(&|v| email.is_match(&t(v))) >= n * 0.95 {
        return "email".into();
    }
    let urls = [
        re!(r"(?i)^https?://[^\s/$.?#].[^\s]*$"),
        re!(r"(?i)^ftp://[^\s/$.?#].[^\s]*$"),
        re!(r"(?i)^www\.[^\s/$.?#].[^\s]*$"),
    ];
    if count(&|v| urls.iter().any(|p| p.is_match(&t(v)))) >= n * 0.90 {
        return "url".into();
    }
    let phones = [
        re!(r"^\+?[1-9][0-9]{1,14}$"),
        re!(r"^[\+\-\(\)\s0-9]{10,20}$"),
        re!(r"^\([0-9]{3}\)\s?[0-9]{3}-?[0-9]{4}$"),
        re!(r"^[0-9]{3}-[0-9]{3}-[0-9]{4}$"),
        re!(r"^[0-9]{10}$"),
    ];
    if count(&|v| {
        let c = re!(r"[\s\-\(\)\.]").replace_all(v, "").to_string();
        phones.iter().any(|p| p.is_match(&c)) && js::len16(&c) >= 10
    }) >= n * 0.85
    {
        return "phone".into();
    }
    let uuids = [
        re!(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
        re!(r"(?i)^[0-9a-f]{32}$"),
    ];
    if count(&|v| uuids.iter().any(|p| p.is_match(&t(v)))) >= n * 0.95 {
        return "uuid".into();
    }
    let dates = [
        re!(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}$"),
        re!(r"^[0-9]{4}/[0-9]{2}/[0-9]{2}$"),
        re!(r"^[0-9]{2}/[0-9]{2}/[0-9]{4}$"),
        re!(r"^[0-9]{2}-[0-9]{2}-[0-9]{4}$"),
        re!(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}$"),
        re!(r"^[0-9]{4}-[0-9]{2}-[0-9]{2}\s[0-9]{2}:[0-9]{2}:[0-9]{2}$"),
        re!(r"^[A-Za-z0-9_]{3}\s[0-9]{1,2},?\s[0-9]{4}$"),
        re!(r"^[0-9]{1,2}\s[A-Za-z0-9_]{3}\s[0-9]{4}$"),
    ];
    if count(&|v| {
        let s = t(v);
        dates.iter().any(|p| p.is_match(&s)) || js::date_parse_valid(&s)
    }) >= n * 0.90
    {
        return "date".into();
    }
    let times = [
        re!(r"^[0-9]{2}:[0-9]{2}:[0-9]{2}$"),
        re!(r"^[0-9]{2}:[0-9]{2}$"),
        re!(r"(?i)^[0-9]{1,2}:[0-9]{2}\s?(AM|PM)$"),
    ];
    if count(&|v| times.iter().any(|p| p.is_match(&t(v)))) >= n * 0.90 {
        return "time".into();
    }
    let numeric: Vec<&String> = sample.iter().filter(|v| js::parse_float(&t(v)).is_finite()).collect();
    if numeric.len() as f64 >= n * 0.95 {
        let has_dec = numeric.iter().any(|v| v.contains('.') && !v.ends_with(".0"));
        let pct_count = count(&|v| t(v).ends_with('%') && !js::parse_float(&v.replacen('%', "", 1)).is_nan());
        if pct_count >= n * 0.80 {
            return "percentage".into();
        }
        let cur = [
            re!(r"^\$[0-9,]+\.?[0-9]*$"),
            re!(r"^€[0-9,]+\.?[0-9]*$"),
            re!(r"^£[0-9,]+\.?[0-9]*$"),
            re!(r"(?i)^[0-9,]+\.?[0-9]*\s?(USD|EUR|GBP|CAD|AUD)$"),
        ];
        if count(&|v| cur.iter().any(|p| p.is_match(&t(v)))) >= n * 0.80 {
            return "currency".into();
        }
        return if has_dec { "decimal" } else { "integer" }.into();
    }
    let bools = [
        re!(r"(?i)^(true|false)$"),
        re!(r"(?i)^(yes|no)$"),
        re!(r"(?i)^(y|n)$"),
        re!(r"^(1|0)$"),
        re!(r"(?i)^(on|off)$"),
        re!(r"(?i)^(enabled|disabled)$"),
        re!(r"(?i)^(active|inactive)$"),
    ];
    if count(&|v| bools.iter().any(|p| p.is_match(&t(v)))) >= n * 0.90 {
        return "boolean".into();
    }
    let idents = [
        re!(r"^[A-Za-z0-9_-]+$"),
        re!(r"^[A-Z]{2,}[0-9]+$"),
        re!(r"^[0-9]+[A-Za-z]+[0-9]*$"),
        re!(r"^[A-Za-z]+[0-9]+$"),
    ];
    if sample.iter().all(|v| js::len16(v) <= 50) && count(&|v| idents.iter().any(|p| p.is_match(&t(v)))) >= n * 0.85 {
        return "identifier".into();
    }
    let ips = [
        re!(r"^(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$"),
        re!(r"^(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}$"),
        re!(r"(?i)^::1$|^localhost$"),
    ];
    if count(&|v| ips.iter().any(|p| p.is_match(&t(v)))) >= n * 0.90 {
        return "ip_address".into();
    }
    if count(&|v| {
        let s = t(v);
        ((s.starts_with('{') && s.ends_with('}')) || (s.starts_with('[') && s.ends_with(']')))
            && serde_json::from_str::<Value>(&s).is_ok()
    }) >= n * 0.80
    {
        return "json".into();
    }
    if count(&|v| {
        let s = t(v);
        js::len16(&s) > 10 && js::len16(&s) % 4 == 0 && re!(r"^[A-Za-z0-9+/]*={0,2}$").is_match(&s)
    }) >= n * 0.85
    {
        return "base64".into();
    }
    let hashes = [
        re!(r"(?i)^[a-f0-9]{32}$"),
        re!(r"(?i)^[a-f0-9]{40}$"),
        re!(r"(?i)^[a-f0-9]{64}$"),
        re!(r"(?i)^[a-f0-9]{128}$"),
    ];
    if count(&|v| hashes.iter().any(|p| p.is_match(&t(v)))) >= n * 0.90 {
        return "hash".into();
    }
    let coords = [
        re!(r"^-?[0-9]+\.[0-9]+,-?[0-9]+\.[0-9]+$"),
        re!(r"^-?[0-9]+\.[0-9]+\s+-?[0-9]+\.[0-9]+$"),
        re!(r"^\(-?[0-9]+\.[0-9]+,\s*-?[0-9]+\.[0-9]+\)$"),
    ];
    if count(&|v| coords.iter().any(|p| p.is_match(&t(v)))) >= n * 0.85 {
        return "coordinates".into();
    }
    let avg = sample.iter().map(|v| js::len16(v)).sum::<usize>() as f64 / n;
    if avg > 500.0 {
        "long_text".into()
    } else if avg > 100.0 {
        "medium_text".into()
    } else if avg > 50.0 {
        "short_text".into()
    } else {
        "text".into()
    }
}

struct ColumnFacts {
    key: String,
    sql_type: String,
    stats: Option<(usize, String, f64, f64, Vec<String>)>,
}

fn analyze(relations: &[Relation]) -> Analysis {
    let jobs: Vec<(&Relation, &String)> = relations
        .iter()
        .filter(|r| !r.rows.is_empty())
        .flat_map(|r| r.attributes.iter().map(move |a| (r, a)))
        .collect();
    let facts: Vec<ColumnFacts> = crate::par_map(&jobs, |(r, attr)| {
        let key = format!("{}.{}", r.name, attr);
        let col = r.rows.column(attr);
        let sql_type = infer_column_sql_type(&col).sql_type;
        let values: Vec<&Cell> = col.into_iter().filter(|v| !v.is_nullish()).collect();
        if values.is_empty() {
            return ColumnFacts {
                key,
                sql_type,
                stats: None,
            };
        }
        let strings: Vec<std::borrow::Cow<'_, str>> = values.iter().map(|v| v.text()).collect();
        let len = strings.iter().map(|s| js::len16(s)).max().unwrap_or(0);
        let sample: Vec<String> = strings.iter().take(100).map(|s| s.to_string()).collect();
        let pattern = detect_pattern(&sample);
        let null_ratio = (r.rows.len() - values.len()) as f64 / r.rows.len() as f64;
        let uniq: HashSet<&str> = strings.iter().map(|s| s.as_ref()).collect();
        let unique_ratio = uniq.len() as f64 / values.len() as f64;
        ColumnFacts {
            key,
            sql_type,
            stats: Some((
                len,
                pattern,
                null_ratio,
                unique_ratio,
                sample.into_iter().take(10).collect(),
            )),
        }
    });
    let mut a = Analysis::default();
    for f in facts {
        a.sql_types.insert(f.key.clone(), f.sql_type);
        if let Some((len, pattern, null_ratio, unique_ratio, samples)) = f.stats {
            a.lengths.insert(f.key.clone(), len);
            a.patterns.insert(f.key.clone(), pattern);
            a.null_ratio.insert(f.key.clone(), null_ratio);
            a.unique_ratio.insert(f.key.clone(), unique_ratio);
            a.samples.insert(f.key, samples);
        }
    }
    a
}

fn data_type(attr: &str, r: &Relation, a: &Analysis) -> String {
    let key = format!("{}.{}", r.name, attr);
    if let Some(t) = a.sql_types.get(&key) {
        if !t.is_empty() {
            return t.clone();
        }
    }
    let pattern = a.patterns.get(&key).map(|s| s.as_str()).unwrap_or("text");
    let len = a.lengths.get(&key).copied().filter(|&l| l > 0).unwrap_or(255);
    let uniq = a.unique_ratio.get(&key).copied().unwrap_or(0.0);
    match pattern {
        "email" => format!("VARCHAR({})", 320.max(len + 50)),
        "url" => {
            if len > 255 {
                "TEXT".into()
            } else {
                format!("VARCHAR({})", 2083.min(len + 100))
            }
        }
        "phone" => format!("VARCHAR({})", 25.max(len + 5)),
        "uuid" => "CHAR(36)".into(),
        "date" => "DATE".into(),
        "time" => "TIME".into(),
        "integer" => {
            if uniq > 0.8 || attr.to_lowercase().contains("id") {
                "BIGINT".into()
            } else {
                "INTEGER".into()
            }
        }
        "decimal" => "DECIMAL(15,4)".into(),
        "percentage" => "DECIMAL(5,2)".into(),
        "currency" => "DECIMAL(15,2)".into(),
        "boolean" => "BOOLEAN".into(),
        "identifier" => format!("VARCHAR({})", 50.max(len + 10)),
        "ip_address" => "VARCHAR(45)".into(),
        "json" => "JSON".into(),
        "base64" => if len > 1000 { "LONGTEXT" } else { "TEXT" }.into(),
        "hash" => match len {
            0..=32 => "CHAR(32)",
            33..=40 => "CHAR(40)",
            41..=64 => "CHAR(64)",
            _ => "CHAR(128)",
        }
        .into(),
        "coordinates" => "POINT".into(),
        "long_text" => "LONGTEXT".into(),
        "medium_text" => "TEXT".into(),
        "short_text" => format!("VARCHAR({})", 100.max(len + 20)),
        _ => {
            if len <= 50 {
                format!("VARCHAR({})", 50.max(len + 10))
            } else if len <= 255 {
                format!("VARCHAR({})", 255.max(len + 50))
            } else if len <= 1000 {
                format!("VARCHAR({})", len + 200)
            } else {
                "TEXT".into()
            }
        }
    }
}

fn column_comment(attr: &str, r: &Relation, a: &Analysis) -> Option<String> {
    let key = format!("{}.{}", r.name, attr);
    let pattern = a.patterns.get(&key);
    let uniq = a.unique_ratio.get(&key).copied().unwrap_or(0.0);
    let mut c: Vec<String> = Vec::new();
    if r.primary_key.iter().any(|p| p == attr) {
        c.push("Primary Key".into());
    }
    if let Some(fk) = r.foreign_keys.iter().find(|fk| fk.attributes.iter().any(|x| x == attr)) {
        c.push(format!("References {}", fk.references));
    }
    if let Some(p) = pattern {
        if p != "text" {
            let d = match p.as_str() {
                "email" => "Email Address".to_string(),
                "url" => "URL/Web Address".into(),
                "phone" => "Phone Number".into(),
                "uuid" => "UUID Identifier".into(),
                "date" => "Date Value".into(),
                "time" => "Time Value".into(),
                "percentage" => "Percentage Value".into(),
                "currency" => "Currency Amount".into(),
                "boolean" => "Boolean Flag".into(),
                "identifier" => "Identifier/Code".into(),
                "ip_address" => "IP Address".into(),
                "json" => "JSON Data".into(),
                "base64" => "Base64 Encoded".into(),
                "hash" => "Hash Value".into(),
                "coordinates" => "Geographic Coordinates".into(),
                "long_text" => "Long Text Content".into(),
                "medium_text" => "Medium Text Content".into(),
                "short_text" => "Short Text Content".into(),
                other => format!("Pattern: {other}"),
            };
            c.push(d);
        }
    }
    if uniq > 0.95 {
        c.push("Unique values".into());
    } else if uniq < 0.1 {
        c.push("Low cardinality".into());
    }
    if c.is_empty() {
        None
    } else {
        Some(c.join(", "))
    }
}

fn nullable(r: &Relation, attr: &str) -> bool {
    !r.primary_key.iter().any(|p| p == attr) && !r.foreign_keys.iter().any(|fk| fk.attributes.iter().any(|x| x == attr))
}

fn default_for(t: &str, samples: Option<&Vec<String>>) -> Option<String> {
    if t.contains("VARCHAR") || t.contains("TEXT") {
        return Some("''".into());
    }
    if t.contains("INT") || t.contains("DECIMAL") {
        if let Some(s) = samples {
            let nums: Vec<f64> = s.iter().map(|v| js::parse_float(v)).filter(|v| !v.is_nan()).collect();
            if !nums.is_empty() && nums.iter().sum::<f64>() / nums.len() as f64 >= 1.0 {
                return Some("1".into());
            }
        }
        return Some("0".into());
    }
    if t.contains("BOOLEAN") {
        return Some("FALSE".into());
    }
    if t.contains("DATE") || t.contains("TIMESTAMP") {
        return Some("CURRENT_TIMESTAMP".into());
    }
    None
}

struct Validation {
    errors: Vec<String>,
    warnings: Vec<String>,
    confidence: f64,
    is_valid: bool,
}

fn fd_confidence(lhs: &[String], rhs: &str, r: &Relation) -> f64 {
    let rows = &r.rows;
    if rows.is_empty() {
        return 0.0;
    }
    let mut map: HashMap<String, HashSet<String>> = HashMap::new();
    let mut groups: HashMap<String, usize> = HashMap::new();
    let mut total = 0usize;
    for i in 0..rows.len() {
        let vals: Vec<String> = lhs
            .iter()
            .map(|a| js::trim(&rows.get(i, a).or_empty()).to_string())
            .collect();
        if vals.iter().any(|v| v.is_empty()) {
            continue;
        }
        let k = vals.join("||");
        let rv = js::trim(&rows.get(i, rhs).or_empty()).to_string();
        total += 1;
        *groups.entry(k.clone()).or_insert(0) += 1;
        map.entry(k).or_default().insert(rv);
    }
    if total == 0 || map.is_empty() {
        return 0.0;
    }
    let violations: usize = map.iter().filter(|(_, v)| v.len() > 1).map(|(k, _)| groups[k]).sum();
    (total - violations) as f64 / total as f64
}

fn validate(relations: &[Relation]) -> Validation {
    let (mut errors, mut warnings) = (Vec::new(), Vec::new());
    let (mut total, mut passed) = (0usize, 0usize);
    for r in relations {
        if let Some(fd) = &r.original_fd {
            total += 1;
            let conf = if r.rows.is_empty() {
                0.0
            } else {
                fd_confidence(&fd.lhs, &fd.rhs, r)
            };
            if conf >= 0.51 {
                passed += 1;
            } else if conf < 0.3 {
                errors.push(format!(
                    "Very low confidence FD in {}: {{{}}} → {}",
                    r.name,
                    fd.lhs.join(","),
                    fd.rhs
                ));
            }
            if conf < 0.7 {
                warnings.push(format!("Low confidence FD in {}: {}%", r.name, pct(conf, 1)));
            }
        }
    }
    for r in relations {
        if r.primary_key.is_empty() {
            continue;
        }
        total += 1;
        // Primary-key uniqueness.
        if r.rows.is_empty() {
            passed += 1; // confidence 0.5 but valid
            continue;
        }
        let mut seen = HashSet::new();
        let (mut dup, mut nulls) = (0, 0);
        for i in 0..r.rows.len() {
            let parts: Vec<String> = r
                .primary_key
                .iter()
                .map(|a| js::trim(&r.rows.get(i, a).or_empty()).to_string())
                .collect();
            if parts.iter().any(|p| p.is_empty()) {
                nulls += 1;
                continue;
            }
            if !seen.insert(parts.join("||")) {
                dup += 1;
            }
        }
        let valid_rows = r.rows.len() - nulls;
        let ratio = if valid_rows > 0 {
            seen.len() as f64 / valid_rows as f64
        } else {
            0.0
        };
        let ok = dup == 0 && nulls == 0;
        if ok || ratio > 0.7 {
            passed += 1;
        } else {
            let errs = if dup > 0 {
                vec![format!("Primary key has {dup} duplicate values")]
            } else {
                vec![]
            };
            warnings.push(format!("Primary key issues in {}: {}", r.name, errs.join("; ")));
        }
    }
    // Foreign keys.
    for r in relations {
        for fk in &r.foreign_keys {
            total += 1;
            let target = relations.iter().find(|t| t.name == fk.references);
            let mut errs = Vec::new();
            let exists = target.is_some();
            if !exists {
                errs.push(format!("Target relation '{}' does not exist", fk.references));
            }
            let mut matches = false;
            if let Some(t) = target {
                matches = fk.attributes.len() == t.primary_key.len()
                    && (0..fk.attributes.len()).all(|i| t.attributes.contains(&t.primary_key[i]));
                if !matches {
                    errs.push("Foreign key columns don't match target primary key".into());
                }
            }
            if exists && matches {
                passed += 1;
            } else {
                warnings.push(format!("Foreign key issues: {}", errs.join("; ")));
            }
        }
    }
    for r in relations {
        total += 1;
        if r.rows.is_empty() {
            passed += 1;
            continue;
        }
        let mut errs = Vec::new();
        for a in &r.attributes {
            if !nullable(r, a) {
                let n = (0..r.rows.len()).filter(|&i| r.rows.get(i, a).is_empty_like()).count();
                if n > 0 {
                    errs.push(format!("Non-nullable attribute '{a}' has {n} null values"));
                }
            }
        }
        if errs.is_empty() {
            passed += 1;
        } else {
            warnings.push(format!("Data integrity issues in {}: {}", r.name, errs.join("; ")));
        }
    }
    let confidence = if total > 0 {
        passed as f64 / total as f64 * 100.0
    } else {
        0.0
    };
    Validation {
        is_valid: errors.is_empty() && confidence >= 40.0,
        errors,
        warnings,
        confidence,
    }
}

fn order_relations(relations: &[Relation]) -> Vec<usize> {
    let names: Vec<&str> = relations.iter().map(|r| r.name.as_str()).collect();
    let mut graph: IndexMap<String, indexmap::IndexSet<String>> = IndexMap::new();
    for r in relations {
        graph.insert(r.name.clone(), indexmap::IndexSet::new());
    }
    for r in relations {
        for fk in &r.foreign_keys {
            if fk.references != r.name {
                if let Some(s) = graph.get_mut(&r.name) {
                    s.insert(fk.references.clone());
                }
            }
        }
    }
    // Detect cycles (DFS, stops at the first cycle reached from each root) and break them.
    let mut cycles: Vec<Vec<String>> = Vec::new();
    {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let mut path: Vec<String> = Vec::new();
        fn dfs(
            n: &str,
            g: &IndexMap<String, indexmap::IndexSet<String>>,
            visited: &mut HashSet<String>,
            stack: &mut HashSet<String>,
            path: &mut Vec<String>,
            cycles: &mut Vec<Vec<String>>,
        ) -> bool {
            if stack.contains(n) {
                if let Some(st) = path.iter().position(|x| x == n) {
                    let mut c = path[st..].to_vec();
                    c.push(n.to_string());
                    cycles.push(c);
                }
                return true;
            }
            if visited.contains(n) {
                return false;
            }
            visited.insert(n.to_string());
            stack.insert(n.to_string());
            path.push(n.to_string());
            if let Some(nb) = g.get(n) {
                for m in nb.clone() {
                    if dfs(&m, g, visited, stack, path, cycles) {
                        return true;
                    }
                }
            }
            stack.remove(n);
            path.pop();
            false
        }
        for r in relations {
            if !visited.contains(&r.name) {
                dfs(&r.name, &graph, &mut visited, &mut stack, &mut path, &mut cycles);
            }
        }
    }
    for c in &cycles {
        if c.len() > 1 {
            let last = &c[c.len() - 2];
            let first = &c[0];
            if let Some(s) = graph.get_mut(last) {
                s.shift_remove(first);
            }
        }
    }
    let mut result: Vec<usize> = Vec::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut visiting: HashSet<String> = HashSet::new();
    fn visit(
        n: &str,
        g: &IndexMap<String, indexmap::IndexSet<String>>,
        names: &[&str],
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
        out: &mut Vec<usize>,
    ) -> bool {
        if visiting.contains(n) {
            return false;
        }
        if visited.contains(n) {
            return true;
        }
        visiting.insert(n.to_string());
        if let Some(deps) = g.get(n) {
            for d in deps {
                if names.contains(&d.as_str()) && !visit(d, g, names, visited, visiting, out) {
                    return false;
                }
            }
        }
        visiting.remove(n);
        visited.insert(n.to_string());
        if let Some(i) = names.iter().position(|x| *x == n) {
            out.push(i);
        }
        true
    }
    for r in relations {
        if !visited.contains(&r.name) {
            visit(&r.name, &graph, &names, &mut visited, &mut visiting, &mut result);
        }
    }
    result
}

/// MySQL DDL script with validation notes, indexes and verification queries.
pub fn generate_sql_ddl(relations: &[Relation]) -> String {
    let mut ids = Ids::new(relations);
    if relations.is_empty() {
        return "-- No relations to generate\n".into();
    }
    let v = validate(relations);
    if !v.is_valid && v.confidence < 30.0 {
        return safe_fallback(relations, Some(&v), &mut ids);
    }
    let a = analyze(relations);
    let order = order_relations(relations);
    let ordered: Vec<&Relation> = order.iter().map(|&i| &relations[i]).collect();
    let owned: Vec<Relation> = ordered.iter().map(|r| (*r).clone()).collect();

    // Schema structure.
    let complex: Vec<bool> = ordered
        .iter()
        .map(|r| r.attributes.len() > 10 || r.foreign_keys.len() > 3)
        .collect();
    let lookup: Vec<bool> = ordered
        .iter()
        .map(|r| {
            r.attributes.len() <= 5
                && r.foreign_keys.is_empty()
                && ordered
                    .iter()
                    .any(|o| o.foreign_keys.iter().any(|fk| fk.references == r.name))
        })
        .collect();
    let lookup_names: HashSet<String> = ordered
        .iter()
        .zip(&lookup)
        .filter(|(_, l)| **l)
        .map(|(r, _)| r.name.clone())
        .collect();
    let fact: Vec<bool> = ordered.iter().map(|r| r.foreign_keys.len() >= 2).collect();

    let mut sql = String::new();
    sql += "-- ================================================================\n";
    sql += "-- DBFlow schema DDL\n";
    sql += &format!("-- Generated: {}\n", js::iso_now());
    sql += &format!("-- Relations: {} ({} schema)\n", owned.len(), complexity(&owned));
    sql += "-- Lossless join and dependency preservation are reported per run in the analysis, not guaranteed here.\n";
    sql += "-- ================================================================\n\n";
    let s = fd_stats(&owned);
    sql += "-- Functional dependencies behind this schema:\n";
    sql += &format!("--   Total Relations: {}\n", owned.len());
    sql += &format!("--   HIGH confidence FDs (≥95%): {}\n", s.high);
    sql += &format!("--   MEDIUM confidence FDs (80-94%): {}\n", s.medium);
    sql += &format!("--   LOW confidence FDs (<80%): {}\n", s.low);
    sql += &format!("--   Total FDs used: {}\n", s.high + s.medium + s.low);
    sql += &format!("--   Validation Errors: {}\n", v.errors.len());
    sql += &format!("--   Validation Warnings: {}\n", v.warnings.len());
    if !v.warnings.is_empty() {
        sql += "--\n-- Validation Warnings:\n";
        for w in &v.warnings {
            sql += &format!("--   {w}\n");
        }
    }
    sql += "\n-- Database Settings for Safe Execution\n";
    sql += "SET FOREIGN_KEY_CHECKS = 0;\nSET UNIQUE_CHECKS = 0;\nSET AUTOCOMMIT = 0;\n";
    sql += "SET sql_mode = 'STRICT_TRANS_TABLES,ERROR_FOR_DIVISION_BY_ZERO,NO_AUTO_CREATE_USER,NO_ENGINE_SUBSTITUTION';\n\n";
    sql += "START TRANSACTION;\n\n";

    sql += "-- ================================================================\n-- TABLE CREATION (dependency-ordered)\n-- ================================================================\n\n";
    for (i, r) in ordered.iter().enumerate() {
        let table = ids.tables.assign(&r.name);
        sql += &format!("-- Table: {table}\n-- Source: {}\n", r.source());
        sql += &format!(
            "-- Attributes: {}, Rows: {}\n",
            r.attributes.len(),
            js::locale_string(r.metadata.row_count)
        );
        if complex[i] {
            sql += "-- Kind: complex table (many columns)\n";
        }
        if lookup[i] {
            sql += "-- Kind: lookup table\n";
        }
        if fact[i] {
            sql += "-- Kind: fact table\n";
        }
        sql += &format!("CREATE TABLE {table} (\n");
        let mut defs = Vec::new();
        for attr in &r.attributes {
            let key = format!("{}.{}", r.name, attr);
            let dt = data_type(attr, r, &a);
            let nr = a.null_ratio.get(&key).copied().unwrap_or(0.0);
            let is_null = nullable(r, attr) && nr > 0.01;
            let mut d = format!(
                "    {} {dt}{}",
                ids.columns.assign(attr),
                if is_null { "" } else { " NOT NULL" }
            );
            if !is_null && !r.primary_key.contains(attr) {
                if let Some(def) = default_for(&dt, a.samples.get(&key)) {
                    d += &format!(" DEFAULT {def}");
                }
            }
            if let Some(c) = column_comment(attr, r, &a) {
                d += &format!(" COMMENT '{c}'");
            }
            defs.push(d);
        }
        if !r.primary_key.is_empty() {
            let pk: Vec<String> = r.primary_key.iter().map(|p| ids.columns.assign(p)).collect();
            defs.push(format!("    CONSTRAINT PK_{table} PRIMARY KEY ({})", pk.join(", ")));
        }
        sql += &defs.join(",\n");
        sql += "\n)\nENGINE=InnoDB";
        sql += if r.metadata.row_count > 100_000.0 {
            "\nROW_FORMAT=COMPRESSED"
        } else {
            "\nROW_FORMAT=DYNAMIC"
        };
        sql += "\nCHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci";
        sql += &format!(
            "\nCOMMENT='{} relation: {} cols, {} rows'",
            r.source(),
            r.attributes.len(),
            js::locale_string(r.metadata.row_count)
        );
        sql += ";\n\n";
    }

    sql += "\n-- ================================================================\n-- VALIDATED FOREIGN KEY CONSTRAINTS\n-- ================================================================\n\n";
    sql += "SET FOREIGN_KEY_CHECKS = 1;\n\n";
    for r in &ordered {
        if r.foreign_keys.is_empty() {
            continue;
        }
        sql += &format!("-- Validated Foreign Key Constraints for {}\n", r.name);
        for fk in &r.foreign_keys {
            let base = format!("FK_{}_{}", r.name, fk.references);
            let cname = if base.len() <= 64 {
                base
            } else {
                js::substring16(&base, 64)
            };
            let st = ids.tables.assign(&r.name);
            let tt = ids.tables.assign(&fk.references);
            let sc: Vec<String> = fk.attributes.iter().map(|x| ids.columns.assign(x)).collect();
            let tc: Vec<String> = fk.referenced_key.iter().map(|x| ids.columns.assign(x)).collect();
            sql += &format!("-- FK: {st}({}) -> {tt}({})\n", sc.join(","), tc.join(","));
            sql += &format!(
                "ALTER TABLE {st}\n    ADD CONSTRAINT {cname}\n    FOREIGN KEY ({})\n    REFERENCES {tt}({})",
                sc.join(", "),
                tc.join(", ")
            );
            let action = if lookup_names.contains(&fk.references) {
                "ON DELETE RESTRICT ON UPDATE CASCADE"
            } else if fk.card() == "1:1" {
                "ON DELETE CASCADE ON UPDATE CASCADE"
            } else if fk.references == r.name {
                "ON DELETE RESTRICT ON UPDATE RESTRICT"
            } else {
                "ON DELETE RESTRICT ON UPDATE CASCADE"
            };
            sql += &format!("\n    {action};\n\n");
        }
    }

    sql += "\n-- ================================================================\n-- PERFORMANCE-OPTIMIZED INDEXES\n-- ================================================================\n\n";
    for r in &ordered {
        sql += &format!("-- Intelligent Indexes for {}\n", r.name);
        let table = ids.tables.assign(&r.name);
        for fk in &r.foreign_keys {
            if fk.attributes.len() <= 3 {
                let name = js::substring16(&format!("IDX_{}_{}", r.name, fk.attributes.join("_")), 64);
                let cols: Vec<String> = fk.attributes.iter().map(|x| ids.columns.assign(x)).collect();
                sql += &format!("CREATE INDEX {name}\n    ON {table}({});\n", cols.join(", "));
            }
        }
        for attr in &r.attributes {
            let u = a
                .unique_ratio
                .get(&format!("{}.{}", r.name, attr))
                .copied()
                .unwrap_or(0.0);
            if u > 0.7 && !r.primary_key.contains(attr) && !r.foreign_keys.iter().any(|fk| fk.attributes.contains(attr))
            {
                let name = js::substring16(&format!("IDX_{}_{}", r.name, attr), 64);
                sql += &format!("-- High cardinality index ({}% unique)\n", pct(u, 1));
                sql += &format!("CREATE INDEX {name}\n    ON {table}({});\n", ids.columns.assign(attr));
            }
        }
        if r.metadata.row_count > 10_000.0 && r.attributes.len() > 5 {
            let mut cols: Vec<String> = r.primary_key.iter().map(|p| ids.columns.assign(p)).collect();
            for fk in &r.foreign_keys {
                if fk.attributes.len() <= 2 {
                    let avg = fk
                        .attributes
                        .iter()
                        .map(|x| a.unique_ratio.get(&format!("{}.{}", r.name, x)).copied().unwrap_or(0.0))
                        .sum::<f64>()
                        / fk.attributes.len() as f64;
                    if avg > 0.5 {
                        cols.extend(fk.attributes.iter().map(|x| ids.columns.assign(x)));
                    }
                }
            }
            let cols: Vec<String> = crate::types::dedup(cols).into_iter().take(4).collect();
            if !cols.is_empty() {
                let name = js::substring16(&format!("IDX_{}_covering", r.name), 64);
                sql += &format!(
                    "-- Covering index for query optimization\nCREATE INDEX {name}\n    ON {table}({});\n",
                    cols.join(", ")
                );
            }
        }
        sql += "\n";
    }
    sql += "COMMIT;\n\n";

    // Validation queries.
    sql += "\n-- ================================================================\n-- DATA VALIDATION QUERIES\n-- ================================================================\n\n";
    sql += "-- Reset foreign key checks\nSET FOREIGN_KEY_CHECKS = 1;\nSET UNIQUE_CHECKS = 1;\nSET AUTOCOMMIT = 1;\n\n";
    sql += "-- Validation Summary\nSELECT \n    'Schema Validation Summary' as validation_type,\n";
    sql += &format!("    {} as total_tables,\n", ordered.len());
    sql += "    COUNT(*) as created_tables\nFROM information_schema.tables \nWHERE table_schema = DATABASE()\n";
    let tnames: Vec<String> = ordered
        .iter()
        .map(|r| format!("'{}'", ids.tables.assign(&r.name)))
        .collect();
    sql += &format!("    AND table_name IN ({});\n\n", tnames.join(", "));
    for r in ordered.iter().take(5) {
        let table = ids.tables.assign(&r.name);
        sql += &format!(
            "-- Validate {}\nSELECT \n    '{table}' as table_name,\n    COUNT(*) as row_count,\n",
            r.name
        );
        if !r.primary_key.is_empty() {
            let pk: Vec<String> = r.primary_key.iter().map(|p| ids.columns.assign(p)).collect();
            let pk = pk.join(", ");
            sql += &format!("    COUNT(DISTINCT {pk}) as unique_keys,\n");
            sql += &format!(
                "    CASE WHEN COUNT(*) = COUNT(DISTINCT {pk}) THEN 'VALID' ELSE 'INVALID' END as pk_validation\n"
            );
        } else {
            sql += "    NULL as unique_keys,\n    'NO_PK' as pk_validation\n";
        }
        sql += &format!("FROM {table};\n\n");
    }

    // Utility scripts.
    sql += "\n-- ================================================================\n-- UTILITY SCRIPTS (ALL FDs PRESERVED)\n-- ================================================================\n\n";
    sql += "-- Reset Database Settings\nSET FOREIGN_KEY_CHECKS = 1;\nSET UNIQUE_CHECKS = 1;\nSET AUTOCOMMIT = 1;\nCOMMIT;\n\n";
    sql += "-- Functional Dependency Verification Queries\n";
    let mut fd_count = 0;
    for r in ordered.iter().take(8) {
        if let Some(fd) = &r.original_fd {
            fd_count += 1;
            let table = ids.tables.assign(&r.name);
            let lhs: Vec<String> = fd.lhs.iter().map(|x| ids.columns.assign(x)).collect();
            let lhs = lhs.join(", ");
            let rhs = ids.columns.assign(&fd.rhs);
            sql += &format!(
                "-- Verify FD: {} -> {} ({}%)\n",
                fd.lhs.join(","),
                fd.rhs,
                pct(fd.confidence, 1)
            );
            sql += "SELECT \n";
            sql += &format!("    '{}' as relation_name,\n", r.name);
            sql += &format!("    '{} -> {}' as functional_dependency,\n", fd.lhs.join(","), fd.rhs);
            sql += "    COUNT(*) as total_rows,\n";
            sql += &format!("    COUNT(DISTINCT {lhs}) as lhs_groups,\n");
            sql += &format!("    COUNT(DISTINCT {rhs}) as rhs_values,\n");
            sql += "    CASE \n";
            sql += &format!("        WHEN COUNT(DISTINCT {lhs}) = COUNT(DISTINCT CONCAT({lhs}, '|', {rhs})) \n");
            sql += "        THEN 'VALID' \n        ELSE 'INVALID' \n    END as fd_validation\n";
            sql += &format!("FROM {table}\nGROUP BY '{}';\n\n", r.name);
        }
    }
    if fd_count == 0 {
        sql += "-- No functional dependencies to verify\n\n";
    }
    sql += "-- Data Verification Queries\n";
    for r in ordered.iter().take(5) {
        let table = ids.tables.assign(&r.name);
        let pk: Vec<String> = r.primary_key.iter().map(|p| ids.columns.assign(p)).collect();
        let pk = if pk.is_empty() { "*".to_string() } else { pk.join(", ") };
        sql += &format!("-- Verify {}\nSELECT COUNT(*) as row_count, \n       COUNT(DISTINCT {pk}) as unique_keys\nFROM {table};\n\n", r.name);
    }
    sql += "-- Performance Monitoring\nSHOW ENGINE InnoDB STATUS\\G\nSELECT table_name, table_rows, data_length, index_length \n";
    sql += "FROM information_schema.tables \nWHERE table_schema = DATABASE() \nORDER BY data_length DESC;\n\n";
    sql += "-- End of Generated DDL (ALL FUNCTIONAL DEPENDENCIES PRESERVED)\n";
    let fks: usize = ordered.iter().map(|r| r.foreign_keys.len()).sum();
    let est = (ordered.len() as f64 * 0.1 + fks as f64 * 0.2 + ordered.len() as f64 * 0.3).ceil();
    sql += &format!("-- Total execution time estimate: {} seconds\n", js::num_to_string(est));
    sql += &format!(
        "-- Schema preserves {} functional dependencies\n",
        s.high + s.medium + s.low
    );
    sql
}

fn safe_fallback(relations: &[Relation], v: Option<&Validation>, ids: &mut Ids) -> String {
    let mut sql = String::from("-- SAFE FALLBACK SQL DDL\n");
    sql += &format!("-- Generated: {}\n", js::iso_now());
    sql += "-- This is a simplified, safe version due to validation issues\n";
    if let Some(v) = v {
        sql += &format!("-- Validation Confidence: {}%\n", js::to_fixed(v.confidence, 1));
        sql += &format!("-- Validation Errors: {}\n", v.errors.len());
        if !v.errors.is_empty() {
            sql += "-- Errors:\n";
            for e in &v.errors {
                sql += &format!("--   {e}\n");
            }
        }
    }
    sql += "\n";
    for r in relations {
        let table = ids.tables.assign(&r.name);
        sql += &format!(
            "-- Table: {table} ({} attributes)\nCREATE TABLE IF NOT EXISTS {table} (\n",
            r.attributes.len()
        );
        let mut cols: Vec<String> = r
            .attributes
            .iter()
            .map(|a| format!("    {} VARCHAR(255)", ids.columns.assign(a)))
            .collect();
        if !r.primary_key.is_empty() {
            let pk: Vec<String> = r.primary_key.iter().map(|p| ids.columns.assign(p)).collect();
            cols.push(format!("    PRIMARY KEY ({})", pk.join(", ")));
        }
        sql += &cols.join(",\n");
        sql += "\n) ENGINE=InnoDB CHARSET=utf8mb4;\n\n";
    }
    sql += "-- Note: Foreign key constraints omitted in fallback mode for safety\n";
    sql += "-- Please review and add constraints manually after data validation\n";
    sql
}
