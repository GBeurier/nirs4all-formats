# Foss NIRSystems / WinISI `.NIR` / `.DA` / `.cal` / `.eqa`

Native binary has **no open reader**. v1 strategy: ingest WinISI / DA1650 / DS2500 / DS3 text exports only.

⚠ **Extension collision** with BUCHI NIRCal `.nir`. Never route by extension alone — sniff the header signature.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `synthetic_winisi_export.txt` | ~60 KB | Generated locally | CC-0 | Mock WinISI II calibration text export (header lines + wavelengths + sample matrix with reference value column). Matches the layout described in the WinISI manual. |
| `synthetic_ds3_report.csv` | ~2 KB | Generated locally | CC-0 | Mock DS3 / Inframatic CSV report (instrument, method, sample/protein/moisture/etc.). Matches the layout described in the [DS3 manual p. 45](https://www.manualslib.com/manual/2155011/Foss-Nirs-Ds3.html?page=45). |
| `foss_xds_wheat2_sensAIfood.csv` | 26 KB | [`Zenodo 16759587 — sensAIfood Cordoba`](https://zenodo.org/records/16759587) (`Wheat2_sensAIfood_UnivCordoba.csv`) | **CC-BY-4.0** (CRA-W / Univ. Cordoba, IG19145 sensAIfood) | Real CSV export of 2 wheat samples scanned on a **Foss XDS Monochromator XM-1000** (400-2500 nm). Header layout `ID,Spectrometer,Cereal,Variety,Country,Year,Moisture,Protein,400,402,…,2498` is the canonical Foss XDS / NIRSystems wide CSV. |
| `foss_xds_wheat2_sensAIfood_metadata.xlsx` | 20 KB | Same Zenodo record | CC-BY-4.0 | Accompanying metadata (reference value methods, instrument settings) for the Wheat2 CSV. |
| `foss_xds_barleyground_sensAIfood.csv` | 80 KB | Same Zenodo record (`BarleyGround_sensAIfood_UnivCordoba.csv`) | CC-BY-4.0 | 7 ground-barley samples scanned on the same Foss XDS, same column layout. Useful for batch-of-many-samples regression tests. |

## Parser hints

- Native `.NIR` / `.DA` / `.cal` / `.eqa` binaries: **no reliable open reader**. Recommend documenting "vendor SDK only" and providing a clear error.
- WinISI text export: header section followed by a matrix block. Field separator is typically whitespace.
- DS3/Inframatic CSV report: standard CSV, but the header carries instrument and method metadata that should be parsed into the `metadata` dict.
- Foss XDS wide CSV (sensAIfood fixtures): metadata columns first (`ID,Spectrometer,Cereal,Variety,Country,Year,Moisture,Protein`), then ~1050 reflectance columns whose **header values are the wavelengths in nm** (400 → 2498 at 2 nm step for XDS; 1100 → 2498 for NIRSYSTEM-5000). The loader's "first numeric column-name = wavelength" heuristic should classify everything before it as metadata/targets.
- If you need a real binary `.NIR` sample for parser development, vendors sometimes provide them upon request; meanwhile use the synthetic fixtures here for structural tests, and the sensAIfood CSVs above for the real-text-export ingestion path.
