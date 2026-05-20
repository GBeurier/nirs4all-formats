# Perten DA / Inframatic

Vendor-proprietary native format; CSV export is the practical path. Field-feed analyzer where spectra are typically already pre-calibrated, so reports often contain only the property predictions (moisture, protein, …) and not the raw spectrum.

## Samples

| File | Source | License |
|---|---|---|
| `synthetic_perten.csv` | Generated locally | CC-0 | Mock Perten DA 7250 wheat-NIR report with `SampleID,Date,Time,Operator,Moisture,Protein,Starch,Hardness,Test_Weight` columns. |

## Parser hints

- CSV with a small header block (method, date) before the column-name row.
- This is a **property-only** export — when ingesting it, the loader should make the absence of a spectrum explicit, not silently produce empty `signals`.
- For the Inframatic / DA 7250 with spectra-included exports, expect an additional wavelength-block column set after the property columns.
- **No publicly redistributable real samples** were found.
