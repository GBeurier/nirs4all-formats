# Si-Ware NeoSpectra

> **Status:** Supported (scoped) · **Vendor:** Si-Ware · **Extensions:** `.csv`, `.xlsx`

Si-Ware NeoSpectra is a handheld MEMS FT-NIR spectrometer family. NeoSpectra data
currently enters nirs4all-io through open export layouts rather than a native
project container, so there is **no dedicated NeoSpectra reader**: the coverage
is split across the generic tabular readers. The single-measurement Scanner
export is not yet covered.

## Instruments & software

Si-Ware NeoSpectra Scanner and NeoSpectra-Micro handheld spectrometers, and the
NeoSpectra slices distributed in the Open Soil Spectral Library (OSSL,
Woodwell/KSSL) and forensic research datasets.

## File structure

Three open layouts are recognised, each routed to an existing reader:

- **Axis-first CSV** — wavelength column then values; read by the
  [row-spectral-table](row-spectral-table.md) reader.
- **Wide OSSL CSV** — metadata/target columns before numeric wavelength headers,
  one spectrum per row; read by the wide [`csv_like`](text-readers-001.md)
  reader.
- **First-row-wavelength `.xlsx`** — an `axis`/`data` descriptor in the first
  cell and wavelengths along the first row; read by the [Excel](excel.md)
  reader.

## What nirs4all-io extracts

- **Signals** — `absorbance` (or the column-declared signal) over a wavelength
  axis in `nm`, one record per spectrum row.
- **Sample identity** — OSSL `id.layer_uuid_txt` is promoted to
  `metadata.sample_id`.
- **Targets** — soil/site laboratory properties are preserved as `targets` when
  the cells are numeric.
- The OSSL column-name descriptor (schema-only, no spectral axis) is refused
  explicitly rather than misread.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Axis-first CSV export | Supported | Via row-spectral-table. |
| Wide OSSL CSV matrix | Supported | Via `csv_like`; empty rows ignored, `id.layer_uuid_txt` → `sample_id`. |
| First-row-wavelength `.xlsx` export | Supported | Via the Excel reader; `axis: wavelength (nm) / data: absorbance (a.u.)`. |
| OSSL column-name descriptor | Detected / refused | Documents the schema but carries no spectral axis. |
| Native single-measurement Scanner export | Blocked | No "one measurement per file" fixture has been found. |

## Limitations & known gaps

- A native single-measurement NeoSpectra Scanner export fixture is missing; only
  wide OSSL-style matrices and Excel exports are covered.
- Validation across all OSSL scanner/replicate blocks is still partial.
- Soil-chemistry target names are preserved verbatim, not yet typed/normalised.

## Reference readers

Generic CSV/Excel tooling (`pandas`, `openpyxl`); no dedicated reference reader.

## Samples & validation

Fixtures under `samples/siware_neospectra/` are golden-backed in
`crates/nirs4all-io/tests/goldens/`:
`synthetic_neospectra.csv` (1 record, axis-first, 200-point `nm` axis),
`neospectra_ossl_50samples_slice.csv` (24 records, real OSSL Woodwell/KSSL
slice, 601-point `nm` axis) and `neospectra_forensic_K_avg.xlsx` (88 records,
real UvA forensic averaged spectra, 160-point `nm` axis). OSSL data is
CC-BY-4.0 (Zenodo 13122321); the forensic export is from Figshare/UvA. The
`neospectra_ossl_column_names.csv` descriptor is a tested expected refusal.
