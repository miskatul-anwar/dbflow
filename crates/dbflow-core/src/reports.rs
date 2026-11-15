//! Markdown/SVG output files of a single-file run and the multi-file report.

use crate::analysis::TableAnalysis;
use crate::data_processor::AtomicityResult;
use crate::identifiers::md_escape;
use crate::js;
use crate::normalization::KeyResult;
use crate::schema::BuiltSchema;
use crate::types::{CrossLink, Fd, Relation};
use indexmap::IndexMap;
use serde_json::Value;

fn pct1(x: f64) -> String {
    js::to_fixed(x * 100.0, 1)
}

fn now() -> String {
    js::iso_now()
}

fn has(ops: &[String], op: &str) -> bool {
    ops.iter().any(|o| o == op)
}

fn lhs_text(fd: &Fd) -> String {
    fd.lhs.join(", ")
}

pub fn fd_markdown(fds: &[Fd]) -> String {
    let mut c = String::from("# Functional Dependencies Analysis\n\n");
    c += &format!("**Generated:** {}\n**Total Dependencies:** {}\n\n", now(), fds.len());
    if fds.is_empty() {
        c += "## No Dependencies Found\n\nNo functional dependencies were discovered in the dataset. This could indicate:\n\n";
        c += "- The dataset is already in a highly normalized form\n- The dataset lacks sufficient data patterns\n- Additional data analysis may be required\n\n";
        return c;
    }
    let high: Vec<&Fd> = fds.iter().filter(|f| f.conf() > 0.98).collect();
    let med = fds.iter().filter(|f| f.conf() > 0.90 && f.conf() <= 0.98).count();
    let low = fds.iter().filter(|f| f.conf() <= 0.90).count();
    c += "## Summary\n\n";
    c += &format!("- **High Confidence (>98%):** {} dependencies\n", high.len());
    c += &format!("- **Medium Confidence (90-98%):** {med} dependencies\n");
    c += &format!("- **Low Confidence (<90%):** {low} dependencies\n\n");
    c += "## All Discovered Dependencies\n\n";
    c += "| # | Left Hand Side (LHS) | Right Hand Side (RHS) | Confidence | Coverage | Unique Groups | Quality |\n";
    c += "|---|---------------------|----------------------|------------|----------|---------------|----------|\n";
    for (i, fd) in fds.iter().enumerate() {
        let q = if fd.conf() > 0.98 {
            "High"
        } else if fd.conf() > 0.90 {
            "Medium"
        } else {
            "Low"
        };
        c += &format!(
            "| {} | **{}** | **{}** | {}% | {}% | {} | {q} |\n",
            i + 1,
            lhs_text(fd),
            fd.rhs.text(),
            js::to_fixed(fd.conf() * 100.0, 2),
            js::to_fixed(fd.coverage.unwrap_or(f64::NAN) * 100.0, 2),
            fd.unique_groups
                .map(js::num_to_string)
                .unwrap_or_else(|| "undefined".into())
        );
    }
    if !high.is_empty() {
        c += "\n## High-Confidence Dependencies (Recommended for Normalization)\n\n";
        for (i, fd) in high.iter().enumerate() {
            c += &format!("### {}. {} → {}\n\n", i + 1, lhs_text(fd), fd.rhs.text());
            c += &format!("- **Confidence:** {}%\n", js::to_fixed(fd.conf() * 100.0, 2));
            c += &format!(
                "- **Coverage:** {}%\n",
                js::to_fixed(fd.coverage.unwrap_or(f64::NAN) * 100.0, 2)
            );
            c += &format!(
                "- **Unique Groups:** {}\n",
                fd.unique_groups
                    .map(js::num_to_string)
                    .unwrap_or_else(|| "undefined".into())
            );
            c += &format!(
                "- **Violations:** {}\n\n",
                fd.violations
                    .map(js::num_to_string)
                    .unwrap_or_else(|| "undefined".into())
            );
            c += "**Interpretation:** This dependency is highly reliable and should be used for normalization.\n\n";
        }
    }
    c += "\n## Methodology\n\nFunctional dependencies were discovered using:\n\n";
    c += "- **Algorithm:** Optimized FD mining with adaptive thresholds\n- **Confidence Threshold:** 95% minimum (adaptive)\n";
    c += "- **LHS Size Limit:** 4 attributes maximum (adaptive)\n- **Minimality Check:** Ensured minimal left-hand sides\n";
    c += "- **Optimizations:** Progressive sampling, streaming algorithms\n\n";
    c += "## Metrics Explanation\n\n";
    c += "- **Confidence:** Percentage of data that follows this dependency (higher is better)\n";
    c += "- **Coverage:** Percentage of data groups covered by this dependency\n";
    c += "- **Unique Groups:** Number of distinct LHS value combinations\n";
    c += "- **Violations:** Number of records that violate this dependency\n\n";
    c
}

pub fn fd_svg(fds: &[Fd]) -> String {
    let (w, h, rad) = (800.0, 600.0, 40.0);
    let mut s = format!("<svg width=\"{w}\" height=\"{h}\" xmlns=\"http://www.w3.org/2000/svg\">");
    s += "<defs><style>.high-conf { fill: #dcfce7; stroke: #16a34a; } .med-conf { fill: #fef3c7; stroke: #d97706; } .low-conf { fill: #fee2e2; stroke: #dc2626; }</style></defs>";
    let cols = ((fds.len() * 2).min(20) as f64).sqrt().ceil();
    let cw = w / cols;
    let ch = h / (10.0 / cols).ceil();
    let n = js::num_to_string;
    for (i, fd) in fds.iter().take(10).enumerate() {
        let row = (i as f64 / cols).floor();
        let col = i as f64 % cols;
        let x = col * cw + cw / 2.0;
        let y = row * ch + ch / 2.0;
        let conf = fd.confidence.unwrap_or(0.0);
        let cls = if conf > 0.98 {
            "high-conf"
        } else if conf > 0.90 {
            "med-conf"
        } else {
            "low-conf"
        };
        s += &format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{rad}\" class=\"{cls}\" stroke-width=\"2\"/>",
            n(x - 80.0),
            n(y)
        );
        s += &format!("<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\" font-family=\"Inter, sans-serif\">{}</text>", n(x - 80.0), n(y), lhs_text(fd));
        s += &format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{rad}\" class=\"{cls}\" stroke-width=\"2\"/>",
            n(x + 80.0),
            n(y)
        );
        s += &format!("<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"10\" font-family=\"Inter, sans-serif\">{}</text>", n(x + 80.0), n(y), fd.rhs.text());
        s += &format!("<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#374151\" stroke-width=\"2\" marker-end=\"url(#arrow)\"/>", n(x - 40.0), n(y), n(x + 40.0), n(y));
        s += &format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"8\">{}%</text>",
            n(x),
            n(y - 10.0),
            js::to_fixed(conf * 100.0, 1)
        );
    }
    s += "<defs><marker id=\"arrow\" markerWidth=\"10\" markerHeight=\"10\" refX=\"9\" refY=\"3\" orient=\"auto\" markerUnits=\"strokeWidth\"><polygon points=\"0,0 0,6 9,3\" fill=\"#374151\" /></marker></defs>";
    s += "</svg>";
    s
}

pub fn key_markdown(k: &KeyResult) -> String {
    let mut c = String::from("# Key Analysis Report\n\n");
    c += &format!(
        "**Generated:** {}\n**Candidate Keys:** {}\n**Super Keys:** {}\n\n",
        now(),
        k.candidate_keys.len(),
        k.superkeys.len()
    );
    if let Some(m) = &k.search_metrics {
        c += "## Search Performance Metrics\n\n";
        c += &format!(
            "- **Strategy Used:** {}\n- **Total Duration:** {}ms\n",
            m.final_strategy,
            js::num_to_string(m.total_duration)
        );
        c += &format!(
            "- **Combinations Evaluated:** {}\n- **Search Phases:** {}\n\n",
            m.total_evaluated,
            m.phases.len()
        );
        c += "### Phase Breakdown\n\n| Phase | Evaluated | Keys Found | Duration (ms) | Success |\n|-------|-----------|------------|---------------|----------|\n";
        for p in &m.phases {
            c += &format!(
                "| {} | {} | {} | {} | {} |\n",
                p.name,
                p.evaluated,
                p.keys_found,
                js::num_to_string(p.duration),
                if p.success { "Yes" } else { "No" }
            );
        }
        c += "\n";
    }
    c += "## Candidate Keys\n\n";
    if k.candidate_keys.is_empty() {
        c += "No candidate keys were discovered. This may indicate:\n\n- Insufficient functional dependencies\n- Complex key structures requiring manual analysis\n- Data quality issues\n\n";
    } else {
        c += "Candidate keys are minimal super keys that uniquely identify tuples:\n\n";
        for (i, key) in k.candidate_keys.iter().enumerate() {
            c += &format!(
                "### {}. {{{}}}\n\n- **Size:** {} attribute(s)\n",
                i + 1,
                key.join(", "),
                key.len()
            );
            c += &format!(
                "- **Type:** {} key\n",
                if key.len() == 1 { "Simple" } else { "Composite" }
            );
            c += &format!(
                "- **Recommendation:** {}\n\n",
                if key.len() <= 2 {
                    "Suitable for primary key"
                } else {
                    "Consider surrogate key"
                }
            );
        }
    }
    c += "## Super Keys\n\n";
    if k.superkeys.is_empty() {
        c += "No super keys were identified.\n\n";
    } else {
        c += "Super keys are sets of attributes that uniquely identify tuples:\n\n";
        for (i, key) in k.superkeys.iter().enumerate() {
            let cand = k
                .candidate_keys
                .iter()
                .any(|ck| ck.len() == key.len() && ck.iter().all(|a| key.contains(a)));
            c += &format!(
                "{}. {{{}}}{}\n",
                i + 1,
                key.join(", "),
                if cand { " *(Candidate Key)*" } else { "" }
            );
        }
        c += "\n";
    }
    c += "## Analysis Summary\n\n";
    c += &format!(
        "- **Total Super Keys Found:** {}\n- **Minimal Keys (Candidates):** {}\n",
        k.superkeys.len(),
        k.candidate_keys.len()
    );
    let avg = if k.candidate_keys.is_empty() {
        "N/A".to_string()
    } else {
        js::to_fixed(
            k.candidate_keys.iter().map(|x| x.len()).sum::<usize>() as f64 / k.candidate_keys.len() as f64,
            1,
        )
    };
    c += &format!("- **Average Key Size:** {avg}\n\n## Recommendations\n\n");
    if !k.candidate_keys.is_empty() {
        let simple: Vec<&Vec<String>> = k.candidate_keys.iter().filter(|x| x.len() == 1).collect();
        let comp: Vec<&Vec<String>> = k.candidate_keys.iter().filter(|x| x.len() > 1).collect();
        if let Some(s) = simple.first() {
            c += &format!(
                "- **Primary Key Suggestion:** Consider using {{{}}} as the primary key\n",
                s.join(", ")
            );
        } else if !comp.is_empty() {
            let mut best = comp[0];
            for x in &comp {
                if x.len() < best.len() {
                    best = x;
                }
            }
            c += &format!(
                "- **Primary Key Suggestion:** Consider using {{{}}} as the primary key\n",
                best.join(", ")
            );
        }
        if k.candidate_keys.len() > 1 {
            c += "- **Alternative Keys:** Other candidate keys can serve as unique constraints\n";
        }
        c += "- **Index Strategy:** Create indexes on all candidate keys for optimal performance\n";
    } else {
        c += "- **Action Required:** Manual key identification needed\n- **Consider:** Adding surrogate keys (auto-increment IDs)\n- **Review:** Data quality and functional dependencies\n";
    }
    c
}

pub fn atomicity_markdown(a: &AtomicityResult) -> String {
    let mut c = String::from("# Atomicity Analysis Report\n\n");
    c += &format!(
        "**Generated:** {}\n**Purpose:** First Normal Form (1NF) Compliance Check\n\n",
        now()
    );
    let total = a.atomicity.len();
    let atomic = a.atomicity.values().filter(|v| **v).count();
    let viol = a.violations.len();
    c += &format!("## Summary\n\n- **Total Attributes:** {total}\n- **Atomic Attributes:** {atomic}\n- **Non-Atomic Attributes:** {viol}\n");
    c += &format!("- **1NF Compliance:** {}\n\n", if viol == 0 { "YES" } else { "NO" });
    if viol == 0 {
        c += "## 1NF Compliance Achieved\n\nAll attributes in the dataset are atomic. The data satisfies First Normal Form requirements.\n\n";
    } else {
        c += "## 1NF Violations Detected\n\nThe following attributes contain non-atomic values:\n\n";
        for (attr, vs) in &a.violations {
            c += &format!("### {attr}\n\n**Sample Violations:**\n\n");
            for (i, v) in vs.iter().take(3).enumerate() {
                c += &format!("{}. Row {}: \"{}\"\n", i + 1, v.row, v.value);
            }
            c += "\n**Recommended Action:** Decompose into separate attributes or normalize values\n\n";
        }
    }
    c += "## Detailed Attribute Analysis\n\n| Attribute | Status | Action Required |\n|-----------|--------|----------------|\n";
    for (attr, ok) in &a.atomicity {
        c += &format!(
            "| {attr} | {} | {} |\n",
            if *ok { "Atomic" } else { "Non-Atomic" },
            if *ok { "None" } else { "Decompose" }
        );
    }
    c
}

fn sql_type_of(t: &str) -> &'static str {
    match t {
        "string" => "VARCHAR(255)",
        "integer" => "INTEGER",
        "decimal" => "DECIMAL(10,2)",
        "boolean" => "BOOLEAN",
        "date" => "DATE",
        "datetime" => "TIMESTAMP",
        "email" => "VARCHAR(255)",
        "url" => "TEXT",
        "phone" => "VARCHAR(20)",
        "text" => "TEXT",
        "number" => "NUMERIC",
        _ => "TEXT",
    }
}

fn type_distribution(p: &IndexMap<String, String>) -> IndexMap<String, usize> {
    let mut d = IndexMap::new();
    for t in p.values() {
        *d.entry(t.clone()).or_insert(0) += 1;
    }
    d
}

pub fn profile_markdown(p: &IndexMap<String, String>) -> String {
    let mut c = String::from("# Data Type Profile Report\n\n");
    c += &format!(
        "**Generated:** {}\n**Purpose:** Data Quality and Type Analysis\n\n",
        now()
    );
    if p.is_empty() {
        c += "No data profiling results available.\n";
        return c;
    }
    c += "## Data Type Distribution\n\n";
    for (t, n) in type_distribution(p) {
        c += &format!("- **{t}:** {n} attribute(s)\n");
    }
    c += "\n## Attribute Details\n\n| Attribute | Data Type | Recommended SQL Type |\n|-----------|-----------|---------------------|\n";
    for (a, t) in p {
        c += &format!("| {a} | {t} | {} |\n", sql_type_of(t));
    }
    c += "\n## Data Quality Recommendations\n\n1. **String Attributes:** Consider VARCHAR length optimization\n2. **Numeric Attributes:** Validate precision and scale requirements\n";
    c += "3. **Date Attributes:** Ensure consistent date formats\n4. **Boolean Attributes:** Standardize true/false representations\n\n";
    c
}

pub fn nf1_markdown(rels: &[Relation]) -> String {
    let mut c = String::from("# First Normal Form (1NF) Analysis\n\n");
    c += &format!("**Generated:** {}\n**Relations Created:** {}\n\n", now(), rels.len());
    c += "## 1NF Transformation Process\n\nFirst Normal Form ensures that:\n1. All attribute values are atomic (indivisible)\n2. No repeating groups exist\n3. Each row is unique\n\n";
    if rels.is_empty() {
        c += "No 1NF transformation was necessary. The original data may already be in 1NF.\n";
        return c;
    }
    c += "## Generated Relations\n\n";
    for (i, r) in rels.iter().enumerate() {
        c += &format!(
            "### {}. {}\n\n**Attributes:** {{{}}}\n",
            i + 1,
            r.name,
            r.attributes.join(", ")
        );
        c += &format!("**Primary Key:** {{{}}}\n", r.primary_key.join(", "));
        c += &format!(
            "**Row Count:** {}\n**Source:** {}\n\n",
            js::num_to_string(r.metadata.row_count),
            r.source()
        );
    }
    c
}

pub fn nf2_markdown(rels: &[Relation]) -> String {
    let mut c = String::from("# Second Normal Form (2NF) Analysis\n\n");
    c += &format!("**Generated:** {}\n**Relations Created:** {}\n\n", now(), rels.len());
    c += "## 2NF Decomposition Process\n\nSecond Normal Form eliminates partial dependencies by ensuring that:\n1. The relation is in First Normal Form (1NF)\n2. Every non-prime attribute is fully functionally dependent on the primary key\n\n";
    if rels.is_empty() {
        c += "No 2NF decomposition was necessary. The original relation may already be in 2NF or higher.\n";
        return c;
    }
    c += "## Generated Relations\n\n";
    for (i, r) in rels.iter().enumerate() {
        c += &format!(
            "### {}. {}\n\n**Attributes:** {{{}}}\n",
            i + 1,
            r.name,
            r.attributes.join(", ")
        );
        c += &format!("**Primary Key:** {{{}}}\n", r.primary_key.join(", "));
        c += &format!("**Row Count:** {}\n\n", js::num_to_string(r.metadata.row_count));
    }
    c
}

fn card_relationship(card: f64) -> &'static str {
    if card > 0.95 {
        "One-to-One (1:1)"
    } else if card > 0.3 {
        "One-to-Many (1:N)"
    } else {
        "Many-to-Many (M:N)"
    }
}

fn num_or_na(v: &Value) -> String {
    match v.as_f64() {
        Some(n) if n != 0.0 => js::num_to_string(n),
        _ => "N/A".into(),
    }
}

pub fn cardinality_markdown(ca: &IndexMap<String, Value>) -> String {
    let mut c = String::from("# Cardinality Analysis Report\n\n");
    c += &format!("**Generated:** {}\n\n", now());
    if ca.is_empty() {
        c += "No cardinality analysis results available.\n";
        return c;
    }
    c += "## Attribute Cardinality Summary\n\n| Attribute | Total Values | Unique Values | Cardinality Ratio | Data Type | Relationship Type |\n";
    c += "|-----------|--------------|---------------|-------------------|-----------|-------------------|\n";
    for (a, v) in ca {
        let card = v["cardinality"].as_f64().unwrap_or(0.0);
        let cr = if card != 0.0 {
            format!("{}%", pct1(card))
        } else {
            "N/A".into()
        };
        c += &format!(
            "| {a} | {} | {} | {cr} | {} | {} |\n",
            num_or_na(&v["totalValues"]),
            num_or_na(&v["uniqueValues"]),
            v["dataType"].as_str().unwrap_or("string"),
            card_relationship(card)
        );
    }
    c += "\n## Cardinality Insights\n\n";
    let high = ca
        .values()
        .filter(|v| v["cardinality"].as_f64().unwrap_or(0.0) > 0.9)
        .count();
    let low = ca
        .values()
        .filter(|v| v["cardinality"].as_f64().unwrap_or(0.0) < 0.3)
        .count();
    if high > 0 {
        c += &format!("**High Uniqueness Attributes (>90%):** {high} attributes\nThese attributes are suitable for primary keys or unique constraints.\n\n");
    }
    if low > 0 {
        c += &format!("**Low Uniqueness Attributes (<30%):** {low} attributes\nThese attributes may benefit from normalization or indexing strategies.\n\n");
    }
    c
}

pub fn lossless_markdown(l: &Value) -> String {
    let ok = l["isLossless"].as_bool().unwrap_or(false);
    let mut c = String::from("# Lossless Join Analysis\n\n");
    c += &format!(
        "**Generated:** {}\n**Result:** {}\n\n",
        now(),
        if ok { "LOSSLESS" } else { "LOSSY" }
    );
    c += &format!(
        "## Analysis Summary\n\nThe decomposition is {}. This means that joining the decomposed relations {} produce the original relation without any loss or addition of information.\n\n",
        if ok { "lossless" } else { "lossy" },
        if ok { "will" } else { "will NOT" }
    );
    if let Some(d) = l.get("details").filter(|d| d.is_object()) {
        let or_na = |v: &Value| match v {
            Value::Number(n) if n.as_f64() != Some(0.0) => js::num_to_string(n.as_f64().unwrap_or(0.0)),
            _ => "N/A".into(),
        };
        c += "## Technical Details\n\n";
        c += &format!(
            "- **Algorithm Used:** {}\n",
            d["algorithm"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("Chase Algorithm")
        );
        c += &format!("- **Relations Tested:** {}\n", or_na(&d["relationCount"]));
        c += &format!("- **Functional Dependencies:** {}\n\n", or_na(&d["fdCount"]));
    }
    c += "## Implementation Notes\n\n";
    if ok {
        c += "- The decomposition preserves all information from the original relation\n- Natural joins of the decomposed relations will reconstruct the original data\n- No data anomalies will occur during join operations\n";
    } else {
        c += "- Some information may be lost when joining the decomposed relations\n- Additional constraints may be needed to prevent data anomalies\n- Consider revising the decomposition strategy\n";
    }
    c
}

fn fd_list(c: &mut String, title: &str, list: &[Value]) {
    if list.is_empty() {
        return;
    }
    *c += &format!("## {title}\n\n");
    for (i, fd) in list.iter().take(10).enumerate() {
        let lhs = match &fd["lhs"] {
            Value::Array(a) => a
                .iter()
                .map(crate::table::json_to_js_string)
                .collect::<Vec<_>>()
                .join(", "),
            other => crate::table::json_to_js_string(other),
        };
        *c += &format!("{}. {lhs} → {}\n", i + 1, crate::table::json_to_js_string(&fd["rhs"]));
    }
    if list.len() > 10 {
        *c += &format!("... and {} more\n", list.len() - 10);
    }
    *c += "\n";
}

pub fn preservation_markdown(d: &Value) -> String {
    let ok = d["isPreserved"].as_bool().unwrap_or(false);
    let mut c = String::from("# Dependency Preservation Analysis\n\n");
    c += &format!(
        "**Generated:** {}\n**Result:** {}\n\n## Analysis Summary\n\n",
        now(),
        if ok { "PRESERVED" } else { "NOT PRESERVED" }
    );
    if ok {
        c += "All functional dependencies are preserved in the decomposition. This ensures that all integrity constraints can be enforced without joins.\n\n";
    } else {
        c += "Some functional dependencies are lost in the decomposition. This may require joins to enforce certain integrity constraints.\n\n";
    }
    if !d["preservedCount"].is_null() {
        c += "## Preservation Statistics\n\n";
        c += &format!(
            "- **Total Dependencies:** {}\n",
            crate::table::json_to_js_string(&d["totalDependencies"])
        );
        c += &format!(
            "- **Preserved Dependencies:** {}\n",
            crate::table::json_to_js_string(&d["preservedCount"])
        );
        c += &format!(
            "- **Lost Dependencies:** {}\n",
            crate::table::json_to_js_string(&d["lostCount"])
        );
        c += &format!(
            "- **Preservation Ratio:** {}%\n\n",
            pct1(d["preservationRatio"].as_f64().unwrap_or(f64::NAN))
        );
    }
    fd_list(
        &mut c,
        "Preserved Dependencies",
        d["preserved"].as_array().map(|a| a.as_slice()).unwrap_or(&[]),
    );
    fd_list(
        &mut c,
        "Lost Dependencies",
        d["lost"].as_array().map(|a| a.as_slice()).unwrap_or(&[]),
    );
    c
}

pub fn performance_markdown(pm: &Value, memory_source: &str) -> String {
    let mut c = String::from("# Performance Analysis Report\n\n");
    c += &format!(
        "**Generated:** {}\n**Total Duration:** {}\n",
        now(),
        pm["totalDuration"].as_str().unwrap_or("")
    );
    c += &format!(
        "**Peak Memory Usage:** {} MB\n\n",
        js::to_fixed(pm["peakMemoryUsage"].as_f64().unwrap_or(0.0), 2)
    );
    c += "## Performance Checkpoints\n\n| Checkpoint | Time (ms) | Memory (MB) | Phase |\n|------------|-----------|-------------|-------|\n";
    let cps = pm["checkpoints"].as_array().cloned().unwrap_or_default();
    for cp in &cps {
        let m = cp["memory"]
            .as_f64()
            .filter(|m| *m > 0.0)
            .map(|m| js::to_fixed(m, 2))
            .unwrap_or_else(|| "< 0.01".into());
        c += &format!(
            "| {} | {} | {m} | Processing |\n",
            cp["name"].as_str().unwrap_or(""),
            js::num_to_string(cp["time"].as_f64().unwrap_or(0.0))
        );
    }
    c += if memory_source == "estimate" {
        "\n*Memory is an estimate of the loaded data (this runtime does not report heap usage).*\n"
    } else {
        "\n*Memory is the resident memory of the native engine.*\n"
    };
    c += "\n## Optimizations Applied\n\n";
    for o in pm["optimizationsApplied"].as_array().cloned().unwrap_or_default() {
        c += &format!("- {}\n", o.as_str().unwrap_or(""));
    }
    c += "\n## Cache Performance\n\n";
    if let Some(rate) = pm["cacheHitRate"].as_f64() {
        let hits = pm["cacheHits"].as_f64().unwrap_or(0.0);
        let misses = pm["cacheMisses"].as_f64().unwrap_or(0.0);
        c += &format!(
            "- **Partition Cache Hit Rate:** {}% ({} of {} partition requests served from cache during FD mining)\n\n",
            pct1(rate),
            js::num_to_string(hits),
            js::num_to_string(hits + misses)
        );
    } else {
        c += "- **Partition Cache Hit Rate:** not applicable (FD Miner did not run)\n\n";
    }
    c += "## Performance Insights\n\n";
    let total = cps.last().and_then(|c| c["time"].as_f64()).unwrap_or(0.0);
    c += if total < 10_000.0 {
        "- **Excellent Performance:** Process completed in under 10 seconds\n"
    } else if total < 30_000.0 {
        "- **Good Performance:** Process completed efficiently\n"
    } else {
        "- **Extended Processing:** Consider data sampling for larger datasets\n"
    };
    c
}

pub struct ReportInput<'a> {
    pub ops: &'a [String],
    pub fds: &'a [Fd],
    pub keys: &'a KeyResult,
    pub r1: &'a [Relation],
    pub r2: &'a [Relation],
    pub r3: &'a [Relation],
    pub row_count: usize,
    pub columns: &'a [String],
    pub cardinality: &'a IndexMap<String, Value>,
    pub lossless: &'a Value,
    pub preservation: &'a Value,
    pub atomicity: &'a AtomicityResult,
    pub profile: &'a IndexMap<String, String>,
    pub performance: &'a Value,
}

pub fn process_report(i: &ReportInput) -> String {
    let ops = i.ops;
    let mut c = String::from("# Database Normalization Analysis Report\n\n");
    c += &format!("**Generated:** {}\n**Author:** miskatul-anwar\n", now());
    c += &format!(
        "**Dataset:** {} records, {} attributes\n\n",
        i.row_count,
        i.columns.len()
    );
    c += "## Executive Summary\n\n";
    if has(ops, "FD Miner") && !i.fds.is_empty() {
        let high = i.fds.iter().filter(|f| f.conf() > 0.98).count();
        c += &format!(
            "**Functional Dependencies:** {} discovered ({high} high-confidence)\n",
            i.fds.len()
        );
    }
    if has(ops, "Key Discovery") {
        c += &format!(
            "**Primary Keys:** {} candidate keys identified\n",
            i.keys.candidate_keys.len()
        );
    }
    if has(ops, "3NF") && !i.r3.is_empty() {
        c += &format!("**Normalized Schema:** {} relations in Third Normal Form\n", i.r3.len());
    }
    let lossless = i.lossless["isLossless"].as_bool().unwrap_or(false);
    if has(ops, "Lossless Check") {
        c += &format!("**Lossless Join:** {}\n", if lossless { "VERIFIED" } else { "FAILED" });
    }
    let ratio = i.preservation["preservationRatio"].as_f64();
    if has(ops, "Dependency Preservation") {
        let r = ratio.filter(|r| *r != 0.0).map(pct1).unwrap_or_else(|| "0".into());
        c += &format!("**Dependency Preservation:** {r}% of dependencies preserved\n");
    }
    c += "\n## Analysis Results by Operation\n\n";
    if has(ops, "Data Loader") || i.row_count > 0 {
        c += "### Data Loader\n\n";
        if i.row_count == 0 {
            c += "**Status:** No data loaded\n\n";
        } else {
            c += &format!(
                "**Status:** Successfully loaded\n**Records:** {}\n**Attributes:** {}\n**Columns:** {}\n\n",
                js::locale_usize(i.row_count),
                i.columns.len(),
                i.columns.join(", ")
            );
        }
    }
    if has(ops, "Data Cleaner") {
        c += &format!(
            "### Data Cleaner\n\n**Status:** Data cleaning completed\n**Records After Cleaning:** {}\n**Actions:** Removed null values, standardized formats, validated integrity\n\n",
            js::locale_usize(i.row_count)
        );
    }
    if has(ops, "Atomicity Checker") {
        c += "### Atomicity Checker\n\n";
        let total = i.atomicity.atomicity.len();
        let atomic = i.atomicity.atomicity.values().filter(|v| **v).count();
        let v = i.atomicity.violations.len();
        c += &format!(
            "**Status:** {}\n**Atomic Attributes:** {atomic}/{total}\n",
            if v == 0 { "PASSED" } else { "VIOLATIONS FOUND" }
        );
        if v > 0 {
            c += &format!("**Non-Atomic Attributes:** {v}\n**Action Required:** Decompose non-atomic values\n");
        }
        c += "\n";
    }
    if has(ops, "Data Type Profiler") {
        c += "### Data Type Profiler\n\n";
        if i.profile.is_empty() {
            c += "**Status:** No data profiling available\n\n";
        } else {
            c += "**Status:** Profiling completed\n**Data Types Identified:**\n";
            for (t, n) in type_distribution(i.profile) {
                c += &format!("- {t}: {n} attribute(s)\n");
            }
            c += "\n";
        }
    }
    if has(ops, "FD Miner") {
        c += "### Functional Dependency Miner\n\n";
        if i.fds.is_empty() {
            c += "**Status:** No functional dependencies discovered\n**Implication:** Dataset may already be highly normalized or lacks clear patterns\n\n";
        } else {
            let high: Vec<&Fd> = i.fds.iter().filter(|f| f.conf() > 0.98).collect();
            let med = i.fds.iter().filter(|f| f.conf() > 0.90 && f.conf() <= 0.98).count();
            c += &format!(
                "**Status:** Mining completed successfully\n**Total Dependencies:** {}\n**High Confidence (>98%):** {}\n**Medium Confidence (90-98%):** {med}\n",
                i.fds.len(),
                high.len()
            );
            if !high.is_empty() {
                c += "**Key Dependencies:**\n";
                for fd in high.iter().take(5) {
                    c += &format!("- {} → {} ({}%)\n", lhs_text(fd), fd.rhs.text(), pct1(fd.conf()));
                }
                if high.len() > 5 {
                    c += &format!("- ... and {} more\n", high.len() - 5);
                }
            }
            c += "\n";
        }
    }
    if has(ops, "Key Discovery") {
        c += "### Key Discovery\n\n";
        let k = i.keys;
        if k.candidate_keys.is_empty() {
            c += "**Status:** No natural keys found\n**Recommendation:** Consider adding surrogate keys (auto-increment IDs)\n\n";
        } else {
            c += &format!(
                "**Status:** Key discovery completed\n**Candidate Keys:** {}\n**Super Keys:** {}\n",
                k.candidate_keys.len(),
                k.superkeys.len()
            );
            if let Some(m) = &k.search_metrics {
                c += &format!(
                    "**Search Strategy:** {}\n**Processing Time:** {}ms\n",
                    m.final_strategy,
                    js::num_to_string(m.total_duration)
                );
            }
            c += "**Discovered Keys:**\n";
            for key in &k.candidate_keys {
                c += &format!(
                    "- {{{}}} ({})\n",
                    key.join(", "),
                    if key.len() == 1 { "Simple" } else { "Composite" }
                );
            }
            if let Some(s) = k.candidate_keys.iter().find(|x| x.len() == 1) {
                c += &format!("**Recommended Primary Key:** {{{}}}\n", s.join(", "));
            } else {
                let mut best = &k.candidate_keys[0];
                for x in &k.candidate_keys {
                    if x.len() < best.len() {
                        best = x;
                    }
                }
                c += &format!("**Recommended Primary Key:** {{{}}}\n", best.join(", "));
            }
            c += "\n";
        }
    }
    if has(ops, "Cardinality Analyzer") {
        c += "### Cardinality Analyzer\n\n";
        if i.cardinality.is_empty() {
            c += "**Status:** No cardinality analysis available\n\n";
        } else {
            let attrs: Vec<&String> = i.cardinality.keys().collect();
            let card = |a: &String| i.cardinality[a]["cardinality"].as_f64().unwrap_or(0.0);
            let high: Vec<&String> = attrs.iter().copied().filter(|a| card(a) > 0.9).collect();
            let low: Vec<&String> = attrs.iter().copied().filter(|a| card(a) < 0.3).collect();
            c += &format!(
                "**Status:** Analysis completed\n**Attributes Analyzed:** {}\n**High Uniqueness (>90%):** {} attributes\n**Low Uniqueness (<30%):** {} attributes\n",
                attrs.len(),
                high.len(),
                low.len()
            );
            if !high.is_empty() {
                c += &format!(
                    "**Key Candidates:** {}\n",
                    high.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                );
            }
            if !low.is_empty() {
                c += &format!(
                    "**Normalization Candidates:** {}\n",
                    low.iter().take(3).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                );
            }
            c += "\n";
        }
    }
    if has(ops, "1NF") {
        c += "### First Normal Form (1NF)\n\n";
        if i.r1.is_empty() {
            c += "**Status:** Data already in 1NF\n**Result:** No transformation required\n\n";
        } else {
            c += &format!(
                "**Status:** 1NF transformation applied\n**Relations Created:** {}\n**Atomicity:** Ensured all attribute values are indivisible\n**Repeating Groups:** Eliminated\n",
                i.r1.len()
            );
            for r in i.r1 {
                c += &format!(
                    "**Relation:** {} ({} attributes, {} rows)\n",
                    r.name,
                    r.attributes.len(),
                    js::num_to_string(r.metadata.row_count)
                );
            }
            c += "\n";
        }
    }
    if has(ops, "2NF") {
        c += "### Second Normal Form (2NF)\n\n";
        if i.r2.is_empty() {
            c += "**Status:** No partial dependencies found\n**Result:** Data already in 2NF or higher\n\n";
        } else {
            c += &format!("**Status:** 2NF decomposition completed\n**Relations Created:** {}\n**Partial Dependencies:** Eliminated\n", i.r2.len());
            for r in i.r2 {
                let pk = if r.primary_key.is_empty() {
                    "None".to_string()
                } else {
                    r.primary_key.join(", ")
                };
                c += &format!(
                    "**Relation:** {}\n  - Attributes: {{{}}}\n  - Primary Key: {{{pk}}}\n  - Rows: {}\n",
                    r.name,
                    r.attributes.join(", "),
                    js::num_to_string(r.metadata.row_count)
                );
            }
            c += "\n";
        }
    }
    if has(ops, "3NF") {
        c += "### Third Normal Form (3NF)\n\n";
        if i.r3.is_empty() {
            c += "**Status:** No 3NF relations generated\n\n";
        } else {
            let connected = i.r3.iter().filter(|r| !r.foreign_keys.is_empty()).count();
            let rels: usize = i.r3.iter().map(|r| r.foreign_keys.len()).sum();
            c += &format!(
                "**Status:** 3NF synthesis completed using Bernstein algorithm\n**Final Schema:** {} normalized relations\n**Connected Relations:** {connected}\n**Relationships:** {rels}\n**Relations Overview:**\n",
                i.r3.len()
            );
            for r in i.r3 {
                let pk = if r.primary_key.is_empty() {
                    "None".to_string()
                } else {
                    r.primary_key.join(", ")
                };
                c += &format!(
                    "**{}**\n  - Attributes: {{{}}}\n  - Primary Key: {{{pk}}}\n",
                    r.name,
                    r.attributes.join(", ")
                );
                if !r.foreign_keys.is_empty() {
                    c += &format!("  - Foreign Keys: {}\n", r.foreign_keys.len());
                }
                c += &format!("  - Source: {}\n", r.source());
            }
            c += "\n";
        }
    }
    if has(ops, "Lossless Check") {
        c += &format!(
            "### Lossless Join Check\n\n**Status:** {}\n",
            if lossless { "PASSED" } else { "FAILED" }
        );
        if let Some(d) = i.lossless.get("details").filter(|d| d.is_object()) {
            c += &format!(
                "**Algorithm:** {}\n**Relations Tested:** {}\n**Dependencies Used:** {}\n",
                d["algorithm"].as_str().unwrap_or(""),
                crate::table::json_to_js_string(&d["relationCount"]),
                crate::table::json_to_js_string(&d["fdCount"])
            );
        }
        if lossless {
            c += "**Result:** Decomposition preserves all information\n**Implication:** Natural joins will reconstruct original data perfectly\n";
        } else {
            c += "**Result:** Information may be lost during join operations\n**Recommendation:** Review decomposition strategy\n";
        }
        c += "\n";
    }
    let preserved = i.preservation["isPreserved"].as_bool().unwrap_or(false);
    if has(ops, "Dependency Preservation") {
        c += &format!(
            "### Dependency Preservation\n\n**Status:** {}\n",
            if preserved { "PRESERVED" } else { "PARTIALLY PRESERVED" }
        );
        if !i.preservation["totalDependencies"].is_null() {
            c += &format!("**Preservation Rate:** {}%\n", pct1(ratio.unwrap_or(f64::NAN)));
            c += &format!(
                "**Dependencies Preserved:** {}/{}\n",
                crate::table::json_to_js_string(&i.preservation["preservedCount"]),
                crate::table::json_to_js_string(&i.preservation["totalDependencies"])
            );
            if i.preservation["lostCount"].as_f64().unwrap_or(0.0) > 0.0 {
                c += &format!(
                    "**Dependencies Lost:** {}\n**Implication:** Some integrity constraints may require joins to enforce\n",
                    crate::table::json_to_js_string(&i.preservation["lostCount"])
                );
            }
        }
        if preserved {
            c += "**Result:** All functional dependencies are preserved\n**Benefit:** Integrity constraints can be enforced without joins\n";
        }
        c += "\n";
    }
    if has(ops, "ERD Generator") {
        c += "### Entity Relationship Diagram Generator\n\n";
        if i.r3.is_empty() {
            c += "**Status:** No relations available for ERD generation\n\n";
        } else {
            let connected =
                i.r3.iter()
                    .filter(|r| {
                        !r.foreign_keys.is_empty()
                            || i.r3
                                .iter()
                                .any(|o| o.foreign_keys.iter().any(|fk| fk.references == r.name))
                    })
                    .count();
            c += &format!(
                "**Status:** ERD generated successfully\n**Format:** Mermaid diagram syntax\n**Entities:** {}\n**Connected Entities:** {connected}\n**File:** erd_diagram.mmd\n\n",
                i.r3.len()
            );
        }
    }
    if has(ops, "Relation Generator") {
        c += &format!(
            "### Relation Schema Generator\n\n**Status:** Relation schemas generated\n**Format:** Detailed markdown documentation\n**Relations Documented:** {}\n**Includes:** Primary keys, foreign keys, attribute types, row counts\n**File:** relation_schemas.md\n\n",
            i.r3.len()
        );
    }
    if has(ops, "SQL Generator") {
        c += "### SQL DDL Generator\n\n";
        if i.r3.is_empty() {
            c += "**Status:** No relations available for SQL generation\n\n";
        } else {
            let fks: usize = i.r3.iter().map(|r| r.foreign_keys.len()).sum();
            c += &format!(
                "**Status:** SQL DDL generated successfully\n**Database:** CREATE TABLE statements for all relations\n**Tables:** {}\n**Foreign Key Constraints:** {fks}\n**Indexes:** Automatically generated for foreign keys\n**File:** database_schema.sql\n\n",
                i.r3.len()
            );
        }
    }
    let pm = i.performance;
    c += &format!(
        "### Performance Analysis\n\n**Total Processing Time:** {}\n**Peak Memory Usage:** {} MB\n",
        pm["totalDuration"].as_str().unwrap_or(""),
        js::to_fixed(pm["peakMemoryUsage"].as_f64().unwrap_or(0.0), 2)
    );
    let opts = pm["optimizationsApplied"].as_array().cloned().unwrap_or_default();
    c += &format!("**Optimizations Applied:** {}\n", opts.len());
    for o in &opts {
        c += &format!("- {}\n", o.as_str().unwrap_or(""));
    }
    let total = pm["checkpoints"]
        .as_array()
        .and_then(|a| a.last())
        .and_then(|c| c["time"].as_f64())
        .unwrap_or(0.0);
    c += if total < 10_000.0 {
        "**Performance Rating:** Excellent (under 10 seconds)\n"
    } else if total < 30_000.0 {
        "**Performance Rating:** Good (under 30 seconds)\n"
    } else {
        "**Performance Rating:** Adequate (consider optimization for larger datasets)\n"
    };
    c += "\n## Actionable Recommendations\n\n";
    let mut recs: Vec<String> = Vec::new();
    if has(ops, "Key Discovery") {
        let k = &i.keys.candidate_keys;
        if !k.is_empty() {
            if let Some(s) = k.iter().find(|x| x.len() == 1) {
                recs.push(format!(
                    "**Primary Key Implementation:** Use {{{}}} as your primary key for optimal performance",
                    s.join(", ")
                ));
            } else {
                let mut best = &k[0];
                for x in k {
                    if x.len() < best.len() {
                        best = x;
                    }
                }
                recs.push(format!(
                    "**Composite Key Implementation:** Use {{{}}} as your primary key, but consider a surrogate key for better performance",
                    best.join(", ")
                ));
            }
        } else {
            recs.push("**Key Strategy:** Add an auto-increment ID column as a surrogate primary key".into());
        }
    }
    if has(ops, "FD Miner") && !i.fds.is_empty() {
        let high = i.fds.iter().filter(|f| f.conf() > 0.98).count();
        if high > 0 {
            recs.push(format!("**Normalization Priority:** Focus on the {high} high-confidence functional dependencies for reliable normalization"));
        }
        let low = i.fds.iter().filter(|f| f.conf() < 0.90).count();
        if low > 0 {
            recs.push(format!(
                "**Data Quality:** Review {low} low-confidence dependencies - they may indicate data quality issues"
            ));
        }
    }
    if has(ops, "Lossless Check") && !lossless {
        recs.push("**Schema Review Required:** Decomposition is not lossless - information may be lost during joins. Consider revising the normalization strategy".into());
    }
    if has(ops, "Dependency Preservation") && !preserved {
        let lost = i.preservation["lostCount"].as_f64().unwrap_or(f64::NAN);
        let total = i.preservation["totalDependencies"].as_f64().unwrap_or(f64::NAN);
        recs.push(format!(
            "**Integrity Constraints:** {}% of functional dependencies are lost. You may need complex join operations to enforce some business rules",
            js::to_fixed(lost / total * 100.0, 1)
        ));
    }
    if has(ops, "SQL Generator") {
        recs.push(
            "**Database Implementation:** Use the generated SQL DDL to create your normalized database schema".into(),
        );
        recs.push(
            "**Index Strategy:** Implement the suggested foreign key indexes for optimal query performance".into(),
        );
    }
    if has(ops, "ERD Generator") {
        recs.push("**Documentation:** Use the generated ERD for database documentation and team communication".into());
    }
    if has(ops, "3NF") {
        recs.push("**Query Optimization:** Monitor join performance in your normalized schema and consider materialized views for frequently accessed data".into());
        recs.push("**Backup Strategy:** Implement regular backups focusing on your core entity tables first".into());
    }
    if recs.is_empty() {
        recs.push("**Next Steps:** Review the individual operation results above for specific recommendations".into());
    }
    for (n, r) in recs.iter().enumerate() {
        c += &format!("{}. {r}\n", n + 1);
    }
    c += "\n## Next Steps\n\n";
    let mut steps: Vec<&str> = Vec::new();
    if has(ops, "SQL Generator") {
        steps.push("**Database Creation:** Execute the generated SQL DDL to create your normalized database schema");
    }
    if has(ops, "3NF") && !i.r3.is_empty() {
        steps.push("**Data Migration:** Plan the migration of your existing data into the new normalized structure");
        steps.push("**Application Updates:** Update your application code to work with the new table structure");
    }
    if has(ops, "ERD Generator") {
        steps.push("**Team Review:** Share the ERD with your development team for review and approval");
    }
    steps.push("**Testing:** Thoroughly test the new schema with representative data and queries");
    steps.push("**Performance Monitoring:** Monitor query performance and adjust indexes as needed");
    steps.push("**Documentation:** Update your database documentation with the new schema");
    for (n, s) in steps.iter().enumerate() {
        c += &format!("{}. {s}\n", n + 1);
    }
    c += "\n---\n*Report generated by DBFlow Database Normalization Engine*\n";
    c
}

// ---------------------------------------------------------------------------
// Multi-file report
// ---------------------------------------------------------------------------

fn code(s: &str) -> String {
    let t = s.replace('`', "'").replace("\r\n", " ").replace('\n', " ");
    format!("`{t}`")
}

fn list(xs: &[String]) -> String {
    xs.iter().map(|x| code(x)).collect::<Vec<_>>().join(", ")
}

pub struct MultiTableEntry<'a> {
    pub table_name: &'a str,
    pub file_name: &'a str,
    pub row_count: usize,
    pub analysis: &'a TableAnalysis,
}

pub fn multi_table_report(
    tables: &[MultiTableEntry],
    links: &[CrossLink],
    schema: &BuiltSchema,
    dialect: &str,
) -> String {
    let mut out: Vec<String> = vec!["# Multi-CSV Database Normalization Report".into(), String::new()];
    out.push(format!(
        "**Files**: {}",
        tables
            .iter()
            .map(|t| md_escape(t.file_name))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.push(format!("**Tables analysed**: {}", tables.len()));
    out.push(format!("**Relations after normalization**: {}", schema.relations.len()));
    out.push(format!("**Cross-table foreign keys**: {}", links.len()));
    out.push(format!("**SQL dialect**: {}", md_escape(dialect)));
    out.push(String::new());
    out.push("## 1. Per-table analysis".into());
    out.push(String::new());
    for t in tables {
        let a = t.analysis;
        out.push(format!(
            "### {} ({} rows, {} columns)",
            code(t.table_name),
            t.row_count,
            a.attributes.len()
        ));
        out.push(String::new());
        out.push(format!("- **Source file**: {}", md_escape(t.file_name)));
        if let Some(k) = &a.key_choice {
            out.push(format!(
                "- **Primary key**: {} — {}",
                list(&k.primary_key),
                md_escape(&k.reason)
            ));
        }
        if !a.key_result.candidate_keys.is_empty() {
            out.push(format!(
                "- **Candidate keys**: {}",
                a.key_result
                    .candidate_keys
                    .iter()
                    .map(|k| format!("{{{}}}", list(k)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if let Some(s) = &a.fd_search {
            out.push(format!(
                "- **Functional dependencies**: {} exact (used), {} approximate ({})",
                s.exact,
                s.approximate,
                if s.approximate_used { "used" } else { "not used" }
            ));
            if s.truncated {
                out.push(format!(
                    "- **Search limit**: stopped after determinants of size {}",
                    s.level_reached
                ));
            }
        }
        let fin = if !a.relations_3nf.is_empty() {
            &a.relations_3nf
        } else {
            &a.relations_2nf
        };
        if !fin.is_empty() {
            out.push("- **Normalized relations**:".into());
            for r in fin {
                out.push(format!(
                    "  - {}: {} (key: {})",
                    code(&r.name),
                    list(&r.attributes),
                    list(&r.primary_key)
                ));
            }
        }
        if let Some(chase) = a.lossless_check.get("chaseLossless").and_then(|v| v.as_bool()) {
            let inst = &a.lossless_check["instanceLossless"];
            let txt = match inst.as_bool() {
                None => "not decided on data",
                Some(true) => "confirmed on the data",
                Some(false) => "NOT reproduced by the data",
            };
            out.push(format!(
                "- **Lossless join**: {} by the chase; {txt}",
                if chase { "yes" } else { "no" }
            ));
        }
        if let Some(pc) = a.dependency_preservation.get("preservedCount") {
            let d = &a.dependency_preservation;
            let lost = d["lostCount"].as_f64().unwrap_or(0.0);
            out.push(format!(
                "- **Dependency preservation**: {}/{} preserved{}",
                crate::table::json_to_js_string(pc),
                crate::table::json_to_js_string(&d["totalDependencies"]),
                if lost != 0.0 {
                    format!(" ({} lost)", js::num_to_string(lost))
                } else {
                    String::new()
                }
            ));
        }
        for n in &a.notes {
            out.push(format!("- ⚠️ {}", md_escape(n)));
        }
        out.push(String::new());
    }
    out.push("## 2. Cross-table relationships".into());
    out.push(String::new());
    if !links.is_empty() {
        out.push("| From | To | Cardinality | Nullable | Containment | Confidence |".into());
        out.push("| :--- | :--- | :--- | :--- | :--- | :--- |".into());
        for l in links {
            out.push(format!(
                "| {} | {} | {} | {} | {}% | {}% |",
                code(&format!("{}.{}", l.source_table, l.src_cols().join(" + "))),
                code(&format!("{}.{}", l.target_table, l.tgt_cols().join(" + "))),
                l.cardinality,
                if l.optional == Some(true) { "yes" } else { "no" },
                pct1(l.match_ratio),
                pct1(l.confidence)
            ));
        }
    } else {
        out.push("*No foreign keys were detected between the files.*".into());
    }
    out.push(String::new());
    if !schema.junctions.is_empty() {
        out.push("### Many-to-many relationships (junction tables)".into());
        out.push(String::new());
        for j in &schema.junctions {
            out.push(format!(
                "- {} links {}",
                code(&j.relation),
                j.between.iter().map(|b| code(b)).collect::<Vec<_>>().join(" ↔ ")
            ));
        }
        out.push(String::new());
    }
    if !schema.unplaced.is_empty() {
        out.push("### Links that could not be placed".into());
        out.push(String::new());
        for u in &schema.unplaced {
            out.push(format!(
                "- {} → {}: {}",
                code(&format!("{}.{}", u.link.source_table, u.link.src_cols().join(" + "))),
                code(&format!("{}.{}", u.link.target_table, u.link.tgt_cols().join(" + "))),
                md_escape(&u.reason)
            ));
        }
        out.push(String::new());
    }
    out.push("## 3. Final schema".into());
    out.push(String::new());
    out.push("| Relation | Columns | Primary key | Foreign keys |".into());
    out.push("| :--- | :--- | :--- | :--- |".into());
    for r in &schema.relations {
        let fks = r
            .foreign_keys
            .iter()
            .map(|fk| format!("{} → {}", list(&fk.attributes), code(&fk.references)))
            .collect::<Vec<_>>()
            .join("; ");
        let fks = if fks.is_empty() { "—".to_string() } else { fks };
        out.push(format!(
            "| {} | {} | {} | {fks} |",
            code(&r.name),
            list(&r.attributes),
            list(&r.primary_key)
        ));
    }
    out.push(String::new());
    out.join("\n")
}
