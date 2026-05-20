# R Binding

The R package exposes the Rust core with R-native ergonomics. Parser logic must
stay in Rust.

Current experimental surfaces:

- `nirs4allio_open_records(path)`: returns normalized Rust records as R lists;
- `nirs4allio_open_dataset(path, signal=NULL)`: returns an `nirs4allio_dataset`;
- `as.matrix(dataset)`: returns the spectral matrix;
- `as.data.frame(dataset)`: returns sample IDs, targets and spectral columns;
- `nirs4allio_as_tibble(dataset)`: optional tibble conversion.

Temporary transport:

- the bridge calls `nirs4all-io read-json`;
- `NIRS4ALL_IO_CLI` can point to a prebuilt binary;
- in a source checkout it falls back to `cargo run -p nirs4all-io-cli`.

Planned native surfaces:

- C ABI backed package path replacing subprocess transport;
- target extraction helpers;
- local package build before registry publication.
