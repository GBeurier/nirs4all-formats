# Parquet (`.parquet`)

Columnar binary format. Used as the internal cache format by `nirs4all`.
Supported by `nirs4all.data.loaders.ParquetLoader` and by the native Rust
`nirs4all-formats` Parquet reader for canonical NIRS tables.

## Samples

| File | Size | Source | License |
|---|---|---|---|
| `synthetic_nirs.parquet` | ~50 KB | Generated locally | CC-0 | Same content as the CSV fixture (50 × 200), Zstd-compressed. |
| `alltypes_plain.parquet` | 1.8 KB | [`apache/parquet-testing@master/data/alltypes_plain.parquet`](https://github.com/apache/parquet-testing/blob/master/data/alltypes_plain.parquet) | Apache-2.0 | Canonical "all types" Parquet test fixture from the Apache project — useful for negative-path tests (when a Parquet file does **not** contain spectra). |

## Parser hints

- Reference readers: `pyarrow.parquet`, `fastparquet`, `pandas.read_parquet`.
- The loader detects "this is a NIRS table" vs "this is some other Parquet file" by inspecting column names and dtypes.
