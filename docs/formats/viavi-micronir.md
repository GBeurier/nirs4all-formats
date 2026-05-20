# VIAVI MicroNIR

Status: partial.

VIAVI MicroNIR coverage is export-based. Native `.pri` project files remain
customer-only, so the supported path is CSV/XLSX spectral matrices.

## Implemented

- synthetic MicroNIR CSV matrix export through `spectral_matrix`;
- real UvA forensic MicroNIR 1700 `.xlsx` exports through `excel`;
- first-cell axis descriptors such as `axis: wavelength (nm) / data:
  absorbance (a.u.)`;
- sample IDs in the first column promoted to `metadata.sample_id`.

## Supported Fixtures

| Fixture | Records | Axis | Notes |
|---|---:|---|---|
| `samples/viavi_micronir/synthetic_micronir.csv` | 20 | wavelength, `nm`, 200 points | Synthetic CSV matrix export. |
| `samples/viavi_micronir/micronir_forensic_K_avg.xlsx` | 88 | wavelength, `nm`, 125 points | Real MicroNIR 1700 ketamine set. |
| `samples/viavi_micronir/micronir_forensic_T_avg.xlsx` | 71 | wavelength, `nm`, 125 points | Real MicroNIR 1700 THC set. |

## Missing

- native `.pri` project reverse engineering;
- vendor metadata fields beyond the exported worksheet labels;
- paired reference-reader comparison for the native project format.
