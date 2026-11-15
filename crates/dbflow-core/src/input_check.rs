//! Checks on the uploaded files, run before any analysis:
//!
//! * what a file really contains, whatever its extension (a PDF, an image, a program, JSON or
//!   source code is rejected instead of being read as a one-column table);
//! * how far a workbook expands when it is unzipped (a small "compression bomb" could otherwise
//!   need gigabytes of memory);
//! * which files repeat another file exactly, and which files overlap another file (same columns,
//!   mostly the same rows), with an optional merge of overlapping files.

use crate::csv;
use crate::encoding::decode_bytes;
use crate::pipeline::InputFile;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::Read;

// ----- what a file contains ----------------------------------------------------------------------

/// Signatures of common files that are never tables.
fn signature(b: &[u8]) -> Option<&'static str> {
    let starts = |p: &[u8]| b.starts_with(p);
    if starts(b"%PDF-") {
        return Some("a PDF document");
    }
    if starts(b"\x89PNG\r\n\x1a\n") {
        return Some("a PNG image");
    }
    if starts(b"\xff\xd8\xff") {
        return Some("a JPEG image");
    }
    if starts(b"GIF87a") || starts(b"GIF89a") {
        return Some("a GIF image");
    }
    if b.len() >= 12 && starts(b"RIFF") && matches!(&b[8..12], b"WEBP" | b"WAVE" | b"AVI ") {
        return Some("an image, audio or video file");
    }
    if b.len() >= 12 && &b[4..8] == b"ftyp" {
        return Some("a video or image file (MP4, MOV or HEIC)");
    }
    if starts(b"OggS") || starts(b"fLaC") || starts(b"\x1aE\xdf\xa3") {
        return Some("an audio or video file");
    }
    if b.len() >= 4 && starts(b"ID3") && b[3] <= 4 {
        return Some("an MP3 audio file");
    }
    if starts(b"\x1f\x8b") {
        return Some("a gzip archive");
    }
    if starts(b"7z\xbc\xaf\x27\x1c") {
        return Some("a 7-Zip archive");
    }
    if starts(b"Rar!\x1a\x07") {
        return Some("a RAR archive");
    }
    if starts(b"\xfd7zXZ\x00") {
        return Some("an xz archive");
    }
    if b.len() >= 4 && starts(b"BZh") && b[3].is_ascii_digit() && b[3] != b'0' {
        return Some("a bzip2 archive");
    }
    if starts(b"\x7fELF") {
        return Some("a Linux program");
    }
    if starts(b"MZ") && b.len() >= 64 && b[..64].contains(&0) {
        return Some("a Windows program");
    }
    if starts(b"\xcf\xfa\xed\xfe") || starts(b"\xce\xfa\xed\xfe") || starts(b"\xca\xfe\xba\xbe") {
        return Some("a compiled program");
    }
    if starts(b"SQLite format 3\x00") {
        return Some("an SQLite database");
    }
    if starts(b"\x00asm") {
        return Some("a WebAssembly module");
    }
    if starts(b"{\\rtf") {
        return Some("an RTF document");
    }
    None
}

/// A ZIP container (xlsx, ods and other Office files) or an OLE2 container (xls).
pub fn is_container(b: &[u8]) -> bool {
    (b.len() >= 4 && b[0] == 0x50 && b[1] == 0x4b && matches!(b[2], 0x03 | 0x05 | 0x07))
        || (b.len() >= 8 && b[..4] == [0xd0, 0xcf, 0x11, 0xe0])
}

/// `Err` when the bytes are a known kind of file that is not a table.
pub fn reject_known_non_table(name: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Err(format!("{name} is empty."));
    }
    match signature(bytes) {
        Some(kind) => Err(format!("{name} is {kind}, not a table.")),
        None => Ok(()),
    }
}

const SAMPLE_BYTES: usize = 64 * 1024;

/// Line starts that mark program source code.
const CODE_STARTS: [&str; 24] = [
    "#include",
    "#define",
    "#!",
    "#pragma",
    "import ",
    "from ",
    "package ",
    "using ",
    "def ",
    "fn ",
    "func ",
    "function ",
    "class ",
    "public ",
    "private ",
    "protected ",
    "const ",
    "let ",
    "var ",
    "return ",
    "//",
    "/*",
    "*/",
    "<?php",
];

fn is_code_line(line: &str) -> bool {
    let t = line.trim();
    if CODE_STARTS.iter().any(|s| t.starts_with(s)) {
        return true;
    }
    if t.ends_with('{') || t == "}" || t == "};" || t == "});" {
        return true;
    }
    // A statement: ends with a semicolon and has code punctuation. Semicolon-separated data
    // ("1;Ann;Lisbon;") has none of it.
    t.ends_with(';') && ["(", " = ", "<<", "->", "::"].iter().any(|p| t.contains(p))
}

/// `Err` when delimited-text input is really binary data, markup, JSON or program source code.
/// Only the first 64 KB are examined.
pub fn check_text_table(name: &str, bytes: &[u8]) -> Result<(), String> {
    let sample = &bytes[..bytes.len().min(SAMPLE_BYTES)];
    let (text, _) = decode_bytes(sample);
    let total = text.chars().count();
    let odd = text
        .chars()
        .filter(|&c| (c.is_control() && !matches!(c, '\t' | '\n' | '\r')) || c == '\u{fffd}')
        .count();
    if total > 0 && odd > 8 && odd * 100 > total {
        return Err(format!("{name} contains binary data, not text. It is not a table."));
    }
    let body = text.trim_start_matches('\u{feff}').trim_start();
    if body.starts_with('<') {
        return Err(format!(
            "{name} contains XML or HTML markup, not a delimited table. Save it as CSV or as a spreadsheet first."
        ));
    }
    if body.starts_with('{') || body.starts_with('[') {
        let mut values = serde_json::Deserializer::from_str(body).into_iter::<serde_json::Value>();
        let is_json = match values.next() {
            Some(Ok(v)) => v.is_object() || v.is_array(),
            // The sample ends inside a long JSON document.
            Some(Err(e)) => e.is_eof(),
            None => false,
        };
        if is_json {
            return Err(format!(
                "{name} contains JSON, not a delimited table. Export it as CSV first."
            ));
        }
    }
    let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).take(60).collect();
    let code = lines.iter().filter(|l| is_code_line(l)).count();
    if code >= 3 && code * 5 >= lines.len() * 2 {
        return Err(format!("{name} looks like program source code, not a table."));
    }
    Ok(())
}

// ----- how far a workbook expands ----------------------------------------------------------------

/// Expanded size above which a workbook entry must also have a plausible compression ratio.
const RATIO_CHECK_BYTES: u64 = 16 << 20;
/// Real workbooks compress about 5–50 times; compression bombs compress hundreds of times or more.
const MAX_RATIO: u64 = 200;
/// Memory needed to read a workbook, per byte of expanded XML (measured: about 7.6).
const READ_BYTES_PER_EXPANDED_BYTE: u64 = 8;

#[cfg(target_arch = "wasm32")]
fn expansion_limit() -> u64 {
    // WebAssembly memory is at most 4 GB.
    512 << 20
}

#[cfg(not(target_arch = "wasm32"))]
fn expansion_limit() -> u64 {
    let hard = 2u64 << 30;
    match crate::available_memory_bytes() {
        Some(free) => hard.min(free / READ_BYTES_PER_EXPANDED_BYTE),
        None => hard,
    }
}

fn mb(n: u64) -> String {
    format!("{} MB", n.div_ceil(1 << 20))
}

/// What a ZIP container holds, measured by actually decompressing it (declared sizes can lie).
pub struct ZipSummary {
    pub entries: Vec<String>,
}

/// Decompress every entry of a ZIP container while counting the bytes, and stop as soon as the
/// total passes the limit or an entry expands implausibly far.
pub fn check_zip_expansion(name: &str, bytes: &[u8]) -> Result<ZipSummary, String> {
    let limit = expansion_limit();
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        // Not a readable ZIP: the workbook reader reports the problem.
        return Ok(ZipSummary { entries: vec![] });
    };
    let mut total = 0u64;
    let mut entries = Vec::with_capacity(zip.len());
    let mut buf = vec![0u8; 64 * 1024];
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        let entry_name = entry.name().map(|n| n.to_string()).unwrap_or_default();
        entries.push(entry_name.clone());
        let packed = entry.compressed_size().max(1);
        let mut expanded = 0u64;
        loop {
            let n = match entry.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n as u64,
                // Damaged or encrypted entry: the workbook reader reports it.
                Err(_) => break,
            };
            expanded += n;
            total += n;
            if expanded > RATIO_CHECK_BYTES && expanded / packed > MAX_RATIO {
                return Err(format!(
                    "{name} looks like a compression bomb: {} inside it expands from {} to more than {}. It was not read.",
                    entry_name,
                    mb(packed),
                    mb(expanded)
                ));
            }
            if total > limit {
                return Err(format!(
                    "{name} expands to more than {} when unzipped, which needs more memory than this computer can spare ({} for reading it). Split the workbook into smaller files.",
                    mb(limit),
                    mb(limit.saturating_mul(READ_BYTES_PER_EXPANDED_BYTE))
                ));
            }
        }
    }
    Ok(ZipSummary { entries })
}

/// A plain-language description of a ZIP container that is not a spreadsheet.
pub fn describe_non_spreadsheet_zip(entries: &[String], bytes: &[u8]) -> Option<&'static str> {
    let has = |p: &str| entries.iter().any(|e| e.starts_with(p));
    if has("xl/") || has("content.xml") && !is_opendocument_text(bytes) {
        return None;
    }
    if has("word/") {
        return Some("a Word document");
    }
    if has("ppt/") {
        return Some("a PowerPoint presentation");
    }
    if is_opendocument_text(bytes) {
        return Some("an OpenDocument text or presentation file");
    }
    Some("a ZIP archive")
}

fn is_opendocument_text(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(200)];
    let s = String::from_utf8_lossy(head);
    s.contains("application/vnd.oasis.opendocument.text")
        || s.contains("application/vnd.oasis.opendocument.presentation")
}

// ----- repeated and overlapping files ------------------------------------------------------------

/// A file that was not analysed, and why.
#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub struct Skipped {
    pub file: String,
    pub reason: String,
}

/// A file with exactly the same contents as an earlier file.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Duplicate {
    pub file: String,
    pub same_as: String,
}

/// Two files with the same columns and mostly the same rows.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overlap {
    pub files: [String; 2],
    pub rows: [usize; 2],
    pub shared_rows: usize,
    pub only_in_first: usize,
    pub only_in_second: usize,
    /// The identifier column used to match records, when there is one.
    pub key: Option<String>,
    pub shared_keys: usize,
    /// Keys present in both files with different values in the other columns.
    pub conflicts: usize,
    pub conflict_keys: Vec<String>,
    #[serde(skip)]
    pub index: [usize; 2],
}

/// Files merged into one table at the user's request.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Merge {
    pub into: String,
    pub from: Vec<String>,
    pub rows: usize,
    /// Conflicting records for which the first file's version was kept.
    pub conflicts_resolved: usize,
}

/// Everything the input checks found, reported with the result.
#[derive(Clone, Debug, Default, Serialize)]
pub struct InputChecks {
    pub skipped: Vec<Skipped>,
    pub duplicates: Vec<Duplicate>,
    pub overlaps: Vec<Overlap>,
    pub merged: Vec<Merge>,
}

impl InputChecks {
    pub fn is_empty(&self) -> bool {
        self.skipped.is_empty() && self.duplicates.is_empty() && self.overlaps.is_empty() && self.merged.is_empty()
    }
}

/// How each file is named in messages: its name, plus "(file k)" when several files share it.
pub fn labels(names: &[String]) -> Vec<String> {
    let mut count: HashMap<String, usize> = HashMap::new();
    for n in names {
        *count.entry(n.to_lowercase()).or_default() += 1;
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    names
        .iter()
        .map(|n| {
            let k = n.to_lowercase();
            if count[&k] < 2 {
                return n.clone();
            }
            let i = seen.entry(k).or_default();
            *i += 1;
            format!("{n} (file {i})")
        })
        .collect()
}

/// A hash of the contents that ignores a byte-order mark, line-ending style and trailing blank space.
pub fn fingerprint(bytes: &[u8]) -> (usize, u64) {
    let b = bytes.strip_prefix(&[0xef, 0xbb, 0xbf][..]).unwrap_or(bytes);
    let end = b.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(0, |i| i + 1);
    let b = &b[..end];
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let mut len = 0;
    let mut start = 0;
    for (i, &c) in b.iter().enumerate() {
        if c == b'\r' && b.get(i + 1) == Some(&b'\n') {
            h.write(&b[start..i]);
            len += i - start;
            start = i + 1;
        }
    }
    h.write(&b[start..]);
    len += b.len() - start;
    (len, h.finish())
}

struct Parsed {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn parse_all(file: &InputFile) -> Option<Parsed> {
    let (text, _) = decode_bytes(&file.bytes);
    let r = csv::parse(&text, &Default::default()).ok()?;
    let rows = (0..r.data.len())
        .map(|i| r.headers.iter().map(|h| r.data.get(i, h).js_string()).collect())
        .collect();
    Some(Parsed {
        headers: r.headers,
        rows,
    })
}

/// The header of a delimited file, normalised for comparison (lower case, sorted).
fn header_set(file: &InputFile) -> Option<Vec<String>> {
    let sample = &file.bytes[..file.bytes.len().min(SAMPLE_BYTES)];
    let (text, _) = decode_bytes(sample);
    let first = text.trim_start_matches('\u{feff}').lines().next()?;
    let delimiter = csv::detect_delimiter(&text);
    let (cells, _) = csv::parse_cells(first, delimiter);
    let mut h: Vec<String> = cells
        .first()?
        .iter()
        .map(|c| c.value.trim().to_lowercase())
        .filter(|c| !c.is_empty())
        .collect();
    if h.len() < 2 {
        return None;
    }
    h.sort();
    Some(h)
}

fn row_hash(row: &[String], order: &[usize]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for &i in order {
        row[i].trim().hash(&mut h);
    }
    h.finish()
}

/// Column positions of `p` in the order of the sorted lower-case header.
fn sorted_order(p: &Parsed) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..p.headers.len()).collect();
    idx.sort_by_key(|&i| p.headers[i].trim().to_lowercase());
    idx
}

fn unique_non_empty(p: &Parsed, col: usize) -> bool {
    let mut seen = HashSet::with_capacity(p.rows.len());
    p.rows
        .iter()
        .all(|r| !r[col].trim().is_empty() && seen.insert(r[col].trim()))
}

/// The column that identifies records in both files: unique and never empty in each, preferring
/// identifier-like names ("id", "customer_id", ...).
fn shared_key(a: &Parsed, b: &Parsed) -> Option<(usize, usize)> {
    let mut best: Option<(u8, usize, usize)> = None;
    for (ia, h) in a.headers.iter().enumerate() {
        let Some(ib) = b.headers.iter().position(|x| x.trim().eq_ignore_ascii_case(h.trim())) else {
            continue;
        };
        if !unique_non_empty(a, ia) || !unique_non_empty(b, ib) {
            continue;
        }
        let score = crate::keys::looks_like_identifier(h.trim());
        if best.map_or(true, |(s, _, _)| score > s) {
            best = Some((score, ia, ib));
        }
    }
    best.map(|(_, ia, ib)| (ia, ib))
}

fn compare(a: &Parsed, b: &Parsed) -> (usize, Option<String>, usize, Vec<String>, usize) {
    let (oa, ob) = (sorted_order(a), sorted_order(b));
    let ha: HashSet<u64> = a.rows.iter().map(|r| row_hash(r, &oa)).collect();
    let hb: HashSet<u64> = b.rows.iter().map(|r| row_hash(r, &ob)).collect();
    let shared_rows = ha.intersection(&hb).count();
    let Some((ka, kb)) = shared_key(a, b) else {
        return (shared_rows, None, 0, vec![], 0);
    };
    let by_key: HashMap<&str, u64> = a.rows.iter().map(|r| (r[ka].trim(), row_hash(r, &oa))).collect();
    let (mut shared_keys, mut conflicts, mut samples) = (0, 0, vec![]);
    for r in &b.rows {
        if let Some(&h) = by_key.get(r[kb].trim()) {
            shared_keys += 1;
            if h != row_hash(r, &ob) {
                conflicts += 1;
                if samples.len() < 5 {
                    samples.push(r[kb].trim().to_string());
                }
            }
        }
    }
    (
        shared_rows,
        Some(a.headers[ka].trim().to_string()),
        shared_keys,
        samples,
        conflicts,
    )
}

/// Pairs of files with the same columns whose rows (or record keys) mostly match.
pub fn find_overlaps(files: &[InputFile], labels: &[String]) -> Vec<Overlap> {
    let headers: Vec<Option<Vec<String>>> = files.iter().map(header_set).collect();
    let mut parsed: HashMap<usize, Option<Parsed>> = HashMap::new();
    let mut out = Vec::new();
    for i in 0..files.len() {
        for j in i + 1..files.len() {
            if headers[i].is_none() || headers[i] != headers[j] {
                continue;
            }
            for k in [i, j] {
                parsed.entry(k).or_insert_with(|| parse_all(&files[k]));
            }
            let (Some(Some(a)), Some(Some(b))) = (parsed.get(&i), parsed.get(&j)) else {
                continue;
            };
            let smaller = a.rows.len().min(b.rows.len());
            if smaller == 0 {
                continue;
            }
            let (shared_rows, key, shared_keys, conflict_keys, conflicts) = compare(a, b);
            if shared_rows.max(shared_keys) * 2 < smaller {
                continue;
            }
            out.push(Overlap {
                files: [labels[i].clone(), labels[j].clone()],
                rows: [a.rows.len(), b.rows.len()],
                shared_rows,
                only_in_first: a.rows.len() - shared_rows,
                only_in_second: b.rows.len() - shared_rows,
                key,
                shared_keys,
                conflicts,
                conflict_keys,
                index: [i, j],
            });
        }
    }
    out
}

fn csv_line(cells: impl Iterator<Item = String>) -> String {
    cells
        .map(|c| crate::spreadsheet::csv_field(&c))
        .collect::<Vec<_>>()
        .join(",")
}

/// Merge each group of overlapping files into the group's first file. Rows already present are not
/// repeated; for a record key present in several files with different values, the first file's
/// version is kept. Returns the files that remain and what was merged.
pub fn merge_overlapping(
    files: Vec<InputFile>,
    labels: Vec<String>,
    overlaps: &[Overlap],
) -> (Vec<InputFile>, Vec<String>, Vec<Merge>) {
    // Union-find over overlapping pairs; every group is merged into its lowest index.
    let mut parent: Vec<usize> = (0..files.len()).collect();
    fn root(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for o in overlaps {
        let (a, b) = (root(&mut parent, o.index[0]), root(&mut parent, o.index[1]));
        if a != b {
            parent[a.max(b)] = a.min(b);
        }
    }
    let mut groups: Vec<Vec<usize>> = vec![vec![]; files.len()];
    for i in 0..files.len() {
        let r = root(&mut parent, i);
        groups[r].push(i);
    }
    let mut merged_into: HashMap<usize, (Vec<u8>, Merge)> = HashMap::new();
    for g in groups.iter().filter(|g| g.len() > 1) {
        let parts: Vec<Parsed> = g.iter().filter_map(|&k| parse_all(&files[k])).collect();
        if parts.len() != g.len() {
            continue;
        }
        let base = &parts[0];
        // Every file in the group has the same columns; write them in the first file's order.
        let col_of = |p: &Parsed| -> Vec<usize> {
            base.headers
                .iter()
                .map(|h| {
                    p.headers
                        .iter()
                        .position(|x| x.trim().eq_ignore_ascii_case(h.trim()))
                        .unwrap_or(0)
                })
                .collect()
        };
        let key = parts[1..]
            .iter()
            .map(|p| shared_key(base, p).map(|(ka, _)| ka))
            .collect::<Option<Vec<_>>>()
            .and_then(|ks| ks.first().copied().filter(|k| ks.iter().all(|x| x == k)));
        let order = sorted_order(base);
        let mut seen_rows: HashSet<u64> = HashSet::new();
        let mut seen_keys: HashSet<String> = HashSet::new();
        let mut text = csv_line(base.headers.iter().cloned());
        let (mut rows, mut conflicts) = (0usize, 0usize);
        for p in &parts {
            let cols = col_of(p);
            for r in &p.rows {
                let aligned: Vec<String> = cols.iter().map(|&c| r[c].clone()).collect();
                if !seen_rows.insert(row_hash(&aligned, &order)) {
                    continue;
                }
                if let Some(k) = key {
                    if !seen_keys.insert(aligned[k].trim().to_string()) {
                        conflicts += 1;
                        continue;
                    }
                }
                text.push('\n');
                text.push_str(&csv_line(aligned.into_iter()));
                rows += 1;
            }
        }
        merged_into.insert(
            g[0],
            (
                text.into_bytes(),
                Merge {
                    into: labels[g[0]].clone(),
                    from: g[1..].iter().map(|&k| labels[k].clone()).collect(),
                    rows,
                    conflicts_resolved: conflicts,
                },
            ),
        );
    }
    let absorbed: HashSet<usize> = groups
        .iter()
        .filter(|g| g.len() > 1 && merged_into.contains_key(&g[0]))
        .flat_map(|g| g[1..].iter().copied())
        .collect();
    let mut out_files = Vec::new();
    let mut out_labels = Vec::new();
    let mut merges = Vec::new();
    for (i, (mut f, l)) in files.into_iter().zip(labels).enumerate() {
        if absorbed.contains(&i) {
            continue;
        }
        if let Some((bytes, m)) = merged_into.remove(&i) {
            f.bytes = bytes;
            merges.push(m);
        }
        out_files.push(f);
        out_labels.push(l);
    }
    (out_files, out_labels, merges)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_are_recognised_and_text_is_not() {
        assert!(reject_known_non_table("a.csv", b"%PDF-1.4 ...").is_err());
        assert!(reject_known_non_table("a.csv", b"\x89PNG\r\n\x1a\n....").is_err());
        assert!(reject_known_non_table("a.csv", b"   \n").is_err());
        assert!(reject_known_non_table("a.csv", b"MZ_code,name\n1,a\n").is_ok());
        assert!(reject_known_non_table("a.csv", b"BZh,x\n1,2\n").is_ok());
        assert!(reject_known_non_table("a.csv", b"id,name\n1,a\n").is_ok());
    }

    #[test]
    fn text_checks_reject_json_markup_code_and_binary_only() {
        assert!(check_text_table("a.csv", br#"[{"id":1},{"id":2}]"#).is_err());
        assert!(check_text_table("a.csv", b"{\"id\":1}\n{\"id\":2}\n").is_err());
        assert!(check_text_table("a.csv", b"<html><table></table></html>").is_err());
        let cpp = b"#include <iostream>\nint main(){\n  std::cout << \"hi\" << std::endl;\n  return 0;\n}\n";
        assert!(check_text_table("a.csv", cpp).is_err());
        let bin: Vec<u8> = (0u8..=255).cycle().take(2000).collect();
        assert!(check_text_table("a.csv", &bin).is_err());
        // Real tables pass, including semicolon-separated rows and bracketed headers.
        assert!(check_text_table("a.csv", b"id;name;city;\n1;Ann;Lisbon;\n2;Bob;Paris;\n").is_ok());
        assert!(check_text_table("a.csv", b"[id],name\n1,a\n").is_ok());
        assert!(check_text_table("a.csv", b"note\n\"{a} b\"\nplain\n").is_ok());
        let utf16: Vec<u8> = "id,name\n1,Ann\n"
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let mut with_bom = vec![0xff, 0xfe];
        with_bom.extend(utf16);
        assert!(check_text_table("a.csv", &with_bom).is_ok());
    }

    #[test]
    fn labels_number_only_repeated_names() {
        let n = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            labels(&n(&["data.csv", "orders.csv", "Data.csv"])),
            n(&["data.csv (file 1)", "orders.csv", "Data.csv (file 2)"])
        );
    }

    #[test]
    fn fingerprint_ignores_line_endings_and_bom() {
        assert_eq!(fingerprint(b"a,b\r\n1,2\r\n"), fingerprint(b"\xef\xbb\xbfa,b\n1,2"));
        assert_ne!(fingerprint(b"a,b\n1,2"), fingerprint(b"a,b\n1,3"));
    }
}
