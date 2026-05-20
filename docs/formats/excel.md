# Excel Spectral Tables

Status: experimental.

The Excel reader uses the pure-Rust `calamine` crate. It currently supports
OOXML workbooks (`.xlsx` / `.xlsm`) whose selected worksheet contains:

- one header row;
- numeric wavelength headers for spectral columns;
- optional metadata columns such as `sample_id`;
- optional numeric target columns such as `protein`.

The reader prefers a worksheet named `spectra` and otherwise falls back to the
first worksheet. It emits one `SpectralRecord` per non-empty data row.

## Supported Fixtures

| Fixture | Records | Axis | Signal | Targets |
|---|---:|---|---|---|
| `samples/excel/synthetic_nirs.xlsx` | 50 | wavelength, `nm`, 200 points | `absorbance` | `protein` |

## Dispatch Boundaries

Legacy `.xls` OLE workbooks, multi-sheet metadata/reference layouts and
workbooks where Excel has coerced wavelengths into dates remain pending. The
current reader is intentionally limited to numeric spectral headers so malformed
lab transfers fail clearly instead of silently producing shifted axes.
