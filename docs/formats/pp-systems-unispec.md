# PP Systems UniSpec SC / DC

> **Status:** Experimental · **Vendor:** PP Systems · **Extensions:** `.SPT` (UniSpec SC), `.SPU` (UniSpec DC)

The PP Systems UniSpec SC (single-channel) and UniSpec DC (dual-channel) are
field spectroradiometers. When their `.SPT` / `.SPU` exports expose an axis-first
ASCII table, nirs4all-formats reads them through the
[row-spectral-table reader](row-spectral-table.md). The current fixtures are
synthetic, so the coverage is scoped until a real field acquisition can validate
production headers, units and metadata.

## Instruments & software

Produced by PP Systems UniSpec SC / DC instruments and their host software.
`.SPT` files come from the single-channel SC; `.SPU` files from the dual-channel
DC (two radiometer channels). A separate `pp_systems` reader recognises — and
deliberately refuses — Arctic LTER UniSpec-DC vegetation-index products, which
are derived summaries rather than raw spectra.

## File structure

An optional header preamble of key/value lines (e.g. `File`, `Date`, `Notes`),
then an axis-first table: an explicit wavelength column followed by numeric
signal columns. The reader requires that explicit axis column and does not claim
arbitrary PP Systems reports by extension alone.

## What nirs4all-formats extracts

- **Signals** — `DN` columns are emitted as `raw_counts`; `Reflectance` columns
  as reflectance. UniSpec SC exposes `dn_white`, `dn_target`, `reflectance`;
  UniSpec DC exposes `channel_a_dn`, `channel_b_dn`, `reflectance`.
- **Axis** — wavelength in `nm`.
- **Metadata** — header key/value lines (`File`, `Date`, `Notes`, …) are
  preserved under `metadata.vendor`.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| UniSpec SC `.SPT` (axis-first ASCII) | Experimental | Synthetic fixture; real field acquisition wanted. |
| UniSpec DC `.SPU` (axis-first ASCII, two channels) | Experimental | Synthetic fixture; real two-channel acquisition wanted. |
| Arctic LTER UniSpec-DC index products (CSV/XLSX) | Detected / refused | Derived NDVI/EVI/PRI/WBI/Chl/LAI summaries, not raw spectra. |

## Limitations & known gaps

- The committed `.SPT` / `.SPU` fixtures are synthetic, so production headers,
  units and instrument metadata are not yet validated against a real field
  export.
- The local Arctic LTER UniSpec-DC CSV/XLSX files are vegetation-index products,
  not raw `.SPT/.SPU` spectra; the `pp_systems` reader refuses them with the
  dedicated `pp-systems-unispec-derived-indices` diagnostic and points to a
  separate reflectance data-scan file that is not present in
  `samples_local/pp_systems/`.
- PP Systems acquisition metadata is preserved verbatim but not normalized into
  typed fields.

## Reference readers

The axis-first exports are readable with `pandas` or R `read.table`;
nirs4all-formats adds axis detection, signal typing and provenance. A comparison
against SPECCHIO or another trusted import path is planned if one becomes
available.

## Samples & validation

Fixtures under `samples/pp_systems/` are golden-backed / semantic-tested on the
`nm` axis: `synthetic_unispec.SPT` (UniSpec SC, 1 record, 200 points,
`dn_white` / `dn_target` / `reflectance`) and `synthetic_unispec_dc.SPU`
(UniSpec DC, 1 record, 200 points, `channel_a_dn` / `channel_b_dn` /
`reflectance`). The Arctic LTER index products in `samples_local/pp_systems/`
are expected refusals via `pp-systems-unispec-derived-indices` and do not change
the raw UniSpec coverage status.
