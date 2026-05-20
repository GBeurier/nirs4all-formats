# Si-Ware NeoSpectra

Status: partial.

NeoSpectra data currently enters through open export layouts rather than a
native project container. The reader coverage is split across generic tabular
readers:

- `spectral_table` for axis-first CSV exports;
- `csv_like` for wide OSSL CSV matrices with metadata/targets before numeric
  wavelength headers;
- `excel` for first-row wavelength `.xlsx` exports with an axis/data descriptor
  in the first cell.

## Implemented

- synthetic NeoSpectra-style CSV export;
- real OSSL Woodwell/KSSL wide CSV slice with `id.layer_uuid_txt` promoted to
  `metadata.sample_id`;
- real UvA forensic NeoSpectra `.xlsx` export with `axis: wavelength (nm) /
  data: absorbance (a.u.)`;
- preservation of soil/site laboratory properties as targets when cells are
  numeric.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/siware_neospectra/synthetic_neospectra.csv` | 1 | wavelength, `nm`, 200 points | Axis-first synthetic export. |
| `samples/siware_neospectra/neospectra_ossl_50samples_slice.csv` | 24 | wavelength, `nm`, 601 points | Real OSSL slice; empty rows are ignored. |
| `samples/siware_neospectra/neospectra_forensic_K_avg.xlsx` | 88 | wavelength, `nm`, 160 points | Real UvA forensic averaged spectra. |

## Missing

- a native single-measurement NeoSpectra Scanner export fixture;
- broader validation across all OSSL scanner/replicate blocks;
- typed normalization of soil chemistry target names.

