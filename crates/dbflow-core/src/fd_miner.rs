//! Functional-dependency mining by partition refinement (TANE-style lattice search),
//! plus sampled mining with full-data validation (HyFD-style).

use crate::js;
use crate::table::{Cell, Table};
use crate::types::Fd;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NullSemantics {
    #[default]
    Equal,
    Distinct,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MinedFd {
    pub lhs: Vec<String>,
    pub rhs: String,
    pub confidence: f64,
    pub coverage: f64,
    pub violations: f64,
    pub unique_groups: f64,
}

impl MinedFd {
    pub fn to_fd(&self) -> Fd {
        Fd::new(
            self.lhs.clone(),
            self.rhs.clone(),
            self.confidence,
            self.coverage,
            self.violations,
            self.unique_groups,
        )
    }
}

#[derive(Clone, Default)]
pub struct MineOptions<'a> {
    pub max_lhs_size: Option<usize>,
    pub min_confidence: Option<f64>,
    pub null_semantics: NullSemantics,
    pub near_key_ratio: Option<f64>,
    pub complete_determinants_only: bool,
    pub near_key_min_rows: Option<usize>,
    pub min_rows_for_non_key_determinants: Option<usize>,
    pub aborted: Option<&'a dyn Fn() -> bool>,
    pub time_budget_ms: Option<f64>,
    pub sample_size: Option<usize>,
    pub max_rounds: Option<usize>,
    pub seed: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingInfo {
    pub sampled: bool,
    pub total_rows: usize,
    pub sample_size: usize,
    pub rounds: usize,
    pub rows_added: usize,
    pub refuted: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MineResult {
    pub fds: Vec<MinedFd>,
    pub duration_ms: f64,
    pub combinations_evaluated: u64,
    pub exact_count: usize,
    pub truncated: bool,
    pub level_reached: usize,
    pub partition_cache: Option<CacheStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingInfo>,
}

fn cell_key(c: &Cell) -> std::borrow::Cow<'_, str> {
    match c {
        Cell::Num(n) => std::borrow::Cow::Owned(js::num_to_string(*n)),
        Cell::Str(s) => std::borrow::Cow::Borrowed(s.as_str()),
        other => std::borrow::Cow::Owned(other.json_key()),
    }
}

/// Integer-encode columns. Returns codes per column and the dictionary size per column.
fn encode_columns(data: &Table, attributes: &[String], ns: NullSemantics) -> Vec<(Vec<u32>, usize)> {
    crate::par_map(attributes, |attr| {
        let col = data.col(attr);
        let mut codes = Vec::with_capacity(data.len());
        let mut dict: HashMap<std::borrow::Cow<'_, str>, u32> = HashMap::new();
        let mut next: u32 = 0;
        let mut null_code: Option<u32> = None;
        for r in 0..data.len() {
            let raw = data.get_idx(r, col);
            if raw.is_missing() {
                let c = match ns {
                    NullSemantics::Distinct => {
                        next += 1;
                        next - 1
                    }
                    NullSemantics::Equal => *null_code.get_or_insert_with(|| {
                        next += 1;
                        next - 1
                    }),
                };
                codes.push(c);
                continue;
            }
            let code = *dict.entry(cell_key(raw)).or_insert_with(|| {
                next += 1;
                next - 1
            });
            codes.push(code);
        }
        (codes, next as usize)
    })
}

#[derive(Clone, Default)]
struct Partition {
    groups: Vec<Vec<u32>>,
    class_count: usize,
}

fn partition_of_column(codes: &[u32], dict: usize, rows: usize) -> Partition {
    let mut by_code: Vec<Vec<u32>> = vec![Vec::new(); dict];
    for (r, &c) in codes.iter().enumerate() {
        by_code[c as usize].push(r as u32);
    }
    let mut covered = 0;
    let groups: Vec<Vec<u32>> = by_code
        .into_iter()
        .filter(|g| g.len() >= 2)
        .inspect(|g| covered += g.len())
        .collect();
    Partition {
        class_count: groups.len() + rows - covered,
        groups,
    }
}

struct Refiner {
    slot: Vec<u32>,
    buckets: Vec<Vec<u32>>,
}

impl Refiner {
    fn new(max_dict: usize) -> Refiner {
        Refiner {
            slot: vec![u32::MAX; max_dict.max(1)],
            buckets: Vec::new(),
        }
    }

    /// Refine `p` by `codes`; returns the refined partition (when wanted) and the split rows.
    fn refine(&mut self, p: &Partition, codes: &[u32], rows: usize, keep: bool) -> (Option<Partition>, usize, usize) {
        let mut new_groups = Vec::new();
        let mut covered = 0usize;
        let mut groups_after = 0usize;
        let mut split_rows = 0usize;
        for group in &p.groups {
            let mut used = 0usize;
            for &r in group {
                let c = codes[r as usize] as usize;
                let s = self.slot[c];
                let b = if s == u32::MAX {
                    self.slot[c] = used as u32;
                    if self.buckets.len() <= used {
                        self.buckets.push(Vec::new());
                    }
                    self.buckets[used].clear();
                    used += 1;
                    used - 1
                } else {
                    s as usize
                };
                self.buckets[b].push(r);
            }
            for &r in group {
                self.slot[codes[r as usize] as usize] = u32::MAX;
            }
            if used <= 1 {
                if keep {
                    new_groups.push(group.clone());
                }
                covered += group.len();
                groups_after += 1;
                continue;
            }
            let mut largest = 0;
            for b in &self.buckets[..used] {
                largest = largest.max(b.len());
                if b.len() >= 2 {
                    if keep {
                        new_groups.push(b.clone());
                    }
                    covered += b.len();
                    groups_after += 1;
                }
            }
            split_rows += group.len() - largest;
        }
        let class_count = groups_after + rows - covered;
        let refined = if keep {
            Some(Partition {
                groups: new_groups,
                class_count,
            })
        } else {
            None
        };
        (refined, split_rows, class_count)
    }
}

/// Ascending index combinations of size `size` out of `n`.
fn combinations(n: usize, size: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    if size == 0 || size > n {
        return out;
    }
    let mut idx: Vec<usize> = (0..size).collect();
    loop {
        out.push(idx.clone());
        let mut i = size as isize - 1;
        while i >= 0 && idx[i as usize] == n - size + i as usize {
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

fn sort_fds(fds: &mut [MinedFd]) {
    fds.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.lhs.len().cmp(&b.lhs.len()))
            .then(b.coverage.partial_cmp(&a.coverage).unwrap_or(std::cmp::Ordering::Equal))
    });
}

/// Mine functional dependencies from rows (uniform keys expected; attributes from the first row).
pub fn mine(data: &Table, options: &MineOptions) -> MineResult {
    let started = js::now_ms();
    let empty = MineResult::default();
    if data.is_empty() {
        return empty;
    }
    let attributes = data.keys();
    if attributes.len() < 2 {
        return empty;
    }
    let max_lhs = options.max_lhs_size.unwrap_or(4).min(attributes.len() - 1).max(1);
    let min_conf = options.min_confidence.unwrap_or(0.85);
    let encoded = encode_columns(data, &attributes, options.null_semantics);
    let rows = data.len();
    let near_key_ratio = options.near_key_ratio.unwrap_or(1.0);
    let mut nullable = HashSet::new();
    if options.complete_determinants_only {
        for (i, a) in attributes.iter().enumerate() {
            let col = data.col(a);
            if (0..rows).any(|r| data.get_idx(r, col).is_missing()) {
                nullable.insert(i);
            }
        }
    }
    let prune_near = near_key_ratio < 1.0 && rows >= options.near_key_min_rows.unwrap_or(200);
    let min_rows_nonkey = options.min_rows_for_non_key_determinants.unwrap_or(0);
    let out_of_budget = || {
        options.aborted.map(|f| f()).unwrap_or(false)
            || options
                .time_budget_ms
                .map(|b| js::now_ms() - started > b)
                .unwrap_or(false)
    };

    let max_dict = encoded.iter().map(|(_, d)| *d).max().unwrap_or(1);
    let mut refiner = Refiner::new(max_dict);
    let base: Vec<Partition> = encoded.iter().map(|(c, d)| partition_of_column(c, *d, rows)).collect();
    let constant: HashSet<usize> = (0..base.len()).filter(|&i| base[i].class_count == 1).collect();

    let mut results: Vec<MinedFd> = Vec::new();
    let mut evaluated = 0u64;
    let mut exact = 0usize;
    let mut cache: HashMap<Vec<usize>, Partition> = HashMap::new();
    let (mut hits, mut misses) = (0u64, 0u64);
    for (a, p) in base.iter().enumerate() {
        cache.insert(vec![a], p.clone());
    }
    let mut superkeys: Vec<Vec<usize>> = Vec::new();
    let mut mined_by_rhs: HashMap<usize, Vec<(Vec<usize>, f64)>> = HashMap::new();
    let mut truncated = false;
    let mut level_reached = 0;

    'search: for level in 1..=max_lhs {
        if level >= 3 {
            cache.retain(|k, _| k.len() >= level - 1);
        }
        for combo in combinations(attributes.len(), level) {
            if out_of_budget() {
                truncated = true;
                break 'search;
            }
            if !nullable.is_empty() && combo.iter().any(|i| nullable.contains(i)) {
                continue;
            }
            if level > 1 && combo.iter().any(|i| constant.contains(i)) {
                continue;
            }
            if level > 1 && superkeys.iter().any(|k| k.iter().all(|a| combo.contains(a))) {
                continue;
            }
            if cache.contains_key(&combo) {
                hits += 1;
            } else {
                misses += 1;
                let parent_key = &combo[..combo.len() - 1];
                let last = combo[combo.len() - 1];
                let empty_partition = Partition::default();
                let parent = match cache.get(parent_key) {
                    Some(p) => {
                        hits += 1;
                        p
                    }
                    None => {
                        misses += 1;
                        &empty_partition
                    }
                };
                let (refined, _, _) = refiner.refine(parent, &encoded[last].0, rows, true);
                cache.insert(combo.clone(), refined.unwrap_or_default());
            }
            let partition = &cache[&combo];
            let base_count = partition.class_count;
            if base_count == 0 {
                continue;
            }
            let is_superkey = base_count == rows;
            if prune_near && !is_superkey && base_count as f64 / rows as f64 > near_key_ratio {
                continue;
            }
            if !is_superkey && rows < min_rows_nonkey {
                continue;
            }
            let names: Vec<String> = combo.iter().map(|&i| attributes[i].clone()).collect();
            for rhs in 0..attributes.len() {
                if combo.contains(&rhs) {
                    continue;
                }
                let existing = mined_by_rhs.get(&rhs);
                if existing
                    .map(|l| {
                        l.iter()
                            .any(|(c, conf)| *conf == 1.0 && c.iter().all(|a| combo.contains(a)))
                    })
                    .unwrap_or(false)
                {
                    continue;
                }
                evaluated += 1;
                let (holds, split_rows, confidence) = if is_superkey {
                    (true, 0usize, 1.0)
                } else {
                    let (_, split, count) = refiner.refine(partition, &encoded[rhs].0, rows, false);
                    let holds = count == base_count;
                    if !holds && min_conf >= 1.0 {
                        continue;
                    }
                    let conf = if holds { 1.0 } else { 1.0 - split as f64 / rows as f64 };
                    if conf < min_conf {
                        continue;
                    }
                    (holds, split, conf)
                };
                if !holds
                    && existing
                        .map(|l| {
                            l.iter()
                                .any(|(c, conf)| *conf >= confidence && c.iter().all(|a| combo.contains(a)))
                        })
                        .unwrap_or(false)
                {
                    continue;
                }
                if holds {
                    exact += 1;
                }
                results.push(MinedFd {
                    lhs: names.clone(),
                    rhs: attributes[rhs].clone(),
                    confidence: if holds { 1.0 } else { confidence },
                    coverage: base_count as f64 / rows as f64,
                    violations: if holds { 0.0 } else { split_rows as f64 },
                    unique_groups: base_count as f64,
                });
                mined_by_rhs.entry(rhs).or_default().push((combo.clone(), confidence));
            }
            if is_superkey {
                superkeys.push(combo.clone());
            }
        }
        level_reached = level;
    }
    sort_fds(&mut results);
    MineResult {
        fds: results,
        duration_ms: (js::now_ms() - started).round(),
        combinations_evaluated: evaluated,
        exact_count: exact,
        truncated,
        level_reached,
        partition_cache: Some(CacheStats { hits, misses }),
        sampling: None,
    }
}

fn imul(a: u32, b: u32) -> u32 {
    a.wrapping_mul(b)
}

/// Deterministic PRNG (mulberry32), same sequence as the JavaScript engine.
fn mulberry32(seed: u32) -> impl FnMut() -> f64 {
    let mut a = seed;
    move || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = a;
        t = imul(t ^ (t >> 15), t | 1);
        t ^= t.wrapping_add(imul(t ^ (t >> 7), t | 61));
        (t ^ (t >> 14)) as f64 / 4_294_967_296.0
    }
}

pub fn reservoir_sample_indices(total: usize, size: usize, seed: u32) -> Vec<usize> {
    if size >= total {
        return (0..total).collect();
    }
    let mut rand = mulberry32(seed);
    let mut res: Vec<usize> = (0..size).collect();
    for i in size..total {
        let j = (rand() * (i + 1) as f64).floor() as usize;
        if j < size {
            res[j] = i;
        }
    }
    res.sort_unstable();
    res
}

pub struct Validation {
    pub fd: MinedFd,
    pub witness: Vec<usize>,
}

/// Group id of every row for one determinant (rows agreeing on all `cols` share an id).
fn group_ids(cols: &[&Vec<u32>], rows: usize) -> (Vec<u32>, usize) {
    match cols.len() {
        0 => (vec![0; rows], usize::from(rows > 0)),
        1 => {
            let mut map: HashMap<u32, u32> = HashMap::new();
            let ids = cols[0]
                .iter()
                .map(|&c| {
                    let n = map.len() as u32;
                    *map.entry(c).or_insert(n)
                })
                .collect();
            (ids, map.len())
        }
        _ => {
            let mut map: HashMap<Vec<u32>, u32> = HashMap::new();
            let mut key = Vec::with_capacity(cols.len());
            let ids = (0..rows)
                .map(|r| {
                    key.clear();
                    key.extend(cols.iter().map(|c| c[r]));
                    if let Some(&id) = map.get(&key) {
                        id
                    } else {
                        let n = map.len() as u32;
                        map.insert(key.clone(), n);
                        n
                    }
                })
                .collect();
            (ids, map.len())
        }
    }
}

/// Check dependencies against every row.
pub fn validate(data: &Table, fds: &[MinedFd], ns: NullSemantics) -> Vec<Validation> {
    if fds.is_empty() || data.is_empty() {
        return vec![];
    }
    let attributes = data.keys();
    let needed: Vec<String> = attributes
        .iter()
        .filter(|a| fds.iter().any(|f| f.rhs == **a || f.lhs.contains(a)))
        .cloned()
        .collect();
    let enc = encode_columns(data, &needed, ns);
    let by_name: HashMap<&str, &Vec<u32>> = needed
        .iter()
        .map(|s| s.as_str())
        .zip(enc.iter().map(|(c, _)| c))
        .collect();
    let rows = data.len();
    // Many dependencies share a determinant: group the rows once per determinant.
    let mut groups: HashMap<Vec<String>, (Vec<u32>, usize)> = HashMap::new();
    fds.iter()
        .map(|fd| {
            let rhs = by_name.get(fd.rhs.as_str());
            let lhs: Option<Vec<&Vec<u32>>> = fd.lhs.iter().map(|a| by_name.get(a.as_str()).copied()).collect();
            let (Some(rhs), Some(lhs)) = (rhs, lhs) else {
                return Validation {
                    fd: fd.clone(),
                    witness: vec![],
                };
            };
            let (ids, n_groups) = groups.entry(fd.lhs.clone()).or_insert_with(|| group_ids(&lhs, rows));
            // Fast pass: first row and RHS code per group; remember the first conflicting pair.
            let mut first: Vec<u32> = vec![u32::MAX; *n_groups];
            let mut conflict: Vec<usize> = vec![];
            for r in 0..rows {
                let g = ids[r] as usize;
                let p = first[g];
                if p == u32::MAX {
                    first[g] = r as u32;
                } else if rhs[p as usize] != rhs[r] && conflict.is_empty() {
                    conflict = vec![p as usize, r];
                }
            }
            let g = *n_groups as f64;
            if conflict.is_empty() {
                return Validation {
                    fd: MinedFd {
                        confidence: 1.0,
                        violations: 0.0,
                        unique_groups: g,
                        coverage: g / rows as f64,
                        ..fd.clone()
                    },
                    witness: vec![],
                };
            }
            // Slow pass: rows outside the majority RHS value of each group.
            let mut counts: HashMap<(u32, u32), usize> = HashMap::new();
            for r in 0..rows {
                *counts.entry((ids[r], rhs[r])).or_insert(0) += 1;
            }
            let mut total = vec![0usize; *n_groups];
            let mut max = vec![0usize; *n_groups];
            for (&(gid, _), &n) in &counts {
                total[gid as usize] += n;
                max[gid as usize] = max[gid as usize].max(n);
            }
            let violations: usize = (0..*n_groups).map(|i| total[i] - max[i]).sum();
            Validation {
                fd: MinedFd {
                    confidence: 1.0 - violations as f64 / rows as f64,
                    violations: violations as f64,
                    unique_groups: g,
                    coverage: g / rows as f64,
                    ..fd.clone()
                },
                witness: conflict,
            }
        })
        .collect()
}

/// Mine on a reservoir sample, then validate every reported dependency on all rows.
pub fn mine_sampled(data: &Table, options: &MineOptions) -> MineResult {
    let sample_size = options.sample_size.unwrap_or(10_000);
    if data.len() <= sample_size {
        return mine(data, options);
    }
    let started = js::now_ms();
    let min_conf = options.min_confidence.unwrap_or(0.85);
    let max_rounds = options.max_rounds.unwrap_or(6);
    let mut chosen: std::collections::BTreeSet<usize> =
        reservoir_sample_indices(data.len(), sample_size, options.seed.unwrap_or(42))
            .into_iter()
            .collect();
    let (mut rows_added, mut refuted, mut rounds) = (0usize, 0usize, 0usize);
    let mut result = MineResult::default();
    let mut totals = CacheStats::default();
    let mut validated: Vec<Validation> = vec![];
    while rounds < max_rounds {
        rounds += 1;
        let idx: Vec<usize> = chosen.iter().copied().collect();
        let sample = data.select_rows(&idx);
        result = mine(&sample, options);
        if let Some(c) = &result.partition_cache {
            totals.hits += c.hits;
            totals.misses += c.misses;
        }
        validated = validate(data, &result.fds, options.null_semantics);
        let failed: Vec<&Validation> = result
            .fds
            .iter()
            .zip(&validated)
            .filter(|(fd, v)| fd.confidence == 1.0 && v.fd.confidence < 1.0)
            .map(|(_, v)| v)
            .collect();
        if failed.is_empty() || result.truncated || options.aborted.map(|f| f()).unwrap_or(false) {
            break;
        }
        let mut added = 0;
        for f in failed {
            for &r in &f.witness {
                if chosen.insert(r) {
                    added += 1;
                }
            }
        }
        rows_added += added;
        if added == 0 {
            break;
        }
    }
    let mut final_fds = Vec::new();
    for (i, fd) in result.fds.iter().enumerate() {
        let v = validated.get(i).map(|v| v.fd.clone()).unwrap_or_else(|| fd.clone());
        if fd.confidence == 1.0 && v.confidence < 1.0 {
            refuted += 1;
        }
        if v.confidence == 1.0 || v.confidence >= min_conf {
            final_fds.push(v);
        }
    }
    sort_fds(&mut final_fds);
    let exact = final_fds.iter().filter(|f| f.confidence == 1.0).count();
    MineResult {
        fds: final_fds,
        exact_count: exact,
        duration_ms: (js::now_ms() - started).round(),
        partition_cache: Some(totals),
        sampling: Some(SamplingInfo {
            sampled: true,
            total_rows: data.len(),
            sample_size: chosen.len(),
            rounds,
            rows_added,
            refuted,
        }),
        ..result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mines_simple_dependency() {
        let rows: Vec<serde_json::Value> = (0..20)
            .map(|i| serde_json::json!({ "id": i.to_string(), "city": format!("c{}", i % 4), "zip": format!("z{}", i % 4) }))
            .collect();
        let t = Table::from_json_rows(rows);
        let r = mine(
            &t,
            &MineOptions {
                max_lhs_size: Some(2),
                ..Default::default()
            },
        );
        assert!(r
            .fds
            .iter()
            .any(|f| f.lhs == vec!["city".to_string()] && f.rhs == "zip" && f.confidence == 1.0));
        assert!(r.fds.iter().any(|f| f.lhs == vec!["id".to_string()] && f.rhs == "city"));
    }

    #[test]
    fn mulberry_matches_js() {
        // First value of mulberry32(42) in JavaScript: 0.6011037519201636
        let mut r = mulberry32(42);
        assert!((r() - 0.6011037519201636).abs() < 1e-15);
    }
}
