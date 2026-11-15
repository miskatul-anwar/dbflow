<div align="center">

<img src="public/icon.png" alt="DBFlow logo" width="120" />

# DBFlow

**Turn raw CSV and spreadsheet files into a verified, normalized relational database: ER diagram, SQL schema, normalized data and a report.**

[![License: MIT](https://img.shields.io/badge/License-MIT-115e59.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/miskatul-anwar/dbflow?logo=github&color=0284c7)](https://github.com/miskatul-anwar/dbflow/releases)
[![CI](https://github.com/miskatul-anwar/dbflow/actions/workflows/ci.yml/badge.svg)](https://github.com/miskatul-anwar/dbflow/actions/workflows/ci.yml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-FFC131?logo=tauri&logoColor=black)](https://tauri.app)
[![Engine: Rust](https://img.shields.io/badge/Engine-Rust-b7410e?logo=rust&logoColor=white)](crates/dbflow-core)

*One desktop app from raw tables to 1NF, 2NF, 3NF or BCNF, an ER diagram and SQL. Private by design: your data never leaves your machine.*

</div>

---

## What is DBFlow?

DBFlow is a **free, open-source desktop app** for Windows, macOS and Linux that designs a relational database from the data you already have. It serves two purposes:

- **Database engineering.** Turn flat exports into a normalized, populated database, with the schema checked against your data.
- **Database education.** See every stage of database design, from the first functional dependency to the final SQL, change one step and compare the results.

Drop in one or many files: CSV, Excel workbooks and templates (`.xlsx`, `.xlsm`, `.xlsb`, `.xls`, `.xltx`, `.xltm`, `.xlam`, `.xlt`, `.xla`, `.xlw`), OpenDocument spreadsheets (`.ods`, `.ots`, `.fods`) and Excel 2003 SpreadsheetML (`.xml`). Each visible, non-empty sheet becomes a table. Files are checked first (see [Input checks](#input-checks)). DBFlow then:

- **cleans and profiles** the data (missing values, atomicity, types, column meaning);
- **mines the functional dependencies** that hold on every row (partition refinement, sampled and validated on the full data);
- **finds every candidate key** (attribute closure and Lucchesi–Osborn enumeration) and picks an identifier-like primary key;
- **finds foreign keys across files** (inclusion dependencies scored by containment, coverage, name and type);
- **normalizes** to 1NF, 2NF, 3NF (Bernstein synthesis) or BCNF;
- **verifies every split**: a chase proves the join lossless, a re-join of your rows confirms it, and a projected-closure test reports which dependencies are preserved;
- **generates** an ER diagram, SQL DDL, the normalized data and a report.

## Highlights

| | |
|---|---|
| **Private by design** | Everything runs locally and the app makes no network requests. The two small models it uses ship inside the installer. |
| **Verified results** | Only dependencies that hold on every row shape the schema, and every decomposition is checked on your data before the report shows it. |
| **Scales to wide, multi-file data** | Files are analysed in parallel within a memory budget. Nine related tables with 431,991 rows and 891 columns (335 MB) take about a minute on a four-core laptop; a table of 999,999 rows or 999 columns takes under half a minute. See [Scalability](#scalability). |
| **Visible, editable pipelines** | Type a goal in plain words or click operations on a React Flow canvas; preview the plan, change it and undo it. |
| **AI naming on or off** | New tables are named by a small on-device model, or, with the switch off, from their key columns, instantly. |
| **One Rust engine** | Parsing, mining, normalization and every output come from one Rust crate: native in the desktop app, WebAssembly in the browser and the command line. |
| **Portable outputs** | Mermaid and Graphviz diagrams, `.sql`, `.csv`, `.md`, `.pdf` and workflow `.json` files. |

## Installation

### Download (recommended)

Use the [**download page**](https://miskatul-anwar.github.io/dbflow/), which picks the installer for your system, or grab one from the [**Releases page**](https://github.com/miskatul-anwar/dbflow/releases):

| Platform | Artifact |
|---|---|
| Windows | `.msi` / `.exe` |
| macOS | `.dmg` / `.app` |
| Linux | `.deb` / `.rpm` / `.AppImage` |
| Android 7 or newer (64-bit ARM) | `.apk` |

> [!NOTE]
> macOS builds are unsigned. On first launch right-click → **Open**, or allow the app under *System Settings → Privacy & Security*. The Windows installer is not code-signed either, so SmartScreen may ask you to choose *More info → Run anyway*.

> [!NOTE]
> **Android.** The app is not in the Play Store. Open the `.apk` on the phone and allow your browser or file manager to install apps when Android asks. It runs the same engine as the desktop app, entirely on the phone, with an interface made for touch on phones and tablets and the same features: workspaces, *Data*, *Pipeline* and *Results* screens behind a bottom bar, goals, the request box with its plan preview, and a step checklist with undo, redo, open and save instead of the canvas. Files are chosen with the system file picker, and downloads open a *Save as* screen. Large inputs need a phone with plenty of memory. AI table names are off by default on a phone (Settings turns them on).

### Build from source

Prerequisites: **[Bun](https://bun.sh)**, **Rust ≥ 1.80** with the WebAssembly target, **wasm-bindgen-cli** (same version as the `wasm-bindgen` crate), and the [Tauri v2 system dependencies](https://tauri.app/start/prerequisites/).

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130
```

```bash
git clone https://github.com/miskatul-anwar/dbflow.git
cd dbflow
bun install
bun run tauri dev    # development (builds the engine first)
bun run tauri build  # production bundle
```

<details>
<summary><b>Linux system packages (Debian/Ubuntu)</b></summary>

```bash
sudo apt update && sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev patchelf
```
</details>

## Quick start

1. Launch DBFlow and give your first workspace a name.
2. Load one or more files with **Upload**.
3. Build a pipeline: type a goal such as *"normalize to 3NF, check lossless join, draw an ER diagram and generate SQL for PostgreSQL"* and press **Build**, or click operations in the sidebar (a locked step adds the steps it needs first).
4. Press **Run**, or **Output** to run and open the results in one go.
5. Browse the results: normal forms, dependencies, keys, cardinalities, the lossless-join and preservation checks, the ER diagram, the SQL schema, the generated files and the full report. Rename any new table, then download what you need.

## The pipeline

A pipeline is a chain of these 19 operations. Each one lists the steps it needs, and the planner, the canvas and the command line share this one definition ([`src/config/operations.ts`](src/config/operations.ts)).

| Phase | Operations |
|---|---|
| Prepare | **Data Loader** (CSV, Excel and ODS files), Data Cleaner, Atomicity Checker, Data Type Profiler, Column Classifier |
| Analyze | FD Miner, Key Discovery, Relation Detector (cross-file foreign keys), Cardinality Analyzer |
| Normalize | 1NF, 2NF, 3NF, BCNF, Lossless Check, Dependency Preservation |
| Output | ERD Generator, Relation Generator, SQL Generator, Report Generator |

Pipelines save to and load from versioned JSON (**Save** / **Open**). Older operation names, such as *CSV Data Loader* and *NER Classifier*, are still understood.

## Outputs

- **ER diagram** in Crow's foot, Chen, Barker or UML notation (Mermaid and Graphviz; copy or download as source or SVG).
- **SQL DDL** for PostgreSQL, MySQL, SQLite or standard SQL, with sized types, `NOT NULL`, `UNIQUE`, primary and foreign keys, tables ordered parents first.
- **Normalized data**: one CSV file per table, an INSERT script, and queries that check every foreign key after loading.
- **Report**: what each step found and every check's verdict, shown in the app and exported as Markdown or PDF.

For large inputs (more than about a million cells) the result stays compact: tables report row counts, and each table's data is exported once as CSV (the INSERT script is left out; the CSV files load faster).

## Input checks

Before any analysis, DBFlow checks what each file really contains, whatever its extension:

- **Not a table.** A PDF, an image, an archive, a program, a Word document, JSON, HTML or program source code is left out, with the reason. The other files are still analysed. A run fails only when no file is left, and then it says why.
- **Disguised workbooks.** An Excel or OpenDocument workbook saved with a `.csv` name is read as a workbook. Delimited text saved with a spreadsheet name is read as CSV.
- **Compression bombs.** A workbook is a ZIP file. DBFlow measures how far it expands before reading it, and refuses a file that expands implausibly far or needs more memory than the computer can spare.
- **Identical files.** A file with the same contents as another file is analysed once.
- **Files with the same name.** They become separate tables (`data`, `data_2`), and messages call them "data.csv (file 1)" and "data.csv (file 2)".
- **Overlapping files.** Files with the same columns and mostly the same rows are reported, with the number of shared rows and of records that conflict. **Merge overlapping files and run again** on the Output page combines them into one table. Repeated rows are kept once, and for conflicting records the first file's version is kept.

The Output page lists everything the checks found under *Input checks*.

## Workspaces

Work on several datasets at once. Each tab is a separate workspace with its own files, pipeline and results, and a run keeps going in the background while you use another tab or open a new one.

- `Ctrl/Cmd+T` opens a workspace (you name it first), `Ctrl/Cmd+W` closes one, `Ctrl+Tab` switches.
- Rename with the pencil button, a double-click, `F2` or the right-click menu; give a tab a colour with its dot.
- Closing a workspace with work in it asks first, in the app, and stops its run. Closing the last one asks for the name of the workspace that replaces it.
- Pressing **Output** while a run is in progress opens its results as soon as it finishes.

Workspaces live as long as the app is open; use **Save** to keep a pipeline.

## Request box

Type a request such as *"Can you pls convert it to 1NF"* in the bar above the canvas (`Ctrl/Cmd+K` focuses it). DBFlow shows the pipeline it would build, highlights the steps you named, adds the ones they need, shows the SQL system you asked for, and you press **Apply**, **Apply & Run** or **Cancel**. **Undo last build** restores the previous canvas in one step.

Exact keywords are matched first; a small sentence-embedding model (all-MiniLM-L6-v2, int8, 23 MB, Apache-2.0) bundled in the installer handles paraphrases. It runs inside the app on WebAssembly, so your sentence never leaves the machine. Accuracy: [docs/EVALUATION.md](docs/EVALUATION.md).

## Naming new tables

Normalization creates tables that your files did not have. The toolbar switch decides how they are named, for every workspace:

- **AI names** (default): a small on-device language model (SmolLM2) suggests a name from the column names only, never from cell values. Names are marked *suggested*.
- **Key names**: names are built from the table's key columns (`region_code` → *Region*, `order_id, line_no` → *Order Line*) and marked *from key*. No model runs, so large runs finish much sooner.

Either way you can rename a table on the Output page; DBFlow remembers your name for those columns on later runs.

## Scalability

Measured with the native engine, all 19 operations, on a four-core laptop:

| Input | Rows | Columns | Time | Peak memory |
|---|---|---|---|---|
| Nine related files (retail export) | 431,991 | 891 | 55 s (115 s on one core) | 7.8 GB |
| One table | 999,999 | 9 | 16 s | 1.4 GB |
| One table | 9,999 | 999 | 23 s | 1.8 GB |

What makes this possible: dependencies are mined on a sample and validated on all rows; a nearly unique column (and every set containing it) is not used as a determinant, since two unrelated measures together are a key only by accident; keys are enumerated without trying every subset of columns; files are analysed in parallel, largest first, within a memory budget (half of the available memory by default); short text is stored inside each cell, so splitting tables copies no strings. The result does not depend on the number of threads.

Reproduce it with the bundled generator, which writes a deterministic retail export (nine denormalized tables of 99 columns that copy facts such as a city and its region, linked by nine foreign keys):

```bash
cargo run --release -p dbflow-core --example demo_data -- demo                       # the nine files
cargo run --release -p dbflow-core --example demo_data -- demo --single 999999 9      # one wide or long table
cargo run --release -p dbflow-core --example bench -- demo/*.csv                       # time and peak memory
```

Details and limits are in [docs/EVALUATION.md](docs/EVALUATION.md). Dependencies are those that hold on your data: on wide tables some are coincidental (customer, employee and date together may identify an order), and they appear in the report as such.

## Command line

The same engine runs without the window:

```bash
bun run cli analyze data/orders.csv data/customers.csv --out out/ --dialect postgresql --json
```

Options: `--out`, `--dialect` (`postgresql`, `mysql`, `sqlite`, `ansi`), `--ops "FD Miner,3NF"`, `--max-lhs`, `--json`, `--verbose`.
`bun run bench` scores key, foreign-key and relation recovery on ground-truth schemas, and `bun run bench:nl` scores the request box.

## Quality

DBFlow is evaluated against the ISO/IEC 25010 product quality model of the SQuaRE (ISO/IEC 25000) series; [docs/QUALITY.md](docs/QUALITY.md) records the requirement, the evidence and the limits for every characteristic. This is a self-assessment, not a certification.

| Command | What it checks |
|---|---|
| `bun run quality` | typecheck, lint, all frontend and Rust tests, production-dependency audit |
| `bun run test` / `bun run test:rust` | the frontend and the Rust test suites |
| `bun run audit:a11y` | WCAG 2.1 AA with axe-core on every page, light and dark |
| `bun run audit:design` | design-rule consistency (type, buttons, cards, radii) |

## Architecture at a glance

```
┌──────────────────────────── WebView (UI only) ─────────────────────────────┐
│  React + TypeScript + Tailwind CSS + React Flow                            │
│  ├─ Workspaces: pipeline canvas, request box, operation palette            │
│  └─ Output pages (diagrams, SQL, reports); on-device models in workers     │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │ Tauri commands (desktop) / WebAssembly (browser, CLI)
┌──────────────────────────────────▼─────────────────────────────────────────┐
│  Rust engine (crates/dbflow-core): tabular input · cleaning ·              │
│  FD mining · keys · 1NF/2NF/3NF/BCNF · chase and preservation checks ·     │
│  cross-file foreign keys · schema · ERD · SQL · reports                    │
│  parallel per file (native) · memory budget · compact results              │
└────────────────────────────────────────────────────────────────────────────┘
```

All computation is in Rust; TypeScript only drives the interface. Read the full story in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Project structure

```
dbflow/
├── src/                  # React frontend (UI only)
│   ├── components/       # Canvas nodes, dialogs, panels, output views
│   ├── hooks/            # Workspace tabs, workflow graph, runs, naming switch
│   ├── engine/           # Bridge to the Rust engine (WebAssembly loader, JSON calls)
│   ├── services/         # Thin APIs over the engine, request-box resolver, naming model
│   ├── workers/          # Web Workers: engine, embedding model, naming model
│   ├── config/           # Operation registry
│   └── utils/            # Pure helpers (validation, edge rules, result cache)
├── crates/
│   ├── dbflow-core/      # The Rust engine; examples/ holds the bench and demo-data generator
│   └── dbflow-wasm/      # WebAssembly bindings of the engine
├── src-tauri/            # Desktop shell: window, CSP, native engine commands
├── scripts/              # CLI, benchmarks, accessibility and design audits
├── docs/                 # Architecture, design, evaluation, quality, releasing
└── .github/              # CI, release automation, issue/PR templates
```

| Document | Contents |
|---|---|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | Structure, data flow, algorithms, parallelism and memory |
| [EVALUATION.md](docs/EVALUATION.md) | Key, foreign-key and relation recovery; request-box accuracy; scalability |
| [QUALITY.md](docs/QUALITY.md) | ISO/IEC 25010 self-assessment and how to re-run it |
| [DESIGN.md](docs/DESIGN.md) | Interface design rules |
| [RELEASING.md](docs/RELEASING.md) | How a release is cut |
| [CHANGELOG.md](CHANGELOG.md) | What changed in each version |

## Contributing

Contributions are welcome: code, docs, bug reports, feature ideas. See [`CONTRIBUTING.md`](CONTRIBUTING.md) to get started. Good first issues are labeled [`good first issue`](https://github.com/miskatul-anwar/dbflow/labels/good%20first%20issue).

Please read our [`Code of Conduct`](CODE_OF_CONDUCT.md): be kind, be constructive.

## Security

Found a vulnerability? Please follow our [security policy](SECURITY.md) and report privately; do not open a public issue.

## License

[MIT](LICENSE) © Miskatul Anwar

---

<div align="center">

If DBFlow saves you time, please consider starring the repo to help others discover the project.

[Report Bug](https://github.com/miskatul-anwar/dbflow/issues/new?template=bug_report.yml) · [Request Feature](https://github.com/miskatul-anwar/dbflow/issues/new?template=feature_request.yml)

</div>
