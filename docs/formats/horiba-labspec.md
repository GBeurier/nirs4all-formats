# Horiba LabSpec / JobinYvon

> **Status:** Partial · **Vendor:** Horiba (Jobin Yvon) · **Extensions:** `.xml`, `.txt`, `.l6m`

Horiba's LabSpec software (and the older Jobin Yvon line) drives Raman
microscopes and writes both interchange exports (LSX XML, LabSpec text) and the
native LabSpec 6 binary container. nirs4all-io reads the XML and text exports and
one experimental LabSpec 6 `.l6m` map layout. The format is Raman, adjacent to
the core NIRS point-spectrum scope, and is supported for spectroscopy
interchange and ML-workflow compatibility.

## Instruments & software

Written by Horiba LabSpec and legacy Jobin Yvon / LabRam software for Raman
acquisitions — single spectra, parameter ranges, line scans, maps and time
series.

## File structure

The reader sniffs by extension plus content:

- **LSX XML** (`.xml`) — JobinYvon export with `<LSX_Data>`, `<LSX_Tree>` and
  `<LSX_Matrix>` payloads; spectra, ranges, linescans and maps.
- **LabSpec text** (`.txt`) — `#`-prefixed metadata headers followed by a
  two-column, series-row or map-row numeric section.
- **LabSpec 6 binary** (`.l6m`) — detected by the `LabSpec6` magic; the reader
  scans the container for the main `Intens` float32 payload, the `Spectr` axis
  payload and the matching 2D spatial axes.

## What nirs4all-io extracts

- **Signals** — one `SpectralRecord` per spectrum or map pixel, with a single
  `intensity` signal typed `RawCounts`, keeping the native spectral axis order.
- **Axis** — units normalised conservatively: `nm` → wavelength; `1/cm`, `cm-1`
  and corrupted `cm-?` headers → wavenumber (`cm-1`); `eV` → energy. Text exports
  that omit the axis unit default to `cm-1` with warning
  `horiba_labspec_text_axis_unit_inferred`.
- **Metadata** — for XML and text maps/linescans, `spatial_x`, `spatial_y` and
  their unit fields are stored in metadata.
- **Provenance & warnings** — the `.l6m` path emits
  `horiba_labspec6_binary_experimental` on every record because it is validated
  against a single public fixture.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| LSX XML single spectrum / range | Supported | `nm`, `cm-1` and `eV` axes; range axis ascending. |
| LSX XML linescan / map | Supported | One record per point; spatial coordinates in metadata. |
| LabSpec two-column / series-row text | Supported | Legacy LabRam and modern LabSpec exports. |
| LabSpec time series-row text | Supported | One record per row. |
| LabSpec 6 `.l6m` map | Experimental | One observed layout, validated against the paired text export. |
| LabSpec 6 `.l6s` single-spectrum, other LabSpec 6 layouts, calibration files | Not yet supported | Pending redistributable samples. |

## Limitations & known gaps

- Native binary support is deliberately narrow: only one `.l6m` map layout is
  decoded. `.l6s` single-spectrum files and other LabSpec 6 layouts/calibration
  files are not handled until redistributable fixtures exist.
- Full LabSpec 6 metadata is not yet promoted into typed fields.
- Full-array conformance is planned as isolated jobs; the runtime imports no
  GPL reference-reader code.

## Reference readers

Cross-checked against `rsciio.jobinyvon` (JobinYvon XML),
`spectrochempy.read_labspec()` (LabSpec text) and `ccoverstreet/horiba-raman`
(LabSpec 6 mapping text and the paired `.l6m` fixture).

## Samples & validation

Fixtures under `samples/raman_horiba/` cover LSX XML spectra/range/linescan/map
(`nm`, `cm-1`, `eV` axes) and LabSpec two-column, series-row, time-series and
LabSpec 6 map-row text exports, all golden-backed. The experimental
`AlN_Gd2O3_indepth.l6m` (72 records, `cm-1`, 498 points) is compared cell-for-cell
against the paired `labspec6_Gd2O3_AlN_map.txt`: intensities match exactly,
spectral axes match within text-rounding tolerance, and `spatial_x`/`spatial_y`
follow the same x-slowest/y-fastest map order. XML range and linescan branches
also carry semantic tests beyond the golden summaries.
