# Parquet NIRS Tables

Native Rust reader for canonical tabular NIRS datasets stored as Apache
Parquet.

## Scope Implemented

- Sniffs `.parquet` files by `PAR1` magic.
- Opens Parquet through Arrow with Zstd support.
- Accepts tables whose spectral columns are named by numeric wavelength values
  and whose column types are `float32` or `float64`.
- Requires at least 8 spectral columns and a strictly ascending axis to avoid
  false positives on generic Parquet files.
- Maps numeric non-spectral columns to targets.
- Maps `sample_id`, `sample` or `id` string columns to metadata
  `sample_id`.
- Refuses non-spectral Parquet files, including the Apache
  `alltypes_plain.parquet` fixture.

## Record Mapping

- one `SpectralRecord` per table row;
- signal name: `absorbance`;
- signal type: `absorbance`;
- axis: wavelength, `nm`, from numeric column names;
- targets: numeric non-spectral columns, for example `protein`;
- metadata: `sample_id`, `row_index` and a `parquet` summary object.

## Fixtures and Reference Checks

Committed fixtures:

| File | Expected output |
|---|---|
| `samples/parquet/synthetic_nirs.parquet` | 50 records, 200 wavelength columns, target `protein` |
| `samples/parquet/alltypes_plain.parquet` | refused as non-spectral |

Reference readers: `pyarrow.parquet`, `fastparquet`,
`pandas.read_parquet`, and `nirs4all.data.loaders.ParquetLoader`.

## Missing / Next Work

- Add schema metadata support for explicit units and signal type.
- Add compression variants beyond the committed Zstd fixture as needed.
- Add projection-based reading for very wide Parquet datasets if performance
  requires it.
