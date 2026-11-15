//! Multivalued dependencies (MVDs) and Fourth Normal Form.
//!
//! An MVD `X ->> Y` holds in a relation `R` when, for every value of `X`, the rows are exactly every combination
//! of their `Y` values with their `Z = R − X − Y` values: `Y` and `Z` vary independently given `X`. A relation is
//! in 4NF when every nontrivial MVD has a superkey on its left. BCNF only looks at functional dependencies, so a
//! BCNF relation such as (course, teacher, book), where a course's teachers and books are independent, keeps
//! repeating every teacher–book pairing; 4NF splits it into (course, teacher) and (course, book).
//!
//! MVDs are found on the data. A test is exact (every `X` group must be a complete product), and an MVD is only
//! reported with evidence: at least one group where both sides take several values (so at least four rows). A
//! functional dependency is never reported as an MVD here (that case is BCNF's). The decomposition is Fagin's:
//! `R` becomes `X ∪ Y` and `X ∪ Z`, which is lossless exactly when `X ->> Y` holds; the chase below proves it.

use crate::js;
use crate::naming::new_relation_id;
use crate::normalization as nf;
use crate::table::Table;
use crate::types::{Fd, Relation};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Relations wider than this are not searched (the number of candidate MVDs grows combinatorially).
pub const MAX_MVD_ATTRIBUTES: usize = 16;
/// Largest left-hand side and right-hand side tried.
const MAX_LHS: usize = 2;
const MAX_RHS: usize = 2;
/// Time budget for the whole 4NF step of one table.
const BUDGET_MS: f64 = 15_000.0;

/// A multivalued dependency found on the data, with its evidence.
#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Mvd {
    pub lhs: Vec<String>,
    pub rhs: Vec<String>,
    /// The rest of the relation (`R − X − Y`): `X ->> rest` holds too.
    pub rest: Vec<String>,
    /// The relation it was found in.
    pub relation: String,
    /// Groups of rows (one per `X` value) where both sides take several values.
    pub witness_groups: usize,
    pub witness_rows: usize,
}

/// Rows of the relation as distinct integer tuples (one dictionary per column).
fn encode(rows: &Table, attrs: &[String]) -> Vec<Vec<u32>> {
    let cols: Vec<Option<usize>> = attrs.iter().map(|a| rows.col(a)).collect();
    let mut dicts: Vec<HashMap<String, u32>> = vec![HashMap::new(); attrs.len()];
    let mut seen: HashSet<Vec<u32>> = HashSet::with_capacity(rows.len());
    let mut out = Vec::with_capacity(rows.len());
    for r in 0..rows.len() {
        let t: Vec<u32> = cols
            .iter()
            .zip(dicts.iter_mut())
            .map(|(&c, d)| {
                let v = rows.get_idx(r, c).text_or_empty();
                if let Some(&code) = d.get(v.as_ref()) {
                    return code;
                }
                let n = d.len() as u32;
                d.insert(v.into_owned(), n);
                n
            })
            .collect();
        if seen.insert(t.clone()) {
            out.push(t);
        }
    }
    out
}

fn combinations(items: &[usize], k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut idx: Vec<usize> = (0..k).collect();
    if k == 0 || k > items.len() {
        return out;
    }
    loop {
        out.push(idx.iter().map(|&i| items[i]).collect());
        let mut i = k;
        while i > 0 && idx[i - 1] == items.len() - k + i - 1 {
            i -= 1;
        }
        if i == 0 {
            return out;
        }
        idx[i - 1] += 1;
        for j in i..k {
            idx[j] = idx[j - 1] + 1;
        }
    }
}

/// The values of columns `cols` of a tuple as one number: exact for up to two columns (dictionary codes are
/// 32-bit), a 64-bit fingerprint for more (used only for the remaining columns of a candidate MVD).
fn pack(t: &[u32], cols: &[usize]) -> u64 {
    match cols {
        [a] => t[*a] as u64,
        [a, b] => ((t[*a] as u64) << 32) | t[*b] as u64,
        _ => cols.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &c| {
            (h ^ t[c] as u64).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(17)
        }),
    }
}

/// Evidence for `X ->> Y` (groups and rows where both sides vary), or `None` when it does not hold.
fn test_mvd(groups: &[Vec<u32>], tuples: &[Vec<u32>], y: &[usize], z: &[usize]) -> Option<(usize, usize)> {
    let (mut wg, mut wr) = (0, 0);
    let (mut ys, mut zs) = (HashSet::new(), HashSet::new());
    for g in groups {
        ys.clear();
        zs.clear();
        for &i in g {
            let t = &tuples[i as usize];
            ys.insert(pack(t, y));
            zs.insert(pack(t, z));
        }
        // Rows are distinct, so the group is the full product exactly when its size is |Y values| x |Z values|.
        if ys.len() * zs.len() != g.len() {
            return None;
        }
        if ys.len() > 1 && zs.len() > 1 {
            wg += 1;
            wr += g.len();
        }
    }
    (wg >= 1 && wr >= 4).then_some((wg, wr))
}

/// The same test with the full column values (no fingerprints): run on a candidate before it is reported.
fn exact_test(groups: &[Vec<u32>], tuples: &[Vec<u32>], y: &[usize], z: &[usize]) -> bool {
    groups.iter().all(|g| {
        let proj = |cols: &[usize]| -> HashSet<Vec<u32>> {
            g.iter()
                .map(|&i| cols.iter().map(|&c| tuples[i as usize][c]).collect())
                .collect()
        };
        proj(y).len() * proj(z).len() == g.len()
    })
}

/// A nontrivial MVD whose left side is not a key of the relation (a 4NF violation), smallest sides first.
pub fn find_violating_mvd(name: &str, attrs: &[String], rows: &Table, deadline: f64) -> Option<Mvd> {
    let k = attrs.len();
    if !(3..=MAX_MVD_ATTRIBUTES).contains(&k) {
        return None;
    }
    let tuples = encode(rows, attrs);
    if tuples.len() < 4 {
        return None;
    }
    // A left side containing a column that is unique on its own is a key: skip it without grouping.
    let unique: Vec<bool> = (0..k)
        .map(|c| {
            let mut seen = HashSet::with_capacity(tuples.len());
            tuples.iter().all(|t| seen.insert(t[c]))
        })
        .collect();
    let all: Vec<usize> = (0..k).collect();
    let mut by_x: HashMap<u64, Vec<u32>> = HashMap::new();
    for lsize in 1..=MAX_LHS.min(k - 2) {
        for x in combinations(&all, lsize) {
            if x.iter().any(|&c| unique[c]) {
                continue;
            }
            if js::now_ms() > deadline {
                return None;
            }
            by_x.clear();
            for (i, t) in tuples.iter().enumerate() {
                by_x.entry(pack(t, &x)).or_default().push(i as u32);
            }
            // X is a key on this data: every MVD with X on the left is fine for 4NF.
            if by_x.len() == tuples.len() {
                continue;
            }
            // Only groups of two or more rows can disprove (or witness) an MVD.
            let groups: Vec<Vec<u32>> = by_x.drain().map(|(_, g)| g).filter(|g| g.len() > 1).collect();
            let rest: Vec<usize> = all.iter().copied().filter(|c| !x.contains(c)).collect();
            for rsize in 1..=MAX_RHS.min(rest.len() / 2) {
                for y in combinations(&rest, rsize) {
                    // X ->> Y and X ->> Z are the same split; with equal sizes, test it once.
                    if rsize * 2 == rest.len() && y[0] != rest[0] {
                        continue;
                    }
                    let z: Vec<usize> = rest.iter().copied().filter(|c| !y.contains(c)).collect();
                    if let Some((wg, wr)) =
                        test_mvd(&groups, &tuples, &y, &z).filter(|_| exact_test(&groups, &tuples, &y, &z))
                    {
                        let names = |cs: &[usize]| cs.iter().map(|&c| attrs[c].clone()).collect::<Vec<_>>();
                        return Some(Mvd {
                            lhs: names(&x),
                            rhs: names(&y),
                            rest: names(&z),
                            relation: name.to_string(),
                            witness_groups: wg,
                            witness_rows: wr,
                        });
                    }
                }
            }
        }
    }
    None
}

/// A key for a projection: a candidate key from the dependencies that apply to it, when it is unique on the data;
/// otherwise every attribute (relations split off by an MVD are usually all-key).
fn key_for(attrs: &[String], rows: &Table, fds: &[Fd]) -> Vec<String> {
    let projected: Vec<Fd> = fds
        .iter()
        .filter(|f| f.lhs.iter().all(|a| attrs.contains(a)) && f.rhs.list().iter().all(|r| attrs.contains(r)))
        .cloned()
        .collect();
    let unique = |key: &[String]| {
        let cols: Vec<Option<usize>> = key.iter().map(|a| rows.col(a)).collect();
        let mut seen = HashSet::with_capacity(rows.len());
        (0..rows.len()).all(|r| {
            seen.insert(
                cols.iter()
                    .map(|&c| rows.get_idx(r, c).text_or_empty().into_owned())
                    .collect::<Vec<_>>(),
            )
        })
    };
    nf::find_candidate_keys(attrs, &projected)
        .into_iter()
        .filter(|k| k.len() < attrs.len())
        .min_by_key(|k| k.len())
        .filter(|k| unique(k))
        .unwrap_or_else(|| attrs.to_vec())
}

pub struct FourNf {
    pub relations: Vec<Relation>,
    pub mvds: Vec<Mvd>,
    pub notes: Vec<String>,
}

/// Decompose every relation until no nontrivial MVD has a non-key left side (Fagin's 4NF decomposition).
/// Relations without rows (released for large inputs) and very wide relations are kept as they are.
pub fn apply_4nf(relations: Vec<Relation>, fds: &[Fd], table_name: &str) -> FourNf {
    let deadline = js::now_ms() + BUDGET_MS;
    let mut work: std::collections::VecDeque<Relation> = relations.into();
    let (mut out, mut mvds, mut notes) = (Vec::new(), Vec::new(), Vec::new());
    let mut wide: Vec<usize> = Vec::new();
    let mut splits = 0;
    while let Some(rel) = work.pop_front() {
        if rel.rows.is_empty() || splits >= 64 {
            out.push(rel);
            continue;
        }
        if rel.attributes.len() > MAX_MVD_ATTRIBUTES {
            wide.push(rel.attributes.len());
            out.push(rel);
            continue;
        }
        let Some(m) = find_violating_mvd(&rel.name, &rel.attributes, &rel.rows, deadline) else {
            out.push(rel);
            continue;
        };
        splits += 1;
        let keep = |side: &[String]| -> Vec<String> {
            rel.attributes
                .iter()
                .filter(|a| m.lhs.contains(a) || side.contains(a))
                .cloned()
                .collect()
        };
        for attrs in [keep(&m.rhs), keep(&m.rest)] {
            let rows = nf::extract_relation_data(&rel.rows, &attrs);
            let pk = key_for(&attrs, &rows, fds);
            work.push_front(nf::make_relation(new_relation_id(), attrs, pk, rows, "4NF_Decomposed"));
        }
        mvds.push(m);
    }
    if !wide.is_empty() {
        notes.push(format!(
            "{table_name}: 4NF searches relations of up to {MAX_MVD_ATTRIBUTES} columns; {} wider {} ({} columns) {} not searched.",
            wide.len(),
            if wide.len() == 1 { "relation" } else { "relations" },
            wide.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "),
            if wide.len() == 1 { "was" } else { "were" }
        ));
    }
    if js::now_ms() > deadline {
        notes.push(format!(
            "{table_name}: the 4NF search reached its time limit; some multivalued dependencies may be missing."
        ));
    }
    nf::establish_foreign_keys(&mut out);
    FourNf {
        relations: out,
        mvds,
        notes,
    }
}

/// Lossless-join test by the chase with functional and multivalued dependencies. Each relation contributes a
/// tableau row (`a` symbols on its attributes, unique `b` symbols elsewhere); an FD equates symbols, an MVD
/// `X ->> Y` adds the row that takes `X ∪ Y` from one row and the rest from another that agrees on `X`. The join
/// is lossless when a row of only `a` symbols appears.
pub fn chase_lossless(relations: &[Relation], fds: &[Fd], mvds: &[Mvd]) -> bool {
    let mut all: Vec<String> = Vec::new();
    for r in relations {
        for a in &r.attributes {
            if !all.contains(a) {
                all.push(a.clone());
            }
        }
    }
    let n = all.len();
    if relations.len() <= 1 {
        return true;
    }
    let pos = |a: &str| all.iter().position(|x| x == a);
    // Symbol 0..n are the distinguished `a_j`; larger numbers are `b` symbols.
    let mut next = n as u32;
    let mut t: Vec<Vec<u32>> = relations
        .iter()
        .map(|r| {
            (0..n)
                .map(|j| {
                    if r.attributes.contains(&all[j]) {
                        j as u32
                    } else {
                        next += 1;
                        next
                    }
                })
                .collect()
        })
        .collect();
    let distinguished = |t: &[Vec<u32>]| t.iter().any(|row| row.iter().enumerate().all(|(j, &s)| s == j as u32));
    let fd_rules: Vec<(Vec<usize>, usize)> = fds
        .iter()
        .flat_map(|f| {
            let lhs: Option<Vec<usize>> = f.lhs.iter().map(|a| pos(a)).collect();
            f.rhs
                .list()
                .into_iter()
                .filter_map(move |r| Some((lhs.clone()?, pos(&r)?)))
        })
        .collect();
    let mvd_rules: Vec<(Vec<usize>, Vec<usize>)> = mvds
        .iter()
        .filter_map(|m| {
            let x: Vec<usize> = m.lhs.iter().map(|a| pos(a)).collect::<Option<_>>()?;
            let y: Vec<usize> = m.rhs.iter().filter_map(|a| pos(a)).collect();
            Some((x, y))
        })
        .collect();
    for _ in 0..200 {
        if distinguished(&t) {
            return true;
        }
        let mut changed = false;
        for (lhs, rhs) in &fd_rules {
            for i in 0..t.len() {
                for j in i + 1..t.len() {
                    if lhs.iter().all(|&c| t[i][c] == t[j][c]) && t[i][*rhs] != t[j][*rhs] {
                        let (keep, drop) = (t[i][*rhs].min(t[j][*rhs]), t[i][*rhs].max(t[j][*rhs]));
                        for row in t.iter_mut() {
                            if row[*rhs] == drop {
                                row[*rhs] = keep;
                            }
                        }
                        changed = true;
                    }
                }
            }
        }
        let rows_now = t.len();
        for (x, y) in &mvd_rules {
            for i in 0..rows_now {
                for j in 0..rows_now {
                    if i == j || !x.iter().all(|&c| t[i][c] == t[j][c]) {
                        continue;
                    }
                    let row: Vec<u32> = (0..n)
                        .map(|c| {
                            if x.contains(&c) || y.contains(&c) {
                                t[i][c]
                            } else {
                                t[j][c]
                            }
                        })
                        .collect();
                    if !t.contains(&row) && t.len() < 4096 {
                        t.push(row);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    distinguished(&t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::Cell;

    fn table(cols: &[&str], rows: &[&[&str]]) -> Table {
        let names: Vec<String> = cols.iter().map(|c| c.to_string()).collect();
        let mut t = Table::with_columns(&names);
        for r in rows {
            t.push_row(r.iter().map(|v| Cell::str(*v)).collect());
        }
        t
    }

    fn ctb() -> Table {
        let mut rows: Vec<Vec<&str>> = Vec::new();
        for (course, teachers, books) in [
            ("Databases", vec!["Ann", "Bob"], vec!["Date", "Ullman", "Elmasri"]),
            ("Physics", vec!["Cy"], vec!["Feynman", "Halliday"]),
            ("Algebra", vec!["Dee", "Eve"], vec!["Artin", "Lang"]),
        ] {
            for t in &teachers {
                for b in &books {
                    rows.push(vec![course, t, b]);
                }
            }
        }
        let refs: Vec<&[&str]> = rows.iter().map(|r| r.as_slice()).collect();
        table(&["course", "teacher", "book"], &refs)
    }

    #[test]
    fn finds_the_course_teacher_book_mvd() {
        let attrs: Vec<String> = ["course", "teacher", "book"].iter().map(|s| s.to_string()).collect();
        let m = find_violating_mvd("ctb", &attrs, &ctb(), f64::MAX).expect("an MVD");
        assert_eq!(m.lhs, vec!["course"]);
        assert!(m.rhs == vec!["teacher"] || m.rhs == vec!["book"], "{m:?}");
        assert_eq!(m.witness_groups, 2);
    }

    #[test]
    fn decomposes_into_4nf_losslessly() {
        let data = ctb();
        let attrs: Vec<String> = ["course", "teacher", "book"].iter().map(|s| s.to_string()).collect();
        let rel = nf::make_relation("ctb".into(), attrs.clone(), attrs.clone(), data, "BCNF_Decomposed");
        let r = apply_4nf(vec![rel], &[], "ctb");
        assert_eq!(r.relations.len(), 2);
        let mut sets: Vec<Vec<String>> = r.relations.iter().map(|r| r.attributes.clone()).collect();
        sets.sort();
        assert_eq!(sets, vec![vec!["course", "book"], vec!["course", "teacher"]]);
        assert_eq!(r.relations.iter().map(|r| r.rows.len()).sum::<usize>(), 7 + 5);
        assert!(chase_lossless(&r.relations, &[], &r.mvds));
        // Without the MVD the chase cannot show it (no functional dependencies hold).
        assert!(!chase_lossless(&r.relations, &[], &[]));
    }

    #[test]
    fn no_mvd_without_evidence_or_for_a_functional_dependency() {
        let attrs: Vec<String> = ["id", "name", "city"].iter().map(|s| s.to_string()).collect();
        // id is a key: nothing to report.
        let keyed = table(
            &["id", "name", "city"],
            &[&["1", "a", "x"], &["2", "b", "y"], &["3", "c", "x"], &["4", "d", "y"]],
        );
        assert!(find_violating_mvd("t", &attrs, &keyed, f64::MAX).is_none());
        // name -> city is an FD (BCNF's case), not reported as an MVD.
        let fd = table(
            &["id", "name", "city"],
            &[
                &["1", "a", "x"],
                &["2", "a", "x"],
                &["3", "b", "y"],
                &["4", "b", "y"],
                &["5", "a", "x"],
            ],
        );
        assert!(find_violating_mvd("t", &attrs, &fd, f64::MAX).is_none());
        // Varying sides that are not a complete product: no MVD.
        let partial = table(
            &["c", "t", "b"],
            &[
                &["1", "a", "x"],
                &["1", "a", "y"],
                &["1", "b", "x"],
                &["2", "c", "z"],
                &["2", "d", "w"],
            ],
        );
        let cols: Vec<String> = ["c", "t", "b"].iter().map(|s| s.to_string()).collect();
        assert!(find_violating_mvd("t", &cols, &partial, f64::MAX).is_none());
    }
}
