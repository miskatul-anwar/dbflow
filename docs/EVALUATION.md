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
| One file at a time | 107.8 s | 13.1 s | 120.9 s | 6.0 GB |
| Files in parallel (default) | 45.9 s | 13.8 s | 59.7 s | 7.7 GB |
| Before this revision | — | — | out of memory in key discovery on the first table | — |

Result: 19,801 exact dependencies, 197 candidate keys, 38 relations, all nine foreign keys found (plus three
links through unique name columns), every decomposition lossless by the chase (eight of nine also confirmed by
re-joining the data; the orders join exceeds the 250,000-row limit of that check) and every dependency preserved. The
composite key of order lines, `(order_id, line_no)`, is found although `order_id` looks nearly unique in the 10,000-row
sample. Limits: some of the 38 relations come from coincidental dependencies of wide tables (on orders, customer,
employee and date together identify an order); they are correct for the data but not meaningful.

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
