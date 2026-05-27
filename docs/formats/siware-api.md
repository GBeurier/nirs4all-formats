# Si-Ware API JSON / CSV

> **Status:** Partial · **Vendor:** Spectro Inc. / Si-Ware · **Extensions:** `.json`, `.csv`

The Si-Ware API export is the cloud/API payload produced by Spectro Inc.
NeoSpectra-style workflows: a single-measurement JSON document carrying a
wavelength axis and an absorbance spectrum, optionally with model predictions
and instrument metadata. The dedicated reader handles the JSON payload; the
companion CSV stream is read through the generic axis-first reader.

## Instruments & software

Spectro Inc. / Si-Ware NeoSpectra handheld FT-NIR cloud API responses. The
committed fixtures are synthetic stand-ins for a real credentialed response.

## File structure

- **JSON** — a `measurement` object with numeric `wavelengths` and `absorbance`
  arrays (and optional `wavelength_units`), an optional `instrument` object
  (`vendor`/`model`/`serial`), an optional `predictions` object, and optional
  measurement metadata (id, timestamp, operator, GPS, temperature, humidity).
- **CSV** — an axis-first table (wavelength column followed by absorbance),
  routed to the [row-spectral-table](row-spectral-table.md) reader.

## What nirs4all-formats extracts

- **Signals** — one `absorbance` signal (typed `Absorbance`).
- **Axis** — a wavelength axis; the unit is taken from
  `measurement.wavelength_units`, defaulting to `nm`.
- **Targets** — numeric entries from the `predictions` object are promoted to
  `targets` (e.g. `protein`, `moisture`).
- **Metadata** — instrument vendor/model/serial, measurement id, timestamp,
  operator, GPS latitude/longitude and simple environmental fields (temperature,
  humidity) are preserved as record metadata.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Single-measurement JSON (`measurement.wavelengths` + `measurement.absorbance`) | Partial | Synthetic fixture only; dedicated reader claims this schema. |
| Companion CSV stream | Supported | Axis-first export; read by row-spectral-table, comment metadata preserved under `metadata.notes`. |

## Limitations & known gaps

- Both committed fixtures are synthetic. A real credentialed API response,
  schema-drift examples and a reference comparison for unit labels, predictions
  and optional metadata fields are still needed before broad release (matrix:
  *utile incomplet*).

## Reference readers

Standard JSON/CSV tooling; no dedicated reference reader.

## Samples & validation

`samples/siware_api/synthetic_siware_api.json` (1 record, `absorbance`, 200-point
`nm` axis, `protein`/`moisture` targets) is golden-backed in
`crates/nirs4all-formats/tests/goldens/` (`siware_api_json.summary.json`); the
companion `synthetic_siware_api.csv` is golden-backed via the row-spectral-table
reader (`row_spectral_table_siware_api_csv.summary.json`). The JSON probe
reports format `siware-api-json` at `Confidence::Definite`.
