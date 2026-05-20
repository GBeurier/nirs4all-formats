# WiTec WIP (Raman)

Native binary `.wip` / `.wid` (WiTec Project file) — nirs4all-io now detects
signed `WIT^` project files and refuses them with ASCII-export guidance. One
real `.wip` fixture is now committed (see below) so the detection magic and
header parsing can be validated against a true vendor binary.

The supported decode path remains the **WiTec ASCII export** ("Export Spectrum as Text").

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `Si-wafer-Raman-Spectrum-1.txt` | ~30 KB | [`FAIRmat-NFDI/pynxtools-raman@main/tests/data/witec/`](https://github.com/FAIRmat-NFDI/pynxtools-raman/blob/main/tests/data/witec/Si-wafer-Raman-Spectrum-1.txt) | Apache-2.0 | Silicon-wafer single Raman spectrum, **exported from `Petri-dish-test.wip`** (header reference). Confirms the ASCII export pathway used to bridge `.wip` to open tools. |
| `Sa4.wip` | 19 MB | [`Zenodo 7907659 — "Raman data"`](https://zenodo.org/records/7907659) (`Sa4.wip` from the ZrO₂ Raman imaging dataset, DOI `10.5281/zenodo.7907659`) | **ODbL v1.0** | Real WITec Project FIVE 5.2 PLUS `.wip` for sample Sa4 (zirconium-oxide phase analysis). Lets the magic-byte detector and header-decode routines run against a genuine vendor binary; the file remains paired with the linked publication (DOI `10.1016/j.saa.2023.122625`). |

## Parser hints

- ASCII export starts with `// `-prefixed comment header carrying instrument / acquisition metadata (Laser line, Integration time, Map info, etc.), then a 2-column `wavenumber [rel. 1/cm]    Intensity [CCD cts]` block.
- Magic of a real `.wip` (for refusal-path detection): bytes 0-3 are `57 49 54 5e` (`WIT^`) per public reverse-engineering notes. The committed `Sa4.wip` confirms this signature in the wild; the repository's signature test now matches both a synthetic and the real file.
- v1 strategy: refuse `.wip/.wid` with an explicit error pointing users to WiTec Project FIVE's "Export Spectrum as Text" command, and parse the ASCII export. A future native decoder can use `Sa4.wip` as its primary fixture.
