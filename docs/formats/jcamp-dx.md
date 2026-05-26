# JCAMP-DX

> **Status:** Supported (scoped) · **Vendor:** Vendor-neutral / IUPAC · **Extensions:** `.jdx`, `.dx`, `.jcm`, `.jcamp`

JCAMP-DX is the IUPAC labelled-data interchange format for IR, NIR, Raman, UV-Vis
and NMR spectra. nirs4all-io ships a native Rust reader for the high-value
spectroscopic shapes: dense `XYDATA` tables (plain and packed), `XYPOINTS`,
`NTUPLES` pages, sparse `PEAK TABLE` / `PEAK ASSIGNMENTS`, top-level multi-block
files and `DATA TYPE=LINK` containers.

## Instruments & software

JCAMP-DX is written as an export by a wide range of spectrometers and chemometric
software rather than being tied to one vendor. Committed fixtures cover NIST
WebBook IR exports, Bruker test spectra, IUPAC NMR `NTUPLES` examples and an Ocean
Optics SpectraSuite `LINK` export.

## File structure

A JCAMP-DX file is plain ASCII organised as `##KEY=value` labelled-data records
(LDRs) terminated by `##END=`. The reader sniffs any file containing
`##JCAMP-DX=` or `##JCAMPDX=` and dispatches on the data shape:

- **`XYDATA=(X++(Y..Y))`** — the X axis is reconstructed from `FIRSTX` and
  `DELTAX`, or from `FIRSTX`, `LASTX` and `NPOINTS` when `DELTAX` is absent.
  Each data line begins with an X checkpoint.
- **`XYPOINTS=(XY..XY)`** — explicit X/Y pairs are read directly rather than
  reconstructing the axis.
- **`NTUPLES`** — multi-page tables; the NMR real/imaginary layout
  (`SYMBOL=X,R,I,N`, `VAR_TYPE=INDEPENDENT,DEPENDENT,DEPENDENT,PAGE`) emits the
  real and imaginary channels as separate signals on one record.
- **`PEAK TABLE` / `PEAK ASSIGNMENTS`** — sparse peak lists (see below).
- **`DATA TYPE=LINK`** and top-level multi-block files — one or several records.

Ordinate encodings supported inside `XYDATA`/`NTUPLES` lines: plain AFFN rows,
PAC adjacent signed numbers, SQZ pseudo-digits, DIF difference coding and DUP
repeat counts. `YFACTOR` scales ordinates; `XFACTOR` scales peak abscissas and
widths inside peak tables.

## What nirs4all-io extracts

- **Signals** — one or more `SpectralRecord`s. `XYDATA`/`XYPOINTS` produce a
  single `signal`; `NTUPLES` produces named channels (`real`, `imaginary`, …);
  peak blocks produce one `peak_intensity` signal. Signal type is mapped from
  `YUNITS` (Absorbance / Transmittance / Reflectance, else `Unknown`).
- **Axis** — values plus unit/kind from `XUNITS` (or `NTUPLES` `UNITS`):
  wavenumber `cm-1`, wavelength `nm`/`um`, frequency `hz`, energy `eV`, time `s`,
  otherwise `index`. Native axis order is preserved.
- **LINK containers** — two modes. *Composite (same-axis)*: when every child
  reuses one axis the reader emits a single record whose signals are the
  children (the Ocean Optics flow: `sample`, `dark_reference`, `white_reference`
  plus a computed `processed` transmittance, `(sample - dark) / (white - dark) *
  100`). *Fan-out (heterogeneous axes)*: one record per child, each carrying
  `link_parent_id`, `link_index`, `link_total` and a `link_relation`
  (`sample`/`dark`/`reference`/`interferogram`/`fid`/`peaks`/`unknown`) inferred
  from `DATA TYPE` or `TITLE`. Top-level multi-block files behave like fan-out and
  also keep the legacy `jcamp_block_index`.
- **Metadata** — all LDRs are preserved under `metadata.jcamp`; the full per-peak
  list is exported under `jcamp_peak_table`.
- **Provenance & warnings** — source file + SHA-256, reader name/version, and
  structured warnings (see below).

### Peak tables

Sparse `PEAK TABLE` / `PEAK ASSIGNMENTS` blocks become a single `peak_intensity`
signal whose axis carries the listed peak abscissas in their native order (so
`SpectralAxis.order` may be ascending, descending or non-monotonic). Shapes
`(XY..XY)`, `(XYW..XYW)`, `(XYM..XYM)`, `(XYA)`, `(XYWA)`, `(XYMA)` and
`DATA TABLE=(XY..XY), PEAK …` are parsed; assignment text is the first `<…>`
substring on a line. When both a `PEAK TABLE` and `PEAK ASSIGNMENTS` exist in one
block the richer ASSIGNMENTS form wins and the other is preserved under
`jcamp_peak_table_dropped`.

### X-checkpoint verification

For `XYDATA`, line-start X values are checked against the reconstructed axis,
accepting either physical checkpoints or checkpoints that require `XFACTOR`. On
mismatch the reader emits a structured `jcamp_xydata_x_checkpoint_drift` warning
carrying the absolute and relative drift at the first mismatch so downstream tools
can act on it rather than parse a free-form string. (In the committed Bruker
fixtures `FIRSTX`/`DELTAX` are physical while line checkpoints are scaled
integers.)

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `XYDATA` AFFN / PAC / SQZ / DIF / DUP | Supported | X axis reconstructed; checkpoints verified. |
| `XYPOINTS=(XY..XY)` | Supported | Explicit X/Y pairs. |
| `NTUPLES` NMR real/imaginary, FID | Supported | Channels split into signals; `SECONDS` axis typed as time. |
| Top-level multi-block files | Supported | One record per block with `link_*` metadata. |
| `LINK` composite (same axis) | Supported | Ocean Optics flow; computed transmittance. |
| `LINK` fan-out (heterogeneous axes) | Supported | One record per child via `read_bytes`. |
| `PEAK TABLE` / `PEAK ASSIGNMENTS` (top-level) | Supported | All documented JCAMP-DX 5.0 shapes parsed. |
| `PEAK TABLE` children inside `LINK` | Refused | Mixing dense + sparse on one record has no clear semantics; use a standalone peak file. |
| General real-world `LINK` semantics | Partial | Beyond Ocean Optics, generic LINK shapes still need scoping. |

## Limitations & known gaps

- For `XYDATA`/`NTUPLES`, `XFACTOR` is preserved in metadata but not applied to
  the reconstructed axis.
- When `NPOINTS` declares more points than can be decoded the file is rejected as
  malformed; when it declares fewer, the record is truncated with a warning.
- Peak-table shape parsing covers all documented JCAMP-DX 5.0 variants, but only a
  synthetic peak fixture is committed; real vendor peak tables (embedded `>`,
  multi-line assignments, `;`-separated groups) are still wanted.
- Generic multi-block `LINK` files with heterogeneous semantics, and `LINK`s that
  combine dense spectra with peak tables, are not yet handled.
- The Ocean Optics zero-denominator case has no missing-value marker in the data
  model: those points are set to `0.0` with a provenance warning.

## Reference readers

Cross-checked conceptually against `jcamp` (`jcamp.jcamp_readfile`, wired into the
conformance harness), SpectroChemPy, `nmrglue`, ChemoSpec and hyperSpec. The
current reader is intentionally narrower than these mature libraries, prioritising
the high-value NIR/IR `XYDATA` cases.

## Samples & validation

Fixtures live under `samples/jcamp_dx/` and are covered by golden summaries in
`crates/nirs4all-io/tests/goldens/`; the probe reports format `jcamp-dx` at
`Confidence::Definite`. Committed control values:

| File | Encoding | Points | Axis | Value control |
|---|---|---:|---|---|
| `nist_water_ir.jdx` | plain AFFN | 3917 | `388.677 → 3799.45426 cm-1` | `0.438 → 0.885` |
| `nist_sucrose_ir.jdx` | two top-level `XYDATA` blocks | 7153 × 2 | `7498.994 → 600.88399 cm-1` | reflectance first values `0.422011`, `0.471453` |
| `BRUKSQZ.DX` | SQZ | 16384 | `24038.5 → 0.0 Hz` | `2259260 → 1505988` |
| `BRUKDIF.DX` | DIF/DUP | 16384 | `24038.5 → 0.0 Hz` | `2254931 → 1513177` |
| `SPECFILE.DX` | mixed SQZ/DIF/DUP | 1801 | `400.0 → 4000.0 cm-1` | `97.737187 → 82.830985` |
| `BRUKNTUP.DX` | `NTUPLES` R/I pages | 16384 × 2 | `24038.5 → 0.0 Hz` | real `2254931 → 1513177`, imaginary `-6966283 → -7303022` |
| `TESTFID.DX` | `NTUPLES` FID R/I pages | 16384 × 2 | `0.0 → 0.6815317 s` | real `2979.837825 → -60241.607962`, imaginary `6214.555864 → -6063.227393` |
| `OceanOptics_period.jdx` | `LINK` + `XYPOINTS` | 3648 × 4 | `176.36 → 893.69 nm` | computed transmittance `0.0 → 171.977070` |
| `synthetic_peak_assignments.jdx` | `PEAK ASSIGNMENTS (XYA)` | 4 peaks | `3300 → 1050 cm-1` | absorbance `0.42 → 0.55`, sum `2.00` |

Full-array conformance reports against open JCAMP readers for every committed
fixture are tracked in `docs/CONFORMANCE.md`.
