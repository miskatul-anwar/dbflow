//! Identifier handling shared by the ERD, SQL and report emitters.

use std::collections::{HashMap, HashSet};
use unicode_normalization::UnicodeNormalization;

/// Keep word characters only; never empty, never starts with a digit.
pub fn sanitize_identifier(raw: &str) -> String {
    let decomposed: String = raw.nfkd().filter(|c| !('\u{0300}'..='\u{036f}').contains(c)).collect();
    let mut s = String::with_capacity(decomposed.len());
    let mut in_bad = false;
    for c in decomposed.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            s.push(c);
            in_bad = false;
        } else if !in_bad {
            s.push('_');
            in_bad = true;
        }
    }
    let s = s.trim_matches('_').to_string();
    let s = if s.is_empty() { "col".to_string() } else { s };
    if s.as_bytes()[0].is_ascii_digit() {
        format!("c_{s}")
    } else {
        s
    }
}

#[derive(Clone, Debug, Default)]
pub struct IdentifierRegistry {
    used: HashSet<String>,
    by_original: HashMap<String, String>,
}

impl IdentifierRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Unique sanitized name for `original`; the same original always maps to the same name.
    pub fn assign(&mut self, original: &str) -> String {
        if let Some(k) = self.by_original.get(original) {
            if !k.is_empty() {
                return k.clone();
            }
        }
        let base = sanitize_identifier(original);
        let mut candidate = base.clone();
        let mut n = 2;
        while self.used.contains(&candidate.to_lowercase()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        self.used.insert(candidate.to_lowercase());
        self.by_original.insert(original.to_string(), candidate.clone());
        candidate
    }

    /// A new unique name derived from `base`, even if `base` was handed out before.
    pub fn fresh(&mut self, base: &str) -> String {
        let clean = sanitize_identifier(base);
        let mut candidate = clean.clone();
        let mut n = 2;
        while self.used.contains(&candidate.to_lowercase()) {
            candidate = format!("{clean}_{n}");
            n += 1;
        }
        self.used.insert(candidate.to_lowercase());
        candidate
    }

    pub fn get(&self, original: &str) -> Option<String> {
        self.by_original.get(original).cloned()
    }
}

/// Escape text for a Markdown table cell or inline span.
pub fn md_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.push_str("\\\\"),
            '|' | '`' | '*' | '_' | '[' | ']' | '<' | '>' => {
                out.push('\\');
                out.push(c);
            }
            '\r' if chars.peek() == Some(&'\n') => {
                chars.next();
                out.push(' ');
            }
            '\n' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registry() {
        let mut r = IdentifierRegistry::new();
        assert_eq!(r.assign("Order Date"), "Order_Date");
        assert_eq!(r.assign("Order-Date"), "Order_Date_2");
        assert_eq!(r.assign("Order Date"), "Order_Date");
        assert_eq!(sanitize_identifier("1st"), "c_1st");
        assert_eq!(sanitize_identifier("Café"), "Cafe");
        assert_eq!(sanitize_identifier("***"), "col");
    }
}
