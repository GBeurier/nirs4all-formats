# CLI Contract

The CLI binary is `nirs4all-io`.

Current commands:

```bash
nirs4all-io probe path/to/file
nirs4all-io read-json path/to/file
```

`probe` prints JSON candidate readers with format, reader, confidence and
reason.

`read-json` opens the file through the native Rust registry and prints the
normalized `SpectralRecord` array as JSON. This is currently also the transport
used by the Python bridge while the native extension/C ABI bindings are being
filled in.

Planned commands:

- `inspect`: summarize records without dumping arrays;
- `convert`: write Arrow or Parquet;
- `validate`: compare against golden output;
- `bench`: run reader-level performance scenarios.
