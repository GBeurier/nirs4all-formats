# Spectral Evolution / PSR SED

> **Status:** Supported (scoped) · **Vendor:** Spectral Evolution · **Extensions:** `.sed`

`.sed` is the ASCII export written by Spectral Evolution field spectrometers
(PSR and SR series) for reflectance and raw-DN measurements across the
VNIR/SWIR range. The file is a key/value header followed by a `Data:` table
whose first column is wavelength. nirs4all-io parses the table, types each
signal column and promotes the field-acquisition header into canonical
metadata.

## Instruments & software

Produced by Spectral Evolution's DARWin software for PSR-1100/2500/3500 and
SR-1900/3500/6500 handheld spectrometers. The committed corpus is drawn from the
PSR-3500 family; SR-series firmware variants remain under-sampled.

## File structure

- A key/value header (e.g. `Version:`, `Instrument:`, `Measurement:`,
  `Channels:`, `Wavelength Range:`, GPS and detector lines), then a literal
  `Data:` line.
- The line after `Data:` is the column header; the first column is `Wvl` and
  the remaining columns are value channels (DN reference/target, `Reflect. %`
  or `Reflect. [1.0]`).
- The numeric block follows, one wavelength per row.

## What nirs4all-io extracts

- **Signals** — one signal per value column after `Wvl`. Reflectance columns are
  typed `reflectance`; DN / reference / target columns are typed `raw_counts`.
- **Axis** — a wavelength axis in `nm`.
- **Units** — inferred from column labels: `DN` for normalized DN columns, `%`
  for `Reflect. %`, and `1` for `Reflect. [1.0]`.
- **Metadata** — the raw header under `metadata.vendor`, plus promoted
  canonical fields: instrument/model/serial, measurement mode, radiometric
  calibration, declared point count, wavelength range, source signal
  labels/units, detector channels, reference/target detector temperatures and
  integration times, battery voltages, scan averages, dark mode, foreoptic and
  its signal units, GPS latitude/longitude/altitude, GPS time, satellite counts
  and acquisition start/end date and time.
- **Warnings & quality flags** — `sed_missing_reflectance_signal` (plus a
  `missing_reflectance_signal` quality flag) when only DN channels are present;
  `sed_point_count_mismatch` and `sed_column_count_mismatch` when declared
  counts disagree with the parsed table.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| PSR+3500 DN + reflectance export | Supported | Raw DN reference/target plus reflectance, 2151-point axis. |
| PSR-3500 reflectance acquisition | Supported | Firmware/header drift, GPS/date/time and colon-style foreoptic parsing. |
| DN-only export (no reflectance) | Supported | Read as raw channels; flagged, not promoted to reflectance. |
| SR-3500 / SR-6500 firmware | Partial | Firmware-specific headers under-covered; more samples wanted. |

## Limitations & known gaps

- The reader does not reconstruct reflectance from DN-only acquisitions; the
  `sed_missing_reflectance_signal` warning is left for downstream handling.
- Signal-unit inference is limited to the column labels observed in committed
  fixtures.
- SR-series firmware headers and explicit calibrated radiance/irradiance units
  still need redistributable samples.

## Reference readers

Compared full-array against `spectrolab` (R subprocess) in `tests/conformance/`;
`specdal` is an additional reference candidate.

## Samples & validation

Three fixtures under `samples/spectral_evolution/` are golden-backed in
`crates/nirs4all-io/tests/goldens/`: `1566060_09506_working.sed` (PSR+3500 DN +
reflectance), `1566060_15025_not_working.sed` (broken-but-valid DN-only export
carrying the `missing_reflectance_signal` flag) and
`serbinsh_cvars_grape_leaf.sed` (PSR-3500 grape-leaf acquisition with canonical
GPS/date/time metadata). The conformance suite (`pytest -m conformance`)
compares the reflectance signal against `spectrolab` with a `1e-6` relative
tolerance on axis and values. The probe reports format `spectral-evolution-sed`
at `Confidence::Definite`.
