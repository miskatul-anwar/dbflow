# DBFlow Architecture

How DBFlow is structured, how data flows through it, and where the
important algorithms live.

## Overview

DBFlow is a [Tauri 2](https://tauri.app) desktop app with a **Rust engine** and a **React UI**.
Every parser and algorithm lives in one Rust crate, `crates/dbflow-core`. The desktop app links
it natively and runs analyses on a background thread; the same crate is compiled to WebAssembly
(`crates/dbflow-wasm`) for browser development, Web Workers, the CLI and the test suite. One
engine therefore serves every runtime, and results are the same everywhere.

```
┌──────────────────────────── WebView (UI only) ─────────────────────────────┐
│  App.tsx (Workspace)                                                       │
│  ├── OperationPalette   add pipeline nodes (click or drag)                 │
│  ├── FlowCanvas         React Flow canvas rendering the pipeline           │
│  └── OutputPage         ERD, SQL, FDs, keys, normal forms, report          │
│                                                                            │
│  Hooks:     useWorkflowGraph (nodes, edges, undo/redo)                     │
│             useNormalization (run lifecycle) → engineClient                │
│  src/engine: WebAssembly loader + JSON bridge (`engine('nf.findKeys', …)`) │
│  src/services: thin TypeScript APIs over the engine; NL workflow builder;  │
│                on-device naming model (Web Worker)                         │
└───────────────┬───────────────────────────────────────┬────────────────────┘
                │ Tauri commands (desktop)               │ WebAssembly (browser,
                │ engine_upload / prepare / finish /     │ worker, tests, CLI)
                │ cancel / call                          │
┌───────────────▼───────────────────────────────────────▼────────────────────┐
│ Rust engine — crates/dbflow-core                                           │
│   csv · encoding · spreadsheet (calamine)   ingest                         │
│   data_utils · data_processor               cleaning, atomicity, profiling │
│   relations                                 column classifier, cross-file FKs│
│   fd_miner                                  partition-refinement FD mining │
│   keys                                      primary keys, 1NF child tables,│
│                                             re-join check on the data      │
│   normalization                             closures, keys, cover, 2NF/3NF/│
│                                             BCNF, chase, preservation      │
│   naming                                    relation ids and names         │
│   schema · ddl · legacy · reports · erd_dot outputs                        │
│   analysis · pipeline                       per-table analysis, whole runs │
│   api                                       JSON dispatcher for every call │
│   js                                        JavaScript-compatible number,  │
│                                             date and collation semantics   │
└────────────────────────────────────────────────────────────────────────────┘
```

### Runs in two phases

The on-device naming model runs in the webview, between the engine's two phases:

1. `prepare` reads every file (spreadsheets become one table per sheet), runs the selected
   operations for each table and returns the naming questions (column lists only).
2. The UI asks the naming model, then `finish` applies user and model names, detects
   cross-file foreign keys, assembles the schema and builds every output.

In the desktop app the files are sent to Rust once as raw bytes (`engine_upload`), progress
streams back over a Tauri channel and `engine_cancel` sets a flag the engine checks between
steps and inside FD mining. In the browser the WebAssembly build runs in a Web Worker and
cancelling terminates the worker.

### Compatibility layer

The engine replaced a TypeScript implementation. `js.rs` reproduces the JavaScript behaviour
the outputs depended on (`Number()` coercion, `toFixed`, `toLocaleString`, `localeCompare`,
`Date.parse`), and the TypeScript modules in `src/services` keep their original APIs as thin
wrappers, so the existing test suite (golden files included) runs unchanged against Rust.

Spreadsheets are read by `calamine` in Rust. SheetJS remains only as a fallback for formats
calamine does not decode (`.xlsb` short-cell records, BIFF2-5 `.xls`, SpreadsheetML 2003 `.xml`
and flat `.fods`).

## The processing pipeline

The canvas selects which operations run. `analysis::analyze_rows` (Rust)
executes them for one table; `pipeline::prepare` / `finish` call it for one
file or for every file.

1. **Ingest** — `spreadsheetProcessor` first turns each visible sheet of an Excel or OpenDocument workbook into an in-memory CSV (so a workbook is a multi-file run with one table per sheet); then `CSVProcessor` parses each file (delimiter sniffing, quoted
   fields, duplicate header repair; large files stream).
2. **Clean / profile** — empty and null-like cells are normalized; column
   types and multi-valued columns are reported. A column counts as
   multi-valued only if a large share of its cells are lists of short items
   and it is not free text.
3. **Classify** — each column gets an entity tag (e-mail, price, id, …) from
   column-name and value evidence.
4. **Mine FDs** — lattice search with partition refinement. Missing values are
   one value (`nullSemantics: 'equal'`, default) or all distinct. Exact and
   approximate dependencies are returned; **only exact ones** feed keys and
   normal forms unless `acceptApproximateFDs` is set. A time budget and an
   abort signal stop long searches and flag the result as truncated.
5. **Keys** — candidate keys by attribute closure; `chooseKey` picks one that
   is unique and has no missing cells, preferring identifier-like single
   columns, and adds a `row_id` surrogate when none exists.
6. **Normalize** — 1NF (non-destructive), 2NF, Bernstein 3NF synthesis, BCNF
   decomposition.
7. **Verify** — chase (lossless with respect to the FDs), re-join of the
   projections on the data (lossless on this data), and dependency
   preservation by projected closure. The report shows both lossless verdicts.
8. **Relate (multi-file)** — foreign keys are inclusion dependencies scored by
   containment, coverage, name similarity and type; accidental containment
   between dense id sequences is rejected; nullable foreign keys are marked
   optional.
9. **Assemble** — `buildSchema` gives every normalized relation a unique name,
   keeps the key chosen for each file on the relation that holds it, and
   attaches each cross-file foreign key to the relations that actually contain
   the columns.
10. **Emit** — Mermaid ER diagram, DDL (PostgreSQL, MySQL, SQLite or ANSI) with
    inferred sized types, `NOT NULL` and `UNIQUE`, tables ordered parents
    first, and a Markdown report. Text from the input files is escaped.

## Why the FD miner is fast enough

Columns are integer-encoded once. Partitions are refined along the LHS
lattice, reusing the parent's partition, and only the previous level is kept
in memory. `X → Y` holds exactly when refining π(X) with Y creates no new
classes; violation counts fall out of the same step, giving approximate FDs.
Supersets of keys and non-minimal determinants are pruned. Limits: LHS size is
capped (default 4) and the search stops at the time budget; both are reported.

## Natural-language workflow builder

```
NLCommandBar → useNLWorkflow → resolver (keywords, then embeddings) → planner → useWorkflowGraph
                                   └ EmbeddingProvider: bundled MiniLM in a Web Worker (ONNX/WASM) | none
```

- `src/services/nlp/workflow/` is pure TypeScript with no React: `planner.ts` (dependency closure of the requested operations, sorted by `order`), `anchors.ts` (aliases and example phrases per operation), `lexicalResolver.ts` (clauses, negation, options), `resolver.ts` (hybrid), `anchorIndex.ts` (embeds anchors once, caches in memory and IndexedDB keyed by model, framing and anchor hash), `ollamaProvider.ts`.
- `src/config/operations.ts` stays the only definition of operations and dependencies; a test fails if an operation has no anchors.
- The plan is previewed as chips (dimmed = added as a dependency). Apply replaces or extends the canvas; the toast Undo restores the previous snapshot in one step.
- The model (`public/models`, 23 MB, sha256-checked) and the ONNX WebAssembly runtime (`public/ort`, copied from `onnxruntime-web` by `scripts/prepare-nl-assets.mjs` before `dev`/`build`) ship inside the installer. `embed.worker.ts` loads them with remote loading disabled. The CSP gains only `'wasm-unsafe-eval'` (WebAssembly compilation, not JavaScript `eval`).
- Pitfall worth knowing: transformers.js only looks for local tokenizer files when `localModelPath` is a path rather than an http(s) URL, so the worker passes `/models/`.

## Workspace tabs

`AppShell` (`src/App.tsx`) hosts one `Workspace` per tab, each inside its own `ReactFlowProvider`. Inactive tabs, and all tabs while the Output page is open, stay mounted but hidden, so graphs, files, results and running jobs survive. State is per tab: the graph hook, uploaded files, result cache (`resultCache.ts`, keyed by tab id) and the engine worker for each run. The embedding worker is shared by all tabs (one copy of the model in memory). Anything DOM-global must be namespaced per tab: React Flow gets `id={tabId}` (marker ids), the edge gradient id and the NL input id include the tab id, and keyboard shortcuts only act in the active tab. Tab state is in `useWorkspaceTabs`; it is not persisted across launches.

## Workflow graphs

Canvas state (nodes = operations, edges = chain or manual connections) lives
in `useWorkflowGraph`.

- State updaters are pure; logging and id bookkeeping happen outside them
  (React StrictMode double-invokes render-phase functions).
- Snapshots are versioned JSON (`format: "dbflow-workflow"`, `version: 2`);
  the legacy format still loads.
- Operations are registered in `src/config/operations.ts`, the single source
  of truth for phases, dependencies and ordering.

## Security notes

- Untrusted text (headers, cell values) reaches Markdown and Mermaid. Raw HTML
  in Markdown is not rendered, table/span text is escaped, Mermaid runs with
  `securityLevel: 'strict'`, and the webview has a strict
  Content-Security-Policy (`src-tauri/tauri.conf.json`).
- No network requests are made. The natural-language box uses a model bundled with the app; there is no server and no download.

## Testing

| Layer | Tool | Location |
|---|---|---|
| Algorithms (FD mining, keys, normal forms, chase, FK detection, SQL types) | Vitest | `src/**/*.test.ts`, `src/tests/` |
| End-to-end pipelines (single and multi-file) | Vitest | `src/tests/pipelineSoundness.test.ts`, `e2ePlatformIntegration.test.ts` |
| Types | `tsc` strict | CI |
| Rust engine and shell | `cargo test --workspace`, `clippy`, `fmt` | `crates/*/src`, `src-tauri/src` |

CI (`.github/workflows/ci.yml`) builds the WebAssembly engine, then runs typecheck → lint →
tests → build for the frontend, and fmt → clippy → test for the Rust workspace on Linux,
Windows and macOS. The Vitest suite exercises the Rust engine through its WebAssembly build.

## Release engineering

Tag-driven: pushing `vX.Y.Z` triggers `.github/workflows/release.yml`, which
builds installers (MSI, DMG x2 arches, deb/rpm/AppImage) via `tauri-action`
and publishes a GitHub Release. See `docs/RELEASING.md`.
