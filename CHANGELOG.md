# Changelog

All notable changes to DBFlow are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [7.0.0] - 2026-11-26

### Changed

- Toasts and system notifications now have separate jobs. Toasts (shadcn's Sonner, on desktop and Android) answer
  what you just did: a pipeline built from a request (with **Undo**), a file that could not be read, a check that
  stopped a run. The same message replaces itself instead of stacking, and at most three show at once. System
  notifications are only for a run that finishes or fails while you are away from DBFlow (another app, minimised,
  phone in the background). A run that finishes while you are looking is a toast; one in another workspace names that
  workspace and offers **View** (Android) or **View results** (desktop). Errors are no longer shown twice, and a run
  you cancel raises nothing.
- Android: the run progress is a floating card. It shows the phase with a spinner, the percent and elapsed time, a bar
  in the phase's colour, what the engine is doing to which table, and a four-step phase tracker, with a compact
  Cancel button.
- Android: the request field uses a quiet focus style (darker border and a soft halo).

### Fixed

- Android: notifications show the DBFlow logo. The status-bar icon is a white silhouette of the logo and the
  notification carries the full-colour logo; before, Android fell back to a generic "i" icon.
- Android: focusing the request field no longer draws a black rectangle inside it.
- Android: the "nothing to run" message no longer mentions goals, which were removed in 6.7.3.

## [6.7.3] - 2026-11-26

### Changed

- Android: the Pipeline screen opens with the same request bar as the desktop app (type a sentence, Build, preview the
  plan as chips, then Replace / Extend / Apply & Run / Cancel, and Undo last build), as a compact field at the top of
  the screen with a labelled Build button and corners that match the Run button. The Goal list is gone; the step checklist below
  stays.

### Fixed

- Android: shared buttons inside the phone layout kept their own fill and border (the phone button reset now sits in
  the base CSS layer instead of overriding every utility class).

## [6.7.2] - 2026-11-25

### Changed
- **Menu sidebar (desktop).** Files and Operations sit together under one sidebar titled *Menu*, which holds the collapse (and reset-width) button. Collapsed, the pill reads *Menu*.
- **The header has no upload button**: files are added in the Files card (button or drag and drop).
- **Coloured workspace tabs** show their colour as a tint only; the bar along the tab's top edge is gone.

## [6.7.1] - 2026-11-25

### Added
- **Files sidebar (desktop).** The left column has a *Files* card above *Operations*, in the same style, listing the workspace's files.
  - Add one file at a time or many at once, with the + button, the header's upload button or by dropping files on the card. Files are added to the list (before, each upload replaced it); a file already in the list is skipped.
  - Each file has a menu: *Rename* (also double-click or F2; the extension is kept and names must be unique), *Replace…*, *Save a copy*, *Move up* / *Move down* (the merge of overlapping files keeps the first file's version of a conflicting record), and *Remove*. The trash icon removes every file.
  - The list is read-only during a run.

### Changed
- **Far fewer notifications.** Only a run's result and errors that stopped an action reach the system notification centre. Warnings (for example one per step when a pipeline is built from a request) and routine confirmations (files added, workflow saved) stay in the log. The notifier also drops a repeat of the same message within 5 s and any further message of the same level within 1.5 s.
- **Android tablets get a tablet layout** (Material 3 window size classes). From 600dp wide: a navigation rail on the left instead of the bottom bar, wider content, the Run button and dialogs floating as cards. From 840dp (a tablet in landscape): the Pipeline screen in two columns, the plan beside its steps.

### Fixed
- On tablets the bottom bar's active pill floated above the label instead of behind the icon.
- **Desktop: the window's title bar now follows the app's theme.** It was fixed to light (`theme: "Light"` in the window config). It now follows the system, and an explicit light/dark choice in the app sets it too (Tauri `Window.setTheme`, with the `core:window:allow-set-theme` permission).
- **Tooltips match the app.** Every `title` hint (graph nodes, workspace tabs, diagram zoom, the request bar…) now shows as the app's own tooltip, ink on paper in light and paper on ink in dark, instead of the system's white box.
- **Dark mode: the workspace tab's colour dot and the colour picker's swatch outlines** were drawn in a fixed dark navy and disappeared; they now use the theme's colours.

## [6.7.0] - 2026-11-24

### Changed
- **Minimalist redesign, desktop and Android.** One quiet ink-on-paper palette (shadcn's zinc neutrals) in both themes, with a near-black main action.
  - No tutorial text, guides or subtitles on screen: operation, result and goal descriptions are tooltips; secondary toolbar actions are icons with tooltips (shadcn/Radix); helper lines and empty-state hints are gone.
  - Subtle motion with Framer Motion (page, tab, dialog, sheet and toast transitions; the results sidebar's highlight glides between rows) and GSAP (summary figures count up). Every animation follows the system's reduce-motion setting.
  - The accessibility audits still pass with 0 violations on desktop and phone, in light and dark.

## [6.6.8] - 2026-11-24

### Changed
- **Every save asks where to put the file**, in the system's file dialog: the ER diagram (SVG, PNG, DOT), the report PDF, SQL, generated data files and saved pipelines. In the desktop app the dialog opens in Downloads with the file's name and type filled in; before, files went to Downloads without asking. Android already asked. In a browser, Chrome and Edge show their save picker; Firefox and Safari keep downloading as the browser is set to.

### Fixed
- In a browser, a save that fails part-way (for example, a full disk) leaves the chosen file as it was instead of half-written.

## [6.6.7] - 2026-11-17

### Fixed
- **Android: the app closed when AI table names ran on a large file.** The naming model (about 390 MB, more in memory) ran inside Android's WebView, which Android kills when it runs out of memory, closing the app. AI table names are now unavailable on phones: Settings explains that names come from key columns (and can be renamed), and the model is left out of the Android package, which shrinks by about 390 MB.
- **Android: squeezed names in *Column meaning*.** A long file name squeezed the table name to one letter per line; the detail now wraps below the name. Long names in the result views wrap instead of running off the screen.
- **Android accessibility** (found by the new `bun run audit:a11y:mobile`): secondary text, key badges and error colours now meet WCAG 2.1 AA contrast in both themes (127 failures before); switches and the result views' buttons are at least 48 dp.

- **Android asks for workspace names as the desktop app does**: on launch (*Name your workspace* → *Start*), and when the last workspace is closed (*Name the new workspace*). Before, it only asked when *New workspace* was tapped.

### Changed
- **Android looks like an Android app** (Material 3): the system font, a one-line app bar, a navigation bar with a pill behind the active screen, sentence-case section headers, Material switches, press feedback as a tint, no scrollbars, and result views without a frame inside a frame.

### Added
- `docs/QUALITY.md` evaluates the Android app against ISO/IEC 25010 (it covered only the desktop app before), with the phone accessibility audit as a measure.

## [6.6.6] - 2026-11-16

### Fixed
- **Android: processing always failed with "unknown error".** Android's WebView cannot pass a binary request body to the app, so the file upload reached the engine as JSON and was rejected. On Android the app now sends each file as base64 text, which the engine decodes (it also accepts a byte array). Errors from the app engine now show their own text instead of "unknown error".
- **Android app icon.** The Android app showed Tauri's default icon; it now uses the DBFlow logo at every screen density. CI now fails if the Android project's launcher icons differ from the ones generated in `src-tauri/icons/android`.
- **Download page.** Simpler: the app's name and one line about it, the download box, install help for the chosen system (now including Android), and the file list with checksums folded away. The feature list and screenshot are gone.
- **Release workflow.** The release's CI check now receives the Android signing secrets, the draft release is addressed by id (a draft has no tag yet), and the tag is created on the commit that was built.

## [6.6.0] - 2026-11-15

### Added
- **Fourth normal form (4NF).** A new *4NF* operation, which runs after BCNF, removes redundancy that BCNF cannot see: independent lists stored in one table, such as a course's teachers and its books.
  - **Mining:** multivalued dependencies (`X ->> Y`) are found on the data. The test is exact: every `X` group must be the complete product of its `Y` values and its other values. A dependency is reported only with evidence, meaning at least one group where both sides vary (at least four rows). Functional dependencies are left to BCNF.
  - **Decomposition:** Fagin's. A table `R` is split into `X ∪ Y` and `X ∪ (R − X − Y)` until no non-key left side remains.
  - **Lossless check:** the chase uses multivalued dependencies too, so the splits are proven lossless, and re-joining the rows confirms it on the data.
  - **Where it shows:**
    - result fields `relations4NF` and `multivaluedDependencies`, or `fourthNormalForm` per table in a multi-file run
    - a 4NF tab on the *Normal Forms* view, listing each dependency with its evidence
    - a 4NF section in the report
    - the files `relations_4nf.json`, `multivalued_dependencies.json` and one CSV per relation
  - **Request box:** "4NF", "fourth normal form" and "multivalued dependencies" now plan it.
  - **Android app:** a *4NF schema* goal.
  - **Limits:** relations of up to 16 columns, up to two columns on each side, 15 seconds per table.
- **A touch interface for the Android app (phones and tablets), with every feature of the desktop app.** The desktop interface (a canvas, side panels and many small controls) is hard to use on a touch screen, so the Android app has its own interface; the desktop app is unchanged. It is function-first: flat lists, one accent colour, 48 px touch targets, and colour only where it means something (the four phases, primary and foreign keys, errors). It runs on the desktop app's own state hooks, so features behave the same.
  - **Workspaces:** switch, create, rename, colour and close (with a confirmation when there is work in it) from the app bar. Runs continue in the background while another workspace is open. Each workspace keeps its results, and its title follows the first file added.
  - **Data:** the files to analyse, each with its type and size.
  - **Pipeline:** Undo, Redo, Open and Save of workflow files (the same `.json` as the desktop app). A goal list builds a whole pipeline in one tap (3NF schema, BCNF schema, keys and foreign keys, clean and profile, all steps). The request box shows the desktop's plan preview (requested and added steps, dialect), then Replace, Extend, Apply & run, Dismiss, "did you mean" choices and Undo of the last build. The 19 steps are a filterable checklist by phase: checking a step adds its prerequisites, and unchecking it removes the steps that need it.
  - **Running:** a panel above the bottom bar with progress, the engine's current step, the four phases and Cancel. It does not block the screen, so you can switch workspaces during a run. Run completion and failure go to the system notifications.
  - **Results:** four numbers (tables, links, dependencies, rows), then tabs:
    - Overview: input checks with the merge, the normalization journey or column meaning, and the new schema with PK and FK tags
    - Tables, with renaming
    - Diagram, in four notations, with downloads
    - SQL: copy and save
    - Keys, Links, Dependencies and Checks
    - Report: Markdown and PDF
    - Files
  - **Settings:** AI table names (off by default on a phone, where the model is slow), dark theme, and Reset everything.
  - **Tablets:** content keeps a readable width, and sheets open as centred dialogs.
  - **Navigation:** Android's back button moves back through the screens. In a browser, `?ui=mobile` shows this interface.
- **Android app.** Each release now includes `DBFlow_<version>_android_arm64.apk` (Android 7 or newer, 64-bit ARM). It runs the same Rust engine as the desktop app, entirely on the phone. The download page has an Android tab and picks it on Android phones. The release workflow builds and signs the APK and attaches it to the release; signing uses the repository secrets `ANDROID_KEYSTORE_BASE64`, `ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` and `ANDROID_KEY_PASSWORD`.

### Changed — build and release
- **Every release contains the desktop and Android apps, published together.**
  - **Draft first:** the release workflow creates a draft and checks that the tag matches the version in every file and that the changelog has a section for it. That section becomes the release notes.
  - **Tested before building:** it runs the full CI, then builds the desktop installers and the signed APK in parallel. Before, Android waited for the slowest desktop build.
  - **Complete or not published:** it publishes only when all eight installers are attached, and adds `SHA256SUMS.txt`. Before, the release went public when the first installer finished.
  - **Android checks:** the signing secrets are required, and the APK's signature and version are checked.
  - **Re-runs:** they reuse the draft, and an already published release is never overwritten.
  - **Fixes:** a manual run no longer falls back to the tag `v2.4.0`, and the tag input is no longer pasted into shell code.
- **CI:**
  - The checks shared by every job come from one setup action (`.github/actions/setup`).
  - The version check runs on every pull request.
  - The format check runs once instead of on three operating systems.
  - Runs on `main` are no longer cancelled by later pushes.
  - Every job has a timeout, and the Android build caches Gradle and checks the APK's version.
- **Dependabot:** it now covers the Cargo workspace at the repository root (it pointed at `src-tauri` and missed the engine crates and the lockfile), the Android Gradle project, and the shared setup action.
- **Docs:** `docs/RELEASING.md` is rewritten for the new process.

### Fixed
- Downloads (SQL, CSV, diagrams, reports, PDF and saved workflows) all go through one helper. On Android, where the WebView ignores download links, it opens the system *Save as* screen and writes the file there.
- On a phone-width screen the toolbar buttons overlapped and *Output* was cut off; the toolbar now wraps onto two rows. On the Output page *Back to workflow* ran into the title; below 640 px it shows the arrow only.

## [6.5.2] - 2026-11-15

### Added
- **Input checks.** Every file is checked before analysis, whatever its extension, and the Output page lists what the checks found under *Input checks* (engine result field `inputChecks`).
  - Files that are not tables are left out with the reason: PDF documents, images, audio and video, archives, programs, SQLite databases, Word and PowerPoint documents, OpenDocument text, JSON (a document or one object per line), XML or HTML markup, binary data and program source code. Empty files are reported as empty.
  - An Excel or OpenDocument workbook saved with a `.csv` name is read as a workbook.
  - A file with exactly the same contents as an earlier file (ignoring line endings and a byte-order mark) is analysed once. Before, the copy became a second table with invented foreign keys to the first.
  - Files with the same columns and mostly the same rows are reported, with shared rows, matching records and conflicting records. **Merge overlapping files and run again** combines them into one table (engine option `mergeOverlappingFiles`): repeated rows are kept once and, for conflicting records, the first file's version is kept.
  - Files that share a name are called "data.csv (file 1)" and "data.csv (file 2)" in messages.

### Fixed
- **Compression bombs.** A 1.4 MB `.xlsx` that expands to 1.5 GB made the engine use 11.4 GB of memory. Workbooks are now measured by decompressing them before they are read: an entry that expands more than 200 times beyond 16 MB, or a workbook that needs more memory than is available (or more than 2 GB unzipped; 512 MB in the browser), is refused in a fraction of a second. The interface's fallback reader runs the same check first.
- One unreadable file no longer stops a multi-file run. A bad workbook stopped the whole run, while a bad CSV was skipped; now any bad file is left out with its reason and the run continues. A run fails only when no file can be analysed, and the error gives each file's reason.
- A PDF or another non-spreadsheet saved with a spreadsheet extension was read as a one-column table. A generic XML file read as a spreadsheet failed with "Cannot read properties of undefined"; it now says the file is not a valid spreadsheet.
- Notes from converting workbooks (skipped hidden or empty sheets) were missing from the log of multi-file runs.

## [6.5.1] - 2026-11-15

### Changed
- The **CSV Data Loader** operation is now called **Data Loader** ("Loads CSV, Excel and ODS files"), since it reads spreadsheets as well. Saved pipelines, CLI scripts and engine calls that use the old name keep working, and requests that mention Excel or spreadsheets now resolve to it.

## [6.5.0] - 2026-11-15

### Added
- **AI naming switch.** The toolbar's *AI names / Key names* button (shared by every workspace, remembered on this machine) turns the on-device naming model off. New relations are then named from their key columns by rule (`region_code` → "Region", `order_id, line_no` → "Order Line"), marked *from key* on the Output page and renamable as before. Runs skip the model entirely, which makes them much faster. Engine option `namingMode: "rules"`.
- **Parallel multi-file analysis.** In the desktop app the files of a run are analysed at the same time, one thread per file (largest first), within a memory budget (half of the available memory by default; `memoryBudgetMb`, `maxParallelFiles`). Progress is merged into one bar and cancelling stops every file. Results are identical to a one-thread run (tested). The desktop app keeps one CPU core free for its window.
- A deterministic demonstration dataset generator: `cargo run --release -p dbflow-core --example demo_data -- <dir>` writes nine related retail tables of 99 columns each (999 to 99,999 rows; 431,991 rows and 891 columns in all). `examples/bench` now reports peak memory and can save the result (`DBFLOW_BENCH_OUT`).
- `bun run audit:design` (the script was documented but not registered).

### Changed — scalability
- Wide tables no longer exhaust memory. Candidate keys are enumerated with the Lucchesi–Osborn algorithm instead of a search over every subset of attributes (which ran out of memory on a 99-column table), with the same keys as result.
- FD mining prunes the supersets of a column that is nearly unique on the full data (two unrelated measures were otherwise found as a key, with thousands of dependencies each). A key found in a sample is confirmed on the full data before it yields dependencies, so a composite key such as `(order_id, line_no)` is still found when one of its columns looks unique in the sample. The 30 s time budget now covers all sampling rounds together (each round used to get its own).
- Cells are smaller and copied without copying text: short values (up to 22 bytes) are stored inline and longer ones are shared, so projecting rows into relations no longer duplicates strings. Normal-form steps borrow their input instead of copying it, and the dependency checks no longer copy rows.
- Inputs above 999,999 cells get a compact result: relations report `rowCount` instead of carrying their rows, and each relation's data is exported once as CSV (the 2NF CSVs, the per-relation JSON rows, the BCNF CSVs that repeat the 3NF ones and the INSERT script are left out, with a note in the log). Multi-file runs release the rows of the intermediate normal forms as soon as a table is analysed.
- Column types are inferred once per schema and shared by the ER diagram, the DDL and the data export (they were inferred three times). Cross-file foreign-key profiling and matching run in parallel and stop counting once a candidate can no longer reach the containment threshold; normalized-data export deduplicates rows with a typed key instead of a JSON string per row and exports relations in parallel.
- A single column's near-uniqueness is judged on the full data, not on the FD sample (a customer id with ten orders each looked unique in a sample of a million-row table, so its dependencies were not found).
- On the nine-table demonstration set (335 MB of CSV) a full run takes about 55 s on four cores (115 s on one) with about 7.8 GB peak memory; a 999,999-row table takes 16 s and a 999-column table 23 s. Before these changes key discovery ran out of memory on the first table.
- A relation split out by 3NF is keyed by its most identifier-like column when several of its columns determine each other (`city_code` rather than `city_population`).
- The key chosen for a table is now the primary key of its main 2NF and 3NF relation (it used to be whichever candidate key sorted first by name, e.g. a unique measure column instead of `store_id`).

### Fixed
- **Desktop app reset after large runs.** Opening the results passed the whole result through the router's history state; WebKit (the webview on Linux and macOS) rejects large history entries, and the router then reloaded the page, which cleared every workspace. Results now go through the per-workspace cache and only the workspace id is navigated.
- Relation cards on the Output page showed "0 rows" for large inputs (they now read the reported row count).
- Pressing **Output** during a run no longer rebuilds the canvas as one long row of steps and starts a second run; the results open when the running pipeline finishes. With no results yet, Output runs the pipeline on the canvas (only an empty canvas gets the full default pipeline, laid out in rows like any other).
- Closing the last workspace asks for the name of the workspace that replaces it (it used to open "Workspace 2" without asking).
- Closing a workspace with work in it asks in an in-app dialog that matches the workspace-name dialog, with focus on Cancel, instead of the browser's confirm box; a running pipeline is mentioned and stopped when the workspace closes.
- Dialogs make the whole page behind them inert (the app shell around a dialog used to stay reachable by keyboard and screen readers).
- The window stays responsive during long runs: progress updates are applied at most once per frame, the result cache is written after the run instead of during the click, very large generated files are kept in memory only (not serialised into session storage), and in the desktop app file uploads run off the main thread.
- Files uploaded for a run that is cancelled before it starts are released in the desktop app.
- Loading a file that is not a saved workflow shows a notice instead of a browser alert.

## [6.0.0] - 2026-11-14

### Changed — the engine is now Rust
- **Every parser and algorithm moved from TypeScript to Rust** (`crates/dbflow-core`): CSV parsing (delimiter sniffing, quotes, encodings), spreadsheet reading (calamine), cleaning, atomicity and type profiling, the column classifier, functional-dependency mining, key selection, closures and canonical cover, 1NF/2NF/3NF/BCNF, the chase, dependency preservation and the re-join check, cross-file foreign keys, schema assembly, the Mermaid/Chen/Barker/UML diagram builders, SQL DDL in every dialect, normalized-data export and all reports. React keeps only the UI.
- **Native in the desktop app.** Runs execute on a background thread of the app process through Tauri commands (`engine_upload`, `engine_prepare`, `engine_finish`, `engine_cancel`), with progress streamed over a channel and cancellation by flag. Per-column and per-relation work runs on all CPU cores.
- **WebAssembly everywhere else.** The same crate is compiled to WebAssembly (`crates/dbflow-wasm`, built by `bun run build:engine`) for browser development, the Web Worker fallback, the CLI and the test suite, so every runtime uses one implementation.
- **Faster.** On a 150 000-row, 10-column file with all 19 operations, a full run takes 2.8 s natively and 4.4 s as WebAssembly, against 6.5 s for the TypeScript engine of 5.0.0. Results are the same: the 671 existing tests, golden snapshots included, pass unchanged against the Rust engine, and the generated files match 5.0.0 except for time stamps and JSON key order.
- The TypeScript modules under `src/services` keep their APIs as thin wrappers over the engine (`engine('nf.findKeys', …)`); a JSON dispatcher (`api.rs`) exposes every engine function to both WebAssembly and Tauri (`engine_call`).
- Spreadsheets are read by the Rust engine. SheetJS remains only as a fallback for formats it cannot decode (`.xlsb` short-cell records, BIFF2-5 `.xls`, SpreadsheetML 2003 `.xml`, flat `.fods`).
- The process log of a single-file run now covers that run only (it used to accumulate across runs), and the performance report lists the Rust engine among the optimizations.

### Build
- New prerequisites for building from source: the `wasm32-unknown-unknown` Rust target and `wasm-bindgen-cli` 0.2.130. `bun run dev`, `bun run build` and `bun run test` build the engine first; `bun run test:rust` runs the Rust tests.
- The Rust code is a Cargo workspace at the repository root (`crates/dbflow-core`, `crates/dbflow-wasm`, `src-tauri`); CI runs `cargo fmt`, `clippy` and `cargo test` for the whole workspace and builds the WebAssembly engine before the frontend checks.
- Minimum Rust version is 1.80.

## [5.0.0] - 2026-11-14

### Changed
- **React 19** (from 18), with React Router, React Markdown 10, Framer Motion 14, Lucide 1, Mermaid 12, Vite 8, `@vitejs/plugin-react` 6, Tailwind 4.3 and the other frontend packages at their latest releases. The Rust side moves to the latest Tauri 2 patch release and crate versions.
- **TypeScript 6.0**: the `baseUrl` option is dropped (the `@/` alias uses a relative `paths` entry). TypeScript 7 is not used yet because typescript-eslint does not support it.
- **Bun** is the package manager: `bun.lock` replaces `package-lock.json`, and CI, the release workflow, the Tauri dev/build hooks and the docs use `bun`.
- CI actions moved to current major versions; Node 24 is used for the tooling that still needs it.

## [4.5.0] - 2026-11-14

### Changed
- The software is renamed from NormaFlow to **DBFlow**: app name, window title, package and crate names, bundle identifier, repository links, documentation and the download page. The logo and all behaviour are unchanged.
- Browser storage keys and the workflow snapshot format id use the `dbflow` prefix now; settings and saved results stored under the old `normaflow` keys are not carried over.

## [4.0.0] - 2026-11-13

### Added (download page)
- `site/`: a one-page website that picks the installer for the visitor's system (Windows, macOS with both Apple Silicon and Intel, Linux), lists every installer with its size and SHA-256, and explains the unsigned-installer prompts. It reads the latest release from the GitHub API when the page loads, so it needs no update after a release; if GitHub cannot be reached it links to the Releases page. No cookies, no analytics, a strict content-security policy. The hero is a real result (`enrollment.csv` under 3NF and BCNF, with the dependency that BCNF loses) in relation-schema notation, set in Computer Modern and Inter (SIL OFL, bundled in `site/assets/fonts` with their licences). `.github/workflows/pages.yml` publishes it to GitHub Pages when `site/` changes on `main`.

### Fixed
- The README release badge said v2.6.0 for a v3.0.0 release; it now reads the latest release from GitHub.

### Added ("run everything" in the request box)
- A request such as "run all steps", "run everything", "use every option", "the whole pipeline" or "end to end" now builds the pipeline from **every operation in the sidebar** (all 19). Before, these words were ignored and the request matched nothing. Skips still apply: "run everything except the report" gives 18 steps, and "run all steps without 3NF" also drops the steps that need 3NF (BCNF, Relation Generator, SQL Generator). A SQL dialect and "run" in the same request still work. Plain requests such as "normalize all tables to 3NF" are unchanged.

### Changed (operation renamed)
- The **NER Classifier** operation is now the **Column Classifier**. It was never named-entity recognition: it is a rule-based step that labels each column (identifier, person name, price, date, ...) from its header and sample values, and the labels are shown in the multi-file overview only. Saved pipelines, scripts and command-line calls that use the old name still work, and "run ner" still finds the step.

### Changed (quality document)
- `docs/QUALITY.md` now states that DBFlow follows the ISO/IEC 25000 SQuaRE series as a self-assessment (not a certification), with current numbers (646 tests, 0 accessibility violations), the Windows, Linux and macOS install check, and the limits of the on-device naming model.

### Removed
- Root-level leftovers: `NOT_DONE.md`, `IMPROVEMENTS.md` (the audit work list and its open-items file; both remain in git history), a duplicate `icon.png` (the app uses `public/icon.png`) and `shell.nix`.

### Fixed
- SQL dialect: a dialect named in the request box ("... generate SQL for PostgreSQL", also MySQL, SQLite, ANSI) was shown in the preview but never reached the engine, so the SQL always came out in the default dialect. It is now passed on each run in that workspace and kept until another dialect is requested. For a single file it switches to the dialect-aware generator (the one multi-file runs use); with no dialect requested, the single-file output is unchanged.
- Renaming a relation: the box opens with the current name selected, so typing replaces it instead of adding to it.
- Relation naming: when the decomposition uses a key equivalent to the table's own (for example `product_name` for `product_id`), no relation held the whole key and the file name went to the first relation, often the wrong one. The file name now goes to the relation holding the most key columns, then the one with the largest key of its own.
- Output page sidebar: the active row's teal outline was cut off along its top edge when it was the first row under a section heading (Overview, Normal Forms, ERD Diagram); the sticky heading covered it. There is now a little space under each heading.

### Changed (floating workspace tabs)
- The workspace tabs are now floating, fully rounded pills with the same radius and shadow as the header, instead of file-folder tabs joined to it. The space is even: 12 px above the strip, 12 px between the strip and the header, 12 px at the left edge, 8 px between tabs. The active tab has a teal outline; a tab colour shows as a bar along its top edge. The header still takes the active tab's tint.

### Added (model-based relation names)
- Relations created by normalization are no longer named by joining column names (`address_city_zip`). The engine gives each new relation an opaque id (`relation_1`, ...); the relation that holds the table's key keeps the file name; the others are named by a small on-device model (SmolLM2-360M-Instruct, 4-bit, Apache-2.0) as one noun phrase, or stay "unnamed" with their id when there is no answer. No naming rules are written by hand.
- The model sees the **column names only** (never rows) and runs in a Web Worker with remote loading off, so data stays on the machine. Only the relations of the final schema are named (one model call per distinct set of columns, about 1 s each in Node; about 58 s for a 3-relation table including the first model load in Chromium).
- Names are marked **suggested** (model) or **unnamed**, with a **rename** link. A name you type is remembered for the same table and columns and wins over the model on the next run. Output text (ERD, SQL, report) uses these names; a rename shows in the relation list at once and in generated text on the next run.
- Measured quality: 15 of 22 tables named correctly (68%) on Chinook and Northwind; junction tables tend to come out as one of their parents. Treat names as suggestions. See `docs/notes/model-naming-spike.md`.
- **Installer size:** the model is about 373 MB and is fetched at build time by `scripts/prepare-nl-assets.mjs` (SHA-256 checked, git-ignored); release builds fail if it cannot be fetched. This is a conscious departure from the "no multi-hundred-MB model" goal, accepted by the project owner.
- Tests, the CLI and builds without the model name relations by id (`relation_N`) and say so; existing tests now assert structure rather than concatenated names.

### Added (workspace naming)
- The app asks for a workspace name when it opens, and the **+** button (or Ctrl/Cmd+T) asks for a name before adding a workspace. Enter confirms, Escape or Cancel adds nothing (the first prompt cannot be dismissed, since a workspace must exist), an empty name is refused, and a duplicate name is allowed with a note. A name you choose is kept when you upload a file (a file used to rename an untouched tab). The dialog is modal: the page behind it is inert and Tab stays inside.

### Added
- **Light and dark mode.** A switch at the end of the workspace strip (so it is on both the pipeline and the Output page) toggles the theme. With no choice made, the app follows the operating system; the choice is remembered. Diagrams stay on a light page in dark mode so their colours stay true.
- `docs/QUALITY.md`: the app assessed against the ISO/IEC 25010 characteristics and ISO/IEC 25012 data quality, with the evidence for each and what is not covered. `npm run quality` runs the automated gates; `npm run audit:a11y` runs an axe-core audit of the pipeline page and every Output tab in both themes (currently 0 violations).

### Changed (pipeline and Output page, a calmer look)
- **Pipeline canvas:** steps now wrap into rows of five (they used to run in one line, so a full pipeline fitted at an unreadable zoom), the view re-frames when steps are added, and wrapped links route as right-angle steps. Nodes are flat cards with a small accent chip; the glowing emerald Start node, pulsing halo, travelling pulses, marching dashes and teal-to-violet gradient edges are gone.
- **Output page:** every summary strip uses one neutral `StatCard` (the number in ink, a small dot for good / watch / problem) instead of green / yellow / red / blue tinted boxes; the column-classification pills are neutral except primary and foreign keys; section headings are in order.
- Cardinality and Keys lose their leftover blue / green / purple / yellow pills and tiles: neutral chips, teal accents only where something is a key.
- Removed about 800 lines of unused legacy CSS.

### Changed (design consistency)
- Large schemas (more than 12 relations) open the ERD as the compact entity / relationship overview instead of with every attribute drawn; **Show attributes** brings them back.
- Text sizes are down from nine to seven steps (the 10.5 and 11.5 px sizes folded into 11 px) and the pipeline step chips use the same 8 px radius as buttons. `npm run audit:design` measures typefaces, text sizes, button heights and radii, and card radii on every view in both themes, and the rules are written down in `docs/DESIGN.md`.

### Fixed (accessibility, WCAG 2.1 AA)
- Workspace tab buttons and the canvas zoom controls now ease on hover and focus like every other button (they snapped).
- The "added because a later step needs it" chips in the plan preview failed contrast in light mode; fixed.
- `npm run audit:ui` checks the whole interface in one pass: the pipeline page (empty and plan preview) and all ten Output tabs, for one table and for three, in light and dark at 900, 1300 and 1700 px (144 views), for page overflow, content running past its panel, console errors and axe violations. Currently clean.
- The workspace strip is a labelled navigation of real buttons (it nested buttons inside tabs, which screen readers cannot operate).
- Contrast: the main action (teal 700, 5.5:1), small grey text, locked operations (no longer faded), status badges, code comments and line numbers.
- Result panels and diagrams are focusable regions, so scrolled content works from the keyboard; the pipeline page has a heading and landmarks.
- `prefers-reduced-motion` now disables every animation and transition.
- Dev dependencies: vitest 3 to 5 clears the two critical advisories (tinypool); shipped dependencies have no high or critical advisories.

### Fixed (engine: fewer coincidental dependencies)
- **Over-decomposition on real data:** 11 Chinook tables came out as 48 BCNF relations, many of them built from coincidental dependencies. Dependency mining now (a) skips near-unique determinants (their dependencies hold by accident, which also makes mining about 3.7x faster on large tables) and (b) only uses determinants with no missing values. Chinook now yields 409 dependencies instead of 779 and 30 relations instead of 48, and the generated SQL loads into SQLite with foreign keys enforced and no violations (it did not load before).
- **Foreign keys:** a link is no longer drawn in both directions between a key and its child table (it produced a bogus 1:1 `invoice.InvoiceId -> invoiceline.InvoiceLineId`), and links where not every value matches are listed as unplaced instead of becoming a constraint that fails on load.
- **CSV:** the delimiter sniffer no longer misreads files with escaped quotes followed by delimiter-heavy text.

### Changed (small-sample guard)
- Tables with fewer than 10 rows now only use key determinants when mining dependencies: with so few rows almost any two columns look dependent by chance, and each such dependency split off a spurious relation. On Chinook this removes 25 coincidental dependencies (409 to 384) and takes the result from 30 to 25 relations, with the generated SQL still loading with foreign keys enforced. It is a heuristic: a genuine dependency in a table of under 10 rows is no longer found. The threshold is the `minRowsForNonKeyDeterminants` option (0 turns it off). The Chen diagram's 1 / M / N labels also sit further from the boxes.

### Added (Output page, part 6)
- **Small-table warning** on the Dependencies tab: when dependencies come from tables with fewer than 100 rows, a note says how many (Chinook: 295 of 409, from `customer`, `employee`, `genre`, `mediatype` and `playlist`) and that with so few rows a dependency can hold by chance, so they should be treated as candidates to check. It does not change what the engine mines.

### Fixed (Output page, part 5)
- **Narrow windows:** the dependency table no longer clips its **View** column below ~1000 px (Coverage and Quality columns hide as the window narrows). Checked at 900x640, 1300x820 and 1700x1000.
- **Generated SQL:** per-table comments no longer promise "Performance optimized" / "Optimized for reference queries" / "Optimized for analytical queries" (they now say what the table is), and the section banner no longer says "VALIDATED".
- **Cardinality** (single table and multi-table) and **Generated Files** at 900 px: the cardinality table no longer clips its last columns (Total values, Data type, Values matched and Confidence hide as the window narrows; relationship types shorten to 1:N / M:N), and file sizes no longer wrap onto three lines.
- **Normal Forms** at 900 px: the BCNF tab was cut off and long relation names ran into the attribute count; the tab labels shorten and the card header wraps.
- **Quality Assurance:** "Lost Dependencies: 0" is grey instead of an alarming red.
- **Chen ERD:** the 1 / M / N cardinality labels sit clear of the entity boxes.

### Fixed (Output page, part 4)
- **Crow's foot** has the **Show attributes** toggle too (the Mermaid diagram collapses to table names only), so all four ERD notations share the same controls.
- **Normal Forms** in the sidebar counted relations across all four levels (145 for 11 tables); it now shows the relations in the final schema.
- **Report:** the quality chips for a multi-table run read "Dependencies lost (9/11 tables)" when 9 of 11 were preserved; they now say "Dependencies preserved (9/11 tables)" and turn amber unless all pass. "Data retained" is no longer blank for multi-table runs.
- **Primary key choice:** when two candidate keys identify each other (an id and a unique name), 3NF synthesis now keeps the preferred one (the id) instead of whichever came first in the minimal cover. Album is keyed on `AlbumId`, not `Title`.
- **SQL header:** the generated DDL no longer claims "MATHEMATICALLY VALIDATED … GUARANTEES: lossless join + dependency preservation" or carries an author line; it states what it is and points to the per-run analysis for the checks.

### Changed (Output page polish, part 3)
- **Crow's foot** now has the same toolbar as the other notations: zoom, fit, Copy Mermaid, download `.mmd` and SVG. All four notations render through one viewer.
- **Chen** diagrams of large schemas (more than 10 relations) use a left-to-right tree layout instead of a force layout, so labels stay readable at 100%.
- **Copy buttons** (SQL, ERD source, generated-file previews) confirm with a check mark and "Copied" for a moment; a blocked clipboard no longer throws.
- **SQL tab** header matches the other panels (compact title, same corners).

### Added (ERD notations)
- The ERD tab now offers four notations: **Chen**, **Crow's foot**, **Barker** and **UML**. Barker shows rounded entities with `#` unique identifier / `*` mandatory / `o` optional attributes (optional is read from empty values in your data), dashed lines for optional relationships and a bar on the crow's foot when the relationship is part of the identifier. UML shows class boxes with typed attributes and «PK» / «FK» stereotypes and multiplicities (`1`, `0..1`, `0..*`, `1..*`). Chen, Barker and UML are drawn by the bundled Graphviz, with zoom, optional attribute lists and DOT / SVG download named after the notation. Large schemas lay out left-to-right so they grow downwards. Chen cardinality labels now sit beside the entity instead of under its edge.

### Changed (Output page polish)
- **Functional dependencies:** the table now fits its panel (it used to overflow and hide the View column), shows which table each dependency comes from, and has a confidence filter (All / High / Medium / Low), a search box and paging (50 at a time, with Show more / Show all) so hundreds of dependencies stay usable.
- **Multi-table runs now fill every tab.** Cardinality & Relations lists the foreign keys found between your tables (cardinality, share of values matched, confidence); Keys shows primary and candidate keys per table; Quality Assurance shows lossless-join and dependency-preservation per table with a summary and expandable detail. The report's quality chips use the same per-table verdicts. Before, these tabs said "No data" even though the checks had run.
- **Normal forms:** compact 1NF / 2NF / 3NF / BCNF tabs with counts, smaller card headings and section labels.
- **ERD:** zoom buttons (and Ctrl + scroll) plus fit-to-width, so large diagrams are readable.
- **Overview** for a single table now shows rows, columns, dependencies and duration plus the normalization journey (it was just four cards, two of them meaningless for one table); the multi-table column classification uses the same card style.
- **ERD:** small diagrams keep their natural size instead of stretching to the panel; large ones are packed so separate groups of tables sit side by side. Crow's-foot view uses the same compact header. Generated Files category chips scroll in one row instead of wrapping into four.
- **Transitions:** switching sections now eases in (the `nf-fade-in` animation was referenced but never defined, so nothing animated); hover and focus states are consistent, reduced-motion is respected, and leftover gradient headers and inline font styles are gone.

### Fixed
- **View** on a functional dependency now opens its analysis directly under that row instead of at the bottom of the whole list, so it works with hundreds of dependencies.

### Changed (smaller start-up bundle)
- The start-up JavaScript dropped from about 1.95 MB to about 0.8 MB (React, the flow canvas and the app). The Output page, Mermaid (crow's-foot diagrams) and the PDF library now load on first use. Build warning about large chunks is gone: what remains above 500 kB is Graphviz WebAssembly, Mermaid's diagram engines, the spreadsheet parser and the embedding worker, all loaded on demand from the installed app.

## [3.0.0] - 2026-11-11

### Changed (system notifications)
- Messages no longer pop up inside the app. Pipeline results, errors, warnings, saves and uploads are sent to the operating system's notification centre. The title is the **workspace name** (a renamed workspace shows its new name) and the text is the message; the OS already shows the app name and icon. Routine confirmations (adding a step, connecting two steps) are only logged. The in-app toast stack, the bottom status pill and the notification sound are removed, and the `sonner` dependency with them.
- The desktop app uses the Tauri notification plugin (permission is asked on first use); a plain browser falls back to the Web Notifications API; if neither is allowed nothing is shown.
- The **Undo** that used to sit on the "Pipeline built" toast is now an **Undo last build** button under the command bar.

### Added (Chen ER diagrams)
- The ERD tab now draws **Chen notation** by default: rectangles for entities (double border for weak entities), diamonds for relationships with 1 / M / N cardinality, ellipses for attributes with keys underlined. Foreign-key columns appear as relationships, and junction tables as one many-to-many relationship that carries their own columns. Layout is done by Graphviz compiled to WebAssembly and bundled with the app, so nothing needs installing and it works offline. Switch to **Crow's foot** (Mermaid) with the toggle, hide attributes for a compact overview, and download the diagram as SVG or DOT.

### Added
- The **Full Report** tab shows real components (summary, quality checks, normalization journey, strongest dependencies, performance, operations) instead of a Markdown preview. **Download PDF** builds a PDF from the generated Markdown, offline.
- Natural-language input is more robust: Unicode and zero-width cleanup, a 400-character cap, conservative typo repair, "don't run it" and "SQL is not needed" understood, embedding timeouts and vector validation, and no unhandled errors in the command bar.

### Fixed
- Performance report: **Memory (MB)** is no longer `N/A` in webviews without a heap counter (it shows an estimate of the loaded data, labelled), and **Cache Hit Rate** is measured (partition-cache hits during FD mining) instead of a constant 0.0%.

### Changed (header layout)
- Undo, redo and Reset now sit beside the logo on the left; Open, Save and Upload are centred in the header; Run and Output stay on the right. The three zones are a grid with equal side columns, so the middle group stays exactly centred.

### Changed (Output page look)
- The Output page now uses the same layout as the pipeline page: a rounded header card, a sidebar card styled like the operations palette (titled header, grouped sections, tinted icon tiles, name and one-line description per row) and a white rounded content panel.
- Removed the header labels "Workspace …", "Complete Pipeline Executed" and "N Tables Processed". The tab strip already says which workspace you are in, and the header now just reads "Analysis results" with the time the page was opened.

### Changed (tabs on every page)
- The workspace tab strip now sits above both the pipeline and the Output page, so you can switch, add, rename, recolour and close tabs from either one, in any order. Ctrl/Cmd+T, Ctrl/Cmd+W and Ctrl+Tab work on both pages.
- The Output page follows the active tab: each tab shows its own results (and resets to its Overview), and a tab that has not run yet shows "No results yet for …" with a button back to its pipeline. One tab's fresh results are never shown under another tab.
- The Output header takes the active tab's colour, continuing the tab like the pipeline header does.

### Fixed (Output page)
- The **SQL Schema** tab took seconds to open on large schemas. It used `react-syntax-highlighter` (Prism), which builds a React element for every token of every line before showing anything. It is replaced by a small virtualized viewer (`CodeViewer`) that renders and highlights only the rows on screen, with line numbers, horizontal scroll and its own SQL highlighting. `react-syntax-highlighter` and its types are removed from the dependencies.
- The Output page now says which workspace its results belong to: a badge with the tab's name and colour in the header (the header and its bottom border take the tab's colour). With several tabs open, results can no longer be mistaken for another tab's.

### Removed (header)
- The **Full 3NF** button. Build the pipeline from the sidebar or the text box instead.

### Added (spreadsheet input)
- Excel and LibreOffice/OpenDocument workbooks can be loaded next to, or instead of, CSV files: `.xlsx .xlsm .xlsb .xltx .xltm .xlam`, `.xls .xlt .xla .xlw` (Excel 2.x to 2003), Excel 2003 XML, `.ods .ots .fods`. Formats are read from the file's bytes, so a wrongly named file still loads.
- Each visible, non-empty sheet becomes one table (`book.csv`, or `book_Sheet.csv` when there are several), so multi-sheet workbooks get cross-table foreign-key detection. Hidden and empty sheets are skipped and logged.
- Dates become ISO 8601, cached formula results are used (formulas are never evaluated, macros never read), error cells become empty, float noise (`0.30000000000000004`) is removed, and text ids keep their leading zeros.
- Damaged, truncated and password-protected files are rejected with a message naming the file; tab-separated text saved as `.xls` is read as CSV.
- New dependency: SheetJS `xlsx` 0.20.3, installed from the SheetJS CDN (the npm `xlsx@0.18.5` has known CVEs). Loaded on demand (about 160 kB gzipped), only when a spreadsheet is opened.
- Workers are now built as ES modules (`worker.format: 'es'`), matching how they are already created, so they can lazy-load code.

### Changed (sidebar)
- The operations sidebar is now a list: every operation with its name and a one-line description, grouped by phase with per-phase and overall progress (`8/19`), and a filter box. Narrowing it gives an icon-only rail with tooltips; widths from 64 to 360 px; arrow keys move between operations.
- Each operation shows what a click will do: added (check mark), ready, or locked (a prerequisite is missing). Clicking a locked step now adds the steps it needs, in order, instead of failing with an error: "Added 3NF with 7 prerequisites: …".
- The default sidebar width and its saved setting changed, so old saved widths no longer apply.
- Removed about 425 lines of the old icon-grid and tooltip CSS.

### Removed
- The built-in sample dataset. Nothing is preloaded any more, so results always come from files you upload. Run, Full 3NF and Output now ask for an uploaded file instead of silently analysing four sample rows.

### Changed (UI consistency)
- One shared `Button` (primary / secondary / ghost, one radius, two heights) replaces the dozens of hand-styled buttons. Documented in `docs/DESIGN.md`.
- Header: every action now has an icon and a label (labels collapse to icons on narrow windows); "Run" is the only primary button and "Synthesize 3NF" became "Full 3NF". Undo and redo are the only icon-only buttons.
- Natural-language box: the particle-effect input is replaced by a plain text field and a "Build" button; Apply / Replace is primary, Apply & Run secondary, Cancel ghost.
- Output page: Back, Copy and Download use the shared buttons (emerald and gradient variants are gone); per-row actions are labelled icon buttons.
- Operation palette: rounded squares with the same radius and border as other buttons instead of bouncing circles.
- Removed unused UI code (the shadcn sidebar, sheet, tabs, tooltip, skeleton, separator, input, code-block and entity-analysis components) and the `react-icons`, `@tabler/icons-react` and four Radix dependencies that only they used. Icons are lucide-only.

### Fixed
- App icon: the source image had a 1 px semi-transparent grey line down its left edge, which was carried into every generated size (taskbar, dock, installer). The edge is cleared and the whole set regenerated with `tauri icon`; `bundle.icon` now lists the files the Tauri docs recommend (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico` with the 32 px layer first) instead of `logo.ico` and a `.webp`.
- The upload button no longer shows a `1` badge before any upload. The count is exactly the files you uploaded.

### Changed
- The active workspace tab now joins the header with no gap: they share one surface colour and the header sits directly under the tab strip.
- The header shows only the app icon; the "DBFlow" title and subtitle are gone.
- Removed the "Embeddings: ready/offline" label under the natural-language box. The bundled model needs no setup, and keyword matching works regardless.

### Added
- Tab colours: a dot on each tab opens a picker (nine presets, a custom colour, or none). The colour marks the tab and tints the header of the active tab. Also in the right-click menu.
- Workspace tabs: several independent workspaces in one window, each with its own canvas, files, results and running job. Click to switch, rename with the pencil on the tab, double-click, `F2` or the right-click menu (a new tab is named after its first uploaded file), `×` or middle-click to close (asks first if it holds a pipeline), `+` or `Ctrl/Cmd+T` for a new one, `Ctrl/Cmd+W` to close, `Ctrl+Tab` to cycle. Up to 12 tabs. Tabs and their state now also survive a visit to the Output page, which previously reset the canvas.
- Natural-language workflow builder: type a request ("convert it to 1NF"), preview the minimal pipeline with its dependencies, then Apply or Apply & Run; one-click Undo. Keywords match first; a bundled all-MiniLM-L6-v2 model (23 MB, runs in-app on WebAssembly) handles paraphrases. Nothing to install and no network use. The installer grows by about 37 MB (model plus WASM runtime); the CSP adds `'wasm-unsafe-eval'`.
- `npm run bench:nl` and a 77-case labelled eval set.
- Command-line mode: `npm run cli -- analyze data/*.csv --out out/ --dialect postgresql --json` runs the same engine as the app.
- Evaluation harness (`npm run bench`): precision/recall/F1 for primary keys, foreign keys and recovered relations on ground-truth schemas, with runtime.
- Normalized data export for multi-file runs: one deduplicated CSV per relation, `INSERT` script in dependency order, and orphan-check queries per foreign key.
- Composite foreign keys and junction (many-to-many) tables are detected; junction tables are noted in the ER diagram and report.
- 1NF moves multi-valued columns into child tables.
- Character-encoding detection for CSV input: UTF-8, UTF-16 (BOM or not) and Windows-1252.
- ESLint with a warning cap that may only go down (`npm run lint`, also in CI).
- Property-based tests (`fast-check`) for lossless decomposition and a differential test of the FD miner against brute force; golden-file tests for ERD and DDL.

### Changed (second round)
- Large files are read completely. FDs are mined on a reservoir sample and every reported FD is validated on all rows; rows that refute a candidate are added to the sample and mining repeats.
- The engine runs in a Web Worker; Cancel terminates it. It falls back to the main thread when needed.
- FD miner uses stripped partitions and skips constant columns in larger determinants.
- Single-file ER diagram and DDL use the shared sized-type inference and collision-free identifiers; single-file outputs run from a step table instead of inline blocks.
- Removed unused dependencies: `uuid`, `@tauri-apps/api`, `@tauri-apps/plugin-opener` (JS side), `@fontsource/dm-sans`, `@fontsource/dm-serif-display`, `autoprefixer`, `postcss`, `tw-animate-css`.
- CSP allows `worker-src 'self' blob:`.
- The logger no longer prints a trailing `undefined`.

### Changed
- One analysis engine. Multi-file runs no longer go to the native Rust engine, which only parsed files and never mined dependencies or normalized. Every file now runs through the same TypeScript routine as a single file and honours the nodes on the canvas.
- Multi-file ER diagram, DDL and report are built from the *normalized* relations, with cross-file foreign keys attached to the relations that hold the columns.
- Only dependencies that hold on every row feed key discovery and the normal forms. Approximate dependencies are still reported but need `acceptApproximateFDs` to be used.
- Missing values are now handled one way in the FD miner: all missing values in a column are one value (`nullSemantics: 'equal'`, default) or all distinct (`'distinct'`). No rows are dropped.
- Primary keys are chosen from candidate keys that are unique and complete; a surrogate `row_id` is added (and materialized) when none exists.
- Foreign keys are found from the data (inclusion dependencies scored by containment, coverage, name and type evidence) instead of requiring `<table>_id` column names; nullable foreign keys are marked optional.
- Column-level atomicity: text containing "and", "&", "+" or parentheses, and name/address columns with commas, are no longer reported as multi-valued.
- Generated DDL infers sized types (`VARCHAR(n)`, `DECIMAL(p,s)`, `BIGINT`, `BOOLEAN`, `TIMESTAMP`), keeps zero-padded digit strings as text, and emits `NOT NULL`/`UNIQUE` from the data. Identifiers that collide after sanitizing get unique names.
- Tables are created parents-first in multi-file DDL.
- Lossless-join reports the chase verdict and a re-join of the projections on the data; the "guaranteed" wording is gone.
- Results are kept in memory with a slimmed copy in `sessionStorage`, so large inputs no longer overflow the storage quota.

### Removed
- Native Rust analysis engine, SQLite history and their dependencies (`sqlx`, `tokio`, `csv`); the Rust side is now just the window shell.
- Unused modules: console, natural-language prompt bar, compatibility checker and its hook, JSON-from-LLM helpers.
- `rehype-raw`: reports embed text from CSV files, so raw HTML in Markdown is no longer rendered.
- `.xlsx` as an accepted file type (there was no spreadsheet parser).

### Security
- Strict Content-Security-Policy for the webview; Mermaid runs with `securityLevel: 'strict'`; Markdown tables and spans escape CSV-derived text.

### Fixed
- Removed the 100-row analysis cap that the UI hook applied to large files; truncation is now reported.
- FD mining can be cancelled and has a 30 s default time budget; a cut-short search is flagged.
- Dependency-preservation lists now use the same projected-closure test as the verdict.
- Notification sound resolved through the bundler instead of a dev-server path.

## [2.6.0] - 2026-08-24

### Added
- Circular icon-only operation rail: every pipeline operation is now a circular logo button grouped by pipeline phase; hovering reveals a slide-out card with the operation name, description and dependency hints (portal-rendered so it never clips).
- Shared visual identity (`operationVisuals`): per-operation lucide icons and phase accent palette (Prepare/teal, Analyze/cyan, Normalize/violet, Generate/amber) used consistently by the rail and the canvas nodes.
- Themed pipeline nodes: phase-colored gradient icon chips, accent side bar, operation descriptions, hover lift and selection glow; Start node features an emerald gradient with a breathing halo.
- Lively gradient pipeline edges: teal→cyan→violet gradient stroke with marching dashes, a travelling pulse dot (`animateMotion`) and glow underlay, per the official React Flow "Animating Edges" and "Turbo Flow" examples.
- React Flow theming via `--xy-*` CSS variables, glossy connection handles, phase-colored minimap nodes and a refined dotted canvas background.
- Auto fit-view framing the full chain after "Synthesize 3NF" builds it.

### Changed
- Operation palette search box and collapse toggles replaced by the compact icon rail (labels are revealed on hover).
- Node spacing increased (220 → 300 px) so pipeline edges are clearly visible between nodes.

### Fixed
- "Synthesize 3NF" built the full pipeline without any edges — the canvas is now properly wired node-to-node (`replaceGraph` derives chain edges when none are supplied).
- React Flow's base stylesheet was never imported, leaving controls/handles/edges unstyled.

## [2.5.0] - 2025-08-24

### Added
- Interactive canvas controls toolbar (`<Controls />`) supporting zoom in, zoom out, fit view, and interactive canvas lock.
- Directional arrow markers (`MarkerType.ArrowClosed`) and custom styling on all React Flow pipeline edges.
- Modern high-density dual-pane workspace architecture with transient toast notifications.

### Changed
- Removed redundant bottom console and natural-language prompt bar to maximize canvas viewport and performance.
- Streamlined toolbar controls for rapid schema synthesis and workflow state management.

## [2.4.0] - 2025-08-24

### Added
- FOSS project infrastructure: MIT `LICENSE`, `CONTRIBUTING.md`, Code of Conduct, security policy, issue/PR templates, CI pipeline and automated multi-platform release builds.
- Fast functional-dependency mining based on partition refinement with lattice pruning (replaces exhaustive pairwise re-validation).
- NLP-based semantic column profiler fusing column-name evidence with value-pattern evidence.
- Manual node creation: operation palette supports click-to-add and drag-and-drop onto the canvas.
- Rust engine: parallel per-file CSV processing, sampled value statistics, data-driven SQL type inference, value-overlap foreign-key verification, configurable Ollama endpoint.
- Unit tests (Vitest + cargo test) and `npm run typecheck` / `npm run test` scripts.

### Fixed
- Entity classifier corrupted table names by stripping every letter "s" instead of only a trailing plural.
- React StrictMode double-invocation caused duplicate nodes/edges when adding operations (side effects moved out of state updaters).
- Ollama chat over Tauri dropped conversation history (only last message was sent).
- LLM responses wrapped in Markdown code fences failed JSON parsing during flow generation.
- 3NF decomposition projected full original rows into decomposed relations instead of the relation's attributes.
- FD mining mutated arrays being iterated (`lhs.sort()` inside combination loop).
- Mermaid ERD emitted invalid trailing spaces after non-PK columns; identifiers are now sanitized.
- Duplicate dependencies removed (`reactflow` v11 alongside `@xyflow/react` v12, `framer-motion` alongside `motion`).

## [2.3.0] - 2025-08-02

### Added
- Multi-CSV pipeline with cross-table relationship detection.
- Rust core engine (`process_multi_csv_rust`) for high-speed parsing and ERD/SQL/report generation.
- Ollama AI analyst integration and natural-language workflow generation.
- Visual workflow canvas (React Flow), undo/redo, workflow save/load.
- Normalization reports, Mermaid ERDs and consolidated SQL DDL outputs.
