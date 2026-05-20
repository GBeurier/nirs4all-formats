# CSV / TSV / generic text

Plain text tabular data. Already supported in `nirs4all.data.loaders.CSVLoader`; `nirs_loader` should add header heuristics to detect the wavelength row, sample-ID column, and reference value columns.

## Samples

All generated locally (CC-0):

| File | Notes |
|---|---|
| `synthetic_nirs.csv` | 50 samples × 200 wavelengths, comma-separated, with `sample_id`, `protein`, and wavelength columns. |
| `synthetic_nirs_semicolon.csv` | Same data with `;` separator (European Excel convention). |
| `synthetic_nirs.tsv` | Tab-separated variant. |
| `idl_envi_output.txt` | Whitespace-separated IDL/ENVI-style text output with `;` comment header lines. |

## Parser hints

- Auto-detect delimiter via `csv.Sniffer` or by counting candidates (`,`, `;`, `\t`, `|`, multiple spaces).
- Wavelength axis: usually either column headers (numeric) or the first column. Heuristic: try parsing the first row as floats — if it parses, the row is wavelengths; else assume it's column names.
- Sample-ID heuristic: first non-numeric column.
- Reference / target columns: typically named `protein`, `moisture`, `fat`, `Y`, `target`, etc. Expose them in `targets`, not `metadata`.
- Locale: be tolerant of `.` vs `,` decimal separators.
