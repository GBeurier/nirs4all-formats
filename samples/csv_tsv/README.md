# CSV / TSV / generic text

Plain text tabular data. Already supported in `nirs4all.data.loaders.CSVLoader`; `nirs_loader` should add header heuristics to detect the wavelength row, sample-ID column, and reference value columns.

## Samples

| File | Source | License | Notes |
|---|---|---|---|
| `synthetic_nirs.csv` | Generated locally | CC-0 | 50 samples × 200 wavelengths, comma-separated, with `sample_id`, `protein`, and wavelength columns. |
| `synthetic_nirs_semicolon.csv` | Generated locally | CC-0 | Same data with `;` separator (European Excel convention). |
| `synthetic_nirs.tsv` | Generated locally | CC-0 | Tab-separated variant. |
| `idl_envi_output.txt` | Generated locally | CC-0 | Whitespace-separated IDL/ENVI-style text output with `;` comment header lines. |
| `auroranir_handheld_barley_sensAIfood.csv` | [`Zenodo 15838272 — sensAIfood Grainit`](https://zenodo.org/records/15838272) (`Barley_sensAIfood_Grainit.csv`) | **CC-BY-4.0** (CRA-W / Grainit, IG19145 sensAIfood) | Real handheld **AuroraNIR** (Innovative Optical Sensing Solutions / NIR.Industries) acquisition: 86 barley samples × 700 wavelengths (950-1650 nm at 1 nm) with `ID,Spectrometer,Cereal,Country,Year,Moisture,Protein` metadata columns. Exercises the "miniaturised handheld with sub-2-µm range" loader path. |
| `auroranir_handheld_barley_sensAIfood_metadata.xlsx` | Same Zenodo record | CC-BY-4.0 | Reference-value methods + instrument settings for the barley CSV above. |

## Parser hints

- Auto-detect delimiter via `csv.Sniffer` or by counting candidates (`,`, `;`, `\t`, `|`, multiple spaces).
- Wavelength axis: usually either column headers (numeric) or the first column. Heuristic: try parsing the first row as floats — if it parses, the row is wavelengths; else assume it's column names.
- Sample-ID heuristic: first non-numeric column.
- Reference / target columns: typically named `protein`, `moisture`, `fat`, `Y`, `target`, etc. Expose them in `targets`, not `metadata`.
- Locale: be tolerant of `.` vs `,` decimal separators.
- The sensAIfood CSVs (`auroranir_handheld_barley_sensAIfood.csv` here, the Foss XDS variants in `foss_winisi/`) share a common header convention: `ID,Spectrometer,Cereal,Variety,Country,Year,<targets…>,<wavelengths…>` — recognise it as a "metadata + targets + wide spectra" layout where the boundary is the first column-name that parses as a float.
