<div align="center">

<img src="public/icon.png" alt="DBFlow logo" width="120" />

# DBFlow

**Turn raw CSV files into normalized database schemas — ERDs, SQL DDL and full normalization reports — in seconds.**

[![License: MIT](https://img.shields.io/badge/License-MIT-115e59.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/miskatul-anwar/dbflow?logo=github&color=0284c7)](https://github.com/miskatul-anwar/dbflow/releases)
[![CI](https://github.com/miskatul-anwar/dbflow/actions/workflows/ci.yml/badge.svg)](https://github.com/miskatul-anwar/dbflow/actions/workflows/ci.yml)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri%202-FFC131?logo=tauri&logoColor=black)](https://tauri.app)
[![React](https://img.shields.io/badge/Frontend-React%2018-61DAFB?logo=react&logoColor=black)](https://react.dev)

*One desktop app for CSV → 1NF → 2NF → 3NF → ERD → SQL. Private by design — your data never leaves your machine.*

</div>

---

## What is DBFlow?

DBFlow is a **free & open-source cross-platform desktop app** that reverse-engineers relational databases from CSV exports.

Drop in one or many CSV or spreadsheet files (Excel `.xlsx` `.xlsm` `.xlsb` `.xls`, LibreOffice `.ods`; every sheet becomes a table) and DBFlow will:

- 🧹 **Clean & validate** your data (atomicity checks, null handling, type profiling)
- ⛏️ **Mine functional dependencies** with a fast partition-refinement algorithm (TANE-inspired lattice pruning)
- 🔑 **Discover candidate keys** using attribute-closure search over the FD lattice
- 🧩 **Decompose to 3NF / BCNF** with lossless-join verification (chase, plus a re-join on your data) and dependency-preservation analysis
- 🕸️ **Detect cross-table foreign keys** from the data (inclusion dependencies scored by containment, coverage, name and type evidence)
- 🧠 **Classify columns semantically** (NLP profiler: names, e-mails, prices, dates, locations, organizations…)
- 📊 **Generate Mermaid ERDs**, consolidated SQL DDL, and a Markdown normalization report

## Highlights

| | |
|---|---|
| 🔒 **Private by design** | All processing happens locally; no network calls are made. The natural-language box runs a small model that ships inside the installer. |
| 🦀 **Rust engine** | Parsing, dependency mining, normalization and every output come from one Rust engine: native in the desktop app, WebAssembly in the browser, the same code for one file or many. |
| 🧮 **Sound algorithms** | Functional-dependency mining, closure-based key discovery, 3NF synthesis. Only dependencies that hold on every row shape the schema. |
| 🖱️ **Visual pipelines** | Build workflows on an interactive React Flow canvas — click or drag the operations you want. |
| 📦 **Portable outputs** | Mermaid, `.sql`, `.md` and workflow `.json` files you can commit to your repo. |
| 🪶 **Lightweight** | Native binary, tiny footprint compared to Electron alternatives. |

## Installation

### Download (recommended)

Use the [**download page**](https://miskatul-anwar.github.io/dbflow/), which picks the installer for your system, or grab one from the [**Releases page**](https://github.com/miskatul-anwar/dbflow/releases):

| Platform | Artifact |
|---|---|
| Windows | `.msi` / `.exe` |
| macOS | `.dmg` / `.app` |
| Linux | `.deb` / `.rpm` / `.AppImage` |

> [!NOTE]
> macOS builds are unsigned. On first launch right-click → **Open**, or allow the app under *System Settings → Privacy & Security*. The Windows installer is not code-signed either, so SmartScreen may ask you to choose *More info → Run anyway*.

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
<summary><b>Linux one-liner (Debian/Ubuntu)</b></summary>

```bash
sudo apt update && sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```
</details>

## Quick start

1. Launch DBFlow and upload one or more CSV or spreadsheet files with the **Upload** button.
2. Build your pipeline by clicking operations in the sidebar (a locked step adds the steps it needs), by describing it in the text box, or by typing a request such as "normalize to 3NF and generate SQL".
3. Press **Run**.
4. Open the **Output** tab to browse the ERD, SQL DDL, FDs, keys, cardinalities and the full report — then download what you need.

## Workspace tabs

Work on several datasets at once: each tab is a separate workspace with its own pipeline, files and results, and long runs keep going in the background while you use another tab. `Ctrl/Cmd+T` opens a tab, `Ctrl/Cmd+W` closes it, `Ctrl+Tab` switches, rename a tab with its pencil button, double-click, `F2` or right-click. Tabs live as long as the app is open (they are not saved between launches; use the Save button to keep a pipeline).

## Natural-language box

Type a request such as *“Can you pls convert it to 1NF”* in the bar above the canvas (`Ctrl/Cmd+K` focuses it). DBFlow shows the pipeline it would build, including the steps added because others need them, and you press **Apply** or **Apply & Run**.

Nothing extra to install: exact keywords are matched first, and a small sentence-embedding model (all-MiniLM-L6-v2, int8, 23 MB, Apache-2.0) bundled in the installer handles paraphrases the keywords miss. It runs inside the app on WebAssembly; your sentence never leaves the machine, and neither does your data. Accuracy: [docs/EVALUATION.md](docs/EVALUATION.md).

## Command line

The same engine runs without the window:

```bash
bun run cli analyze data/orders.csv data/customers.csv --out out/ --dialect postgresql --json
```

Options: `--out`, `--dialect` (`postgresql`, `mysql`, `sqlite`, `ansi`), `--ops "FD Miner,3NF"`, `--max-lhs`, `--json`, `--verbose`.
`bun run bench` scores key, foreign-key and relation recovery on ground-truth schemas (see [docs/EVALUATION.md](docs/EVALUATION.md)).

## Architecture at a glance

```
┌──────────────────────────── WebView (UI only) ─────────────────────────────┐
│  React 19 + TypeScript + Tailwind + React Flow                             │
│  ├─ Workflow canvas (nodes = pipeline operations)                          │
│  └─ Output pages (diagrams, SQL, reports), on-device naming model          │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │ Tauri commands (desktop) / WebAssembly (browser)
┌──────────────────────────────────▼─────────────────────────────────────────┐
│  Rust engine (crates/dbflow-core): CSV & spreadsheet parsing · cleaning ·  │
│  FD mining · keys · 1NF/2NF/3NF/BCNF · chase and preservation checks ·     │
│  foreign keys · schema · ERD · SQL · reports                               │
└────────────────────────────────────────────────────────────────────────────┘
```

Read the full story in [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Project structure

```
dbflow/
├── src/                  # React frontend
│   ├── components/       # UI components (canvas nodes, panels, output views)
│   ├── hooks/            # Reusable state logic (workflow graph, execution)
│   ├── engine/           # Bridge to the Rust engine (WebAssembly loader, JSON calls)
│   ├── services/         # Thin TypeScript APIs over the engine, NL workflow builder
│   ├── config/           # Operation registry
│   └── utils/            # Pure helpers (validation, edge rules, result cache)
├── crates/
│   ├── dbflow-core/      # The Rust engine: every parser and algorithm
│   └── dbflow-wasm/      # WebAssembly bindings of the engine
├── src-tauri/            # Desktop shell: window, CSP, native engine commands
├── docs/                 # Architecture & release docs
└── .github/              # CI, release automation, issue/PR templates
```

## Contributing

Contributions are welcome — code, docs, bug reports, feature ideas! See [`CONTRIBUTING.md`](CONTRIBUTING.md) to get started. Good first issues are labeled [`good first issue`](https://github.com/miskatul-anwar/dbflow/labels/good%20first%20issue).

Please read our [`Code of Conduct`](CODE_OF_CONDUCT.md) — be kind, be constructive.

## Security

Found a vulnerability? Please follow our [security policy](SECURITY.md) and report privately — do not open a public issue.

## License

[MIT](LICENSE) © Miskatul Anwar

---

<div align="center">

If DBFlow saves you time, please consider giving the repo a ⭐ — it helps others discover the project!

[Report Bug](https://github.com/miskatul-anwar/dbflow/issues/new?template=bug_report.yml) · [Request Feature](https://github.com/miskatul-anwar/dbflow/issues/new?template=feature_request.yml)

</div>

