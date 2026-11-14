//! One SQL type-inference spec for generated DDL.

use crate::js;
use crate::table::{Cell, Table};
use indexmap::IndexMap;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InferredColumn {
    pub sql_type: String,
    pub not_null: bool,
}

const VARCHAR_STEPS: [usize; 7] = [16, 32, 64, 128, 255, 500, 1000];

pub fn infer_column_sql_type(values: &[&Cell]) -> InferredColumn {
    let present: Vec<std::borrow::Cow<'_, str>> = values
        .iter()
        .filter(|v| !v.is_missing())
        .map(|v| match v.text() {
            std::borrow::Cow::Borrowed(s) => std::borrow::Cow::Borrowed(js::trim(s)),
            std::borrow::Cow::Owned(s) => std::borrow::Cow::Owned(js::trim(&s).to_string()),
        })
        .collect();
    let not_null = present.len() == values.len() && !values.is_empty();
    if present.is_empty() {
        return InferredColumn {
            sql_type: "VARCHAR(255)".into(),
            not_null: false,
        };
    }
    fn digits(s: &str) -> bool {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
    fn int_like(v: &str) -> bool {
        let d = v.strip_prefix('-').unwrap_or(v);
        digits(d) && d.len() <= 18
    }
    fn decimal_like(v: &str) -> bool {
        let d = v.strip_prefix('-').unwrap_or(v);
        match d.split_once('.') {
            Some((i, f)) => digits(i) && digits(f),
            None => digits(d),
        }
    }
    fn date_like(v: &str) -> bool {
        let b = v.as_bytes();
        b.len() == 10
            && b[4] == b'-'
            && b[7] == b'-'
            && b.iter()
                .enumerate()
                .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    }
    let no_leading_zero = |v: &str| {
        let d = v.strip_prefix('-').unwrap_or(v).as_bytes();
        !(d.len() >= 2 && d[0] == b'0' && d[1].is_ascii_digit())
    };
    let all = |r: &regex::Regex| present.iter().all(|v| r.is_match(v));
    let all_fn = |f: fn(&str) -> bool| present.iter().all(|v| f(v));

    if all_fn(int_like) && present.iter().all(|v| no_leading_zero(v)) {
        let big = present.iter().any(|v| js::number(v).abs() > 2_147_483_647.0);
        return InferredColumn {
            sql_type: if big { "BIGINT" } else { "INTEGER" }.into(),
            not_null,
        };
    }
    if all_fn(decimal_like) && present.iter().all(|v| no_leading_zero(v)) {
        let mut int_digits = 1;
        let mut scale = 0;
        for v in &present {
            let v = v.replacen('-', "", 1);
            let mut parts = v.split('.');
            let i = parts.next().unwrap_or("");
            let f = parts.next().unwrap_or("");
            int_digits = int_digits.max(i.len());
            scale = scale.max(f.len());
        }
        let precision = (int_digits + 2 + scale).min(38);
        return InferredColumn {
            sql_type: format!("DECIMAL({precision},{})", scale.min(precision)),
            not_null,
        };
    }
    if all_fn(date_like) {
        return InferredColumn {
            sql_type: "DATE".into(),
            not_null,
        };
    }
    if all(re!(
        r"^[0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9]{2}:[0-9]{2}(:[0-9]{2}(\.[0-9]+)?)?(Z|[+-][0-9]{2}:?[0-9]{2})?$"
    )) {
        return InferredColumn {
            sql_type: "TIMESTAMP".into(),
            not_null,
        };
    }
    if all(re!(r"(?i)^(true|false)$")) {
        return InferredColumn {
            sql_type: "BOOLEAN".into(),
            not_null,
        };
    }
    let longest = present.iter().map(|v| js::len16(v)).max().unwrap_or(0);
    let sql_type = match VARCHAR_STEPS.iter().find(|&&s| longest <= s) {
        Some(s) => format!("VARCHAR({s})"),
        None => "TEXT".into(),
    };
    InferredColumn { sql_type, not_null }
}

pub fn infer_column_types(rows: &Table, attributes: &[String]) -> IndexMap<String, InferredColumn> {
    let mut out = IndexMap::new();
    for a in attributes {
        out.insert(a.clone(), infer_column_sql_type(&rows.column(a)));
    }
    out
}
