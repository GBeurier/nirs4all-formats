# Excel Spectral Tables

Status: experimental.

The Excel reader uses the pure-Rust `calamine` crate. It currently supports
OOXML workbooks (`.xlsx` / `.xlsm`) whose selected worksheet contains:

- one header row;
- numeric wavelength headers for spectral columns;
- optional metadata columns such as `sample_id`;
- optional numeric target columns such as `protein`;
- optional first-cell descriptors such as `axis: wavelength (nm) / data:
  absorbance (a.u.)`, where the first column contains sample IDs.

The reader prefers a worksheet named `spectra` and otherwise falls back to the
first worksheet. It emits one `SpectralRecord` per non-empty data row.

If optional worksheets named `metadata` and/or `references` are present, they
are joined to the spectral rows by `sample_id`. `metadata` columns are copied to
record metadata, while numeric `references` columns are copied to targets.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/excel/synthetic_nirs.xlsx` | 50 | wavelength, `nm`, 200 points | `absorbance` | `protein` |
| `samples/excel/synthetic_multisheet_nirs.xlsx` | 4 | wavelength, `nm`, 4 points | `absorbance` | `protein`, `moisture` |
| `samples/excel/nirone_forensic_T_avg.xlsx` | 71 | wavelength, `nm`, 201 points | `absorbance` | none |
| `samples/excel/scio_forensic_P_avg.xlsx` | 71 | wavelength, `nm`, 331 points | `raw` | none |
| `samples/siware_neospectra/neospectra_forensic_K_avg.xlsx` | 88 | wavelength, `nm`, 160 points | `absorbance` | none |
| `samples/viavi_micronir/micronir_forensic_K_avg.xlsx` | 88 | wavelength, `nm`, 125 points | `absorbance` | none |
| `samples/viavi_micronir/micronir_forensic_T_avg.xlsx` | 71 | wavelength, `nm`, 125 points | `absorbance` | none |

## Dispatch Boundaries

Legacy `.xls` OLE workbooks, caller-selected non-canonical sheet names and
workbooks where Excel has coerced wavelengths into dates remain pending. The
current reader is intentionally limited to numeric spectral headers so malformed
lab transfers fail clearly instead of silently producing shifted axes.
