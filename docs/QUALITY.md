# Quality model (ISO/IEC 25000 SQuaRE)

DBFlow follows the **ISO/IEC 25000 SQuaRE** series (Systems and software Quality Requirements and Evaluation).
Its product quality is specified and evaluated with the **ISO/IEC 25010** quality model, the data it produces with
**ISO/IEC 25012**, and its measures are reported in the style of **ISO/IEC 25020 / 25023**. This page says, for each
characteristic, what the requirement is, how it is checked, and what is **not** covered.

**What this does and does not claim.** It is a self-assessment by the project, carried out on the model and
measures above. It is **not** a third-party evaluation and **not** a certificate: SQuaRE gives a model and
a process, not a pass mark, and no accredited body has evaluated DBFlow. Say "evaluated against ISO/IEC 25010
(SQuaRE)", not "ISO certified" or "compliant with ISO/IEC 25000".

`bun run quality` runs the automated gates (typecheck, lint, tests, production-dependency audit).
`bun run audit:a11y` runs the accessibility audit, `bun run audit:ui` the full layout and accessibility pass over 144 views, and `bun run audit:design` the design-consistency check (type, buttons, cards, radii) (all three need `bun run build` and `vite preview`).

## How the SQuaRE divisions are covered

| Division | Standard | Where it is applied in this project |
|---|---|---|
| Quality management (2500n) | ISO/IEC 25000, 25001 | This page is the quality plan: one owner (the maintainer), one page per release, reviewed with the release checklist below. |
| Quality model (2501n) | ISO/IEC 25010:2023 (product), 25012 (data) | The two tables below: every product characteristic of the 2023 model, and the data-quality characteristics of the outputs. |
| Quality measurement (2502n) | ISO/IEC 25020, 25023 | *Measurement*: each measure has a value and the command that reproduces it. |
| Quality requirements (2503n) | ISO/IEC 25030 | The *Requirement* column states, per characteristic, what DBFlow must do; *Not covered* states the limits. |
| Quality evaluation (2504n) | ISO/IEC 25040 | *Reviewing a release*: establish the requirements (this page), specify the measures, run them, and record the result under *Checked on this revision*. |

| ISO/IEC 25010 characteristic | Requirement | Evidence | Not covered |
|---|---|---|---|
| **Functional suitability** (completeness, correctness, appropriateness) | Every operation produces what its name says; normalization output is sound. | 682 frontend tests and 29 Rust tests, including a differential FD-miner test, golden outputs for the ERD/DDL, adversarial normalization suites, and an instance-level lossless-join check on every decomposition. Candidate keys are enumerated completely (Lucchesi–Osborn), with tests against textbook examples. Real-data check: the Chinook export loads into SQLite with foreign keys enforced and no violations. On the nine-table demonstration set every decomposition is lossless by the chase and all 19,806 mined dependencies are preserved; tables from 9 to 999 columns and from 9,999 to 999,999 rows give the same schema. | Output on data that violates the tool's assumptions (for example, tables under 10 rows only use key determinants, see CHANGELOG). Dependencies are those that hold on the data: on wide or small tables some are coincidental (e.g. customer, employee and date together identifying an order) and become extra relations; the report lists them, but the tool does not judge plausibility. Relation names come from a small on-device model and are right about two times in three (15 of 22 tables in the project's benchmark, `docs/notes/model-naming-spike.md`); they are shown as *suggested* and can be renamed, or built from the key columns when AI naming is off. |
| **Performance efficiency** (time behaviour, resource use, capacity) | Interactive analysis of files with tens of thousands of rows; batch analysis of hundreds of thousands of rows and hundreds of columns on a laptop without running out of memory. | FD mining samples above 10,000 rows and validates on the full data, with a 30 s budget for all sampling rounds, and prunes near-unique determinants with their supersets. Files of a run are analysed in parallel within a memory budget; cells store short text inline and share long text; large inputs get a compact result. Measured (`examples/bench`, 4 cores): the nine-table demonstration set, 431,991 rows × 891 columns (335 MB), in about 55 s with 7.8 GB peak memory (115 s on one core); one 999,999-row table in 16 s with 1.4 GB; one 999-column table in 23 s with 1.8 GB. 11 Chinook tables (15,607 rows) run in about 1 s. Turning AI naming off removes the model's cost from a run. Start-up bundle is about 0.8 MB; the Output page, Mermaid, Graphviz and PDF code load on first use. | No automated performance budget in CI; timings are manual and from one machine. Peak memory is about 20 times the CSV size of each file being analysed. The naming model adds a first-load cost: in headless Chromium a run with a few relations took about 30 to 100 s including the model's first load. The installer carries the model (about 373 MB). |
| **Compatibility** (interoperability, co-existence) | Open formats in and out; no conflict with other software. | CSV, XLSX/XLS/ODS in; SQL (PostgreSQL, MySQL, SQLite, ANSI), CSV/JSON data, Mermaid, Graphviz DOT, SVG, Markdown and PDF out. Runs offline; no background services. | Only SQLite loading is verified by test; the other dialects are generated, not executed. |
| **Interaction capability** (usability: learnability, operability, user-error protection, accessibility, inclusivity) | WCAG 2.1 AA on every screen, both themes; keyboard operable; no motion that cannot be turned off. | `bun run audit:a11y` (axe-core) reports **0 violations** on the pipeline page and all ten Output tabs in light and dark. Workspaces are a labelled `nav` of real buttons; result panels are focusable regions; the main action meets 5.5:1 contrast; `prefers-reduced-motion` disables every animation and transition; destructive actions ask first in an in-app dialog (focus on Cancel, page behind inert, no browser `confirm`/`alert`); closing the last workspace asks for the name of its replacement; undo/redo and "Undo last build". Light and dark themes with a switch (follows the system until chosen). | No screen-reader pass by a person; axe finds roughly a third of WCAG issues. Diagrams are drawn for a light page and stay on one in dark mode, and have no text alternative beyond their source (DOT/Mermaid copy and download). |
| **Reliability** (maturity, fault tolerance, recoverability) | A bad file or a stuck worker never takes the app down; a run gives the same result every time. | Error boundaries per workspace; pipeline cancellation (tested for parallel runs) and time budgets; a workspace closed mid-run stops its run; Output pressed during a run waits for it instead of starting another; the embedding worker is restarted after a timeout; results are cached per workspace; the natural-language bar never throws (hostile-input tests). Parallel and one-thread runs give identical results (test), and the demonstration data is generated deterministically. | No soak or crash-recovery testing of the desktop shell. |
| **Security** (confidentiality, integrity, non-repudiation, accountability, authenticity) | Data never leaves the machine; untrusted file content cannot run code. | Local-first: strict CSP (`default-src 'self'`, no remote connect), bundled models and runtime, no telemetry. In a complete test run (single and multi-file, with the naming model) with every non-local request blocked, the application made none. CSV headers and values are escaped in SQL, Markdown, Mermaid and Graphviz output (tests with hostile names). Raw HTML is never rendered from reports. `bun audit --prod`: no advisories in the 395 shipped packages. The desktop engine copies uploads off the main thread and drops the files of a run that is cancelled before it starts. | Dev tooling still has moderate advisories (ESLint family). The desktop installer is not code-signed. No external security review. |
| **Maintainability** (modularity, reusability, analysability, modifiability, testability) | A change in one area does not break another; behaviour is documented. | Computation lives in the Rust engine and the UI only renders (high cohesion, one JSON boundary between them); engine, UI and workers are separate modules with 711 tests; `docs/ARCHITECTURE.md`, `docs/DESIGN.md`, a changelog; shared `Button`, `Modal`, `StatCard` and design tokens; `cargo clippy` with warnings as errors. | 175 lint warnings remain, mostly `any` in older report code; the cap is enforced so it cannot grow. |
| **Flexibility** (adaptability, installability, replaceability, scalability) | Installs from one installer; adapts to the system. | Tauri installers for Windows, macOS and Linux built by CI on a tag; the maintainer installed and ran the desktop application on **Windows, Linux and macOS** and reports that it works as intended; theme follows the OS; layouts checked at 900, 1300 and 1700 px. | The cross-platform install check is manual and has no recorded test matrix (OS versions, hardware). The automated checks above run on one platform. |
| **Safety** (added in the 2023 edition) | Nothing in the app can harm people or property. | The app only reads the files you give it and writes files you choose. | Not applicable beyond that. |

## Data quality (ISO/IEC 25012)

What the tool reports about **your** data, and how its own outputs are made trustworthy:

| Characteristic | How it is handled |
|---|---|
| Accuracy / consistency | Dependencies carry measured confidence; approximate ones are flagged and not used unless allowed. Every decomposition is checked for lossless join and dependency preservation, on the data as well as symbolically. |
| Completeness | Missing values are counted per column; determinants with missing values are not used as keys; foreign keys list the share of values matched and whether they are optional. |
| Credibility | Warnings say when a result rests on little data (tables under 100 rows). A key seen in a sample is confirmed on all rows before it is used. |
| Understandability | Every figure on the Output page is labelled; the PDF report states what was and was not checked. |
| Traceability | The report lists the operations run, the options, and the engine's own notes. |

## Measurement (ISO/IEC 25020, 25023)

| Measure | Value | How to reproduce |
|---|---|---|
| Automated tests | 682 frontend + 29 Rust, all passing | `bun run test`, `cargo test --workspace` |
| Accessibility violations (axe, WCAG 2.1 AA + best practice) | 0 (pages, all Output tabs and every dialog, both themes) | `bun run audit:a11y` |
| Design-rule deviations | 0 | `bun run audit:design` |
| High / critical advisories in shipped dependencies | 0 (395 packages) | `bun audit --prod` |
| Lint errors | 0 (175 warnings, capped) | `bun run lint`, `cargo clippy --workspace --all-targets -- -D warnings` |
| Start-up JavaScript | about 0.8 MB | `bun run build` |
| Throughput, nine tables, 431,991 rows × 891 columns | about 55 s on 4 cores (115 s on 1), 7.8 GB peak | `cargo run --release -p dbflow-core --example demo_data -- demo` then `cargo run --release -p dbflow-core --example bench -- demo/*.csv` |

## Checked on this revision

Re-run for this revision: typecheck, lint (0 errors, 175 warnings), 682 frontend tests, 29 Rust tests, `cargo clippy
--workspace -D warnings` and `cargo fmt`, the production-dependency audit (no advisories), `bun run audit:a11y`
(0 violations, both themes), an axe pass over every dialog (0 violations, both themes), `bun run audit:design` (consistent)
and the throughput measures above. The desktop application was built and driven end to end on Linux (WebKitGTK under
Xvfb, through `tauri-driver`): a new workspace opens in about 0.2 s while a three-file run is in progress, and the
results open without resetting the other workspaces. The full-page layout audit (`bun run audit:ui`) was not re-run,
because it runs the naming model in every scenario and takes very long.

## Reviewing a release

1. `bun run quality`
2. `bun run build`, then `bunx vite preview --port 4311`, `bun run audit:a11y` and `bun run audit:design`
3. Run the throughput measure (see *Measurement*) and compare it with the previous revision.
4. Read the CHANGELOG entry and the limits above; update this page if a characteristic changed, and record the run under *Checked on this revision*.
