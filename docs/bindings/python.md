# Python Binding

Python bindings are thin wrappers over the Rust core. Parser logic must stay in
Rust.

Current experimental surfaces:

- `open_records(path)`: returns normalized Rust records as Python dictionaries;
- `open_dataset(path, signal=None)`: collapses compatible records into `NirsDataset`;
- `to_numpy_matrix(dataset)`: returns `(X, wavelengths, targets)`;
- `to_pandas_frame(dataset)`: returns one metadata/target/spectral DataFrame;
- `to_sklearn_bunch(dataset, target=None)`: returns a scikit-learn-style `Bunch`;
- `SklearnDatasetProvider(path, target=None)`: small provider wrapper for sklearn examples;
- `TorchSpectralDataset(dataset, target=None)`: torch `Dataset` adapter;
- `to_nirs4all_spectrodataset(dataset, ...)`: fills `nirs4all.data.SpectroDataset`.

Temporary transport:

- the bridge calls `nirs4all-io read-json`;
- `NIRS4ALL_IO_CLI` can point to a prebuilt binary;
- in a source checkout it falls back to `cargo run -p nirs4all-io-cli`.

Planned native surfaces:

- PyO3 or C ABI backed wheel, replacing subprocess transport;
- wheel builds through GitHub Actions after the first readers are stable.
