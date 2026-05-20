# WiTec WIP (Raman)

Native binary `.wip` / `.wid` (WiTec Project file) — nirs4all-io now detects
signed `WIT^` and `WIT_PR06` project files. Legacy `WIT^` layouts and unknown
`WIT_PR06` layouts are refused with ASCII-export guidance; the committed
`Sa4.wip` `WIT_PR06` TDGraph layout decodes experimentally into spectra.

The supported decode path remains the **WiTec ASCII export** ("Export Spectrum as Text").

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `Si-wafer-Raman-Spectrum-1.txt` | ~30 KB | [`FAIRmat-NFDI/pynxtools-raman@main/tests/data/witec/`](https://github.com/FAIRmat-NFDI/pynxtools-raman/blob/main/tests/data/witec/Si-wafer-Raman-Spectrum-1.txt) | Apache-2.0 | Silicon-wafer single Raman spectrum, **exported from `Petri-dish-test.wip`** (header reference). Confirms the ASCII export pathway used to bridge `.wip` to open tools. |
| `Sa4.wip` | 19 MB | [`Zenodo 7907659 — "Raman data"`](https://zenodo.org/records/7907659) (`Sa4.wip` from the ZrO2 Raman imaging dataset, DOI `10.5281/zenodo.7907659`) | **ODbL v1.0** | Real WITec Project FIVE 5.2 PLUS `.wip` for sample Sa4 (zirconium-oxide phase analysis). Decodes as 4410 valid TDGraph spectra with 1024 raw-count points each, Raman-shift axis from the embedded excitation wavelength and map coordinates from the TDGraph space transformation; the file remains paired with the linked publication (DOI `10.1016/j.saa.2023.122625`). |

## Parser hints

- ASCII export starts with `// `-prefixed comment header carrying instrument / acquisition metadata (Laser line, Integration time, Map info, etc.), then a 2-column `wavenumber [rel. 1/cm]    Intensity [CCD cts]` block.
- Observed magics: legacy `WIT^` and Project FIVE `WIT_PR06`.
- Current native strategy: decode only the committed `WIT_PR06` TDGraph Sa4 layout, refuse all other binary layouts explicitly, and parse WiTec ASCII exports for broad interchange.
