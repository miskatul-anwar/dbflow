//! Relation ids and names (source, user, model or placeholder).

use crate::identifiers::{sanitize_identifier, IdentifierRegistry};
use crate::js;
use crate::types::{same_set, Relation};
use indexmap::IndexMap;
use std::cell::Cell as StdCell;
use std::collections::HashMap;

thread_local! {
    static COUNTER: StdCell<u64> = const { StdCell::new(0) };
}

/// Start numbering again (once per analysis run).
pub fn reset_relation_ids() {
    COUNTER.with(|c| c.set(0));
}

/// An opaque id for a new relation.
pub fn new_relation_id() -> String {
    COUNTER.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        format!("relation_{n}")
    })
}

fn norm(s: &str) -> String {
    js::trim(s).to_lowercase()
}

/// Identifies a relation by what it contains.
pub fn relation_signature(table_name: &str, attributes: &[String], primary_key: &[String]) -> String {
    let mut attrs: Vec<String> = attributes.iter().map(|a| norm(a)).collect();
    js::sort_strings(&mut attrs);
    let mut key: Vec<String> = primary_key.iter().map(|a| norm(a)).collect();
    js::sort_strings(&mut key);
    format!("{}|{}|{}", norm(table_name), attrs.join(","), key.join(","))
}

/// Validates a model answer; `None` when it is not a usable name.
pub fn clean_model_name(raw: &str, columns: &[String]) -> Option<String> {
    let first = re!(r"\r?\n").split(raw).next().unwrap_or("");
    let s = re!(r#"^["'`“”‘’\s]+|["'`“”‘’.!?:;,\s]+$"#).replace_all(first, "");
    let s = re!(r"\s+").replace_all(&s, " ");
    let s = js::trim(&s).to_string();
    let s = re!(r"(?i)\s+table$").replace(&s, "").to_string();
    let len = js::len16(&s);
    if !(2..=40).contains(&len) {
        return None;
    }
    if re!(r"[0-9:=,;()\[\]{}<>|/\\]").is_match(&s) {
        return None;
    }
    if s.split(' ').count() > 4 {
        return None;
    }
    if re!(r"(?i)^(the|a|an|this|these|one|there|it|here|name|table)\b").is_match(&s) {
        return None;
    }
    let compact = |x: &str| {
        x.to_lowercase()
            .chars()
            .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            .collect::<String>()
    };
    if columns.iter().any(|c| compact(c) == compact(&s)) {
        return None;
    }
    if !re!(r"[A-Za-z]").is_match(&s) {
        return None;
    }
    let mut chars = s.chars();
    let first = chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default();
    Some(format!("{first}{}", chars.as_str()))
}

#[derive(Clone, Debug)]
pub struct ChosenName {
    pub text: String,
    pub source: String,
}

/// Index of the relation that keeps the table's own name.
pub fn main_of(set: &[Relation], primary_key: &[String]) -> Option<usize> {
    let held = |r: &Relation| primary_key.iter().filter(|k| r.attributes.contains(k)).count() as i64;
    let mut idx: Vec<usize> = (0..set.len()).collect();
    idx.sort_by(|&a, &b| {
        let (ra, rb) = (&set[a], &set[b]);
        (held(rb) - held(ra))
            .cmp(&0)
            .then(
                (same_set(&rb.primary_key, primary_key) as i64 - same_set(&ra.primary_key, primary_key) as i64).cmp(&0),
            )
            .then((rb.primary_key.len() as i64 - ra.primary_key.len() as i64).cmp(&0))
            .then((rb.attributes.len() as i64 - ra.attributes.len() as i64).cmp(&0))
    });
    idx.first().copied()
}

/// Gives every relation in one set its final name and rewrites foreign-key targets.
pub fn name_relation_set(
    set: &mut [Relation],
    table_name: &str,
    primary_key: &[String],
    chosen: Option<&HashMap<String, ChosenName>>,
    registry: &mut IdentifierRegistry,
) {
    if set.is_empty() {
        return;
    }
    let main = main_of(set, primary_key);
    let decided: Vec<(String, String)> = set
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if Some(i) == main {
                return (table_name.to_string(), "source".to_string());
            }
            let src = r.name_source.as_deref();
            if (src == Some("model") || src == Some("user")) && r.display_name.as_deref().is_some_and(|d| !d.is_empty())
            {
                return (
                    r.display_name.clone().unwrap_or_default(),
                    src.unwrap_or_default().to_string(),
                );
            }
            if let Some(pick) =
                chosen.and_then(|c| c.get(&relation_signature(table_name, &r.attributes, &r.primary_key)))
            {
                return (pick.text.clone(), pick.source.clone());
            }
            (r.name.clone(), "placeholder".to_string())
        })
        .collect();
    let mut rename: IndexMap<String, String> = IndexMap::new();
    for (r, (text, _)) in set.iter().zip(&decided) {
        rename.insert(r.name.clone(), registry.fresh(&sanitize_identifier(text)));
    }
    for r in set.iter_mut() {
        for fk in &mut r.foreign_keys {
            if let Some(n) = rename.get(&fk.references) {
                fk.references = n.clone();
            }
        }
    }
    for (r, (text, source)) in set.iter_mut().zip(decided) {
        r.display_name = if source == "placeholder" { None } else { Some(text) };
        r.name_source = Some(source);
        r.source_table = Some(table_name.to_string());
        if let Some(n) = rename.get(&r.name) {
            r.name = n.clone();
        }
    }
}

/// One table's relation sets for naming. `final_set` indexes into `sets`; `None` means an empty final set.
pub struct NamableTable<'a> {
    pub table_name: String,
    pub primary_key: Vec<String>,
    pub final_set: Option<&'a [Relation]>,
    pub others: Vec<&'a [Relation]>,
}

/// Collect user-typed names and the questions for the model.
pub fn plan_names(
    tables: &[NamableTable],
    user_names: &HashMap<String, String>,
) -> (HashMap<String, ChosenName>, IndexMap<String, Vec<String>>) {
    let mut chosen = HashMap::new();
    let mut to_ask: IndexMap<String, Vec<String>> = IndexMap::new();
    for t in tables {
        let fin = t.final_set.unwrap_or(&[]);
        let main = main_of(fin, &t.primary_key);
        for (i, r) in fin.iter().enumerate() {
            if Some(i) == main {
                continue;
            }
            let sig = relation_signature(&t.table_name, &r.attributes, &r.primary_key);
            let typed = user_names
                .get(&sig)
                .map(|s| js::trim(s).to_string())
                .filter(|s| !s.is_empty());
            if let Some(typed) = typed {
                chosen.insert(
                    sig,
                    ChosenName {
                        text: typed,
                        source: "user".into(),
                    },
                );
            } else if !to_ask.contains_key(&sig) {
                to_ask.insert(sig, r.attributes.clone());
            }
        }
        for set in &t.others {
            for r in set.iter() {
                let sig = relation_signature(&t.table_name, &r.attributes, &r.primary_key);
                let typed = user_names
                    .get(&sig)
                    .map(|s| js::trim(s).to_string())
                    .filter(|s| !s.is_empty());
                if let Some(typed) = typed {
                    chosen.entry(sig).or_insert(ChosenName {
                        text: typed,
                        source: "user".into(),
                    });
                }
            }
        }
    }
    (chosen, to_ask)
}

/// Fold model answers into the chosen names.
pub fn accept_model_names(
    chosen: &mut HashMap<String, ChosenName>,
    to_ask: &IndexMap<String, Vec<String>>,
    answers: &HashMap<String, String>,
) {
    for (sig, cols) in to_ask {
        let raw = answers.get(sig).map(|s| s.as_str()).unwrap_or("");
        if let Some(text) = clean_model_name(raw, cols) {
            chosen.insert(
                sig.clone(),
                ChosenName {
                    text,
                    source: "model".into(),
                },
            );
        }
    }
}
