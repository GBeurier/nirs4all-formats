# WiTec WIP / WID

Status: detected and refused; ASCII exports are covered by the row-oriented
spectral table reader.

## Format

WiTec `.wip` and `.wid` files are binary project containers produced by WiTec
Project / Project FIVE for confocal Raman workflows. They can contain single
spectra, maps, line scans, image/navigation metadata and project-tree objects.
Public reverse-engineering notes indicate a `WIT^` magic at the beginning of
the binary container, but there is no redistributable fixture in this repository
yet.

The practical open interchange path is the WiTec ASCII export. The committed
fixture `samples/raman_witec/Si-wafer-Raman-Spectrum-1.txt` is parsed by
`nirs4all_io::readers::spectral_table` as raw CCD counts with a nanometer axis.

## Implemented

- `.wip` and `.wid` sniffing when the file starts with `WIT^`;
- definite `witec-wip` probe result for signed binary project files;
- explicit refusal on read with guidance to export spectra from WiTec
  Project/FIVE as ASCII text;
- WiTec ASCII single-spectrum export support through `row-spectral-table`;
- synthetic probe/read-refusal tests, because no redistributable binary WIP
  fixture is available.

## Missing

- native binary project-tree parser;
- extraction of single spectra, maps, line scans and time series from `.wip`;
- typed WiTec metadata normalization for laser, objective, integration time,
  positions and map geometry;
- validation against a real `.wip` / `.wid` fixture with redistribution or
  private-test permission;
- conformance reports against external WiTec-capable readers where license and
  fixture terms allow it.

## Validation Notes

Current validation is intentionally limited to signature detection and refusal.
Native decoding must not be promoted until a real project file is available and
the decoded spectra can be compared with an ASCII export from the same project.
