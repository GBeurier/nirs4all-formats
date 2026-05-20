# CLI Contract

The CLI binary is `nirs4all-io`.

Current commands:

```bash
nirs4all-io probe path/to/file
nirs4all-io read-json path/to/file
nirs4all-io read-json --rows 10:20 --cols 30:40 path/to/cube.hdr
```

`probe` prints JSON candidate readers with format, reader, confidence and
reason.

`read-json` opens the file through the native Rust registry and prints the
normalized `SpectralRecord` array as JSON. For image cubes, `--rows` and
`--cols` accept half-open `START:END` pixel windows; an omitted end such as
`10:` means "to the cube edge". These options currently apply to ENVI Standard
and ERDAS LAN / AVIRIS cube readers. This command is currently also the
transport used by the Python bridge while the native extension/C ABI bindings
are being filled in.

Planned commands:

- `inspect`: summarize records without dumping arrays;
- `convert`: write Arrow or Parquet;
- `validate`: compare against golden output;
- `bench`: run reader-level performance scenarios.
