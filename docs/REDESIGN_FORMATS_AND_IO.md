# Design: `nirs4all-io` → `nirs4all-formats`, and a new `nirs4all-io` dataset-bridge library

> Status: **implementation base — decisions resolved, Codex-reviewed** · Date: 2026-05-27 · Owner: GBeurier
> Scope: (1) rename the current Rust file-reader project `nirs4all-io` → `nirs4all-formats`;
> (2) build a **new** `nirs4all-io` library that bridges arbitrary user inputs to the two
> pipeline data formats (`SpectroDataset` and `dag-ml-data`), matching the full expressiveness
> of nirs4all's `DatasetConfig`/`DatasetLoader`, consuming `nirs4all-formats` as its file layer,
> and adding a score-based **inference** engine plus declarative **conventions**.
>
> **Confirmed plan**: **Phase 1** = Python MVP, `SpectroDataset`-compatible, built by **copying the
> business logic out of `nirs4all`/`nirs4all-studio` into a self-contained `nirs4all-io` and
> re-orchestrating it** (Appendix D) — **without touching the originals**; **Phase 2** = a Rust MVP,
> `dag-ml-data`-compatible, soon after (planned in Appendices H.2/J). The re-plug of nirs4all/studio onto
> io is a separate, user-owned effort (out of scope).
>
> **How to read this as an implementation base**: §6 = resolved decisions · §8 = phased backlog with
> per-story copy-logic tags · Appendix D = the reuse map · Appendix E = `DatasetSpec` schema (**E.1
> column selectors, E.2 multi-file merge & relational joins**) · Appendices F–H = inference, conventions,
> materializers · Appendix I = public API · Appendix J = the dag-ml-data gate · Appendix K = tests ·
> **Appendix L = the declaration cookbook (worked use cases — start here to see what you can load)** ·
> Appendix C = the Codex review + integration table.

---

## 0. TL;DR

- **Today**: `nirs4all-io` is a Rust-first, **read-only** library that turns ~58 spectroscopy file
  formats into per-file `SpectralRecord`s. It has **no dataset-level concepts** (no X/Y, no
  train/test, no folds). The dataset-level logic (config, loaders, folder conventions, inference)
  lives in the **Python** `nirs4all` library and is surfaced by `nirs4all-studio`.
- **Proposal**: rename the file-reader to **`nirs4all-formats`** (it *is* a formats library), and
  build a new **`nirs4all-io`** = the **dataset assembly layer**: *Resolve → Infer → Configure →
  Materialize*. It owns conventions, the dataset config schema, the inference engine, and emits
  either a `SpectroDataset` (Python) or a `dag-ml-data` contract (Rust). It must not duplicate
  **parsers** — but (Codex review) "route all reading through `nirs4all-formats`" hides real cost: the
  current Rust tabular reader (`csv_like.rs`) is far from nirs4all's loader semantics (hardcodes
  `nm`/`Absorbance`, requires numeric headers, no NA/categorical/param-precedence). The MVP therefore
  **copies the loader business logic** out of nirs4all into `nirs4all-io` (so io has **no runtime
  dependency on nirs4all** — see the hard constraint below) and routes *vendor* files through formats;
  translating that copied logic to Rust is the Phase-2 effort.
- **Headline new capability**: because file reading goes through `nirs4all-formats`, the new
  `nirs4all-io` can assemble a dataset from a **folder of vendor spectra (OPUS/JCAMP/ASD/…) + a
  reference table** — something the current Python `FolderParser` cannot do (it only scans
  CSV-family files).
- **⛔ Hard scope constraint (user, 2026-05-27)**: this effort **does NOT touch `nirs4all` or
  `nirs4all-studio`** — no edits, no "wiring into" their runtimes. They are in production but *lack
  hindsight* ("manque de recul"); we are **externalizing and generalizing the process from scratch** in
  a clean `nirs4all-io`. The later **re-plug** of nirs4all/studio onto `nirs4all-io` (more likely the
  studio first) is a **separate, user-owned effort with its own planning — out of scope here.**
- **Confirmed direction (user, 2026-05-27)** — two phases, name `nirs4all-io` kept:
  - **Phase 1 — Python MVP, `SpectroDataset`-compatible**, built **copy-and-re-orchestrate**: take the
    *business logic* that already exists in `nirs4all` + `nirs4all-studio` (loader parsing, `FolderParser`
    patterns, `ConfigNormalizer` alias map, `AutoDetector`, `SignalTypeDetector`, `detect_task_type`, the
    **unwired** `RoleAssigner`/`ColumnSelector`/`RowSelector`/`SampleLinker`/`PartitionAssigner` algorithms,
    fold-file parsing, studio `targetTypeDetection.ts`) and **copy it into `nirs4all-io`, then
    re-orchestrate / generalize / make format-compatible** — we are free to re-architect (the originals
    lack recul); we do **not** modify the originals and do **not** rewrite from a blank algorithm. See the
    **Reuse Map (Appendix D)**.
  - **Phase 2 — Rust MVP, `dag-ml-data`-compatible**, "très vite" after Phase 1: translate the copied
    Python logic to Rust. `dag-ml-data` is **planned now** (Appendix H.2 + readiness gate J) but its
    *implementation* waits until that lib is complete enough (gaps: no Python pkg, no `Wavenumber` axis,
    no nirs4all connector).
- **Self-contained principle**: `nirs4all-io` has **no runtime dependency on `nirs4all`** (otherwise the
  later re-plug — nirs4all calling io — would be circular). The single allowed touch-point is a **lazy
  import of the `SpectroDataset` class at materialization** (the pattern formats already uses in
  `to_spectrodataset`); nirs4all may be a **dev/test-only** dependency as a parity oracle. The new value
  is the *DatasetSpec contract + conventions + inference + vendor-corpus + format compatibility*.
- **Biggest risks**: name reuse/PyPI churn; re-implementing a large, partly-unused config surface; the
  **intentional, indefinite duplication** with nirs4all/studio (they keep their code until the user
  re-plugs) → mitigated by a read-only parity oracle, not by deletion; the `dag-ml-data` impedance
  mismatch; and uncalibrated "confidence" numbers.

---

## 1. Current state (grounded)

### 1.1 `nirs4all-io` (the project to rename)
- Rust workspace, MIT, `0.1.0-alpha.0`. Crates: `nirs4all-io-core` (model/contracts), `nirs4all-io`
  (registry + 42 readers + directory walker, the facade), `nirs4all-io-capi` (additive C ABI,
  currently a **scaffold** — only `n4io_abi_version`/`n4io_string_free`/`n4io_core_is_available`),
  `nirs4all-io-cli` (binary `nirs4all-io`: `probe`/`read-json`/`scan`). Bindings: Python
  (`nirs4all_io` PyO3 `_native` + pure-Python projections incl. `to_spectrodataset`), R
  (`nirs4allio`), WASM (`nirs4all-io-wasm`). (`README.md`, `CLAUDE.md`, `docs/DIRECTIONS.md`.)
- **Read-only**: the `Reader` trait has `sniff`/`read_path`/`read_bytes` — **no write/encode**
  anywhere. The only "output" is JSON serialization of records.
- **Data model** (`crates/nirs4all-io-core/src/model.rs`): `SpectralRecord { signals:
  BTreeMap<String, SpectralArray>, signal_type, targets, metadata, provenance, quality_flags }`;
  each `SpectralArray` owns its own `SpectralAxis` (Wavelength/Wavenumber/…); `SignalType` enum;
  `Provenance` with per-source SHA-256. **It models a single acquisition file. There is no
  dataset, no X/Y matrix, no split, no fold.**
- **Integration today**: the Python `nirs4all` library has **0 references** to `nirs4all-io`. The
  only bridge is `SpectralRecordSet.to_spectrodataset()` in the io Python binding
  (`bindings/python/python/nirs4all_io/records.py`), which lazily imports `nirs4all.data`. So the
  rename has **no blast radius inside `nirs4all`**.
- **Direction** (`docs/DIRECTIONS.md`, ADR accepted 2026-05-20): Rust is the reference
  implementation "from day one"; Python/R are distribution targets; "a future C ABI surface
  comparable in spirit to `pls4all`"; parsers live only in Rust.

### 1.2 `dag-ml-data` (one of the two target formats)
- Rust-first **data contract + planning** layer, `0.1.0-alpha.0`, no nirs4all/nirs4all-io
  dependency. Generalizes SpectroDataset's multi-source logic into typed, serializable, fingerprinted
  contracts consumed by `dag-ml`.
- Core schema (`crates/dag-ml-data-core/src/model.rs`):
  `DatasetSchema { dataset_id, sample_ids: Vec<SampleId>, sources: Vec<SourceDescriptor>,
  targets: BTreeMap<TargetId, RepresentationSpec>, metadata: BTreeMap<String, RepresentationSpec> }`.
  `SourceDescriptor { id, name, type_id, modality, native_representation, sample_key, granularity,
  schema, tags }`. `RepresentationSpec { id, type_id, rank, axes, container, dtype, sparse, ragged }`.
  `AxisSpec { name, kind (wavelength/feature/…), unit, size, variable, coordinates }`.
- **Sample indexing is by string ID**, not row. Reps/groups/augmentation/exclusion/targets live in a
  `SampleRelationTable` (`observation_id ↔ sample_id` 1:N; `group_id`, `origin_id`, `repetition_id`,
  `target_id`, `augmented`, `excluded`).
- **By design it has NO folds, NO train/test partitions, NO first-class signal type, NO file
  ingestion.** `DataView` exposes only opaque `partition`/`fold_id` *strings*; folds/leakage/OOF
  belong to `dag-ml`. Raw arrays are served by a host provider behind a C ABI vtable (or carried as
  `NumericFeatureMatrixF64`). No Python package yet (only a `ctypes` smoke). The nirs4all connector
  is Roadmap Phase 4 (unimplemented).

### 1.3 nirs4all `DatasetConfigs` / `DatasetLoader` (the expressiveness to match)
- Entry `DatasetConfigs` (`nirs4all/data/config.py`) accepts: dir path, `.json`/`.yaml` config file,
  config dict (with a **huge key-alias map** — partition × role synonyms in
  `data/parsers/normalizer.py`), `files`/`sources`/`variations` formats, lists (multi-dataset),
  in-memory `ndarray`/`(X,y)`/`(X,y,partition_info)`/dict-of-arrays, and prebuilt `SpectroDataset`(s).
- `ConfigNormalizer` dispatches to `VariationsParser`/`SourcesParser`/`FilesParser`/`FolderParser`
  and validates with a Pydantic `DatasetConfigSchema`. **Crucial implementation fact**: the rich
  vocabulary is all **down-converted to legacy keys** (`{train,test}_{x,y,group}` + `_filter`/`_params`
  + `global_params`), and the **runtime loader `handle_data` understands only those legacy keys**
  plus Y-by-column-index extraction. `ColumnSelector`/`RoleAssigner`/`RowSelector`/`SampleLinker`/
  `PartitionAssigner` exist and are exported but are **not wired into the loader**. Fold-file loading
  is a *pipeline* step (`{"split": path}`), not part of dataset loading.
- Loaders (`data/loaders/`): CSV (`.csv/.csv.gz/.csv.zip`), NumPy (`.npy/.npz`), Parquet
  (`.parquet/.pq` + partitioned dir), Excel (`.xlsx/.xls`), MATLAB (`.mat` v5 + v7.3), Tar/Zip, with
  registry + priority + CSV fallback. NA policies, categorical encoding, per-global/partition/file
  param precedence.
- `FolderParser` (`data/parsers/folder_parser.py`) recognizes `Xcal/Xval/Ycal/Yval`,
  `Xtrain/Xtest/Ytrain/Ytest`, `Mcal/Meta*/metadata*`, bare stems `X/Y/M`, fold files, with
  word-boundary matching for short patterns and multi-source detection (same pattern matched by
  multiple files → list). **Only CSV-family extensions are scanned.**
- The full capability set is captured as an **85-item Expressiveness Checklist** (Appendix A); it is
  the acceptance criteria for "≥ the expressiveness of DatasetConfig/Loader".

### 1.4 nirs4all-studio inference (the model for "infer with confidence")
- The studio backend (`api/datasets.py`) is a thin delegation layer; **all real inference lives in
  the `nirs4all` Python library**:
  - `FolderParser` → directory layout (no confidence; studio hardcodes `0.95`).
  - `AutoDetector`/`detect_file_parameters` (`data/detection/detector.py`) → delimiter, decimal,
    header, header_unit, signal_type, with **per-field confidence**.
  - `SignalTypeDetector`/`detect_signal_type` (`data/signal_type.py`) → rich, water-band-aware signal
    typing with a single confidence + human reason.
  - `detect_task_type` (`core/task_detection.py`) + a richer frontend `detectTargetType`
    (`src/lib/playground/targetTypeDetection.ts`).
- **There is no column-level role inference**: roles are either filename-driven (`FolderParser`) or
  user-supplied (`RoleAssigner`). The confidence model is **ad-hoc per-field magic numbers** in
  `[0,1]`; the frontend buckets them (≥0.8 green / ≥0.6 amber / else red).
- A ported, language-agnostic **heuristics catalogue (A–K)** was extracted (Appendix B): filename→role,
  header-unit classification, numeric-header=wavelengths, header presence, signal-type-from-header,
  signal-type-from-values, preprocessed-data guard, water-band tiebreaker, task/target type,
  delimiter/decimal sniffing, confidence presentation.

---

## 2. Goals, requirements, non-goals

### 2.1 Goals (from the request)
1. Rename `nirs4all-io` → `nirs4all-formats` cleanly (crates, bindings, C ABI, env vars, docs,
   provenance literals, goldens, package names).
2. New `nirs4all-io` = bridge **user files ↔ pipeline data format** (`SpectroDataset` **or**
   `dag-ml-data`).
3. **≥ the expressiveness of `DatasetConfig`/`DatasetLoader`**: directory auto-parse; X/Y/metadata
   separate or combined; train/test/folds together or separate; all population combinations; sample
   indexing by **id or row**; multi-source; etc. (Appendix A is the acceptance list.)
4. **Consume `nirs4all-formats`** as the file layer (read any supported format), and also accept
   `nirs4all-formats` outputs (records / a scanned vendor folder) as input.
5. **Inference tool**: given *any* input (formats file, directory, list of CSVs, …) infer X / Y /
   metadata / train-test / spectra-type / wavelengths / task-type **with confidence percentages**,
   to drive recommendations and automate dataset loading.
6. **Declarative conventions** (e.g. `Xval`/`Xcal` folders, `Xtrain`/`Xtest.csv`), extensible.

### 2.2 Functional requirements (condensed; see Appendix A for the full 85)
- **R-IN** Inputs: dir path; single file (tabular *or* vendor); list/glob of files; config dict/JSON/
  YAML (with alias normalization); in-memory arrays / `(X,y[,split])` / dict-of-arrays; prebuilt
  target object; a `nirs4all-formats` `RecordSet` or scanned folder.
- **R-ROLE** Role assignment at **file** granularity *and* **column** granularity (the latter is new
  vs studio).
- **R-COLSEL** Rich **column-role selectors** so X / Y / metadata / index can be **mixed and interleaved
  within one file**: by index, name, list, slice `"a:b"`, **regex** (e.g. wavelength columns), **dtype**,
  `"rest"`, or `"auto"` (inferred). (Appendix E.1.)
- **R-MERGE** Combine a multi-file `input` into one source: **`concat_samples`** (stack rows — e.g. 3
  batch CSVs), **`concat_features`** (hstack column-blocks), or **`by_key`** (relational join).
  (Appendix E.2.)
- **R-JOIN** **Relational joins** across sources by a key column with explicit **cardinality** (`1:1`,
  **`m:1`** lookup/dimension table, `1:m`) and a **coverage policy** (`complete`/`warn`/`drop`/`error`),
  incl. a keyed **`lookup`** metadata table (few rows, broadcast to many samples). (Appendix E.2, L.)
- **R-PART** Partitions train/test/val/predict via: separate files, a split column, a percentage, an
  index list, an index file, or in-memory split info.
- **R-FOLD** Folds via inline list, fold file (csv nirs4all / csv assignment / json / yaml / txt), or a
  fold/group column.
- **R-MULTI** Multi-source X (legacy list, named sources, per-source params, folder auto-detect),
  shared targets/metadata, per-source link keys.
- **R-IDX** Sample indexing by **row** or by **id column**; cross-file join by key (`link_by`).
- **R-AXIS** Wavelength/feature headers + units (nm/cm⁻¹/none/text/index) with conversion.
- **R-SIG** Signal type per source (absorbance/reflectance/…); detect or force.
- **R-PARAMS** Loading params (delimiter/decimal/header/encoding/NA/categorical/format-specific) with
  global→partition→file precedence.
- **R-INFER** Produce a `DatasetPlan` with per-decision confidence + evidence + alternatives + warnings
  + an editable resolved config.
- **R-CONV** Pluggable convention profiles; built-ins for nirs4all-classic, train/test, bare, and
  **vendor-corpus**.
- **R-OUT** Materialize to `SpectroDataset` (Python) and to `dag-ml-data` (`DatasetSchema` +
  `SampleRelationTable` + arrays + the partition/fold sidecar).

### 2.3 Non-goals (scope boundaries)
- **⛔ No modification of `nirs4all` or `nirs4all-studio`.** No edits to their code, no wiring into their
  runtimes, no deletion of their (in-prod) code. `nirs4all-io` is a *standalone externalization*; the
  later re-plug of nirs4all/studio onto it is **user-owned and out of scope here.** Allowed: read those
  repos to **copy business logic** into io, and import them **dev/test-only** as a parity oracle.
- **No runtime dependency on `nirs4all`** (would make the future re-plug circular). The only touch-point
  is a **lazy import of `SpectroDataset`** at materialization (the existing `to_spectrodataset` pattern).
- No chemometrics/modelling, no GUI (inherited from the formats non-goals).
- No new **parsers** in `nirs4all-io` — vendor byte decoding stays in `nirs4all-formats` ("parsers live
  only in the formats core"); tabular parsing logic is *copied* into io for Phase 1, then moved to Rust.
- Not (initially) a dataset **writer/exporter** to disk formats — `nirs4all-formats` is read-only;
  serialization of the *contract* (config/plan/dag-ml-data JSON) is in scope, writing vendor files is not.

---

## 3. Naming & repository topology

| Layer | Today | Proposed | Lib type |
|---|---|---|---|
| File reading (bytes → records/tables) | `nirs4all-io` (Rust) | **`nirs4all-formats`** (Rust) | read-only |
| Dataset assembly (files+config+inference → dataset) | *(in Python `nirs4all`)* | **`nirs4all-io`** (new) | assembly |
| Pipeline data contract (Rust) | `dag-ml-data` | unchanged | contract |
| Modelling + `SpectroDataset` | `nirs4all` (Python) | unchanged | consumer |
| App | `nirs4all-studio` | unchanged | consumer |

- The new `nirs4all-io` **depends on** `nirs4all-formats` (Rust crate dep) and can target
  `dag-ml-data` (Rust crate dep) and `SpectroDataset` (via its Python binding).
- **Naming caveat (see Critique C1)**: reusing `nirs4all-io` for a *different* artifact while the
  PyPI/crate name historically meant "file readers" is a real footgun; and `nirs4all-datasets`
  already exists in the workspace (collision with any "data/dataset" rename). The name is the user's
  call; this doc keeps `nirs4all-io` but flags the migration cost.

---

## 4. Target architecture

### 4.1 Layered model

```
                 ┌───────────────────────────────────────────────────────────┐
   any input ──► │                      nirs4all-io                           │
 (dir / files /  │                                                            │
  glob / config/ │  1. RESOLVE   normalize input → InputSet (refs + hints)    │
  arrays / record│  2. INFER     InputSet → DatasetPlan (+confidence)         │ ──► DatasetPlan
  set)           │  3. CONFIG    DatasetSpec (canonical, serializable IR)     │      (recommendations)
                 │  4. MATERIALIZE  DatasetSpec → target                      │
                 └───────┬───────────────────────────────┬───────────────────┘
                         │ reads via                      │ emits
                         ▼                                ▼
                 ┌───────────────┐            ┌───────────────┬───────────────┐
                 │ nirs4all-     │            │ SpectroDataset │  dag-ml-data  │
                 │ formats       │            │ (Python obj)   │  (schema +    │
                 │ (sniff/read/  │            │                │  relations +  │
                 │  describe)    │            │                │  arrays +     │
                 └───────────────┘            └───────────────┴──split sidecar)│
```

### 4.2 Responsibility split (the key boundary)

**(D2 narrowed after Codex review — formats emits *neutral evidence*, never dataset roles.)**

- **`nirs4all-formats` owns "what is this file, structurally"** — neutral, context-free facts only:
  - sniff + decode (existing), **plus** a `describe`/`probe` surface returning *without a full read*:
    format probe; **explicitly format-declared** axes/units/signal-type (vendor formats like OPUS *do*
    declare absorbance + a wavenumber axis — that stays here); discovered sidecars; table shape
    (rows/cols); **delimiter/header *candidates*** (not a verdict); per-column numeric distributions
    and dtype; content hash. Confidence here is only about *parsing* (e.g. "delimiter `;` fits 0.8"),
    never about dataset role.
  - It must **not** decide that a numeric column is X/Y/metadata/a wavelength-header/a fold-id/a
    join-key, nor infer signal type from bare values — those need dataset context.
- **`nirs4all-io` owns "how these files compose into a dataset"** — all *contextual* inference:
  - role/partition/fold/multi-source/link/column-role assignment; **signal-type inference from values**
    (when the format didn't declare it); task-type; wavelength-vs-metadata column decisions
    (Appendix B heuristics A, C, F–I);
  - the `DatasetSpec` config schema + alias normalizer + validation;
  - the `DatasetPlan` (composes formats' neutral `describe` with its own structural inference);
  - materialization to both targets.

> Rationale (revised): formats stays a *file/record* layer that reports neutral evidence + whatever the
> format itself declares; **all role/dataset-level inference lives in `nirs4all-io`**. A CSV column has
> no intrinsic role. This still removes today's duplication (one `describe` instead of Python
> `AutoDetector` + studio TS), but does not over-claim authority for the file layer.

### 4.3 Package layout (revised: Python MVP now, Rust core extraction later)

> Per the revised D1, the **MVP is the Python package below**. The Rust-core layout is the *target
> end-state* once R/WASM/`dag-ml-data` justify the extraction — not the starting point.

```
nirs4all-io/                      # new repo (sibling of nirs4all-formats)
  crates/
    nirs4all-io-core/             # DatasetSpec IR, DatasetPlan, conventions ruleset,
                                  #   inference engine, validation, alias normalizer (pure Rust)
    nirs4all-io/                  # facade: Resolve+Infer+Materialize; depends on
                                  #   nirs4all-formats (+ optional dag-ml-data feature)
    nirs4all-io-capi/             # C ABI (infer/plan/config-validate/dag-ml-data emit)
    nirs4all-io-cli/              # `infer` / `plan` / `load` / `validate` / `convert`
  bindings/
    python/   # nirs4all_io: build SpectroDataset; expose plan/config; wraps formats' Python dep
    r/  wasm/ # inference + plan in the browser / R
  conventions/                    # built-in convention profiles (TOML) + JSON schema
  docs/  samples/  tests/
```

**MVP (recommended start)** — a single **self-contained** Python package `nirs4all_io/` that imports
`nirs4all_formats` (vendor files) + carries **copied loader logic** (tabular; no nirs4all runtime dep),
and builds `SpectroDataset` via a lazy class import. The
conventions/plan/spec are Python dataclasses + a **versioned JSON schema** (the JSON schema is the
canonical, machine-validatable form; TOML is the human convention format — see D3). This JSON
schema + the inference scoring + neutral-descriptor contract are written to be **portable**, so the
later Rust-core extraction reuses them verbatim. (Trade-offs in §6, D1.)

---

## 5. Component designs

### 5.1 `DatasetSpec` — the canonical, serializable config IR

A clean superset of the legacy keys, covering Appendix A. JSON/YAML-authorable; on input, an **alias
normalizer** (porting `normalizer.py`'s partition×role synonym map) maps any of `xtrain`/`X_cal`/
`calibration_features`/… to canonical fields. Sketch:

```jsonc
{
  "name": "mango", "description": "...", "task_type": "auto|regression|binary|multiclass",
  "sample_index": { "by": "row" },                 // or {"by":"id","key":"Sample_ID"}
  "signal_type": "auto",                            // global default
  "conventions": ["nirs4all-classic"],              // profiles to apply during inference
  "sources": [                                      // multi-source X / Y / metadata, ordered
    { "id": "nir", "role": "features",
      "input": "Xcal.csv | glob | [files] | {array} | {record_set: ...}",
      "partition": "train|test|val|predict|auto",
      "columns": { "features": "2:-1", "targets": -1, "metadata": [0,1] },  // column-role (optional)
      "link_by": "Sample_ID",
      "params": { "delimiter": ";", "decimal": ".", "header": true,
                  "header_unit": "cm-1", "signal_type": "auto",
                  "encoding": "utf-8", "na": {"policy":"abort"}, "categorical": "auto" } }
  ],
  "partitions": {                                   // when a single combined input must be split
    "by": "files|column|percentage|index|index_file",
    "column": "set", "train_values": ["cal"], "test_values": ["val"],
    "train": "80%", "shuffle": true, "random_state": 0, "stratify": "y" },
  "folds": { "inline": [{"train":[...],"val":[...]}] }  // or {"file":"folds.csv"} or {"column":"cv"}
  // repetition / aggregate carried for SpectroDataset parity
}
```

Notes:
- Legacy `train_x`/`test_y`/… remain accepted on input (normalized into `sources[*]` + `partitions`).
- Column-role (`columns`) **and** `link_by` are first-class here (they exist in nirs4all only as
  unwired utilities — we wire them in).
- The spec is the **single source of truth** the materializers consume; the `DatasetPlan` produces one.

### 5.2 Convention engine

Declarative, profile-based ruleset (TOML), replacing the hardcoded `FILE_PATTERNS`. Built-in profiles:

```toml
[profile.nirs4all-classic]                 # cal/val
train_x = ["xcal","x_cal","cal_x","calx"]; test_x = ["xval","x_val","val_x","valx"]
train_y = ["ycal","y_cal","cal_y","caly"]; test_y = ["yval","y_val","val_y","valy"]
train_meta = ["mcal","metacal","metadata_cal", ...]; test_meta = [...]
folds = ["folds","fold","cv","cv_folds","splits","cross_validation"]
bare  = { x = ["x"], y = ["y"], meta = ["m","meta","metadata","group"] }

[profile.train-test]                        # sklearn-ish
train_x = ["x_train","xtrain","train_x","trainx"]; test_x = ["x_test","xtest","test_x","testx"]
train_y = ["y_train","ytrain"]; test_y = ["y_test","ytest"]

[profile.vendor-corpus]                     # NEW: folder of vendor spectra + reference table
spectra_glob = ["*.dx","*.0","*.spc","*.spa","*.sed","*.sig","*.asd", ...]  # via formats sniffing
reference    = ["y","ref","reference","targets","labels","meta","metadata"]
sample_key   = "filename_stem"              # join spectra to reference rows by stem or an id column

[match]
short_pattern_word_boundary = true          # patterns ≤2 chars need a delimiter boundary
extensions = ["any-formats-supported"]      # extension set comes from formats' registry, not hardcoded
```

- Profiles are composable and user-extensible (drop a TOML in a `conventions/` dir or pass inline).
- The `vendor-corpus` profile is the bridge's headline new capability: **N vendor files = N samples**,
  targets joined from a reference table by filename stem or id column.

### 5.3 Inference engine + `DatasetPlan` (the confidence-scored output)

`infer(input, conventions, hints) -> DatasetPlan`. The plan is the recommendation artifact and the
seed for auto-loading. Sketch:

```jsonc
{
  "input": { "kind": "directory", "path": "..." },
  "structure": { "kind": "train_test_folder|single_combined|x_y_separate|multisource_folder|vendor_corpus|in_memory",
                 "confidence": 0.92, "evidence": ["matched Xcal+Xval+Ycal+Yval (nirs4all-classic)"] },
  "assignments": [                              // file-granularity
    { "ref": "Xcal.csv.gz", "role": "features", "partition": "train", "source_index": 0,
      "confidence": 0.95, "evidence": ["filename matches train_x:xcal"],
      "alternatives": [{"role":"features","partition":"test","confidence":0.05}] }
  ],
  "columns": [                                  // column-granularity (combined files) — NEW
    { "ref": "data.csv",
      "column_roles": [ {"col":"Sample_ID","role":"metadata","confidence":0.9,"evidence":["non-numeric id-like"]},
                        {"col":"400.0","role":"feature","confidence":0.95,"evidence":["monotonic nm wavelength"]},
                        {"col":"protein","role":"target","confidence":0.7,"evidence":["low-cardinality float, last col"]} ] }
  ],
  "params":  { "Xcal.csv.gz": { "delimiter": {"value":";","confidence":0.8}, ... } },   // from formats.describe
  "axis":    { "unit": "nm", "n": 256, "range": [950,1650], "confidence": 0.95 },
  "signal_type": { "value": "absorbance", "confidence": 0.78, "reason": "water-band peaks @1450/1940nm" },
  "task_type":   { "value": "regression", "confidence": 0.8 },
  "warnings": ["'extra.csv' unassigned (no convention match)"],
  "recommendations": ["accept as train/test split; review 'protein' as target (0.70)"],
  "resolved_config": { /* DatasetSpec */ },
  "overall_confidence": 0.88
}
```

**Scoring model — call it a *score*, not a calibrated probability, until proven otherwise (revised
after Codex review).** Each decision is a hypothesis set scored by weighted evidence rules
(Appendix B); the raw score is a normalized `best / Σ` over mutually-exclusive hypotheses, mirroring
`SignalTypeDetector`. To make a surfaced "confidence %" *mean* something, the design requires (not
optional):
1. **Documented rule weights** in one versioned table, not scattered constants; with **negative
   evidence** (rules that *lower* a hypothesis), not only positive matches.
2. **A labeled fixture corpus** (`samples/inference/`) with ground-truth roles/partitions/types,
   **split by vendor/domain** (so we measure generalization, not memorization). The studio's existing
   confidences are *not* ground truth — they are partly hard-coded (`datasets.py:363`,
   `ParsingStep.tsx:48`).
3. **Calibration + reporting**: confusion matrices and precision/recall per decision, plus a
   calibration metric (**Brier / ECE**) so the number tracks real correctness; recalibrate (e.g.
   isotonic) if it doesn't.
4. **Ambiguity & abstention**: explicit ambiguity classes and an **abstain threshold** — when the top
   two hypotheses are close, the plan must say "ambiguous, please choose" rather than emit a confident
   wrong answer. Every decision carries an **evidence trace** (which rules fired, with weights).
Until 2–3 exist, label outputs as *scores* in the UI, and treat them as triage/ranking, not
probabilities (see Critique C5).

### 5.4 Resolver

Normalizes any input to an `InputSet { items: [{ref, kind, identity, hints}], origin }`. **Each item
needs a stable identity (Codex review)** — `(path | archive-member | record-index)` + a content hash +
provenance — plus **sidecar grouping** (e.g. ENVI `.img`+`.hdr`, `_gt.mat`) and **deterministic
ordering**, so plans/fingerprints are reproducible and joins are stable.
- directory → walk via `nirs4all-formats` `scan`/`walk_path` (per-file sniffed format + sidecars);
- list/glob → expand (normalize paths; handle case sensitivity + duplicate stems);
- archives (`.zip`/`.tar*`) → enumerate members as items with `archive-member` identities;
- single file → one item (tabular or vendor — formats decides);
- config dict/JSON/YAML → already a `DatasetSpec` (skip inference unless `auto` fields remain);
- arrays / `(X,y[,split])` / dict-of-arrays → in-memory items;
- a formats `RecordSet`, **or an already-constructed `SpectroDataset`** → items with attached
  records/arrays (required for true Python parity).

### 5.5 Materializers

**→ `SpectroDataset`** (Python; generalizes today's `to_spectrodataset`; builds the object via a **lazy
import of the `SpectroDataset` class** — no runtime nirs4all dep): per features source
`add_samples(X, indexes={"partition": p}, headers, header_unit)`; `add_targets(y)`;
`add_metadata(df, headers)`; `set_signal_type`, `set_task_type`, `set_folds`, `set_repetition`,
`set_aggregate*`. **Must preserve the full set of existing semantics** (Codex): train-only / test-only
paths, repetition/aggregation, task/signal detection, folds, metadata, and indexer alignment — verified
against nirs4all as a read-only **parity oracle** (Story 5.2). The MVP uses **copied loader logic** for
tabular array reading; it does not re-read bytes through formats except for vendor files.

**→ `dag-ml-data`** (Rust; behind a spike — see D4. The mapping below was **corrected after Codex
review**):
- Model identities explicitly (D6): `sample_id`, `observation_id`, `source_id`, `repetition_id` —
  `sample_ids` from the id column (`by:id`) or **synthesized stable ids** (`by:row`); row number alone
  is not a stable identity for dag-ml.
- Build `DatasetSchema`: one `SourceDescriptor` per features source; `targets`
  (`BTreeMap<TargetId, RepresentationSpec>` — multi-Y); `metadata`.
- **Axis gap (factual fix)**: `dag-ml-data`'s `AxisKind` has **no `Wavenumber`** (only `Wavelength`,
  `Frequency`, …; `model.rs:8`). cm⁻¹ spectra — the common MIR/NIR case — therefore cannot map to a
  native wavenumber axis. Options: a `feature` axis with `unit:"cm-1"` + `coordinates`, or `frequency`;
  neither is ideal. **Resolve with the dag-ml-data owners** (propose adding `Wavenumber`).
- Build `SampleRelationTable`: observation↔sample (reps), `group_id`, `origin_id` (augmentation),
  `target_id`, `excluded`. **Lossy-conversion warning**: dag-ml-data's `origin_id` is an *observation*
  id (`relation.rs:8`) while dag-ml core's relation uses `origin_sample_id` (`dag-ml/.../relation.rs:17`);
  the observation→sample collapse loses information when several observations share a sample. Carry
  both ids and validate.
- **Folds/partitions do NOT live in `dag-ml-data`** and are **not** passed as `DataView.partition`
  strings (the previous draft was wrong — dag-ml-data filtering ignores those fields, `handle.rs:657`).
  `dag-ml` derives train/validation/predict sample-ids from **campaign split state**
  (`FoldTrain/FoldValidation/FullTrain/Predict`, `dag-ml/.../data.rs:35`). To feed dag-ml the bridge
  must emit a real **`CampaignSpec`/`FoldSet` + `DataBinding` + fingerprints**, and the contract dag-ml
  actually consumes is **`ExternalDataPlanEnvelope`** (`dag-ml/.../data.rs:660`), *not* dag-ml-data's
  `CoordinatorDataPlanEnvelope` (`coordinator.rs:60`). Validate via `dag-ml validate-data-binding` in
  the spike. Signal type → `SourceDescriptor.tags`; arrays via `NumericFeatureMatrixF64` / the C ABI
  host provider.

---

## 6. Key decisions (resolved with the user 2026-05-27)

### D1 — Language of the new `nirs4all-io`  *(primary — DECIDED)*
- **Option A — Rust-first full core + Python/R/WASM bindings.** Pros: ecosystem consistency (formats,
  dag-ml-data, dag-ml are all Rust + the DIRECTIONS ADR); one source of truth for conventions/spec/
  inference across Python, R, **browser WASM**, CLI; native `dag-ml-data` emission. Cons: largest
  up-front effort; raises the contribution bar. **Codex correction**: the earlier claim that this ports
  "only ~2 pages of decision logic, not the loaders" is **false** — matching current behavior also
  requires reproducing NA handling, categorical metadata, per-level param precedence, archives, and the
  `SpectroDataset` indexer semantics, and "route everything through formats" hides a large
  *formats* tabular-reader rewrite (`csv_like.rs` hardcodes `nm`/`Absorbance`, requires numeric
  headers). So Option A is a **rewrite, not a port**.
- **Option B — Python-first single package.** Pros: fastest; the mature business logic is already Python;
  near-zero friction to `SpectroDataset` (lazy import). Cons: odd-one-out in a Rust ecosystem; cannot
  itself serve R/WASM; feeding `dag-ml-data` needs FFI/ctypes.
- **Option C — Hybrid: Rust for neutral file descriptors + `dag-ml-data` schema pieces; Python for the
  copied loader/inference logic + `SpectroDataset`.**
- **DECISION (confirmed): B → C, in two phases.** **Phase 1** = Python MVP, `SpectroDataset`-compatible,
  built by **copying the business logic out of nirs4all/studio into a self-contained `nirs4all-io` and
  re-orchestrating it** (Appendix D) — **without touching nirs4all/studio** (D5) and with **no runtime
  dependency on nirs4all** (only a lazy `SpectroDataset` import at materialization). **Phase 2** = a Rust
  MVP, `dag-ml-data`-compatible, soon after — *translate the copied Python logic to Rust*; the portable
  contracts (DatasetSpec JSON schema, scoring rules, neutral-descriptor interface) are authored in
  Phase 1 so the Rust core reuses them. Rust-first-now (A) is rejected: it optimizes for bindings before
  semantic parity is proven, and the mature code + the primary target both live in Python.

### D2 — Where does byte/value-level detection live? *(narrowed after Codex review)*
Recommendation: **`nirs4all-formats` emits only *neutral* file evidence** via `describe`/`probe` —
format probe, format-*declared* axes/units/signal-type, sidecars, table shape, delimiter/header
*candidates*, per-column numeric distributions, content hash. **All role/dataset-level inference —
incl. signal-type inference from bare values, and wavelength-vs-metadata column decisions — stays in
`nirs4all-io`** (it needs dataset context). This still removes today's duplication (one `describe`
instead of Python `AutoDetector` + studio TS) without letting the file layer over-claim authority.

### D3 — Conventions format *(refined after Codex review)*
Recommendation: **a versioned, machine-validatable JSON schema is the canonical form; TOML is the
human-authoring convenience.** Profiles are composable + user-extensible; the extension set comes from
the formats registry (not CSV-only). Ship `nirs4all-classic`, `train-test`, `bare`, `vendor-corpus`.
The engine must also model sidecar grouping, archives, case sensitivity, duplicate stems, path
normalization, multiple-spectra-per-sample, multiple targets, and a **dropped-row audit** (see C8).

### D4 — `dag-ml-data` target *(DECIDED: Phase 2, planned now, implemented after the lib is ready)*
dag-ml-data is `0.1.0-alpha`, no Python package, no nirs4all connector, lacks folds/partitions/signal-
type, and (corrected) `dag-ml` consumes `ExternalDataPlanEnvelope` + `DataBinding` + campaign
`FoldSet`, not dag-ml-data's `CoordinatorDataPlanEnvelope`. **DECISION**: the mapping is **fully planned
now** (Appendix H.2) and a **readiness gate** lists what dag-ml-data must ship first (Appendix J);
implementation starts when that gate is green and a spike passes real `dag-ml validate-data-binding`.
Co-design the missing `Wavenumber` axis + `origin_id`/`origin_sample_id` mapping with the dag-ml owners.

### D5 — Relationship to `nirs4all` / `nirs4all-studio` *(DECIDED 2026-05-27: clean externalization, no modification)*
**Superseded by the user's hard constraint.** This effort **does not modify, wire into, or delete any
code in `nirs4all` or `nirs4all-studio`** — they stay in production exactly as-is. `nirs4all-io` is a
**from-scratch externalization** that *copies their business logic* and re-orchestrates it (the originals
"lack recul", so io is free to re-architect). The previously-proposed "delete the duplicated inference in
nirs4all/studio" is **removed** from this scope. The **re-plug** (refactoring nirs4all/studio — likely
the studio first — to call `nirs4all-io`) is a **separate, user-owned effort** with its own planning.
Consequences: (a) intentional, indefinite duplication during the externalization (managed via a
read-only parity oracle, not deletion — Critique C6); (b) `nirs4all-io` keeps **no runtime dependency on
nirs4all** so the future re-plug is not circular; (c) our deliverable includes a clean, documented
**integration seam** (stable `infer`/`load` API + JSON spec/plan) so the user can re-plug later cheaply.

### D6 — Sample identity *(revised after Codex review)*
Support `by:row` defaults for `SpectroDataset` parity, **but model `sample_id`, `observation_id`,
`source_id`, and `repetition_id` explicitly** in the spec and the resolver. For `dag-ml-data`/`dag-ml`,
a row number is not a stable identity; reps/groups/augmentation joins all need real ids.

---

## 7. Rename plan (Phase 0) — `nirs4all-io` → `nirs4all-formats`

Scoped from the rename audit. Do on a dedicated branch; the tree currently has unrelated WIP
(felix_f750) — commit/stash it first. **Sizing (corrected after Codex review): this is L/XL, not a
mechanical M** — see steps 1, 10, 11 below for the under-counted surface.

1. **Crates** (**8 `Cargo.toml`, not 5**: root workspace + 4 crates + `bindings/python` +
   `bindings/wasm` + `bindings/r/.../src/rust`, plus `[workspace.dependencies]` path deps):
   `nirs4all-io-core→nirs4all-formats-core`, `nirs4all-io→nirs4all-formats`,
   `nirs4all-io-capi→nirs4all-formats-capi`, `nirs4all-io-cli→nirs4all-formats-cli` (+ `[[bin]] name`).
   Rename the 4 crate directories.
2. **Rust import paths**: `use nirs4all_io::` → `use nirs4all_formats::` (~290 sites; mechanical).
3. **Reader provenance literals** (42): `"nirs4all_io::readers::<fmt>"` → `"nirs4all_formats::readers::<fmt>"`.
4. **Goldens** (231 files, ~103k occurrences): **do not hand-edit** — regenerate via the (renamed)
   accept env: `NIRS4ALL_FORMATS_ACCEPT_GOLDENS=1 cargo test -p nirs4all-formats --test goldens`.
5. **C ABI**: symbol prefix `n4io_`→`n4fmt_`, macros `N4IO_`→`N4FMT_`, header guard `NIRS4ALL_IO_H`→
   `NIRS4ALL_FORMATS_H`, `cbindgen.toml`, generated `include/nirs4all_formats.h`, the C example.
6. **Env vars**: `NIRS4ALL_IO_CLI/REPO/ACCEPT_GOLDENS/RUN_LOCAL` → `NIRS4ALL_FORMATS_*`.
7. **Python**: dist `nirs4all-io`→`nirs4all-formats`, module `nirs4all_io`→`nirs4all_formats`,
   PyO3 `_native`, package dir, **and the `_RESERVED = "nirs4all_io."` metadata prefix** →
   `nirs4all_formats.` (note: this prefix is written into `SpectroDataset`/bundle metadata — see C2).
8. **R**: package `nirs4allio`→`nirs4allformats`, all `nirs4allio_*` functions + S3 class + `man/*.Rd`.
9. **WASM**: crate/lib `nirs4all-io-wasm`/`nirs4all_io_wasm` → `*_formats_*`; regenerate `pkg*`.
10. **CI/release + docs build (under-counted — Codex)**: `.github/workflows/*` (esp. `release.yml`
    artifact names ~`:80`/`:128`: wheels, sdist, per-OS C ABI archive + header, R tarball), the
    generated WASM `pkg*` binding names, Sphinx docs build outputs, and conformance/reverse-lab helper
    strings (`tools/reverse-lab`, `known_skips.toml`/`tolerances.toml` references).
11. **Repo/URLs/docs**: `github.com/GBeurier/nirs4all-io`→`nirs4all-formats`; docs prose; `README`,
    `CLAUDE.md`, `AGENTS.md`, `STATUS.md`.
12. **Publishing + registry implications (C1)**: release `nirs4all-formats` as a **new** PyPI/crate name
    (major bump); decide whether to publish a final `nirs4all-io` shim that errors with a "moved/renamed"
    message, *before* the new library reclaims the `nirs4all-io` name; coordinate so the reused name does
    not silently install a different package. Decide on transitional compatibility aliases (or none, per
    the no-backward-compat rule) explicitly.
13. **Green gate** must pass end-to-end after the rename (fmt/clippy/test/no-default-features/wasm/
    bindings/docs).

---

## 8. Backlog (epics → stories, ordered)

Legend: `[S]`mall / `[M]`edium / `[L]`arge / `[XL]`. Acceptance criteria abbreviated. **`[copy-logic:…]`**
= copy the *business logic* out of nirs4all/studio into a self-contained `nirs4all-io` and re-orchestrate
(Appendix D), **without touching the originals**.

> **Two phases (user-confirmed) + copy-and-re-orchestrate.** Story numbers are stable (cross-referenced
> elsewhere); the **phase tags** below group them. **⛔ No story edits `nirs4all` or `nirs4all-studio`.**
> - **Phase 0** — rename (EPIC 0).
> - **Phase 1 — Python MVP, `SpectroDataset`** (EPIC 2 core IR, EPIC 3 resolve+infer, EPIC 4.1/4.2
>   materialize via *copied loader logic*, EPIC 5 integration *seam* only). Built **copy-first** per
>   Appendix D. `formats.describe` (EPIC 1) is **not** a P1 blocker — P1 uses the `detector.py` logic
>   copied into `nirs4all-io`.
> - **Phase 2 — Rust MVP, `dag-ml-data`** (EPIC 1 `describe` ported to Rust, the Rust-core extraction
>   gated by 6.4, EPIC 4.3 spike + 4.4 emit). **Blocked by the Appendix J readiness gate** on the
>   `dag-ml-data` lib.
> - **Out of scope** (user-owned, later): re-plugging nirs4all/studio onto `nirs4all-io` + deleting their
>   duplicated code.
> - **Cross-cutting**: EPIC 6 (hardening) + the read-only parity oracle (Story 5.2).
> Rename and Epics 1–4 skew **larger** than the first sizing.

### EPIC 0 — Rename to `nirs4all-formats` *(unblocks the name)* — overall `[L/XL]`
- 0.0 `[S]` **Rename audit + migration/registry plan**: full identifier inventory (8 Cargo.toml,
  CI/release artifacts, R/WASM/Python names, env vars, `_RESERVED` prefix, goldens), the PyPI/crate
  reuse decision (C1), and the compatibility-alias decision. *AC*: a checklist + go/no-go on name reuse.
- 0.1 `[M]` Rename crates + dirs + path deps; `cargo build --workspace` green.
- 0.2 `[S]` Rewrite Rust import paths + 42 provenance literals; regenerate goldens; `cargo test` green.
- 0.3 `[M]` Rename C ABI prefix/macros/header + cbindgen + env vars; C example builds; header regenerated.
- 0.4 `[M]` Rename Python/R/WASM packages incl. `_RESERVED` prefix; per-binding tests green.
- 0.5 `[S]` Docs/URLs/branding; green gate; publish `nirs4all-formats` (new name, major bump).
  *AC*: full green gate; `pip install nirs4all-formats` + `cargo add nirs4all-formats` work; old name documented as moved.

### EPIC 1 — `nirs4all-formats` `describe` surface (D2) *(Phase 2: Rust port; in Phase 1 the same logic is the lifted Python `detector.py` living inside `nirs4all-io`)*
- 1.1 `[M]` `describe_path/bytes` returning detected params (delimiter/decimal/header), value stats,
  row/col counts, per-field confidence — port heuristics B, D, J. *AC*: matches current
  `detect_file_parameters` on a fixture set.
- 1.2 `[M]` Wavelength-header + axis detection (heuristic C) and signal-type detection (E–H, incl.
  water-band) with confidence + reason. *AC*: parity with `SignalTypeDetector` on labeled spectra.
- 1.3 `[S]` Expose `describe` through C ABI + Python/R/WASM. *AC*: a **WASM host/demo** (in io's own
  examples) can call it — *not* studio (studio is untouched; its later adoption is user-owned).

### EPIC 2 — `nirs4all-io` core IR (D1, D3) *(Phase 1 — the canonical contract; do first)*
- 2.0 `[M]` **Versioned schema + id/join semantics + migration aliases**: `schema_version`, explicit
  `sample_id`/`observation_id`/`source_id`/`repetition_id` model (D6), and the join-key semantics for
  cross-file/vendor-corpus linking. *AC*: schema is versioned; id model documented + validated.
- 2.1 `[L]` `DatasetSpec` IR (Appendix E, incl. **column selectors E.1 + merge/join E.2**) + JSON/YAML
  (de)serialization + validation. **`[copy-logic: lift nirs4all data/schema/config.py —
  DatasetConfigSchema/FileConfig/SourceConfig/PartitionConfig/FoldConfig/LoadingParams — into io;
  complete the FileConfig/ColumnConfig stubs; add merge/join/cardinality/lookup]`** *AC*: round-trips;
  rejects invalid specs with clear errors.
- 2.2 `[M]` Alias normalizer. **`[copy-logic: copy data/parsers/normalizer.py partition×role synonym map]`**
  *AC*: the alias table in Appendix A.4 all map to canonical fields (table-driven test).
- 2.3 `[M]` Convention engine + built-in profiles (`nirs4all-classic`, `train-test`, `bare`,
  `vendor-corpus`) — Appendix G; user profiles loadable. **`[copy-logic/generalize: data/parsers/
  folder_parser.py FILE_PATTERNS → declarative profiles; extend the extension set via the formats
  registry]`** *AC*: reproduces `FolderParser` matches on its fixtures **and** matches a vendor-corpus
  fixture.
- 2.4 `[M]` **Vocabulary reference + use-case cookbook** (Appendices E.1/E.2 + L): document **every**
  declaration element (roles, column selectors, merge modes, join cardinality + coverage, lookup tables,
  partitions, folds, params) with ≥1 runnable example + a fixture. **Gate: every vocabulary element has
  ≥1 cookbook entry + a passing fixture** — *undocumented vocabulary is treated as unshipped* (the user's
  adoption bar: "sinon ce ne sera pas utilisé").

### EPIC 3 — Resolver + Inference + `DatasetPlan` (R-INFER) *(Phase 1; full spec in Appendix F)*
- 3.1 `[M]` Resolver for all input kinds → `InputSet` (stable identity + hash + sidecar grouping).
  **`[reuse: extend data/config_parser.py browse + formats scan/walk_path]`** *AC*: each R-IN input kind
  resolves.
- 3.2 `[L]` Structural inference (file-granularity roles/partitions/folds/multi-source) composing
  conventions + `describe`; emit `DatasetPlan` with score/evidence/alternatives. **`[copy-logic: lift the
  detect-unified orchestration approach from studio api/datasets.py (design reference) + FolderParser
  logic into io; do NOT modify studio]`**
- 3.3 `[L]` **Column-granularity** role inference (X/Y/metadata within a combined file) — new vs studio.
  **`[copy-logic: lift the algorithms from the unwired data/selection/ RoleAssigner/ColumnSelector/
  RowSelector + detector.py wavelength-header (C) + core/task_detection.py (I) into io's clean
  orchestration — do NOT wire into nirs4all]`** *AC*: correct column roles on combined-file fixtures.
- 3.4 `[M]` Labeled inference corpus `samples/inference/` (**split by vendor/domain**) + precision/recall
  test + documented rule weights with **negative evidence** (Critique C5). **`[reuse: seed weights from
  detector.py/signal_type.py thresholds, then recalibrate — studio numbers are not ground truth]`** *AC*:
  role/partition/signal-type precision ≥ target on a held-out split.
- 3.5 `[S]` `infer` CLI + plan JSON schema. *AC*: `nirs4all-io infer DIR --json` emits a valid plan.
- 3.6 `[M]` **Calibration + abstention**: Brier/ECE report, ambiguity classes, abstain threshold,
  evidence traces; UI labels outputs as *scores* until calibrated. *AC*: calibration report produced;
  ambiguous cases abstain instead of guessing.

### EPIC 4 — Materialization *(Phase 1: 4.0/4.1a/4.1b/4.2 · Phase 2: 4.3/4.4; mappings in Appendix H)*
- 4.0 `[L]` **Relational join/merge engine** (Codex; do **before** the SpectroDataset builder): implement
  `concat_samples`/`concat_features`/`by_key`/`none` + cross-source joins with **cardinality validation**,
  **duplicate-key policy**, **coverage** (complete/warn/drop/error), **m:1 broadcast honoring lookup
  roles**, composite/virtual keys, and a **dropped-row audit**. **`[copy-logic: SampleLinker + the join
  semantics; generalize]`** *AC*: every E.2 behavior covered by L.4–L.8/L.14/L.18 fixtures.
- 4.1a `[L]` **Loader-copy**: copy the tabular loader logic (CSV/Parquet/Excel/MATLAB/archive + NA +
  categorical + param precedence) into io (no nirs4all import); vendor files via formats. **`[copy-logic:
  data/loaders/*]`** *AC*: reads every App. A.2 format; parity-oracle clean (run continuously — Codex).
- 4.1b `[M]` **SpectroDataset builder**: assemble loader+join outputs into a `SpectroDataset` via a **lazy
  class import** (treat nirs4all as an *optional target dependency* — its `data/__init__` is heavy);
  cover multi-source, folds, repetition/aggregate, signal/task type. *AC*: Expressiveness Checklist
  (App. A) matches `DatasetConfigs` on the dev-only oracle.
- 4.2 `[M]` Loading-params precedence (global→partition→file) + NA/categorical. **`[copy-logic:
  apply_na_policy + get_effective_params from data/loaders/base.py]`** *AC*: parity on the oracle.
- 4.3 `[S]` **dag-ml contract spike (Phase 2 gate; needs Appendix J green)**: hand-author a
  `CampaignSpec`/`FoldSet`/`DataBinding`/`ExternalDataPlanEnvelope` and pass `dag-ml
  validate-data-binding`; resolve the missing `Wavenumber` axis + `origin_id`/`origin_sample_id` mapping
  with the dag-ml owners. *AC*: a real binding validates; mapping gaps have agreed resolutions.
- 4.4 `[L]` `DatasetSpec` → `dag-ml-data` (`DatasetSchema` + `SampleRelationTable` + fingerprints) and
  the `dag-ml` campaign artifacts (`FoldSet`/`DataBinding`/`ExternalDataPlanEnvelope`) per Appendix H.2;
  signal type → `tags`; arrays via `NumericFeatureMatrixF64`/vtable. *AC*: emitted contract validates in
  `dag-ml` end-to-end (not just dag-ml-data schema validation).
- 4.5 `[M]` **Provenance + resource/security limits**: carry content hashes/provenance into both
  targets; bound memory/file-count/recursion on untrusted inputs and archives. *AC*: large/adversarial
  archives don't OOM; provenance round-trips.

### EPIC 5 — Integration *seam* (NOT integration; D5) *(Phase 1 — we do NOT touch nirs4all/studio)*
> The actual re-plug of nirs4all/studio onto `nirs4all-io` is **out of scope / user-owned** (D5). This
> epic only makes the seam clean so that re-plug is cheap later — **no edits to nirs4all or studio.**
- 5.1 `[M]` **Stable integration seam**: a documented public API (`infer`/`load`/`DatasetSpec`/
  `DatasetPlan`, Appendix I) + a versioned JSON spec/plan contract, designed so nirs4all could later do
  `dataset=<spec|plan|nirs4all-io result>` and studio could call `infer`/`load`. *AC*: API + JSON schema
  frozen + documented; example showing how a host *would* call it (in io's own examples, not in
  nirs4all/studio).
- 5.2 `[S]` **Parity oracle (read-only)** — *land early (with 4.1a) and run continuously* (Codex): a
  dev/test harness that runs nirs4all's `DatasetConfigs` + studio detection on the fixtures and diffs them
  against io's output — **imports nirs4all read-only, no edits**. *AC*: reports parity/drift; CI dev-only.
- 5.3 `[S]` **Re-plug guide (doc only)**: document the seam + a recommended re-plug sequence (studio
  first) for the user's separate effort. *AC*: a `docs/REPLUG.md` in `nirs4all-io` — no changes to
  nirs4all/studio.
- 5.4 `[S]` **Copy-provenance + license clearance (Codex; see C12)**: a manifest of every block of logic
  copied from nirs4all/studio (source path → io module) + resolve the **CeCILL-2.1 → MIT** license
  question (relicense io portions, dual-license, or owner waiver — GBeurier owns both). *AC*: provenance
  manifest exists; io's license is consistent and documented before any copied code ships.
- *(Deletion of duplicated code in nirs4all/studio is intentionally **not** in this backlog — it belongs
  to the user's later re-plug.)*

### EPIC 6 — Hardening *(cross-cutting)*
- 6.1 `[M]` Adversarial inputs (ambiguous folders, mixed formats, unaligned row counts, missing
  targets) → graceful plan warnings, not crashes.
- 6.2 `[S]` Performance pass on large vendor corpora (thousands of files); parallel scan.
- 6.3 `[S]` Conformance: plan/spec round-trip + sidecar/archive grouping fixtures (C8).
- 6.4 `[M]` **Cross-language golden tests** (Python/R/WASM emit identical plans) — gate for the Rust-core
  extraction; until then, asserts the portable JSON-schema/scoring contract.
- 6.5 `[S]` **Import-boundary CI (Codex)**: assert `import nirs4all_io` does **not** import `nirs4all`;
  only `load(..., target="spectrodataset")` may lazy-import it (and `nirs4all.data.__init__` is heavy, so
  keep it lazy). *AC*: a test fails if `nirs4all` appears in `sys.modules` after `import nirs4all_io`.

**Order — Phase 0** 0.0→0.5 (rename) · **Phase 1 (Python MVP, copy-into-io)** 2.0/2.1/2.2/**2.4**
(spec+ids+vocabulary docs, *copy schema + normalizer*) → 2.3 (conventions) → 3.1/3.2/3.3 (resolve+infer,
*copy selection-util logic*) → **4.0 (join/merge engine) → 4.1a (loader-copy) + 5.2 (parity oracle, runs
continuously) → 4.1b (SpectroDataset builder) → 4.2** → 3.4/3.6 (corpus+calibration) → 5.1/5.3/5.4 (seam,
re-plug guide, license) → 6.5 (import-boundary) · **Phase 2 (Rust/dag-ml-data, after Appendix J)** 1.*
(`describe`→Rust) → 6.4 (cross-lang gate) → 4.3 spike → 4.4 (`dag-ml`) · **6** hardening throughout.
**(Re-plug + deletion in nirs4all/studio = out of scope.)**
**Phase-1 MVP** = 0, 2.0–2.4, 3.1/3.2/3.3, **4.0/4.1a/4.1b/4.2**, 5.1/5.2/5.4 (folder/files/**vendor-corpus**
+ mixed-column/lookup declarations → SpectroDataset, scored `DatasetPlan`), **self-contained** (copied
logic, no runtime nirs4all dep) — no Rust core, no `dag-ml-data` yet. **2.4 (vocabulary + cookbook docs)
is part of the MVP gate** — adoption depends on it.

---

## 9. Critique of the project (risks & pushback)

- **C1 — Name reuse is a footgun.** `nirs4all-io` historically means "file readers"; after the swap,
  `pip install nirs4all-io` silently resolves to a *different* library. Plus `nirs4all-datasets`
  already exists (collision pressure on any "data" rename). *Mitigation*: major version bump + a clear
  "moved to `nirs4all-formats`" release + deprecation note on the old PyPI page; consider an alternative
  name for the new layer (e.g. `nirs4all-assemble`, `nirs4all-loader`) if churn is unacceptable. **This
  is the user's call but should be made consciously.**
- **C2 — Persisted-prefix change.** The `_RESERVED = "nirs4all_io."` metadata prefix is written into
  `SpectroDataset`/bundle metadata. Renaming it to `nirs4all_formats.` is correct per "no backward
  compat", but any *already-exported* bundles carry the old prefix. Decide: re-export, or a one-time
  read-side rename. Document it.
- **C3 — Expressiveness trap.** "All possible combinations" of `DatasetConfig` is a combinatorial spec,
  and a large slice (variations modes, `sources` format, shared targets, column-split `PartitionConfig`,
  `link_by`) is **not even wired into the nirs4all runtime today**. Re-implementing all of it risks
  building a big surface that mostly duplicates *unused* sugar. *Mitigation*: implement the
  runtime-effective subset first (legacy keys + folder conventions + folds + multi-source + the
  genuinely-new column-role/vendor-corpus), and defer exotic config sugar until there is a consumer.
- **C4 — `dag-ml-data` impedance mismatch (sharpened by Codex; the original mapping was wrong).** It
  has no folds, no train/test, no signal type, and no file ingestion; `0.1.0-alpha`, no Python package,
  no nirs4all connector. **Corrections**: (a) `dag-ml` consumes `ExternalDataPlanEnvelope` +
  `DataBinding` + campaign `FoldSet`, **not** dag-ml-data's `CoordinatorDataPlanEnvelope`; (b)
  partitions/folds are **not** `DataView.partition` strings — those are ignored by dag-ml-data filtering
  (`handle.rs:657`) — they come from campaign split state; (c) `AxisKind` has **no `Wavenumber`**, so
  cm⁻¹ spectra have no native axis; (d) `origin_id` (observation) vs `origin_sample_id` (sample) is a
  lossy conversion under repetitions. *Mitigation*: D4 — design now, ship SpectroDataset first, and
  **gate any dag-ml claim behind a spike that passes real `dag-ml validate-data-binding`**; co-design
  the axis + relation gaps with the dag-ml owners.
- **C5 — "Confidence %" is a *score*, not a calibrated probability.** The current numbers are hand-tuned
  constants; the studio's are partly hard-coded (`datasets.py:363`, `ParsingStep.tsx:48`) so they are
  **not ground truth**. Surfacing them as percentages overclaims rigor. *Mitigation*: documented
  rule-weight table **with negative evidence**, a vendor/domain-split labeled corpus, precision/recall +
  **Brier/ECE calibration**, ambiguity classes, and an **abstain threshold** (Stories 3.4, 3.6). Until
  calibrated, label outputs as *scores* and use them only for ranking/triage.
- **C6 — Intentional, indefinite duplication (revised per the no-touch constraint).** Because we do
  **not** modify or delete nirs4all/studio (D5), io and the in-prod stacks coexist *by design* until the
  user's separate re-plug. The real risk is **drift** (io and nirs4all diverging in behavior).
  *Mitigation*: the read-only **parity oracle** (Story 5.2) runs both on the fixtures in CI and flags
  divergence — duplication is accepted, drift is not. Deletion is explicitly *not* our concern.
- **C7 — Rust-port maintainability.** If contributors are Python-centric, a Rust-first `nirs4all-io`
  raises the bar (the formats repo already notes "stricter API discipline" for reverse-engineered
  parsers). *Mitigation*: keep io's Rust core small (decision logic only), push byte parsing to formats,
  and invest in the Python/R/WASM binding ergonomics.
- **C8 — Vendor-corpus join is genuinely hard.** Joining N vendor files to a reference table by filename
  stem vs id column is the most error-prone new path, and (Codex) must also handle sidecar grouping,
  archives, case sensitivity, duplicate stems, path normalization, multiple-spectra-per-sample, and
  multiple targets. *Mitigation*: explicit `sample_key` semantics in the `vendor-corpus` profile;
  strong plan warnings + a **dropped-row audit report** on ambiguous/partial joins; adversarial fixtures
  (6.3).
- **C9 — Scope creep into modelling.** The bridge must resist absorbing splitter/CV logic (that's
  nirs4all/dag-ml). Keep folds as *data the user provides or a column*, not as a CV algorithm.
- **C10 — Copying the loader logic is real work (Phase 1) and a Rust rewrite (Phase 2).** Because io is
  self-contained (no nirs4all runtime dep), Phase 1 must **copy** nirs4all's loader logic (CSV/Parquet/
  Excel/MATLAB/archive + NA + categorical + param precedence) into io and re-orchestrate it — cheaper
  than a fresh design, but not free; the immature Rust `csv_like.rs` (hardcoded `nm`/`Absorbance`,
  numeric-header requirement) is **not** a shortcut. *Mitigation*: Phase 1 copies the Python logic (the
  algorithms are well understood); Phase 2 translates it to Rust; the parity oracle (5.2) guards
  fidelity. Vendor files always go through `nirs4all-formats`.
- **C11 — Resolver identity/provenance is foundational, not incidental.** Reproducible plans,
  fingerprints, and stable joins require per-item identity (path/member/record) + content hash +
  deterministic ordering from day one (Story 3.1); retrofitting it later breaks fingerprints.
- **C12 — License: copying CeCILL-2.1 logic into an MIT lib is a real constraint (Codex).** `nirs4all` is
  **CeCILL-2.1**; `nirs4all-formats` is **MIT**. "Copy the business logic" therefore crosses a license
  boundary — copied loader/detector/selection code cannot silently become MIT. Since **GBeurier owns
  both**, options: relicense the copied io portions, dual-license, or an owner waiver — but it must be a
  **conscious, documented decision before any copied code ships** (Story 5.4), with a copy-provenance
  manifest (source → io module). Do not assume MIT for `nirs4all-io` by default.
- **C13 — Relational joins are a real engine, not config sugar (Codex).** m:1 broadcast, cardinality +
  duplicate-key validation, coverage audits, composite/virtual keys, and role-honoring lookups are a
  dedicated component (Story 4.0) that **io owns** — `dag-ml-data` does not join (E.2). Under-scoping it
  was the main gap in the cookbook's first draft.

---

## 10. Resolved decisions (user, 2026-05-27)
0. **⛔ Scope**: ✅ **Do NOT touch `nirs4all` or `nirs4all-studio`.** `nirs4all-io` is a from-scratch
   externalization that *copies* their business logic and re-orchestrates it; io stays self-contained
   (no runtime nirs4all dep; `SpectroDataset` via lazy import only). The **re-plug** of nirs4all/studio
   onto io (likely studio first) is a **separate user-owned effort — out of scope here.**
1. **D1**: ✅ **Phase 1 Python MVP (`SpectroDataset`), copy-and-re-orchestrate → Phase 2 Rust MVP
   (`dag-ml-data`), soon after.** Not Rust-first-now.
2. **Name**: ✅ **Keep `nirs4all-io`** for the new layer (rename the reader to `nirs4all-formats`). The
   C1 churn/collision risk is accepted and handled via the migration step (§7.12); a final "moved"
   shim on the old `nirs4all-io` package name is published before the new lib reclaims it.
3. **D4**: ✅ **`dag-ml-data` planned now (Appendix H.2), implemented in Phase 2 once the lib is ready
   (Appendix J gate) + the `dag-ml validate-data-binding` spike passes.** Co-design `Wavenumber` +
   relation mapping with the dag-ml owners.
4. **D5**: ✅ **No modification or deletion of nirs4all/studio code** in this effort (superseded by
   item 0). Drift is managed by a read-only **parity oracle**, not by deleting their code.
5. **Doc home**: stays in `nirs4all-formats/docs/`; move to the new `nirs4all-io` repo once it exists.
6. **Language**: English (matches all repo docs).

> Remaining open items are not blockers: exact `FoldSet`/`CampaignSpec` field-level mapping (nailed by
> the Phase-2 spike, Story P2.1) and the `Wavenumber`-axis resolution (co-design with dag-ml owners).

---

## Appendix A — Expressiveness Checklist (acceptance criteria)

The full 85-item checklist distilled from `DatasetConfigs`/`DatasetLoader`/`ConfigNormalizer`/
`FolderParser`/`SpectroDataset`. Grouped; each item is a capability the new `nirs4all-io` must match.

**A.1 Input container forms**: (1) folder path; (2) `.json`/`.yaml` config; (3) dict with canonical
keys; (4) dict with **any alias**; (5) dict with `folder`; (6) `files:[...]`; (7) `sources:[...]`;
(8) `variations:[...]` (+mode/select/prefix); (9) list (multi-dataset); (10) ndarray X-only→test;
(11) `(X,y)`→train; (12) `(X,y,partition_info)` int/slice/index; (13) dict-of-arrays;
(14) prebuilt `SpectroDataset`/list. *(+ new: a formats `RecordSet` / scanned vendor folder.)*

**A.2 File formats**: (15) CSV; (16) gzip CSV/`.gz`; (17) zip CSV/`.zip`(+member); (18) `.npy`;
(19) `.npz`(+key); (20) Parquet (+columns/filters); (21) Parquet dir; (22) Excel (+sheet/usecols/
skip); (23) MATLAB v5 + v7.3 (+variable); (24) Tar family (+member); (25) pluggable loaders.
*(In the new design these are provided by `nirs4all-formats`, which also adds the vendor formats.)*

**A.3 Folder conventions**: (26) `Xcal/Xval/Ycal/Yval`; (27) `Xtrain/Xtest/Ytrain/Ytest` + orderings;
(28) metadata `Mcal/Mtrain/Meta*/metadata*`; (29) bare `X/Y/M`; (30) fold files; (31) multi-source
auto-detect; (32) word-boundary for short patterns.

**A.4 X/Y/metadata population**: (33) three separate files; (34) Y as X-columns via `_y_filter`;
(35) Y multi-column file with sub-select; (36) multiple targets; (37) classification auto-encode +
categorical mode; (38) metadata never NaN-dropped; (39) row-count alignment check.

**A.5 Train/test/predict**: (40) separate files; (41) predict partition; (42) split by column;
(43) percentage (+shuffle/random_state/stratify); (44) index-list / index-file; (45) in-memory
partition_info.

**A.6 Multi-source**: (46) legacy list (source 0 owns Y/meta); (47) per-source params list / broadcast;
(48) named `sources`; (49) shared targets/metadata; (50) folder multi-source.

**A.7 Variations**: (51) separate; (52) concat (+prefix); (53) select; (54) compare; (55) per-variation
provenance.

**A.8 Folds**: (56) inline list; (57) fold file (csv-nirs4all / csv-assignment / json / yaml / txt);
(58) by column; (59) in-memory `set_folds`; (60) single-fold→val-to-test.

**A.9 Sample indexing**: (61) positional (default); (62) by-id `repetition` grouping; (63) cross-file
`link_by`.

**A.10 Headers/wavelengths**: (64) header from CSV row; (65) `header_unit` nm/cm⁻¹/none/text/index +
conversion; (66) generated headers for npy/mat/ndarray.

**A.11 Signal type**: (67) per-file/source type; (68) constructor force; (69) auto-detect.

**A.12 Loading params (precedence file>partition>global)**: (70) delimiter/decimal/header; (71) CSV
auto-detect; (72) encoding (+latin-1); (73) NA policy + fill config; (74) categorical mode; (75) root
shorthand; (76) format-specific params.

**A.13 Task/aggregation/meta**: (77) task_type (auto/regression/binary/multiclass, force); (78)
aggregate (+method, +exclude-outliers/T²); (79) repetition↔aggregate auto-link; (80) name/description;
(81) per-dataset list values in multi-dataset configs.

**A.14 Validation/robustness**: (82) schema validation + enum coercion + clear errors; (83) optional
config validator (presence/value/file-existence/mixed-format); (84) train-only or test-only tolerated;
(85) loader fallback on unsupported/failed format.

> Reimplementer note: in nirs4all the runtime loader consumes only the **legacy keys** + Y-by-column
> extraction; everything else is config sugar normalized into those keys, and folds load via a
> pipeline step. The new `nirs4all-io` **wires in** the currently-unwired role/partition/link
> utilities, so it can be *more* faithful to the declared config than nirs4all's runtime is today.

---

## Appendix B — Reusable inference heuristics (port targets)

(Distilled from `nirs4all/data/parsers/folder_parser.py`, `data/detection/detector.py`,
`data/signal_type.py`, `core/task_detection.py`, studio `targetTypeDetection.ts`.)

- **A. Filename → role/split** (case-insensitive): the Xcal/Xval/Ycal/Yval, Xtrain/Xtest, M/Meta/
  metadata, folds, and bare-stem `x/y/m/meta/metadata/group` sets; multiple matches → multi-source;
  patterns ≤2 chars need a delimiter word-boundary.
- **B. Header-unit**: nm `^\d{3,4}(\.\d+)?(nm)?$`; cm⁻¹ `^\d{4,5}(\.\d+)?(cm-1|wavenumber)?$`; text
  `^[A-Za-z]`/`feature_\d+`/`[xX]_?\d+`; index `^\d{1,3}$`; nm-vs-cm⁻¹ value-range tiebreak.
- **C. Numeric header = wavelengths**: all-numeric, ≥10 cols, strictly monotonic, range nm[200–2600]/
  cm⁻¹[400–15000], spacing CV ≤ 0.5; confidence by header-vs-data range contrast.
- **D. Header presence**: row-0 numeric-ratio ≥0.3 below following rows ⇒ header; all-numeric row-0 may
  still be a wavelength header (rule C).
- **E. Signal type from header text**: `abs(orbance)?`/`log(1/[RT])`/`A=`; `reflect`/`^R$`/`R%`;
  `transmit`/`^T$`/`T%` (+ abbreviation map).
- **F. Signal type from values**: absorbance min≥−0.5 & max∈[0.5,5] & mean∈[0.2,2]; reflectance
  max≤1.2 & mean∈[0.1,0.8]; reflectance% max∈(1.5,120] & mean∈[10,80]; transmittance (lower means);
  confidence = best/Σ, <0.7 ⇒ UNKNOWN.
- **G. Preprocessed guard**: mean≈0&std>0.1 (centered); |std−1|<0.1&|mean|<0.1 (SNV); derivative
  pattern ⇒ skip signal typing.
- **H. Water-band tiebreak**: O–H bands 1450/1940/2500 nm (cm⁻¹ 6897/5155/4000); peak⇒absorbance,
  dip⇒R/T.
- **I. Task/target type**: integer 2→binary / 3–100→multiclass / >100→regression; [0,1] {0,1}→binary;
  ordinal ints [0,10] with coverage≥0.6; decimals [0,1]/[0,100] with enough unique→regression;
  low-cardinality (≤20)→classification unless decimals/range>10.
- **J. Delimiter/decimal sniff**: try `,;\t|space`, score by column-count consistency; decimal by
  dot-vs-comma numeric-cell counts; encodings utf-8→latin-1→cp1252.
- **K. Confidence presentation**: ≥0.8 green / ≥0.6 amber / else red; integer percent + field-named
  reason.

---

## Appendix C — Codex review

Reviewer: **Codex `gpt-5.5`, reasoning effort xhigh, sandbox read-only** (`codex exec`), cross-checking
the claims against the real `nirs4all-io`, `dag-ml-data`, `dag-ml`, `nirs4all`, and `nirs4all-studio`
trees. Verbatim output below; the body of this doc (§§0,4,5,6,7,8,9,10) has been revised to incorporate
the accepted points (see the integration table after the review).

### C.1 Verbatim review

> **VERDICT:** Do not approve as written. The rename is plausible, but the new architecture
> underestimates loader parity work, overplaces dataset inference in the file layer, and describes a
> `dag-ml` mapping that does not match the real contracts.
>
> **Top 5 Problems**
> 1. **D1 Rust-first is not justified by the evidence.** The mature behavior is Python-side:
>    `DatasetConfig`, `DatasetLoader`, `SpectroDataset`, folder parsing, target/signal heuristics,
>    NA/categorical handling, and Studio UX. Porting "only decision logic" is false; matching current
>    behavior requires a loader rewrite.
> 2. **The `dag-ml-data` mapping is not feasible as described.** `dag-ml` consumes
>    `ExternalDataPlanEnvelope`, `DataBinding`, and campaign `FoldSet`; it does not consume the proposed
>    `dag-ml-data` `CoordinatorDataPlanEnvelope` as the external contract.
> 3. **D2 blurs the file-level/dataset-level boundary.** Format sniffing and neutral table statistics
>    belong in `nirs4all-formats`; role assignment, sample joins, folds, partitions, and most
>    signal-type inference belong in dataset-level `nirs4all-io`.
> 4. **The confidence model is not meaningful yet.** Weighted rule scores are not calibrated
>    probabilities. The doc lacks ground truth corpora, calibration metrics, conflict handling,
>    abstention, negative evidence, and per-domain thresholds.
> 5. **Rename/backlog sizing is too optimistic.** The identifier footprint is larger than Section 7
>    says, and the backlog schedules deletion/replacement before parity, validation contracts, and
>    migration strategy exist.
>
> **Per-Section Findings**
> - **Boundary:** The proposed split is directionally right: file readers should remain file/record
>   readers, and the new IO layer should compose datasets. But pushing "byte/value detection" wholesale
>   into formats is wrong. A CSV/TSV file does not know whether a numeric column is `X`, `y`, metadata, a
>   wavelength header, a fold id, or a join key without dataset context.
> - **D1:** A Rust-first full core optimizes for bindings before proving semantic parity. The Python
>   side already owns the primary target type and the mature config surface. A hybrid is safer: Rust for
>   file-format probing/record extraction and neutral descriptors; Python for `SpectroDataset`
>   materialization and existing loader semantics until the new spec is stable.
> - **D2:** `formats.describe()` should emit neutral evidence: format probe, explicit vendor axes,
>   sidecars, table shape, delimiter/header candidates, numeric distributions, and explicit file-declared
>   units. It should not authoritatively assign roles or infer dataset-level signal type unless the
>   format itself declares it.
> - **5.2 Conventions:** TOML profiles are useful, but the convention engine must model sidecar
>   grouping, archives, case sensitivity, duplicate stems, path normalization, multiple spectra per
>   sample, multiple targets, missing joins, and dropped-row audit reports. The vendor-corpus join by
>   filename stem/id is far too underspecified.
> - **5.3 Confidence:** "Normalized weighted rules + corpus" is not enough. Call it a score until
>   calibrated. Required: labeled corpora, train/test split by vendor/domain, confusion matrices,
>   Brier/ECE calibration, ambiguity classes, abstain thresholds, and evidence traces.
> - **5.4 Resolver:** Scanning paths is not enough. The resolver needs stable identities for
>   path/member/record, content hashes, sidecar grouping, provenance, and deterministic ordering. It
>   also needs to ingest in-memory arrays and already-constructed `SpectroDataset` objects if it claims
>   Python parity.
> - **5.5 SpectroDataset:** Materialization is plausible, but not trivial. Current Python behavior
>   includes train/test-only loading paths, repetition/aggregation, task/signal detection, folds,
>   metadata, and indexer alignment. The new layer must preserve those semantics before replacing
>   anything.
> - **5.5 `dag-ml-data`:** The proposed partition/fold sidecar mapping is wrong. `dag-ml` derives
>   train/validation/predict sample ids from campaign split state, not arbitrary `DataView.partition`
>   strings. The bridge must generate or validate a real `FoldSet`, `DataBinding`, fingerprints, and
>   `ExternalDataPlanEnvelope`.
> - **Section 7 Rename:** The plan misses CI/release artifacts, generated binding names, package
>   registry implications, compatibility aliases, docs build outputs, and conformance/helper strings.
>   Treat as L/XL, not mechanical M.
> - **Section 8 Backlog:** The order is backwards. Define the canonical dataset spec and target contract
>   first; spike real `dag-ml` validation before promising mappings; only then build inference and
>   deletion plans.
>
> **Factual Corrections**
> - `dag-ml-data` `AxisKind` has no `Wavenumber`; it has `Wavelength`, `Frequency`, etc. See
>   `dag-ml-data/crates/dag-ml-data-core/src/model.rs:8`.
> - `DataView.partition` and `fold_id` exist, but `dag-ml-data` filtering ignores them; filtering uses
>   sample/source/augmentation fields. See `dag-ml-data/crates/dag-ml-data-core/src/model.rs:212` and
>   `.../handle.rs:657`.
> - `dag-ml` consumes `ExternalDataPlanEnvelope`, not the `dag-ml-data` `CoordinatorDataPlanEnvelope`
>   described in the doc. See `dag-ml/crates/dag-ml-core/src/data.rs:660`,
>   `dag-ml/crates/dag-ml-cli/src/main.rs:900`, and `dag-ml-data/.../coordinator.rs:60`.
> - `dag-ml` partition semantics are campaign/fold driven: `FoldTrain`, `FoldValidation`, `FullTrain`,
>   `Predict`. See `dag-ml/crates/dag-ml-core/src/data.rs:35` and `.../runtime.rs:5807`.
> - `dag-ml-data` relations use `origin_id` as an observation id; `dag-ml` core relations use
>   `origin_sample_id`. That conversion is lossy when multiple observations share a sample. See
>   `dag-ml-data/.../relation.rs:8` and `dag-ml/crates/dag-ml-core/src/relation.rs:17`.
> - Current Rust CSV-like reading is not close to Python loader parity: simple delimiter splitting,
>   requires numeric spectral headers, hard-codes `nm` and `Absorbance`. See
>   `crates/nirs4all-io/src/readers/csv_like.rs:74`, `:131`, and `.../readers/util.rs:148`.
> - Python config richness is partly normalized back to legacy train/test keys, not fully first-class at
>   runtime. See `nirs4all/.../data/parsers/normalizer.py:303`, `.../data/schema/config.py:1816`, `:1874`.
> - `ColumnConfig` and `FileConfig` are explicitly marked future/stub concepts in Python. See
>   `nirs4all/.../data/schema/config.py:264` and `:611`.
> - Folder parsing is CSV/archive-light and top-level, not a general recursive corpus resolver. See
>   `nirs4all/.../data/parsers/folder_parser.py:60`, `:207`, `:312`.
> - Studio confidence is already partly hard-coded, so it is not a credible calibration source by
>   itself. See `nirs4all-studio/api/datasets.py:363` and `.../DatasetWizard/ParsingStep.tsx:48`.
> - Rename footprint: the doc says `Cargo.toml x5`, but this workspace has 8 relevant `Cargo.toml`
>   files including root, Python, R Rust, WASM, and four crates. CI/release also embeds names. See
>   `Cargo.toml:3`, `bindings/wasm/Cargo.toml:2`, `bindings/r/nirs4allio/src/rust/Cargo.toml:2`,
>   `.github/workflows/release.yml:80`, `:128`.
>
> **Recommendations**
> - **D1:** Reject Rust-first full core for now. Choose either Python-first for `SpectroDataset` MVP, or
>   hybrid: Rust owns neutral file descriptors and `dag-ml`-oriented schema pieces; Python owns existing
>   loader parity and `SpectroDataset` materialization.
> - **D2:** Narrow it. Put file sniffing, sidecar discovery, explicit format metadata, and neutral table
>   stats in `nirs4all-formats`. Keep role assignment, joins, partitions, folds, task type, and
>   contextual signal inference in new `nirs4all-io`.
> - **D3:** Keep declarative conventions, but define a versioned schema and machine-readable validation.
>   TOML can be the human format; do not make TOML the only canonical representation.
> - **D4:** Design now, implement after a spike that passes real `dag-ml validate-data-binding` using
>   `CampaignSpec`, `FoldSet`, `DataBinding`, fingerprints, and `ExternalDataPlanEnvelope`.
> - **D5:** Do not delete Python loaders early. Delete duplicated inference only after parity tests prove
>   the new layer preserves NA handling, categorical metadata, archives, joins, folds, aggregation, and
>   signal/task behavior.
> - **D6:** Support row defaults, but model `sample_id`, `observation_id`, `source_id`, and
>   `repetition_id` explicitly. For `dag-ml`, row number alone is not a stable identity.
>
> **Backlog Fix**
> Reorder it as: rename audit/migration plan; canonical `DatasetSpec` + ID/join semantics; Python MVP
> over existing loader; neutral `formats.describe`; inference corpus/calibration; vendor-corpus join
> fixtures; real `dag-ml` contract spike; then replacement/deletion. Resize rename and Epics 1-4 upward,
> and add stories for schema versioning, migration aliases, provenance, security/resource limits,
> sidecar/archive grouping, calibration reports, and cross-language golden tests.

### C.2 Integration decisions (how this doc changed)

| Codex point | Disposition | Where applied |
|---|---|---|
| D1: reject Rust-first-now; Python-first MVP / hybrid | **Accepted** | §0 TL;DR, §4.3, §6 D1 |
| "port only decision logic" is false; loader work is a rewrite | **Accepted** | §0, §6 D1, Critique **C10** |
| D2: formats emits *neutral* evidence only; role/contextual-signal inference stays in io | **Accepted** | §4.2, §6 D2 |
| dag-ml consumes `ExternalDataPlanEnvelope`+`DataBinding`+`FoldSet`, not `CoordinatorDataPlanEnvelope` | **Accepted (factual fix)** | §5.5, §6 D4, Critique C4 |
| `DataView.partition`/`fold_id` ignored by dag-ml-data filtering | **Accepted (factual fix)** | §5.5, Critique C4 |
| `AxisKind` has no `Wavenumber` (cm⁻¹ gap) | **Accepted (factual fix)** | §5.5, §6 D4, Critique C4 |
| `origin_id` (obs) vs `origin_sample_id` (sample) lossy | **Accepted (factual fix)** | §5.5, Critique C4 |
| Confidence: score-not-probability; corpus split, Brier/ECE, abstention, negative evidence | **Accepted** | §5.3, §6 D3, Story 3.6, Critique C5 |
| Resolver needs stable identity/hash/sidecar/ordering + ingest prebuilt `SpectroDataset` | **Accepted** | §5.4, Story 3.1, Critique C11 |
| Conventions: versioned machine-validatable schema; TOML is human format; sidecar/archive/dupe handling | **Accepted** | §6 D3, Story 2.0, Critique C8 |
| D5: delete only duplicated *inference*, parity-gated; keep loaders | **Superseded 2026-05-27**: user constraint = **do not touch nirs4all/studio at all** (no deletion); re-plug is user-owned/out-of-scope | §6 D5, §10.0, Epic 5 (now "seam only") |
| D6: model `sample_id`/`observation_id`/`source_id`/`repetition_id` explicitly | **Accepted** | §6 D6, Story 2.0 |
| Rename is 8 Cargo.toml + CI/release/registry; L/XL | **Accepted (factual fix)** | §7, Epic 0 |
| Backlog reorder: spec+spike first, deletion last; add stories | **Accepted** | §8 (reorder note, Stories 0.0, 2.0, 3.6, 4.3, 4.5, 6.4) |
| dag-ml-data: design-now/ship-after-spike | **Accepted** | §6 D4, Stories 4.3/4.4 |

No points were rejected; two were *already* aligned with the draft (the file/dataset boundary direction;
Python config richness normalized to legacy keys) and are now stated more sharply.

### C.3 — Second review (revised doc, 2026-05-27): two-phase / no-touch + vocabulary

After the user added the no-touch constraint, the two-phase plan, and the "vast + documented vocabulary"
requirement, Codex (`gpt-5.5`, xhigh, read-only) re-reviewed. Verdict: *"Do not approve as an
implementation base yet; the two-phase/no-touch framing is mostly fixed, but lookup/join semantics and
cookbook coverage are still materially wrong."* All points were **accepted and applied**:

| Codex finding (2nd round) | Disposition | Where applied |
|---|---|---|
| `kind:lookup` → dag-ml-data `PerGroup+sample_key` is **false** (planner resolves by id/adapter, `planner.rs:235`); io must join itself; `group_id` only for leakage keys | **Accepted (factual fix)** | E.2 note, H.2, Critique C13 |
| **L.8 conflates sample-id with the lookup key** (`site_code` is m:1) | **Accepted** | L.8 rewritten (`sample_index: by row`; `site_code` = metadata + explicit join) |
| JSON map **order can't carry "first-match-wins"** | **Accepted** | E.1 → ordered list; map only if disjoint; overlaps = error |
| Cookbook **coverage claims false** (name_range/rest/auto/strict_columns/by_key/1:m/drop/error unexercised); matrix must be **generated** | **Accepted** | L.18 feature gallery + CI-generated matrix |
| Lazy import **circular-safe but heavy** (`nirs4all.data.__init__` imports visualization); no-touch leftover in App. K | **Accepted** | App. K reworded; Story 4.1b "optional target dep"; Story 6.5 import-boundary CI |
| Separate `sample_id`/`observation_id`/`join_key`; don't overload `index`; explicit `left/right/left_on/right_on`; composite + virtual keys; duplicate-key + complete-vs-error semantics | **Accepted** | E.1/E.2 + schema (`key:` not `index`); join table |
| Lookup must **honor roles** after broadcast (L.14 broadcasts targets) | **Accepted** | E.2 + L.8/L.14 |
| Declare **out-of-scope layouts** (long/tidy pivot, ragged, JSON/NDJSON) | **Accepted** | E.2 "supported vs out-of-scope" |
| Backlog: **join engine story** before materialization; **split 4.1** (XL); **copy-provenance/license** (CeCILL→MIT); parity oracle **earlier**; import-boundary CI; story **2.4 into MVP**; fix 1.3 AC | **Accepted** | Stories 4.0/4.1a/4.1b, 5.4, 6.5; Critique **C12**; reordered §8; MVP line |

---

## Appendix D — Reuse Map (copy-and-re-orchestrate; the Phase-1 head-start)

**Principle (user, 2026-05-27):** most of this already exists in `nirs4all` + `nirs4all-studio` — *don't
reinvent it; if it's well done, copy the business logic, then extend / complete / generalize / make
format-compatible.* **⛔ We do not modify the originals and do not import nirs4all at runtime** — we
**copy the logic into a self-contained `nirs4all-io`**. The originals lack recul, so io is free to
re-architect while copying. Actions: **COPY-LOGIC** (copy the algorithm into io, re-orchestrated) ·
**GENERALIZE** (COPY-LOGIC + make declarative/format-agnostic/multi-target) · **PORT** (translate
language: TS→Python now, Python→Rust in P2) · **REFERENCE** (design reference / read-only parity oracle
only — never imported at runtime, never edited) · **EMIT(lazy)** (the *sole* runtime touch-point: lazily
import the `SpectroDataset` *class* at materialization — the existing `to_spectrodataset` pattern). Paths
are under `nirs4all/nirs4all/` and `nirs4all-studio/`, read **only** to copy from.

| # | Capability | Existing component (path) | State today | Action in `nirs4all-io` | Story |
|---|---|---|---|---|---|
| 1 | Config schema | `data/schema/config.py` (`DatasetConfigSchema`, `FileConfig`, `SourceConfig`, `PartitionConfig`, `FoldConfig`, `LoadingParams`) | mature; `FileConfig`/`ColumnConfig` are future/stubs | **COPY-LOGIC** → `DatasetSpec` (App. E); complete column-role | 2.1 |
| 2 | Key/alias normalization | `data/parsers/normalizer.py` (partition×role synonym map) | mature, very broad | **COPY-LOGIC** (the synonym map) | 2.2 |
| 3 | Config dispatch / parse | `data/config_parser.py`, `data/parsers/files_parser.py` | mature | **COPY-LOGIC** → resolver | 3.1 |
| 4 | Folder conventions | `data/parsers/folder_parser.py` (`FILE_PATTERNS`, word-boundary, multi-source) | mature but CSV-only, top-level | **GENERALIZE** → declarative profiles (App. G) + vendor exts (formats) + recursive + sidecar | 2.3 |
| 5 | Tabular loaders | `data/loaders/{loader,base,csv_loader_new,numpy_loader,parquet_loader,excel_loader,matlab_loader,archive_loader}.py` | mature, battle-tested | **COPY-LOGIC** into io as the P1 materialization engine (no nirs4all import); route *vendor* files to formats. **Don't redesign the algorithm.** | 4.1 |
| 6 | NA / categorical / param precedence | `data/loaders/base.py` `apply_na_policy`; `LoadingParams.get_effective_params` | mature | **COPY-LOGIC** | 4.2 |
| 7 | File-param detection (delimiter/decimal/header/header_unit) | `data/detection/detector.py` (`AutoDetector`, `HEADER_PATTERNS`, sniffers) | mature (auto-detect is bypassed-by-default *in the loader*, but the detector itself works) | **COPY-LOGIC** into io (P1) → **PORT** to Rust `formats.describe` (P2) | 1.1, 3.2 |
| 8 | Wavelength-header detection | `detector.py` `_detect_wavelength_header` | mature | **COPY-LOGIC** into column-role inference | 3.3 |
| 9 | Signal-type detection | `data/signal_type.py` (`SignalTypeDetector`, water-band, `from_string`) | mature, rich | **COPY-LOGIC** into io signal inference (dataset-context, per D2) + extend with calibration/abstention | 1.2, 3.3 |
| 10 | Task-type detection | `core/task_detection.py` `detect_task_type` | mature, simple | **COPY-LOGIC**; unify with the ported TS (#18) | 3.3 |
| 11 | Column-role assignment | `data/selection/role_assigner.py`/`column_selector.py`/`row_selector.py` | **EXIST but UNWIRED** in the runtime loader | **COPY-LOGIC** — the algorithms are already written (just unused); copy them into io's orchestration. Biggest head-start. | 3.3, 4.1 |
| 12 | Cross-file linking by key | `data/selection/sample_linker.py` (`SampleLinker`) | **EXISTS but UNWIRED** | **COPY-LOGIC** (id/row indexing, vendor-corpus join) | 4.1 |
| 13 | Single-file partitioning | `data/partition/partition_assigner.py` (column/percentage/index/index-file) | **EXISTS but UNWIRED** | **COPY-LOGIC** (partitions) | 4.1 |
| 14 | Fold-file parsing | `controllers/splitters/fold_file_loader.py` (`FoldFileParser`: csv-nirs4all / csv-assignment / json / yaml / txt) | mature (pipeline-step only) | **COPY-LOGIC/GENERALIZE** → dataset-level fold loading | 4.1 |
| 15 | `SpectroDataset` build | `data/config.py` `DatasetConfigs._load_dataset`; `data/dataset.py` `SpectroDataset.add_*`/`set_*` | mature | **COPY-LOGIC** (the build orchestration) + **EMIT(lazy)** the `SpectroDataset` class (App. H.1) | 4.1 |
| 16 | records → `SpectroDataset` | `nirs4all-formats` `bindings/python/.../records.py` `to_spectrodataset` | exists (export helper; **this is ours** — formats repo) | **GENERALIZE** → the formats→dataset bridge | 4.1 |
| 17 | Studio directory inference | `nirs4all-studio/api/datasets.py` `detect-unified`/`auto-detect`/`detect-format` | thin delegation orchestration | **REFERENCE** the orchestration approach + **COPY-LOGIC** into `DatasetPlan`. **No "replace caller" — studio is untouched.** | 3.2 |
| 18 | Studio target-type heuristics | `nirs4all-studio/src/lib/playground/targetTypeDetection.ts` | richer than the Python `detect_task_type` | **PORT** (TS→Python) into io inference | 3.3 |
| 19 | Studio confidence UX | `nirs4all-studio/.../DatasetWizard/ParsingStep.tsx` `ConfidenceIndicator` (0.8/0.6 buckets), `FileMappingStep.tsx` | mature UX | **REFERENCE** for plan-presentation thresholds (the UX re-plug is the user's later studio work) | 3.6 |
| 20 | Config validation | `data/schema/validation/validators.py` (`ConfigValidator`, error codes) | mature, optional | **COPY-LOGIC** → `DatasetSpec` validation + plan warnings | 2.1 |

**No deletion in this scope.** `nirs4all`/`nirs4all-studio` stay in production, untouched. The only
nirs4all runtime touch-point is **EMIT(lazy)** of the `SpectroDataset` class (#15). nirs4all/studio may be
imported **dev/test-only** as a read-only parity oracle (Story 5.2) to detect drift on #5/#6/#7/#15. Any
future dedup of the originals is part of the **user-owned re-plug — out of scope** (D5).

---

## Appendix E — `DatasetSpec` v1 (full schema)

Canonical form = versioned JSON Schema (machine-validated); YAML/dict accepted on input and
alias-normalized (D3, story 2.2). Every field maps to ≥1 Expressiveness-Checklist item (App. A).

```jsonc
{
  "schema_version": 1,                                  // required; migration-gated
  "name": "mango", "description": "...",                // A.13/80
  "task_type": "auto|regression|binary|multiclass",     // A.13/77

  "sample_index": {                                     // A.9 — id vs row (D6)
    "by": "row|id",
    "key": "Sample_ID",                                 // column name or filename-stem rule when by=id
    "observation_id": "auto|<column>",                  // explicit obs id for reps/augmentation
    "repetition_id": "auto|<column>", "group_id": "<column>" },

  "signal_type": "auto|absorbance|reflectance|reflectance%|transmittance|transmittance%|log(1/R)|kubelka-munk",  // A.11/67-69; global default

  "conventions": ["nirs4all-classic"],                  // profiles applied during inference (App. G); A.3

  "sources": [                                          // A.6 multi-source; ordered; ≥1 produces features
    { "id": "data", "role": "features|targets|metadata|mixed",  // "mixed" ⇒ roles come from `columns` (A.4)
      "kind": "table|lookup",                            // "lookup" = keyed dimension table (few rows, m:1) — E.2
      "modality": "spectroscopy|markers|metadata|image",
      "input": "<path|glob|[p1,p2,p3]|{array}|{record_set}|{spectrodataset}>", // A.1/A.2; a LIST ⇒ see `merge`
      "merge": "concat_samples|concat_features|by_key|none",  // combine a multi-file input (E.2; R-MERGE)
      "partition": "train|test|val|predict|auto",        // A.5
      "key": "Sample_ID",                                // col(s) aligning THIS source to the sample axis; NOT a role (E.2). composite: ["a","b"]; virtual: "filename_stem"
      "columns": [                                       // ORDERED column-role selectors (E.1; R-COLSEL) — mixed X/Y/meta OK
        {"role": "features", "select": {"regex": "^\\d+(\\.\\d+)?$"}},   //   select ∈ idx | "name" | [list] | "a:b" | {regex} | {dtype} | {name_range} | "rest" | {"auto":{candidates:[…]}}
        {"role": "targets",  "select": ["protein","moisture"]},
        {"role": "metadata", "select": ["site_code","date"]},
        {"role": "ignore",   "select": ["notes"]} ],     //   map shorthand also OK IFF selectors are disjoint
      "join": { "left": "data", "right": "<other source id>",   // relational join (E.2; R-JOIN) — io performs it
                "left_on": "site_code", "right_on": "site_code",
                "cardinality": "1:1|m:1|1:m", "coverage": "complete|warn|drop|error" },
      "variations": [ {"id":"snv","preprocessing":{"type":"SNV"}} ],          // A.7 (optional)
      "params": {                                        // A.12; precedence file>partition>global
        "delimiter": ";", "decimal_separator": ".", "has_header": true,
        "header_unit": "cm-1|nm|none|text|index",        // A.10/65
        "signal_type": "auto", "encoding": "utf-8",
        "na": { "policy": "auto|abort|remove_sample|remove_feature|replace|ignore",
                "fill": { "method": "value|mean|median|forward_fill|backward_fill",
                          "fill_value": 0, "per_column": false } },            // A.12/73
        "categorical": "auto|preserve|none",             // A.12/74
        "format": { "sheet_name": 0, "usecols": null, "columns": null,
                    "variable": null, "key": null, "member": null } } }        // A.12/76
  ],

  "partitions": {                                        // A.5 — split a single combined input (copy-logic #13)
    "by": "files|column|percentage|index|index_file",
    "column": "set", "train_values": ["cal"], "test_values": ["val"],
    "predict_values": [], "unknown_policy": "train|test|drop|error",
    "train": "80%", "test": "20%", "predict": null,      // or index lists / "0:80%"
    "train_file": null, "test_file": null,
    "shuffle": true, "random_state": 0, "stratify": "y" },

  "folds": {                                             // A.8 (copy #14)
    "inline": [ {"train": [/*ids|rows*/], "val": [/*...*/]} ],
    "file": "folds.csv", "format": "auto|csv|json|yaml|txt",
    "column": "cv_fold" },

  "aggregate": { "by": "Sample_ID|true", "method": "mean|median|vote",          // A.13/78
                 "exclude_outliers": false, "outlier_threshold": 0.95 },
  "repetition": "Sample_ID",                              // A.13/79

  "params": { /* global LoadingParams, lowest precedence; root-shorthand allowed (A.12/75) */ },

  "validation": { "check_file_existence": true, "allow_train_only": true,       // A.14
                  "allow_test_only": true }
}
```

Multi-dataset (A.1/9): a top-level JSON **array** of `DatasetSpec`. In-memory inputs (A.1/10-13) bypass
`sources[].input` paths and attach arrays directly. A prebuilt `SpectroDataset` (A.1/14) is accepted by
the resolver and passed through. **Every App. A item must trace to a field here** (acceptance: story 2.1).

### E.1 — Column-role selectors (R-COLSEL)

Within a tabular source, `columns` assigns each column a **role**: `features` (X), `targets` (Y),
`metadata`, `weights`, `ignore`. X / Y / metadata may be **mixed and interleaved** in one file. The
**join/identity key is NOT a column role** — it is declared by the source's `key:` field (E.2) and by the
dataset's `sample_index` (D6), so identity and roles never get conflated.

**Canonical form is an ordered list** (JSON/YAML object key order must not carry semantics — Codex):
```yaml
columns:
  - { role: features, select: { regex: '^\d+(\.\d+)?$' } }   # evaluated top→bottom; explicit precedence
  - { role: targets,  select: [protein, moisture] }
  - { role: metadata, select: rest }
```
A **map shorthand** (`{features: …, targets: …}`) is also accepted **only when the selectors are
disjoint**; any overlap is a **validation error** (no order-dependent "first wins"). `strict_columns:
false` relaxes the list form to first-match-wins if a user opts in.

| Selector form | Meaning | Example |
|---|---|---|
| integer / `[ints]` | column(s) by position | `-1`, `[0,1]` |
| `"a:b"` | positional slice (Python semantics; negatives OK) | `"2:-1"` |
| `["name", …]` | by header name | `["protein","moisture"]` |
| `{"name_range": ["400","2500"]}` | contiguous header range by name | spectra block |
| `{"regex": "…"}` | header regex | `{"regex":"^\\d+(\\.\\d+)?$"}` |
| `{"dtype": "numeric\|string\|datetime\|bool"}` | by inferred dtype | `{"dtype":"string"}`→metadata |
| `"rest"` | every column not yet assigned (**≤1 per spec**) | catch-all |
| `"auto"` | inference engine decides — must list `candidates` (roles) + abstains if ambiguous | unknown layouts |

Rules: selectors must not overlap (error) unless `strict_columns:false`; **at most one `rest`**; columns
left unmatched with no `rest` → error (no silent default); `auto` carries `{candidates:[…]}` and emits a
plan `ambiguous` entry rather than guessing (App. F).

### E.2 — Source keys, multi-file merge & relational joins (R-MERGE, R-JOIN)

**Identity vs keys (Codex — keep distinct):** `sample_index` (D6) defines the canonical *sample identity*
(`by: row` or `by: id, key: <col>`); a per-source **`key:`** names the column(s) that align *this
source's rows* to the sample axis (default = row order); **join keys** are named separately in `join`
(`left_on`/`right_on`) and may differ from either. Keys may be **composite** (a list of columns) or a
**virtual key** (`filename_stem` for vendor-file corpora).

A source whose `input` is a **list of files** is combined per `merge`:
- **`concat_samples`** — vertically **stack rows** (union of columns; missing → null = *schema-union*);
  e.g. 3 batch CSVs → one sample axis. Per-sample provenance records the origin file.
- **`concat_features`** — horizontally **stack column-blocks** for the *same* samples, aligned by `key`
  (preferred) or row order; column names namespaced to avoid clashes.
- **`by_key`** — relational **join** of the listed files on their `key` (sugar for an internal `join`).
- **`none`** — keep as separate sources (default for a single path).

Cross-source **joins** are explicit (no overloaded `on`):
```yaml
join: { left: measurements, right: sites,
        left_on: site_code, right_on: site_code,    # or composite: [a,b]; or virtual: filename_stem
        cardinality: '1:1|m:1|1:m', coverage: 'complete|warn|drop|error' }
```
**Shorthand** (used in the cookbook for brevity): `join: {to, on, how}` ≡ `{left:<this source>,
right:<to>, left_on:<on>, right_on:<on>, cardinality:<how>}`.

| `cardinality` | Meaning | Duplicate-key rule |
|---|---|---|
| `1:1` | aligned (by key or row) | duplicate on either side → error |
| **`m:1`** | **lookup / dimension table**: many left rows → one right row; right columns **broadcast** to each match | duplicate *right* key → error; duplicate left OK |
| `1:m` | one left row → many right rows (left fields broadcast; sample axis grows) | duplicate *left* key → error |

| `coverage` (governs *unmatched left keys*) | Behavior |
|---|---|
| **`complete`** | every left key must match (assert left ⊆ right); else error. *(Cardinality/duplicate violations are always errors, independent of `coverage`.)* |
| `warn` | keep all; warn on misses; fill missing right columns with null |
| `drop` | drop unmatched left rows (dropped-row audit — C8) |
| `error` | hard error on the **first** miss (vs `complete` which reports the full set) |

A `kind: "lookup"` source is a **dimension table**: not itself a sample source; it contributes columns
via an `m:1` join, and **each contributed column keeps the role assigned in the lookup's own `columns`**
(so a lookup may broadcast `metadata` *or* `targets` — roles are honored, not forced to metadata).

**⚠️ dag-ml-data has no native join (Codex, verified):** `SourceDescriptor.sample_key`/`granularity` are
descriptor fields; the planner resolves sources by id/adapter, **not** by joining on `sample_key`
(`dag-ml-data/.../planner.rs:235`). So **`nirs4all-io` performs every join/broadcast itself** and feeds
dag-ml-data already-aligned per-sample data; `SampleRelation.group_id` is set **only when the join key is
also a leakage/grouping unit** (`relation.rs:9`), not for every lookup. (Appendix H.2.)

**Supported vs out-of-scope on-disk layouts** (state the boundary so users know — adoption depends on it):
- *Supported*: mixed X/Y/metadata columns; positional or keyed 1:1; `concat_samples`/`concat_features`;
  m:1 lookups; composite + virtual (`filename_stem`) keys; schema-union concat; recursive globs with
  include/exclude; a second **units header row** (mapped to `header_unit`); train/test/val/predict;
  external folds.
- *Out of scope for the MVP (declared, may come later)*: long/tidy → wide **pivot**; **ragged**
  per-row wavelength grids; arbitrary nested **JSON/NDJSON**; database/SQL sources. The plan must
  **detect and clearly refuse** these with a pointer, not mis-load them.

Every selector/merge/join element has a worked example in Appendix L; the coverage matrix there is
**generated from the fixtures in CI**, not hand-asserted (story 2.4).

---

## Appendix F — Inference engine (full spec)

**Pipeline:** `resolve(InputSet)` → per-item neutral `describe` (App. D #7-9 logic) → per-decision
hypothesis generation → weighted scoring → normalization → abstention/ambiguity gate → `DatasetPlan`
(+ a `resolved_spec: DatasetSpec` that `load` can execute).

**Decision types** (each independently scored, each carries an evidence trace):
`structure` · `file_role` (X/Y/metadata/folds) · `file_partition` (train/test/val/predict) ·
`source_index` · `column_role` (per column in a combined file) · `params` (from describe) · `axis`
(wavelengths+unit) · `signal_type` · `task_type`.

**Scoring.** For a decision with hypotheses `H`: `raw(h) = Σ_i w_i · e_i(h)`, where each rule `e_i ∈
[-1,1]` (positive **and negative** evidence — C5). `score(h) = softmax(raw)` over `H`. **Abstain** when
`score(top1) − score(top2) < margin` (per-decision, tuned on the corpus) → the plan emits
`{ambiguous: true, choices: [...]}` instead of a verdict. Rule weights live in **one versioned table**
(seeded from `detector.py`/`signal_type.py` thresholds — App. B — then recalibrated; story 3.4).

**Rule table (excerpt; full table is the deliverable of story 3.4).**

| Decision | Rule (App. B ref) | Δ |
|---|---|---|
| file_role | filename matches profile `train_x` pattern (A) | +0.6 |
| file_role | filename also matches a `*_y` pattern (conflict) | −0.3 |
| column_role=feature | column header is a monotonic nm/cm⁻¹ wavelength in a ≥10-col block (C) | +0.5 |
| column_role=target | low-cardinality numeric, last column, not wavelength-like (I) | +0.3 |
| column_role=metadata | non-numeric / id-like / high-cardinality string | +0.4 |
| signal_type=absorbance | water-band peak @1450/1940 nm (H) | +0.3 |
| signal_type=reflectance | values in [0,1.2], mean∈[0.1,0.8] (F) | +0.3 |
| signal_type | data looks preprocessed (SNV/derivative) (G) | abstain |
| task_type | integer target, 2 unique → binary (I) | +0.5 |

**`DatasetPlan` schema:**

```jsonc
{
  "input": {"kind": "...", "ref": "..."},
  "structure": {"kind": "...", "score": 0.92, "evidence": [...], "ambiguous": false},
  "assignments": [ {"ref": "...", "role": "...", "partition": "...", "source_index": 0,
                    "score": 0.95, "evidence": [...], "alternatives": [...], "ambiguous": false} ],
  "columns": [ {"ref": "...", "column_roles": [ {"col": "...", "role": "...", "score": 0.9,
                    "evidence": [...], "alternatives": [...]} ]} ],
  "params": {"<ref>": {"delimiter": {"value": ";", "score": 0.8}, ...}},   // from describe
  "axis": {"unit": "nm", "n": 256, "range": [950,1650], "score": 0.95},
  "signal_type": {"value": "absorbance", "score": 0.78, "reason": "...", "ambiguous": false},
  "task_type": {"value": "regression", "score": 0.8},
  "warnings": [...], "recommendations": [...], "dropped_rows": [...],       // join audit (C8)
  "resolved_spec": { /* DatasetSpec — editable; load() executes it */ },
  "overall_score": 0.88, "calibration": {"method": "isotonic|none", "brier": null}
}
```

**Calibration (story 3.6).** Corpus split by vendor/domain → confusion matrices + precision/recall +
**Brier/ECE**; if mis-calibrated, fit isotonic recalibration; tune per-decision abstain margins. Until
this exists, the UI labels values as **scores** (not probabilities) and uses them for ranking/triage.

---

## Appendix G — Convention profiles (full)

**Profile schema** (canonical JSON; TOML is the human form):

```jsonc
{ "profile": "nirs4all-classic", "version": 1,
  "roles": { "train_x": ["xcal","x_cal","cal_x","calx"],
             "test_x":  ["xval","x_val","val_x","valx","xtest","x_test","test_x","testx"],
             "train_y": ["ycal","y_cal","cal_y","caly"], "test_y": ["yval","ytest", ...],
             "train_meta": ["mcal","metacal","metadata_cal", ...], "test_meta": [...],
             "folds": ["folds","fold","cv","cv_folds","splits","cross_validation"] },
  "bare": { "x": ["x"], "y": ["y"], "meta": ["m","meta","metadata","group"] },
  "match": { "short_pattern_word_boundary": true, "case_insensitive": true,
             "extensions": "from-formats-registry", "recursive": false } }
```

**Built-ins shipped:** `nirs4all-classic` (cal/val — copied from `FolderParser.FILE_PATTERNS`),
`train-test` (`x_train`/`x_test`/`y_train`/`y_test`), `bare` (`X`/`Y`/`M`), **`vendor-corpus`** (new).

```jsonc
{ "profile": "vendor-corpus", "version": 1,
  "spectra": { "match": "by-format-sniff", "formats": ["opus","jcamp","spc","asd","sed","sig", ...] },
  "reference": { "names": ["y","ref","reference","targets","labels","meta","metadata"] },
  "join": { "sample_key": "filename_stem|<id-column>", "on_missing": "warn|drop|error",
            "on_duplicate_stem": "warn|error", "multi_spectra_per_sample": "reps|error" } }
```

**Matching algorithm:** (1) enumerate items (resolver — incl. archive members + sidecar groups);
(2) normalize names (lowercase, path-normalize); (3) for each role, test patterns — patterns ≤2 chars
require a delimiter word-boundary (`. _ - space`), longer = substring; (4) >1 file per role → multi-source
list; (5) unmatched files → second-pass bare-stems; (6) `vendor-corpus`: sniff spectra via formats, then
**join** to the reference table by `sample_key` and emit a **dropped-row audit** (C8). Profiles are
composable (`["nirs4all-classic","vendor-corpus"]`) and user profiles load from a `conventions/` dir or
inline in the `DatasetSpec`.

---

## Appendix H — Materializer mappings

### H.1 — `DatasetSpec` → `SpectroDataset` (Phase 1)

Flow: io re-implements the `_load_dataset` build orchestration from **copied** logic (tabular reading =
copied loader logic; vendor reading = `nirs4all-formats`); the resulting `SpectroDataset` object is
created via a **lazy import of the class** — `from nirs4all.data import SpectroDataset` *inside the
materializer* (the `to_spectrodataset` pattern), so io has no top-level nirs4all dependency and nirs4all
is never modified. The calls below are made on that lazily-obtained class:

| `DatasetSpec` | `SpectroDataset` call (`data/dataset.py`) |
|---|---|
| features source, partition `p` | `add_samples(X, indexes={"partition": p}, headers, header_unit)` |
| targets | `add_targets(y)` |
| metadata | `add_metadata(df, headers)` |
| `signal_type` | `set_signal_type(sig, src, forced=…)` |
| `task_type` | `set_task_type(...)` |
| `folds` (inline/file/column) | parse via lifted `FoldFileParser` → `set_folds([(train,val),…])` |
| `partitions` (column/%/index) | resolve via wired-in `PartitionAssigner` → per-sample partition |
| `sample_index.by=id` + `link_by` | wired-in `SampleLinker` → align sources by key |
| `repetition` / `aggregate*` | `set_repetition(col)` / `set_aggregate*` |

### H.2 — `DatasetSpec` → `dag-ml-data` + `dag-ml` (Phase 2; structs verified against source)

**Two artifact sets** (the bridge produces both; `dag-ml` consumes the campaign side):

1. **`dag-ml-data`** (`crates/dag-ml-data-core/src/model.rs`, `relation.rs`):
   - `DatasetSchema { dataset_id, sample_ids: Vec<SampleId>, sources: Vec<SourceDescriptor>,
     targets: BTreeMap<TargetId, RepresentationSpec>, metadata: BTreeMap<String, RepresentationSpec> }`.
   - per features source → `SourceDescriptor { id, name, type_id, modality, native_representation,
     sample_key, granularity, schema, tags }`; `RepresentationSpec { id, type_id, rank, axes, container,
     dtype, sparse, ragged }`; an `AxisSpec { name, kind, unit, size, coordinates }` carries wavelengths.
   - relations → `SampleRelationTable { rows: [SampleRelation { observation_id, sample_id, source_id,
     target_id, group_id, origin_id, repetition_id, augmented, excluded, metadata }] }`.
   - **Joins/lookups are resolved by `nirs4all-io` BEFORE emitting (Codex):** dag-ml-data does not join on
     `sample_key` (`planner.rs:235` resolves by id/adapter), so io feeds it **already-aligned, already-
     broadcast** per-sample sources. A `kind:lookup` m:1 table becomes broadcast columns on the sample
     rows; set `SampleRelation.group_id` **only** when the lookup/join key is also a leakage/grouping unit
     (`relation.rs:9`) — *not* for every lookup.
   - compute `schema_fingerprint` / `plan_fingerprint` / `relation_fingerprint` (exist in dag-ml-data).

   **AxisKind mapping (gap — `model.rs:8` has no `Wavenumber`):**
   | spectra unit | maps to | note |
   |---|---|---|
   | nm | `AxisKind::Wavelength`, `unit:"nm"` | clean |
   | cm⁻¹ | **interim:** `AxisKind::Feature` (or `Frequency`) + `unit:"cm-1"` + `coordinates` | **propose adding `Wavenumber`** to dag-ml-data (co-design) |
   | signal type | `SourceDescriptor.tags["signal_type"]` | dag-ml-data has no signal-type field |

2. **`dag-ml` campaign side** (`crates/dag-ml-core/src/{data,fold,campaign}.rs`) — folds/partitions live
   **here**, not in dag-ml-data:
   - `FoldSet { id, sample_ids, folds: [{fold_id, train_sample_ids, validation_sample_ids, metadata}],
     sample_groups }` ← built from the spec's folds **or** a partition column (NOT invented CV).
   - `DataBinding { node_id, input_name, request_id, schema_fingerprint, plan_fingerprint,
     relation_fingerprint?, output_representation, feature_set_id?, source_ids, require_relations,
     view_policy: DataViewPolicy, metadata }` ← references the dag-ml-data fingerprints.
   - `ExternalDataPlanEnvelope { schema_version=1, schema_fingerprint, plan_fingerprint,
     relation_fingerprint?, coordinator_relations? }` — **this is what `dag-ml` consumes** (it drops the
     plan body); `DataBinding::validate_envelope` requires all three fingerprints to match.
   - `DataViewPolicy { fit_partition, predict_partition, include_augmented_*, include_excluded,
     require_sample_ids, unsafe_flags }` — leakage-safe defaults; the bridge sets sane defaults only.

   **Lossy mapping (relation):** dag-ml-data `origin_id` = *observation* id; `dag-ml` core relation uses
   `origin_sample_id`. The bridge must **carry both** (or a resolver) so the observation→sample collapse
   under repetitions is not silently wrong. Resolve with dag-ml owners (story 4.3).

**Validation gate:** the emitted set must pass `dag-ml validate-data-binding` (CLI `ValidateDataBinding`)
end-to-end — not merely dag-ml-data schema validation (AC of story 4.4).

---

## Appendix I — Public API (proposed)

**Python (Phase 1):**
```python
import nirs4all_io as nio

plan = nio.infer("data/mango/", conventions=["nirs4all-classic"])   # -> DatasetPlan
print(plan.recommendations); plan.warnings                          # triage
ds = nio.load(plan, target="spectrodataset")                        # accept the plan → SpectroDataset
# or skip inference with an explicit spec:
ds = nio.load({"sources": [...], "folds": {...}}, target="spectrodataset")
spec = nio.DatasetSpec.from_yaml("dataset.yaml"); spec.validate()
# vendor corpus + reference table (headline new capability):
plan = nio.infer(["spectra/*.0", "reference.csv"], conventions=["vendor-corpus"])
```
`infer(input, *, conventions=None, hints=None) -> DatasetPlan` · `load(input|spec|plan, *,
target="spectrodataset"|"dag-ml-data", **overrides)` · `DatasetSpec` (`.from_dict/.from_yaml/.from_plan/
.validate/.to_dict`) · `DatasetPlan` (`.resolved_spec/.warnings/.recommendations/.accept(overrides)`).

**CLI:** `nirs4all-io infer DIR [--json]` · `nirs4all-io load SPEC --target …` · `nirs4all-io validate SPEC`.

**Rust (Phase 2):** `nirs4all_io::infer(input) -> DatasetPlan` · `…::to_dag_ml_data(spec) ->
(DatasetSchema, SampleRelationTable, fingerprints)` · `…::to_campaign_artifacts(spec) -> (FoldSet,
DataBinding, ExternalDataPlanEnvelope)`; Python/R/WASM bindings mirror `infer`/`load`. The DatasetSpec
JSON schema + scoring rules authored in Phase 1 are shared verbatim (cross-language gate, story 6.4).

---

## Appendix J — `dag-ml-data` readiness checklist (Phase-2 gate, blocks story 4.4)

Implement the Rust/dag-ml-data target only when these are green (status from the current code):

- [ ] **External construction path**: a Python package **or** a stable C ABI builder to construct +
      serialize `DatasetSchema`/`SampleRelationTable`/`DataPlan` from outside Rust. *Today: only a
      `ctypes` smoke + provider vtable; no Python package.* ❌
- [ ] **cm⁻¹ axis**: add `AxisKind::Wavenumber` **or** ratify the interim `Feature`+`unit:"cm-1"`
      convention in docs. *Today: no `Wavenumber` (`model.rs:8`).* ❌
- [ ] **Relation id mapping**: agreed handling of `origin_id` (obs) ↔ `origin_sample_id` (sample) and
      `repetition_id`. *Today: mismatch across repos.* ❌
- [ ] **Fingerprints exposed**: `schema/plan/relation` fingerprint fns callable from the bridge.
      *Today: implemented in dag-ml-data.* ✅
- [ ] **Array host path**: a production `NumericFeatureMatrixF64` / provider-vtable path the bridge can
      fill. *Today: in-memory test provider only.* ⚠️ partial
- [ ] **`dag-ml validate-data-binding`** reachable as the contract target. *Today: CLI exists
      (`ValidateDataBinding`).* ✅
- [ ] **Connector direction aligned**: dag-ml-data Roadmap Phase 4 ("SpectroDataset connector") and this
      bridge agree on ownership so we don't build conflicting bridges. ❌ (co-design)

---

## Appendix K — Test & acceptance strategy

- **Parity (P1, gates 4.1/4.2):** for each App. A item, assert `nio.load(spec, target="spectrodataset")`
  equals `DatasetConfigs(equivalent_legacy_config)` on `content_hash`, shapes, partitions, folds,
  headers/units, signal type, task type — with nirs4all imported **dev/test-only** as a read-only oracle.
  This **guards against drift** between io and the in-prod stacks (C6); **no deletion is in scope** (D5).
- **Inference (3.4/3.6):** labeled `samples/inference/` **split by vendor/domain**; report precision/
  recall per decision + Brier/ECE; assert abstention on the ambiguous fixtures.
- **Conventions (6.3):** `FolderParser`-parity fixtures + `vendor-corpus` + sidecar/archive/duplicate-stem
  + dropped-row-audit fixtures (C8).
- **dag-ml-data conformance (P2, 4.4):** emitted artifacts pass `dag-ml validate-data-binding`;
  fingerprints cross-checked; `Wavenumber`/relation gaps covered.
- **Cross-language goldens (6.4):** identical `DatasetPlan` JSON from Python and (later) Rust — the gate
  that authorizes the Rust-core extraction.
- **Adversarial/security (6.1/4.5):** ambiguous/missing/unaligned/huge-archive inputs → warnings, not
  crashes; memory/file-count/recursion bounds; provenance + content hashes round-trip into both targets.

---

## Appendix L — Dataset declaration cookbook (use cases)

The vocabulary is only useful if users can map *what they have on disk* to a declaration. Each case below
is a real on-disk situation → the exact `DatasetSpec` (YAML shown; dict/JSON equivalent). **Every element
of Appendices E.1/E.2 appears in ≥1 case** (story 2.4 gate). All cases are also `infer()`-able (the engine
proposes the spec with scores) — the explicit spec is what `accept`/`load` runs. Selectors are
first-match-wins; unmatched columns default to `features`.

### L.8 (flagship) — 3 CSVs merged, X & Y mixed in columns, indexed by a key → complete metadata lookup
*On disk:* `batch_a.csv`, `batch_b.csv`, `batch_c.csv` — each row a sample; columns are **mixed**:
wavelength columns (X), a few named target columns (Y), a `site_code` key, and some per-row metadata.
Plus `sites.csv` — a **small, complete** table with one row per site (region, soil, instrument…).
```yaml
name: mango_multi_batch
task_type: regression
sample_index: { by: row }                        # each measurement row is a sample (site_code is NOT the id)
sources:
  - id: measurements
    role: mixed                                  # column roles assigned below (X & Y interleaved is fine)
    input: [batch_a.csv, batch_b.csv, batch_c.csv]
    merge: concat_samples                        # stack the 3 files' rows into one sample axis (E.2)
    columns:                                     # ORDERED selectors; disjoint here (E.1)
      - { role: features, select: { regex: '^\d+(\.\d+)?$' } }   # wavelength-named cols (950, 951.5, …) → X
      - { role: targets,  select: [protein, moisture] }          # named cols → Y
      - { role: metadata, select: [site_code, date, operator] }  # site_code kept as metadata + join key
      - { role: ignore,   select: [notes] }
  - id: sites                                     # the "limited but complete" metadata dimension table
    kind: lookup                                  # not a sample source; contributes columns via the join
    input: sites.csv                              # 1 row per site_code
    columns: [ { role: metadata, select: rest } ] # all site cols broadcast AS METADATA (role honored)
    join: { left: measurements, right: sites, left_on: site_code, right_on: site_code,
            cardinality: m:1, coverage: complete }
```
*Mechanism:* `concat_samples` stacks the 3 files (sample identity = row, **not** `site_code`); `columns`
splits the mixed X/Y/metadata; the explicit join broadcasts each `sites.csv` row to every matching
measurement (**m:1**); `coverage: complete` **asserts every `site_code` resolves** (else error); a
duplicate `site_code` in `sites.csv` is a cardinality error. `nirs4all-io` performs the join itself
(E.2). Per-sample provenance records which batch file each row came from.

### L.1 — One CSV, all columns spectra, no targets (predict-only)
```yaml
sources: [{ id: x, role: features, input: spectra.csv, partition: predict,
            params: { has_header: true, header_unit: nm } }]
```
### L.2 — One CSV, spectra + last column is the target
```yaml
sources: [{ id: data, role: mixed, input: data.csv,
            columns: { features: '0:-1', targets: -1 } }]      # slice X, last col Y (E.1)
```
### L.3 — One CSV, X/Y/metadata mixed, identified by name & regex
```yaml
sources: [{ id: data, role: mixed, input: data.csv, key: id,
            columns: { index: id, features: { regex: '^\d' }, targets: [protein],
                       metadata: { dtype: string }, weights: [w] } }]
```
### L.4 — Three separate files X / Y / metadata, row-aligned (1:1)
```yaml
sources:
  - { id: x, role: features, input: X.csv }
  - { id: y, role: targets,  input: Y.csv, join: { to: x, how: '1:1' } }   # positional 1:1
  - { id: m, role: metadata, input: M.csv, join: { to: x, how: '1:1' } }
```
### L.5 — X.csv + Y.csv aligned by an id column (not row order)
```yaml
sample_index: { by: id, key: Sample_ID }
sources:
  - { id: x, role: features, input: X.csv, key: Sample_ID }
  - { id: y, role: targets,  input: Y.csv, key: Sample_ID,
      join: { to: x, on: Sample_ID, how: '1:1', coverage: complete } }
```
### L.6 — Many batch CSVs stacked (same schema)
```yaml
sources: [{ id: data, role: mixed, input: 'batches/*.csv', merge: concat_samples,
            columns: { features: { regex: '^\d' }, targets: [y] } }]
```
### L.7 — Two instrument blocks for the same samples, hstacked by key
```yaml
sources: [{ id: x, role: features, input: [nir.csv, mir.csv], merge: concat_features,
            key: id }]                                 # column blocks joined on id
```
### L.9 — Train/test as separate files (or just a folder + convention)
```yaml
sources:
  - { id: xtr, role: features, input: Xcal.csv, partition: train }
  - { id: ytr, role: targets,  input: Ycal.csv, partition: train, join: { to: xtr, how: '1:1' } }
  - { id: xte, role: features, input: Xval.csv, partition: test }
  - { id: yte, role: targets,  input: Yval.csv, partition: test, join: { to: xte, how: '1:1' } }
# equivalently: just point infer() at the folder with conventions:[nirs4all-classic]
```
### L.10 — One combined file with a split column
```yaml
sources: [{ id: data, role: mixed, input: all.csv,
            columns: { features: { regex: '^\d' }, targets: [y], metadata: [set] } }]
partitions: { by: column, column: set, train_values: [cal], test_values: [val] }
```
### L.11 — Percentage split, stratified
```yaml
sources: [{ id: data, role: mixed, input: all.csv, columns: { features: '0:-1', targets: -1 } }]
partitions: { by: percentage, train: '80%', test: '20%', shuffle: true, random_state: 0, stratify: y }
```
### L.12 — Predefined CV folds (file or column)
```yaml
sources: [{ id: x, role: features, input: X.csv }, { id: y, role: targets, input: Y.csv }]
folds: { file: folds.csv, format: auto }          # or { column: cv_fold } on a combined file
```
### L.13 — Multi-source (NIR + chemical markers), shared targets, joined by id
```yaml
sample_index: { by: id, key: id }
sources:
  - { id: nir,     role: features, modality: spectroscopy, input: nir.csv,     key: id }
  - { id: markers, role: features, modality: markers,      input: markers.csv, key: id,
      join: { to: nir, on: id, how: '1:1' } }
  - { id: y,       role: targets,  input: targets.csv, key: id,
      join: { to: nir, on: id, how: '1:1', coverage: complete } }
```
### L.14 — Folder of vendor spectra (OPUS) + a reference table (vendor-corpus)
*On disk:* `spectra/*.0` (Bruker OPUS, one file per sample) + `reference.csv` keyed by filename stem.
```yaml
conventions: [vendor-corpus]
sources:
  - { id: spectra, role: features, input: 'spectra/*.0' }          # read by nirs4all-formats
  - { id: ref, role: mixed, input: reference.csv, kind: lookup,
      columns: { index: sample, targets: [protein], metadata: [variety] },
      join: { to: spectra, on: filename_stem, how: 'm:1', coverage: warn } }
```
### L.15 — Repeated measurements (reps): per-rep spectra + sample-level metadata, then aggregate
```yaml
sample_index: { by: id, key: sample_id, repetition_id: scan_id }
sources:
  - { id: scans, role: features, input: scans.csv, key: sample_id }   # N rows per sample
  - { id: meta,  role: metadata, kind: lookup, input: samples.csv,
      join: { to: scans, on: sample_id, how: 'm:1', coverage: complete } }           # 1 row per sample
repetition: sample_id
aggregate: { by: sample_id, method: median }
```
### L.16 — Excel workbook: X on one sheet, Y on another
```yaml
sources:
  - { id: x, role: features, input: book.xlsx, params: { format: { sheet_name: spectra } } }
  - { id: y, role: targets,  input: book.xlsx, params: { format: { sheet_name: refs } },
      join: { to: x, how: '1:1' } }
```
### L.17 — NumPy arrays + a metadata CSV
```yaml
sources:
  - { id: x, role: features, input: X.npy }
  - { id: y, role: targets,  input: y.npy,  join: { to: x, how: '1:1' } }
  - { id: m, role: metadata, input: meta.csv, join: { to: x, how: '1:1' } }
```
### L.18 — Feature gallery (exercises the remaining vocabulary; one fixture each)
```yaml
# (a) name_range + rest + strict_columns; (b) auto with candidates; (c) by_key merge of 3 files;
# (d) coverage: drop with audit; (e) coverage: error; (f) 1:m expansion.
# a) contiguous wavelength block by header name, everything else is metadata, overlaps are errors:
sources: [{ id: a, role: mixed, input: wide.csv, strict_columns: true, columns: [
  { role: features, select: { name_range: ['400','2500'] } },
  { role: targets,  select: [protein] },
  { role: metadata, select: rest } ] }]
# b) unknown layout → let inference decide per column, but only among these roles (abstains if unsure):
# columns: [ { role: auto, select: '*', candidates: [features, targets, metadata] } ]
# c) by_key merge of 3 partial files sharing `id`:
# sources: [{ id: data, input: [a.csv,b.csv,c.csv], key: id, merge: by_key }]
# d) lookup but tolerate+drop unmatched samples (audited):  join: { ..., cardinality: m:1, coverage: drop }
# e) strict reference:                                       join: { ..., cardinality: 1:1, coverage: error }
# f) one sample row → many time-points:                     join: { ..., cardinality: 1:m, coverage: complete }
```

> **Coverage matrix — generated from fixtures in CI, not hand-asserted (story 2.4 gate; Codex).** A CI
> check parses every `samples/cookbook/*` fixture, records which selector/merge/join/coverage element it
> exercises, and **fails if any vocabulary element from E.1/E.2 has zero fixtures**. Current cases map as:
> L.1/L.2 (slice, predict-only) · L.3 (regex/name/dtype/weights/key) · L.6 (concat_samples glob) ·
> L.7 (concat_features) · L.4/L.5/L.13/L.16/L.17 (1:1 by row & by key) · L.8/L.14/L.15 (m:1 lookup;
> coverage complete/warn; roles-honored broadcast) · L.9/L.10/L.11/L.12 (partitions, folds) · **L.18**
> (name_range, rest, strict_columns, auto, by_key, coverage drop/error, 1:m). The matrix in this doc is
> illustrative; **the CI-generated one is authoritative** — anything with zero fixtures is *unshipped*.
