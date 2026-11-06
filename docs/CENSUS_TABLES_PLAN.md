# Census tables: plan

Status: planned 2026-11-06, nothing built. Requested by the user on 2026-11-06. Order (user, 2026-11-06): after the user-requests list (`docs/USER_REQUESTS_2026-11-05.md`), before server connections and planetary.

## What the user asked for

A full American Community Survey and Census tables importer, and a custom table generator:

- The only setup is entering a Census API key.
- Every Census field is sorted by **what it actually measures**, not by its code or the Bureau's label. For example, "Total population" brings together every field that counts the total population.
- Good search, with useful categories and keywords.
- It downloads the matching boundaries itself and joins the data to them by GEOID automatically.
- Each field and table points to the Census Bureau's own documentation.

## What exists today (D2, `crates/engine/src/census.rs`, `ui/src/dialogs/CensusDialog.tsx`)

- **Join only:** "Join Census Data" adds variables to a layer the user already has, by a GEOID field. The user types variable codes or picks one of 9 presets (`ui/src/census/presets.ts`).
- **Datasets:** ACS 5-year, ACS 1-year, 2020 DHC and 2020 P.L. 94-171.
- **Levels:** state, county, tract and block group. The level is guessed from the GEOID length.
- **Limits:** at most 40 variables. Estimates only, with no margins of error.
- **Missing values:** the Bureau's "no value" codes (−666666666 and so on) become empty and are counted.
- **API key:** stored in the OS keychain (`keyring`, Tauri shell only). It is redacted from messages, and the citation never holds it (DECISIONS D2).
- **Boundaries:** TIGER/Line by year comes from `tiger.rs`, which uses TIGERweb's ArcGIS REST service.

The new feature builds on all of this and keeps the existing join as one way in.

## Design

### 1. A metadata index: what every field measures

The Census API publishes, for each dataset and year, `variables.json` (code, label, concept, group), `groups.json` (tables) and `geography.json` (the geography levels and the `for`/`in` patterns each needs). None of these needs a key. The importer builds its own index from them:

- **Variable:** dataset, year, code, the Bureau's label (for example `Estimate!!Total:!!Male:!!Under 5 years`), table (group), concept, and universe where the Bureau gives one.
- **Measure:** what the number is, normalised from the label path, concept and table number:
  - **Subject:** for example population, income, housing value or commute time.
  - **Universe:** for example people, households, housing units or workers 16+.
  - **Breakdown:** the label path below "Total", such as Male → Under 5 years.
  - **Statistic:** count, median, mean, aggregate, percent, ratio, or margin of error.
  - **Unit:** people, households, dollars of that year, minutes, years and so on.
- **Same-measure groups:** variables that measure exactly the same thing in different tables are grouped. For example, total population is in B01003_001E, B01001_001E, B02001_001E, DP05_0001E and S0101_C01_001E.
  - Grouping happens only when universe, breakdown and statistic all match, and it is confirmed with data: the index builder fetches one state and checks the values are equal before grouping.
  - The search shows one "Total population" entry, with the simplest table as the default and the others listed as "also in".
  - Variables that are related but not equal are shown as "related", never merged.
- **Topics (categories)** come from the table-number subject (B01 age and sex, B19 income, B25 housing and so on) and are mapped to user-facing headings:
  - **People:** population, age, sex.
  - **Race and Hispanic origin.**
  - **Households and families.**
  - **Income and earnings.**
  - **Poverty and assistance.**
  - **Work and jobs:** employment, occupation, industry.
  - **Commuting.**
  - **Education.**
  - **Housing:** occupancy, tenure, value, rent, costs, structure.
  - **Health insurance and disability.**
  - **Origin and language:** nativity, citizenship, ancestry, language.
  - **Veterans.**
  - **Internet and computers.**
  - **Moving:** migration.
  - **Group quarters.**
  - **Data quality:** allocation tables, shown last.
- **Built by a script** (`scripts/census_index.py`, or a Rust bin) for the supported datasets and years. The output is shipped in `resources/` as one compressed file. The app can refresh it for a new year without a key.
- **Curated files**, small and reviewed by hand:
  - Topic overrides.
  - Synonyms and keywords: people / residents → population; broadband → internet subscription; commute → travel time to work; kids → under 18; renters → renter-occupied.
  - Plain-language names for the most used measures.

The Bureau's own wording always stays visible beside the plain name.

### 2. Search

- **Search runs over the index, offline.** It ranks with BM25 over the plain name, the Bureau's label, the concept, the topic and the keywords. The index is about 30,000 entries a year, so a small Rust implementation is enough and needs no new library. If DuckDB's FTS extension is already bundled, it is an alternative; check that in C0.
- **Ranking favours:**
  - A whole-measure entry over its breakdowns: "Median household income" comes before "…householder 25 to 44".
  - Detailed tables (B) and the Data Profile (DP) over others for common measures.
  - Measures available at the geography level the user has chosen.
- **Filters:** topic, dataset, year, geography level, and statistic (count / median / percent).
- **Browse** works without typing: topic → table → breakdown tree, with counts.
- **A test list of queries** sets the expected top results. It covers "total population", "median income", "rent", "commute time", "broadband", "poverty children", "veterans", "language spoken at home" and "home value". The ranking can't regress against it.

### 3. Fetching data

The existing `fetch_level` is generalised:

- **Geography levels:**
  - nation, region, division, state, county, county subdivision, place, tract, block group;
  - ZCTA, congressional district, state legislative districts, school districts, CBSA;
  - American Indian / Alaska Native / Native Hawaiian areas;
  - and urban areas where the dataset has them.
  - The `for`/`in` pattern for each comes from `geography.json`, not hand-written.
- **Area:** whole US, chosen states, chosen counties, or "the area of my map / a layer". For the last, the boundaries come first, and then only the needed parents are requested.
- **Batching:**
  - Requests are chunked to the API's variable limit, and across parents (block groups need county loops in some years).
  - A few requests run at once, with polite back-off.
  - Every request goes through `progress::Task` and can be cancelled. Downloads run in the background, using the UR-9 mechanism.
- **Margins of error:** estimates and their margins (E/M) are fetched together by default.
  - Annotation codes become empty values with a reason column ("suppressed", "not applicable", "too few samples"), and are counted in the report.
- **Cache:** answers are cached on disk by URL **with the key removed**. Repeating a request is free, and re-opening a project doesn't refetch.
- **API key:** the existing keychain flow. Settings and the first-run screen of the importer get a sign-up link, a "test key" button, and a clear message when the keyless daily limit is hit. The key never goes into a project, cache file name, log, citation or error.

### 4. Boundaries and the automatic join

- **Boundaries match the data's year.** ACS year Y uses that year's TIGER boundaries.
  - Tracts and block groups changed in 2020.
  - Connecticut's counties became planning regions from the 2022 ACS.
  - ZCTAs follow the decennial census.
  - The year rule is a tested table, never a guess.
- **Two kinds of boundaries:**
  - **Cartographic boundary files** (generalised, clipped to the shoreline, smaller; best for maps) are the default.
  - **TIGER/Line** (full detail) is an option for analysis.
  - Both are public domain from the Census Bureau; check in C0 where `tiger.rs` already gets them and add cartographic boundaries.
- **The GEOID** is built from the API's component columns (state + county + tract …, zero-padded) and matched to the boundary's GEOID field. ZCTA, place and district fields differ by year; the rules are in one place, with tests.
- **The result is one new layer:** boundaries, plus one column per measure, plus a margin-of-error column per estimate.
  - Unmatched rows on either side are counted and listed, never dropped silently.
  - Water-only and zero-population areas stay, with empty values.
- **Column names:** short plain names by default (`total_population`, `median_hh_income`), with the Census code kept in the field's metadata. An option uses the codes as column names instead.
  - Each field's metadata holds: code, Bureau label, universe, statistic, unit, year, margin-of-error partner and documentation link. This needs optional field metadata in the project format, with old projects still loading; check `layers.rs` in C0.
- **Citations** go through `sources.rs`: dataset, year, table IDs and the API URL without the key. Derived layers inherit as now.

### 5. Custom table generator

- **The basket:** the user picks measures from search into a basket, chooses a geography level, area and year, then chooses either "with boundaries" (a layer) or "table only". Table only uses the UR-14 tables and exports to Excel, CSV and the rest.
- **Derived columns, each carrying its margin of error:**
  - **Percent of a total:** the denominator is offered from the same table's universe total.
  - **Per 1,000 people.**
  - **Density per km² or mi²:** geodesic area, by the project rule.
  - **Sums of breakdowns:** for example "under 18" from the age groups.
  - **Change between two years:** only between non-overlapping 5-year periods, with the Bureau's comparison warning.
- **Margins of error** for sums, proportions, ratios and differences use the Census Bureau's published formulas (ACS handbook "Understanding and Using ACS Data", Ch. 8). That's a published method, so it gets a row in `docs/OPEN_SOURCE.md`.
  - A reliability flag uses the coefficient of variation, with the threshold cited and settable.
- **Medians and other non-additive statistics** are never summed. The UI refuses that with a sentence saying why.
- **Saving a table spec** in the project allows re-running it for another year or area.

### 6. Documentation links

Each measure, table and dataset links to the Bureau's own pages:

- **The variable's API page:** `https://api.census.gov/data/{year}/{path}/variables/{code}.json`.
- **The table on data.census.gov:** for example `https://data.census.gov/table/ACSDT5Y2023.B19013`.
- **The dataset's API page:** `https://api.census.gov/data/{year}/{path}.html`.
- **ACS technical documentation and subject definitions** for that year, and the margin-of-error guidance.

C0 checks that each URL template resolves for several years before it is shipped. Links are opened in the system browser.

### 7. Other Census tables (after ACS works)

- **Decennial:** 2020 DHC and P.L. 94-171 (already there as join targets), and 2010 SF1.
- **Population Estimates:** yearly totals and components.
- **County and ZIP Business Patterns:** establishments, employment and payroll.
- **ACS 1-year supplemental estimates.**

Each is one more dataset in the same index, search and fetch path. Non-Census sources such as CDC PLACES / SVI are out of scope here.

## Work packages

| Id | Who | What | Done when |
|---|---|---|---|
| C0 | Opus spike | Fetch `variables.json`, `groups.json` and `geography.json` for ACS 5-year (two years) and DHC 2020. Measure their size. Confirm annotation codes, `for`/`in` patterns, universe availability, the Connecticut and 2020 boundary changes, the cartographic boundary file locations and the doc URL templates. Prototype the measure parser on B01, B19 and B25. | A short report in this file: sizes, what parses, what doesn't, and decisions confirmed or changed |
| C1 | Sonnet | Index builder and loader: variables → measures → same-measure groups (with value check) → topics; curated files; shipped resource; refresh for a new year | Engine tests over the full ACS index: every variable has a topic and statistic; group equality checks pass; size within budget |
| C2 | Sonnet | Search and browse in the engine, with filters; the query test list | The query list passes; search under 50 ms on the full index |
| C3 | Sonnet | Fetch engine: all levels from `geography.json`, areas, chunking, margins of error, annotations, cache without the key, rate limit, cancel | Tests against a local fake API with a fake key; the key never in cache, log, error or citation |
| C4 | Sonnet | Boundaries by year (cartographic and TIGER), GEOID rules, automatic join, unmatched report, field metadata in the project format, citations | Engine tests per level and year incl. Connecticut 2022 and ZCTA; old projects open unchanged |
| C5 | Sonnet | UI: Census Data dialog or workspace with search, topics, browse, basket, geography and area, year, preview (row and size estimate), background run, doc links; the old join kept as "Join to my layer" | e2e with the fake API; a11y audit clean |
| C6 | Sonnet | Custom table generator: derived columns with margins of error, table-only output, saved specs | Engine tests of every margin-of-error formula against the handbook's worked examples |
| C7 | Sonnet | Other datasets (decennial 2010, PEP, CBP/ZBP, ACS supplemental) | Each searchable, fetchable and joinable |
| C8 | Opus + user | Real-key check by the user on the Mac app; Opus reviews correctness (GEOID, years, margins of error, key handling) | User signs off |

Close review by Opus: C1 grouping, C3 key handling, C4 GEOID and boundary years, C4 project format, C6 margin-of-error formulas.

## Decisions for the user

1. **Order: decided by the user, 2026-11-06:** after the user-requests list, before server connections and planetary.
2. **Column names:** plain names with the codes in metadata (recommended), or Census codes.
3. **Years shipped in the index:** recommended, the last 5 ACS 5-year releases plus DHC 2020, about a few MB (C0 measures it), with older years fetched on demand.

## Not decided yet / to check in C0

- Whether `groups.json` gives a universe for every ACS table, or whether the table shells are needed.
- Whether the bundled DuckDB has the FTS extension.
- The exact keyless daily limit and the message the API returns.
- Whether `tiger.rs`'s TIGERweb source has cartographic boundaries, or whether they come from the Bureau's file server.
