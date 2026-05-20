# Foss NIRSystems / WinISI `.NIR` / `.DA` / `.cal` / `.eqa`

Native binary has **no open reader**. v1 strategy: ingest WinISI / DA1650 / DS2500 / DS3 text exports only.

⚠ **Extension collision** with BUCHI NIRCal `.nir`. Never route by extension alone — sniff the header signature.

## Samples

| File | Size | Source | License |
|---|---|---|---|
| `synthetic_winisi_export.txt` | ~60 KB | Generated locally | CC-0 | Mock WinISI II calibration text export (header lines + wavelengths + sample matrix with reference value column). Matches the layout described in the WinISI manual. |
| `synthetic_ds3_report.csv` | ~2 KB | Generated locally | CC-0 | Mock DS3 / Inframatic CSV report (instrument, method, sample/protein/moisture/etc.). Matches the layout described in the [DS3 manual p. 45](https://www.manualslib.com/manual/2155011/Foss-Nirs-Ds3.html?page=45). |

## Parser hints

- Native `.NIR` / `.DA` / `.cal` / `.eqa` binaries: **no reliable open reader**. Recommend documenting "vendor SDK only" and providing a clear error.
- WinISI text export: header section followed by a matrix block. Field separator is typically whitespace.
- DS3/Inframatic CSV report: standard CSV, but the header carries instrument and method metadata that should be parsed into the `metadata` dict.
- If you need a real `.NIR` sample for parser development, vendors sometimes provide them upon request; meanwhile use the synthetic fixtures here for structural tests.
