# WiTec WIP / WID

> **Status:** Partial · **Vendor:** WiTec · **Extensions:** `.wip`, `.wid`

WiTec `.wip` and `.wid` files are binary project containers written by WiTec
Project / Project FIVE for confocal Raman workflows. They can hold single
spectra, maps, line scans, image/navigation metadata and project-tree objects.
nirs4all-io decodes one observed Project FIVE map layout natively; the broad
interchange path remains the WiTec ASCII export, handled by the
[row-oriented spectral table reader](row-spectral-table.md). The format is Raman,
adjacent to the core NIRS point-spectrum scope.

## Instruments & software

Produced by WiTec Project and Project FIVE for WiTec confocal Raman microscopes.
Public fixtures show both the older `WIT^` signature and the Project FIVE
`WIT_PR06` signature; the committed `Sa4.wip` fixture is a `WIT_PR06` project
containing a `TDGraph` spectral map.

## File structure

The reader sniffs `.wip`/`.wid` by the leading project signature (`WIT^` or
`WIT_PR06`) and emits a `Definite` probe for both. Only the `WIT_PR06` TDGraph
layout is decoded:

- the supported geometry `SizeX=90`, `SizeY=55`, `SizeGraph=1024`, `DataType=6`
  (unsigned 16-bit);
- `LineValid` flags, validated strictly as boolean (`0`/`1`) bytes, so an
  interrupted acquisition emits its valid spectra instead of all physical grid
  slots;
- the WiTec `FreePolynom` calibration coefficients, used to reconstruct a
  wavelength axis;
- the TDGraph `SpaceTransformationID` model/world origin, scale and rotation
  matrices, used to derive physical map positions.

## What nirs4all-io extracts

- **Signals** — one `SpectralRecord` per valid spectrum, each with a single
  `raw_counts` signal typed `RawCounts` (raw CCD counts).
- **Axis** — the `FreePolynom` wavelength axis converted to Raman shift (`cm-1`)
  using the `ExcitationWaveLength` (532.099 nm in the fixture).
- **Metadata** — physical map positions in micrometres; diagnostic counts for
  valid/invalid lines, physical grid slots and physical spectrum index; and the
  `FreePolynom` order and bin range used before Raman-shift conversion.
- **Provenance & warnings** — every record carries
  `witec_wip_experimental_parser`, plus explicit warnings for the layout limit,
  the derived Raman-shift axis and the derived map coordinates.
- **ASCII export** — WiTec single-spectrum ASCII exports decode through
  `row-spectral-table` as raw CCD counts with a nanometre axis.

## Variants & support status

| Variant | Status | Notes |
|---|---|---|
| `WIT_PR06` TDGraph map (`Sa4.wip` geometry) | Experimental | The one decoded native layout; strict `LineValid`, Raman-shift axis, physical coordinates. |
| WiTec ASCII single-spectrum export | Supported | Routed to the row-oriented spectral table reader. |
| Legacy `WIT^` projects | Detected / refused | Sniffed but refused with an explicit unsupported-layout error. |
| Other `WIT_PR06` layouts / project-tree objects | Detected / refused | General single spectra, maps, line scans and time series not yet extracted. |

## Limitations & known gaps

- There is no general native project-tree parser; only the single `WIT_PR06`
  TDGraph geometry above is decoded, and all other native layouts (including
  legacy `WIT^`) are refused rather than guessed.
- Typed WiTec metadata (laser, objective, integration time, acquisition
  settings, broader map geometry) is not yet normalised.
- A paired ASCII export from the same `Sa4.wip` project, and conformance reports
  against external WiTec-capable readers, are still wanted.

## Reference readers

Layout cross-checks reference `pynxtools-raman`, `hySpc.read.Witec` and the
`LabberI2A` WIPfile loader.

## Samples & validation

The native path is validated by `samples/raman_witec/Sa4.wip` (Zenodo 7907659,
ODbL v1.0): the regression test asserts 4410 emitted spectra, 1024 Raman-shift
points, the first raw-count values, the underlying wavelength range, the 532.099 nm
excitation wavelength and first/last map coordinates, plus the diagnostic layout
metadata (4950 physical grid slots, 49 valid lines, 6 invalid lines, 4410 emitted
spectra). The ASCII path uses `samples/raman_witec/Si-wafer-Raman-Spectrum-1.txt`.
The reader stays experimental until more WiTec project variants and paired vendor
ASCII exports are available.
