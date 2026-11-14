//! Chen, Barker and UML ER diagrams as Graphviz DOT.

use crate::types::Relation;
use std::collections::{HashMap, HashSet};

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace("\r\n", " ")
        .replace('\n', " ")
}

fn html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn is_junction(r: &Relation) -> bool {
    let fks: Vec<_> = r.foreign_keys.iter().filter(|f| !f.references.is_empty()).collect();
    if fks.len() < 2 {
        return false;
    }
    let cols: HashSet<&String> = fks.iter().flat_map(|f| f.attributes.iter()).collect();
    r.primary_key.len() >= 2 && r.primary_key.iter().all(|c| cols.contains(c))
}

pub fn is_weak(r: &Relation) -> bool {
    let cols: HashSet<&String> = r.foreign_keys.iter().flat_map(|f| f.attributes.iter()).collect();
    !is_junction(r) && r.primary_key.iter().any(|c| cols.contains(c)) && r.primary_key.iter().any(|c| !cols.contains(c))
}

pub fn build_chen_dot(relations: &[Relation], attributes: bool, tree: bool) -> String {
    let known: HashSet<&String> = relations.iter().map(|r| &r.name).collect();
    let mut entity_id: HashMap<String, String> = HashMap::new();
    let mut counter = 0;
    let mut next = |p: &str| {
        let s = format!("{p}{counter}");
        counter += 1;
        s
    };
    let mut lines = vec![
        "graph ERD {".to_string(),
        format!(
            "  graph [overlap=false, splines=true, sep=\"+8\", outputorder=edgesfirst, pad=0.3, bgcolor=\"white\", pack=true, packmode=\"node\"{}];",
            if tree { ", rankdir=LR, nodesep=0.25, ranksep=0.6" } else { "" }
        ),
        "  node [fontname=\"Helvetica\", fontsize=11, color=\"#374151\", fontcolor=\"#111827\"];".into(),
        "  edge [color=\"#6b7280\", fontname=\"Helvetica\", fontsize=10, fontcolor=\"#0f766e\"];".into(),
    ];
    let junctions: Vec<&Relation> = relations.iter().filter(|r| is_junction(r)).collect();
    let entities: Vec<&Relation> = relations.iter().filter(|r| !is_junction(r)).collect();
    for e in &entities {
        let id = next("e");
        entity_id.insert(e.name.clone(), id.clone());
        lines.push(format!(
            "  {id} [label=\"{}\", shape=box, style=\"filled\", fillcolor=\"#ecfdf5\"{}, penwidth=1.4];",
            esc(&e.name),
            if is_weak(e) { ", peripheries=2" } else { "" }
        ));
    }
    let attr_node =
        |lines: &mut Vec<String>, next: &mut dyn FnMut(&str) -> String, owner: &str, attr: &str, key: bool| {
            let id = next("a");
            let label = if key {
                format!("<<U>{}</U>>", html(attr))
            } else {
                format!("\"{}\"", esc(attr))
            };
            lines.push(format!(
                "  {id} [label={label}, shape=ellipse, style=\"filled\", fillcolor=\"#ffffff\", fontsize=10];"
            ));
            lines.push(format!("  {owner} -- {id};"));
        };
    let relationship = |lines: &mut Vec<String>,
                        next: &mut dyn FnMut(&str) -> String,
                        label: &str,
                        ends: &[(String, &str)],
                        identifying: bool| {
        let id = next("r");
        lines.push(format!(
            "  {id} [label=\"{}\", shape=diamond, style=\"filled\", fillcolor=\"#fef9c3\"{}];",
            esc(label),
            if identifying { ", peripheries=2" } else { "" }
        ));
        for (end, card) in ends {
            lines.push(format!(
                "  {id} -- {end} [headlabel=\"{card}\", labeldistance=3.6, labelangle=-55, labelfontsize=11, labelfontcolor=\"#0f766e\", penwidth=1.2];"
            ));
        }
        id
    };
    for e in &entities {
        let id = entity_id[&e.name].clone();
        let fk_cols: HashSet<&String> = e.foreign_keys.iter().flat_map(|f| f.attributes.iter()).collect();
        if attributes {
            for a in &e.attributes {
                if fk_cols.contains(a) {
                    continue;
                }
                attr_node(&mut lines, &mut next, &id, a, e.primary_key.contains(a));
            }
        }
        for fk in &e.foreign_keys {
            let Some(target) = entity_id.get(&fk.references).cloned() else {
                continue;
            };
            let pk = &e.primary_key;
            let one = !pk.is_empty()
                && pk.iter().all(|c| fk.attributes.contains(c))
                && fk.attributes.iter().all(|c| pk.contains(c));
            let identifying = is_weak(e) && fk.attributes.iter().any(|c| pk.contains(c));
            relationship(
                &mut lines,
                &mut next,
                &fk.attributes.join(", "),
                &[(target, "1"), (id.clone(), if one { "1" } else { "N" })],
                identifying,
            );
        }
    }
    for j in &junctions {
        let fks: Vec<_> = j
            .foreign_keys
            .iter()
            .filter(|f| entity_id.contains_key(&f.references))
            .collect();
        if fks.len() < 2 {
            continue;
        }
        let ends: Vec<(String, &str)> = fks
            .iter()
            .enumerate()
            .map(|(i, f)| (entity_id[&f.references].clone(), if i == 0 { "M" } else { "N" }))
            .collect();
        let rel = relationship(&mut lines, &mut next, &j.name, &ends, false);
        if attributes {
            let cols: HashSet<&String> = j.foreign_keys.iter().flat_map(|f| f.attributes.iter()).collect();
            for a in &j.attributes {
                if !cols.contains(a) {
                    attr_node(&mut lines, &mut next, &rel, a, j.primary_key.contains(a));
                }
            }
        }
    }
    let dangling: Vec<String> = relations
        .iter()
        .flat_map(|r| {
            r.foreign_keys
                .iter()
                .filter(|f| !f.references.is_empty() && !known.contains(&f.references))
                .map(move |f| format!("{} -> {}", r.name, f.references))
        })
        .collect();
    if !dangling.is_empty() {
        lines.push(format!(
            "  // references to relations not in this diagram: {}",
            dangling
                .iter()
                .map(|d| d.replace(['\r', '\n'], " "))
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    lines.push("}".into());
    lines.join("\n")
}

pub fn attribute_count(relations: &[Relation]) -> usize {
    relations.iter().map(|r| r.attributes.len()).sum()
}

pub fn optional_columns(r: &Relation) -> HashSet<String> {
    let mut out = HashSet::new();
    let n = r.rows.len();
    let step = (n / 2000).max(1);
    let mut i = 0;
    while i < n {
        for a in &r.attributes {
            if r.rows.get(i, a).is_empty_like() {
                out.insert(a.clone());
            }
        }
        i += step;
    }
    out
}

struct Edge {
    parent: String,
    child: String,
    columns: Vec<String>,
    optional: bool,
    identifying: bool,
    one_to_one: bool,
}

fn edges_of(relations: &[Relation]) -> Vec<Edge> {
    let names: HashSet<&String> = relations.iter().map(|r| &r.name).collect();
    let mut edges = Vec::new();
    for r in relations {
        let opt = optional_columns(r);
        for fk in &r.foreign_keys {
            if fk.references.is_empty() || !names.contains(&fk.references) || fk.references == r.name {
                continue;
            }
            let pk = &r.primary_key;
            edges.push(Edge {
                parent: fk.references.clone(),
                child: r.name.clone(),
                columns: fk.attributes.clone(),
                optional: fk.attributes.iter().any(|c| opt.contains(c)),
                identifying: fk.attributes.iter().any(|c| pk.contains(c)),
                one_to_one: !pk.is_empty()
                    && pk.len() == fk.attributes.len()
                    && pk.iter().all(|c| fk.attributes.contains(c)),
            });
        }
    }
    edges
}

fn head(kind: &str, count: usize) -> Vec<String> {
    vec![
        format!("{kind} ERD {{"),
        format!(
            "  graph [rankdir={}, nodesep=0.4, ranksep=0.8, pad=0.3, bgcolor=\"white\", splines=true, pack=true];",
            if count > 10 { "LR" } else { "TB" }
        ),
        "  node [shape=plaintext, fontname=\"Helvetica\", fontsize=11];".into(),
        "  edge [color=\"#6b7280\", fontname=\"Helvetica\", fontsize=9, fontcolor=\"#0f766e\"];".into(),
    ]
}

pub fn build_barker_dot(relations: &[Relation], attributes: bool) -> String {
    let mut lines = head("digraph", relations.len());
    let ids: HashMap<&String, String> = relations
        .iter()
        .enumerate()
        .map(|(i, r)| (&r.name, format!("e{i}")))
        .collect();
    for r in relations {
        let pk = &r.primary_key;
        let fk_cols: HashSet<&String> = r.foreign_keys.iter().flat_map(|f| f.attributes.iter()).collect();
        let opt = optional_columns(r);
        let rows: String = if attributes {
            r.attributes
                .iter()
                .filter(|a| pk.contains(a) || !fk_cols.contains(a))
                .map(|a| {
                    let mark = if pk.contains(a) {
                        "#&nbsp;*"
                    } else if opt.contains(a) {
                        "o&nbsp;&nbsp;"
                    } else {
                        "*&nbsp;&nbsp;"
                    };
                    format!("<TR><TD ALIGN=\"LEFT\">{mark} {}</TD></TR>", html(a))
                })
                .collect()
        } else {
            String::new()
        };
        lines.push(format!(
            "  {} [label=<<TABLE STYLE=\"ROUNDED\" BORDER=\"1\" CELLBORDER=\"0\" CELLSPACING=\"0\" CELLPADDING=\"4\" COLOR=\"#374151\" BGCOLOR=\"#ecfdf5\"><TR><TD><B>{}</B></TD></TR>{}</TABLE>>];",
            ids[&r.name],
            html(&r.name.to_uppercase()),
            if rows.is_empty() { String::new() } else { format!("<HR/>{rows}") }
        ));
    }
    for e in edges_of(relations) {
        let foot = if e.one_to_one {
            "none"
        } else if e.identifying {
            "crowtee"
        } else {
            "crow"
        };
        lines.push(format!(
            "  {} -> {} [dir=both, arrowtail=none, arrowhead={foot}, {}label=\"{}\"];",
            ids[&e.parent],
            ids[&e.child],
            if e.optional { "style=dashed, " } else { "" },
            esc(&e.columns.join(", "))
        ));
    }
    lines.push("}".into());
    lines.join("\n")
}

pub fn build_uml_dot(relations: &[Relation], attributes: bool) -> String {
    let mut lines = head("graph", relations.len());
    let ids: HashMap<&String, String> = relations
        .iter()
        .enumerate()
        .map(|(i, r)| (&r.name, format!("c{i}")))
        .collect();
    for r in relations {
        let pk: HashSet<&String> = r.primary_key.iter().collect();
        let fk: HashSet<&String> = r.foreign_keys.iter().flat_map(|f| f.attributes.iter()).collect();
        let types = &r.metadata.attribute_types;
        let rows: String = if attributes {
            r.attributes
                .iter()
                .map(|a| {
                    let name = if pk.contains(a) {
                        format!("<U>{}</U>", html(a))
                    } else {
                        html(a)
                    };
                    let tags: Vec<String> = [pk.contains(a).then_some("PK"), fk.contains(a).then_some("FK")]
                        .into_iter()
                        .flatten()
                        .map(|t| format!("&#171;{t}&#187;"))
                        .collect();
                    let ty = types
                        .get(a)
                        .filter(|t| !t.is_empty())
                        .map(|t| format!(" : {}", html(t)))
                        .unwrap_or_default();
                    let tag = if tags.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", tags.join(" "))
                    };
                    format!("<TR><TD ALIGN=\"LEFT\">+ {name}{ty}{tag}</TD></TR>")
                })
                .collect()
        } else {
            String::new()
        };
        lines.push(format!(
            "  {} [label=<<TABLE BORDER=\"1\" CELLBORDER=\"0\" CELLSPACING=\"0\" CELLPADDING=\"4\" COLOR=\"#374151\" BGCOLOR=\"#ffffff\"><TR><TD BGCOLOR=\"#e0f2f1\"><B>{}</B></TD></TR>{}</TABLE>>];",
            ids[&r.name],
            html(&r.name),
            if rows.is_empty() { String::new() } else { format!("<HR/>{rows}") }
        ));
    }
    for e in edges_of(relations) {
        let parent = if e.optional { "0..1" } else { "1" };
        let child = if e.one_to_one {
            if e.optional {
                "0..1"
            } else {
                "1"
            }
        } else if e.optional || !e.identifying {
            "0..*"
        } else {
            "1..*"
        };
        lines.push(format!(
            "  {} -- {} [taillabel=\"{parent}\", headlabel=\"{child}\", labeldistance=2.2, labelangle=-30, labelfontsize=10, label=\"{}\"];",
            ids[&e.parent],
            ids[&e.child],
            esc(&e.columns.join(", "))
        ));
    }
    lines.push("}".into());
    lines.join("\n")
}
