# Row-Oriented Spectral Tables

> **Status:** Supported · **Vendor:** Generic / instrument text exports · **Extensions:** `.csv`, `.tsv`, `.txt`, `.dat`, `.asc`, `.SPT`, `.SPU`

Many instruments and conversion tools export a spectrum as a text table where
**each row is one spectral point** and the **first column is the spectral axis**
(wavelength, wavenumber or index), followed by one or more value columns. This
reader handles that "axis-first" orientation. Its sibling, the
[delimited-text reader](text-readers-001.md), handles the opposite layout — one
spectrum per row with numeric spectral headers.

## Instruments & software

This is a vendor-neutral text reader. It is the recommended path whenever an
instrument or its software can export an axis-first table. Committed fixtures
come from Si-Ware NeoSpectra, PP Systems UniSpec SC/DC, JASCO and Shimadzu text
exports, ENVI/ECOSTRESS/ASTER spectrum text, USGS SPECPR ASCII, WiTec ASCII and
MODTRAN albedo output.

## File structure

A short optional metadata/comment preamble, then a numeric block whose first
column is the axis. The reader recognises the axis from any of:

- an explicit axis header such as `Wavelength_nm`, `WAVELENGTH_um`,
  `Wavelength`, `X-Axis` or `wavenumber`;
- a comment-prefixed header such as `; Wavelength S000 S001`;
- a metadata descriptor such as `First Column: X` / `X Units`, including
  JASCO-style `XUNITS` / `YUNITS` followed by `XYDATA`.

The delimiter (comma, tab or whitespace) is auto-detected and the native axis
order (ascending or descending) is preserved.

## What nirs4all-io extracts

- **Signals** — one signal per numeric column after the axis, named from the
  column header (e.g. `absorbance`, `dn_white`, `reflectance`, `s000`). The
  signal type is inferred from the label when possible, otherwise `Unknown`.
- **Axis** — values and unit from the first column; the axis kind
  (Wavelength/Wavenumber/Index) follows the declared unit.
- **Metadata** — vendor key/value lines are preserved under `metadata.vendor`.
- **Provenance** — source file + SHA-256, reader name and version.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| Explicit axis header (`.csv`/`.tsv`/`.txt`) | Supported | One signal per value column. |
| Whitespace `.dat` (e.g. MODTRAN albedo) | Supported | Axis in `um`, value mapped to reflectance/albedo. |
| PP Systems UniSpec SC `.SPT` / DC `.SPU` export | Supported (synthetic fixtures) | Two-channel + reflectance; field acquisitions still wanted. |
| ECOSTRESS / ASTER / ENVI `*.spectrum.txt` | Supported | Metadata-described columns. |
| JASCO / Shimadzu / WiTec text export | Supported | Routed here rather than to the binary vendor reader. |

## Limitations & known gaps

- Single-column spectral libraries are **not** handled here. The legacy USGS
  `AREF` one-column dump is read by the dedicated
  [`usgs-aref-single-column`](usgs-speclib.md) reader with a generated index
  axis, because the file embeds no wavelengths.
- The sniffer is intentionally content-based: it does not claim matrix-style
  calibration tables, target-only reports, or arbitrary two-column CSV files
  without an axis header. Headerless two-column Ocean Optics CSV stays with the
  [Ocean Optics reader](ocean-optics.md).
- Deleted-value sentinels are preserved numerically; a masking policy is still
  pending in the shared data model.
- Vendor metadata is preserved verbatim but not yet normalised into typed
  fields.

## Reference readers

`pandas.read_csv`, R `read.table`, and the `nirs4all` `CSVLoader` read the same
exports; nirs4all-io adds axis detection, signal typing and provenance.

## Samples & validation

Fixtures live under `samples/` (Si-Ware, PP Systems, ENVI/ECOSTRESS, JASCO,
Shimadzu, USGS SPECPR, WiTec, MODTRAN) and are covered by golden summaries in
`crates/nirs4all-io/tests/goldens/`. The probe reports format
`row-spectral-table` at `Confidence::Likely`.
