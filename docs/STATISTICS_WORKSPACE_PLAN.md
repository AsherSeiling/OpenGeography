# Statistics workspace

Written 2026-11-09 from the user's request for "a separate statistics workspace in the plan". Planned, not built. Packages ST0–ST7; order in `docs/MASTER_PLAN.md`.

## What it is

A place to explore and test a layer's or table's numbers without building a node tree. It works like the layout workspace: opening a statistics sheet changes the window, and Back to Map returns it.

Today statistics are scattered tools in the Shift+A menu and the "Analysis" ribbon groups (Summary Statistics, Moran's I, Hot Spots, and the new T4 tools). Each returns a table or a layer, with no charts of its own and no general tests or regression.

## Layout of the workspace

Same style as the rest of the app: ribbon plus dockable panes.

| Area | Holds |
|---|---|
| Left: Data | Layers and tables, their fields with types and missing counts; filter; "Selected features only" |
| Centre: Sheet | A scrolling column of analysis cards. Each card shows its result tables, chart and notes |
| Right: Properties | The selected card's inputs and options, drawn from its `ToolSpec` |
| Ribbon | Contextual tabs: Explore, Charts, Tests, Regression, Spatial, Export |

## Rules

**One tool definition.** Every analysis is a `ToolSpec` in `nodes/` with a `Statistics › …` category. The workspace reads `tool_specs` and runs the same tool, so each analysis also works as a node in a tree. Existing tools move into the workspace's list by category; no second catalog.

**The sheet is a document.** It's saved in the project, like a layout: `__stat_sheets` in the project database, created with `IF NOT EXISTS`, so old projects still open. A card stores its inputs and options. Results are cached against the data version, and a card whose data changed is marked out of date, never silently recomputed or left looking current.

**Source data is never changed.** "Save residuals" or "Save predictions" writes a new layer that keeps the inputs' citations and adds the method.

**Statistical correctness.**
- Every card states n used, and how many rows were dropped and why (missing, non-numeric). Dropped rows are never silent.
- Each result states its exact method, in words and with a citation: Welch or Student, sample or population SD (n−1 by default), the quantile definition (R type 7, DuckDB's `quantile_cont`).
- Tests show effect size and confidence interval beside the p-value, and show the assumption checks that apply.
- A sheet that runs many tests says so. Nothing is corrected silently; adjusted p-values (Holm, Benjamini–Hochberg) are an option.
- Features near each other are usually not independent, which breaks the assumptions of ordinary tests. Tests on spatial data carry a note saying so, and regression reports Moran's I of its residuals.
- Random methods have a visible seed.
- Anything distance- or area-based follows the project's rules: explicit CRS, geodesic by default, or refuse and say to project first.
- Census margins of error: when the Census importer exists, a field with a margin column warns that ordinary tests ignore that error.

**Checked against published answers.** Every method is tested against reference values, not only that it runs. Use NIST Statistical Reference Datasets (public domain) for descriptive statistics, regression and ANOVA, and textbook worked examples or stored R output for the rest. R isn't shipped or linked; only its numbers are used as expected values.

**Scale.** Totals, means and quantiles run in DuckDB. Rank tests and regression load columns into memory, with a stated row limit and a clear refusal above it. Large scatter plots switch to density binning, and the chart says so. Long work uses `progress::Task` and can be cancelled.

**Charts have one renderer.** Engine SVG, in a new `crates/engine/src/stats/chart.rs`, so a chart looks the same in the sheet, in a layout, and in a PNG or PDF. Today's layout `Chart` element only does bar and pie, and stays as it is.

**Linked to the map.** Clicking a bar, box or point selects those features through the same selection store as the map and table (UX10), and a selection on the map highlights in the chart.

## ST0 result (lead, 2026-11-12)

**Libraries.** `statrs` 0.19.1 (MIT) for distributions (t, F, chi-square, normal, studentized range via its CDFs where present, otherwise our own code with a citation) and `nalgebra` 0.35.0 (Apache-2.0) for matrices (QR for OLS, never the normal equations). Both permissive, both need Rust 1.89+ (we build with 1.99). They're added by the first package that uses them (ST2 or ST4), with rows in `docs/OPEN_SOURCE.md` and `THIRD_PARTY_NOTICES.md`. Shapiro–Wilk is reimplemented from Royston (1995, AS R94) with a citation. No GPL code.

**Result format.** Every statistics tool returns one `StatResult` (engine `stats/result.rs`, serde, mirrored in `api.ts`):

| Field | Holds |
|---|---|
| `method` | One sentence naming the exact method ("Welch two-sample t-test, two-sided") |
| `citations` | Method references as text, plus the input layers' sources |
| `n_used`, `dropped` | Rows used, and `[{reason, count}]` for every dropped row; never empty when rows were dropped |
| `tables` | `[{title, columns: [{name, unit?, kind}], rows}]`; numbers stay full precision, the UI rounds |
| `charts` | `[ChartSpec]`: chart data, not SVG (kind, series, axes, and per mark the feature ids or a bin key) |
| `notes` | Assumption checks, spatial-independence warning, multiple-testing note, density-binning note |
| `seed` | Shown when a random method ran |
| `data_version` | The input layers' version when computed; a sheet card compares it to mark itself out of date |

In a node tree the tool's first table becomes its output table layer, so the same `ToolSpec` works in a tree and on a sheet card.

**Charts.** `stats/chart.rs` renders a `ChartSpec` to SVG at a given size: one renderer for the sheet, layouts, PNG and PDF. The sheet asks for the SVG with `stats_chart_svg(spec, width, height)`.

**Linked selection.** Each mark in the SVG carries `data-mark="<index>"`; the `ChartSpec` maps that index to feature ids (or, for a density bin, a bin key the engine turns into ids on click). The UI inserts the SVG inline (it's ours, but it's still parsed with DOMParser and scripts are stripped), finds the clicked mark with `closest("[data-mark]")`, and selects through the shared selection store. Highlighting a map selection only toggles a `sel` class on the matching marks; it never re-renders the chart.

## Packages

| Task | Effort | Model | What |
|---|---|---|---|
| ST0 | 0.5 | Opus | **Spike:** choose and licence-check the maths libraries (`statrs` for distributions and `nalgebra` for matrices are the candidates, neither is in the build yet); the result format (tables, chart data, notes, citation); how the UI hit-tests engine SVG for linked selection. Writes the result format into this plan. |
| ST1 | 1.5 | Sonnet (Opus reviews the project format) | **Workspace shell:** sheet document, save and reopen, dock tab `stats:<id>`, enter and leave (generalise `enterWorkspace` in `layout/actions.ts`), contextual ribbon tabs, Data pane, cards, out-of-date marks. |
| ST2 | 1 | Sonnet | **Describe:** n, missing, mean, median, mode, SD, variance, min, max, range, quartiles, IQR, skewness, kurtosis, CV, weighted versions, by-group; frequency tables and cross-tabulations. |
| ST3 | 1.5 | Sonnet | **Charts:** histogram (Freedman–Diaconis default, editable bins), box plot (Tukey whiskers), scatter with trend line, Q–Q plot, bar, line, scatter matrix, correlation heat map; linked selection. Okabe–Ito colours. |
| ST4 | 1.5 | Sonnet (Opus checks the statistics) | **Tests:** one-sample, two-sample (Welch default) and paired t; one-way ANOVA with Tukey HSD; chi-square for independence and goodness of fit; proportions; Mann–Whitney, Wilcoxon signed-rank, Kruskal–Wallis; Shapiro–Wilk and Levene; Pearson, Spearman and Kendall correlation with intervals. |
| ST5 | 1.5 | Sonnet (Opus checks the statistics) | **Regression:** OLS with several predictors and categorical terms (coefficients, intervals, R², adjusted R², F, VIF, Breusch–Pagan, residual plots, Moran's I of residuals); logistic regression; save residuals and predictions as a new layer. |
| ST6 | 2.5 | Sonnet (Opus checks the statistics) | **Spatial statistics, including T7:** a spatial weights manager (queen, rook, k-nearest, distance band; geodesic on geographic CRSs; islands reported); Moran scatter plot and Ripley L plot with envelope for tools that exist; **T7's** LISA, GWR / MGWR, and spatial lag and error regression, moved here so they get their charts. |
| ST7 | 1 | Sonnet | **Share:** send a card's chart or table to a layout (a new layout element that points at the card, added without changing old layouts); export tables as CSV and charts as PNG or SVG; a report of the whole sheet with its methods and citations. XLSX uses whatever the Census work adds. |

**Total:** about 11 sessions, of which 1.5 is T7 moved from Phase T. ST2 and ST3 can run alongside ST4–ST6 once ST1 is merged.

**Order inside the phase:** ST0 → ST1 → then two lanes split by files: **A (UI):** `ui/src/stats/`, ST1, ST3, ST7. **B (engine):** `crates/engine/src/stats/` and `nodes/`, ST2, ST4, ST5, ST6.

**Review stops:** ST0, ST1 (project format), ST4, ST5, ST6.

**Not in this phase:** time series, survival analysis, multivariate methods such as PCA and factor analysis, Bayesian methods, and statistics directly on rasters (zonal tools already cover the common case). Each can be a later card, since cards are `ToolSpec`s.

Each package adds its cases to `docs/TEST_PLAN.md`, and every method and library goes in `docs/OPEN_SOURCE.md` in the same commit.
