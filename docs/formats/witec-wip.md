# WiTec WIP / WID

Status: experimental native subset; ASCII exports are covered by the
row-oriented spectral table reader.

## Format

WiTec `.wip` and `.wid` files are binary project containers produced by WiTec
Project / Project FIVE for confocal Raman workflows. They can contain single
spectra, maps, line scans, image/navigation metadata and project-tree objects.
Observed public fixtures include both older `WIT^` signatures and the Project
FIVE `WIT_PR06` signature. The committed `Sa4.wip` fixture is a `WIT_PR06`
project containing a `TDGraph` spectral map.

The practical broad interchange path remains the WiTec ASCII export. The
committed fixture `samples/raman_witec/Si-wafer-Raman-Spectrum-1.txt` is parsed
by `nirs4all_io::readers::spectral_table` as raw CCD counts with a nanometer
axis.

## Implemented

- `.wip` and `.wid` sniffing when the file starts with `WIT^` or `WIT_PR06`;
- definite `witec-wip` probe result for signed binary project files;
- experimental `WIT_PR06` TDGraph decoder for the committed `Sa4.wip` layout:
  `SizeX=90`, `SizeY=55`, `SizeGraph=1024`, `DataType=6`;
- `LineValid` handling so the interrupted acquisition emits 4410 valid spectra
  instead of 4950 physical grid slots;
- strict boolean validation of `LineValid` bytes (`0`/`1`) and diagnostic
  metadata for valid/invalid line counts, physical grid slots and physical
  spectrum index;
- spectral axis reconstruction from the WiTec `FreePolynom` coefficients,
  converted to Raman shift (`cm-1`) from `ExcitationWaveLength`;
- diagnostic metadata for the `FreePolynom` order and bin range used to build
  the wavelength axis before Raman-shift conversion;
- physical map position metadata in micrometers, derived from the TDGraph
  `SpaceTransformationID` and its model/world origin, scale and rotation
  matrices;
- strict refusal for legacy `WIT^` and unknown `WIT_PR06` layouts;
- WiTec ASCII single-spectrum export support through `row-spectral-table`;
- real `Sa4.wip` TDGraph regression tests.

## Missing

- general native binary project-tree parser;
- extraction of arbitrary single spectra, maps, line scans and time series from
  `.wip`;
- typed WiTec metadata normalization for laser, objective, integration time,
  acquisition settings and broader map geometry;
- validation against an ASCII export from the same `Sa4.wip` project;
- conformance reports against external WiTec-capable readers where license and
  fixture terms allow it.

## Validation Notes

Current native validation is intentionally narrow. The `Sa4.wip` test asserts
4410 spectra, 1024 Raman-shift points, the first raw-count values, the
underlying wavelength range, the 532.099 nm excitation wavelength and first/last
map coordinates. It also asserts the diagnostic layout metadata: 4950 physical
grid slots, 49 valid lines, 6 invalid lines and 4410 emitted spectra. The parser
emits `witec_wip_experimental_parser` plus explicit warnings for derived
Raman-shift axis and map coordinates, and must stay experimental until more
WiTec project variants and paired vendor ASCII exports are available.
