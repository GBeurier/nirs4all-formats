# WiTec WIP (Raman)

Native binary `.wip` / `.wid` (WiTec Project file) — nirs4all-io now detects
signed `WIT^` project files and refuses them with ASCII-export guidance, but no
native parser ships yet because no permissively-licensed binary sample is
available.

The workflow path that does work is the **WiTec ASCII export** ("Export Spectrum as Text"), which is what we ship here.

## Samples

| File | Source | License |
|---|---|---|
| `Si-wafer-Raman-Spectrum-1.txt` | [`FAIRmat-NFDI/pynxtools-raman@main/tests/data/witec/Si-wafer-Raman-Spectrum-1.txt`](https://github.com/FAIRmat-NFDI/pynxtools-raman/blob/main/tests/data/witec/Si-wafer-Raman-Spectrum-1.txt) | Apache-2.0 | Silicon-wafer single Raman spectrum, **exported from `Petri-dish-test.wip`** (header reference). Confirms the ASCII export pathway used to bridge `.wip` to open tools. |

## Parser hints

- ASCII export starts with `// `-prefixed comment header carrying instrument / acquisition metadata (Laser line, Integration time, Map info, etc.), then a 2-column `wavenumber [rel. 1/cm]    Intensity [CCD cts]` block.
- Magic of a real `.wip` (for refusal-path detection): bytes 0-3 are `57 49 54 5e` (`WIT^`) per public reverse-engineering notes. The repository validates this only with a synthetic signature test until a redistributable binary fixture exists.
- v1 strategy: refuse `.wip/.wid` with an explicit error pointing users to WiTec Project FIVE's "Export Spectrum as Text" command, and parse the ASCII export.
