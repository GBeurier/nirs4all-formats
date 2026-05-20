# Row-Oriented Spectral Tables

Status: experimental.

This reader covers text exports where each data row is one spectral sample
point and the first column is the spectral axis. It complements the existing
`csv_like` reader, which handles the opposite orientation: one spectrum per row
with numeric spectral headers.

## Supported Layouts

The Rust reader `nirs4all_io::readers::spectral_table` accepts `.csv`, `.tsv`,
`.txt`, `.dat`, `.asc`, `.SPT` and `.SPU` when the content has either:

- an explicit axis header such as `Wavelength_nm`, `WAVELENGTH_um`,
  `Wavelength`, `X-Axis` or `wavenumber`;
- a comment-prefixed axis header such as `; Wavelength S000 S001`; or
- metadata that describes `First Column: X` / `X Units` before the numeric
  block, including JASCO-style `XUNITS` / `YUNITS` followed by `XYDATA`.

It emits one `SpectralRecord` with one signal per numeric column after the
axis. The native axis order is preserved.

## Current Fixtures

| Fixture | Parsed signals | Notes |
|---|---|---|
| `samples/siware_neospectra/synthetic_neospectra.csv` | `absorbance` | CSV export with `#` metadata. |
| `samples/modtran/synthetic_albedo.dat` | `albedo` | Whitespace text, wavelength in `um`, albedo mapped to reflectance. |
| `samples/pp_systems/synthetic_unispec.SPT` | `dn_white`, `dn_target`, `reflectance` | UniSpec SC style export. |
| `samples/pp_systems/synthetic_unispec_dc.SPU` | `channel_a_dn`, `channel_b_dn`, `reflectance` | UniSpec DC style export. |
| `samples/envi_sli/ecostress_b.spectrum.txt`, `ecostress_a.spectrum.txt`, `aster_granite.spectrum.txt` | `reflectance` | ECOSTRESS / ASTER / ENVI text spectra with metadata-described columns. |
| `samples/csv_tsv/idl_envi_output.txt` | `s000` ... `s004` | IDL/ENVI comment-prefixed header. |
| `samples/shimadzu/synthetic_uvprobe.txt` | `sample_s000` | UVProbe-style quoted CSV export; signal type remains unknown because the header only says sample. |
| `samples/jasco/synthetic_jws_export.txt` | `absorbance` | JASCO text export with `XYDATA`. |
| `samples/specpr/asphalt_gds366.27407.asc` | `reflectance`, `standard_deviation` | USGS SPECPR ASCII export, wavelength in `um`. |
| `samples/raman_witec/Si-wafer-Raman-Spectrum-1.txt` | `spectrum__000__spec_data_1` | WiTec ASCII export with a unit row; parsed as raw CCD counts. |

## Dispatch Boundaries

The sniffer is intentionally content-based. It does not claim matrix-style
calibration tables, CSV reports, target-only exports, or arbitrary two-column
CSV files without an axis header. The Ocean Optics generic two-column CSV
reader remains responsible for committed headerless Ocean Optics CSV exports.

## Limitations

- Single-column spectral libraries are not parsed by this reader. The legacy
  USGS `AREF` one-column fixture is handled by the dedicated
  `usgs-aref-single-column` reader with a generated index axis because the file
  does not embed wavelengths.
- Deleted-value sentinels are preserved numerically for now; masking policy is
  still pending in the shared data model.
- Vendor-specific metadata is preserved under `metadata.vendor`, but it is not
  normalized into typed fields yet.
