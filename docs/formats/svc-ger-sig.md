# SVC / GER SIG

> **Status:** Supported (scoped) · **Vendor:** Spectra Vista (SVC) / GER · **Extensions:** `.sig`

`.sig` is the ASCII export written by Spectra Vista Corporation (and legacy GER)
field spectrometers — HR-1024i, GER 3700 and related VNIR/SWIR instruments. Each
file pairs a reference and target acquisition and computes a reflectance
channel. nirs4all-formats reads the three spectral columns and promotes the rich SVC
acquisition header (instrument, foreoptic, per-detector settings, radiometric
factors and overlap policy) into canonical metadata.

## Instruments & software

Produced by SVC's HR-1024i software and by older GER instruments (GER 3700).
Both PDA-style and laptop-style headers are handled. The committed corpus covers
Acer-PDA, laptop-firmware, GER 3700 PDA and HR-1024i field exports.

## File structure

- A `Spectra Vista SIG Data` header block of `key=value` lines (`instrument`,
  `optic`, `integration`, `scan coadds`, `temp`, `battery`, `factors`, `time`,
  `latitude`/`longitude`, `gpstime`, `units`, `comm`, etc.), then a literal
  `data=` line.
- The numeric block follows `data=`: whitespace-separated rows of four columns —
  wavelength, reference, target and reflectance.

## What nirs4all-formats extracts

- **Signals** — three per record: `reference`, `target` and `reflectance`. The
  `reflectance` channel carries unit `%`; reference/target are typed from the
  `units=` header (radiance) with `unit=null` because SVC firmware exposes
  uncalibrated DN-derived radiance.
- **Axis** — a wavelength axis in `nm`.
- **Metadata** — the raw header under `metadata.vendor`, plus promoted canonical
  fields: `instrument_model`/`instrument_serial` (from `HI: <serial>
  (<model>)`), `foreoptic` (two strings), per-detector Si/InGaAs1/InGaAs2
  integration times, coadds and temperatures split into reference/target,
  `battery_voltages_volts`, `error_codes`, `memory_slots`, `source_signal_units`
  (from `units`), acquisition start/end date and time, GPS latitude/longitude
  and GPS time, and — parsed from the `factors=` bracket —
  `radiometric_factors` (three floats), `overlap_policy` (`preserve`/`remove`),
  `matching_type` and `overlap_break_wavelengths_nm`.
- **Quality flags** — `matched_overlap_corrected`, `overlap_removed`,
  `detector_overlap_preserved`, `resampled_export` (from the overlap policy,
  `comm=` or a `_resamp` filename), `white_reference` (`_WR_` filename
  convention) and `declared_bad_fixture` (with a matching provenance warning).

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Acer-PDA exports (clean / white-reference) | Supported | 1024-point spectra; reflectance anchors and acquisition metadata asserted. |
| Laptop-firmware exports | Supported | Reference/target/reflectance plus `detector_overlap_preserved`. |
| Matched-overlap-corrected export | Supported | `overlap_break_wavelengths_nm` and `matching_type` parsed. |
| GER 3700 PDA | Supported | Parsed `radiometric_factors` and overlap breakpoints. |
| HR-1024i field / resampled export | Supported | Resampling, overlap removal and the 350–2500 nm / 1 nm axis covered. |
| HR-1024i firmware ≥ 3.0 | Partial | Needs more independent samples beyond the spectrolab / R-FieldSpectra corpora. |

## Limitations & known gaps

- Reference/target channels are typed `radiance` from the `units=` header but
  carry `unit=null`; a redistributable calibrated `.sig` would let a real
  spectral-radiance unit be promoted on top of the `radiometric_factors`
  triplet.
- Per-detector spectral ranges (Si/InGaAs1/InGaAs2 split points) are not
  surfaced; they are implicit in `overlap_break_wavelengths_nm` only for
  overlap-removed files.
- HR-1024i firmware ≥ 3.0 variants and historical GER 1500 files still need
  independent samples.

## Reference readers

Compared full-array against `spectrolab` (R subprocess) in `tests/conformance/`;
`specdal` is an additional reference candidate. The promoted metadata surface
mirrors what `spectrolab`/`specdal` expose (instrument, foreoptic, integration,
coadds, temperatures, battery, error codes, factors and overlap policy), so the
comparison can stay text-only.

## Samples & validation

Fifteen fixtures under `samples/svc_ger/` are golden-backed in
`crates/nirs4all-formats/tests/goldens/` with direct semantic assertions covering the
Acer-PDA, laptop, matched-overlap-corrected, two declared-bad, GER 3700 PDA and
HR-1024i Barrow variants. Control values include
`BNL13001_000_moc.sig` (`overlap_break_wavelengths_nm = [970, 1901]`,
`matching_type = "Radiance @ 976 - 1010 / NIR-SWIR On"`),
`serbinsh_gr070214_003.sig` (`overlap_break_wavelengths_nm = [985, 1906]`) and
`serbinsh_BEO_CakeEater_Pheno_026_resamp.sig` (`LENS 4(1)` foreoptic, 350–2500 nm
/ 1 nm axis). The conformance suite compares the reflectance signal against
`spectrolab` with a `1e-6` relative tolerance. The probe reports format
`svc-ger-sig` at `Confidence::Definite`.
