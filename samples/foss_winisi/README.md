# Foss NIRSystems / WinISI `.NIR` / `.DA` / `.cal` / `.eqa`

Native `.cal` / `.nir` binaries are now decoded by the reverse-engineered `foss_winisi` reader; the `.DA` / `.eqa` formats are not yet sampled. The WinISI / DA1650 / DS2500 / DS3 text exports remain the fallback where only an export is available.

⚠ **Extension collision** with BUCHI NIRCal `.nir`. Never route by extension alone — sniff the header signature. The native `foss_winisi` reader claims a file when its binary header carries the version word plus either an ASCII `ISIscan` / `NIRSystems` marker (monochromator files) or a `NIRS DS` instrument model at `0x82` (DS-series benches), so WinISI, FOSS DS and BUCHI each keep their own `.nir` payloads.

## Samples

| File | Size | Source | License | Notes |
|---|---|---|---|---|
| `synthetic.cal` | ~2 KB | Generated locally | CC-0 | Synthetic ISIscan/WinISI native `.cal`: 2 samples, 6-pt nm axis, moisture/protein constituent targets; byte-accurate fixture for the native `foss_winisi` reader golden. |
| `synthetic_ds2500.nir` | ~4 KB | Generated locally | CC-0 | Synthetic FOSS **DS2500** native `.nir` (`foss-ds-nir`): 4 samples, two-segment 100-pt nm axis (400 + 1100 nm), `NIRS DS2500` model, spectra-only. CC0 stand-in for the local-only DS2500 customer corpus. |
| `synthetic_ds3f.nir` | ~3 KB | Generated locally | CC-0 | Synthetic FOSS **DS3 F** native `.nir` (`foss-ds-nir`): 3 samples, single-segment 50-pt nm axis (1100 nm), `NIRS DS3 F` model, spectra-only. |
| `synthetic_winisi_export.txt` | ~60 KB | Generated locally | CC-0 | Mock WinISI II calibration text export (header lines + wavelengths + sample matrix with reference value column). Matches the layout described in the WinISI manual. |
| `synthetic_ds3_report.csv` | ~2 KB | Generated locally | CC-0 | Mock DS3 / Inframatic CSV report (instrument, method, sample/protein/moisture/etc.). Matches the layout described in the [DS3 manual p. 45](https://www.manualslib.com/manual/2155011/Foss-Nirs-Ds3.html?page=45). |
| `foss_xds_wheat2_sensAIfood.csv` | 26 KB | [`Zenodo 16759587 — sensAIfood Cordoba`](https://zenodo.org/records/16759587) (`Wheat2_sensAIfood_UnivCordoba.csv`) | **CC-BY-4.0** (CRA-W / Univ. Cordoba, IG19145 sensAIfood) | Real CSV export of 2 wheat samples scanned on a **Foss XDS Monochromator XM-1000** (400-2500 nm). Header layout `ID,Spectrometer,Cereal,Variety,Country,Year,Moisture,Protein,400,402,…,2498` is the canonical Foss XDS / NIRSystems wide CSV. |
| `foss_xds_wheat2_sensAIfood_metadata.xlsx` | 20 KB | Same Zenodo record | CC-BY-4.0 | Accompanying metadata (reference value methods, instrument settings) for the Wheat2 CSV. |
| `foss_xds_barleyground_sensAIfood.csv` | 80 KB | Same Zenodo record (`BarleyGround_sensAIfood_UnivCordoba.csv`) | CC-BY-4.0 | 7 ground-barley samples scanned on the same Foss XDS, same column layout. Useful for batch-of-many-samples regression tests. |

## Parser hints

- Native `.cal` / `.nir` binaries: decoded by the `foss_winisi` reader. NIRSystems/ISIscan monochromator files probe as `foss-winisi-cal` / `foss-winisi-nir`; the newer FOSS DS-series benches (DS2500, DS3 F) share the identical container but probe as `foss-ds-nir` (pinned by the `NIRS DS` model at `0x82`, no ISIscan/NIRSystems string). All are `Definite` confidence. The real vendor `.cal` / `.nir` corpus is **local-only** under `samples_local/foss_winisi/` (private, licence TBD), validated against the ISIscan `.txt` exports where available; only the `synthetic.*` fixtures above are redistributable. The `.DA` / `.eqa` binaries are not yet sampled.
- WinISI text export: header section followed by a matrix block. Field separator is typically whitespace.
- DS3/Inframatic CSV report: standard CSV, but the header carries instrument and method metadata that should be parsed into the `metadata` dict.
- Foss XDS wide CSV (sensAIfood fixtures): metadata columns first (`ID,Spectrometer,Cereal,Variety,Country,Year,Moisture,Protein`), then ~1050 reflectance columns whose **header values are the wavelengths in nm** (400 → 2498 at 2 nm step for XDS; 1100 → 2498 for NIRSYSTEM-5000). The loader's "first numeric column-name = wavelength" heuristic should classify everything before it as metadata/targets.
- If you need a real binary `.NIR` sample for parser development, vendors sometimes provide them upon request; meanwhile use the synthetic fixtures here for structural tests, and the sensAIfood CSVs above for the real-text-export ingestion path.
