# Evaluation

`bun run bench` (or `bun run bench --scale 5 --json`) runs the engine on
ground-truth databases defined in `src/eval/datasets.ts` and reports
precision / recall for:

- **Primary keys** and **foreign keys**, from the tables exported as separate
  key-less CSV files (composite keys and a junction table included).
- **Recovered relations**, from one denormalized join of all tables: a relation
  counts as recovered when its attribute set equals an original table's.
- Runtime for both runs.

Datasets are synthetic (seeded, so runs repeat): `retail` (customers, products,
orders, order_items with a composite key) and `university` (students, majors,
courses, enrolments as a junction table). Real benchmarks (TPC-H, Northwind,
Chinook, Sakila, MusicBrainz) are not included yet.

## Latest numbers (scale 1)

| Dataset | PK P/R | FK P/R | Relations P/R |
|---|---|---|---|
| retail | 100% / 100% | 100% / 100% | 22% / 50% |
| university | 100% / 100% | 100% / 100% | 50% / 50% |

Reading them: key and foreign-key detection is solid on these schemas.
Recovering the original tables from a *denormalized* file is weak: with a few
hundred rows many coincidental dependencies hold (for example
`customer_name, order_date → order_id`), and 3NF synthesis turns them into
extra relations. Ranking FDs by plausibility and asking the user to accept or
reject them is the planned fix; this harness is how
to measure it.

`src/eval/harness.test.ts` keeps a regression floor (recall and precision of
keys and foreign keys at or above 90%).

## Scalability

`examples/demo_data.rs` generates a deterministic retail database exported as nine denormalized CSV files of 99 columns
each: customers (9,999 rows), suppliers (999), products (9,999), stores (999), employees (9,999), orders (99,999), order
lines (99,999), shipments (99,999) and payments (99,999) — 431,991 rows, 891 columns, 335 MB. Each file copies facts
about other entities (a city and its region, a category and its department, a carrier, a payment method), so 3NF has
transitive dependencies to remove, and nine foreign keys connect the files.

```
cargo run --release -p dbflow-core --example demo_data -- demo
cargo run --release -p dbflow-core --example bench -- demo/*.csv
DBFLOW_BENCH_OPTIONS='{"maxParallelFiles":1}' cargo run --release -p dbflow-core --example bench -- demo/*.csv
```

All 19 operations, 4-core machine, native engine:

| Run | Analysis | Outputs | Total | Peak memory |
|---|---|---|---|---|
| One file at a time | 104.1 s | 11.2 s | 115.3 s | 5.8 GB |
| Files in parallel (default) | 41.8 s | 12.9 s | 54.7 s | 7.8 GB |
| Before this revision | — | — | out of memory in key discovery on the first table | — |

Result: 19,806 dependencies, 197 candidate keys, 39 relations, all nine foreign keys found (plus three
links through unique name columns), every decomposition lossless by the chase (seven of nine also confirmed by
re-joining the data; the orders and payments joins exceed the 250,000-row limit of that check) and every dependency
preserved. The
composite key of order lines, `(order_id, line_no)`, is found although `order_id` looks nearly unique in the 10,000-row
sample. Limits: some of the 38 relations come from coincidental dependencies of wide tables (on orders, customer,
employee and date together identify an order); they are correct for the data but not meaningful.

Single-table series (`demo_data -- <dir> --single ROWS COLS`: an order table whose customer, city and region facts
3NF must split out, padded with measures):

| Rows × columns | CSV | Total time | Peak memory | Relations (3NF) | Lossless / preserved |
|---|---|---|---|---|---|
| 9,999 × 9 | 0.6 MB | 0.17 s | 37 MB | 4 | yes / 22 of 22 |
| 99,999 × 9 | 6.2 MB | 1.5 s | 0.3 GB | 4 | yes / 22 of 22 |
| 999,999 × 9 | 64 MB | 15.6 s | 1.4 GB | 4 | yes (chase) / 22 of 22 |
| 9,999 × 99 | 7.9 MB | 1.9 s | 0.5 GB | 4 | yes / 112 of 112 |
| 9,999 × 999 | 80 MB | 23.3 s | 1.8 GB | 4 | yes / 5,004 of 5,004 |

The same four relations (order, customer, city, region) are found at every size.

## Natural-language workflow builder

`bun run bench:nl` runs 77 labelled requests (`src/eval/nlWorkflowCases.ts`: direct keywords, acronyms, compound requests, negations, paraphrases, typos, and 10 junk sentences that must build nothing).

| Resolver | Plan accuracy | Junk rejected | By kind |
|---|---|---|---|
| Keywords only | 78% | 10 / 10 | keyword 20/20, acronym 10/10, compound 8/8, negation 4/4, paraphrase 7/17, typo 3/8 |
| Bundled model only | 79% | 10 / 10 | keyword 19/20, acronym 7/10, compound 3/8, negation 3/4, paraphrase 15/17, typo 6/8 |
| **Hybrid (shipped)** | **94%** | **10 / 10** | keyword 20/20, acronym 10/10, compound 8/8, negation 4/4, paraphrase 15/17, typo 6/8 |

The model is `Xenova/all-MiniLM-L6-v2` (int8, 384-dim). Thresholds in `DEFAULT_THRESHOLDS` (`accept 0.40`, `margin 0.02`, `floor 0.30`) are the middle of a plateau found with `bun run bench:nl --sweep`; the best single point (margin 0, 96%) was not used because it sits on the edge of the plateau.

**Caveat.** Thresholds were tuned and scored on the same 77 cases, so real-world accuracy will be lower. Remaining misses: "is anything lost when the tables are joined back together" (picks Relation Detector instead of Lossless Check), "make sure the split kept all the dependencies" (Lossless Check instead of Dependency Preservation) and "clen the dat". Add held-out cases before trusting the figure.

An earlier attempt used EmbeddingGemma through a local Ollama server. It was dropped because it required users to install and run a separate program; a 23 MB bundled model gives good results without that.

Reproduce: `bun run bench:nl --sweep --failures`.
