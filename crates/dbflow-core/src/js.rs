//! JavaScript-compatible primitives.
//!
//! The engine used to be TypeScript, and its outputs (reports, DDL, type
//! inference) depend on JavaScript semantics: `Number()` coercion,
//! `Number#toString`, `toFixed`, `toLocaleString`, `localeCompare` and the
//! permissive `Date.parse`. These helpers reproduce them so results stay the
//! same after the move to Rust.

use std::cmp::Ordering;

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }
}

/// Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Days since 1970-01-01 for a civil date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// `new Date(ms).toISOString()`.
pub fn iso_from_ms(ms: f64) -> String {
    let ms = ms.floor() as i64;
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let h = rem / 3_600_000;
    let mi = (rem / 60_000) % 60;
    let s = (rem / 1000) % 60;
    let milli = rem % 1000;
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{milli:03}Z")
}

pub fn iso_now() -> String {
    iso_from_ms(now_ms())
}

/// `new Date(ms).toLocaleTimeString('en-US')` for a time zone `offset_minutes` east of UTC.
pub fn locale_time(ms: f64, offset_minutes: f64) -> String {
    let local = (ms + offset_minutes * 60_000.0).floor() as i64;
    let rem = local.rem_euclid(86_400_000);
    let h = rem / 3_600_000;
    let mi = (rem / 60_000) % 60;
    let s = (rem / 1000) % 60;
    let (h12, ampm) = match h {
        0 => (12, "AM"),
        1..=11 => (h, "AM"),
        12 => (12, "PM"),
        _ => (h - 12, "PM"),
    };
    format!("{h12}:{mi:02}:{s:02} {ampm}")
}

/// JavaScript `String.prototype.trim` whitespace.
pub fn is_js_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{FEFF}'
}

pub fn trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// Length in UTF-16 code units (`String#length`).
pub fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// First `n` UTF-16 code units of `s` (`substring(0, n)`), never splitting a character.
pub fn substring16(s: &str, n: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = c.len_utf16();
        if used + w > n {
            break;
        }
        used += w;
        out.push(c);
    }
    out
}

fn decimal_literal(s: &str) -> bool {
    // [+-]?(digits[.digits?]|.digits)([eE][+-]?digits)?
    let b = s.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - fs;
    }
    if int_digits == 0 && frac_digits == 0 {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let es = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == es {
            return false;
        }
    }
    i == b.len()
}

/// `Number(string)`.
pub fn number(s: &str) -> f64 {
    let t = trim(s);
    if t.is_empty() {
        return 0.0;
    }
    match t {
        "Infinity" | "+Infinity" => return f64::INFINITY,
        "-Infinity" => return f64::NEG_INFINITY,
        _ => {}
    }
    let lower = t.to_ascii_lowercase();
    for (prefix, radix) in [("0x", 16), ("0o", 8), ("0b", 2)] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            if rest.is_empty() {
                return f64::NAN;
            }
            let mut v = 0f64;
            for c in rest.chars() {
                match c.to_digit(radix) {
                    Some(d) => v = v * radix as f64 + d as f64,
                    None => return f64::NAN,
                }
            }
            return v;
        }
    }
    if decimal_literal(t) {
        t.parse::<f64>().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// `parseFloat(string)`.
pub fn parse_float(s: &str) -> f64 {
    let t = s.trim_start_matches(is_js_space);
    let b = t.as_bytes();
    let mut i = 0;
    let mut sign = 1.0;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        if b[i] == b'-' {
            sign = -1.0;
        }
        i += 1;
    }
    if t[i..].starts_with("Infinity") {
        return sign * f64::INFINITY;
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut end = i;
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > i + 1 || i > start {
            end = j;
            i = j;
        }
    }
    if end == start || (end == start + 1 && &t[start..end] == ".") {
        return f64::NAN;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let es = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > es {
            end = j;
        }
    }
    t[start..end].parse::<f64>().map(|v| sign * v).unwrap_or(f64::NAN)
}

/// `isNaN(x)` for a string argument.
pub fn is_nan_str(s: &str) -> bool {
    number(s).is_nan()
}

/// `Number.prototype.toString()`.
pub fn num_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return "0".into();
    }
    let neg = x < 0.0;
    let e = format!("{:e}", x.abs());
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let rest = if k > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        let sign = if n > 0 { "+" } else { "-" };
        format!("{}{rest}e{sign}{}", &digits[..1], (n - 1).abs())
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

/// `Number.prototype.toFixed(digits)` (ties round away from zero, as in JavaScript).
pub fn to_fixed(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.abs() >= 1e21 || x.is_infinite() {
        return num_to_string(x);
    }
    let neg = x < 0.0;
    let exact = format!("{:.*}", digits + 30, x.abs());
    let (int_part, frac_part) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut all: Vec<u8> = int_part.bytes().chain(frac_part.bytes().take(digits)).collect();
    let next = frac_part.as_bytes().get(digits).copied().unwrap_or(b'0');
    if next >= b'5' {
        let mut i = all.len();
        loop {
            if i == 0 {
                all.insert(0, b'1');
                break;
            }
            i -= 1;
            if all[i] == b'9' {
                all[i] = b'0';
            } else {
                all[i] += 1;
                break;
            }
        }
    }
    let s = String::from_utf8(all).unwrap_or_default();
    let int_len = s.len() - digits;
    let body = if digits == 0 {
        s
    } else {
        format!("{}.{}", &s[..int_len], &s[int_len..])
    };
    let is_zero = body.bytes().all(|b| b == b'0' || b == b'.');
    if neg && !is_zero {
        format!("-{body}")
    } else {
        body
    }
}

/// `Number.prototype.toLocaleString('en-US')`.
pub fn locale_string(x: f64) -> String {
    if !x.is_finite() {
        return num_to_string(x);
    }
    let fixed = to_fixed(x, 3);
    let (int_part, frac) = fixed.split_once('.').unwrap_or((&fixed, ""));
    let (sign, digits) = int_part.strip_prefix('-').map(|d| ("-", d)).unwrap_or(("", int_part));
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{frac}")
    }
}

pub fn locale_usize(n: usize) -> String {
    locale_string(n as f64)
}

// ---------------------------------------------------------------------------
// localeCompare (approximation of the ICU root collation for typical names)
// ---------------------------------------------------------------------------

const PUNCT_ORDER: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";

fn base_char(c: char) -> char {
    use unicode_normalization::UnicodeNormalization;
    c.to_string().nfd().next().unwrap_or(c)
}

fn primary(c: char) -> (u32, u32) {
    if c.is_whitespace() {
        return (0, c as u32);
    }
    if let Some(p) = PUNCT_ORDER.find(c) {
        return (1, p as u32);
    }
    if c.is_ascii_digit() {
        return (3, c as u32);
    }
    let b = base_char(c);
    if b.is_alphabetic() {
        let l = b.to_lowercase().next().unwrap_or(b);
        return (4, l as u32);
    }
    if c.is_ascii() {
        return (2, c as u32);
    }
    (5, c as u32)
}

/// `a.localeCompare(b)`.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let pa: Vec<(u32, u32)> = a.chars().map(primary).collect();
    let pb: Vec<(u32, u32)> = b.chars().map(primary).collect();
    match pa.cmp(&pb) {
        Ordering::Equal => {}
        o => return o,
    }
    // Secondary: accents (unaccented first).
    let sa: Vec<bool> = a.chars().map(|c| base_char(c) != c).collect();
    let sb: Vec<bool> = b.chars().map(|c| base_char(c) != c).collect();
    match sa.cmp(&sb) {
        Ordering::Equal => {}
        o => return o,
    }
    // Tertiary: lowercase before uppercase.
    let ta: Vec<bool> = a.chars().map(|c| c.is_uppercase()).collect();
    let tb: Vec<bool> = b.chars().map(|c| c.is_uppercase()).collect();
    match ta.cmp(&tb) {
        Ordering::Equal => a.cmp(b),
        o => o,
    }
}

/// Plain UTF-16 code-unit comparison (`Array#sort` default / `<` on strings).
pub fn code_unit_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `[...strings].sort()` (default sort compares UTF-16 code units).
pub fn sort_strings(v: &mut [String]) {
    v.sort_by(|a, b| code_unit_cmp(a, b));
}

// ---------------------------------------------------------------------------
// Date.parse (V8 behaviour, validity only)
// ---------------------------------------------------------------------------

fn iso_es5(s: &str) -> Option<bool> {
    let re = crate::re!(
        r"^([+-][0-9]{6}|[0-9]{4})(?:-([0-9]{2})(?:-([0-9]{2}))?)?(?:T([0-9]{2}):([0-9]{2})(?::([0-9]{2})(?:\.[0-9]+)?)?(Z|[+-][0-9]{2}:[0-9]{2})?)?$"
    );
    let c = re.captures(s)?;
    let num = |i: usize| c.get(i).map(|m| m.as_str().parse::<u32>().unwrap_or(99));
    if let Some(m) = num(2) {
        if !(1..=12).contains(&m) {
            return Some(false);
        }
    }
    if let Some(d) = num(3) {
        if !(1..=31).contains(&d) {
            return Some(false);
        }
    }
    if let Some(h) = num(4) {
        let mi = num(5).unwrap_or(0);
        let sec = num(6).unwrap_or(0);
        if h > 24 || mi > 59 || sec > 59 || (h == 24 && (mi > 0 || sec > 0)) {
            return Some(false);
        }
    }
    Some(true)
}

enum Tok {
    Num(u64, usize),
    Word(String),
    Sym(char),
}

fn tokenize(s: &str) -> Vec<Tok> {
    let chars: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() {
            let st = i;
            let mut v: u64 = 0;
            while i < chars.len() && chars[i].is_ascii_digit() {
                v = v.saturating_mul(10).saturating_add(chars[i] as u64 - '0' as u64);
                i += 1;
            }
            out.push(Tok::Num(v, i - st));
        } else if c.is_alphabetic() {
            let st = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            out.push(Tok::Word(chars[st..i].iter().collect::<String>().to_lowercase()));
        } else if is_js_space(c) {
            i += 1;
        } else if c == '(' {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '(' {
                    depth += 1;
                } else if chars[i] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
        } else {
            out.push(Tok::Sym(c));
            i += 1;
        }
    }
    out
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const DAYS: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
const ZONES: [&str; 12] = [
    "ut", "utc", "gmt", "z", "est", "edt", "cst", "cdt", "mst", "mdt", "pst", "pdt",
];

fn legacy_date(s: &str) -> bool {
    let toks = tokenize(s);
    let mut day: Vec<(u64, usize)> = Vec::new();
    let mut month_named = false;
    let mut has_number = false;
    let mut has_time = false;
    let mut i = 0;
    while i < toks.len() {
        match &toks[i] {
            Tok::Num(n, len) => {
                has_number = true;
                if matches!(toks.get(i + 1), Some(Tok::Sym(':'))) && !has_time {
                    // h:m[:s[.ms]]
                    let h = *n;
                    let m = match toks.get(i + 2) {
                        Some(Tok::Num(m, _)) => *m,
                        _ => return false,
                    };
                    let mut j = i + 3;
                    let mut sec = 0;
                    if matches!(toks.get(j), Some(Tok::Sym(':'))) {
                        match toks.get(j + 1) {
                            Some(Tok::Num(s, _)) => {
                                sec = *s;
                                j += 2;
                            }
                            _ => return false,
                        }
                        if matches!(toks.get(j), Some(Tok::Sym('.'))) {
                            if let Some(Tok::Num(_, _)) = toks.get(j + 1) {
                                j += 2;
                            }
                        }
                    }
                    if h > 24 || m > 59 || sec > 59 {
                        return false;
                    }
                    has_time = true;
                    i = j;
                    continue;
                }
                if has_time && i > 0 && matches!(toks.get(i - 1), Some(Tok::Sym('+')) | Some(Tok::Sym('-'))) {
                    i += 1; // time-zone offset
                    continue;
                }
                if day.len() == 3 {
                    return false;
                }
                day.push((*n, *len));
            }
            Tok::Word(w) => {
                let w3: String = w.chars().take(3).collect();
                let known = w == "am"
                    || w == "pm"
                    || w == "t"
                    || (w.len() >= 3 && DAYS.contains(&w3.as_str()))
                    || ZONES.contains(&w.as_str());
                if w.len() >= 3 && MONTHS.contains(&w3.as_str()) {
                    month_named = true;
                } else if !known && has_number {
                    // An unrecognized word after a number makes the date invalid (V8).
                    return false;
                }
            }
            Tok::Sym(_) => {}
        }
        i += 1;
    }
    let ok_month = |m: u64| (1..=12).contains(&m);
    let ok_day = |d: u64| (1..=31).contains(&d);
    if month_named {
        return match day.as_slice() {
            [] => false,
            [(n, len)] => *len >= 3 || *n > 31 || ok_day(*n),
            [(a, la), (b, _)] => {
                if *la >= 3 || *a > 31 {
                    ok_day(*b)
                } else {
                    ok_day(*a)
                }
            }
            _ => false,
        };
    }
    match day.as_slice() {
        [] => false,
        [(n, len)] => *len >= 3 || *n > 31 || ok_month(*n),
        [(a, la), (b, _)] => {
            if *la >= 3 || *a > 31 {
                ok_month(*b)
            } else {
                ok_month(*a) && ok_day(*b)
            }
        }
        [(a, la), (b, _), (c, _)] => {
            if *la >= 3 || *a > 31 {
                ok_month(*b) && ok_day(*c)
            } else {
                ok_month(*a) && ok_day(*b)
            }
        }
        _ => false,
    }
}

/// `!isNaN(Date.parse(s))`.
pub fn date_parse_valid(s: &str) -> bool {
    if let Some(v) = iso_es5(s) {
        return v;
    }
    legacy_date(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(num_to_string(1.0), "1");
        assert_eq!(num_to_string(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(num_to_string(1e21), "1e+21");
        assert_eq!(num_to_string(1e-7), "1e-7");
        assert_eq!(num_to_string(123.5), "123.5");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(-0.0001, 2), "0.00");
        assert_eq!(to_fixed(99.95, 1), "100.0");
        assert_eq!(locale_string(1234567.0), "1,234,567");
        assert!(number("0x1A") == 26.0);
        assert!(number(" 12 ") == 12.0);
        assert!(number("12abc").is_nan());
        assert!(number("") == 0.0);
        assert_eq!(parse_float("12abc"), 12.0);
        assert!(parse_float("abc").is_nan());
        assert_eq!(parse_float(".5x"), 0.5);
    }

    #[test]
    fn dates() {
        assert!(date_parse_valid("2024-01-05"));
        assert!(date_parse_valid("Jan 1, 2023"));
        assert!(date_parse_valid("1/2/2020"));
        assert!(date_parse_valid("Name 1"));
        assert!(!date_parse_valid("hello"));
        assert!(!date_parse_valid("12 hello"));
        assert!(!date_parse_valid("2024-13-01"));
        assert!(date_parse_valid("2024-01-01 10:00"));
    }

    #[test]
    fn collation() {
        assert_eq!(locale_compare("a_b", "ab"), Ordering::Less);
        assert_eq!(locale_compare("a", "B"), Ordering::Less);
        assert_eq!(locale_compare("a", "A"), Ordering::Less);
        assert_eq!(locale_compare("a,b", "a,c"), Ordering::Less);
        assert_eq!(locale_compare("item2", "item10"), Ordering::Greater);
    }
}
